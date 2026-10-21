import { describe, expect, it } from "vitest";

import type { SeriesInfo, SeriesPoints } from "../ipc/types";
import {
  alignSeries,
  formatElapsed,
  latestValues,
  planChart,
} from "./plan";

/** A series, for the tests. */
function series(name: string, unit: string): SeriesInfo {
  return { name, unit };
}

/** The values of a series, for the tests. */
function points(
  name: string,
  unit: string,
  x: readonly number[],
  y: readonly number[],
): SeriesPoints {
  return { name, unit, x, y };
}

describe("the plan of a graph", () => {
  it("puts the series of one unit on one axis", () => {
    const plan = planChart([
      series("Roll", "°"),
      series("Pitch", "°"),
    ]);

    expect(plan.scales).toHaveLength(1);
    expect(plan.scales[0]?.key).toBe("y");
    expect(plan.scales[0]?.side).toBe("left");
    expect(plan.series[0]?.scale).toBe("y");
    expect(plan.series[1]?.scale).toBe("y");
  });

  it("gives a series of another unit an axis of its own", () => {
    const plan = planChart([
      series("Voltage", "V"),
      series("Remaining", "%"),
    ]);

    expect(plan.scales).toHaveLength(2);
    expect(plan.scales[0]?.unit).toBe("V");
    expect(plan.scales[1]?.unit).toBe("%");
    expect(plan.scales[1]?.side).toBe("right");
    expect(plan.series[0]?.scale).toBe("y");
    expect(plan.series[1]?.scale).toBe("y2");
  });

  it("shares one axis between the series of the same unit", () => {
    const plan = planChart([
      series("Voltage", "V"),
      series("Current", "A"),
      series("Another voltage", "V"),
    ]);

    expect(plan.scales).toHaveLength(2);
    expect(plan.series[2]?.scale).toBe("y");
  });

  it("gives every series a colour of its own", () => {
    const plan = planChart([
      series("One", "m"),
      series("Two", "m"),
    ]);

    expect(plan.series[0]?.colour).not.toBe(plan.series[1]?.colour);
  });

  it("survives a graph with no series", () => {
    const plan = planChart([]);

    expect(plan.scales).toHaveLength(0);
    expect(plan.series).toHaveLength(0);
  });
});

describe("the values laid on one axis of time", () => {
  it("puts the values of one series on the instants they were reported", () => {
    const aligned = alignSeries([points("Speed", "m/s", [0, 1, 2], [1, 2, 3])]);

    expect(aligned.x).toEqual([0, 1, 2]);
    expect(aligned.y[0]).toEqual([1, 2, 3]);
  });

  it("gathers the instants of every series into one axis", () => {
    const aligned = alignSeries([
      points("A", "m", [0, 1, 2], [10, 20, 30]),
      points("B", "m", [1, 3], [200, 300]),
    ]);

    expect(aligned.x).toEqual([0, 1, 2, 3]);
    expect(aligned.y[0]).toEqual([10, 20, 30, null]);
    expect(aligned.y[1]).toEqual([null, 200, null, 300]);
  });

  it("leaves a gap where a series has no value", () => {
    const aligned = alignSeries([
      points("A", "m", [0, 2], [1, 2]),
    ]);

    expect(aligned.x).toEqual([0, 2]);
    expect(aligned.y[0]).toEqual([1, 2]);
  });

  it("does not invent a value for a series that has reported nothing", () => {
    const aligned = alignSeries([points("A", "m", [], [])]);

    expect(aligned.x).toEqual([]);
    expect(aligned.y[0]).toEqual([]);
  });

  it("takes the same instant to be the same one", () => {
    const aligned = alignSeries([
      points("A", "m", [0, 1.0000001], [1, 2]),
      points("B", "m", [0, 1], [10, 20]),
    ]);

    expect(aligned.x).toHaveLength(2);
    expect(aligned.y[0]).toEqual([1, 2]);
    expect(aligned.y[1]).toEqual([10, 20]);
  });
});

describe("the value of a series", () => {
  it("is the one that was reported last", () => {
    const values = latestValues([
      points("A", "m", [0, 1], [1, 2]),
      points("B", "m", [0], [9]),
    ]);

    expect(values).toEqual([2, 9]);
  });

  it("is nothing when the series has reported nothing", () => {
    const values = latestValues([points("A", "m", [], [])]);

    expect(values).toEqual([null]);
  });
});

describe("the time axis", () => {
  it("is written in minutes and seconds", () => {
    expect(formatElapsed(0)).toBe("0:00");
    expect(formatElapsed(9)).toBe("0:09");
    expect(formatElapsed(65)).toBe("1:05");
    expect(formatElapsed(600)).toBe("10:00");
  });

  it("does not show a negative or an impossible instant", () => {
    expect(formatElapsed(-5)).toBe("0:00");
    expect(formatElapsed(Number.NaN)).toBe("0:00");
  });
});
