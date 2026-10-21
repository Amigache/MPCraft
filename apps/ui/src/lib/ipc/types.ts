/**
 * The typed IPC contract between the Rust backend and the frontend.
 *
 * These types mirror the Rust structs in `crates/mpcraft-app` exactly. If a Rust type
 * changes, this file changes with it — and the change is visible in review.
 *
 * The frontend never sees a MAVLink message id or a `MAV_CMD` value. It sees intent-level
 * operations and named telemetry fields.
 */

/** Identifies a vehicle on the link. */
export interface VehicleKey {
  readonly systemId: number;
  readonly componentId: number;
}

/** The connection state of the application. */
export type ConnectionStatus =
  | "disconnected"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "failed";

/** A geographic position. */
export interface GeoPoint {
  readonly lat: number;
  readonly lng: number;
  readonly alt: number;
}

/** Vehicle attitude. */
export interface Attitude {
  readonly roll: number;
  readonly pitch: number;
  readonly yaw: number;
}

/** Velocity in the local north-east-down frame. */
export interface Velocity {
  readonly north: number;
  readonly east: number;
  readonly down: number;
  readonly groundSpeed: number;
}

/** Battery state. */
export interface Battery {
  readonly voltage: number;
  readonly current: number;
  readonly remaining: number;
}

/** The state of the global positioning. */
export interface GpsStatus {
  /** The type of the fix, as the MAVLink enumeration. */
  readonly fixType: number;
  /** The number of satellites in use. */
  readonly satellites: number;
  /** The horizontal dilution of precision. */
  readonly hdop: number;
}

/** The inertial measurement of the vehicle. */
export interface Imu {
  /** The acceleration on each axis, in metres per second squared. */
  readonly acceleration: readonly [number, number, number];
  /** The angular rate on each axis, in radians per second. */
  readonly angularRate: readonly [number, number, number];
  /** The magnetic field on each axis, in gauss. */
  readonly magneticField: readonly [number, number, number];
  /** The temperature of the sensor, in degrees Celsius. */
  readonly temperature: number;
}

/** The barometer of the vehicle. */
export interface Barometer {
  /** The absolute pressure, in hectopascals. */
  readonly absolutePressure: number;
  /** The differential pressure, in hectopascals. */
  readonly differentialPressure: number;
  /** The temperature, in degrees Celsius. */
  readonly temperature: number;
}

/** The servos and the radio control. */
export interface Controls {
  /** The pulse width of each of the first eight outputs, in microseconds. */
  readonly outputs: readonly (number | null)[];
  /** The value of each of the first eight radio channels, in microseconds. */
  readonly inputs: readonly (number | null)[];
  /** The strength of the radio signal, as a percentage. */
  readonly signal: number | null;
}

/** The air data of the vehicle. */
export interface Airspeed {
  /** The speed through the air, in metres per second. */
  readonly airspeed: number | null;
  /** The speed over the ground, in metres per second. */
  readonly groundSpeed: number;
  /** The rate of climb, in metres per second. */
  readonly climb: number;
  /** The throttle, as a percentage. */
  readonly throttle: number;
}

/** The state of the estimator of the vehicle. */
export interface Ekf {
  /** The variance of the velocity estimate. */
  readonly velocityVariance: number;
  /** The variance of the horizontal position estimate. */
  readonly horizontalVariance: number;
  /** The variance of the vertical position estimate. */
  readonly verticalVariance: number;
  /** The variance of the compass. */
  readonly compassVariance: number;
  /** The variance of the terrain altitude. */
  readonly terrainVariance: number;
  /** The variance of the airspeed estimate. */
  readonly airspeedVariance: number;
}

/** The vibration of the airframe. */
export interface Vibration {
  /** The vibration on the first axis, in metres per second squared. */
  readonly x: number;
  /** The vibration on the second axis, in metres per second squared. */
  readonly y: number;
  /** The vibration on the third axis, in metres per second squared. */
  readonly z: number;
}

