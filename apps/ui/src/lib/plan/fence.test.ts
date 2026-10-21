import { describe, expect, it } from "vitest";

import type { PlanItem } from "../ipc/types";
import {
  CIRCLE_EXCLUSION,
  CIRCLE_INCLUSION,
  VERTEX_EXCLUSION,
  VERTEX_INCLUSION,
  circleRing,
  circlesOf,
  exclusionsOf,
  ringOf,
  verticesOf,
} from "./fence";

/** One item of a fence, for the test. */
function item(command: string, lat: number, lng: number, radius = 0): PlanItem {
  return {
    seq: 0,
    command,
    commandValue: 5001,
    frame: 3,
    params: [radius, 0, 0, 0],
    x: lat,
    y: lng,
    z: 0,
    current: false,
    autocontinue: true,
  };
}

describe("the fence", () => {
  it("tells the vertices of the polygon from the points excluded from it", () => {
    const items = [
      item(VERTEX_INCLUSION, -35.0, 149.0),
      item(VERTEX_EXCLUSION, -35.1, 149.1),
      item(VERTEX_INCLUSION, -35.2, 149.2),
    ];

    expect(verticesOf(items)).toHaveLength(2);
    expect(exclusionsOf(items)).toHaveLength(1);
    expect(exclusionsOf(items)[0]?.lat).toBeCloseTo(-35.1, 6);
  });

  it("reads the circles, and says which of them are included", () => {
    const items = [
      item(CIRCLE_INCLUSION, -35.0, 149.0, 250),
      item(CIRCLE_EXCLUSION, -35.1, 149.1, 100),
    ];

    const circles = circlesOf(items);

    expect(circles).toHaveLength(2);
    expect(circles[0]?.radius).toBe(250);
    expect(circles[0]?.included).toBe(true);
    expect(circles[1]?.included).toBe(false);
  });

  it("closes the ring of the polygon", () => {
    const items = [
      item(VERTEX_INCLUSION, -35.0, 149.0),
      item(VERTEX_INCLUSION, -35.1, 149.1),
      item(VERTEX_INCLUSION, -35.2, 149.2),
    ];

    const ring = ringOf(items);

    expect(ring).not.toBeNull();
    expect(ring).toHaveLength(4);
    // The first point is the last one: a polygon is closed.
    expect(ring?.[0]).toEqual(ring?.[3]);
  });

  it("has no ring when it has fewer than three vertices", () => {
    const items = [
      item(VERTEX_INCLUSION, -35.0, 149.0),
      item(VERTEX_INCLUSION, -35.1, 149.1),
    ];

    expect(ringOf(items)).toBeNull();
  });

  it("draws a circle of the right size", () => {
    // A circle of a hundred metres around Canberra is about 0.0009 degrees across.
    const ring = circleRing({
      lat: -35.2809,
      lng: 149.13,
      radius: 100,
      included: true,
    });

    expect(ring).toHaveLength(36);

    const latitudes = ring.map((point) => point[1]);
    const span = Math.max(...latitudes) - Math.min(...latitudes);

    // A hundred metres up and down is two hundred metres.
    expect(span).toBeGreaterThan(0.0015);
    expect(span).toBeLessThan(0.002);
  });
});
