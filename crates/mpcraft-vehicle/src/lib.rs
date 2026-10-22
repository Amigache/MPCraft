//! Vehicle state model for MPCraft.
//!
//! # Why this differs from upstream
//!
//! Mission Planner's `CurrentState` exposes roughly 400 public properties, and **each one
//! re-derives its value from the last received packet every time it is read**. The UI reads
//! them continuously at 10 Hz.
//!
//! MPCraft decodes each packet **once** into a [`Telemetry`] snapshot and publishes it through
//! an [`ArcSwap`]. Derived values — ground speed, distance to home — are computed at decode
//! time. Reading telemetry is then a single atomic load, not a re-derivation.
//!
//! # Staleness
//!
//! A value is only as fresh as the last packet that carried it. Rather than hide that, the
//! model records the age of each field group so the UI can grey out what has stopped updating.
//! Upstream never does this, and it is a genuine usability improvement: a frozen attitude
//! indicator that looks live is dangerous.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod graph;
pub mod modes;

use std::sync::Arc;
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};

use mpcraft_mavlink::MavGpsFixType;

/// Identifies a vehicle on a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VehicleId {
    /// The system id of the vehicle.
    pub system_id: u8,
    /// The component id of the vehicle.
    pub component_id: u8,
}

impl VehicleId {
    /// Creates a vehicle id.
    #[must_use]
    pub const fn new(system_id: u8, component_id: u8) -> Self {
        Self {
            system_id,
            component_id,
        }
    }
}

impl std::fmt::Display for VehicleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.system_id, self.component_id)
    }
}

/// A geographic position.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    /// Latitude in degrees.
    pub lat: f64,
    /// Longitude in degrees.
    pub lng: f64,
    /// Altitude in metres above mean sea level.
    pub alt: f64,
}

/// Attitude in the aeronautical frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Attitude {
    /// Roll in degrees.
    pub roll: f64,
    /// Pitch in degrees.
    pub pitch: f64,
    /// Yaw in degrees, clockwise from north.
    pub yaw: f64,
}

/// Velocity in the local north-east-down frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Velocity {
    /// Northward velocity in metres per second.
    pub north: f64,
    /// Eastward velocity in metres per second.
    pub east: f64,
    /// Downward velocity in metres per second.
    pub down: f64,
    /// Ground speed in metres per second.
    pub ground_speed: f64,
}

/// Battery state.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Battery {
    /// Voltage in volts.
    pub voltage: f64,
    /// Current in amperes.
    pub current: f64,
    /// Remaining charge as a percentage.
    pub remaining: f64,
}

/// The identification a vehicle broadcasts.
///
/// Some vehicles say who they are over the air: an identification, a place and what the
/// operator wants to say about them. It is what a regulator asks for, and what a
/// receiver on the ground sees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroneId {
    /// What the identification is: a registration, a serial number, a session.
    pub kind: Option<String>,
    /// The identification itself.
    pub id: Option<String>,
    /// What the vehicle is: a multicopter, a fixed wing, a helicopter.
    pub kind_of_vehicle: Option<String>,
    /// The place the vehicle says it is.
    pub location: Option<DroneIdLocation>,
    /// What the operator says about the vehicle.
    pub description: Option<String>,
}

/// The place a vehicle says it is.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DroneIdLocation {
    /// The latitude, in degrees.
    pub latitude: f64,
    /// The longitude, in degrees.
    pub longitude: f64,
    /// The height over the sea, in metres.
    pub altitude: f64,
    /// The height over the ground, in metres.
    pub height: f64,
    /// The speed over the ground, in metres per second.
    pub speed: f64,
    /// The speed upwards, in metres per second.
    pub vertical_speed: f64,
    /// The direction the vehicle is going, in degrees.
    pub direction: f64,
}

