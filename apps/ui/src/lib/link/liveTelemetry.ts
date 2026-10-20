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
