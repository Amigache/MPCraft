//! The dispatch of the formation: the ten-times-a-second loop that runs the decision for
//! each member and puts it on the wire.
//!
//! The decision itself lives in `mpcraft_vehicle::formation` — pure computation over
//! snapshots — and the wire shape lives in `mpcraft_link::commands`. What belongs to the
//! application is the policy between them: which vehicles are in the formation, which slot
//! each was given, whose state the formation follows, and how often the loop runs. Upstream
//! runs the same loop from its main tick (`Swarm/Formation.cs`, `SendCommand`).
//!
//! # What the loop does on a tick
//!
//! For every member, read from the per-vehicle telemetry rather than from the primary's:
//! a formation is exactly the case where the state of several boards must not be read as
//! one. The leader is skipped, as upstream skips it. The branch is the vehicle's kind, once
//! its heartbeat has been seen — a plane gets the quaternion answer, the others get the
//! position target, and the heading only when the difference is past the dead band.
//!
//! What this loop can prove here is that the right decision reaches the right vehicle in the
//! right message; what it cannot prove without a live fleet is that the vehicles fly into
//! their slots, so the parity row stays partial.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::CommandError;
use mpcraft_geo::formation::{FormationError, Layout, Offset};
use mpcraft_link::{LinkError, Session, SessionState, VehicleId, VehicleKind};
use mpcraft_vehicle::formation::{AttitudeCommand, MulticopterDecision, Swarm};

/// How often the loop runs, which is what upstream's swarm tick does — ten times a second.
pub const TICK_INTERVAL: Duration = Duration::from_millis(100);

/// One vehicle's place in the formation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotSetting {
    /// The vehicle the slot is for.
    pub vehicle: VehicleId,
    /// Metres across the direction of travel, positive to the right.
    pub x: f64,
    /// Metres along the direction of travel, positive ahead.
    pub y: f64,
    /// Metres above the leader.
    pub z: f64,
}

impl SlotSetting {
    /// The offset as the formation model holds it.
    #[must_use]
    pub const fn offset(&self) -> Offset {
        Offset::new(self.x, self.y, self.z)
    }
}

/// Who leads and who sits where.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwarmSettings {
    /// The vehicle the formation follows.
    pub leader: VehicleId,
    /// The members and their slots. The leader is never a member: upstream skips it.
    pub slots: Vec<SlotSetting>,
}

/// What one vehicle is told on one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwarmOrder {
    /// What a plane is told: the quaternion, the mask, the thrust.
    Attitude(AttitudeCommand),
    /// What a multicopter is told: the place, the speeds, and the heading when past the band.
    PositionTarget(MulticopterDecision),
}

/// Why the loop could not run or could not reach the wire.
#[derive(Debug, thiserror::Error)]
pub enum SwarmError {
    /// A slot could not be put onto the ground — the leader is where no UTM zone can be picked.
    #[error("{0}")]
    Formation(#[from] FormationError),
    /// The link could not carry what the loop decided.
    #[error("{0}")]
    Link(#[from] LinkError),
}

impl SwarmError {
    /// The stable code the frontend branches on.
    #[must_use]
    pub fn code(&self) -> &'static str {
        // Both arms are things the operator cannot fix from the interface: a leader that
        // cannot be projected and a link that cannot carry the answer.
        CommandError::CODE_INTERNAL
    }
}

/// The service: the settings, the formation model with what its filters remember, and the loop.
#[derive(Debug, Clone, PartialEq)]
pub struct SwarmService {
    settings: SwarmSettings,
    swarm: Swarm,
}

impl SwarmService {
    /// Creates the service from the settings.
    #[must_use]
    pub fn new(settings: SwarmSettings) -> Self {
        let mut layout = Layout::new("swarm");

        for slot in &settings.slots {
            layout.set_slot(slot.vehicle.system_id, slot.offset());
        }

        let mut swarm = Swarm::new(layout);

        for slot in &settings.slots {
            swarm.add_member(slot.vehicle.system_id);
        }

        Self { settings, swarm }
    }

    /// The settings the service runs from.
    #[must_use]
    pub fn settings(&self) -> &SwarmSettings {
        &self.settings
    }

