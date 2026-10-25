//! Telemetry aggregation.
//!
//! Decodes MAVLink messages into the [`Telemetry`] snapshot the application reads.
//!
//! # Why the values are decoded once
//!
//! Mission Planner's `CurrentState` exposes roughly four hundred properties, and *each one*
//! re-derives its value from the last received packet every time it is read. The user
//! interface reads them continuously.
//!
//! MPCraft decodes each packet once, here, and publishes the result. Reading telemetry is
//! then an atomic load rather than a re-derivation, and the derived values — ground speed,
//! distance to home — are computed at decode time rather than on every access.
//!
//! The state is kept **per vehicle**: a link that carries two boards must not read one
//! board's position as the other's. See [`TelemetryAggregator`] for how the view and the
//! per-vehicle slots relate.

use std::collections::HashMap;

use crate::vehicle::VehicleId;

use mpcraft_vehicle::{
    relays_of, Airspeed, Attitude, Barometer, Battery, BatteryBank, Controller, Controls, DroneId,
    DroneIdLocation, Efi, Ekf, FlightMode, GeoPoint, GpsStatus, Imu, MissionProgress,
    PassthroughData, PlanState, Telemetry, Velocity, Vibration, Wind,
};

use mpcraft_mavlink::dialect::messages::MissionState;
use mpcraft_mavlink::dialect::messages::{MavOdidIdType, MavOdidUaType};
use mpcraft_mavlink::dialect::CharArray;
use mpcraft_mavlink::{MavMessage, MavModeFlag};

/// The identification of a vehicle, as the name of its kind.
///
/// The enumeration is the vehicle's, and the name is what the interface shows: the
/// numbers of the protocol do not belong in a readout.
fn id_kind(id_type: MavOdidIdType) -> String {
    match id_type {
        MavOdidIdType::MAV_ODID_ID_TYPE_SERIAL_NUMBER => "A serial number",
        MavOdidIdType::MAV_ODID_ID_TYPE_CAA_REGISTRATION_ID => "A registration",
        MavOdidIdType::MAV_ODID_ID_TYPE_UTM_ASSIGNED_UUID => "An assigned identifier",
        MavOdidIdType::MAV_ODID_ID_TYPE_SPECIFIC_SESSION_ID => "A session identifier",
        MavOdidIdType::MAV_ODID_ID_TYPE_NONE => "None",
    }
    .to_owned()
}

/// What the vehicle is, as the name of its kind.
fn ua_kind(ua_type: MavOdidUaType) -> String {
    match ua_type {
        MavOdidUaType::MAV_ODID_UA_TYPE_AEROPLANE => "An aeroplane",
        MavOdidUaType::MAV_ODID_UA_TYPE_HELICOPTER_OR_MULTIROTOR => "A helicopter or a multirotor",
        MavOdidUaType::MAV_ODID_UA_TYPE_GYROPLANE => "A gyroplane",
        MavOdidUaType::MAV_ODID_UA_TYPE_HYBRID_LIFT => "A hybrid lift",
        MavOdidUaType::MAV_ODID_UA_TYPE_ORNITHOPTER => "An ornithopter",
        MavOdidUaType::MAV_ODID_UA_TYPE_GLIDER => "A glider",
        MavOdidUaType::MAV_ODID_UA_TYPE_KITE => "A kite",
        MavOdidUaType::MAV_ODID_UA_TYPE_FREE_BALLOON => "A free balloon",
        MavOdidUaType::MAV_ODID_UA_TYPE_CAPTIVE_BALLOON => "A captive balloon",
        MavOdidUaType::MAV_ODID_UA_TYPE_AIRSHIP => "An airship",
        MavOdidUaType::MAV_ODID_UA_TYPE_FREE_FALL_PARACHUTE => "A parachute",
        MavOdidUaType::MAV_ODID_UA_TYPE_ROCKET => "A rocket",
        MavOdidUaType::MAV_ODID_UA_TYPE_TETHERED_POWERED_AIRCRAFT => "A tethered powered aircraft",
        MavOdidUaType::MAV_ODID_UA_TYPE_GROUND_OBSTACLE => "A ground obstacle",
        MavOdidUaType::MAV_ODID_UA_TYPE_OTHER => "Something else",
        MavOdidUaType::MAV_ODID_UA_TYPE_NONE => "None",
    }
    .to_owned()
}

/// The bytes of an identification, as the hexadecimal they are written in.
fn hex_of(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<String>>()
        .join(":")
}

/// The text of a message, as the characters it carries.
fn text_of<const N: usize>(text: &CharArray<N>) -> String {
    text.to_str()
        .unwrap_or_default()
        .trim_end_matches('\0')
        .trim()
        .to_owned()
}

/// Decodes messages into the telemetry snapshots.
///
/// # One state per vehicle
///
/// A link can carry several vehicles, and each one reports its own attitude, position and
/// battery. Decoding them into a single snapshot would mix them: a second board's position
/// would appear as the vehicle's. The aggregator keeps one state per sender, keyed by the
/// pair `(system_id, component_id)`.
///
/// Before a vehicle is selected there is nothing to tell the reports apart by, so the packets
/// are read into a single mixed view — which is what a link with one vehicle shows. Once the
/// selection names a primary vehicle, that vehicle's slot *is* the view and the others stay
/// in their own.
#[derive(Debug, Default)]
pub struct TelemetryAggregator {
    /// The snapshot the session reads: the primary vehicle's state, or the mixed view while
    /// no vehicle has been selected.
    view: Telemetry,
    /// The state of each vehicle seen on the link.
    per_vehicle: HashMap<VehicleId, Telemetry>,
    /// The vehicle the view belongs to, once one has been selected.
    selected: Option<VehicleId>,
}

impl TelemetryAggregator {
    /// Creates an empty aggregator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The current snapshot.
    #[must_use]
    pub fn telemetry(&self) -> &Telemetry {
        &self.view
    }

    /// The state of one vehicle, as the aggregator holds it.
    #[must_use]
    pub fn telemetry_of(&self, vehicle: VehicleId) -> Option<&Telemetry> {
        self.per_vehicle.get(&vehicle)
    }

    /// The state of every vehicle seen, keyed by the pair that identifies it.
    #[must_use]
    pub fn telemetries(&self) -> HashMap<VehicleId, Telemetry> {
        self.per_vehicle.clone()
    }

    /// The vehicle the view belongs to, once one has been selected.
    #[must_use]
    pub fn selected(&self) -> Option<VehicleId> {
        self.selected
    }