/// The state of the engine of a vehicle that has one.
///
/// A vehicle without an engine says nothing about it: the readings are there for the
/// vehicles that have one — a helicopter with a fuel injection, a boat with an engine of
/// its own — and the operator reads them the way upstream reads them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Efi {
    /// The revolutions of the engine per minute.
    pub rpm: f64,
    /// The fuel that has been burnt, in cubic centimetres.
    pub fuel_consumed: f64,
    /// The fuel that flows now, in cubic centimetres per hour.
    pub fuel_flow: f64,
    /// The load of the engine, as a percentage.
    pub engine_load: f64,
    /// The pressure of the barometric, in kilopascals.
    pub barometric_pressure: f64,
    /// The pressure of the fuel, in kilopascals.
    pub fuel_pressure: f64,
    /// The temperature of the intake, in degrees Celsius.
    pub intake_temperature: f64,
    /// The temperature of the head of the cylinder, in degrees Celsius.
    pub head_temperature: f64,
    /// The temperature of the exhaust, in degrees Celsius.
    pub exhaust_temperature: f64,
}

/// The wind where the vehicle is.
///
/// The direction is the one the wind comes **from**, in degrees, and the speed is in
/// metres per second. The vertical speed says whether the air is rising or falling.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Wind {
    /// The direction the wind comes from, in degrees.
    pub direction: f64,
    /// The speed of the wind, in metres per second.
    pub speed: f64,
    /// The vertical speed of the air, in metres per second.
    pub vertical_speed: f64,
}

/// What came back from a serial port of the vehicle.
///
/// The vehicle answers a passthrough with the bytes its port had, and this is them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassthroughData {
    /// The port the bytes came from.
    pub device: u8,
    /// The bytes, as they came.
    pub data: Vec<u8>,
}

/// The state of the global positioning.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GpsStatus {
    /// The type of the fix, as the MAVLink enumeration.
    pub fix_type: MavGpsFixType,
    /// The number of satellites in use.
    pub satellites: u8,
    /// The horizontal dilution of precision.
    ///
    /// A lower number is a better fix. The vehicle reports it in hundredths, so it is
    /// divided here once rather than in every reader.
    pub hdop: f64,
}

/// The flight mode of a vehicle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlightMode {
    /// The numeric custom mode value.
    pub custom_mode: u32,
    /// The human-readable mode name, when known.
    pub name: Option<String>,
}

/// A message the vehicle sent.
///
/// Vehicles report what they are doing with `STATUSTEXT`. The severity is the MAVLink
/// enumeration: 0 is emergency, 1 is alert, 2 is critical, 3 is error, 4 is warning,
/// 5 is notice, 6 is information and 7 is debug.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VehicleMessage {
    /// How serious the message is.
    pub severity: u8,
    /// The text of the message.
    pub text: String,
    /// The id that groups the chunks of a long message.
    pub id: u16,
}

impl VehicleMessage {
    /// The name of the severity, as the MAVLink specification calls it.
    #[must_use]
    pub fn severity_name(&self) -> &'static str {
        match self.severity {
            0 => "EMERGENCY",
            1 => "ALERT",
            2 => "CRITICAL",
            3 => "ERROR",
            4 => "WARNING",
            5 => "NOTICE",
            6 => "INFO",
            7 => "DEBUG",
            _ => "UNKNOWN",
        }
    }
}

/// The state of the plan the vehicle holds.
///
/// The vehicle reports this in `MISSION_CURRENT`, which it streams whenever the plan
/// changes. A vehicle that does not support the reporting says so rather than guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanState {
    /// The vehicle does not report the state of its plan.
    Unknown,
    /// There is no plan on the vehicle.
    NoPlan,
    /// The plan has been uploaded but has not started.
    NotStarted,
    /// The plan is running.
    Active,
    /// The plan is paused.
    Paused,
    /// The plan has run to the end.
    Complete,
}

/// The progress of the plan on the vehicle.
///
/// This is what the interface shows when it highlights the item the vehicle is flying
/// towards, and it is the only way to tell a plan that is running from one that was
/// uploaded and never started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionProgress {
    /// The sequence number of the item the vehicle is executing.
    pub seq: u16,
    /// The number of items in the plan.
    pub total: u16,
    /// The state of the plan.
    pub state: PlanState,
    /// Whether the vehicle is in a mode that can execute the plan.
    ///
    /// A vehicle that is in a mode that cannot fly the plan is *suspended*: the plan is
    /// loaded, but nothing is executing.
    pub in_mission_mode: bool,
    /// The identifier of the mission plan.
    pub mission_id: u32,
    /// The identifier of the fence plan.
    pub fence_id: u32,
    /// The identifier of the rally point plan.
    pub rally_id: u32,
}

