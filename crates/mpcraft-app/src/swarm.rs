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
//! # What this layer decides
//!
//! The settings come from the file, where the places and the decision to run are kept apart:
//! writing slots for a fleet does not start it. A vehicle is named in the file as the link
//! names it (`2:1`), and a name that cannot be read as a vehicle is left out and said so —
//! the same rule a secret that is not hexadecimal follows in the keyring, because guessing at
//! a name would send an order to a board nobody wrote.
//!
//! What this loop can prove here is that the right decision reaches the right vehicle in the
//! right message; what it cannot prove without a live fleet is that the vehicles fly into
//! their slots, so the parity row stays partial.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::CommandError;
use crate::supervisor::SessionSupervisor;
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

    /// The vehicle as the file and the interface name it: `system:component`.
    #[must_use]
    pub fn key(&self) -> String {
        format!("{}:{}", self.vehicle.system_id, self.vehicle.component_id)
    }
}

/// Who leads and who sits where, as the model reads it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwarmSettings {
    /// The vehicle the formation follows, when somebody has named one.
    pub leader: Option<VehicleId>,
    /// The members and their slots. The leader is never a member: upstream skips it.
    pub slots: Vec<SlotSetting>,
}

/// What the interface sends for the formation: what the operator wrote, and the decision.
///
/// The names are sent as the file writes them, so that what is saved is what was written rather
/// than what an interface guessed from it — including a name that cannot be read as a vehicle,
/// which stays in the file and is reported, because a person fixing the file has to see what they
/// actually wrote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwarmRequest {
    /// Who leads, named the way the file names it.
    pub leader: Option<String>,
    /// Whether the loop is run.
    pub enabled: bool,
    /// The places, in the order they were written.
    pub slots: Vec<mpcraft_settings::SwarmSlot>,
}

/// What one vehicle is told on one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwarmOrder {
    /// What a plane is told: the quaternion, the mask, the thrust.
    Attitude(AttitudeCommand),
    /// What a multicopter is told: the place, the speeds, and the heading when past the band.
    PositionTarget(MulticopterDecision),
}

/// Why the loop could not run, could not be started, or could not reach the wire.
#[derive(Debug, thiserror::Error)]
pub enum SwarmError {
    /// A slot could not be put onto the ground — the leader is where no UTM zone can be picked.
    #[error("{0}")]
    Formation(#[from] FormationError),
    /// The link could not carry what the loop decided.
    #[error("{0}")]
    Link(#[from] LinkError),
    /// A name that cannot be read as a vehicle.
    #[error("the name {0} is not a vehicle the link can name: it is not `system:component`")]
    NotAVehicle(String),
    /// The loop is already running.
    #[error("the loop is already running")]
    Busy,
    /// The state of the formation could not be read, which is a fault of the application rather
    /// than of what the operator wrote.
    #[error("the state of the formation is locked")]
    Locked,
}

impl SwarmError {
    /// The stable code the frontend branches on.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            // Both are things the operator can fix from the interface: a name that was written
            // wrong, and a loop that was started twice.
            Self::NotAVehicle(_) | Self::Busy => CommandError::CODE_INVALID_REQUEST,
            // A leader that cannot be projected, a link that cannot carry the answer, and a state
            // that could not be read are not things the operator can fix from the interface.
            Self::Formation(_) | Self::Link(_) | Self::Locked => CommandError::CODE_INTERNAL,
        }
    }
}

/// Reads a vehicle from the name the file gives it.
///
/// The two halves are the pair the protocol identifies a board by. Zero is refused because zero
/// is not a board: it is the broadcast address, and an order sent to it goes nowhere in
/// particular — which is exactly what an operator who wrote `0:1` would not have meant.
pub fn read_vehicle(name: &str) -> Result<VehicleId, SwarmError> {
    let (system, component) = name
        .split_once(':')
        .ok_or_else(|| SwarmError::NotAVehicle(name.to_owned()))?;

    let system: u8 = system
        .parse()
        .map_err(|_| SwarmError::NotAVehicle(name.to_owned()))?;
    let component: u8 = component
        .parse()
        .map_err(|_| SwarmError::NotAVehicle(name.to_owned()))?;

    if system == 0 || component == 0 {
        return Err(SwarmError::NotAVehicle(name.to_owned()));
    }

    Ok(VehicleId::new(system, component))
}

