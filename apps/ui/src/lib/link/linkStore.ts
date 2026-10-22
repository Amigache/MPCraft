import { create } from "zustand";

import { listen } from "../ipc/bridge";
import type {
  ConnectionStatus,
  LossStats,
  Telemetry,
  VehicleEvent,
  VehicleMessage,
} from "../ipc/types";
import { EMPTY_TELEMETRY, liveTelemetry } from "./liveTelemetry";

/**
 * The state of the link, in a store.
 *
 * The backend publishes the whole state in one event, several times a second. A component
 * that subscribed to the whole state would re-render on every tick, whether it shows
 * anything that changed or not. The store keeps the state, and each component selects the
 * slice it draws — so a tick that only moves the vehicle does not touch the message
 * console.
 */

/** The payload of the `link-state` event. */
interface LinkStatePayload {
  readonly status: ConnectionStatus;
  readonly vehicles: readonly VehicleEvent[];
  readonly telemetry: Telemetry;
  readonly loss: LossStats;
  readonly messages: readonly VehicleMessage[];
  readonly board?: string | null;
}

/** The payload of the `playback-state` event. */
interface PlaybackStatePayload {
  readonly open: boolean;
  readonly position: number;
  readonly duration: number;
  readonly telemetry: Telemetry;
}

/** The state of the link. */
export interface LinkStore {
  /** The state of the connection. */
  readonly status: ConnectionStatus;
  /** The vehicles seen on the link. */
  readonly vehicles: readonly VehicleEvent[];
  /** The loss statistics of the link. */
  readonly loss: LossStats;
  /** The messages the vehicle has sent, most recent last. */
  readonly messages: readonly VehicleMessage[];
  /**
   * The board that is on the other end, when the vehicle has said which one it is.
   *
   * The board says it when it starts, and the link keeps what it said.
   */
  readonly board: string | null;
  /** The most recent telemetry snapshot. */
  readonly telemetry: Telemetry;
  /**
   * Whether a log is being played back.
   *
   * A log that is played back is drawn instead of the vehicle on the link, and
   * the link does not overwrite it while it is being shown.
   */
  readonly playback: boolean;
  /**
   * Where the play-back is, in seconds.
   *
   * It is the moment the graph, the grid of the samples and the map are all
   * showing.
   */
  readonly playbackPosition: number;
}

/** The state of a link that is not open. */
const INITIAL: LinkStore = {
  status: "disconnected",
  vehicles: [],
  loss: { received: 0, lost: 0 },
  messages: [],
  board: null,
  telemetry: EMPTY_TELEMETRY,
  playback: false,
  playbackPosition: 0,
};

/**
 * Fills in the fields the backend may leave out.
 *
 * The declared contract has every field of the snapshot, but a backend that has not yet
 * learned to send one leaves it out rather than sending a null. The interface must not
 * care which of the two happened.
 */
function normaliseTelemetry(telemetry: Telemetry | undefined): Telemetry {
  if (!telemetry) {
    return EMPTY_TELEMETRY;
  }

  return {
    attitude: telemetry.attitude ?? null,
    position: telemetry.position ?? null,
    velocity: telemetry.velocity ?? null,
    battery: telemetry.battery ?? null,
    mode: telemetry.mode ?? null,
    armed: telemetry.armed ?? null,
    home: telemetry.home ?? null,
    distanceToHome: telemetry.distanceToHome ?? null,
    bearingToHome: telemetry.bearingToHome ?? null,
    mission: telemetry.mission ?? null,
    gps: telemetry.gps ?? null,
    gps2: telemetry.gps2 ?? null,
    efi: telemetry.efi ?? null,
    droneId: telemetry.droneId ?? null,
    wind: telemetry.wind ?? null,
    passthrough: telemetry.passthrough ?? null,
    imu: telemetry.imu ?? null,
    barometer: telemetry.barometer ?? null,
    controls: telemetry.controls ?? null,
    airspeed: telemetry.airspeed ?? null,
    ekf: telemetry.ekf ?? null,
    vibration: telemetry.vibration ?? null,
    batteries: telemetry.batteries ?? [],
    ahrs2: telemetry.ahrs2 ?? null,
    controller: telemetry.controller ?? null,
  };
}

export const useLinkStore = create<LinkStore>(() => INITIAL);

/** Whether two lists of messages carry the same text. */
function sameMessages(
  left: readonly VehicleMessage[],
  right: readonly VehicleMessage[],
): boolean {
  if (left === right) {
    return true;
  }
  if (left.length !== right.length) {
    return false;
  }
  return left.every((message, index) => {
    const other = right[index];
    return (
      other !== undefined &&
      message.severity === other.severity &&
      message.text === other.text
    );
  });
}

/** Whether two sets of loss statistics are the same. */
function sameLoss(left: LossStats, right: LossStats): boolean {
  return left.received === right.received && left.lost === right.lost;
}

/** Whether two lists of vehicles are the same. */
function sameVehicles(
  left: readonly VehicleEvent[],
  right: readonly VehicleEvent[],
): boolean {
  if (left === right) {
    return true;
  }
  if (left.length !== right.length) {
    return false;
  }
  return left.every((vehicle, index) => {
    const other = right[index];
    return (
      other !== undefined &&
      vehicle.systemId === other.systemId &&
      vehicle.componentId === other.componentId &&
      vehicle.kind === other.kind
    );
  });
}

/** Whether the bridge has already been started. */
let started = false;

/**
 * Starts the subscription to the state of the link.
 *
 * There is exactly one subscription, however many components read the store: the event
 * arrives once and is fanned out from here.
 */
export function startLinkBridge(): void {
  if (started) {
    return;
  }
  started = true;

  void listen<LinkStatePayload>("link-state", (event) => {
    // A log that is being played back is drawn instead of the vehicle on the
    // link, and the link does not take the window back while it is shown.
    if (useLinkStore.getState().playback) {
      return;
    }

    const payload = event.payload;
    const telemetry = normaliseTelemetry(payload.telemetry);

    // The drawing components read the snapshot directly, so that a tick does not have to
    // re-render the tree.
    liveTelemetry.publish(telemetry);

    useLinkStore.setState((state) => {
      const messages = payload.messages ?? [];
      const loss = payload.loss ?? { received: 0, lost: 0 };

      // The state arrives several times a second, and most of it does not change. The
      // references are kept when the content is the same, so that a component that shows
      // one of them is not re-rendered for nothing.
      return {
        status: payload.status,
        vehicles: sameVehicles(state.vehicles, payload.vehicles ?? [])
          ? state.vehicles
          : (payload.vehicles ?? []),
        loss: sameLoss(state.loss, loss) ? state.loss : loss,
        messages: sameMessages(state.messages, messages)
          ? state.messages
          : messages,
        board: payload.board ?? null,
        telemetry,
      };
    });
  });

  // A log that is played back is drawn as if the flight were happening: the telemetry
  // of the log is published, and the window shows the vehicle where it was.
  void listen<PlaybackStatePayload>("playback-state", (event) => {
    const payload = event.payload;

    if (!payload.open) {
      useLinkStore.setState({ playback: false, playbackPosition: 0 });
      return;
    }

    liveTelemetry.publish(normaliseTelemetry(payload.telemetry));
    useLinkStore.setState({ playback: true, playbackPosition: payload.position });
  });
}
/** The primary vehicle on the link, which is the one the interface drives. */
export function primaryVehicle(
  vehicles: readonly VehicleEvent[],
): VehicleEvent | null {
  if (vehicles.length === 0) {
    return null;
  }
  return vehicles[0] ?? null;
}
