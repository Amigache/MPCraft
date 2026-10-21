import { describe, expect, it } from "vitest";

import { distanceBetween } from "../geo";
import { circlePoints } from "./circle";

/** The centre of the circle, for the test. */
const CENTRE = { lat: -35.2809, lng: 149.13 };

describe("the circle of a survey", () => {
  it("has as many points as it was asked for", () => {
    const points = circlePoints({
      centre: CENTRE,
      radius: 500,
      points: 12,
      altitude: 100,
      clockwise: false,
    });

    expect(points).toHaveLength(12);
  });

  it("is a circle of the radius that was asked for", () => {
    const points = circlePoints({
      centre: CENTRE,
      radius: 500,
      points: 12,
      altitude: 100,
      clockwise: false,
    });

    for (const point of points) {
      const distance = distanceBetween(CENTRE.lat, CENTRE.lng, point.x, point.y);
      expect(distance).toBeGreaterThan(480);
      expect(distance).toBeLessThan(520);
    }
  });

  it("is closed: the last point is next to the first", () => {
    const points = circlePoints({
      centre: CENTRE,
      radius: 500,
      points: 8,
      altitude: 100,
      clockwise: false,
    });

    const first = points[0];
    const last = points[points.length - 1];

    expect(first).toBeDefined();
    expect(last).toBeDefined();

    const distance = distanceBetween(
      last?.x ?? 0,
      last?.y ?? 0,
      first?.x ?? 0,
      first?.y ?? 0,
    );

    // The side of a polygon of eight points is shorter than the radius.
    expect(distance).toBeLessThan(500);
  });

  it("turns the other way when it is told to", () => {
    const anticlockwise = circlePoints({
      centre: CENTRE,
      radius: 500,
      points: 4,
      altitude: 100,
      clockwise: false,
    });

    const clockwise = circlePoints({
      centre: CENTRE,
      radius: 500,
      points: 4,
      altitude: 100,
      clockwise: true,
    });

    // The first point is the northernmost in both, and the second one is on the other
    // side.
    expect(anticlockwise[1]?.x).not.toBe(clockwise[1]?.x);
  });

  it("is nothing when the circle is not a circle", () => {
    expect(
      circlePoints({
        centre: CENTRE,
        radius: 500,
        points: 2,
        altitude: 100,
        clockwise: false,
      }),
    ).toHaveLength(0);

    expect(
      circlePoints({
        centre: CENTRE,
        radius: 0,
        points: 12,
        altitude: 100,
        clockwise: false,
      }),
    ).toHaveLength(0);
  });

  it("flies at the height it was told", () => {
    const points = circlePoints({
      centre: CENTRE,
      radius: 500,
      points: 6,
      altitude: 120,
      clockwise: false,
    });

    for (const point of points) {
      expect(point.z).toBe(120);
    }
  });
});