    /// One tick of the loop: what each member is told, in system-id order.
    ///
    /// A member whose heartbeat has not been seen yet is skipped — its kind is not known, so
    /// which branch it belongs to is not known either. A member whose state is not known yet
    /// is skipped for the same reason upstream guards with `lat == 0`.
    ///
    /// # Errors
    ///
    /// Returns an error when a slot cannot be projected.
    pub fn tick(
        &mut self,
        state: &SessionState,
    ) -> Result<Vec<(VehicleId, SwarmOrder)>, FormationError> {
        let Some(leader) = state.telemetries.get(&self.settings.leader) else {
            return Ok(Vec::new());
        };

        if leader.is_empty() {
            return Ok(Vec::new());
        }

        // The slots are read through a copy so that the formation's filters can be used while
        // the settings are being walked.
        let slots = self.settings.slots.clone();
        let mut orders = Vec::new();

        for slot in &slots {
            let vehicle = slot.vehicle;

            if vehicle == self.settings.leader {
                continue;
            }

            let Some(telemetry) = state.telemetries.get(&vehicle) else {
                continue;
            };

            if telemetry.is_empty() {
                continue;
            }

            let kind = state
                .vehicles
                .iter()
                .find(|recorded| recorded.id == vehicle)
                .and_then(|recorded| recorded.kind);

            match kind {
                Some(VehicleKind::ArduPlane) => {
                    if let Some(decision) =
                        self.swarm
                            .plane_decision(leader, vehicle.system_id, telemetry)?
                    {
                        orders.push((vehicle, SwarmOrder::Attitude(decision.attitude_command())));
                    }
                }
                Some(_) => {
                    if let Some(decision) =
                        self.swarm
                            .multicopter_decision(leader, vehicle.system_id, telemetry)?
                    {
                        orders.push((vehicle, SwarmOrder::PositionTarget(decision)));
                    }
                }
                None => {}
            }
        }

        Ok(orders)
    }

    /// Runs one tick and sends what it decided. Returns the number of vehicles that were told.
    ///
    /// The condition-yaw order goes out only when the decision says the difference is past the
    /// dead band, and nothing is waited for: at ten hertz there is no time to wait for an
    /// acknowledgement, and the protocol does not give one for the position target either.
    ///
    /// # Errors
    ///
    /// Returns an error when a slot cannot be projected or the link cannot carry what was
    /// decided.
    pub async fn dispatch(&mut self, session: &Session) -> Result<usize, SwarmError> {
        let state = session.state();
        let orders = self.tick(&state)?;

        for (vehicle, order) in &orders {
            match order {
                SwarmOrder::Attitude(command) => {
                    session.send_attitude_target(*vehicle, command).await?;
                }
                SwarmOrder::PositionTarget(decision) => {
                    session
                        .send_swarm_position_target(*vehicle, decision)
                        .await?;

                    if let Some(yaw) = decision.yaw_deg {
                        session.send_swarm_condition_yaw(*vehicle, yaw).await?;
                    }
                }
            }
        }

        Ok(orders.len())
    }

