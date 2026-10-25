//! What the formation decides for each vehicle on each tick — the part of upstream's
//! `Swarm/Formation.cs` that can be read and tested without a live fleet.
//!
//! The slots and the places they put vehicles live in `mpcraft_geo::formation`; this module is
//! the closed loop on top of them: the heading error, the bank angle worked out from the turn,
//! the thrust demand, and what a multicopter is told when it is not flying like a plane. It is
//! kept as pure computation — snapshots in, a decision out — because the wire dispatch belongs
//! to the link, and a decision that does not need the wire can be tested.
//!
//! Three things are done differently here, on purpose:
//!
//! * Upstream allocates four PIDs per vehicle and calls `get_pid` on two of them; the roll and
//!   yaw PIDs are constructed and never read, because the roll is worked out from the geometry
//!   of the turn and the yaw error is passed through raw. MPCraft keeps only the two the loop
//!   actually uses.
//! * Upstream divides by zero twice in this loop — the sine rule when the vehicle is dead on
//!   the target, and the turn radius when the leader is not banked — and lets infinity decide.
//!   Both guards here give the same outcome infinity gives (no bank), but say so.
//! * Upstream compares the yaw difference to the dead band without wrapping it, so a vehicle at
//!   179° and a leader at -179° are 358° apart and get a correction they do not need. MPCraft
//!   wraps first, which is what the dead band means.

use std::collections::{BTreeMap, BTreeSet};

use mpcraft_geo::formation::{place, wrap_180, FormationError, Layout};

use crate::{GeoPoint, Telemetry, Velocity};

const YAW_DEADBAND_DEG: f64 = 3.0;
const BEARING_FLIP_DEG: f64 = 45.0;
const BANK_LIMIT_DEG: f64 = 85.0;
const ROLL_CONTRIBUTION_LIMIT_DEG: f64 = 20.0;
const NEAR_M: f64 = 30.0;
const FAR_M: f64 = 100.0;
const SINE_MIN_M: f64 = 40.0;
const LEADER_LEAD_M: f64 = 10.0;
const TRAILER_FACTOR: f64 = -0.25;
const GRAVITY_BANK: f64 = 9.8;
const GRAVITY_RADIUS: f64 = 9.80665;
const THRUST_MIN: f64 = 0.1;
const THRUST_MAX: f64 = 1.0;

/// The PID upstream uses in the swarm loop — ArduPilot's `AC_PID` shape.
///
/// The three terms see a *filtered* input, not the raw one: the filter is what stops a step in
/// the error from becoming a spike in the derivative, and only the filtered value drives the
/// integral.
#[derive(Debug, Clone, PartialEq)]
pub struct Pid {
    kp: f64,
    ki: f64,
    kd: f64,
    imax: f64,
    filt_hz: f64,
    dt: f64,
    input: f64,
    derivative: f64,
    integrator: f64,
    reset_filter: bool,
}

impl Pid {
    /// Creates a PID with upstream's constructor arguments.
    #[must_use]
    pub fn new(kp: f64, ki: f64, kd: f64, imax: f64, filt_hz: f64, dt: f64) -> Self {
        Self {
            kp,
            ki,
            kd,
            imax: imax.abs(),
            filt_hz: filt_hz.max(0.01),
            dt,
            input: 0.0,
            derivative: 0.0,
            integrator: 0.0,
            reset_filter: true,
        }
    }

    /// Feeds the filter, as upstream's `set_input_filter_all` does.
    ///
    /// An input that is not finite is left out rather than remembered: an infinite error would
    /// poison the integral and every later tick.
    pub fn set_input_filter_all(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }

        if self.reset_filter {
            self.reset_filter = false;
            self.input = value;
            self.derivative = 0.0;
            return;
        }