/// The inertial measurement of the vehicle.
///
/// The units are the ones an operator reads: metres per second squared for the
/// acceleration, radians per second for the angular rate, and gauss for the magnetic field.
/// The values on the wire are in thousandths, and the conversion happens once, here.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Imu {
    /// The acceleration on each axis, in metres per second squared.
    pub acceleration: [f64; 3],
    /// The angular rate on each axis, in radians per second.
    pub angular_rate: [f64; 3],
    /// The magnetic field on each axis, in gauss.
    pub magnetic_field: [f64; 3],
    /// The temperature of the sensor, in degrees Celsius.
    pub temperature: f64,
}

/// The barometer of the vehicle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Barometer {
    /// The absolute pressure, in hectopascals.
    pub absolute_pressure: f64,
    /// The differential pressure, in hectopascals.
    pub differential_pressure: f64,
    /// The temperature, in degrees Celsius.
    pub temperature: f64,
}

/// The servos and the radio control.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Controls {
    /// The pulse width of each of the first eight outputs, in microseconds.
    pub outputs: [Option<f64>; 8],
    /// The value of each of the first eight radio channels.
    pub inputs: [Option<f64>; 8],
    /// The strength of the radio signal, as a percentage.
    pub signal: Option<f64>,
}

/// The air data of the vehicle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Airspeed {
    /// The speed through the air, in metres per second.
    pub airspeed: Option<f64>,
    /// The speed over the ground, in metres per second.
    pub ground_speed: f64,
    /// The rate of climb, in metres per second.
    pub climb: f64,
    /// The throttle, as a percentage.
    pub throttle: f64,
}

/// The state of the estimator of the vehicle.
///
/// The variances are the squares of the standard deviations, in the units of what they
/// measure: metres per second for the velocity, metres for the position, gauss for the
/// compass.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ekf {
    /// The variance of the velocity estimate.
    pub velocity_variance: f64,
    /// The variance of the horizontal position estimate.
    pub horizontal_variance: f64,
    /// The variance of the vertical position estimate.
    pub vertical_variance: f64,
    /// The variance of the compass.
    pub compass_variance: f64,
    /// The variance of the terrain altitude.
    pub terrain_variance: f64,
    /// The variance of the airspeed estimate.
    pub airspeed_variance: f64,
}

/// The vibration of the airframe.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vibration {
    /// The vibration on the first axis, in metres per second squared.
    pub x: f64,
    /// The vibration on the second axis, in metres per second squared.
    pub y: f64,
    /// The vibration on the third axis, in metres per second squared.
    pub z: f64,
}

/// The state of one battery bank.
///
/// A vehicle may carry several banks: the pack of the propulsion and the pack of the
/// payload, for instance. Each of them has its own cells, its own current and its own
/// remaining charge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatteryBank {
    /// The identifier the vehicle gives the bank.
    pub id: u8,
    /// The voltage of each cell, in volts.
    ///
    /// The cells the vehicle does not have are not in the list: a bank of six cells has
    /// six entries, and not sixteen with ten of them empty.
    pub cells: Vec<f64>,
    /// The current of the bank, in amperes.
    pub current: f64,
    /// The remaining charge of the bank, as a percentage.
    pub remaining: f64,
    /// The temperature of the bank, in degrees Celsius.
    pub temperature: f64,
}

/// What the flight controller is asking the vehicle to do.
///
/// The desired attitude is what the controller wants, and the difference between it and
/// what the vehicle does is how well the controller is tuned.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Controller {
    /// The roll the controller asks for, in radians.
    pub roll: f64,
    /// The pitch the controller asks for, in radians.
    pub pitch: f64,
    /// The error of the altitude, in metres.
    pub altitude_error: f64,
    /// The error of the airspeed.
    pub airspeed_error: f64,
    /// The error of the cross-track, in metres.
    pub cross_track_error: f64,
    /// The distance to the waypoint the vehicle is flying towards, in metres.
    pub waypoint_distance: f64,
}

