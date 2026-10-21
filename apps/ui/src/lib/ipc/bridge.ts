import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";

import type {
  CommandError,
  ConnectRequest,
  ConnectResponse,
  ConnectionStatus,
  FlightMode,
  GraphPreset,
  GraphSeries,
  LossStats,
  MavCommand,
  Parameter,
  Plan,
  PlanItem,
  PlanKind,
  PlanWarning,
  SerialPort,
  Telemetry,
  VehicleEvent,
  VehicleKey,
  VehicleMessage,
  VehicleRequest,
} from "./types";

export { listen };

/**
 * The single place where the frontend talks to the Rust backend.
 *
 * Every command goes through {@link call}, which normalises the error shape so components
 * handle a `code` rather than parsing strings.
 */
export async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw normaliseError(cause);
  }
}

/** Converts an unknown thrown value into the typed error shape. */
export function normaliseError(cause: unknown): CommandError {
  if (
    typeof cause === "object" &&
    cause !== null &&
    "code" in cause &&
    "message" in cause
  ) {
    const candidate = cause as { code: unknown; message: unknown };
    if (typeof candidate.code === "string" && typeof candidate.message === "string") {
      return { code: candidate.code, message: candidate.message };
    }
  }

  return {
    code: "internal",
    message: cause instanceof Error ? cause.message : String(cause),
  };
}

/** The request payload of the commands that name a vehicle. */
function vehicleRequest(vehicle?: VehicleKey): { req: VehicleRequest } {
  const request: VehicleRequest = vehicle
    ? { systemId: vehicle.systemId, componentId: vehicle.componentId }
    : {};

  return { req: request };
}

/** Arms the vehicle. */
export async function arm(vehicle?: VehicleKey): Promise<void> {
  await call<void>("arm", vehicleRequest(vehicle));
}

/** Disarms the vehicle. */
export async function disarm(vehicle?: VehicleKey): Promise<void> {
  await call<void>("disarm", vehicleRequest(vehicle));
}

/** Takes off to an altitude, relative to the home position, in metres. */
export async function takeoff(
  altitude: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("takeoff", { ...vehicleRequest(vehicle), altitude });
}

/** Changes the flight mode. */
export async function setMode(
  customMode: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("set_mode", { ...vehicleRequest(vehicle), customMode });
}

/** Downloads the plan the vehicle holds. */
export async function downloadMission(
  kind: PlanKind,
  vehicle?: VehicleKey,
): Promise<Plan> {
  return call<Plan>("download_mission", { ...vehicleRequest(vehicle), kind });
}

/** Uploads a plan to the vehicle. */
export async function uploadMission(
  items: readonly PlanItem[],
  kind: PlanKind,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("upload_mission", {
    ...vehicleRequest(vehicle),
    kind,
    items,
  });
}

/** Says which item of the plan the vehicle is executing. */
export async function setCurrentItem(
  seq: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("set_current_item", { ...vehicleRequest(vehicle), seq });
}

/** Empties one of the vehicle's plans. */
export async function clearMission(
  kind: PlanKind,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("clear_mission", { ...vehicleRequest(vehicle), kind });
}

/** Makes the vehicle jump to an item of the plan. */
export async function jumpToItem(
  seq: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("jump_to_item", { ...vehicleRequest(vehicle), seq });
}

/** The flight modes the vehicle can be in. */
export async function listFlightModes(): Promise<FlightMode[]> {
  return call<FlightMode[]>("list_flight_modes");
}

/** The serial ports this machine has. */
export async function listSerialPorts(): Promise<SerialPort[]> {
  return call<SerialPort[]>("list_serial_ports");
}

/** The graphs the user can choose. */
export async function listGraphs(): Promise<GraphPreset[]> {
  return call<GraphPreset[]>("list_graphs");
}

/**
 * The values of a graph over the history of the telemetry.
 *
 * `since` is how long back to look, in seconds. A live graph only wants the recent past:
 * asking for the whole history every second would send megabytes over the wire for
 * nothing.
 */
export async function graphSeries(
  name: string,
  since?: number,
): Promise<GraphSeries> {
  return call<GraphSeries>("graph_series", { name, since });
}

/** The parameters the vehicle has reported. */
export async function listParameters(): Promise<Parameter[]> {
  return call<Parameter[]>("list_parameters");
}

/** Asks the vehicle for every parameter it has, and waits for the answers. */
export async function readParameters(vehicle?: VehicleKey): Promise<Parameter[]> {
  return call<Parameter[]>("read_parameters", vehicleRequest(vehicle));
}