/** The state of one battery bank. */
export interface BatteryBank {
  /** The identifier the vehicle gives the bank. */
  readonly id: number;
  /** The voltage of each cell, in volts. */
  readonly cells: readonly number[];
  /** The current of the bank, in amperes. */
  readonly current: number;
  /** The remaining charge of the bank, as a percentage. */
  readonly remaining: number;
  /** The temperature of the bank, in degrees Celsius. */
  readonly temperature: number;
}

/** What the flight controller is asking the vehicle to do. */
export interface Controller {
  /** The roll the controller asks for, in radians. */
  readonly roll: number;
  /** The pitch the controller asks for, in radians. */
  readonly pitch: number;
  /** The error of the altitude, in metres. */
  readonly altitudeError: number;
  /** The error of the airspeed. */
  readonly airspeedError: number;
  /** The error of the cross-track, in metres. */
  readonly crossTrackError: number;
  /** The distance to the waypoint the vehicle is flying towards, in metres. */
  readonly waypointDistance: number;
}

/** Where the vehicle is, in the universal transverse Mercator grid. */
export interface Utm {
  /** The zone number, 1 to 60. */
  readonly zone: number;
  /** The latitude band letter. */
  readonly band: string;
  /** The easting, in metres. */
  readonly easting: number;
  /** The northing, in metres. */
  readonly northing: number;
}

/** Where the vehicle is, in the three systems an operator reads. */
export interface Coordinates {
  /** The latitude in decimal degrees, positive north. */
  readonly lat: number;
  /** The longitude in decimal degrees, positive east. */
  readonly lng: number;
  /** The altitude above mean sea level, in metres. */
  readonly alt: number;
  /** The position in the universal transverse Mercator grid. */
  readonly utm: Utm | null;
  /** The position in the military grid reference system. */
  readonly mgrs: string | null;
}

/** The wind where the vehicle is. */
export interface Wind {
  /** The direction the wind comes from, in degrees. */
  readonly direction: number;
  /** The speed of the wind, in metres per second. */
  readonly speed: number;
  /** The vertical speed of the air, in metres per second. */
  readonly verticalSpeed: number;
}

/** The decoded telemetry snapshot for one vehicle. */
export interface Telemetry {
  readonly attitude: Attitude | null;
  readonly position: GeoPoint | null;
  readonly velocity: Velocity | null;
  readonly battery: Battery | null;
  readonly mode: string | null;
  readonly armed: boolean | null;
  readonly home: GeoPoint | null;
  readonly distanceToHome: number | null;
  readonly bearingToHome: number | null;
  readonly mission: MissionProgress | null;
  readonly gps: GpsStatus | null;
  /** The state of the second receiver, when the vehicle has one. */
  readonly gps2: GpsStatus | null;

  /** The wind where the vehicle is. */
  readonly wind: Wind | null;
  readonly imu: Imu | null;
  readonly barometer: Barometer | null;
  readonly controls: Controls | null;
  readonly airspeed: Airspeed | null;
  readonly ekf: Ekf | null;
  readonly vibration: Vibration | null;
  readonly batteries: readonly BatteryBank[];
  readonly ahrs2: Attitude | null;
  readonly controller: Controller | null;
}

/** The progress of the plan on the vehicle. */
export interface MissionProgress {
  /** The item the vehicle is executing. */
  readonly seq: number;
  /** How many items the plan has. */
  readonly total: number;
  /** The state of the plan, as the MAVLink enumeration. */
  readonly state: number;
  /** Whether the plan is being run. */
  readonly inMissionMode: boolean;
}

/** A message the vehicle sent. */
export interface VehicleMessage {
  /** How serious the message is, as the MAVLink enumeration. */
  readonly severity: number;
  /** The name of the severity, for example `WARNING`. */
  readonly severityName: string;
  /** The text of the message. */
  readonly text: string;
}