        let change = self.alpha() * (value - self.input);
        self.input += change;
        if self.dt > 0.0 {
            self.derivative = change / self.dt;
        }
    }

    /// The three terms added, as upstream's `get_pid`.
    pub fn get_pid(&mut self) -> f64 {
        let p = self.input * self.kp;

        if self.ki.abs() > f64::EPSILON && self.dt > 0.0 {
            self.integrator += self.input * self.ki * self.dt;
            self.integrator = self.integrator.clamp(-self.imax, self.imax);
        }

        p + self.integrator + self.kd * self.derivative
    }

    /// Empties the integrator, which upstream does when the vehicle is still far away so that a
    /// demand built up over the approach does not keep pushing after it arrives.
    pub fn reset_i(&mut self) {
        self.integrator = 0.0;
    }

    fn alpha(&self) -> f64 {
        if self.filt_hz <= 0.0 {
            return 1.0;
        }

        let rc = 1.0 / (std::f64::consts::TAU * self.filt_hz);
        self.dt / (self.dt + rc)
    }
}

/// What one plane is told this tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneDecision {
    /// The place the slot puts the vehicle.
    pub target: GeoPoint,
    /// Bank in degrees, positive to the right of the heading.
    pub roll_deg: f64,
    /// Pitch in degrees, from the altitude error through the PID.
    pub pitch_deg: f64,
    /// Heading error in degrees, wrapped to the range a heading error can be read in.
    pub yaw_error_deg: f64,
    /// Thrust demand, between 0.1 and 1.
    pub thrust: f64,
}

/// What one multicopter is told this tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MulticopterDecision {
    /// The place the slot puts the vehicle, in the frame upstream sends it in.
    pub target: GeoPoint,
    /// The leader's velocity, passed through so the vehicle keeps up without deriving its own.
    pub velocity: Option<Velocity>,
    /// The heading to point at — the leader's, but only when the difference is past the dead
    /// band; `None` says there is nothing to send this tick.
    pub yaw_deg: Option<f64>,
}

/// The PIDs one plane needs, which are the two upstream reads.
#[derive(Debug, Clone, PartialEq)]
struct PlanePids {
    pitch: Pid,
    thrust: Pid,
}

/// The formation: which vehicles are in it, where each sits, and what their filters remember.
///
/// Membership is kept apart from the slots, as `mpcraft_geo::formation` does: a vehicle can be
/// in the formation without having been given a place in it, and that is refused rather than
/// papered over.
#[derive(Debug, Clone, PartialEq)]
pub struct Swarm {
    layout: Layout,
    members: BTreeSet<u8>,
    pids: BTreeMap<u8, PlanePids>,
}

impl Swarm {
    /// Creates a formation from a layout.
    #[must_use]
    pub fn new(layout: Layout) -> Self {
        Self {
            layout,
            members: BTreeSet::new(),
            pids: BTreeMap::new(),
        }
    }

    /// Adds a vehicle to the formation.
    pub fn add_member(&mut self, system_id: u8) {
        self.members.insert(system_id);
    }

    /// The vehicles in the formation, in order.
    #[must_use]
    pub fn members(&self) -> &BTreeSet<u8> {
        &self.members
    }

