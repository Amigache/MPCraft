//! The warnings about a plan.
//!
//! # What this is
//!
//! Upstream checks the plan before it flies it, and the only check it makes is that a
//! waypoint does not have a zero altitude: the vehicle would fly to the ground. It asks
//! the operator to confirm, and the operator can refuse.
//!
//! The rule is the autopilot's, not the interface's, so it lives here: the interface shows
//! what this says, and does not know why.
//!
//! # What it does not check
//!
//! Upstream does not check whether the plan can be flown, whether the altitudes are
//! reachable, or whether the commands make sense together. Neither does this. A warning
//! that cries wolf is worse than no warning at all.

use mpcraft_mavlink::MavCmd;

use crate::Plan;

/// What is wrong with an item of a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningReason {
    /// The altitude is zero, and the vehicle would fly to the ground.
    ///
    /// This is upstream's check, and the reason it exists.
    ZeroAltitude,
}

/// A warning about one item of a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Warning {
    /// The position of the item in the plan.
    pub seq: usize,
    /// What is wrong with it.
    pub reason: WarningReason,
}

/// The commands whose altitude the vehicle flies to.
///
/// A command that does not fly to a height — a delay, a change of speed — is not warned
/// about, whatever its altitude says.
#[must_use]
pub fn uses_altitude(command: MavCmd) -> bool {
    matches!(
        command,
        MavCmd::MAV_CMD_NAV_WAYPOINT
            | MavCmd::MAV_CMD_NAV_LOITER_TIME
            | MavCmd::MAV_CMD_NAV_LOITER_UNLIM
            | MavCmd::MAV_CMD_NAV_LOITER_TURNS
            | MavCmd::MAV_CMD_NAV_LOITER_TO_ALT
    )
}

/// The warnings about a plan.
///
/// An empty list means the plan has nothing to say about itself.
#[must_use]
pub fn validate(plan: &Plan) -> Vec<Warning> {
    plan.items
        .iter()
        .enumerate()
        .filter_map(|(seq, item)| {
            if uses_altitude(item.command) && item.z == 0.0 {
                Some(Warning {
                    seq,
                    reason: WarningReason::ZeroAltitude,
                })
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MissionItem;

    fn waypoint(altitude: f64) -> MissionItem {
        let mut item = MissionItem::new(MavCmd::MAV_CMD_NAV_WAYPOINT);
        item.z = altitude;
        item
    }

    #[test]
    fn a_waypoint_with_no_altitude_is_warned_about() {
        let mut plan = Plan::new(crate::MissionType::Mission);
        plan.push(waypoint(0.0));

        let warnings = validate(&plan);

        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].seq, 0);
        assert_eq!(warnings[0].reason, WarningReason::ZeroAltitude);
    }

    #[test]
    fn a_waypoint_with_an_altitude_is_not() {
        let mut plan = Plan::new(crate::MissionType::Mission);
        plan.push(waypoint(100.0));

        assert!(validate(&plan).is_empty());
    }

    #[test]
    fn a_command_that_does_not_fly_to_a_height_is_not_warned_about() {
        // A delay has no altitude to speak of, and warning about it would be noise.
        let mut plan = Plan::new(crate::MissionType::Mission);
        plan.push(MissionItem::new(MavCmd::MAV_CMD_CONDITION_DELAY));

        assert!(validate(&plan).is_empty());
    }

    #[test]
    fn the_warning_names_the_item_it_is_about() {
        let mut plan = Plan::new(crate::MissionType::Mission);
        plan.push(waypoint(100.0));
        plan.push(waypoint(0.0));
        plan.push(waypoint(0.0));

        let warnings = validate(&plan);

        assert_eq!(warnings.len(), 2);
        assert_eq!(warnings[0].seq, 1);
        assert_eq!(warnings[1].seq, 2);
    }
}