/** A flight mode the vehicle can be in. */
export interface FlightMode {
  /** The custom mode value the vehicle expects. */
  readonly value: number;
  /** The name of the mode. */
  readonly name: string;
}

/**
 * A connection the operator has named and can come back to.
 *
 * The profile is deliberately small: a name to show in a list, the target of the
 * connection, and whether the link is opened when the application starts.
 */
export interface ConnectionProfile {
  /** The name the profile is listed under. */
  readonly name: string;
  /** The connection target, for example `udp:127.0.0.1:14550`. */
  readonly target: string;
  /** Whether the profile is opened when the application starts. */
  readonly autoConnect: boolean;
}

/** A serial port on this machine. */
export interface SerialPort {
  /** The device name, for example `COM3` or `/dev/ttyUSB0`. */
  readonly device: string;
  /** A human-readable description of the device. */
  readonly description: string | null;
}

/** One series of a graph. */
export interface SeriesInfo {
  /** The name of the series. */
  readonly name: string;
  /** The unit the values are in. */
  readonly unit: string;
}

/** A parameter of the vehicle. */
export interface Parameter {
  /** The name of the parameter. */
  readonly name: string;
  /** The value. */
  readonly value: number;
  /** The type the vehicle reported, as the MAVLink enumeration. */
  readonly reportedType: number;
}

/** How the value of a parameter differs between the vehicle and a file. */
export interface ParameterDifference {
  /** The name of the parameter. */
  readonly name: string;
  /** The value the vehicle has, when it has one. */
  readonly current: number | null;
  /** The value the file has, when it has one. */
  readonly other: number | null;
}

/**
 * What is known about a parameter.
 *
 * The description, the unit, the range it takes and the value it had before anybody
 * changed it are the autopilots' knowledge, and the catalogue the backend keeps has
 * them for four thousand parameters.
 */
export interface ParameterMetadata {
  /** The name of the parameter. */
  readonly name: string;
  /** The name it is shown with. */
  readonly displayName: string;
  /** The group it belongs to, when the documentation knows one. */
  readonly group: string;
  /** The value it takes when it has never been changed. */
  readonly default: string;
  /** What it does, in a line. */
  readonly description: string;
  /** What it does, at length. */
  readonly longDescription: string;
  /** The unit the value is in. */
  readonly unit: string;
  /** The smallest value it takes. */
  readonly minimum: string;
  /** The largest value it takes. */
  readonly maximum: string;
  /** The values it can be given, when it is a choice. */
  readonly values: readonly string[];
  /** The vehicle families it belongs to. */
  readonly vehicles: readonly string[];
}

/** A command a plan can hold. */
export interface MavCommand {
  /** The name of the command, for example `MAV_CMD_NAV_WAYPOINT`. */
  readonly name: string;
  /** The value the command travels with. */
  readonly value: number;
  /** The role of each of the seven slots, in order. */
  readonly roles: readonly string[];
  /** The unit of each of the seven slots, in order. */
  readonly units: readonly string[];
  /** The name the specification gives it, for example `MAV_CMD_NAV_WAYPOINT`. */
  readonly canonicalName: string;
}

/** A warning about an item of a plan. */
export interface PlanWarning {
  /** The position of the item in the plan, when the warning is about one. */
  readonly seq: number | null;
  /** What is wrong with it, for example `zeroAltitude`. */
  readonly reason: string;
}

/** One line of the pre-flight checklist. */
export interface ChecklistEntry {
  /** What the item is about. */
  readonly description: string;
  /** The text of the item, with the value in it. */
  readonly text: string;
  /** Whether the condition holds. */
  readonly passed: boolean;
  /** Whether the value was known at all. */
  readonly known: boolean;
  /** Whether the pilot answers this one by hand. */
  readonly manual: boolean;
}

/** A graph the user can choose. */
/** One point of the profile of the terrain along a plan. */
export interface TerrainPoint {
  /** The distance from the start of the plan, in metres. */
  readonly distance: number;
  /** The elevation of the terrain, in metres above the sea. */
  readonly elevation: number;
  /** The height of the plan above the sea, when it is known. */
  readonly height: number | null;
  /** Whether the point is a place of the plan. */
  readonly waypoint: boolean;
}