    /// What one plane is told this tick.
    ///
    /// A vehicle whose position or heading is not known yet gets `None` rather than a decision
    /// worked out from unknown numbers; upstream guards with `lat == 0`, which is the same
    /// intent stated loosely.
    pub fn plane_decision(
        &mut self,
        leader: &Telemetry,
        vehicle_id: u8,
        vehicle: &Telemetry,
    ) -> Result<Option<PlaneDecision>, FormationError> {
        let slot = self
            .layout
            .slot(vehicle_id)
            .ok_or(FormationError::MissingSlot(vehicle_id))?;

        let (leader_position, leader_yaw) = match state_of(leader) {
            Some(state) => state,
            None => return Ok(None),
        };
        let (vehicle_position, vehicle_yaw) = match state_of(vehicle) {
            Some(state) => state,
            None => return Ok(None),
        };

        let target = place(to_geo(leader_position), leader_yaw, slot)?;

        // The two points upstream builds before anything can flip the distance: the leader point
        // ahead of the target and the trailer behind it, both measured along the leader's
        // heading.
        let mut distance = mpcraft_geo::distance_m(to_geo(vehicle_position), target);
        let bearing_to_target = mpcraft_geo::bearing_deg(to_geo(vehicle_position), target);

        let trailer = mpcraft_geo::offset(target, leader_yaw, distance.abs() * TRAILER_FACTOR);
        let leader_point = mpcraft_geo::offset(target, leader_yaw, LEADER_LEAD_M + distance);

        let yaw_error;

        if distance < FAR_M {
            let bearing_to_leader_point =
                mpcraft_geo::bearing_deg(to_geo(vehicle_position), leader_point);
            yaw_error = wrap_180(bearing_to_leader_point - vehicle_yaw);

            // Facing the wrong way round the target is what turns the distance negative: the
            // plane then flies in from behind rather than crashing into the formation from the
            // front.
            if wrap_180(bearing_to_target - bearing_to_leader_point).abs() > BEARING_FLIP_DEG {
                distance = -distance;
            }
        } else {
            let bearing_to_trailer = mpcraft_geo::bearing_deg(to_geo(vehicle_position), trailer);
            yaw_error = wrap_180(bearing_to_trailer - vehicle_yaw);
        }

        let pids = self
            .pids
            .entry(vehicle_id)
            .or_insert_with(PlanePids::upstream_gains);

        pids.pitch
            .set_input_filter_all(target.alt - vehicle_position.alt);
        let pitch = pids.pitch.get_pid();

        // The roll block recomputes the distance positive, as upstream does: the flip belongs to
        // the thrust demand, not to the geometry of the turn.
        let distance_positive = mpcraft_geo::distance_m(to_geo(vehicle_position), target);
        let bearing = if distance_positive < NEAR_M {
            mpcraft_geo::bearing_deg(to_geo(vehicle_position), leader_point)
        } else if distance_positive > FAR_M {
            mpcraft_geo::bearing_deg(to_geo(vehicle_position), trailer)
        } else {
            bearing_to_target
        };

        let bearing_delta = wrap_180(bearing - vehicle_yaw);
        let tangent90 = if bearing_delta > 0.0 { 90.0 } else { -90.0 };

        let mut roll = 0.0;

        if bearing_delta.abs() < BANK_LIMIT_DEG {
            let inside_angle = (tangent90 - bearing_delta).abs();
            let angle_center = 180.0 - inside_angle * 2.0;
            let sine_center = angle_center.to_radians().sin();

            // Upstream divides here without a guard and lets infinity decide; dead on the target
            // the bank works out to zero either way, so the guard says the same thing and stops
            // the infinity.
            if sine_center.abs() > f64::EPSILON {
                let sine1 = distance_positive.max(SINE_MIN_M) / sine_center;
                let turn_radius = sine1 * inside_angle.to_radians().sin();
                let leader_radius = turn_radius_of(leader).map(|r| r - slot.x);

                let averaged = match leader_radius {
                    Some(radius) => (turn_radius.abs() + radius.abs()) / 2.0,
                    None => turn_radius.abs(),
                };

                if averaged > 0.0 {
                    let ground_speed = vehicle.velocity.map_or(0.0, |v| v.ground_speed);
                    let bank =
                        ((ground_speed * ground_speed) / averaged / GRAVITY_BANK).to_degrees();
                    roll = if bearing_delta > 0.0 {
                        bank.abs()
                    } else {
                        -bank.abs()
                    };
                }
            }
        }

        roll += bearing_delta.clamp(-ROLL_CONTRIBUTION_LIMIT_DEG, ROLL_CONTRIBUTION_LIMIT_DEG);

        pids.thrust.set_input_filter_all(distance);
        if distance > 40.0 {
            pids.thrust.reset_i();
        }
        let thrust = pids.thrust.get_pid().clamp(THRUST_MIN, THRUST_MAX);

        Ok(Some(PlaneDecision {
            target: from_geo(target),
            roll_deg: roll,
            pitch_deg: pitch,
            yaw_error_deg: yaw_error,
            thrust,
        }))
    }

