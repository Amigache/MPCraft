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
  "disconnected" | "connecting" | "connected" | "reconnecting" | "failed";

/** A geographic position. */
export interface GeoPoint {
  readonly lat: number;
  readonly lng: number;
  readonly alt: number;
}

/** One cell of a folder of DTED files. */
export interface DtedCell {
  readonly name: string;
  readonly level: string;
  readonly originLat: number;
  readonly originLng: number;
  readonly spacingLat: number;
  readonly spacingLng: number;
  readonly columns: number;
  readonly rows: number;
  readonly checksumFailures: number;
}

/** What a folder of DTED files holds. */
export interface DtedLibraryInfo {
  readonly cells: readonly DtedCell[];
  readonly failures: number;
}

/** What a board said about itself. */
export interface BoardInfo {
  readonly blRev: number;
  readonly boardId: number;
  readonly boardRev: number;
  readonly flashSize: number;
}

/** What came back from a serial port of the vehicle. */
export interface PassthroughData {
  readonly device: number;
  readonly data: readonly number[];
}

/** One source of tiles the map can draw with. */
export interface MapProvider {
  /** The identifier the choice is kept under. */
  readonly id: string;
  /** The name the operator reads it with. */
  readonly name: string;
  /** The address of the style, for a map of vectors. */
  readonly styleUrl: string | null;
  /** The address of the tiles, for a map of pictures. */
  readonly tiles: string | null;
  /** The copyright the source asks to be shown. */
  readonly attribution: string;
}

/** What the corrections of a base station are doing. */
export interface NtripStatus {
  /** Whether the corrections are being sent. */
  readonly running: boolean;
  /** How many bytes of corrections have been sent to the vehicle. */
  readonly bytes: number;
  /** The last failure, when there was one. */
  readonly error: string | null;
  /** The position that was last sent to the caster. */
  readonly position: GeoPoint | null;
}

/** One plugin as the driver sees it, which is what the sheet lists. */
export interface PluginInfo {
  /** The name the manifest says. */
  readonly name: string;
  /** The version the plugin says it is. */
  readonly version: string;
  /** Who wrote it. */
  readonly author: string;
  /** What the manifest asks for, named the way a manifest writes it. */
  readonly capabilities: readonly string[];
  /** How often the plugin is called, once per tick. */
  readonly loopHz: number;
  /** Whether the operator chose to run it. */
  readonly enabled: boolean;
  /** Where the plugin stands. */
  readonly state: string;
  /** The code of the last failure, when there was one. */
  readonly errorCode: string | null;
  /** What the last failure said. */
  readonly errorMessage: string | null;
}

/** What the transponder is doing. */
export interface TransponderStatus {
  /** Whether the transponder is being told what to say. */
  readonly running: boolean;
  /** How many times the vehicle has been told. */
  readonly sent: number;
  /** The last failure, when there was one. */
  readonly error: string | null;
  /** What the transponder says. */
  readonly settings: {
    readonly icaoAddress: number;
    readonly flightId: string;
    readonly squawk: number;
  };
}

/** One key of the signed link, as the operator wrote it. */
export interface SigningKeyEntry {
  /** The name the key is listed under. */
  readonly name: string;
  /** The secret, in hexadecimal. */
  readonly secret: string;
}

/** What the keyring says about what the operator wrote. */
export interface SigningStatus {
  /** The keys, in the order the file wrote them. */
  readonly keys: readonly SigningKeyEntry[];
  /** The names of the secrets that could not be read as thirty-two bytes. */
  readonly refused: readonly string[];
  /** The name of the key the frames are signed with, when there is one. */
  readonly signedWith: string | null;
}

/** One vehicle's place in the formation, as the file writes it. */
export interface FormationSlot {
  /** The vehicle the slot belongs to, named the way the file names it: `system:component`. */
  readonly vehicle: string;
  /** Metres across the direction of travel, positive to the right. */
  readonly x: number;
  /** Metres along the direction of travel, positive ahead. */
  readonly y: number;
  /** Metres above the leader. */
  readonly z: number;
}

