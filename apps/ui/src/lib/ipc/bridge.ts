import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";

import type {
  ChecklistEntry,
  CommandError,
  ConnectRequest,
  ConnectionProfile,
  ConnectResponse,
  ConnectionStatus,
  Coordinates,
  FlightMode,
  GraphPreset,
  GraphSeries,
  LogSummary,
  LogTrackPoint,
  LossStats,
  MavCommand,
  OutputFunction,
  Parameter,
  ParameterDifference,
  ParameterMetadata,
  Plan,
  PlanItem,
  PlanKind,
  PlanWarning,
  SerialPort,
  Telemetry,
  TerrainPoint,
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

/** The pre-flight checklist, checked against the vehicle. */
export async function preflightChecklist(): Promise<ChecklistEntry[]> {
  return call<ChecklistEntry[]>("preflight_checklist");
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
/**
 * Writes several parameters, in the order the vehicle needs them.
 *
 * The order is the autopilots' knowledge: the parameters that turn something on are
 * written last, so that what they turn on is already configured when it is turned on.
 */
export async function writeParametersBatch(
  changes: readonly { name: string; value: number }[],
): Promise<Parameter[]> {
  return call<Parameter[]>("write_parameters_batch", {
    req: {},
    changes: changes.map((change) => ({ ...change })),
  });
}

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
 * The parameters the operator has marked as favourites.
 *
 * The backend keeps them between two runs, in the order they were marked.
 */
export async function listFavouriteParameters(): Promise<string[]> {
  return call<string[]>("list_favourite_parameters");
}

/** Marks a parameter as a favourite, or takes the mark away. */
export async function setFavouriteParameter(
  name: string,
  favourite: boolean,
): Promise<void> {
  await call<void>("set_favourite_parameter", { name, favourite });
}

/**
 * The connections the operator has named.
 *
 * The backend keeps them between two runs, in the order they were saved.
 */
export async function listConnectionProfiles(): Promise<ConnectionProfile[]> {
  return call<ConnectionProfile[]>("list_connection_profiles");
}

/** Saves the connections the operator has named. */
export async function setConnectionProfiles(
  profiles: readonly ConnectionProfile[],
): Promise<void> {
  await call<void>("set_connection_profiles", { profiles: [...profiles] });
}

/**
 * Tells the vehicle to go to a place.
 *
 * This is the guided mode: the vehicle flies to the place and stays there.
 */
export async function setPositionTarget(
  latitude: number,
  longitude: number,
  altitude: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("set_position_target", {
    ...vehicleRequest(vehicle),
    latitude,
    longitude,
    altitude,
  });
}

/**
 * Runs the motors of the vehicle, to see whether they turn.
 *
 * The throttle is a number between minus one and one, where the sign is the direction.
 */
export async function motorTest(
  motor: number,
  throttle: number,
  seconds: number,
): Promise<void> {
  await call<void>("motor_test", { req: {}, motor, throttle, seconds });
}

/**
 * Takes the radio channels over, one at a time.
 *
 * The channels that are not given are left to the radio.
 */
export async function rcOverride(
  channels: readonly (number | null)[],
): Promise<void> {
  await call<void>("rc_override", { req: {}, channels: [...channels] });
}

/** Lets go of the radio channels. */
export async function rcOverrideRelease(): Promise<void> {
  await call<void>("rc_override_release", { req: {} });
}

/** Flies the vehicle with the sticks of the ground station. */
export async function manualControl(
  roll: number,
  pitch: number,
  thrust: number,
  yaw: number,
  buttons = 0,
): Promise<void> {
  await call<void>("manual_control", { req: {}, roll, pitch, thrust, yaw, buttons });
}

/**
 * Asks the vehicle to calibrate one of its sensors.
 *
 * The calibration is carried out by the vehicle: the order says which sensor, and the
 * vehicle answers when it has begun.
 */
export async function calibrate(
  kind: "accelerometer" | "level" | "compass",
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("calibrate", { ...vehicleRequest(vehicle), kind });
}

/** Asks the vehicle to restart. */
export async function reboot(vehicle?: VehicleKey): Promise<void> {
  await call<void>("reboot", vehicleRequest(vehicle));
}

/**
 * What the outputs of the vehicle are for.
 *
 * The vehicle says it with the parameters `SERVO1_FUNCTION` and its brothers, and the
 * names are the ones the specification gives them.
 */
export async function outputFunctions(): Promise<OutputFunction[]> {
  return call<OutputFunction[]>("output_functions");
}

/**
 * The groups the parameters belong to.
 *
 * The groups are the subjects the parameters are about — *Radio Calibration*, *EKF2* —
 * and upstream builds its configuration menu out of them.
 */
/**
 * What a log of a flight holds.
 *
 * The messages it holds and how many of each there are.
 */
export async function logSummary(path: string): Promise<LogSummary> {
  return call<LogSummary>("log_summary", { path });
}

/**
 * The values of a graph over a log.
 *
 * The graph is the same one the live view draws, read from the log instead of from the
 * link.
 */
export async function logSeries(path: string, name: string): Promise<GraphSeries> {
  return call<GraphSeries>("log_series", { path, name });
}

/**
 * Writes the track of a log as a KML document.
 *
 * The document is the one Google Earth reads.
 */
export async function logExportKml(
  log: string,
  output: string,
): Promise<void> {
  await call<void>("log_export_kml", { log, output });
}

/**
 * Writes the samples of a log as a CSV document.
 *
 * The columns are the series of the graph that is chosen.
 */
export async function logExportCsv(
  log: string,
  name: string,
  output: string,
): Promise<void> {
  await call<void>("log_export_csv", { log, name, output });
}

/** The track of a log: the places the vehicle went through. */
export async function logTrack(path: string): Promise<LogTrackPoint[]> {
  return call<LogTrackPoint[]>("log_track", { path });
}

export async function listParameterGroups(): Promise<string[]> {
  return call<string[]>("list_parameter_groups");
}

/**
 * What is known about some parameters.
 *
 * The answer holds one entry per name asked for, and a null for the ones the
 * documentation does not know.
 */
export async function parameterMetadata(
  names: readonly string[],
): Promise<(ParameterMetadata | null)[]> {
  return call<(ParameterMetadata | null)[]>("parameter_metadata", {
    names: [...names],
  });
}

/**
 * The parameters of the vehicle, written to a file.
 *
 * The format is upstream's: a line per parameter, the name and the value separated by a
 * comma.
 */
export async function exportParameters(path: string): Promise<void> {
  await call<void>("export_parameters", { path });
}

/**
 * The parameters of a file, read by the operator before they are written to the
 * vehicle.
 */
export async function importParameters(path: string): Promise<Parameter[]> {
  return call<Parameter[]>("import_parameters", { path });
}

/** How the parameters of the vehicle differ from the ones of a file. */
export async function compareParameters(
  path: string,
): Promise<ParameterDifference[]> {
  return call<ParameterDifference[]>("compare_parameters", { path });
}

/** Asks the vehicle to write its parameters to its own memory. */
export async function writeParametersToFlash(): Promise<void> {
  await call<void>("write_parameters_to_flash", {});
}

/** Where the vehicle is, in the systems an operator reads. */
export async function vehicleCoordinates(): Promise<Coordinates> {
  return call<Coordinates>("vehicle_coordinates");
}

/**
 * The elevation of the terrain along a plan.
 *
 * The ground is fetched from the service of ArduPilot, and the tiles it answers with
 * are kept on the disk, so that the second look at the same place is a look at the disk.
 */
export async function planTerrainProfile(
  kind: PlanKind,
  items: readonly PlanItem[],
): Promise<TerrainPoint[]> {
  return call<TerrainPoint[]>("plan_terrain_profile", { kind, items });
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
  gps2: null,
  efi: null,
  wind: null,
    imu: null,
    barometer: null,
    controls: null,
    airspeed: null,
    ekf: null,
    vibration: null,
    batteries: [],
    ahrs2: null,
    controller: null,
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
