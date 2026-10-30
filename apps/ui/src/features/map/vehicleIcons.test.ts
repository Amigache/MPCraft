import { describe, expect, it } from "vitest";

import { vehicleIcon } from "./vehicleIcons";

describe("the mark of a vehicle", () => {
  it("draws a different shape for every family", () => {
    const families = [
      "ArduCopter",
      "ArduPlane",
      "ArduRover",
      "ArduSub",
      "ArduTracker",
    ] as const;
    const drawings = families.map((family) => vehicleIcon(family));

    for (const drawing of drawings) {
      expect(drawing).toContain("<svg");
      expect(drawing).toContain('viewBox="0 0 24 24"');
    }

    // Two families that were given the same drawing would be two boards that look alike on the
    // map, which is the thing this exists to stop.
    expect(new Set(drawings).size).toBe(families.length);
  });

  it("takes its colour from the sheet rather than from the file", () => {
    const drawing = vehicleIcon("ArduCopter");
    expect(drawing).toContain("currentColor");
    // The marker sits on a map of any colour: a colour written here would be a shape that
    // disappears over the tiles of one provider and not another.
    expect(drawing).not.toMatch(/#[0-9a-fA-F]{3,8}/);
  });

  it("falls back to a craft when the family is not known", () => {
    for (const unknown of [null, undefined, "SomeFutureAutopilot"]) {
      expect(vehicleIcon(unknown)).toContain("<svg");
    }
    expect(vehicleIcon(null)).toBe(vehicleIcon("Other"));
  });
});