/** What the formation is doing. */
export interface FormationStatus {
  /** Whether the operator decided to run the loop. */
  readonly enabled: boolean;
  /** Whether the loop is running now. */
  readonly running: boolean;
  /** How many vehicles the last tick told. */
  readonly told: number;
  /** Who leads, named the way the file names it, when somebody said so. */
  readonly leader: string | null;
  /** The places, in the order the file wrote them. */
  readonly slots: readonly FormationSlot[];
  /** The names that could not be read as vehicles, and were left out. */
  readonly refused: readonly string[];
  /** The last failure of the loop, when there was one. */
  readonly error: string | null;
}

/** What the interface sends for the formation: what the operator wrote, and the decision. */
export interface FormationRequest {
  /** Who leads, named the way the file names it. */
  readonly leader: string | null;
  /** Whether the loop is run. */
  readonly enabled: boolean;
  /** The places, in the order they were written. */
  readonly slots: readonly FormationSlot[];
}

/** A video stream the camera of the vehicle has described. */
export interface VideoStream {
  /** The component that owns the stream. */
  readonly componentId: number;
  /** The id of the stream. */
  readonly streamId: number;
  /** The name of the stream. */
  readonly name: string;
  /** The URI or port of the stream. */
  readonly uri: string;
  /** The horizontal resolution, in pixels. */
  readonly width: number;
  /** The vertical resolution, in pixels. */
  readonly height: number;
  /** How many frames the stream carries in a second. */
  readonly framesPerSecond: number;
  /** How many bits the stream carries in a second. */
  readonly bitrate: number;
  /** Whether the stream is running. */
  readonly running: boolean;
}

/** What a gimbal can do, as its manager says. */
export interface GimbalCapabilities {
  /** Whether the gimbal can be put in its safe position. */
  readonly hasRetract: boolean;
  /** Whether the gimbal can be put in its default position. */
  readonly hasNeutral: boolean;
  /** Whether the gimbal has a roll axis. */
  readonly hasRollAxis: boolean;
  /** Whether the gimbal has a pitch axis. */
  readonly hasPitchAxis: boolean;
  /** Whether the gimbal has a yaw axis. */
  readonly hasYawAxis: boolean;
  /** Whether the yaw can be held against the ground. */
  readonly hasYawLock: boolean;
  /** Whether the yaw can follow the vehicle. */
  readonly hasYawFollow: boolean;
  /** Whether the gimbal can be pointed at an angle. */
  readonly canPointLocal: boolean;
  /** Whether the gimbal can be pointed at a place on the ground. */
  readonly canPointGlobal: boolean;
}

/** How far the axes of a gimbal go, in degrees. */
export interface GimbalLimits {
  /** The lowest roll angle. */
  readonly rollMin: number;
  /** The highest roll angle. */
  readonly rollMax: number;
  /** The lowest pitch angle. */
  readonly pitchMin: number;
  /** The highest pitch angle. */
  readonly pitchMax: number;
  /** The lowest yaw angle. */
  readonly yawMin: number;
  /** The highest yaw angle. */
  readonly yawMax: number;
}

/** The flags a gimbal is running with. */
export interface GimbalFlags {
  /** Whether the gimbal is in its safe position. */
  readonly retracted: boolean;
  /** Whether the gimbal is in its default position. */
  readonly neutral: boolean;
  /** Whether the roll is held against the horizon. */
  readonly rollLock: boolean;
  /** Whether the pitch is held against the horizon. */
  readonly pitchLock: boolean;
  /** Whether the yaw is held against the ground. */
  readonly yawLock: boolean;
  /** Whether the yaw is measured from the way the vehicle is pointing. */
  readonly yawInVehicleFrame: boolean;
  /** Whether the yaw is measured from the ground. */
  readonly yawInEarthFrame: boolean;
}

/** Where a gimbal points, in degrees. */
export interface GimbalAttitude {
  /** The roll angle. */
  readonly roll: number;
  /** The pitch angle. */
  readonly pitch: number;
  /** The yaw angle, always read against the ground. */
  readonly yaw: number;
}

