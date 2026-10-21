import { describe, expect, it } from "vitest";

import type { MavCommand, PlanItem } from "../ipc/types";
import {
  carriesPosition,
  commandOf,
  insertItem,
  itemBefore,
  moveItem,
  patchItem,
  removeItem,
  renumber,
  sameItems,
  slotsOfCommand,
  slotsOfPlan,
} from "./planItems";

/** One item of a plan, for the test. */
function item(seq: number, overrides: Partial<PlanItem> = {}): PlanItem {
  return {
    seq,
    command: "MAV_CMD_NAV_WAYPOINT",
    commandValue: 16,
    frame: 3,
    params: [0, 0, 0, 0],
    x: -35.28,
    y: 149.13,
    z: 100,
    current: false,
    autocontinue: true,
    ...overrides,
  };
}

/** A command of the catalogue, for the test. */
function command(
  value: number,
  name: string,
  roles: readonly string[],
): MavCommand {
  return {
    name,
    value,
    roles,
    units: roles.map(() => ""),
  };
}

const WAYPOINT = command(16, "Waypoint", [
  "empty",
  "empty",
  "empty",
  "other",
  "latitude",
  "longitude",
  "altitude",
]);

const DELAY = command(112, "Delay", [
  "other",
  "empty",
  "empty",
  "empty",
  "empty",
  "empty",
  "empty",
]);

const TAKEOFF = command(22, "Takeoff", [
  "angle",
  "empty",
  "empty",
  "angle",
  "latitude",
  "longitude",
  "altitude",
]);

describe("the order of a plan", () => {
  it("is the sequence of the items", () => {
    const items = renumber([item(3), item(7), item(9)]);

    expect(items.map((entry) => entry.seq)).toEqual([0, 1, 2]);
  });

  it("keeps the item that is not moved where it is", () => {
    const items = moveItem([item(0), item(1), item(2)], 1, 1);

    expect(items.map((entry) => entry.z)).toEqual([100, 100, 100]);
    expect(items.map((entry) => entry.seq)).toEqual([0, 1, 2]);
  });

  it("moves an item up", () => {
    const items = moveItem(
      [item(0, { z: 1 }), item(1, { z: 2 }), item(2, { z: 3 })],
      2,
      -1,
    );

    expect(items.map((entry) => entry.z)).toEqual([1, 3, 2]);
  });

  it("does not move an item off the end of the plan", () => {
    const items = [item(0, { z: 1 }), item(1, { z: 2 })];

    expect(moveItem(items, 0, -1).map((entry) => entry.z)).toEqual([1, 2]);
    expect(moveItem(items, 1, 1).map((entry) => entry.z)).toEqual([1, 2]);
  });

  it("renumbers the plan after a move", () => {
    const items = moveItem([item(0), item(1), item(2)], 0, 1);

    expect(items.map((entry) => entry.seq)).toEqual([0, 1, 2]);
  });

  it("removes an item and closes the gap", () => {
    const items = removeItem(
      [item(0, { z: 1 }), item(1, { z: 2 }), item(2, { z: 3 })],
      1,
    );

    expect(items.map((entry) => entry.z)).toEqual([1, 3]);
    expect(items.map((entry) => entry.seq)).toEqual([0, 1]);
  });

  it("inserts an item after the one that is named", () => {
    const items = insertItem(
      [item(0, { z: 1 }), item(1, { z: 2 })],
      0,
      item(9, { z: 7 }),
    );

    expect(items.map((entry) => entry.z)).toEqual([1, 7, 2]);
    expect(items.map((entry) => entry.seq)).toEqual([0, 1, 2]);
  });

  it("inserts an item at the end when there is nothing to insert after", () => {
    const items = insertItem([item(0, { z: 1 })], null, item(9, { z: 7 }));

    expect(items.map((entry) => entry.z)).toEqual([1, 7]);
  });

  it("knows the item that comes before another one", () => {
    const items = [item(0, { z: 1 }), item(1, { z: 2 })];

    expect(itemBefore(items, 1)?.z).toBe(1);
    expect(itemBefore(items, 0)).toBeNull();
  });
});

describe("a change to an item", () => {
  it("changes the item that is named and no other", () => {
    const items = patchItem([item(0), item(1)], 1, { x: -35.5, z: 250 });

    expect(items[0]?.x).toBe(-35.28);
    expect(items[1]?.x).toBe(-35.5);
    expect(items[1]?.z).toBe(250);
  });

  it("leaves the sequence of the item alone", () => {
    const items = patchItem([item(0), item(1)], 1, { z: 250 });

    expect(items.map((entry) => entry.seq)).toEqual([0, 1]);
  });
});

describe("whether a plan has been changed", () => {
  it("is not changed when the items are the same", () => {
    expect(sameItems([item(0), item(1)], [item(0), item(1)])).toBe(true);
  });

  it("is changed when an item has moved", () => {
    const first = item(0, { z: 1 });
    const second = item(1, { z: 2 });

    expect(sameItems([first, second], [second, first])).toBe(false);
  });

  it("is changed when a value has changed", () => {
    expect(sameItems([item(0)], [item(0, { z: 12 })])).toBe(false);
    expect(sameItems([item(0)], [item(0, { x: 1 })])).toBe(false);
    expect(sameItems([item(0)], [item(0, { frame: 0 })])).toBe(false);
    expect(sameItems([item(0)], [item(0, { autocontinue: false })])).toBe(false);
    expect(
      sameItems([item(0)], [item(0, { params: [0, 0, 0, 5] })]),
    ).toBe(false);
  });

  it("is changed when an item has been added", () => {
    expect(sameItems([item(0)], [item(0), item(1)])).toBe(false);
  });
});

describe("the parameters of a command", () => {
  it("are the slots the command does not leave empty", () => {
    expect(slotsOfCommand(WAYPOINT)).toEqual([3]);
    expect(slotsOfCommand(DELAY)).toEqual([0]);
    expect(slotsOfCommand(TAKEOFF)).toEqual([0, 3]);
  });

  it("are none for a command the ground station does not know", () => {
    expect(slotsOfCommand(undefined)).toEqual([]);
  });

  it("are the slots the whole plan uses", () => {
    const items = [item(0, { commandValue: 16 }), item(1, { commandValue: 112 })];

    expect(slotsOfPlan(items, [WAYPOINT, DELAY])).toEqual([0, 3]);
  });

  it("are none for a plan of commands the ground station does not know", () => {
    expect(slotsOfPlan([item(0, { commandValue: 9999 })], [WAYPOINT])).toEqual([]);
  });
});

describe("whether a command carries a position", () => {
  it("is known for the commands that have one", () => {
    expect(carriesPosition(WAYPOINT)).toBe(true);
    expect(carriesPosition(TAKEOFF)).toBe(true);
  });

  it("is not for the commands that have none", () => {
    expect(carriesPosition(DELAY)).toBe(false);
    expect(carriesPosition(undefined)).toBe(false);
  });
});

describe("the catalogue of commands", () => {
  it("is searched by the value the command travels with", () => {
    expect(commandOf([WAYPOINT, DELAY], 112)?.name).toBe("Delay");
    expect(commandOf([WAYPOINT, DELAY], 9999)).toBeUndefined();
  });
});
