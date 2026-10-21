import { describe, expect, it } from "vitest";

import { distanceBetween } from "../geo";
import { surveyPoints } from "./survey";
import { splinePath } from "./spline";

/** The middle of the field, for the test. */
const CENTRE = { lat: -35.2809, lng: 149.13 };

describe("the reticle of a survey", () => {
  const request = {
    centre: CENTRE,
    length: 200,
    spacing: 50,
    strips: 4,
    heading: 0,
    altitude: 100,
  };

  it("has two points for every strip", () => {
    expect(surveyPoints(request)).toHaveLength(8);
  });

  it("flies the strips in the order it flies them", () => {
    const points = surveyPoints(request);

    // The first strip goes from the south to the north, and the second one back.
    const first = points[0];
    const second = points[1];

    expect(first).toBeDefined();
    expect(second).toBeDefined();

    expect(first?.x ?? 0).toBeLessThan(second?.x ?? 0);
  });

  it("puts the strips apart by the distance that was asked for", () => {
    const points = surveyPoints(request);

    // The middle of the first strip and the middle of the second one: the strips are
    // flown back and forth, so their ends are on opposite sides.
    const middleOfFirst = {
      x: ((points[0]?.x ?? 0) + (points[1]?.x ?? 0)) / 2,
      y: ((points[0]?.y ?? 0) + (points[1]?.y ?? 0)) / 2,
    };

    const middleOfSecond = {
      x: ((points[2]?.x ?? 0) + (points[3]?.x ?? 0)) / 2,
      y: ((points[2]?.y ?? 0) + (points[3]?.y ?? 0)) / 2,
    };

    const apart = distanceBetween(
      middleOfFirst.x,
      middleOfFirst.y,
      middleOfSecond.x,
      middleOfSecond.y,
    );

    expect(apart).toBeGreaterThan(45);
    expect(apart).toBeLessThan(55);
  });

  it("flies at the height it was told", () => {
    for (const point of surveyPoints(request)) {
      expect(point.z).toBe(100);
    }
  });

  it("is nothing when the field is not a field", () => {
    expect(
      surveyPoints({ ...request, strips: 0 }),
    ).toHaveLength(0);

    expect(
      surveyPoints({ ...request, spacing: 0 }),
    ).toHaveLength(0);
  });
});

describe("the smooth path of a plan", () => {
  it("passes through every place of the plan", () => {
    const places = [
      { x: 0, y: 0 },
      { x: 10, y: 10 },
      { x: 20, y: 0 },
      { x: 30, y: 10 },
    ];

    const path = splinePath(places);

    for (const place of places) {
      const found = path.some(
        (point) =>
          Math.abs(point.x - place.x) < 1e-6 && Math.abs(point.y - place.y) < 1e-6,
      );

      expect(found).toBe(true);
    }
  });

  it("starts and ends where the plan does", () => {
    const places = [
      { x: 1, y: 2 },
      { x: 3, y: 4 },
      { x: 5, y: 6 },
    ];

    const path = splinePath(places);

    expect(path[0]?.x).toBeCloseTo(1, 6);
    expect(path[0]?.y).toBeCloseTo(2, 6);

    const last = path[path.length - 1];
    expect(last?.x).toBeCloseTo(5, 6);
    expect(last?.y).toBeCloseTo(6, 6);
  });

  it("is the straight line when there is nothing to curve", () => {
    const places = [
      { x: 0, y: 0 },
      { x: 1, y: 1 },
    ];

    expect(splinePath(places)).toHaveLength(2);
  });
});