    /// What one multicopter is told this tick.
    pub fn multicopter_decision(
        &self,
        leader: &Telemetry,
        vehicle_id: u8,
        vehicle: &Telemetry,
    ) -> Result<Option<MulticopterDecision>, FormationError> {
        let slot = self
            .layout
            .slot(vehicle_id)
            .ok_or(FormationError::MissingSlot(vehicle_id))?;

        let (leader_position, leader_yaw) = match state_of(leader) {
            Some(state) => state,
            None => return Ok(None),
        };
        let (_, vehicle_yaw) = match state_of(vehicle) {
            Some(state) => state,
            None => return Ok(None),
        };

        let target = place(to_geo(leader_position), leader_yaw, slot)?;

        // The dead band is compared against the wrapped difference: upstream compares the raw
        // one, which makes 179° and -179° 358° apart.
        let yaw_deg = if wrap_180(vehicle_yaw - leader_yaw).abs() > YAW_DEADBAND_DEG {
            Some(leader_yaw)
        } else {
            None
        };

        Ok(Some(MulticopterDecision {
            target: from_geo(target),
            velocity: leader.velocity,
            yaw_deg,
        }))
    }
}

impl PlanePids {
    /// The gains upstream constructs for each plane — the two of the four that are read.
    fn upstream_gains() -> Self {
        Self {
            pitch: Pid::new(1.0, 0.03, 0.02, 10.0, 20.0, 0.1),
            thrust: Pid::new(0.01, 0.001, 0.0, 0.5, 20.0, 0.1),
        }
    }
}

/// The state a decision cannot be worked out without: where the vehicle is and which way it is
/// pointed. Unknown state gives `None` rather than a target built from unknown numbers.
fn state_of(vehicle: &Telemetry) -> Option<(GeoPoint, f64)> {
    let position = vehicle.position?;
    let yaw = vehicle.attitude?.yaw;
    Some((position, yaw))
}

fn turn_radius_of(leader: &Telemetry) -> Option<f64> {
    let ground_speed = leader.velocity?.ground_speed;
    if ground_speed <= 1.0 {
        return None;
    }

    let roll = leader.attitude?.roll;
    let tangent = roll.to_radians().tan();
    if tangent.abs() < f64::EPSILON {
        return None;
    }

    Some((ground_speed * ground_speed) / (GRAVITY_RADIUS * tangent))
}

fn to_geo(p: GeoPoint) -> mpcraft_geo::GeoPoint {
    mpcraft_geo::GeoPoint::with_alt(p.lat, p.lng, p.alt)
}

