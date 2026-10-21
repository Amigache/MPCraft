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
}

/** A warning about an item of a plan. */
export interface PlanWarning {
  /** The position of the item in the plan. */
  readonly seq: number;
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
