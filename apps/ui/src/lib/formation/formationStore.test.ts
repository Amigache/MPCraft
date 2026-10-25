import { beforeEach, describe, expect, it } from "vitest";

import { useFormationStore, vehicleName } from "./formationStore";

/**
 * The store of the formation holds what the operator wrote and what the backend says about it.
 *
 * What is tested here is that the two are not merged: a name that cannot be read as a vehicle is
 * not turned into one, and the decision to run a fleet is not something the places can make by
 * themselves. The numbers are checked against values worked out by hand, so that a pass does not
 * depend on the implementation agreeing with itself.
 */

beforeEach(() => {
  useFormationStore.setState({
    leader: "",
    enabled: false,
    slots: [],
    running: false,
    told: 0,
    refused: [],
    error: null,
  });
});

describe("the places an operator wrote", () => {
  it("names a vehicle the way the file names it", () => {
    // The hand: the pair the protocol identifies a board by, written with the separator the file
    // uses. `2` is the system id and `1` is the component id of the autopilot.
    expect(vehicleName(2, 1)).toBe("2:1");
    expect(vehicleName(11, 62)).toBe("11:62");
  });

  it("keeps the metres as the metres they were written as", () => {
    const store = useFormationStore.getState();

    store.addSlot("3:1");
    store.setOffset("3:1", "y", 40.0);
    store.setOffset("3:1", "z", 5.5);

    const slot = useFormationStore.getState().slots[0];

    expect(slot?.vehicle).toBe("3:1");
    // Nothing is scaled between what was typed and what the file will hold: forty metres ahead is
    // forty, not forty divided by the scale factor of a projection somebody chose.
    expect(slot?.x).toBe(0);
    expect(slot?.y).toBe(40);
    expect(slot?.z).toBeCloseTo(5.5);
  });

  it("does not put the same vehicle in the formation twice", () => {
    const store = useFormationStore.getState();

    store.addSlot("2:1");
    store.addSlot("2:1");
    store.addSlot("3:1");

    expect(
      useFormationStore.getState().slots.map((slot) => slot.vehicle),
    ).toEqual(["2:1", "3:1"]);
  });

  it("keeps the places in the order they were written", () => {
    const store = useFormationStore.getState();

    store.addSlot("4:1");
    store.addSlot("2:1");

    expect(
      useFormationStore.getState().slots.map((slot) => slot.vehicle),
    ).toEqual(["4:1", "2:1"]);
  });
});

describe("what the backend says about what was written", () => {
  it("says which name it could not read as a vehicle, without guessing at it", () => {
    // The name that cannot be read stays in the file as it was written, so that somebody fixing
    // the file sees what they actually wrote. The store shows the places that could be read and
    // the names that were left out, and does not turn one into the other.
    useFormationStore.getState().apply({
      enabled: false,
      running: false,
      told: 0,
      leader: "1:1",
      slots: [{ vehicle: "2:1", x: 0, y: 60, z: 0 }],
      refused: ["board two"],
      error: null,
    });

    const state = useFormationStore.getState();

    expect(state.slots.map((slot) => slot.vehicle)).toEqual(["2:1"]);
    expect(state.refused).toEqual(["board two"]);
    expect(state.leader).toBe("1:1");
  });

  it("does not run a fleet because its places were written", () => {
    const store = useFormationStore.getState();

    store.addSlot("2:1");

    // `running` is what the loop says it is doing, not what was asked for: writing a fleet is one
    // thing and sending orders to it is another.
    expect(useFormationStore.getState().running).toBe(false);
    expect(useFormationStore.getState().told).toBe(0);
  });

  it("shows the loop as running when the loop says it is running", () => {
    useFormationStore.getState().apply({
      enabled: true,
      running: true,
      told: 3,
      leader: "1:1",
      slots: [{ vehicle: "2:1", x: 0, y: 60, z: 0 }],
      refused: [],
      error: null,
    });

    const state = useFormationStore.getState();

    expect(state.enabled).toBe(true);
    expect(state.running).toBe(true);
    expect(state.told).toBe(3);
  });

  it("keeps the failure of the last tick", () => {
    useFormationStore.getState().apply({
      enabled: true,
      running: true,
      told: 0,
      leader: "1:1",
      slots: [{ vehicle: "2:1", x: 0, y: 60, z: 0 }],
      refused: [],
      error: "the leader cannot be projected: no zone at the equator",
    });

    expect(useFormationStore.getState().error).toBe(
      "the leader cannot be projected: no zone at the equator",
    );
  });
});