fn from_geo(p: mpcraft_geo::GeoPoint) -> GeoPoint {
    GeoPoint {
        lat: p.lat,
        lng: p.lng,
        alt: p.alt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Attitude;
    use mpcraft_geo::formation::Offset;

    fn state(lat: f64, lng: f64, alt: f64, yaw: f64, roll: f64, ground_speed: f64) -> Telemetry {
        Telemetry {
            position: Some(GeoPoint { lat, lng, alt }),
            attitude: Some(Attitude {
                roll,
                pitch: 0.0,
                yaw,
            }),
            velocity: Some(Velocity {
                north: 0.0,
                east: 0.0,
                down: 0.0,
                ground_speed,
            }),
            ..Telemetry::default()
        }
    }

    // The leader on the central meridian of its zone, where a grid direction is a true one.
    const LEADER: f64 = 3.0; // central meridian of zone 31

    #[test]
    fn the_pid_follows_upstream_step_by_step() {
        let mut pid = Pid::new(1.0, 0.03, 0.02, 10.0, 20.0, 0.1);

        // First value received resets the filter and becomes the input as it is.
        pid.set_input_filter_all(10.0);
        assert!((pid.get_pid() - 10.03).abs() < 1e-9, "{}", pid.get_pid());

        // The same value again only grows the integral.
        pid.set_input_filter_all(10.0);
        assert!((pid.get_pid() - 10.06).abs() < 1e-9);

        // A step of ten moves the filtered input by alpha — 0.926294 at 20 Hz with dt 0.1 —
        // and the derivative sees the move, not the step.
        pid.set_input_filter_all(20.0);
        assert!(
            (pid.get_pid() - 21.233_313).abs() < 1e-4,
            "pid was {}",
            pid.get_pid()
        );
    }

    #[test]
    fn the_integral_clamps_and_can_be_emptied() {
        let mut pid = Pid::new(0.0, 1.0, 0.0, 0.5, 20.0, 0.1);

        pid.set_input_filter_all(10.0);
        assert!((pid.get_pid() - 0.5).abs() < 1e-9); // 1.0 wanted, 0.5 allowed

        pid.reset_i();
        assert_eq!(pid.integrator, 0.0);
    }

    #[test]
    fn an_input_that_is_not_finite_is_left_out() {
        let mut pid = Pid::new(1.0, 0.0, 0.0, 0.0, 20.0, 0.1);

        pid.set_input_filter_all(f64::INFINITY);
        // The filter has not been primed yet, so the first finite value still resets it.
        pid.set_input_filter_all(5.0);
        assert!((pid.get_pid() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn the_turn_radius_of_the_leader_is_worked_out_from_its_bank() {
        let leader = state(0.0, LEADER, 100.0, 0.0, 30.0, 20.0);

        // 400 / (9.80665 · tan 30°) = 70.648 m, as upstream's CurrentState computes it.
        assert!(
            (turn_radius_of(&leader).unwrap_or(0.0) - 70.648_0).abs() < 1e-3,
            "turn radius was {}",
            turn_radius_of(&leader).unwrap_or(0.0)
        );

        // A leader that is not moving or not banked has no turn to share.
        let slow = state(0.0, LEADER, 100.0, 0.0, 30.0, 0.5);
        assert!(turn_radius_of(&slow).is_none());
        let straight = state(0.0, LEADER, 100.0, 0.0, 0.0, 20.0);
        assert!(turn_radius_of(&straight).is_none());
    }

    #[test]
    fn a_plane_facing_across_the_turn_gets_only_the_constrained_yaw_term() {
        let mut swarm = Swarm::new(layout(20.0, 0.0, 0.0));
        swarm.add_member(2);

        let leader = state(0.0, LEADER, 100.0, 0.0, 30.0, 20.0);
        let target = mpcraft_geo::place(
            mpcraft_geo::GeoPoint::with_alt(0.0, LEADER, 100.0),
            0.0,
            Offset::new(20.0, 0.0, 0.0),
        )
        .expect("on the equator");
        // Fifty metres to the west of the target, on the equator: the bearing back is due east.
        let vehicle = mpcraft_geo::offset(target, 270.0, 50.0);
        let vehicle_state = state(vehicle.lat, vehicle.lng, 90.0, 0.0, 0.0, 20.0);

        let decision = swarm
            .plane_decision(&leader, 2, &vehicle_state)
            .expect("the slot exists")
            .expect("both known");

        // The heading error comes from the point ahead of the target: the bearing from the
        // vehicle is atan2(50 east, 60 north) = 39.806°, and the two bearings are 50.194° apart,
        // which is past the flip limit — the distance goes negative and the plane is flown in
        // from behind.
        assert!(
            (decision.yaw_error_deg - 39.805_57).abs() < 0.02,
            "yaw error was {}",
            decision.yaw_error_deg
        );

        // A ninety-degree difference is past the bank limit, so only the constrained term is
        // left: twenty degrees.
        assert!(
            (decision.roll_deg - 20.0).abs() < 1e-9,
            "roll was {}",
            decision.roll_deg
        );

        // The first filtered inputs are the altitude error and the flipped distance: the thrust
        // demand is negative before the clamp, so it sits at its floor.
        assert!((decision.pitch_deg - 10.03).abs() < 1e-9);
        assert!(
            (decision.thrust - 0.1).abs() < 1e-9,
            "thrust was {}",
            decision.thrust
        );

        // The target is the slot put on the grid: twenty metres of grid east reads as 20.008 m
        // on the ellipsoid at the central meridian, where the scale factor is k0; the spherical
        // great-circle model says 19.986 m because it assumes the mean radius rather than the
        // equatorial one, and the two agree only to that difference — which is what a formation
        // of ten metres lives inside.
        let (distance, bearing) = mpcraft_geo::great_circle(
            mpcraft_geo::GeoPoint::with_alt(0.0, LEADER, 100.0),
            mpcraft_geo::GeoPoint::with_alt(
                decision.target.lat,
                decision.target.lng,
                decision.target.alt,
            ),
        );
        assert!(
            (distance - 19.985_6).abs() < 0.02,
            "target was {distance} m away"
        );
        assert!((bearing - 90.0).abs() < 0.01, "target was at {bearing}");
        assert_eq!(decision.target.alt, 100.0);
    }

    #[test]
    fn a_plane_inside_the_bank_limit_gets_the_turn_worked_out_by_the_sine_rule() {
        let mut swarm = Swarm::new(layout(20.0, 0.0, 0.0));
        swarm.add_member(2);

        let leader = state(0.0, LEADER, 100.0, 0.0, 30.0, 20.0);
        let target = mpcraft_geo::place(
            mpcraft_geo::GeoPoint::with_alt(0.0, LEADER, 100.0),
            0.0,
            Offset::new(20.0, 0.0, 0.0),
        )
        .expect("on the equator");
        let vehicle = mpcraft_geo::offset(target, 270.0, 50.0);
        // The same place, pointed sixty degrees rather than north: the bearing difference is
        // thirty, inside the bank limit.
        let vehicle_state = state(vehicle.lat, vehicle.lng, 90.0, 60.0, 0.0, 20.0);

        let decision = swarm
            .plane_decision(&leader, 2, &vehicle_state)
            .expect("the slot exists")
            .expect("both known");

        assert!(
            (decision.yaw_error_deg - (39.805_57 - 60.0)).abs() < 0.02,
            "yaw error was {}",
            decision.yaw_error_deg
        );

        // inside 60°, centre 60°, sine1 = 50 / sin 60° = 57.735, radius = 50.0; averaged with
        // the leader's own turn (70.648) less the slot's offset, (50 + 50.648) / 2 = 50.324;
        // bank = (400 / 50.324) / 9.8 = 0.81107 rad = 46.469°, plus the constrained term of 20.
        assert!(
            (decision.roll_deg - 66.469_4).abs() < 0.01,
            "roll was {}",
            decision.roll_deg
        );

        // The same flip as before: the bearing difference is past forty-five degrees, so the
        // distance is negative and the thrust sits at its floor.
        assert!(
            (decision.thrust - 0.1).abs() < 1e-9,
            "thrust was {}",
            decision.thrust
        );
    }

    #[test]
    fn a_plane_facing_the_wrong_way_round_the_target_is_flown_in_from_behind() {
        let mut swarm = Swarm::new(layout(20.0, 0.0, 0.0));
        swarm.add_member(2);

        let leader = state(0.0, LEADER, 100.0, 0.0, 30.0, 20.0);
        let target = mpcraft_geo::place(
            mpcraft_geo::GeoPoint::with_alt(0.0, LEADER, 100.0),
            0.0,
            Offset::new(20.0, 0.0, 0.0),
        )
        .expect("on the equator");
        // Fifty metres north of the target and pointed south: the bearing to the point ahead of
        // the target is zero, one hundred and eighty degrees away from the bearing to the
        // target, which is past the flip limit.
        let vehicle = mpcraft_geo::offset(target, 0.0, 50.0);
        let vehicle_state = state(vehicle.lat, vehicle.lng, 100.0, 180.0, 0.0, 20.0);

        let decision = swarm
            .plane_decision(&leader, 2, &vehicle_state)
            .expect("the slot exists")
            .expect("both known");

        assert!((decision.yaw_error_deg - (-180.0)).abs() < 1e-9);

        // The distance went negative, so the thrust term is negative before the clamp: the
        // demand is the floor, not a push towards the formation.
        assert!(
            (decision.thrust - 0.1).abs() < 1e-9,
            "thrust was {}",
            decision.thrust
        );

        // Dead on the bearing line the sine rule has nothing to divide by; the bank stays at
        // zero, which is what upstream's infinity works out to as well.
        assert!(
            (decision.roll_deg - 0.0).abs() < 1e-9,
            "roll was {}",
            decision.roll_deg
        );
        assert!((decision.pitch_deg - 0.0).abs() < 1e-9);
    }

    #[test]
    fn a_multicopter_keeps_the_leader_and_only_turns_past_the_dead_band() {
        let mut swarm = Swarm::new(layout(0.0, 0.0, 5.0));
        swarm.add_member(2);

        let leader = state(0.0, LEADER, 100.0, 10.0, 0.0, 15.0);
        let vehicle = state(0.0, LEADER + 0.001, 105.0, 14.0, 0.0, 15.0);

        let decision = swarm
            .multicopter_decision(&leader, 2, &vehicle)
            .expect("the slot exists")
            .expect("both known");

        assert!((decision.target.alt - 105.0).abs() < 1e-9);
        assert_eq!(decision.yaw_deg, Some(10.0));
        assert_eq!(decision.velocity, leader.velocity);

        // Exactly on the band is not past it.
        let on_band = state(0.0, LEADER, 100.0, 13.0, 0.0, 15.0);
        let decision = swarm
            .multicopter_decision(&leader, 2, &on_band)
            .expect("slot")
            .expect("known");
        assert_eq!(decision.yaw_deg, None);

        // And a difference that wraps — 179 against -179 is two degrees round, not three
        // hundred and fifty-eight — stays inside the band, which upstream's raw comparison does
        // not do.
        let wrapped = state(0.0, LEADER, 100.0, 179.0, 0.0, 15.0);
        let turned_leader = state(0.0, LEADER, 100.0, -179.0, 0.0, 15.0);
        let decision = swarm
            .multicopter_decision(&turned_leader, 2, &wrapped)
            .expect("slot")
            .expect("known");
        assert_eq!(decision.yaw_deg, None);
    }

    #[test]
    fn a_vehicle_the_formation_has_not_been_told_about_is_refused_by_name() {
        let mut swarm = Swarm::new(Layout::new("line"));
        swarm.add_member(7);

        let leader = state(0.0, LEADER, 100.0, 0.0, 0.0, 20.0);
        let vehicle = state(0.0, LEADER + 0.01, 100.0, 0.0, 0.0, 20.0);

        assert_eq!(
            swarm.plane_decision(&leader, 7, &vehicle),
            Err(FormationError::MissingSlot(7))
        );
    }

    #[test]
    fn a_vehicle_that_has_not_reported_yet_gets_nothing() {
        let mut swarm = Swarm::new(layout(10.0, 0.0, 0.0));
        swarm.add_member(2);

        let leader = state(0.0, LEADER, 100.0, 0.0, 0.0, 20.0);
        let unknown = Telemetry {
            position: Some(GeoPoint {
                lat: 0.0,
                lng: LEADER,
                alt: 0.0,
            }),
            ..Telemetry::default()
        };

        assert_eq!(swarm.plane_decision(&leader, 2, &unknown), Ok(None));
    }

    fn layout(x: f64, y: f64, z: f64) -> Layout {
        let mut layout = Layout::new("test");
        layout.set_slot(2, Offset::new(x, y, z));
        layout
    }
}