/** One gimbal the vehicle has reported. */
export interface GimbalDevice {
  /** The component that reports this gimbal. */
  readonly componentId: number;
  /** The device id the commands name. */
  readonly deviceId: number;
  /** What the gimbal can do. */
  readonly capabilities: GimbalCapabilities;
  /** How far its axes go. */
  readonly limits: GimbalLimits;
  /** The flags it is running with. */
  readonly flags: GimbalFlags;
  /** Where it points, when the vehicle has said. */
  readonly attitude: GimbalAttitude | null;
  /** Whether the gimbal says something is wrong with it. */
  readonly failed: boolean;
  /** The component that holds the control of the gimbal, `0` when nobody does. */
  readonly controllerComponentId: number;
}

/** What a camera can do, as it says. */
export interface CameraCapabilities {
  /** Whether the camera records video. */
  readonly recordsVideo: boolean;
  /** Whether the camera takes pictures. */
  readonly takesPictures: boolean;
  /** Whether the camera has separate image and video modes. */
  readonly hasModes: boolean;
  /** Whether a picture can be taken while the camera is recording. */
  readonly canTakePictureWhileRecording: boolean;
  /** Whether video can be recorded while the camera is taking pictures. */
  readonly canRecordWhileImaging: boolean;
  /** Whether the camera can be zoomed. */
  readonly hasZoom: boolean;
  /** Whether the camera can follow a point in its picture. */
  readonly canTrackPoint: boolean;
  /** Whether the camera can follow an area of its picture. */
  readonly canTrackRectangle: boolean;
}

/** One camera the vehicle has reported. */
export interface CameraDevice {
  /** The component that reports this camera. */
  readonly componentId: number;
  /** Who made the camera. */
  readonly vendor: string;
  /** Which camera it is. */
  readonly model: string;
  /** The width of the picture, in pixels. */
  readonly width: number;
  /** The height of the picture, in pixels. */
  readonly height: number;
  /** Where the camera says its definition is, when it says one. */
  readonly definitionUri: string;
  /** What the camera can do. */
  readonly capabilities: CameraCapabilities;
  /** The mode the camera is in, as a word the interface shows. */
  readonly mode: string;
  /** Whether a picture can be taken in the mode it is in. */
  readonly takesPictureNow: boolean;
  /** Whether video can be recorded in the mode it is in. */
  readonly recordsVideoNow: boolean;
  /** What the camera says about its pictures, as a word. */
  readonly imageStatus: string;
  /** What the camera says about its video, as a word. */
  readonly videoStatus: string;
}

/** A kind of zoom, as the program that knows the protocol lists it. */
export interface ZoomOption {
  /** The value the command carries. */
  readonly value: number;
  /** The word the interface shows. */
  readonly name: string;
}

/** One aircraft of the traffic, from the vehicle or from a source of the station. */
export interface AdsbVehicle {
  /** The address the aircraft has. */
  readonly icaoAddress: number;
  /** The name it says, when it says one. */
  readonly callsign: string | null;
  /** The code it is squawking, when it says one. */
  readonly squawk: number;
  /** Where it is. */
  readonly latitude: number;
  /** Where it is. */
  readonly longitude: number;
  /** How high it is, in metres. */
  readonly altitude: number;
  /** Which way it is going, in degrees. */
  readonly heading: number;
  /** How fast it is going, in metres per second. */
  readonly speed: number;
  /** How fast it is going up or down, in metres per second. */
  readonly verticalSpeed: number;
  /** Where it was heard from: the vehicle sent it, or the station read it. */
  readonly source: "vehicle" | "station";
}

/** What the relay of the traffic is doing. */
export interface AdsbRelayStatus {
  /** Whether the relay is sending. */
  readonly running: boolean;
  /** How many aircraft have been sent onto the link. */
  readonly sent: number;
  /** Which place of the list the next one will be taken from. */
  readonly index: number;
  /** How many aircraft the station has read and is keeping. */
  readonly aircraft: number;
  /** The last failure, when there was one. */
  readonly error: string | null;
  /** Where the vehicle is, because the list is asked around it. */
  readonly position: GeoPoint | null;
  /** What the relay reads from and how far it looks. */
  readonly settings: {
    readonly server: string;
    readonly radiusNm: number;
  };
}

