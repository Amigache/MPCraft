import type { MavCommand, PlanItem } from "../ipc/types";

/**
 * The editing of a plan.
 *
 * These functions are pure: they take the items of a plan and give back the items of the
 * plan as it is after the edit. They know nothing about the protocol and nothing about the
 * backend — a plan is a list of items, and these functions move them about.
 *
 * The sequence of an item is its place in the plan: the plan is flown in order, so the
 * order is not a detail. Every function that changes the order renumbers the items.
 */

/** How many generic parameters an item has. */
export const PARAM_SLOTS = 4;

/** A change to one item of a plan. */
export type ItemPatch = Partial<Omit<PlanItem, "seq">>;

/** The role of the slot that holds the position of an item. */
const LATITUDE_SLOT = 4;
const LONGITUDE_SLOT = 5;

/** Renumbers the items, so that the sequence of an item is its place in the plan. */
export function renumber(items: readonly PlanItem[]): PlanItem[] {
  return items.map((item, index) =>
    item.seq === index ? item : { ...item, seq: index },
  );
}

/** Applies a change to one item of a plan. */
export function patchItem(
  items: readonly PlanItem[],
  seq: number,
  patch: ItemPatch,
): PlanItem[] {
  return items.map((item) => (item.seq === seq ? { ...item, ...patch } : item));
}

/** Moves an item one place up or down the plan. */
export function moveItem(
  items: readonly PlanItem[],
  seq: number,
  delta: number,
): PlanItem[] {
  const index = items.findIndex((item) => item.seq === seq);
  const target = index + delta;

  if (index < 0 || target < 0 || target >= items.length) {
    return [...items];
  }

  const next = [...items];
  const [moved] = next.splice(index, 1);
  if (moved === undefined) {
    return [...items];
  }
  next.splice(target, 0, moved);

  return renumber(next);
}

/** Removes an item from a plan. */
export function removeItem(
  items: readonly PlanItem[],
  seq: number,
): PlanItem[] {
  return renumber(items.filter((item) => item.seq !== seq));
}

/**
 * Inserts an item into a plan.
 *
 * The item goes after the item with the given sequence, or at the end of the plan when
 * there is no such item.
 */
export function insertItem(
  items: readonly PlanItem[],
  after: number | null,
  item: PlanItem,
): PlanItem[] {
  const index =
    after === null
      ? items.length
      : items.findIndex((entry) => entry.seq === after) + 1;

  const next = [...items];
  next.splice(index < 0 ? items.length : index, 0, item);
  return renumber(next);
}

/** The item that comes before the one with the given sequence. */
export function itemBefore(
  items: readonly PlanItem[],
  seq: number,
): PlanItem | null {
  const index = items.findIndex((item) => item.seq === seq);
  if (index <= 0) {
    return null;
  }
  return items[index - 1] ?? null;
}

/** Whether two lists of items are the same plan. */
export function sameItems(
  left: readonly PlanItem[],
  right: readonly PlanItem[],
): boolean {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((item, index) => {
    const other = right[index];
    if (other === undefined) {
      return false;
    }

    return (
      item.commandValue === other.commandValue &&
      item.frame === other.frame &&
      item.x === other.x &&
      item.y === other.y &&
      item.z === other.z &&
      item.current === other.current &&
      item.autocontinue === other.autocontinue &&
      item.params.every((value, slot) => value === other.params[slot])
    );
  });
}

/** The command of the catalogue with the given value, when it is known. */
export function commandOf(
  commands: readonly MavCommand[],
  value: number,
): MavCommand | undefined {
  return commands.find((command) => command.value === value);
}

/** The parameter slots a command uses, in order. */
export function slotsOfCommand(command: MavCommand | undefined): number[] {
  if (command === undefined) {
    return [];
  }

  const slots: number[] = [];
  for (let slot = 0; slot < PARAM_SLOTS; slot += 1) {
    if ((command.roles[slot] ?? "empty") !== "empty") {
      slots.push(slot);
    }
  }
  return slots;
}

/**
 * The parameter slots the whole plan uses, in order.
 *
 * The columns of the grid follow the plan: a plan of delays has one parameter column and
 * a plan of waypoints has another. Four empty boxes for a command that has no parameters
 * are noise, and noise in a grid of a thousand rows is a fault.
 */
export function slotsOfPlan(
  items: readonly PlanItem[],
  commands: readonly MavCommand[],
): number[] {
  const used = new Set<number>();

  for (const item of items) {
    for (const slot of slotsOfCommand(commandOf(commands, item.commandValue))) {
      used.add(slot);
    }
  }

  return [...used].sort((left, right) => left - right);
}

/** Whether a command carries a position. */
export function carriesPosition(
  command: MavCommand | undefined,
): boolean {
  if (command === undefined) {
    return false;
  }
  return (
    command.roles[LATITUDE_SLOT] === "latitude" &&
    command.roles[LONGITUDE_SLOT] === "longitude"
  );
}
