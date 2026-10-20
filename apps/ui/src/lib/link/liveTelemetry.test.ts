import { describe, expect, it } from "vitest";

import type { Telemetry } from "../ipc/types";
import { EMPTY_TELEMETRY, liveTelemetry } from "./liveTelemetry";

/** A snapshot with an attitude, for the test. */
function snapshot(roll: number): Telemetry {
  return {
    ...EMPTY_TELEMETRY,
    attitude: { roll, pitch: 0, yaw: 0 },
  };
}

describe("the live telemetry", () => {
  it("keeps the last snapshot", () => {
    liveTelemetry.publish(snapshot(1));

    expect(liveTelemetry.value.attitude?.roll).toBe(1);
  });

  it("tells the listeners about every snapshot", () => {
    const seen: number[] = [];
    const stop = liveTelemetry.subscribe((telemetry) => {
      seen.push(telemetry.attitude?.roll ?? 0);
    });

    liveTelemetry.publish(snapshot(2));
    liveTelemetry.publish(snapshot(3));
    stop();
    liveTelemetry.publish(snapshot(4));

    expect(seen).toEqual([2, 3]);
  });

  it("forgets the snapshot when the link closes", () => {
    liveTelemetry.publish(snapshot(5));
    liveTelemetry.reset();

    expect(liveTelemetry.value).toEqual(EMPTY_TELEMETRY);
  });
});