/** One slot of the on-screen display of the vehicle. */
export interface OsdSlot {
  /** The screen the slot is on: five and six are the ones the operator chooses. */
  readonly screen: number;
  /** The number of the slot on that screen. */
  readonly index: number;
  /** The name of the parameter the slot holds, when the vehicle says it. */
  readonly name: string | null;
  /** Whether the vehicle knows the slot. */
  readonly known: boolean;
}

/** One file the vehicle holds, as the file transfer protocol describes it. */
export interface FtpEntry {
  /** The name of the file, without the folder it is in. */
  readonly name: string;
  /** The whole path of the file. */
  readonly path: string;
  /** How many bytes the file has, or -1 when the vehicle did not say. */
  readonly size: number;
  /** Whether the entry is a folder. */
  readonly isDirectory: boolean;
}

/** A file the vehicle holds: its name, its size and its bytes. */
export interface FtpFile {
  /** The path of the file. */
  readonly path: string;
  /** How many bytes the file has. */
  readonly size: number;
  /** The bytes of the file. */
  readonly content: readonly number[];
}

/** One vehicle of the manifest of the firmware. */
export interface FirmwareBoard {
  readonly name: string;
  readonly versions: readonly FirmwareVersion[];
}

/** One version of the firmware of a vehicle. */
export interface FirmwareVersion {
  readonly version: string;
  readonly files: readonly FirmwareFile[];
}

/** The file of the firmware of one board. */
export interface FirmwareFile {
  readonly platform: string;
  readonly url: string;
  readonly format: string;
  readonly size: number;
  readonly sha256: string;
}

/** The manifest of the firmware. */
export interface FirmwareManifest {
  readonly boards: readonly FirmwareBoard[];
}