/** Writes a parameter and waits for the vehicle to confirm it. */
export async function writeParameter(
  name: string,
  value: number,
  vehicle?: VehicleKey,
): Promise<Parameter> {
  return call<Parameter>("write_parameter", {
    ...vehicleRequest(vehicle),
    name,
    value,
  });
}

/**
 * The warnings about a plan, before it is flown.
 *
 * The rule is the autopilot's, so it is evaluated in the model: the interface shows what
 * it says and does not know why.
 */
export async function validateMission(
  items: readonly PlanItem[],
  kind: PlanKind,
): Promise<PlanWarning[]> {
  return call<PlanWarning[]>("validate_mission", { kind, items });
}

/** Writes a plan to a file, in the format Mission Planner uses. */
export async function exportPlan(
  items: readonly PlanItem[],
  kind: PlanKind,
  format: string,
  path: string,
): Promise<void> {
  await call<void>("export_plan", { kind, items, format, path });
}

/** Reads a plan from a file, in the format Mission Planner uses. */
export async function importPlan(
  format: string,
  path: string,
): Promise<Plan> {
  return call<Plan>("import_plan", { format, path });
}

/**
 * The commands a plan can hold, and what their parameters mean.
 *
 * The table is the one Mission Planner's `mavcmd.xml` describes, ported into the model:
 * the meaning of a parameter belongs to the autopilot, not to the screen.
 */
export async function listMavCommands(): Promise<MavCommand[]> {
  return call<MavCommand[]>("list_mav_commands");
}

/** The connection state exposed to the UI. */
export interface ConnectionState {
  readonly status: ConnectionStatus;
  readonly error: CommandError | null;
  readonly connect: (target: string) => Promise<void>;
  readonly disconnect: () => Promise<void>;
}

/** Subscribes to the connection state and provides connect / disconnect actions. */
export function useConnection(): ConnectionState {
  const [status, setStatus] = useState<ConnectionStatus>("disconnected");
  const [error, setError] = useState<CommandError | null>(null);

  const connect = useCallback(async (target: string) => {
    setStatus("connecting");
    setError(null);
    try {
      const request: ConnectRequest = { target };
      const response = await call<ConnectResponse>("connect", { req: request });
      setStatus(response.status);
    } catch (cause) {
      setError(cause as CommandError);
      setStatus("failed");
    }
  }, []);

  const disconnect = useCallback(async () => {
    try {
      await call<ConnectResponse>("disconnect");
      setStatus("disconnected");
    } catch (cause) {
      setError(cause as CommandError);
    }
  }, []);

  useEffect(() => {
    const unlisten = listen<{ status: ConnectionStatus }>("link-state", (event) => {
      setStatus(event.payload.status);
    });
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  return { status, error, connect, disconnect };
}

/**
 * Subscribes to the batched telemetry stream.
 *
 * Telemetry arrives as one snapshot per tick, not one event per field. At 20 Hz with forty
 * fields, per-field events would mean eight hundred IPC messages per second.
 */
export function useTelemetry(
  vehicle: VehicleKey | null,
  onTelemetry: (telemetry: Telemetry) => void,
): void {
  useEffect(() => {
    if (!vehicle) {
      return;
    }

    const unlisten = listen<{ vehicle: VehicleKey; t: Telemetry }>(
      "telemetry",
      (event) => {
        if (
          event.payload.vehicle.systemId === vehicle.systemId &&
          event.payload.vehicle.componentId === vehicle.componentId
        ) {
          onTelemetry(event.payload.t);
        }
      },
    );

    return () => {
      void unlisten.then((off) => off());
    };
  }, [vehicle, onTelemetry]);
}

/** The whole state the backend publishes, in one piece. */
export interface LinkState {
  readonly status: ConnectionStatus;
  readonly vehicles: readonly VehicleEvent[];
  readonly telemetry: Telemetry;
  readonly loss: LossStats;
  readonly messages: readonly VehicleMessage[];
}

/** The state of a link that is not open. */
const EMPTY_STATE: LinkState = {
  status: "disconnected",
  vehicles: [],
  telemetry: {
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
  },
  loss: { received: 0, lost: 0 },
  messages: [],
};

/**
 * Subscribes to the state of the link.
 *
 * The backend publishes the whole state in one event, so the components that show it
 * share one subscription rather than each opening their own.
 */
export function useLinkState(): LinkState {
  const [state, setState] = useState<LinkState>(EMPTY_STATE);

  useEffect(() => {
    const unlisten = listen<LinkState>("link-state", (event) => {
      setState(event.payload);
    });

    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  return state;
}
