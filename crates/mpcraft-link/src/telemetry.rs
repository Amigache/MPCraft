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

use mpcraft_vehicle::{
    Airspeed, Attitude, Barometer, Battery, Controls, FlightMode, GeoPoint, GpsStatus, Imu,
    MissionProgress, PlanState, Telemetry, Velocity,
};

use mpcraft_mavlink::dialect::messages::MissionState;
use mpcraft_mavlink::{MavMessage, MavModeFlag};

/// Decodes messages into a telemetry snapshot.
#[derive(Debug, Default)]
pub struct TelemetryAggregator {
    telemetry: Telemetry,
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
        &self.telemetry
    }

    /// Applies a decoded message to the snapshot.
    ///
    /// Messages that carry no telemetry are ignored.
    pub fn apply(&mut self, message: &MavMessage) {
        match message {
            MavMessage::ATTITUDE(attitude) => {
                self.telemetry.attitude = Some(Attitude {
                    roll: f64::from(attitude.roll),
                    pitch: f64::from(attitude.pitch),
                    yaw: f64::from(attitude.yaw),
                });
            }

            MavMessage::GLOBAL_POSITION_INT(position) => {
                // The integer encoding scales latitude and longitude by 1e7, and the
                // altitudes by 1e3 to give millimetres.
                self.telemetry.position = Some(GeoPoint {
                    lat: f64::from(position.lat) / 1e7,
                    lng: f64::from(position.lon) / 1e7,
                    alt: f64::from(position.alt) / 1e3,
                });

                // The velocity is in centimetres per second, in the north-east-down frame.
                let north = f64::from(position.vx) / 100.0;
                let east = f64::from(position.vy) / 100.0;
                let down = f64::from(position.vz) / 100.0;

                self.telemetry.velocity = Some(Velocity {
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
                let velocity = self.telemetry.velocity.get_or_insert(Velocity {
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
                let airspeed = self.telemetry.airspeed.get_or_insert(Airspeed {
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
                self.telemetry.battery = Some(Battery {
                    voltage: f64::from(status.voltage_battery) / 1000.0,
                    current: f64::from(status.current_battery) / 100.0,
                    remaining: f64::from(status.battery_remaining),
                });
            }

            MavMessage::HOME_POSITION(home) => {
                self.telemetry.home = Some(GeoPoint {
                    lat: f64::from(home.latitude) / 1e7,
                    lng: f64::from(home.longitude) / 1e7,
                    alt: f64::from(home.altitude) / 1e3,
                });
            }

            MavMessage::HEARTBEAT(heartbeat) => {
                self.telemetry.armed = Some(
                    heartbeat
                        .base_mode
                        .contains(MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED),
                );

                // The mode name comes from the vehicle's FLTMODE1 parameter, which is read
                // with the parameter metadata. Until then the numeric custom mode is kept.
                self.telemetry.mode = Some(FlightMode {
                    custom_mode: heartbeat.custom_mode,
                    name: None,
                });
            }

            MavMessage::GPS_RAW_INT(raw) => {
                self.telemetry.gps = Some(GpsStatus {
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
                self.telemetry.imu = Some(Imu {
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
                if self.telemetry.imu.is_none() {
                    self.telemetry.imu = Some(Imu {
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
                self.telemetry.barometer = Some(Barometer {
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
                let mut controls = self.telemetry.controls.unwrap_or(Controls {
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

                self.telemetry.controls = Some(controls);
            }

            MavMessage::RC_CHANNELS(channels) => {
                // The value of each radio channel, in microseconds. The vehicle says how
                // many channels it has, and the ones beyond that count are not reported.
                let mut controls = self.telemetry.controls.unwrap_or(Controls {
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

                self.telemetry.controls = Some(controls);
            }

            MavMessage::MISSION_CURRENT(current) => {
                // The vehicle streams this whenever the plan changes, so it is the state
                // of the plan rather than a momentary report.
                self.telemetry.mission = Some(MissionProgress {
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
        self.recompute_derived();
    }

    /// Recomputes the values that depend on more than one field.
    fn recompute_derived(&mut self) {
        let (Some(position), Some(home)) = (self.telemetry.position, self.telemetry.home) else {
            self.telemetry.distance_to_home = None;
            self.telemetry.bearing_to_home = None;
            return;
        };

        let (distance, bearing) = mpcraft_vehicle::great_circle(position, home);
        self.telemetry.distance_to_home = Some(distance);
        self.telemetry.bearing_to_home = Some(bearing);
    }
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
        GLOBAL_POSITION_INT_DATA, GPS_RAW_INT_DATA, HEARTBEAT_DATA, HOME_POSITION_DATA,
        MISSION_CURRENT_DATA, RAW_IMU_DATA, RC_CHANNELS_DATA, SCALED_PRESSURE_DATA,
        SERVO_OUTPUT_RAW_DATA, SYS_STATUS_DATA, TIMESYNC_DATA, VFR_HUD_DATA,
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

        aggregator.apply(&message);

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
    fn a_sys_status_updates_the_battery() {
        let mut aggregator = TelemetryAggregator::new();

        let message = MavMessage::SYS_STATUS(SYS_STATUS_DATA {
            voltage_battery: 12_340,
            current_battery: -1500,
            battery_remaining: 87,
            ..Default::default()
        });

        aggregator.apply(&message);

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
        aggregator.apply(&home);

        let position = MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
            lat: (35.2819 * 1e7) as i32,
            lon: (149.1300 * 1e7) as i32,
            alt: 584_000,
            ..Default::default()
        });
        aggregator.apply(&position);

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
        aggregator.apply(&position);

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
        aggregator.apply(&message);

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
        aggregator.apply(&message);

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

        aggregator.apply(&message);

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

        aggregator.apply(&message);

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

        aggregator.apply(&message);

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
        aggregator.apply(&MavMessage::TIMESYNC(TIMESYNC_DATA {
            tc1: 1,
            ts1: 2,
            ..Default::default()
        }));

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

        aggregator.apply(&message);

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

        aggregator.apply(&message);

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

        aggregator.apply(&message);

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

        aggregator.apply(&message);

        let controls = aggregator.telemetry().controls.expect("the controls");

        assert_eq!(controls.inputs[0], Some(1000.0));
        assert_eq!(controls.inputs[3], Some(1300.0));
        // The vehicle says it has four channels, so the fifth was not reported: it is
        // unknown, and not a channel sitting at zero.
        assert_eq!(controls.inputs[4], None);
        assert_eq!(controls.signal, Some(90.0));
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

        aggregator.apply(&message);

        let airspeed = aggregator.telemetry().airspeed.expect("the air data");

        assert_eq!(airspeed.airspeed, Some(12.5));
        assert!((airspeed.ground_speed - 14.0).abs() < 1e-6);
        assert!((airspeed.climb + 1.5).abs() < 1e-6);
        assert!((airspeed.throttle - 45.0).abs() < 1e-6);
    }
}