    /// Names the vehicle the view belongs to.
    ///
    /// When the selection happens after packets have already arrived, the state read before
    /// the selection is not thrown away: the mixed view becomes the primary's starting point,
    /// which is what a single-vehicle link would have shown.
    pub fn set_primary(&mut self, vehicle: VehicleId) {
        if self.selected == Some(vehicle) {
            return;
        }

        self.selected = Some(vehicle);

        let slot = self.per_vehicle.entry(vehicle).or_default();

        if slot == &self.view {
            return;
        }

        if slot.is_empty() {
            slot.clone_from(&self.view);
        } else {
            self.view.clone_from(slot);
        }
    }

    /// Applies a decoded message to the state of the vehicle that sent it.
    ///
    /// Before a vehicle is selected the message also reads into the mixed view, which is what
    /// a single-vehicle link shows. After the selection only the primary vehicle's packets
    /// reach the view.
    ///
    /// Messages that carry no telemetry are ignored.
    pub fn apply(&mut self, message: &MavMessage, sender: VehicleId) {
        let slot = self.per_vehicle.entry(sender).or_default();

        apply_to(slot, message);

        if self.selected.is_none_or(|primary| primary == sender) {
            apply_to(&mut self.view, message);
        }
    }
}

/// Applies one decoded message to one vehicle's state.
fn apply_to(telemetry: &mut Telemetry, message: &MavMessage) {
    match message {
        MavMessage::ATTITUDE(attitude) => {
            // The message carries radians; the model speaks degrees, which is what the graphs,
            // the instrument and the formation read. The conversion happens here, once, at the
            // wire boundary, so that no reader downstream has to know which the message used.
            telemetry.attitude = Some(Attitude {
                roll: f64::from(attitude.roll).to_degrees(),
                pitch: f64::from(attitude.pitch).to_degrees(),
                yaw: f64::from(attitude.yaw).to_degrees(),
            });
        }

        MavMessage::GLOBAL_POSITION_INT(position) => {
            // The integer encoding scales latitude and longitude by 1e7, and the
            // altitudes by 1e3 to give millimetres.
            telemetry.position = Some(GeoPoint {
                lat: f64::from(position.lat) / 1e7,
                lng: f64::from(position.lon) / 1e7,
                alt: f64::from(position.alt) / 1e3,
            });

            // The velocity is in centimetres per second, in the north-east-down frame.
            let north = f64::from(position.vx) / 100.0;
            let east = f64::from(position.vy) / 100.0;
            let down = f64::from(position.vz) / 100.0;

            telemetry.velocity = Some(Velocity {
                north,
                east,
                down,
                ground_speed: (north * north + east * east).sqrt(),
            });
        }

        MavMessage::VFR_HUD(hud) => {
            // The HUD carries a friendlier velocity than the position message, but it
            // has no vertical rate in the same frame, so the position message wins for
            // the vector and the HUD fills in the airspeed.
            let velocity = telemetry.velocity.get_or_insert(Velocity {
                north: 0.0,
                east: 0.0,
                down: 0.0,
                ground_speed: 0.0,
            });
            if velocity.ground_speed == 0.0 {
                velocity.ground_speed = f64::from(hud.groundspeed);
            }

            // The HUD is also what tells the vehicle's speed through the air, its rate
            // of climb and its throttle. A vehicle with no airspeed sensor reports
            // zero, which is not the same as not having said: the vehicle that has no
            // sensor does not send the message at all.
            let airspeed = telemetry.airspeed.get_or_insert(Airspeed {
                airspeed: None,
                ground_speed: 0.0,
                climb: 0.0,
                throttle: 0.0,
            });

            airspeed.ground_speed = f64::from(hud.groundspeed);
            airspeed.climb = f64::from(hud.climb);
            airspeed.throttle = f64::from(hud.throttle);

            if hud.airspeed > 0.0 {
                airspeed.airspeed = Some(f64::from(hud.airspeed));
            }
        }

        MavMessage::SYS_STATUS(status) => {
            telemetry.battery = Some(Battery {
                voltage: f64::from(status.voltage_battery) / 1000.0,
                current: f64::from(status.current_battery) / 100.0,
                remaining: f64::from(status.battery_remaining),
            });
        }

        MavMessage::HOME_POSITION(home) => {
            telemetry.home = Some(GeoPoint {
                lat: f64::from(home.latitude) / 1e7,
                lng: f64::from(home.longitude) / 1e7,
                alt: f64::from(home.altitude) / 1e3,
            });
        }

        MavMessage::HEARTBEAT(heartbeat) => {
            telemetry.armed = Some(
                heartbeat
                    .base_mode
                    .contains(MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED),
            );

            // The mode name comes from the vehicle's FLTMODE1 parameter, which is read
            // with the parameter metadata. Until then the numeric custom mode is kept.
            telemetry.mode = Some(FlightMode {
                custom_mode: heartbeat.custom_mode,
                name: None,
            });
        }

        MavMessage::GPS_RAW_INT(raw) => {
            telemetry.gps = Some(GpsStatus {
                fix_type: raw.fix_type,
                satellites: raw.satellites_visible,
                // The vehicle reports the dilution in hundredths.
                hdop: f64::from(raw.eph) / 100.0,
            });
        }

        MavMessage::RAW_IMU(imu) => {
            // The accelerometers are in thousandths of g, the gyroscopes in thousandths
            // of a radian per second, and the magnetometers in thousandths of a gauss.
            // The conversions happen once, here, so that everything downstream reads
            // the units an operator expects.
            telemetry.imu = Some(Imu {
                acceleration: [
                    f64::from(imu.xacc) * 9.81 * 0.001,
                    f64::from(imu.yacc) * 9.81 * 0.001,
                    f64::from(imu.zacc) * 9.81 * 0.001,
                ],
                angular_rate: [
                    f64::from(imu.xgyro) * 0.001,
                    f64::from(imu.ygyro) * 0.001,
                    f64::from(imu.zgyro) * 0.001,
                ],
                magnetic_field: [
                    f64::from(imu.xmag) * 0.001,
                    f64::from(imu.ymag) * 0.001,
                    f64::from(imu.zmag) * 0.001,
                ],
                temperature: f64::from(imu.temperature) * 0.01,
            });
        }

        MavMessage::SCALED_IMU(imu) => {
            // The scaled variant carries the same sensors, already scaled by the
            // vehicle itself. It only reaches the model when the raw one has not.
            if telemetry.imu.is_none() {
                telemetry.imu = Some(Imu {
                    acceleration: [
                        f64::from(imu.xacc) * 9.81 * 0.001,
                        f64::from(imu.yacc) * 9.81 * 0.001,
                        f64::from(imu.zacc) * 9.81 * 0.001,
                    ],
                    angular_rate: [
                        f64::from(imu.xgyro) * 0.001,
                        f64::from(imu.ygyro) * 0.001,
                        f64::from(imu.zgyro) * 0.001,
                    ],
                    magnetic_field: [
                        f64::from(imu.xmag) * 0.001,
                        f64::from(imu.ymag) * 0.001,
                        f64::from(imu.zmag) * 0.001,
                    ],
                    temperature: f64::from(imu.temperature) * 0.01,
                });
            }
        }

        MavMessage::SCALED_PRESSURE(pressure) => {
            // The pressures are in hectopascals and the temperature in hundredths of a
            // degree.
            telemetry.barometer = Some(Barometer {
                absolute_pressure: f64::from(pressure.press_abs),
                differential_pressure: f64::from(pressure.press_diff),
                temperature: f64::from(pressure.temperature) * 0.01,
            });
        }

        MavMessage::SERVO_OUTPUT_RAW(servos) => {
            // The pulse width of each output, in microseconds. The vehicle may report
            // fewer than eight outputs, and the ones it does not report stay unknown
            // rather than becoming zero: a servo at zero microseconds is a fault, not
            // the absence of a report.
            let mut controls = telemetry.controls.unwrap_or(Controls {
                outputs: [None; 8],
                inputs: [None; 8],
                signal: None,
            });

            let outputs = [
                servos.servo1_raw,
                servos.servo2_raw,
                servos.servo3_raw,
                servos.servo4_raw,
                servos.servo5_raw,
                servos.servo6_raw,
                servos.servo7_raw,
                servos.servo8_raw,
            ];

            for (slot, value) in controls.outputs.iter_mut().zip(outputs) {
                *slot = Some(f64::from(value));
            }

            telemetry.controls = Some(controls);
        }

        MavMessage::RC_CHANNELS(channels) => {
            // The value of each radio channel, in microseconds. The vehicle says how
            // many channels it has, and the ones beyond that count are not reported.
            let mut controls = telemetry.controls.unwrap_or(Controls {
                outputs: [None; 8],
                inputs: [None; 8],
                signal: None,
            });

            let values = [
                channels.chan1_raw,
                channels.chan2_raw,
                channels.chan3_raw,
                channels.chan4_raw,
                channels.chan5_raw,
                channels.chan6_raw,
                channels.chan7_raw,
                channels.chan8_raw,
            ];

            for (index, value) in values.iter().enumerate() {
                controls.inputs[index] = if (index as u8) < channels.chancount {
                    Some(f64::from(*value))
                } else {
                    None
                };
            }

            controls.signal = Some(f64::from(channels.rssi));

            telemetry.controls = Some(controls);
        }

        MavMessage::RELAY_STATUS(relay) => {
            // The message carries two masks: which relays the vehicle has, and which of them are
            // on. They are read into the list the interface draws here, once, so that the frontend
            // does not have to know that a relay is a bit in a mask.
            telemetry.relays = Some(relays_of(relay.present, relay.on));
        }

        MavMessage::BATTERY_STATUS(status) => {
            // The vehicle reports each bank on its own, and says which bank it is.
            // The voltages of the cells are in millivolts, and the ones the bank does
            // not have are not reported: a bank of six cells has six entries.
            let cells: Vec<f64> = status
                .voltages
                .iter()
                .take_while(|voltage| **voltage > 0)
                .map(|voltage| f64::from(*voltage) / 1000.0)
                .collect();

            let bank = BatteryBank {
                id: status.id,
                cells,
                current: f64::from(status.current_battery) / 100.0,
                remaining: f64::from(status.battery_remaining.max(0)),
                temperature: f64::from(status.temperature) * 0.01,
            };

            // The banks are kept in the order the vehicle names them, and a bank that
            // is reported again replaces the one that was there.
            let mut batteries = std::mem::take(&mut telemetry.batteries);
            batteries.retain(|known| known.id != bank.id);
            batteries.push(bank);
            batteries.sort_by_key(|known| known.id);

            telemetry.batteries = batteries;
        }

        MavMessage::VIBRATION(vibration) => {
            telemetry.vibration = Some(Vibration {
                x: f64::from(vibration.vibration_x),
                y: f64::from(vibration.vibration_y),
                z: f64::from(vibration.vibration_z),
            });
        }

        MavMessage::EKF_STATUS_REPORT(report) => {
            telemetry.ekf = Some(Ekf {
                velocity_variance: f64::from(report.velocity_variance),
                horizontal_variance: f64::from(report.pos_horiz_variance),
                vertical_variance: f64::from(report.pos_vert_variance),
                compass_variance: f64::from(report.compass_variance),
                terrain_variance: f64::from(report.terrain_alt_variance),
                airspeed_variance: f64::from(report.airspeed_variance),
            });
        }

        MavMessage::AHRS2(ahrs) => {
            // The second estimator, which is the one the vehicle compares the first
            // with. Its angles are radians on the wire and degrees in the model, like the
            // primary attitude's.
            telemetry.ahrs2 = Some(Attitude {
                roll: f64::from(ahrs.roll).to_degrees(),
                pitch: f64::from(ahrs.pitch).to_degrees(),
                yaw: f64::from(ahrs.yaw).to_degrees(),
            });
        }

        MavMessage::NAV_CONTROLLER_OUTPUT(output) => {
            // What the controller wants, against what the vehicle does.
            telemetry.controller = Some(Controller {
                roll: f64::from(output.nav_roll),
                pitch: f64::from(output.nav_pitch),
                altitude_error: f64::from(output.alt_error),
                airspeed_error: f64::from(output.aspd_error),
                cross_track_error: f64::from(output.xtrack_error),
                waypoint_distance: f64::from(output.wp_dist),
            });
        }

        MavMessage::OPEN_DRONE_ID_BASIC_ID(basic) => {
            // The identification the vehicle broadcasts. The parts arrive in
            // different messages, and each one keeps what the others said.
            let mut drone_id = telemetry.drone_id.clone().unwrap_or(DroneId {
                kind: None,
                id: None,
                kind_of_vehicle: None,
                location: None,
                description: None,
            });

            drone_id.kind = Some(id_kind(basic.id_type));
            drone_id.id = Some(hex_of(&basic.id_or_mac));
            drone_id.kind_of_vehicle = Some(ua_kind(basic.ua_type));

            telemetry.drone_id = Some(drone_id);
        }

        MavMessage::OPEN_DRONE_ID_LOCATION(location) => {
            let mut drone_id = telemetry.drone_id.clone().unwrap_or(DroneId {
                kind: None,
                id: None,
                kind_of_vehicle: None,
                location: None,
                description: None,
            });

            drone_id.location = Some(DroneIdLocation {
                latitude: f64::from(location.latitude) / 1e7,
                longitude: f64::from(location.longitude) / 1e7,
                altitude: f64::from(location.altitude_barometric),
                height: f64::from(location.height),
                speed: f64::from(location.speed_horizontal) / 100.0,
                vertical_speed: f64::from(location.speed_vertical) / 100.0,
                direction: f64::from(location.direction) / 100.0,
            });

            telemetry.drone_id = Some(drone_id);
        }

        MavMessage::OPEN_DRONE_ID_SELF_ID(self_id) => {
            let mut drone_id = telemetry.drone_id.clone().unwrap_or(DroneId {
                kind: None,
                id: None,
                kind_of_vehicle: None,
                location: None,
                description: None,
            });

            drone_id.description = Some(text_of(&self_id.description));

            telemetry.drone_id = Some(drone_id);
        }

        MavMessage::EFI_STATUS(efi) => {
            // The state of the engine. Only the vehicles that have one say anything
            // about it.
            telemetry.efi = Some(Efi {
                rpm: f64::from(efi.rpm),
                fuel_consumed: f64::from(efi.fuel_consumed),
                fuel_flow: f64::from(efi.fuel_flow),
                engine_load: f64::from(efi.engine_load),
                barometric_pressure: f64::from(efi.barometric_pressure),
                fuel_pressure: f64::from(efi.fuel_pressure),
                intake_temperature: f64::from(efi.intake_manifold_temperature),
                head_temperature: f64::from(efi.cylinder_head_temperature),
                exhaust_temperature: f64::from(efi.exhaust_gas_temperature),
            });
        }

        MavMessage::WIND(wind) => {
            // The wind where the vehicle is. The direction is the one the wind
            // comes from.
            telemetry.wind = Some(Wind {
                direction: f64::from(wind.direction),
                speed: f64::from(wind.speed),
                vertical_speed: f64::from(wind.speed_z),
            });
        }

        MavMessage::SERIAL_CONTROL(control) => {
            // The answer of the vehicle to a passthrough: the bytes its port had.
            let count = (control.count as usize).min(control.data.len());
            telemetry.passthrough = Some(PassthroughData {
                device: control.device as u8,
                data: control.data[..count].to_vec(),
            });
        }

        MavMessage::GPS2_RAW(raw) => {
            // The second receiver, for a vehicle that has one. It reports the same
            // thing as the first, and it may be looking at a different constellation.
            telemetry.gps2 = Some(GpsStatus {
                fix_type: raw.fix_type,
                satellites: raw.satellites_visible,
                // The vehicle reports the dilution in hundredths.
                hdop: f64::from(raw.eph) / 100.0,
            });
        }

        MavMessage::MISSION_CURRENT(current) => {
            // The vehicle streams this whenever the plan changes, so it is the state
            // of the plan rather than a momentary report.
            telemetry.mission = Some(MissionProgress {
                seq: current.seq,
                total: current.total,
                state: plan_state_of(current.mission_state),
                // The mode is a number: 0 unknown, 1 the vehicle can execute the plan,
                // 2 the vehicle is suspended and cannot.
                in_mission_mode: current.mission_mode == 1,
                mission_id: current.mission_id,
                fence_id: current.fence_id,
                rally_id: current.rally_points_id,
            });
        }

        _ => {}
    }

    // The derived values are recomputed here, not on access.
    recompute_derived(telemetry);
}