/// One slot as the interface reads it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotStatus {
    /// The vehicle the slot belongs to, named the way the file names it.
    pub vehicle: String,
    /// Metres across the direction of travel, positive to the right.
    pub x: f64,
    /// Metres along the direction of travel, positive ahead.
    pub y: f64,
    /// Metres above the leader.
    pub z: f64,
}

/// What the formation is doing.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwarmStatus {
    /// Whether the operator decided to run the loop.
    pub enabled: bool,
    /// Whether the loop is running now.
    pub running: bool,
    /// How many vehicles the last tick told.
    pub told: usize,
    /// Who leads, named the way the file names it, when somebody said so.
    pub leader: Option<String>,
    /// The places, in the order the file wrote them.
    pub slots: Vec<SlotStatus>,
    /// The names that could not be read as vehicles, and were left out.
    pub refused: Vec<String>,
    /// The last failure of the loop, when there was one.
    pub error: Option<String>,
}

/// The state the task owns.
struct Inner {
    settings: SwarmSettings,
    swarm: Swarm,
    enabled: bool,
    running: bool,
    told: usize,
    refused: Vec<String>,
    error: Option<String>,
    task: Option<tokio::task::JoinHandle<()>>,
}

/// The formation, with what its filters remember and the loop that runs it.
#[derive(Clone)]
pub struct SwarmService {
    inner: Arc<std::sync::Mutex<Inner>>,
}

/// A service that has been told nothing yet, which is what `AppState::default()` holds.
///
/// `Default` cannot be derived here because the formation model keeps its filters and has no
/// default of its own; the empty state is what `new` says it is.
impl Default for SwarmService {
    fn default() -> Self {
        Self::new()
    }
}

impl SwarmService {
    /// A service that has been told nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(std::sync::Mutex::new(Inner {
                settings: SwarmSettings {
                    leader: None,
                    slots: Vec::new(),
                },
                swarm: Swarm::new(Layout::new("swarm")),
                enabled: false,
                running: false,
                told: 0,
                refused: Vec::new(),
                error: None,
                task: None,
            })),
        }
    }

    /// Reads the settings from the file: the places, the leader, and the decision to run.
    ///
    /// A name that cannot be read as a vehicle is left out rather than guessed at, and it is kept
    /// so that the interface can say which entry the operator has to fix.
    pub fn apply(&self, file: &mpcraft_settings::Settings) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };

        let settings = SwarmSettings {
            leader: match file.swarm_leader.as_deref() {
                Some(name) => match read_vehicle(name) {
                    Ok(vehicle) => Some(vehicle),
                    Err(_) => {
                        inner.refused.push(name.to_owned());
                        None
                    }
                },
                None => None,
            },
            slots: file
                .swarm_slots
                .iter()
                .filter_map(|slot| match read_vehicle(&slot.vehicle) {
                    Ok(vehicle) => Some(SlotSetting {
                        vehicle,
                        x: slot.x,
                        y: slot.y,
                        z: slot.z,
                    }),
                    Err(_) => {
                        inner.refused.push(slot.vehicle.clone());
                        None
                    }
                })
                .collect(),
        };

        inner.settings = settings;
        inner.enabled = file.swarm_enabled;
        rebuild(&mut inner);
    }

    /// What the formation is doing.
    #[must_use]
    pub fn status(&self) -> SwarmStatus {
        let Ok(inner) = self.inner.lock() else {
            return SwarmStatus::default();
        };

        SwarmStatus {
            enabled: inner.enabled,
            running: inner.running,
            told: inner.told,
            leader: inner
                .settings
                .leader
                .map(|vehicle| format!("{}:{}", vehicle.system_id, vehicle.component_id)),
            slots: inner
                .settings
                .slots
                .iter()
                .map(|slot| SlotStatus {
                    vehicle: slot.key(),
                    x: slot.x,
                    y: slot.y,
                    z: slot.z,
                })
                .collect(),
            refused: inner.refused.clone(),
            error: inner.error.clone(),
        }
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
    pub fn tick(&self, state: &SessionState) -> Result<Vec<(VehicleId, SwarmOrder)>, SwarmError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SwarmError::Locked);
        };

        tick_inner(&mut inner, state).map_err(SwarmError::from)
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
    pub async fn dispatch(&self, session: &Session) -> Result<usize, SwarmError> {
        let orders = self.tick(&session.state())?;

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

        if let Ok(mut inner) = self.inner.lock() {
            inner.told = orders.len();
        }

        Ok(orders.len())
    }

    /// Starts the loop.
    ///
    /// The loop asks the supervisor for the link on every tick rather than holding one, so that a
    /// link that is restored after dropping is the link the loop talks to. A loop that held the
    /// session it started with would keep sending to a link that was closed.
    ///
    /// # Errors
    ///
    /// Returns an error when the loop is already running.
    pub fn start(&self, supervisor: &SessionSupervisor) -> Result<(), SwarmError> {
        let inner = Arc::clone(&self.inner);
        let supervisor = supervisor.clone();

        let task = tokio::spawn(async move {
            while is_running(&inner) {
                let Some(session) = supervisor.session() else {
                    tokio::time::sleep(TICK_INTERVAL).await;
                    continue;
                };

                match dispatch_for(&inner, &session).await {
                    Ok(_) => {}
                    Err(error) => {
                        if let Ok(mut guard) = inner.lock() {
                            guard.error = Some(error.to_string());
                        }

                        tracing::warn!("the formation could not send: {error}");
                    }
                }

                // The stop is honoured between the sends and the next tick, which is what a stop
                // means for a loop that waits on the wire.
                if !is_running(&inner) {
                    break;
                }

                tokio::time::sleep(TICK_INTERVAL).await;
            }
        });

        let Ok(mut inner) = self.inner.lock() else {
            return Err(SwarmError::Busy);
        };

        if inner.running {
            return Err(SwarmError::Busy);
        }

        inner.error = None;
        inner.told = 0;
        inner.running = true;
        inner.task = Some(task);

        Ok(())
    }

    /// Stops the loop.
    pub fn stop(&self) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };

        inner.running = false;

        if let Some(task) = inner.task.take() {
            task.abort();
        }
    }
}

