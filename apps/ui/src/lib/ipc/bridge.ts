import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";

import type {
  AdsbRelayStatus,
  BoardInfo,
  CameraDevice,
  ChecklistEntry,
  CommandAnswer,
  CommandError,
  ConnectRequest,
  ConnectionProfile,
  ConnectResponse,
  ConnectionStatus,
  Coordinates,
  DronecanReport,
  DtedLibraryInfo,
  FirmwareManifest,
  FlightMode,
  FormationRequest,
  FormationStatus,
  FtpEntry,
  FtpFile,
  GraphPreset,
  GraphSeries,
  GeoPoint,
  GimbalDevice,
  LogPlace,
  LogSummary,
  PlaybackInfo,
  LogTrackPoint,
  MapProvider,
  MessageSamples,
  MavCommand,
  NtripStatus,
  OutputFunction,
  OsdSlot,
  Parameter,
  ParameterDifference,
  ParameterMetadata,
  PicturePlace,
  Plan,
  PlanItem,
  PlanKind,
  PlanWarning,
  PluginInfo,
  SelectedVehicle,
  SerialPort,
  SigningKeyEntry,
  SigningStatus,
  StateEvent,
  StreamAnswer,
  TerrainPathPoint,
  TerrainPoint,
  TileBounds,
  TransponderStatus,
  VehicleKey,
  VehicleLog,
  VehicleRequest,
  VideoStream,
  ZoomOption,
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
    if (
      typeof candidate.code === "string" &&
      typeof candidate.message === "string"
    ) {
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

/**
 * Chooses which vehicle the station speaks to.
 *
 * The answer is what the backend says about itself after the choice — the name and the mark come
 * from the registry — so the interface cannot show a vehicle as the one it is reading when the
 * link reads another.
 */
export async function selectVehicle(
  vehicle: VehicleKey,
): Promise<SelectedVehicle> {
  return call<SelectedVehicle>("select_vehicle", vehicleRequest(vehicle));
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

/**
 * Starts sending the corrections of a base station to the vehicle.
 *
 * The address of the caster is the one the operator gives:
 * `http://user:password@host:port/mount`. The corrections go to the port of the GPS of
 * the vehicle.
 */
export async function ntripStart(
  url: string,
  gpsPort: number,
  version1: boolean,
): Promise<void> {
  await call<void>("ntrip_start", {
    req: {},
    url,
    gpsPort,
    version1,
  });
}

/** Stops sending the corrections of a base station. */
export async function ntripStop(): Promise<void> {
  await call<void>("ntrip_stop");
}

/** What the corrections of a base station are doing. */
export async function ntripStatus(): Promise<NtripStatus> {
  return call<NtripStatus>("ntrip_status");
}

/**
 * Starts telling the vehicle what its transponder is to say.
 *
 * The vehicle relays the message, and the aircraft around it see it.
 */
export async function transponderStart(
  icaoAddress: number,
  flightId: string,
  squawk: number,
): Promise<void> {
  await call<void>("transponder_start", {
    req: {},
    icaoAddress,
    flightId,
    squawk,
  });
}

/** Stops telling the vehicle what its transponder is to say. */
export async function transponderStop(): Promise<void> {
  await call<void>("transponder_stop");
}

/** What the transponder is doing. */
export async function transponderStatus(): Promise<TransponderStatus> {
  return call<TransponderStatus>("transponder_status");
}

/**
 * What the keyring says about the keys the operator wrote.
 *
 * The answer keeps every line that was written and names the secrets that could not be read as
 * thirty-two bytes, so a key written wrong is seen as what was written rather than as an empty list.
 */
export async function signingStatus(): Promise<SigningStatus> {
  return call<SigningStatus>("signing_status");
}

/**
 * Saves the keys of the signed link and says what the keyring makes of them.
 *
 * The list that is sent is the list that is kept, in the order it was sent, and the link that is
 * opened next is signed with the first secret in it that can be read.
 */
export async function setSigningKeys(
  keys: readonly SigningKeyEntry[],
): Promise<SigningStatus> {
  return call<SigningStatus>("set_signing_keys", { keys });
}

/**
 * What the formation is doing.
 *
 * The places and the decision are read from the file, so this works without a link: a fleet can be
 * described before any board is on the wire.
 */
export async function formationStatus(): Promise<FormationStatus> {
  return call<FormationStatus>("swarm_status");
}

/**
 * Saves what the operator wrote for the formation and puts it to work.
 *
 * The names are sent as they were written, including one that cannot be read as a vehicle: the
 * answer says which of them the backend left out, and the file keeps what was actually written.
 */
export async function setFormation(
  req: FormationRequest,
): Promise<FormationStatus> {
  return call<FormationStatus>("set_swarm_settings", { req });
}

/** The plugins the folder holds, with what the operator chose. */
export async function listPlugins(): Promise<PluginInfo[]> {
  return call<PluginInfo[]>("list_plugins");
}

/** Marks a plugin as run, or leaves it alone, and keeps the decision for the next run. */
export async function setPluginEnabled(
  name: string,
  enabled: boolean,
): Promise<void> {
  await call<void>("set_plugin_enabled", { name, enabled });
}

/** What the plugins said, most recent last, which is what the message console reads. */
export async function pluginNotes(): Promise<string[]> {
  return call<string[]>("plugin_notes");
}

/** The folder the plugins are read from, which is where an operator puts one. */
export async function pluginFolder(): Promise<string> {
  return call<string>("plugin_folder");
}

/**
 * Starts sending the traffic that the station reads onto the link.
 *
 * The aircraft the vehicle sent are already on the link. The ones the station read from a
 * source of its own are what the relay adds, one each second, so that the other stations
 * listening to this link see them too.
 */
export async function adsbRelayStart(
  server: string,
  radiusNm: number,
): Promise<void> {
  await call<void>("adsb_relay_start", { req: {}, server, radiusNm });
}

/** Stops sending the traffic of the station onto the link. */
export async function adsbRelayStop(): Promise<void> {
  await call<void>("adsb_relay_stop");
}

/** What the relay of the traffic is doing. */
export async function adsbRelayStatus(): Promise<AdsbRelayStatus> {
  return call<AdsbRelayStatus>("adsb_relay_status");
}

/**
 * Reads the nodes of the DroneCAN bus.
 *
 * The call also applies the rule that says when the bus is asked for what its nodes are: once when
 * the link opens, and once more for a node that has reported how it is without saying what it is.
 * What comes back says whether this reading put the ask on the wire, so the sheet can show that
 * rather than an operator guessing it.
 */
export async function dronecanScan(
  req: VehicleRequest = {},
): Promise<DronecanReport> {
  return call<DronecanReport>("dronecan_scan", { req });
}

/** Asks the camera of the vehicle what video streams it has. */
export async function videoStreams(): Promise<VideoStream[]> {
  return call<VideoStream[]>("video_streams", { req: {} });
}

/** Starts one of the camera's video streams. */
export async function videoStreamStart(
  componentId: number,
  streamId: number,
): Promise<void> {
  await call<void>("video_stream_start", { req: {}, componentId, streamId });
}

/** Stops one of the camera's video streams. */
export async function videoStreamStop(
  componentId: number,
  streamId: number,
): Promise<void> {
  await call<void>("video_stream_stop", { req: {}, componentId, streamId });
}

/**
 * What the gimbals of the vehicle can do.
 *
 * What a gimbal says about itself decides which controls the sheet offers: the program that
 * knows the protocol refuses an angle before anything is sent, so the interface only has to
 * show what the vehicle answered.
 */
export async function gimbalDevices(
  vehicle?: VehicleKey,
): Promise<GimbalDevice[]> {
  return call<GimbalDevice[]>("gimbal_devices", vehicleRequest(vehicle));
}

/** Points a gimbal at an angle, in degrees. */
export async function setGimbalAngles(
  deviceId: number,
  pitch: number,
  yaw: number,
  yawLock: boolean,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("set_gimbal_angles", {
    ...vehicleRequest(vehicle),
    deviceId,
    pitch,
    yaw,
    yawLock,
  });
}

/** Moves a gimbal at a rate, without waiting for the vehicle to say it took it. */
export async function slewGimbal(
  deviceId: number,
  pitch: number,
  yaw: number,
  pitchRate: number,
  yawRate: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("slew_gimbal", {
    ...vehicleRequest(vehicle),
    deviceId,
    pitch,
    yaw,
    pitchRate,
    yawRate,
  });
}

/** Puts a gimbal in its safe position. */
export async function retractGimbal(
  deviceId: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("retract_gimbal", { ...vehicleRequest(vehicle), deviceId });
}

/** Puts a gimbal in its default position. */
export async function neutralGimbal(
  deviceId: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("neutral_gimbal", { ...vehicleRequest(vehicle), deviceId });
}

/** Says whether the yaw of a gimbal is held against the ground or follows the vehicle. */
export async function setGimbalYawLock(
  deviceId: number,
  yawLock: boolean,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("set_gimbal_yaw_lock", {
    ...vehicleRequest(vehicle),
    deviceId,
    yawLock,
  });
}

/** Points a gimbal at a place on the ground. */
export async function pointGimbalAtLocation(
  deviceId: number,
  latitude: number,
  longitude: number,
  altitude: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("point_gimbal_at_location", {
    ...vehicleRequest(vehicle),
    deviceId,
    latitude,
    longitude,
    altitude,
  });
}

/** Takes the point of interest away from a gimbal. */
export async function clearGimbalRoi(
  deviceId: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("clear_gimbal_roi", {
    ...vehicleRequest(vehicle),
    deviceId,
  });
}

/** Points a gimbal at another vehicle. */
export async function pointGimbalAtVehicle(
  deviceId: number,
  systemId: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("point_gimbal_at_vehicle", {
    ...vehicleRequest(vehicle),
    deviceId,
    systemId,
  });
}

/** Asks the vehicle what its cameras can do, and asks them to keep saying what they are doing. */
export async function readCamera(
  vehicle?: VehicleKey,
): Promise<CameraDevice[]> {
  return call<CameraDevice[]>("read_camera", vehicleRequest(vehicle));
}

/** The kinds of zoom a camera can be asked for, as the backend lists them. */
export async function cameraZoomOptions(): Promise<ZoomOption[]> {
  return call<ZoomOption[]>("camera_zoom_options");
}

/**
 * Asks a camera to take one picture.
 *
 * A camera answers on its own component, so the command names the component that reported it.
 */
export async function capturePhoto(
  cameraComponentId: number,
  cameraIndex: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("capture_photo", {
    ...vehicleRequest(vehicle),
    cameraComponentId,
    cameraIndex,
  });
}

/** Asks a camera to take pictures at an interval, in seconds. */
export async function startIntervalCapture(
  cameraComponentId: number,
  cameraIndex: number,
  intervalSeconds: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("start_interval_capture", {
    ...vehicleRequest(vehicle),
    cameraComponentId,
    cameraIndex,
    intervalSeconds,
  });
}

/** Stops the pictures a camera was taking at an interval. */
export async function stopIntervalCapture(
  cameraComponentId: number,
  cameraIndex: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("stop_interval_capture", {
    ...vehicleRequest(vehicle),
    cameraComponentId,
    cameraIndex,
  });
}

/** Asks a camera to start recording video. */
export async function startRecording(
  cameraComponentId: number,
  streamId: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("start_recording", {
    ...vehicleRequest(vehicle),
    cameraComponentId,
    streamId,
  });
}

/** Stops the video a camera was recording. */
export async function stopRecording(
  cameraComponentId: number,
  streamId: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("stop_recording", {
    ...vehicleRequest(vehicle),
    cameraComponentId,
    streamId,
  });
}

/** Sets the zoom of a camera, with one of the kinds of zoom the backend listed. */
export async function setCameraZoom(
  cameraComponentId: number,
  level: number,
  zoomType: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("set_camera_zoom", {
    ...vehicleRequest(vehicle),
    cameraComponentId,
    level,
    zoomType,
  });
}

/**
 * Asks a camera to follow a point in its picture.
 *
 * The place is given from `-1` to `1`. Picking it from the picture itself waits on the video
 * player, so the numbers are typed here.
 */
export async function trackPoint(
  cameraComponentId: number,
  x: number,
  y: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("track_point", {
    ...vehicleRequest(vehicle),
    cameraComponentId,
    x,
    y,
  });
}

/** Asks a camera to follow an area of its picture. */
export async function trackRectangle(
  cameraComponentId: number,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("track_rectangle", {
    ...vehicleRequest(vehicle),
    cameraComponentId,
    x1,
    y1,
    x2,
    y2,
  });
}

/**
 * The slots of the on-screen display, and what each of them holds.
 *
 * The screens five and six are the ones the operator chooses. The answer is the name
 * of the parameter each slot holds, when the vehicle says it.
 */
export async function osdSlots(): Promise<OsdSlot[]> {
  return call<OsdSlot[]>("osd_slots", { req: {} });
}

/**
 * Tells the vehicle what a slot of the on-screen display is to hold.
 *
 * The slot holds a parameter, and the range of the parameter is said with it.
 */
export async function osdSlotConfig(
  screen: number,
  index: number,
  name: string,
  min: number,
  max: number,
  increment: number,
): Promise<void> {
  await call<void>("osd_slot_config", {
    req: {},
    slot: { screen, index, name, min, max, increment },
  });
}

/** The sources of tiles the map can draw with. */
export async function listMapProviders(): Promise<MapProvider[]> {
  return call<MapProvider[]>("list_map_providers");
}

/**
 * One tile of the map.
 *
 * The tile is taken from the folder the operator has brought, then from the disk where
 * the tiles that were fetched are kept, and from the service when it is in neither.
 */
export async function mapTile(
  provider: string,
  zoom: number,
  x: number,
  y: number,
): Promise<number[]> {
  return call<number[]>("map_tile", { provider, zoom, x, y });
}

/** The folder of tiles the operator has brought, when there is one. */
export async function mapInjectionFolder(): Promise<string | null> {
  return call<string | null>("map_injection_folder");
}

/** Says which folder of tiles the operator has brought. */
export async function setMapInjectionFolder(
  folder: string | null,
): Promise<void> {
  await call<void>("set_map_injection_folder", { folder });
}

/**
 * Downloads the plan the vehicle keeps in a file of its own.
 *
 * The plan is not asked for with the mission protocol: it is read from the file the
 * vehicle keeps it in, over the file transfer protocol.
 */
export async function downloadMissionFtp(kind: PlanKind): Promise<Plan> {
  return call<Plan>("download_mission_ftp", { kind });
}

/**
 * Writes a plan to the file the vehicle keeps it in.
 *
 * The plan is not written with the mission protocol: it is written to the file the
 * vehicle keeps it in, over the file transfer protocol.
 */
export async function uploadMissionFtp(
  items: readonly PlanItem[],
  kind: PlanKind,
): Promise<void> {
  await call<void>("upload_mission_ftp", { kind, items });
}

/**
 * Writes a part of a plan to the vehicle, leaving the rest of it alone.
 *
 * The vehicle is told which range of the plan is coming, and it asks for the items one
 * at a time. The answer says which items the vehicle took.
 */
export async function uploadMissionPartial(
  kind: PlanKind,
  start: number,
  items: readonly PlanItem[],
  vehicle?: VehicleKey,
): Promise<{ start: number; accepted: number[] }> {
  return call<{ start: number; accepted: number[] }>("upload_mission_partial", {
    ...vehicleRequest(vehicle),
    kind,
    start,
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
export async function readParameters(
  vehicle?: VehicleKey,
): Promise<Parameter[]> {
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
  await call<void>("manual_control", {
    req: {},
    roll,
    pitch,
    thrust,
    yaw,
    buttons,
  });
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

/**
 * Turns one of the relays of the vehicle on or off.
 *
 * A relay the vehicle has said it does not have is refused by the backend before anything is sent,
 * so the error says what the vehicle cannot do rather than what this command failed to do.
 */
export async function setRelay(
  instance: number,
  on: boolean,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("set_relay", { ...vehicleRequest(vehicle), instance, on });
}

/**
 * Asks the vehicle once for the report of its relays.
 *
 * The vehicle does not say what its relays are unless it is asked, so the panel asks when it is
 * opened; what it draws afterwards is the state the event carries, not the answer of this call.
 */
export async function requestRelayStatus(vehicle?: VehicleKey): Promise<void> {
  await call<void>("request_relay_status", vehicleRequest(vehicle));
}

/**
 * Asks the vehicle what it is capable of.
 *
 * The capabilities decide which encoding the items of a plan use, so the plan editor asks once when the
 * link is open; what it uses afterwards is the state of the link rather than the answer of this call.
 */
export async function readCapabilities(vehicle?: VehicleKey): Promise<void> {
  await call<void>("read_capabilities", vehicleRequest(vehicle));
}

/**
 * Asks the vehicle to stream what the flight-data window reads.
 *
 * A vehicle sends by the rates configured on its port, and those rates can all be zero, so a
 * station that does not ask sees nothing. The panel asks once when the link is open; the answers
 * are what the vehicle said to each of the asks, and this function does not decide what they mean.
 */
export async function requestTelemetryStreams(
  vehicle?: VehicleKey,
): Promise<StreamAnswer[]> {
  return call<StreamAnswer[]>(
    "request_telemetry_streams",
    vehicleRequest(vehicle),
  );
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
 * The pictures of a folder, read one by one.
 *
 * A picture carries where it was taken inside itself, and the size it has, which is what makes
 * a footprint on the map rather than a point.
 */
export async function readPictures(folder: string): Promise<PicturePlace[]> {
  return call<PicturePlace[]>("read_pictures", { folder });
}

/**
 * The places a log says the vehicle was, with the instants.
 *
 * Only the messages that carry a position are read, because those are the ones that say where
 * the vehicle was.
 */
export async function logPlaces(path: string): Promise<LogPlace[]> {
  return call<LogPlace[]>("log_places", { path });
}

/**
 * The difference between the clock of a camera and the clock of the log, in seconds.
 *
 * `null` when the instants of the folder cannot be lined up with the log at all.
 */
export async function estimatePictureOffset(
  folder: string,
  path: string,
): Promise<number | null> {
  return call<number | null>("estimate_picture_offset", { folder, path });
}

/**
 * The pictures, with the place the log says the vehicle was when each of them was made.
 */
export async function matchPictures(
  folder: string,
  path: string,
  offset: number,
  lag: number,
): Promise<PicturePlace[]> {
  return call<PicturePlace[]>("match_pictures", { folder, path, offset, lag });
}

/**
 * Copies of the pictures with their places written into them.
 *
 * Answers how many copies were written. A picture whose instant is not in the log and that
 * carries no place of its own is left where it is.
 */
export async function writeGeotagged(
  folder: string,
  output: string,
  path: string,
  offset: number,
  lag: number,
): Promise<number> {
  return call<number>("write_geotagged", { folder, output, path, offset, lag });
}

/**
 * The values of a graph over a log.
 *
 * The graph is the same one the live view draws, read from the log instead of from the
 * link.
 */
export async function logSeries(
  path: string,
  name: string,
): Promise<GraphSeries> {
  return call<GraphSeries>("log_series", { path, name });
}

/**
 * Writes the track of a log as a KML document.
 *
 * The document is the one Google Earth reads.
 */
export async function logExportKml(log: string, output: string): Promise<void> {
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

/**
 * Asks the vehicle where its home is.
 *
 * The vehicle is asked with the command that upstream sends, and the answer is the
 * message the vehicle sends when its home is known.
 */
export async function homePosition(vehicle?: VehicleKey): Promise<GeoPoint> {
  return call<GeoPoint>("home_position", vehicleRequest(vehicle));
}

/**
 * Moves the home of the vehicle.
 *
 * The vehicle is told where the home is, and it confirms the move by reporting the home
 * it now holds. A vehicle that refuses the move keeps the one it had.
 */
export async function setHomePosition(
  position: GeoPoint,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("set_home_position", {
    ...vehicleRequest(vehicle),
    position,
  });
}

/**
 * Opens a folder of DTED cells.
 *
 * The cells are the heights of the ground in the military format, and they are what a
 * survey of a field is planned with.
 */
export async function openDtedFolder(path: string): Promise<DtedLibraryInfo> {
  return call<DtedLibraryInfo>("open_dted_folder", { path });
}

/**
 * The height of the ground at a place, in metres above the sea.
 *
 * The height comes from the folder of cells that is open.
 */
export async function dtedElevation(
  latitude: number,
  longitude: number,
): Promise<number> {
  return call<number>("dted_elevation", { latitude, longitude });
}

/**
 * The zigzag of a survey that follows the ground.
 *
 * The vehicle flies at a constant height above the ground, which is what a camera that
 * needs the same distance to the ground everywhere needs.
 */
export async function dtedPath(
  from: GeoPoint,
  to: GeoPoint,
  stripLength: number,
  stripSpacing: number,
  clearance: number,
  sampleStep: number,
): Promise<TerrainPathPoint[]> {
  return call<TerrainPathPoint[]>("dted_path", {
    from,
    to,
    stripLength,
    stripSpacing,
    clearance,
    sampleStep,
  });
}

/**
 * The manifest of the firmware: the vehicles and the versions of their firmware.
 *
 * The manifest is published by ArduPilot, and it is the list of what can be installed.
 */
export async function firmwareManifest(): Promise<FirmwareManifest> {
  return call<FirmwareManifest>("firmware_manifest");
}

/**
 * Fetches the file of a version of the firmware and writes it down.
 *
 * The answer is how many bytes were written.
 */
export async function firmwareDownload(
  url: string,
  output: string,
): Promise<number> {
  return call<number>("firmware_download", { url, output });
}

/**
 * Writes a firmware to a board, over the port the board is on.
 *
 * The board has to be in its bootloader: it answers the questions of the protocol
 * instead of the MAVLink of a flight. The answer is what the board said about itself.
 *
 * `allowSame` is what the operator says when the board already holds this image; without it the
 * board is not erased for something that is already there.
 */
export async function firmwareWriteToBoard(
  port: string,
  file: string,
  allowSame: boolean,
): Promise<BoardInfo> {
  return call<BoardInfo>("firmware_write_to_board", { port, file, allowSame });
}

/**
 * Erases the firmware of a board, over the port the board is on.
 *
 * The board has to be in its bootloader. It is left without a firmware, which is what
 * the operator wants when the board does not start.
 */
export async function firmwareEraseBoard(port: string): Promise<BoardInfo> {
  return call<BoardInfo>("firmware_erase_board", { port });
}

/**
 * Writes the sector of the bootloader that the vehicle carries in itself.
 *
 * This is a write rather than a restart: the vehicle answers and keeps speaking as a vehicle, so the
 * board has to be put into its bootloader before a file can be written over the port.
 */
export async function flashBootloader(): Promise<void> {
  await call<void>("flash_bootloader", { req: {} });
}

/**
 * Asks the vehicle to restart and stay in its bootloader.
 *
 * The answer is what the vehicle said, named the way the protocol names it, kept apart from a board
 * that never answered. When the board is there the link stops carrying MAVLink afterwards, because the
 * board is then waiting for the protocol of the bootloader.
 */
export async function rebootIntoBootloader(
  vehicle?: VehicleKey,
): Promise<CommandAnswer> {
  return call<CommandAnswer>("reboot_into_bootloader", vehicleRequest(vehicle));
}

/**
 * Lists the files the vehicle holds in a folder.
 *
 * The list is what the vehicle answered with: the name of every entry, whether it is a
 * folder, and how big it is.
 */
export async function ftpList(path: string): Promise<FtpEntry[]> {
  return call<FtpEntry[]>("ftp_list", { path });
}

/**
 * Reads a file the vehicle holds.
 *
 * The bytes are returned as they were read. When a place is given, the file is written
 * down there as well.
 */
export async function ftpRead(path: string, output?: string): Promise<FtpFile> {
  return call<FtpFile>("ftp_read", { path, output: output ?? null });
}

/**
 * Starts the web server, so that another program can follow the flight.
 *
 * The answer is the port it listens on.
 */
export async function httpServerStart(port?: number): Promise<number> {
  return call<number>("http_server_start", { port: port ?? null });
}

/** Stops the web server. */
export async function httpServerStop(): Promise<void> {
  await call<void>("http_server_stop");
}

/**
 * Writes bytes to a serial port of the vehicle.
 *
 * The bytes travel inside a MAVLink message and the vehicle hands them over to the port.
 * The speed of the port is left alone when it is zero.
 */
export async function serialControl(
  device: number,
  data: string,
  baudrate: number,
  timeout: number,
): Promise<void> {
  await call<void>("serial_control", {
    req: {},
    device,
    data,
    baudrate,
    timeout,
  });
}

/**
 * Writes a bundle with what the application knows about itself.
 *
 * The bundle is a zip with a file per subject: the machine, the connection profiles and
 * the state of the link. It is what is sent to whoever is asked to look at a failure.
 */
export async function diagnosticsBundle(output: string): Promise<string> {
  return call<string>("diagnostics_bundle", { output });
}

/**
 * Starts writing the track of the flight to a KML file.
 *
 * The file is written again and again while the vehicle flies, so that another program
 * can follow the flight as it happens. The answer is the name of the file.
 */
export async function startLiveKml(path: string): Promise<string> {
  return call<string>("start_live_kml", { path });
}

/**
 * Stops writing the track of the flight to a KML file.
 *
 * The file is written one last time, so that it holds the whole flight. The answer is
 * the name of the file.
 */
export async function stopLiveKml(): Promise<string> {
  return call<string>("stop_live_kml");
}

/**
 * Opens a log to be played back.
 *
 * The flight is read once, and the window can then be moved through it.
 */
export async function playbackOpen(path: string): Promise<PlaybackInfo> {
  return call<PlaybackInfo>("playback_open", { path });
}

/**
 * Moves the play-back of the log to the given second of the flight.
 *
 * The telemetry of that instant is published, and the window is drawn as the flight
 * was at that moment.
 */
export async function playbackSeek(seconds: number): Promise<void> {
  await call<void>("playback_seek", { seconds });
}

/** Closes the log that is being played back. */
export async function playbackClose(): Promise<void> {
  await call<void>("playback_close", {});
}

/**
 * The logs the vehicle holds in its own memory.
 *
 * A log that is only on the vehicle is a log that is lost when the vehicle is
 * reformatted.
 */
export async function vehicleLogs(vehicle?: VehicleKey): Promise<VehicleLog[]> {
  return call<VehicleLog[]>("vehicle_logs", vehicleRequest(vehicle));
}

/** Fetches a log of the vehicle and writes it to a file. */
export async function downloadLog(
  id: number,
  size: number,
  path: string,
  vehicle?: VehicleKey,
): Promise<void> {
  await call<void>("download_log", {
    ...vehicleRequest(vehicle),
    id,
    size,
    path,
  });
}

/**
 * The samples of one message of a log.
 *
 * The values are the ones the message carries, field by field.
 */
export async function logMessageSamples(
  path: string,
  name: string,
  limit = 200,
): Promise<MessageSamples> {
  return call<MessageSamples>("log_message_samples", { path, name, limit });
}

/**
 * Fetches the tiles of a region and keeps them on the disk.
 *
 * A survey is flown where there is no signal: the tiles of the field are fetched
 * before the flight, and the map is drawn with the ones that are there.
 */
export async function prefetchTiles(
  template: string,
  provider: string,
  bounds: TileBounds,
  zooms: readonly number[],
): Promise<number> {
  return call<number>("prefetch_tiles", {
    template,
    provider,
    bounds,
    zooms: [...zooms],
  });
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
export async function importPlan(format: string, path: string): Promise<Plan> {
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

/**
 * Sends one command of the catalogue to the vehicle.
 *
 * The seven slots are sent as the operator wrote them; a slot the command does not use is sent as
 * zero. The answer is what the vehicle reported, named the way the protocol names it — this
 * function does not decide what the answer means.
 */
export async function runMavCommand(
  command: number,
  params: readonly number[],
  vehicle?: VehicleKey,
): Promise<CommandAnswer> {
  const slots = Array.from({ length: 7 }, (_, index) => params[index] ?? 0);

  return call<CommandAnswer>("run_mav_command", {
    ...vehicleRequest(vehicle),
    command,
    params: slots,
  });
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
    // The state arrives as one document, so this reads the document the contract declares rather than a
    // shape of its own: only the connection status is shown here, but reading it through a private shape
    // is what made the same tick mean two things to two readers.
    const unlisten = listen<StateEvent>("link-state", (event) => {
      setStatus(event.payload.status);
    });

    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  return { status, error, connect, disconnect };
}

/** The state of a link that is not open. */
const EMPTY_STATE: StateEvent = {
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
    droneId: null,
    wind: null,
    passthrough: null,
    imu: null,
    barometer: null,
    controls: null,
    relays: null,
    airspeed: null,
    ekf: null,
    vibration: null,
    batteries: [],
    ahrs2: null,
    controller: null,
  },
  loss: { received: 0, lost: 0 },
  messages: [],
  board: null,
  output: { lines: [], dropped: 0, looksLikeCanBus: false },
  adsb: [],
  gimbals: [],
  cameras: [],
};

/**
 * Subscribes to the state of the link.
 *
 * The backend publishes the whole state in one event, so the components that show it
 * share one subscription rather than each opening their own.
 */
export function useLinkState(): StateEvent {
  const [state, setState] = useState<StateEvent>(EMPTY_STATE);

  useEffect(() => {
    const unlisten = listen<StateEvent>("link-state", (event) => {
      setState(event.payload);
    });

    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  return state;
}
