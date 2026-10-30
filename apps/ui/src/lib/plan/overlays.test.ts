import { describe, expect, it } from "vitest";

import type { MavCommand, PlanItem } from "../ipc/types";
import { courseLine, insertionPoints, loiterCircles } from "./overlays";

function command(value: number, name: string, roles: string[]): MavCommand {
  return {
    name,
    value,
    roles,
    units: roles.map(() => "m"),
    canonicalName: name,
  };
}

function item(
  seq: number,
  lat: number,
  lng: number,
  params: number[] = [0, 0, 0, 0],
  commandValue = 16,
): PlanItem {
  return {
    seq,
    command:
      commandValue === 17 ? "MAV_CMD_NAV_LOITER_UNLIM" : "MAV_CMD_NAV_WAYPOINT",
    commandValue,
    frame: 3,
    params: [params[0] ?? 0, params[1] ?? 0, params[2] ?? 0, params[3] ?? 0],
    x: lat,
    y: lng,
    z: 100,
    current: false,
    autocontinue: true,
  };
}

const waypoint = command(16, "MAV_CMD_NAV_WAYPOINT", [
  "empty",
  "empty",
  "empty",
  "empty",
  "latitude",
  "longitude",
  "altitude",
]);

const loiter = command(17, "MAV_CMD_NAV_LOITER_UNLIM", [
  "empty",
  "distance",
  "empty",
  "empty",
  "latitude",
  "longitude",
  "altitude",
]);

describe("the circles of a plan", () => {
  it("draws the radius a loiter is flown at", () => {
    const circles = loiterCircles(
      [item(0, -35.1, 149.1), item(1, -35.2, 149.2, [0, 80, 0, 0], 17)],
      [waypoint, loiter],
    );

    expect(circles).toHaveLength(1);
    expect(circles[0]).toEqual({ lat: -35.2, lng: 149.2, radius: 80, seq: 1 });
  });

  it("draws nothing for a loiter with no radius", () => {
    // A loiter of no radius is the vehicle standing still: a circle of no size is a dot, and a
    // dot over a waypoint is a mark the operator would read as something else.
    expect(
      loiterCircles([item(0, -35.2, 149.2, [0, 0, 0, 0])], [loiter]),
    ).toEqual([]);
  });

  it("draws nothing for an item that is not a loiter", () => {
    // The distance slot of another command is not a circle: a fence circle is drawn as a fence,
    // and a waypoint's first parameter is a hold time.
    expect(
      loiterCircles([item(0, -35.2, 149.2, [0, 80, 0, 0])], [waypoint]),
    ).toEqual([]);
  });
});

describe("the line of the course", () => {
  it("runs north when the bearing is north", () => {
    const line = courseLine(-35, 149, 0, 100);

    expect(line.end[0]).toBeCloseTo(149, 9);
    // A hundred metres of latitude is about nine ten-thousandths of a degree.
    expect(line.end[1] - line.start[1]).toBeCloseTo(0.000899, 5);
  });

  it("covers more degrees of longitude towards the pole", () => {
    const equator = courseLine(0, 0, 90, 100);
    const south = courseLine(-60, 0, 90, 100);

    expect(Math.abs(equator.end[0])).toBeLessThan(Math.abs(south.end[0]));
    expect(Math.abs(south.end[0])).toBeCloseTo(2 * Math.abs(equator.end[0]), 1);
  });
});

describe("the places between two items", () => {
  it("puts one point between each pair, after the first of the two", () => {
    const points = insertionPoints([
      item(0, -35.0, 149.0),
      item(1, -35.2, 149.2),
      item(2, -35.4, 149.4),
    ]);

    expect(points).toHaveLength(2);
    expect(points[0]?.after).toBe(0);
    expect(points[0]?.lat).toBeCloseTo(-35.1, 6);
    expect(points[0]?.lng).toBeCloseTo(149.1, 6);
    expect(points[1]?.after).toBe(1);
  });

  it("draws none for a plan of one item, because there is no leg to split", () => {
    expect(insertionPoints([item(0, -35, 149)])).toEqual([]);
  });
});