/// Whether the loop is still wanted, read without holding the state of the formation.
fn is_running(inner: &Arc<std::sync::Mutex<Inner>>) -> bool {
    inner.lock().map(|guard| guard.running).unwrap_or(false)
}

/// Runs one tick for a session and sends what it decided.
async fn dispatch_for(
    inner: &Arc<std::sync::Mutex<Inner>>,
    session: &Session,
) -> Result<usize, SwarmError> {
    // The guard is dropped before anything is awaited: the loop must not hold the state of the
    // formation while it waits for the wire.
    let orders = {
        let mut guard = inner.lock().map_err(|_| SwarmError::Locked)?;

        tick_inner(&mut guard, &session.state())?
    };

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

    if let Ok(mut guard) = inner.lock() {
        guard.told = orders.len();
    }

    Ok(orders.len())
}

/// What one tick decides, with the filters of the model kept as they were left.
fn tick_inner(
    inner: &mut Inner,
    state: &SessionState,
) -> Result<Vec<(VehicleId, SwarmOrder)>, FormationError> {
    let Some(leader) = inner.settings.leader else {
        return Ok(Vec::new());
    };

    let Some(leader_state) = state.telemetries.get(&leader) else {
        return Ok(Vec::new());
    };

    if leader_state.is_empty() {
        return Ok(Vec::new());
    }

    // The slots are read through a copy so that the formation's filters can be used while the
    // settings are being walked.
    let slots = inner.settings.slots.clone();
    let mut orders = Vec::new();

    for slot in &slots {
        let vehicle = slot.vehicle;

        if vehicle == leader {
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
                    inner
                        .swarm
                        .plane_decision(leader_state, vehicle.system_id, telemetry)?
                {
                    orders.push((vehicle, SwarmOrder::Attitude(decision.attitude_command())));
                }
            }
            Some(_) => {
                if let Some(decision) =
                    inner
                        .swarm
                        .multicopter_decision(leader_state, vehicle.system_id, telemetry)?
                {
                    orders.push((vehicle, SwarmOrder::PositionTarget(decision)));
                }
            }
            None => {}
        }
    }

    Ok(orders)
}

