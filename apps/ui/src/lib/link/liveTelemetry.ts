import { useEffect, useRef, useState } from "react";

import type { Telemetry } from "../ipc/types";

/**
 * The live telemetry, outside React.
 *
 * The backend publishes a snapshot several times a second. Most of the interface does not
 * need to *re-render* when a snapshot arrives — the artificial horizon and the dials only
 * need their geometry moved — so the snapshot is kept here, in a mutable holder, and the
 * components that draw read it directly.
 *
 * A component that subscribes here is called back in the same tick the snapshot arrived.
 * It must therefore touch the DOM and nothing else: setting an attribute is cheap,
 * setting React state is not.
 */

/** A listener of the live telemetry. */
export type TelemetryListener = (telemetry: Telemetry) => void;

/** The snapshot of a link that has said nothing yet. */
export const EMPTY_TELEMETRY: Telemetry = {
  attitude: null,
  position: null,
  velocity: null,
  battery: null,
  mode: null,
  armed: null,
  home: null,
  distanceToHome: null,
  bearingToHome: null,
  mission: null,
  gps: null,
  imu: null,
  barometer: null,
  controls: null,
  airspeed: null,
  ekf: null,
  vibration: null,
  batteries: [],
  ahrs2: null,
  controller: null,
};

class LiveTelemetry {
  private current: Telemetry = EMPTY_TELEMETRY;

  private readonly listeners = new Set<TelemetryListener>();

  /** The most recent snapshot. */
  get value(): Telemetry {
    return this.current;
  }

  /** Publishes a snapshot to everyone who is listening. */
  publish(telemetry: Telemetry): void {
    this.current = telemetry;
    for (const listener of this.listeners) {
      listener(telemetry);
    }
  }

  /** Forgets the snapshot, which is what happens when the link closes. */
  reset(): void {
    this.publish(EMPTY_TELEMETRY);
  }

  /** Listens to the snapshots. The returned function stops the listening. */
  subscribe(listener: TelemetryListener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
}

/** The live telemetry of the primary vehicle. */
export const liveTelemetry = new LiveTelemetry();

/**
 * Reads a value of the live telemetry, re-rendering the component when it changes.
 *
 * Most of the interface does not need to re-render when a snapshot arrives, and uses
 * {@link LiveValue} for that. A value whose *shape* changes — the battery banks, which
 * appear and disappear — cannot be written into the text of an element, so the
 * component that draws it has to be told.
 *
 * The equality decides when the component is re-rendered. Without one, the value is
 * compared by identity, which is the right answer for a number and the wrong one for an
 * array that is rebuilt on every snapshot.
 */
export function useLiveValue<T>(
  select: (telemetry: Telemetry) => T,
  isEqual?: (left: T, right: T) => boolean,
): T {
  const [value, setValue] = useState(() => select(liveTelemetry.value));

  const selectRef = useRef(select);
  const isEqualRef = useRef(isEqual);

  selectRef.current = select;
  isEqualRef.current = isEqual;

  useEffect(() => {
    const write = (telemetry: Telemetry) => {
      const next = selectRef.current(telemetry);

      setValue((current) => {
        const equal = isEqualRef.current
          ? isEqualRef.current(current, next)
          : Object.is(current, next);

        return equal ? current : next;
      });
    };

    write(liveTelemetry.value);

    return liveTelemetry.subscribe(write);
  }, []);

  return value;
}
