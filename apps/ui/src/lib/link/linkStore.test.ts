import { describe, expect, it } from "vitest";

import type { VehicleEvent } from "../ipc/types";
import { chosenVehicle } from "./linkStore";

/**
 * The vehicle the interface shows as the one it is reading.
 *
 * The choice is made by the link — the boards that answered are what it knows — so the store only
 * reads the mark the event carries. What is tested here is that the rule reads the mark rather
 * than the order of the list, because the two are not the same thing: a link that heard board 2
 * first lists it first, and an operator can then choose board 1.
 */

function vehicle(
  systemId: number,
  componentId: number,
  name: string,
  chosen: boolean,
): VehicleEvent {
  return { systemId, componentId, kind: null, name, chosen };
}

describe("the vehicle the station speaks to", () => {
  it("says there is none before the link has selected one", () => {
    // A link that has heard boards but has not satisfied the selection rule has no vehicle it
    // speaks to; saying "the first one" would name a board the telemetry is not being read from.
    const vehicles = [
      vehicle(1, 1, "1/1", false),
      vehicle(2, 62, "2/62", false),
    ];

    expect(chosenVehicle(vehicles)).toBeNull();
    expect(chosenVehicle([])).toBeNull();
  });

  it("reads the mark rather than the order of the list", () => {
    // The hand: board 2 was heard first, so it is listed first, and the operator chose it. The
    // rule has to answer with board 2 even though board 1 is at the front of the list.
    const vehicles = [
      vehicle(1, 1, "ArduCopter", false),
      vehicle(2, 1, "ArduPlane", true),
    ];

    expect(chosenVehicle(vehicles)?.systemId).toBe(2);

    // And when the choice moves, the rule follows it rather than staying where the list starts.
    const moved = [
      vehicle(1, 1, "ArduCopter", true),
      vehicle(2, 1, "ArduPlane", false),
    ];

    expect(chosenVehicle(moved)?.name).toBe("ArduCopter");
  });
});