    /// Runs the loop while the link is open.
    pub async fn run(&mut self, session: &Session) {
        while session.state().connected {
            match self.dispatch(session).await {
                Ok(_) => {}
                Err(error) => tracing::warn!("the formation could not send: {error}"),
            }

            tokio::time::sleep(TICK_INTERVAL).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpcraft_link::Vehicle;
    use mpcraft_vehicle::{Attitude, GeoPoint, Telemetry, Velocity};

    // The leader on the central meridian of its zone, where a grid direction is a true one:
    // zone 31 spans 0° to 6°, and its central meridian is 3°.
    const LEADER: GeoPoint = GeoPoint {
        lat: -35.0,
        lng: 3.0,
        alt: 100.0,
    };

    /// A state with one vehicle, so that the tick reads it from the per-vehicle slots rather
    /// than from the mixed view.
    fn report(state: &SessionState, vehicle: VehicleId, telemetry: Telemetry) -> SessionState {
        let mut state = state.clone();
        state.telemetries.insert(vehicle, telemetry);
        state
    }

    fn empty_state() -> SessionState {
        SessionState::disconnected()
    }

    fn plane(position: GeoPoint, yaw: f64, velocity: Option<Velocity>) -> Telemetry {
        Telemetry {
            position: Some(position),
            attitude: Some(Attitude {
                roll: 0.0,
                pitch: 0.0,
                yaw,
            }),
            velocity,
            ..Telemetry::default()
        }
    }

    #[test]
    fn a_plane_member_gets_the_answer_the_loop_hand_computed() {
        // The hand: the slot is the leader's own place, and the vehicle sits 60 metres due
        // south of it on the central meridian, facing north.
        //
        //   distance to target    = 60 m, by construction
        //   bearing to target     = 0°, along the meridian
        //   heading error         = 0° — the vehicle already points at the target
        //   pitch term            = 0, the altitudes are equal
        //   thrust term           = 60 × 0.01 + 60 × 0.001 × 0.1 = 0.606, the first tick,
        //                           with the integrator reset because the distance is past 40
        //   bank                  = 0, the sine rule is not reached at zero bearing error
        //   quaternion            = the identity, because all three angles are zero
        let leader = VehicleId::new(1, 1);
        let member = VehicleId::new(2, 1);

        let south_of_leader = GeoPoint {
            lat: LEADER.lat - 60.0 / 6_371_000.0 * 180.0 / std::f64::consts::PI,
            lng: LEADER.lng,
            alt: 100.0,
        };

        let mut state = empty_state();
        state
            .vehicles
            .push(recorded(member, VehicleKind::ArduPlane));

        let state = report(
            &state,
            leader,
            plane(
                LEADER,
                0.0,
                Some(Velocity {
                    north: 8.0,
                    east: 0.0,
                    down: 0.0,
                    ground_speed: 8.0,
                }),
            ),
        );
        let state = report(&state, member, plane(south_of_leader, 0.0, None));

        let settings = SwarmSettings {
            leader,
            slots: vec![SlotSetting {
                vehicle: member,
                x: 0.0,
                y: 0.0,
                z: 0.0,
            }],
        };

        let mut service = SwarmService::new(settings);
        let orders = service.tick(&state).expect("the slot projects");

        assert_eq!(orders.len(), 1);

        let (vehicle, order) = orders[0];
        assert_eq!(vehicle, member);

        let SwarmOrder::Attitude(command) = order else {
            panic!("a plane is told the quaternion answer");
        };

        assert_eq!(command.type_mask, 0x38);
        assert!(
            (command.thrust - 0.606).abs() < 0.01,
            "thrust: {}",
            command.thrust
        );

        // The identity quaternion, read independently of the implementation's matrix.
        assert!((command.quaternion[0] - 1.0).abs() < 1e-9);
        for term in &command.quaternion[1..] {
            assert!(term.abs() < 1e-9, "term: {term}");
        }
    }

    #[test]
    fn a_multicopter_member_gets_the_slot_and_the_leader_speeds() {
        // The hand: on the central meridian with the leader facing north, a slot of forty
        // metres ahead is forty metres north of the leader on the ground — through the grid it
        // is forty divided by the scale factor of the projection, 0.9996. The height is the
        // leader's plus the height of the slot, and the heading is asked for because ten
        // degrees is past the three-degree band.
        let leader = VehicleId::new(1, 1);
        let member = VehicleId::new(3, 1);

        let mut state = empty_state();
        state
            .vehicles
            .push(recorded(member, VehicleKind::ArduCopter));

        let leader_state = plane(
            LEADER,
            0.0,
            Some(Velocity {
                north: 8.0,
                east: 0.0,
                down: 0.0,
                ground_speed: 8.0,
            }),
        );

        let member_state = Telemetry {
            position: Some(GeoPoint {
                lat: -35.2,
                lng: 3.0,
                alt: 90.0,
            }),
            attitude: Some(Attitude {
                roll: 0.0,
                pitch: 0.0,
                yaw: 10.0,
            }),
            ..Telemetry::default()
        };

        let state = report(&report(&state, leader, leader_state), member, member_state);

        let settings = SwarmSettings {
            leader,
            slots: vec![SlotSetting {
                vehicle: member,
                x: 0.0,
                y: 40.0,
                z: 5.0,
            }],
        };

        let mut service = SwarmService::new(settings);
        let orders = service.tick(&state).expect("the slot projects");

        assert_eq!(orders.len(), 1);

        let SwarmOrder::PositionTarget(decision) = orders[0].1 else {
            panic!("a multicopter is told the place and the speeds");
        };

        let expected_lat =
            LEADER.lat + 40.0 / (0.9996 * 6_371_000.0) * 180.0 / std::f64::consts::PI;
        assert!(
            (decision.target.lat - expected_lat).abs() < 5e-6,
            "the target is the slot through the grid: {}",
            decision.target.lat
        );
        assert!((decision.target.lng - LEADER.lng).abs() < 1e-9);
        assert_eq!(decision.target.alt, 105.0);

        // The leader's speeds, passed through rather than derived from the member's own.
        let velocity = decision.velocity.expect("the leader was flying");
        assert_eq!(velocity.north, 8.0);

        // Past the dead band, so the heading is asked for.
        assert_eq!(decision.yaw_deg, Some(0.0));
    }

    #[test]
    fn a_member_inside_the_dead_band_is_not_asked_to_turn() {
        let leader = VehicleId::new(1, 1);
        let member = VehicleId::new(4, 1);

        let mut state = empty_state();
        state
            .vehicles
            .push(recorded(member, VehicleKind::ArduCopter));

        let leader_state = plane(LEADER, 0.0, None);
        let member_state = Telemetry {
            position: Some(GeoPoint {
                lat: -35.2,
                lng: 3.0,
                alt: 90.0,
            }),
            attitude: Some(Attitude {
                roll: 0.0,
                pitch: 0.0,
                yaw: 2.0,
            }),
            ..Telemetry::default()
        };

        let state = report(&report(&state, leader, leader_state), member, member_state);

        let settings = SwarmSettings {
            leader,
            slots: vec![SlotSetting {
                vehicle: member,
                x: 0.0,
                y: 40.0,
                z: 0.0,
            }],
        };

        let mut service = SwarmService::new(settings);
        let orders = service.tick(&state).expect("the slot projects");

        assert_eq!(orders.len(), 1);

        let SwarmOrder::PositionTarget(decision) = orders[0].1 else {
            panic!("a multicopter is told the place and the speeds");
        };

        assert_eq!(
            decision.yaw_deg, None,
            "two degrees is inside the three-degree band"
        );
    }

    #[test]
    fn the_leader_is_skipped_and_vehicles_that_have_not_reported_are_left_out() {
        let leader = VehicleId::new(1, 1);
        let reported = VehicleId::new(2, 1);
        let silent = VehicleId::new(5, 1);

        let mut state = empty_state();
        state
            .vehicles
            .push(recorded(reported, VehicleKind::ArduPlane));

        // The leader has a slot here as well, which is what upstream's loop would have sent a
        // command for if it did not skip the leader; the loop does skip it.
        let settings = SwarmSettings {
            leader,
            slots: vec![
                SlotSetting {
                    vehicle: leader,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                SlotSetting {
                    vehicle: reported,
                    x: 0.0,
                    y: 60.0,
                    z: 0.0,
                },
                SlotSetting {
                    vehicle: silent,
                    x: 0.0,
                    y: 80.0,
                    z: 0.0,
                },
            ],
        };

        let state = report(
            &state,
            leader,
            plane(
                LEADER,
                0.0,
                Some(Velocity {
                    north: 8.0,
                    east: 0.0,
                    down: 0.0,
                    ground_speed: 8.0,
                }),
            ),
        );
        let state = report(
            &state,
            reported,
            Telemetry {
                position: Some(GeoPoint {
                    lat: -35.2,
                    lng: 3.0,
                    alt: 90.0,
                }),
                attitude: Some(Attitude {
                    roll: 0.0,
                    pitch: 0.0,
                    yaw: 0.0,
                }),
                ..Telemetry::default()
            },
        );

        // The silent vehicle has a slot and is a member, but nothing has been heard from it:
        // no heartbeat, so no kind, and no state to decide from.
        assert!(!state.telemetries.contains_key(&silent));

        let mut service = SwarmService::new(settings);
        let orders = service.tick(&state).expect("the slots project");

        let vehicles: Vec<VehicleId> = orders.iter().map(|(vehicle, _)| *vehicle).collect();
        assert_eq!(vehicles, vec![reported]);
    }

    /// A vehicle as the registry holds it, once its heartbeat has been seen.
    fn recorded(vehicle: VehicleId, kind: VehicleKind) -> Vehicle {
        let mut record = Vehicle::new(vehicle);
        record.kind = Some(kind);
        record
    }
}