/** One point of a path that follows the ground. */
export interface TerrainPathPoint {
  readonly lat: number;
  readonly lng: number;
  readonly groundM: number;
  readonly altM: number;
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

/** One relay of the vehicle, as the vehicle said it. */
export interface RelayInstance {
  /** Which relay this is, counting from the first — the number the command carries. */
  readonly number: number;
  /** Whether the vehicle says it has this relay. */
  readonly present: boolean;
  /** Whether it is on. */
  readonly on: boolean;
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

/**
 * The state of the engine of a vehicle that has one.
 */
export interface Efi {
  /** The revolutions of the engine per minute. */
  readonly rpm: number;
  /** The fuel that has been burnt, in cubic centimetres. */
  readonly fuelConsumed: number;
  /** The fuel that flows now, in cubic centimetres per hour. */
  readonly fuelFlow: number;
  /** The load of the engine, as a percentage. */
  readonly engineLoad: number;
  /** The pressure of the barometric, in kilopascals. */
  readonly barometricPressure: number;
  /** The pressure of the fuel, in kilopascals. */
  readonly fuelPressure: number;
  /** The temperature of the intake, in degrees Celsius. */
  readonly intakeTemperature: number;
  /** The temperature of the head of the cylinder, in degrees Celsius. */
  readonly headTemperature: number;
  /** The temperature of the exhaust, in degrees Celsius. */
  readonly exhaustTemperature: number;
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
  /** What came back from a serial port of the vehicle, when something did. */
  readonly passthrough: PassthroughData | null;
  /** The state of the engine, when the vehicle has told it. */
  readonly efi: Efi | null;
  /** The identification the vehicle broadcasts, when it broadcasts one. */
  readonly droneId: DroneId | null;
  readonly imu: Imu | null;
  readonly barometer: Barometer | null;
  readonly controls: Controls | null;
  /**
   * The relays the vehicle has said it has.
   *
   * Null when the vehicle has not said anything: that is the absence of a report, not a relay at
   * zero, and the panel draws no rows for it.
   */
  readonly relays: readonly RelayInstance[] | null;
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
/**
 * One of the flags of a mask.
 */
export interface ParamFlag {
  /** The mask of the flag: the part of the number that is raised when it is on. */
  readonly mask: number;
  /** What the flag is called. */
  readonly text: string;
}

/**
 * One of the values a parameter can be given, when it is a choice.
 */
export interface ParamChoice {
  /** The value the parameter takes when this choice is made. */
  readonly code: number;
  /** What the choice is called. */
  readonly text: string;
}

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
  readonly values: readonly ParamChoice[];
  /** The flags of the mask, when the parameter is one. */
  readonly bitmask: readonly ParamFlag[];
  /** Whether a change to the parameter needs a restart to take effect. */
  readonly rebootRequired: boolean;
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

/**
 * What the vehicle answered to a command that was sent through the catalogue.
 *
 * Both names are the ones the protocol gives, so the interface shows what the vehicle said rather
 * than a word this project chose for it.
 */
export interface CommandAnswer {
  /** The command that was sent, named the way the specification names it. */
  readonly command: string;
  /** The result the vehicle reported, named the way the protocol names it. */
  readonly result: string;
}

/**
 * What the vehicle answered to one of the asks for a stream.
 *
 * The name is the one the dialect gives the message — data, not a word of the interface — and the
 * answer is what the vehicle said, or that it never said anything.
 */
export interface StreamAnswer {
  /** The message that was asked for, named the way the dialect names it. */
  readonly name: string;
  /** What the vehicle answered to the ask. */
  readonly answer: string;
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

/**
 * What a log of a flight holds.
 */
export interface LogSummary {
  /** The messages the log holds, and how many of each there are. */
  readonly messages: readonly {
    /** The name of the message. */
    readonly name: string;
    /** How many of them the log holds. */
    readonly count: number;
  }[];
  /** The instant the log starts at, in microseconds since the epoch. */
  readonly start: number;
  /** The instant the log ends at, in microseconds since the epoch. */
  readonly end: number;
  /** How many records the log holds. */
  readonly records: number;
  /** How many records could not be read. */
  readonly broken: number;
}

/**
 * One of the places of the track of a log.
 */
export interface LogTrackPoint {
  /** The latitude, in degrees. */
  readonly latitude: number;
  /** The longitude, in degrees. */
  readonly longitude: number;
  /** The height over the sea, in metres. */
  readonly altitude: number;
  /** When the vehicle was there, in microseconds since the epoch. */
  readonly at: number;
}

/**
 * The state of a log that is being played back.
 */
export interface PlaybackState {
  /** Whether a log is open. */
  readonly open: boolean;
  /** Where the play-back is, in seconds. */
  readonly position: number;
  /** How long the flight lasted, in seconds. */
  readonly duration: number;
  /** The telemetry of the flight at that instant. */
  readonly telemetry: Telemetry;
}

/**
 * What a log that is being played back says about itself.
 */
export interface PlaybackInfo {
  /** How long the flight lasted, in seconds. */
  readonly duration: number;
  /** How many samples the flight holds. */
  readonly samples: number;
  /** Whether a log is open. */
  readonly open: boolean;
}

/**
 * One of the logs a vehicle holds in its own memory.
 */
export interface VehicleLog {
  /** The number of the log, which is what the data is asked for with. */
  readonly id: number;
  /** When the log was started, in seconds since the epoch. */
  readonly timeUtc: number;
  /** How many bytes the log holds. */
  readonly size: number;
}

/**
 * The identification a vehicle broadcasts.
 */
export interface DroneId {
  /** What the identification is: a registration, a serial number, a session. */
  readonly kind: string | null;
  /** The identification itself. */
  readonly id: string | null;
  /** What the vehicle is: a multicopter, a fixed wing, a helicopter. */
  readonly kindOfVehicle: string | null;
  /** The place the vehicle says it is. */
  readonly location: DroneIdLocation | null;
  /** What the operator says about the vehicle. */
  readonly description: string | null;
}

/**
 * The place a vehicle says it is.
 */
export interface DroneIdLocation {
  /** The latitude, in degrees. */
  readonly latitude: number;
  /** The longitude, in degrees. */
  readonly longitude: number;
  /** The height over the sea, in metres. */
  readonly altitude: number;
  /** The height over the ground, in metres. */
  readonly height: number;
  /** The speed over the ground, in metres per second. */
  readonly speed: number;
  /** The speed upwards, in metres per second. */
  readonly verticalSpeed: number;
  /** The direction the vehicle is going, in degrees. */
  readonly direction: number;
}

/**
 * The value of one field of one sample.
 */
export type SampleValue = string | number | boolean | null;

/**
 * The samples of one message of a log.
 */
export interface MessageSamples {
  /** The name of the message. */
  readonly name: string;
  /** The fields the message has. */
  readonly fields: readonly string[];
  /** The samples, each of them the values of the fields. */
  readonly rows: readonly (readonly SampleValue[])[];
  /** How many of them the log holds. */
  readonly total: number;
  /**
   * When each of them happened, in seconds after the start of the log.
   */
  readonly instants: readonly number[];
}

/**
 * The place a region of the ground covers.
 */
export interface TileBounds {
  /** The latitude of the northern side, in degrees. */
  readonly north: number;
  /** The latitude of the southern side, in degrees. */
  readonly south: number;
  /** The longitude of the eastern side, in degrees. */
  readonly east: number;
  /** The longitude of the western side, in degrees. */
  readonly west: number;
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
  /**
   * The name the interface lists this vehicle under.
   *
   * The name is decided by the link, which is what knows which board is the autopilot and
   * which is another board on the same link.
   */
  readonly name: string;
  /** Whether this is the vehicle the station speaks to. */
  readonly chosen: boolean;
}

/** The answer of the command that chooses a vehicle. */
export interface SelectedVehicle {
  /** The system id. */
  readonly systemId: number;
  /** The component id. */
  readonly componentId: number;
  /** The name the interface lists this vehicle under. */
  readonly name: string;
  /** Whether this is the vehicle the station speaks to. */
  readonly chosen: boolean;
}

/** The loss statistics of the link. */
export interface LossStats {
  /** The number of packets received. */
  readonly received: number;
  /** The number of packets lost. */
  readonly lost: number;
}

/**
 * What the board printed on the wire outside of MAVLink frames.
 *
 * The capture is bounded, so it says what it dropped as well as what it kept: a window that
 * showed only the kept lines would look like the whole output.
 */
export interface BoardOutput {
  /** The lines that were kept, newest first. */
  readonly lines: readonly string[];
  /** How many lines were dropped because the capture is bounded. */
  readonly dropped: number;
  /** Whether the text looks like a DroneCAN bus, as the link decided. */
  readonly looksLikeCanBus: boolean;
}

/** The result of a command that changed the connection state. */
export interface ConnectResponse {
  readonly status: ConnectionStatus;
  readonly vehicle: VehicleKey | null;
}

/**
 * A picture, with the place it says it was taken and the place the log says the vehicle was
 * when it was made.
 */
export interface PicturePlace {
  /** The name of the file, without the folder it is in. */
  readonly name: string;
  /** The width of the picture, in pixels. */
  readonly width: number;
  /** The height of the picture, in pixels. */
  readonly height: number;
  /** The way the picture is turned, as the camera wrote it. */
  readonly orientation: string;
  /** When the camera says the picture was made, in seconds since the epoch. */
  readonly takenAt: number | null;
  /** The place the file itself carries. */
  readonly place: GeoPoint | null;
  /** The instant of the sample of the log the picture was matched with. */
  readonly matchedAt: number | null;
  /** How far the picture's own instant was from that sample, in seconds. */
  readonly awaySeconds: number | null;
  /** The place the log says the vehicle was at that instant. */
  readonly matchedPlace: GeoPoint | null;
}

/** One of the places a log says the vehicle was, with the instant it was there. */
export interface LogPlace {
  /** The instant, in seconds since the epoch. */
  readonly at: number;
  /** The place. */
  readonly place: GeoPoint;
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
