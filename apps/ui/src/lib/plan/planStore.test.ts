import { describe, expect, it } from "vitest";

import type { Plan, PlanItem } from "../ipc/types";
import { currentItem, currentItemNumber } from "./planStore";

/** One item of a plan, for the test. */
function item(seq: number, isCurrent: boolean): PlanItem {
  return {
    seq,
    command: "MAV_CMD_NAV_WAYPOINT",
    commandValue: 16,
    frame: 0,
    params: [0, 0, 0, 0],
    x: -35.28,
    y: 149.13,
    z: 100,
    current: isCurrent,
    autocontinue: true,
  };
}

/** A plan, for the test. */
function plan(items: readonly PlanItem[]): Plan {
  return { kind: "mission", items };
}

describe("the progress of a plan", () => {
  it("is unknown when there is no plan", () => {
    expect(currentItem(null)).toBeNull();
    expect(currentItemNumber(null)).toBeNull();
  });

  it("is the item the backend marked", () => {
    const thePlan = plan([item(0, false), item(1, true), item(2, false)]);

    expect(currentItem(thePlan)?.seq).toBe(1);
    expect(currentItemNumber(thePlan)).toBe(2);
  });

  it("is unknown when no item is marked", () => {
    const thePlan = plan([item(0, false), item(1, false)]);

    expect(currentItem(thePlan)).toBeNull();
    expect(currentItemNumber(thePlan)).toBeNull();
  });
});
