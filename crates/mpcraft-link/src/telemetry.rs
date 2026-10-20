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
    Attitude, Battery, FlightMode, GeoPoint, GpsStatus, MissionProgress, PlanState, Telemetry,
    Velocity,
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
        MISSION_CURRENT_DATA, SYS_STATUS_DATA, TIMESYNC_DATA,
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
}
