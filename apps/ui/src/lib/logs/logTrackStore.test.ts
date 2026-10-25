import { beforeEach, describe, expect, it } from "vitest";

import { lineOf, useLogTrackStore } from "./logTrackStore";

/**
 * The path of a log, and the file it belongs to.
 *
 * What is tested here is the order a coordinate is written in — longitude first, which is what
 * GeoJSON says and is the other way round from the fields of a point — and that a place that is
 * not a place is left out rather than drawn. The expected lines were written down before the
 * implementation was called with them.
 */

beforeEach(() => {
  useLogTrackStore.setState({ log: "", points: [] });
});

describe("the line the map draws", () => {
  it("writes a coordinate longitude first, as GeoJSON says", () => {
    // The hand: a point at 40.5 degrees north and 3.5 degrees west is written [-3.5, 40.5], and
    // a point at 41.0 north and 3.0 west is written [-3.0, 41.0]. Nothing is reordered after this,
    // so every drawing that reads the line reads the same order.
    const points = [
      { latitude: 40.5, longitude: -3.5, altitude: 120, at: 0 },
      { latitude: 41.0, longitude: -3.0, altitude: 130, at: 60_000_000 },
    ];

    expect(lineOf(points)).toEqual([
      [-3.5, 40.5],
      [-3.0, 41.0],
    ]);
  });

  it("leaves out a record that does not carry a place", () => {
    // A log can hold a message with zeroes where the position should be — a record read before the
    // vehicle knew where it was. A line through such a point runs to the middle of the ocean, so
    // the point is not part of the line.
    const points = [
      { latitude: 40.5, longitude: -3.5, altitude: 120, at: 0 },
      { latitude: Number.NaN, longitude: 2.0, altitude: 0, at: 30_000_000 },
      { latitude: 41.0, longitude: -3.0, altitude: 130, at: 60_000_000 },
    ];

    expect(lineOf(points)).toEqual([
      [-3.5, 40.5],
      [-3.0, 41.0],
    ]);
  });
});

describe("the path and the file it was read from", () => {
  it("keeps the path with the log it belongs to", () => {
    const points = [{ latitude: 40.5, longitude: -3.5, altitude: 120, at: 0 }];

    useLogTrackStore.getState().setTrack("C:/flight/mission.tlog", points);

    const state = useLogTrackStore.getState();

    expect(state.log).toBe("C:/flight/mission.tlog");
    expect(state.points).toEqual(points);
  });

  it("does not draw a path of another flight under the log that is open", () => {
    const store = useLogTrackStore.getState();

    store.setTrack("C:/flight/first.tlog", [
      { latitude: 40.5, longitude: -3.5, altitude: 120, at: 0 },
    ]);
    store.setTrack("C:/flight/second.tlog", [
      { latitude: 41.0, longitude: -3.0, altitude: 130, at: 60_000_000 },
    ]);

    const state = useLogTrackStore.getState();

    // The second reading replaces the first: a line on the ground is the path of one flight, and
    // two flights drawn as one line are a line that says something nobody opened.
    expect(state.log).toBe("C:/flight/second.tlog");
    expect(state.points.length).toBe(1);
    expect(state.points[0]?.latitude).toBe(41.0);
  });

  it("draws nothing when the path is let go of", () => {
    useLogTrackStore
      .getState()
      .setTrack("C:/flight/mission.tlog", [
        { latitude: 40.5, longitude: -3.5, altitude: 120, at: 0 },
      ]);

    useLogTrackStore.getState().clear();

    const state = useLogTrackStore.getState();

    expect(state.log).toBe("");
    expect(state.points).toEqual([]);
  });
});