/// The state of a vehicle at a point in time.
///
/// Every optional field is `None` until the corresponding message has been received at least
/// once.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Telemetry {
    /// Attitude, once received.
    pub attitude: Option<Attitude>,
    /// Position, once received.
    pub position: Option<GeoPoint>,
    /// Velocity, once received.
    pub velocity: Option<Velocity>,
    /// Battery state, once received.
    pub battery: Option<Battery>,
    /// Flight mode, once received.
    pub mode: Option<FlightMode>,
    /// Whether the vehicle is armed.
    pub armed: Option<bool>,
    /// The home position, once received.
    pub home: Option<GeoPoint>,
    /// Distance to home in metres, computed when both positions are known.
    pub distance_to_home: Option<f64>,
    /// The bearing to home in degrees, computed when both positions are known.
    pub bearing_to_home: Option<f64>,
    /// The progress of the plan on the vehicle, once reported.
    pub mission: Option<MissionProgress>,
    /// The state of the global positioning, once received.
    pub gps: Option<GpsStatus>,

    /// The state of the second global positioning, once received.
    ///
    /// A vehicle with two receivers reports them separately, and the operator needs
    /// to see both: one of them may be looking at a constellation the other is not.
    pub gps2: Option<GpsStatus>,

    /// The wind, once it is known.
    pub wind: Option<Wind>,
    /// What came back from a serial port of the vehicle, once received.
    pub passthrough: Option<PassthroughData>,
    /// The state of the engine, when the vehicle has told it.
    pub efi: Option<Efi>,
    /// The identification the vehicle broadcasts, when it broadcasts one.
    pub drone_id: Option<DroneId>,
    /// The inertial measurement, once received.
    pub imu: Option<Imu>,
    /// The barometer, once received.
    pub barometer: Option<Barometer>,
    /// The servos and the radio control, once received.
    pub controls: Option<Controls>,
    /// The air data, once received.
    pub airspeed: Option<Airspeed>,
    /// The state of the estimator, once received.
    pub ekf: Option<Ekf>,
    /// The vibration of the airframe, once received.
    pub vibration: Option<Vibration>,
    /// The battery banks the vehicle has reported.
    pub batteries: Vec<BatteryBank>,
    /// The attitude of the second estimator, once received.
    pub ahrs2: Option<Attitude>,
    /// What the flight controller is asking for, once received.
    pub controller: Option<Controller>,
}

impl Telemetry {
    /// Merges an update into the snapshot, recomputing derived values.
    ///
    /// Derived values are recomputed here rather than on access, so a reader of the snapshot
    /// never pays for the derivation.
    pub fn apply(&mut self, update: TelemetryUpdate) {
        if let Some(attitude) = update.attitude {
            self.attitude = Some(attitude);
        }
        if let Some(position) = update.position {
            self.position = Some(position);
        }
        if let Some(velocity) = update.velocity {
            self.velocity = Some(velocity);
        }
        if let Some(battery) = update.battery {
            self.battery = Some(battery);
        }
        if let Some(mode) = update.mode {
            self.mode = Some(mode);
        }
        if let Some(armed) = update.armed {
            self.armed = Some(armed);
        }
        if let Some(home) = update.home {
            self.home = Some(home);
        }
        if let Some(mission) = update.mission {
            self.mission = Some(mission);
        }
        if let Some(gps) = update.gps {
            self.gps = Some(gps);
        }

        if let Some(gps) = update.gps2 {
            self.gps2 = Some(gps);
        }

        if let Some(wind) = update.wind {
            self.wind = Some(wind);
        }

        if let Some(efi) = update.efi {
            self.efi = Some(efi);
        }

        if let Some(drone_id) = update.drone_id {
            self.drone_id = Some(drone_id);
        }
        if let Some(imu) = update.imu {
            self.imu = Some(imu);
        }
        if let Some(barometer) = update.barometer {
            self.barometer = Some(barometer);
        }
        if let Some(controls) = update.controls {
            self.controls = Some(controls);
        }
        if let Some(airspeed) = update.airspeed {
            self.airspeed = Some(airspeed);
        }
        if let Some(ekf) = update.ekf {
            self.ekf = Some(ekf);
        }
        if let Some(vibration) = update.vibration {
            self.vibration = Some(vibration);
        }
        if !update.batteries.is_empty() {
            self.batteries = update.batteries;
        }
        if let Some(ahrs2) = update.ahrs2 {
            self.ahrs2 = Some(ahrs2);
        }
        if let Some(controller) = update.controller {
            self.controller = Some(controller);
        }

        self.recompute_derived();
    }