/// Rebuilds the model from the settings that were just read: the slots, and who is a member.
fn rebuild(inner: &mut Inner) {
    let mut layout = Layout::new("swarm");

    for slot in &inner.settings.slots {
        layout.set_slot(slot.vehicle.system_id, slot.offset());
    }

    let mut swarm = Swarm::new(layout);

    for slot in &inner.settings.slots {
        swarm.add_member(slot.vehicle.system_id);
    }

    inner.swarm = swarm;
    inner.told = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpcraft_link::Vehicle;
    use mpcraft_settings::SwarmSlot;
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

        let service = SwarmService::new();
        service.apply(&settings(
            Some("1:1"),
            false,
            vec![SwarmSlot::with_offset("2:1", 0.0, 0.0, 0.0)],
        ));

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

        let service = SwarmService::new();
        service.apply(&settings(
            Some("1:1"),
            false,
            vec![SwarmSlot::with_offset("3:1", 0.0, 40.0, 5.0)],
        ));

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

        let service = SwarmService::new();
        service.apply(&settings(
            Some("1:1"),
            false,
            vec![SwarmSlot::with_offset("4:1", 0.0, 40.0, 0.0)],
        ));

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
        let mut state = empty_state();
        state
            .vehicles
            .push(recorded(VehicleId::new(2, 1), VehicleKind::ArduPlane));

        // The leader has a slot here as well, which is what upstream's loop would have sent a
        // command for if it did not skip the leader; the loop does skip it.
        let service = SwarmService::new();
        service.apply(&settings(
            Some("1:1"),
            false,
            vec![
                SwarmSlot::with_offset("1:1", 0.0, 0.0, 0.0),
                SwarmSlot::with_offset("2:1", 0.0, 60.0, 0.0),
                SwarmSlot::with_offset("5:1", 0.0, 80.0, 0.0),
            ],
        ));

        let state = report(
            &state,
            VehicleId::new(1, 1),
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
            VehicleId::new(2, 1),
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
        assert!(!state.telemetries.contains_key(&VehicleId::new(5, 1)));

        let vehicles: Vec<VehicleId> = service
            .tick(&state)
            .expect("the slots project")
            .iter()
            .map(|(vehicle, _)| *vehicle)
            .collect();

        assert_eq!(vehicles, vec![VehicleId::new(2, 1)]);
    }

    #[test]
    fn a_name_that_is_not_a_vehicle_is_left_out_and_said() {
        // The rule the keyring follows with a secret that is not hexadecimal: what cannot be read
        // is not guessed at, and the operator is told which entry it was.
        let service = SwarmService::new();
        service.apply(&settings(
            Some("1:1"),
            false,
            vec![
                SwarmSlot::new("2:1"),
                SwarmSlot::new("board two"),
                SwarmSlot::new("0:1"),
            ],
        ));

        let status = service.status();

        assert_eq!(
            status.slots.len(),
            1,
            "only the name that could be read is kept"
        );
        assert_eq!(status.slots[0].vehicle, "2:1");
        assert_eq!(
            status.refused,
            vec!["board two".to_owned(), "0:1".to_owned()],
            "the names that were left out are said"
        );
    }

    #[test]
    fn the_decision_to_run_is_kept_apart_from_the_places() {
        // Writing slots for a fleet does not start it: the file says two things, and the interface
        // has to be able to tell them apart.
        let service = SwarmService::new();
        service.apply(&settings(Some("1:1"), false, vec![SwarmSlot::new("2:1")]));

        let status = service.status();
        assert!(!status.enabled);
        assert!(
            !status.running,
            "a formation that was described is not a formation that runs"
        );

        service.apply(&settings(Some("1:1"), true, vec![SwarmSlot::new("2:1")]));
        assert!(service.status().enabled);
    }

    #[test]
    fn a_leader_that_was_never_named_decides_nothing() {
        let service = SwarmService::new();
        service.apply(&settings(
            None,
            false,
            vec![SwarmSlot::with_offset("2:1", 0.0, 60.0, 0.0)],
        ));

        let orders = service
            .tick(&empty_state())
            .expect("there is nothing to project");
        assert!(
            orders.is_empty(),
            "a formation with no leader follows nobody"
        );
    }

    /// A settings file written for the test, in the shape the file holds it.
    fn settings(
        leader: Option<&str>,
        enabled: bool,
        slots: Vec<mpcraft_settings::SwarmSlot>,
    ) -> mpcraft_settings::Settings {
        mpcraft_settings::Settings {
            connection_profiles: Vec::new(),
            favourite_parameters: Vec::new(),
            signing_keys: Vec::new(),
            plugins: Vec::new(),
            map_injection_folder: None,
            swarm_leader: leader.map(|name| name.to_owned()),
            swarm_enabled: enabled,
            swarm_slots: slots,
            mirrors: Vec::new(),
            pois: Vec::new(),
        }
    }

    /// A vehicle as the registry holds it, once its heartbeat has been seen.
    fn recorded(vehicle: VehicleId, kind: VehicleKind) -> Vehicle {
        let mut record = Vehicle::new(vehicle);
        record.kind = Some(kind);
        record
    }
}