/** What the outputs of the vehicle are for. */
export interface OutputFunction {
  /** The output, counted from one. */
  readonly output: number;
  /** The number the parameter carries. */
  readonly value: number;
  /** The name of the function, when the specification gives it one. */
  readonly name: string | null;
  /** The motor the function drives, when it drives one. */
  readonly motor: number | null;
}

export interface GraphPreset {
  /** The full name, as the interface shows it. */
  readonly name: string;
  /** The group it belongs to. */
  readonly group: string;
  /** The name inside the group. */
  readonly label: string;
  /** What the graph shows. */
  readonly description: string;
  /** Whether the graph comes from Mission Planner's catalogue. */
  readonly upstream: boolean;
  /** The series it draws. */
  readonly series: readonly SeriesInfo[];
}

/** The values of one series. */
export interface SeriesPoints {
  /** The name of the series. */
  readonly name: string;
  /** The unit the values are in. */
  readonly unit: string;
  /** How long after the start of the history each value is, in seconds. */
  readonly x: readonly number[];
  /** The values themselves. */
  readonly y: readonly number[];
}

/** The values of a graph over the history. */
export interface GraphSeries {
  /** The full name of the graph. */
  readonly name: string;
  /** What the graph shows. */
  readonly description: string;
  /** The series, each with its values. */
  readonly series: readonly SeriesPoints[];
}

/** The kind of plan. */
export type PlanKind = "mission" | "fence" | "rally";

/** One item of a plan. */
export interface PlanItem {
  /** The position in the plan. */
  readonly seq: number;
  /** The name of the command, for example `MAV_CMD_NAV_WAYPOINT`. */
  readonly command: string;
  /** The numeric command value. */
  readonly commandValue: number;
  /** The coordinate frame the position is expressed in. */
  readonly frame: number;
  /** The four generic parameters. */
  readonly params: readonly [number, number, number, number];
  /** The latitude, for a command that carries a position. */
  readonly x: number;
  /** The longitude, for a command that carries a position. */
  readonly y: number;
  /** The altitude. */
  readonly z: number;
  /** Whether the item is the one being executed. */
  readonly current: boolean;
  /** Whether the vehicle continues to the next item automatically. */
  readonly autocontinue: boolean;
}

/** A plan, as the interface sees it. */
export interface Plan {
  /** The kind of plan. */
  readonly kind: PlanKind;
  /** The items, in order. */
  readonly items: readonly PlanItem[];
}

/** The vehicle a command is for. */
export interface VehicleRequest {
  /** The system id of the vehicle. Omit it for the primary one on the link. */
  readonly systemId?: number;
  /** The component id of the vehicle. Omit it for the primary one on the link. */
  readonly componentId?: number;
}

/** A vehicle on the link. */
export interface VehicleEvent {
  /** The system id. */
  readonly systemId: number;
  /** The component id. */
  readonly componentId: number;
  /** The vehicle family, when it is known. */
  readonly kind: string | null;
}

/** The loss statistics of the link. */
export interface LossStats {
  /** The number of packets received. */
  readonly received: number;
  /** The number of packets lost. */
  readonly lost: number;
}

/** The result of a command that changed the connection state. */
export interface ConnectResponse {
  readonly status: ConnectionStatus;
  readonly vehicle: VehicleKey | null;
}

/** The shape of every command failure. */
export interface CommandError {
  /**
   * A stable, machine-readable error code.
   *
   * Branch on this, never on the message text: the text is for humans and may be
   * localised or reworded.
   */
  readonly code: string;
  /** A human-readable description, suitable for display. */
  readonly message: string;
}

/** The request payload of the `connect` command. */
export interface ConnectRequest {
  /** The connection target, for example `udp:127.0.0.1:14550`. */
  readonly target: string;
}