    /// Recomputes the values that depend on more than one field.
    fn recompute_derived(&mut self) {
        let (Some(position), Some(home)) = (self.position, self.home) else {
            self.distance_to_home = None;
            self.bearing_to_home = None;
            return;
        };

        let (distance, bearing) = great_circle(position, home);
        self.distance_to_home = Some(distance);
        self.bearing_to_home = Some(bearing);
    }
}

/// A partial update to the telemetry snapshot.
#[derive(Debug, Clone, Default)]
pub struct TelemetryUpdate {
    /// New attitude.
    pub attitude: Option<Attitude>,
    /// New position.
    pub position: Option<GeoPoint>,
    /// New velocity.
    pub velocity: Option<Velocity>,
    /// New battery state.
    pub battery: Option<Battery>,
    /// New flight mode.
    pub mode: Option<FlightMode>,
    /// New armed state.
    pub armed: Option<bool>,
    /// New home position.
    pub home: Option<GeoPoint>,
    /// New progress of the plan on the vehicle.
    pub mission: Option<MissionProgress>,
    /// New state of the global positioning.
    pub gps: Option<GpsStatus>,

    /// New state of the second global positioning.
    pub gps2: Option<GpsStatus>,

    /// New wind.
    pub wind: Option<Wind>,
    /// New state of a serial port of the vehicle.
    pub passthrough: Option<PassthroughData>,
    /// New state of the engine.
    pub efi: Option<Efi>,
    /// New identification of the vehicle.
    pub drone_id: Option<DroneId>,
    /// New inertial measurement.
    pub imu: Option<Imu>,
    /// New barometer reading.
    pub barometer: Option<Barometer>,
    /// New state of the servos and the radio control.
    pub controls: Option<Controls>,
    /// New air data.
    pub airspeed: Option<Airspeed>,
    /// New state of the estimator.
    pub ekf: Option<Ekf>,
    /// New vibration of the airframe.
    pub vibration: Option<Vibration>,
    /// New battery banks.
    pub batteries: Vec<BatteryBank>,
    /// New attitude of the second estimator.
    pub ahrs2: Option<Attitude>,
    /// New wish of the flight controller.
    pub controller: Option<Controller>,
}

/// Computes the distance and initial bearing between two points using the haversine formula.
///
/// Returns the distance in metres and the bearing in degrees clockwise from north.
///
/// ```
/// use mpcraft_vehicle::{great_circle, GeoPoint};
///
/// let canberra = GeoPoint { lat: -35.2809, lng: 149.1300, alt: 0.0 };
/// let sydney = GeoPoint { lat: -33.8688, lng: 151.2093, alt: 0.0 };
///
/// let (distance, bearing) = great_circle(canberra, sydney);
/// assert!((distance - 246_849.0).abs() < 500.0, "distance was {distance}");
/// assert!((bearing - 51.2).abs() < 1.0, "bearing was {bearing}");
/// ```
#[must_use]
pub fn great_circle(from: GeoPoint, to: GeoPoint) -> (f64, f64) {
    mpcraft_geo::great_circle(
        mpcraft_geo::GeoPoint::new(from.lat, from.lng),
        mpcraft_geo::GeoPoint::new(to.lat, to.lng),
    )
}

/// A shared, atomically-updated telemetry snapshot.
///
/// Readers take an atomic load; writers replace the whole snapshot. This is what lets the UI
/// read telemetry at 60 fps while packets arrive at 20 Hz without a lock.
#[derive(Debug)]
pub struct TelemetrySlot {
    inner: ArcSwap<Telemetry>,
}

impl Default for TelemetrySlot {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetrySlot {
    /// Creates an empty slot.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: ArcSwap::from_pointee(Telemetry::default()),
        }
    }

    /// Reads the current snapshot.
    #[must_use]
    pub fn load(&self) -> arc_swap::Guard<Arc<Telemetry>> {
        self.inner.load()
    }

    /// Applies an update and publishes the new snapshot.
    ///
    /// Uses a read-copy-update so a concurrent reader never observes a half-applied state.
    pub fn apply(&self, update: TelemetryUpdate) {
        self.inner.rcu(|current| {
            let mut next = (**current).clone();
            next.apply(update.clone());
            Arc::new(next)
        });
    }
}

