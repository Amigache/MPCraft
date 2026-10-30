import type { MavCommand, PlanItem } from "../ipc/types";

/**
 * What the map draws around the plan that is not a point.
 *
 * Three overlays make a plan read the way Mission Planner's does: the circle an item flies
 * around, and the place between two items where one can be added. They are worked out here,
 * away from the map, because they are arithmetic over the plan and not drawing.
 */

/** A circle an item of the plan flies around. */
export interface PlanCircle {
  /** The place the circle is centred on. */
  readonly lat: number;
  readonly lng: number;
  /** The radius, in metres. */
  readonly radius: number;
  /** The item the circle belongs to. */
  readonly seq: number;
}

/** The slot that carries the distance of a command, when it has one. */
const DISTANCE = "distance";

/** Whether the command is one the vehicle loiters on. */
function isLoiter(command: MavCommand | undefined): boolean {
  return command?.canonicalName.includes("LOITER") ?? false;
}

/**
 * The circles of the loiter items.
 *
 * A loiter is flown around the item at the radius of its distance slot, which is what Mission
 * Planner draws as a circle over the waypoint: the operator has to see the size of the turn
 * before the plan goes to the vehicle. An item whose radius is zero has no circle — a loiter of
 * no radius is the vehicle standing still, and a circle of no size is a dot.
 */
export function loiterCircles(
  items: readonly PlanItem[],
  commands: readonly MavCommand[],
): PlanCircle[] {
  const circles: PlanCircle[] = [];

  for (const item of items) {
    const command = commands.find((entry) => entry.value === item.commandValue);
    if (!isLoiter(command) || command === undefined) {
      continue;
    }

    const slot = command.roles
      .slice(0, item.params.length)
      .findIndex((role) => role === DISTANCE);
    const radius = slot < 0 ? 0 : (item.params[slot] ?? 0);

    if (!Number.isFinite(radius) || radius <= 0) {
      continue;
    }

    circles.push({ lat: item.x, lng: item.y, radius, seq: item.seq });
  }

  return circles;
}

/** A place between two items where one can be added. */
export interface InsertionPoint {
  /** The item the new one goes after. */
  readonly after: number;
  readonly lat: number;
  readonly lng: number;
}

/**
 * The places between two items where a waypoint can be added.
 *
 * Mission Planner draws a small `+` on the line between two waypoints, and pressing it puts a
 * waypoint there: the operator adds to the middle of a leg without moving anything else. The
 * point sits half way along the leg, which is a straight average of the two ends — over the
 * length of a leg the difference from the middle of the great circle is far below the width of
 * a pixel on any map zoom the plan is read at.
 */
export function insertionPoints(items: readonly PlanItem[]): InsertionPoint[] {
  const points: InsertionPoint[] = [];

  for (let index = 1; index < items.length; index += 1) {
    const previous = items[index - 1];
    const item = items[index];

    if (previous === undefined || item === undefined) {
      continue;
    }

    points.push({
      after: previous.seq,
      lat: (previous.x + item.x) / 2,
      lng: (previous.y + item.y) / 2,
    });
  }

  return points;
}