/// Recomputes the values that depend on more than one field.
fn recompute_derived(telemetry: &mut Telemetry) {
    let (Some(position), Some(home)) = (telemetry.position, telemetry.home) else {
        telemetry.distance_to_home = None;
        telemetry.bearing_to_home = None;
        return;
    };

    let (distance, bearing) = mpcraft_vehicle::great_circle(position, home);
    telemetry.distance_to_home = Some(distance);
    telemetry.bearing_to_home = Some(bearing);
}

/// The state of the plan, as the vehicle reports it.
fn plan_state_of(state: MissionState) -> PlanState {
    match state {
        MissionState::MISSION_STATE_UNKNOWN => PlanState::Unknown,
        MissionState::MISSION_STATE_NO_MISSION => PlanState::NoPlan,
        MissionState::MISSION_STATE_NOT_STARTED => PlanState::NotStarted,
        MissionState::MISSION_STATE_ACTIVE => PlanState::Active,
        MissionState::MISSION_STATE_PAUSED => PlanState::Paused,
        MissionState::MISSION_STATE_COMPLETE => PlanState::Complete,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpcraft_mavlink::dialect::messages::{
        AHRS2_DATA, ATTITUDE_DATA, BATTERY_STATUS_DATA, EKF_STATUS_REPORT_DATA,
        GLOBAL_POSITION_INT_DATA, GPS_RAW_INT_DATA, HEARTBEAT_DATA, HOME_POSITION_DATA,
        MISSION_CURRENT_DATA, NAV_CONTROLLER_OUTPUT_DATA, RAW_IMU_DATA, RC_CHANNELS_DATA,
        RELAY_STATUS_DATA, SCALED_PRESSURE_DATA, SERVO_OUTPUT_RAW_DATA, SYS_STATUS_DATA,
        TIMESYNC_DATA, VFR_HUD_DATA, VIBRATION_DATA,
    };
    use mpcraft_mavlink::{MavGpsFixType, MavType};

    #[test]
    fn a_global_position_updates_the_position_and_the_velocity() {
        let mut aggregator = TelemetryAggregator::new();

        let message = MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
            time_boot_ms: 0,
            lat: (35.2809 * 1e7) as i32,
            lon: (149.1300 * 1e7) as i32,
            alt: 584_000,
            relative_alt: 0,
            vx: 100,
            vy: 0,
            vz: 0,
            hdg: 0,
        });

        aggregator.apply(&message, VehicleId::new(1, 1));

        let telemetry = aggregator.telemetry();
        let position = telemetry.position.expect("a position");
        assert!((position.lat - 35.2809).abs() < 1e-4);
        assert!((position.lng - 149.13).abs() < 1e-4);
        assert!((position.alt - 584.0).abs() < 1.0);

        let velocity = telemetry.velocity.expect("a velocity");
        assert!((velocity.north - 1.0).abs() < 1e-6);
        assert!((velocity.ground_speed - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_second_vehicle_does_not_mix_into_the_primary_one() {
        let mut aggregator = TelemetryAggregator::new();

        let first = MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
            lat: (35.2809 * 1e7) as i32,
            lon: (149.1300 * 1e7) as i32,
            alt: 584_000,
            ..Default::default()
        });

        let second = MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
            lat: (-33.8688 * 1e7) as i32,
            lon: (151.2093 * 1e7) as i32,
            alt: 120_000,
            ..Default::default()
        });

        // Before the selection there is nothing to tell the two apart by, so both read into
        // the view — which is what a link with one vehicle shows.
        aggregator.apply(&first, VehicleId::new(1, 1));
        assert!(aggregator.telemetry().position.is_some());

        aggregator.set_primary(VehicleId::new(1, 1));

        // From here the view belongs to vehicle 1 alone. Vehicle 2 keeps its own state, and
        // its position does not appear as the primary's.
        aggregator.apply(&second, VehicleId::new(2, 1));

        let view = aggregator
            .telemetry()
            .position
            .expect("the view keeps a position");
        assert!(
            (view.lat - 35.2809).abs() < 1e-4,
            "the view was taken for vehicle 2"
        );

        let other = aggregator
            .telemetry_of(VehicleId::new(2, 1))
            .expect("vehicle 2 has its own state");
        let other_position = other.position.expect("the second vehicle keeps a position");
        assert!((other_position.lat - -33.8688).abs() < 1e-4);
    }

    #[test]
    fn the_state_read_before_the_selection_is_not_thrown_away() {
        let mut aggregator = TelemetryAggregator::new();

        let message = MavMessage::SYS_STATUS(SYS_STATUS_DATA {
            voltage_battery: 12_340,
            ..Default::default()
        });

        // The battery arrived before the vehicle was selected; the selection seeds the
        // primary's state with what had already been read, rather than starting it empty.
        aggregator.apply(&message, VehicleId::new(1, 1));
        aggregator.set_primary(VehicleId::new(1, 1));

        assert!(aggregator.telemetry().battery.is_some());
    }

    #[test]
    fn a_sys_status_updates_the_battery() {
        let mut aggregator = TelemetryAggregator::new();

        let message = MavMessage::SYS_STATUS(SYS_STATUS_DATA {
            voltage_battery: 12_340,
            current_battery: -1500,
            battery_remaining: 87,
            ..Default::default()
        });

        aggregator.apply(&message, VehicleId::new(1, 1));

        let battery = aggregator.telemetry().battery.expect("a battery");
        assert!((battery.voltage - 12.34).abs() < 1e-6);
        assert!((battery.current - -15.0).abs() < 1e-6);
        assert!((battery.remaining - 87.0).abs() < 1e-6);
    }

    #[test]
    fn the_distance_to_home_appears_once_both_points_are_known() {
        let mut aggregator = TelemetryAggregator::new();

        let home = MavMessage::HOME_POSITION(HOME_POSITION_DATA {
            latitude: (35.2809 * 1e7) as i32,
            longitude: (149.1300 * 1e7) as i32,
            altitude: 584_000,
            ..Default::default()
        });
        aggregator.apply(&home, VehicleId::new(1, 1));

        let position = MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
            lat: (35.2819 * 1e7) as i32,
            lon: (149.1300 * 1e7) as i32,
            alt: 584_000,
            ..Default::default()
        });
        aggregator.apply(&position, VehicleId::new(1, 1));

        let telemetry = aggregator.telemetry();
        let distance = telemetry.distance_to_home.expect("a distance to home");
        assert!(
            (distance - 111.0).abs() < 5.0,
            "the distance to home was {distance}"
        );
        assert!(telemetry.bearing_to_home.is_some());
    }

    #[test]
    fn the_distance_to_home_is_absent_until_both_points_are_known() {
        let mut aggregator = TelemetryAggregator::new();

        let position = MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
            lat: (35.2819 * 1e7) as i32,
            lon: (149.1300 * 1e7) as i32,
            alt: 584_000,
            ..Default::default()
        });
        aggregator.apply(&position, VehicleId::new(1, 1));

        assert!(aggregator.telemetry().distance_to_home.is_none());
    }

    #[test]
    fn a_heartbeat_sets_the_armed_state() {
        let mut aggregator = TelemetryAggregator::new();

        let message = MavMessage::HEARTBEAT(HEARTBEAT_DATA {
            custom_mode: 5,
            mavtype: MavType::MAV_TYPE_QUADROTOR,
            base_mode: MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED,
            ..Default::default()
        });
        aggregator.apply(&message, VehicleId::new(1, 1));

        assert_eq!(aggregator.telemetry().armed, Some(true));
        assert_eq!(
            aggregator.telemetry().mode.as_ref().map(|m| m.custom_mode),
            Some(5)
        );
    }

    #[test]
    fn a_disarmed_heartbeat_clears_the_armed_state() {
        let mut aggregator = TelemetryAggregator::new();

        let message = MavMessage::HEARTBEAT(HEARTBEAT_DATA {
            base_mode: MavModeFlag::empty(),
            ..Default::default()
        });
        aggregator.apply(&message, VehicleId::new(1, 1));

        assert_eq!(aggregator.telemetry().armed, Some(false));
    }

    #[test]
    fn a_mission_current_updates_the_progress_of_the_plan() {
        let mut aggregator = TelemetryAggregator::new();

        let message = MavMessage::MISSION_CURRENT(MISSION_CURRENT_DATA {
            seq: 3,
            total: 12,
            mission_state: MissionState::MISSION_STATE_ACTIVE,
            mission_mode: 1,
            mission_id: 0x1234,
            fence_id: 0x5678,
            rally_points_id: 0x9abc,
        });

        aggregator.apply(&message, VehicleId::new(1, 1));

        let progress = aggregator
            .telemetry()
            .mission
            .expect("the progress of the plan");

        assert_eq!(progress.seq, 3);
        assert_eq!(progress.total, 12);
        assert_eq!(progress.state, PlanState::Active);
        assert!(progress.in_mission_mode);
        assert_eq!(progress.mission_id, 0x1234);
        assert_eq!(progress.fence_id, 0x5678);
        assert_eq!(progress.rally_id, 0x9abc);
    }

    #[test]
    fn a_suspended_plan_is_not_in_mission_mode() {
        let mut aggregator = TelemetryAggregator::new();

        let message = MavMessage::MISSION_CURRENT(MISSION_CURRENT_DATA {
            seq: 0,
            total: 4,
            mission_state: MissionState::MISSION_STATE_PAUSED,
            // 2 means the vehicle is suspended: it cannot execute the plan.
            mission_mode: 2,
            ..Default::default()
        });

        aggregator.apply(&message, VehicleId::new(1, 1));

        let progress = aggregator
            .telemetry()
            .mission
            .expect("the progress of the plan");

        assert_eq!(progress.state, PlanState::Paused);
        assert!(!progress.in_mission_mode);
    }

    #[test]
    fn a_gps_raw_int_updates_the_state_of_the_positioning() {
        let mut aggregator = TelemetryAggregator::new();

        let message = MavMessage::GPS_RAW_INT(GPS_RAW_INT_DATA {
            time_usec: 0,
            fix_type: MavGpsFixType::GPS_FIX_TYPE_3D_FIX,
            lat: (35.2809 * 1e7) as i32,
            lon: (149.13 * 1e7) as i32,
            alt: 584_000,
            eph: 120,
            epv: 150,
            vel: 0,
            cog: 0,
            satellites_visible: 14,
            ..Default::default()
        });

        aggregator.apply(&message, VehicleId::new(1, 1));

        let gps = aggregator
            .telemetry()
            .gps
            .expect("the state of the positioning");
        assert_eq!(gps.fix_type, MavGpsFixType::GPS_FIX_TYPE_3D_FIX);
        assert_eq!(gps.satellites, 14);
        // The vehicle reports the dilution in hundredths.
        assert!((gps.hdop - 1.2).abs() < 1e-6);
    }

    #[test]
    fn an_unrelated_message_leaves_the_snapshot_alone() {
        let mut aggregator = TelemetryAggregator::new();
        let before = aggregator.telemetry().clone();

        // TIMESYNC is not a telemetry message.
        aggregator.apply(
            &MavMessage::TIMESYNC(TIMESYNC_DATA {
                tc1: 1,
                ts1: 2,
                ..Default::default()
            }),
            VehicleId::new(1, 1),
        );

        assert_eq!(aggregator.telemetry().position, before.position);
        assert_eq!(aggregator.telemetry().battery, before.battery);
    }

    #[test]
    fn a_raw_imu_updates_the_inertial_measurement() {
        let mut aggregator = TelemetryAggregator::new();

        let message = MavMessage::RAW_IMU(RAW_IMU_DATA {
            time_usec: 0,
            // One g on the first axis, in thousandths of a g.
            xacc: 1000,
            yacc: 0,
            zacc: -100,
            // One radian per second on the first axis, in thousandths.
            xgyro: 1000,
            ygyro: 0,
            zgyro: 0,
            // Half a gauss on the first axis, in thousandths.
            xmag: 500,
            ymag: 0,
            zmag: 0,
            id: 0,
            // Twenty degrees, in hundredths.
            temperature: 2000,
        });

        aggregator.apply(&message, VehicleId::new(1, 1));

        let imu = aggregator
            .telemetry()
            .imu
            .expect("the inertial measurement");

        assert!((imu.acceleration[0] - 9.81).abs() < 1e-6);
        assert!(imu.acceleration[1].abs() < 1e-6);
        assert!((imu.acceleration[2] + 0.981).abs() < 1e-6);
        assert!((imu.angular_rate[0] - 1.0).abs() < 1e-6);
        assert!((imu.magnetic_field[0] - 0.5).abs() < 1e-6);
        assert!((imu.temperature - 20.0).abs() < 1e-6);
    }

    #[test]
    fn a_scaled_pressure_updates_the_barometer() {
        let mut aggregator = TelemetryAggregator::new();

        let message = MavMessage::SCALED_PRESSURE(SCALED_PRESSURE_DATA {
            time_boot_ms: 0,
            press_abs: 1013.25,
            press_diff: 4.5,
            temperature: 2150,
            temperature_press_diff: 0,
        });

        aggregator.apply(&message, VehicleId::new(1, 1));

        let barometer = aggregator.telemetry().barometer.expect("the barometer");

        assert!((barometer.absolute_pressure - 1013.25).abs() < 1e-6);
        assert!((barometer.differential_pressure - 4.5).abs() < 1e-6);
        assert!((barometer.temperature - 21.5).abs() < 1e-6);
    }

    #[test]
    fn the_servo_outputs_are_the_pulse_widths() {
        let mut aggregator = TelemetryAggregator::new();

        let message = MavMessage::SERVO_OUTPUT_RAW(SERVO_OUTPUT_RAW_DATA {
            time_usec: 0,
            servo1_raw: 1500,
            servo2_raw: 1600,
            servo3_raw: 1700,
            servo4_raw: 1800,
            servo5_raw: 1900,
            servo6_raw: 2000,
            servo7_raw: 2100,
            servo8_raw: 2200,
            port: 0,
            ..Default::default()
        });

        aggregator.apply(&message, VehicleId::new(1, 1));

        let controls = aggregator.telemetry().controls.expect("the controls");

        assert_eq!(controls.outputs[0], Some(1500.0));
        assert_eq!(controls.outputs[7], Some(2200.0));
    }

    #[test]
    fn the_radio_channels_beyond_the_count_of_the_vehicle_are_unknown() {
        let mut aggregator = TelemetryAggregator::new();

        let message = MavMessage::RC_CHANNELS(RC_CHANNELS_DATA {
            time_boot_ms: 0,
            chancount: 4,
            chan1_raw: 1000,
            chan2_raw: 1100,
            chan3_raw: 1200,
            chan4_raw: 1300,
            chan5_raw: 1400,
            rssi: 90,
            ..Default::default()
        });

        aggregator.apply(&message, VehicleId::new(1, 1));

        let controls = aggregator.telemetry().controls.expect("the controls");

        assert_eq!(controls.inputs[0], Some(1000.0));
        assert_eq!(controls.inputs[3], Some(1300.0));
        // The vehicle says it has four channels, so the fifth was not reported: it is
        // unknown, and not a channel sitting at zero.
        assert_eq!(controls.inputs[4], None);
        assert_eq!(controls.signal, Some(90.0));
    }

    #[test]
    fn the_report_of_the_relays_is_read_into_the_list_the_interface_draws() {
        // The hand: the vehicle says it has one relay — the mask 2 is 10 in binary, so the bit at
        // position 1 — and that relays 0 and 1 are on: the mask 3 is 11. Relay 0 is not a relay
        // this vehicle has, so the second mask cannot make it one, and the list holds relay 1.
        let message = MavMessage::RELAY_STATUS(RELAY_STATUS_DATA {
            time_boot_ms: 4_000,
            on: 0b0011,
            present: 0b0010,
        });

        let mut aggregator = TelemetryAggregator::new();
        aggregator.apply(&message, VehicleId::new(1, 1));

        let relays = aggregator
            .telemetry()
            .relays
            .as_ref()
            .expect("the report of the relays");

        assert_eq!(relays.instances.len(), 1);
        assert_eq!(relays.instances[0].number, 1);
        assert!(relays.instances[0].on);

        // A second board reports its own relays, and what it says stays in its own slot: the view
        // belongs to the primary vehicle, so the interface does not draw a relay of board 2 under
        // the report of board 1.
        aggregator.set_primary(VehicleId::new(1, 1));

        aggregator.apply(
            &MavMessage::RELAY_STATUS(RELAY_STATUS_DATA {
                time_boot_ms: 4_500,
                on: 0b0100,
                present: 0b0100,
            }),
            VehicleId::new(2, 1),
        );

        let primary = aggregator
            .telemetry()
            .relays
            .as_ref()
            .expect("the report of the primary");
        assert_eq!(primary.instances.len(), 1);
        assert_eq!(primary.instances[0].number, 1);

        let other = aggregator
            .telemetry_of(VehicleId::new(2, 1))
            .expect("the state of the second board")
            .relays
            .as_ref()
            .expect("the report of the second board");
        assert_eq!(other.instances[0].number, 2);
    }

    #[test]
    fn the_hud_gives_the_air_data() {
        let mut aggregator = TelemetryAggregator::new();

        let message = MavMessage::VFR_HUD(VFR_HUD_DATA {
            airspeed: 12.5,
            groundspeed: 14.0,
            alt: 100.0,
            climb: -1.5,
            heading: 90,
            throttle: 45,
        });

        aggregator.apply(&message, VehicleId::new(1, 1));

        let airspeed = aggregator.telemetry().airspeed.expect("the air data");

        assert_eq!(airspeed.airspeed, Some(12.5));
        assert!((airspeed.ground_speed - 14.0).abs() < 1e-6);
        assert!((airspeed.climb + 1.5).abs() < 1e-6);
        assert!((airspeed.throttle - 45.0).abs() < 1e-6);
    }

    #[test]
    fn a_battery_status_gives_the_cells_of_the_bank() {
        let mut aggregator = TelemetryAggregator::new();

        let message = MavMessage::BATTERY_STATUS(BATTERY_STATUS_DATA {
            id: 1,
            current_battery: -1500,
            battery_remaining: 85,
            temperature: 2500,
            voltages: [16_800, 16_900, 16_950, 16_900, 16_880, 16_870, 0, 0, 0, 0],
            ..Default::default()
        });

        aggregator.apply(&message, VehicleId::new(1, 1));

        let banks = &aggregator.telemetry().batteries;
        assert_eq!(banks.len(), 1);

        let bank = &banks[0];
        assert_eq!(bank.id, 1);
        // The cells the bank does not have are not reported: a bank of six cells has six
        // entries, and not ten with four of them empty.
        assert_eq!(bank.cells.len(), 6);
        assert!((bank.cells[0] - 16.8).abs() < 1e-6);
        assert!((bank.current + 15.0).abs() < 1e-6);
        assert!((bank.remaining - 85.0).abs() < 1e-6);
        assert!((bank.temperature - 25.0).abs() < 1e-6);
    }

    #[test]
    fn a_bank_that_is_reported_again_replaces_the_one_that_was_there() {
        let mut aggregator = TelemetryAggregator::new();

        for remaining in [90_i8, 80] {
            aggregator.apply(
                &MavMessage::BATTERY_STATUS(BATTERY_STATUS_DATA {
                    id: 0,
                    battery_remaining: remaining,
                    voltages: [16_800, 16_900, 0, 0, 0, 0, 0, 0, 0, 0],
                    ..Default::default()
                }),
                VehicleId::new(1, 1),
            );
        }

        let banks = &aggregator.telemetry().batteries;
        assert_eq!(banks.len(), 1);
        assert!((banks[0].remaining - 80.0).abs() < 1e-6);
    }

    #[test]
    fn the_vibration_and_the_estimator_are_reported() {
        let mut aggregator = TelemetryAggregator::new();

        aggregator.apply(
            &MavMessage::VIBRATION(VIBRATION_DATA {
                vibration_x: 1.5,
                vibration_y: 2.5,
                vibration_z: 3.5,
                ..Default::default()
            }),
            VehicleId::new(1, 1),
        );

        aggregator.apply(
            &MavMessage::EKF_STATUS_REPORT(EKF_STATUS_REPORT_DATA {
                velocity_variance: 0.25,
                pos_horiz_variance: 1.5,
                pos_vert_variance: 2.5,
                compass_variance: 0.5,
                terrain_alt_variance: 3.5,
                airspeed_variance: 4.5,
                ..Default::default()
            }),
            VehicleId::new(1, 1),
        );

        let vibration = aggregator.telemetry().vibration.expect("the vibration");
        assert!((vibration.x - 1.5).abs() < 1e-6);
        assert!((vibration.z - 3.5).abs() < 1e-6);

        let ekf = aggregator.telemetry().ekf.expect("the estimator");
        assert!((ekf.velocity_variance - 0.25).abs() < 1e-6);
        assert!((ekf.horizontal_variance - 1.5).abs() < 1e-6);
        assert!((ekf.airspeed_variance - 4.5).abs() < 1e-6);
    }

    #[test]
    fn the_second_estimator_and_the_wish_of_the_controller_are_reported() {
        let mut aggregator = TelemetryAggregator::new();

        aggregator.apply(
            &MavMessage::AHRS2(AHRS2_DATA {
                roll: 0.1,
                pitch: -0.2,
                yaw: 1.0,
                ..Default::default()
            }),
            VehicleId::new(1, 1),
        );

        aggregator.apply(
            &MavMessage::NAV_CONTROLLER_OUTPUT(NAV_CONTROLLER_OUTPUT_DATA {
                nav_roll: 0.05,
                nav_pitch: 0.02,
                alt_error: -3.0,
                aspd_error: 0.5,
                xtrack_error: 12.0,
                wp_dist: 250,
                ..Default::default()
            }),
            VehicleId::new(1, 1),
        );

        let ahrs2 = aggregator.telemetry().ahrs2.expect("the second estimator");
        assert!((ahrs2.roll - 5.729_578).abs() < 1e-6);
        assert!((ahrs2.pitch - -11.459_156).abs() < 1e-6);

        let controller = aggregator
            .telemetry()
            .controller
            .expect("the wish of the controller");
        assert!((controller.roll - 0.05).abs() < 1e-6);
        assert!((controller.altitude_error + 3.0).abs() < 1e-6);
        assert!((controller.cross_track_error - 12.0).abs() < 1e-6);
        assert!((controller.waypoint_distance - 250.0).abs() < 1e-6);
    }

    #[test]
    fn the_attitude_message_carries_radians_and_the_model_reads_degrees() {
        // The hand: 0.05 · 180/π = 2.864789°, -0.02 · 180/π = -1.145916°, 3.0 · 180/π =
        // 171.887339°. The conversion happens at the wire boundary so that the instrument, the
        // graphs and the formation all read the same thing.
        let mut aggregator = TelemetryAggregator::new();

        aggregator.apply(
            &MavMessage::ATTITUDE(ATTITUDE_DATA {
                time_boot_ms: 0,
                roll: 0.05,
                pitch: -0.02,
                yaw: 3.0,
                ..Default::default()
            }),
            VehicleId::new(1, 1),
        );

        let attitude = aggregator.telemetry().attitude.expect("an attitude");
        assert!((attitude.roll - 2.864_789).abs() < 1e-6);
        assert!((attitude.pitch - -1.145_916).abs() < 1e-6);
        assert!((attitude.yaw - 171.887_339).abs() < 1e-6);
    }

    #[test]
    fn the_identification_of_the_vehicle_is_kept() {
        use mpcraft_mavlink::dialect::messages::{MavOdidIdType, MavOdidUaType};
        use mpcraft_mavlink::dialect::messages::{
            OPEN_DRONE_ID_BASIC_ID_DATA, OPEN_DRONE_ID_LOCATION_DATA, OPEN_DRONE_ID_SELF_ID_DATA,
        };

        let mut aggregator = TelemetryAggregator::new();

        aggregator.apply(
            &MavMessage::OPEN_DRONE_ID_BASIC_ID(OPEN_DRONE_ID_BASIC_ID_DATA {
                target_system: 1,
                target_component: 1,
                id_or_mac: [0; 20],
                id_type: MavOdidIdType::MAV_ODID_ID_TYPE_CAA_REGISTRATION_ID,
                ua_type: MavOdidUaType::MAV_ODID_UA_TYPE_HELICOPTER_OR_MULTIROTOR,
                uas_id: [0; 20],
            }),
            VehicleId::new(1, 1),
        );

        aggregator.apply(&MavMessage::OPEN_DRONE_ID_LOCATION(
            OPEN_DRONE_ID_LOCATION_DATA {
                latitude: -351_680_000,
                longitude: 1_491_300_000,
                altitude_barometric: 584.0,
                altitude_geodetic: 600.0,
                height: 50.0,
                timestamp: 1.0,
                direction: 9000,
                speed_horizontal: 1100,
                speed_vertical: -50,
                target_system: 1,
                target_component: 1,
                id_or_mac: [0; 20],
                status: mpcraft_mavlink::dialect::messages::MavOdidStatus::MAV_ODID_STATUS_UNDECLARED,
                height_reference:
                    mpcraft_mavlink::dialect::messages::MavOdidHeightRef::MAV_ODID_HEIGHT_REF_OVER_TAKEOFF,
                horizontal_accuracy:
                    mpcraft_mavlink::dialect::messages::MavOdidHorAcc::MAV_ODID_HOR_ACC_UNKNOWN,
                vertical_accuracy:
                    mpcraft_mavlink::dialect::messages::MavOdidVerAcc::MAV_ODID_VER_ACC_UNKNOWN,
                barometer_accuracy:
                    mpcraft_mavlink::dialect::messages::MavOdidVerAcc::MAV_ODID_VER_ACC_UNKNOWN,
                speed_accuracy:
                    mpcraft_mavlink::dialect::messages::MavOdidSpeedAcc::MAV_ODID_SPEED_ACC_UNKNOWN,
                timestamp_accuracy:
                    mpcraft_mavlink::dialect::messages::MavOdidTimeAcc::MAV_ODID_TIME_ACC_UNKNOWN,
            },
        ), VehicleId::new(1, 1));

        aggregator.apply(
            &MavMessage::OPEN_DRONE_ID_SELF_ID(OPEN_DRONE_ID_SELF_ID_DATA {
                target_system: 1,
                target_component: 1,
                id_or_mac: [0; 20],
                description_type:
                    mpcraft_mavlink::dialect::messages::MavOdidDescType::MAV_ODID_DESC_TYPE_TEXT,
                description: mpcraft_mavlink::dialect::CharArray::new({
                    let mut data = [0_u8; 23];
                    data[..14].copy_from_slice(b"over the field");
                    data
                }),
            }),
            VehicleId::new(1, 1),
        );

        let drone_id = aggregator
            .telemetry()
            .drone_id
            .clone()
            .expect("the vehicle has said who it is");

        assert_eq!(drone_id.kind.as_deref(), Some("A registration"));
        assert_eq!(
            drone_id.kind_of_vehicle.as_deref(),
            Some("A helicopter or a multirotor")
        );
        assert_eq!(drone_id.description.as_deref(), Some("over the field"));

        let location = drone_id.location.expect("the vehicle has said where it is");
        assert!((location.latitude - -35.168).abs() < 1e-6);
        assert!((location.longitude - 149.13).abs() < 1e-6);
        assert!((location.speed - 11.0).abs() < 1e-6);
        assert!((location.vertical_speed - -0.5).abs() < 1e-6);
        assert!((location.direction - 90.0).abs() < 1e-6);
    }
}