/// Tracks how long ago each field group was last updated.
///
/// The UI uses this to distinguish a live value from a frozen one.
#[derive(Debug, Default)]
pub struct FieldFreshness {
    attitude: Option<Instant>,
    position: Option<Instant>,
    velocity: Option<Instant>,
    battery: Option<Instant>,
}

impl FieldFreshness {
    /// Creates an empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that the attitude was updated.
    pub fn mark_attitude(&mut self, now: Instant) {
        self.attitude = Some(now);
    }

    /// Records that the position was updated.
    pub fn mark_position(&mut self, now: Instant) {
        self.position = Some(now);
    }

    /// Records that the velocity was updated.
    pub fn mark_velocity(&mut self, now: Instant) {
        self.velocity = Some(now);
    }

    /// Records that the battery was updated.
    pub fn mark_battery(&mut self, now: Instant) {
        self.battery = Some(now);
    }

    /// The age of the attitude update, if any.
    #[must_use]
    pub fn attitude_age(&self, now: Instant) -> Option<Duration> {
        self.attitude.map(|t| now.saturating_duration_since(t))
    }

    /// The age of the position update, if any.
    #[must_use]
    pub fn position_age(&self, now: Instant) -> Option<Duration> {
        self.position.map(|t| now.saturating_duration_since(t))
    }

    /// The age of the velocity update, if any.
    #[must_use]
    pub fn velocity_age(&self, now: Instant) -> Option<Duration> {
        self.velocity.map(|t| now.saturating_duration_since(t))
    }

    /// The age of the battery update, if any.
    #[must_use]
    pub fn battery_age(&self, now: Instant) -> Option<Duration> {
        self.battery.map(|t| now.saturating_duration_since(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_between_identical_points_is_zero() {
        let p = GeoPoint {
            lat: 10.0,
            lng: 20.0,
            alt: 0.0,
        };
        let (distance, _) = great_circle(p, p);
        assert!(distance < 0.01);
    }

    #[test]
    fn derived_values_appear_only_once_both_points_are_known() {
        let mut telemetry = Telemetry::default();
        assert_eq!(telemetry.distance_to_home, None);

        telemetry.apply(TelemetryUpdate {
            position: Some(GeoPoint {
                lat: -35.0,
                lng: 149.0,
                alt: 500.0,
            }),
            ..Default::default()
        });
        assert_eq!(telemetry.distance_to_home, None, "home is still unknown");

        telemetry.apply(TelemetryUpdate {
            home: Some(GeoPoint {
                lat: -35.001,
                lng: 149.001,
                alt: 600.0,
            }),
            ..Default::default()
        });
        assert!(telemetry.distance_to_home.is_some());
        assert!(telemetry.bearing_to_home.is_some());
    }

    #[test]
    fn the_progress_of_the_plan_reaches_the_snapshot() {
        let mut telemetry = Telemetry::default();
        assert!(telemetry.mission.is_none());

        telemetry.apply(TelemetryUpdate {
            mission: Some(MissionProgress {
                seq: 2,
                total: 7,
                state: PlanState::Active,
                in_mission_mode: true,
                mission_id: 11,
                fence_id: 22,
                rally_id: 33,
            }),
            ..Default::default()
        });

        let progress = telemetry.mission.expect("the progress of the plan");
        assert_eq!(progress.seq, 2);
        assert_eq!(progress.total, 7);
        assert_eq!(progress.state, PlanState::Active);
        assert!(progress.in_mission_mode);
        assert_eq!(progress.mission_id, 11);
    }

    #[test]
    fn snapshot_updates_are_visible_to_later_readers() {
        let slot = TelemetrySlot::new();
        slot.apply(TelemetryUpdate {
            armed: Some(true),
            ..Default::default()
        });
        assert_eq!(slot.load().armed, Some(true));
    }

    #[test]
    fn freshness_tracks_the_age_of_updates() {
        let mut freshness = FieldFreshness::new();
        let now = Instant::now();
        freshness.mark_position(now);

        let age = freshness
            .position_age(now + Duration::from_secs(3))
            .unwrap();
        assert_eq!(age, Duration::from_secs(3));
        assert_eq!(freshness.attitude_age(now), None);
    }

    #[test]
    fn vehicle_id_displays_system_and_component() {
        assert_eq!(VehicleId::new(1, 1).to_string(), "1/1");
    }
}
