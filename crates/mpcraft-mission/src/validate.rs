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

use crate::{mavcmd, MissionItem, Plan};

/// What is wrong with an item of a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningReason {
    /// The altitude is zero, and the vehicle would fly to the ground.
    ///
    /// This is upstream's check, and the reason it exists.
    ZeroAltitude,
    /// The fence has fewer than three vertices, and a polygon needs three.
    ///
    /// The vehicle cannot build a fence out of two points, and it says so when the
    /// plan is uploaded. The warning is about the whole fence, not about one of its
    /// points.
    TooFewFenceVertices,
}

/// A warning about a plan, or about one of its items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Warning {
    /// The position of the item in the plan, when the warning is about one.
    ///
    /// A warning about the whole plan has no item: a fence with too few vertices
    /// is not the fault of any one of them.
    pub seq: Option<usize>,
    /// What is wrong.
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
/// The vertices of the polygon of a fence.
///
/// A fence is a polygon, and its vertices are the points that are included in it. The
/// points that are excluded from it, and the circles, are not vertices: they are holes
/// and other areas.
#[must_use]
pub fn fence_vertices(plan: &Plan) -> Vec<&MissionItem> {
    plan.items
        .iter()
        .filter(|item| item.command == mavcmd::CMD_FENCE_VERTEX_INCLUSION)
        .collect()
}

/// The warnings about a plan.
///
/// An empty list means the plan has nothing to say about itself.
#[must_use]
pub fn validate(plan: &Plan) -> Vec<Warning> {
    let mut warnings: Vec<Warning> = plan
        .items
        .iter()
        .enumerate()
        .filter_map(|(seq, item)| {
            if uses_altitude(item.command) && item.z == 0.0 {
                Some(Warning {
                    seq: Some(seq),
                    reason: WarningReason::ZeroAltitude,
                })
            } else {
                None
            }
        })
        .collect();

    // A polygon needs three points. A fence with fewer cannot be built by the vehicle,
    // and the operator is told before the plan is flown, not after.
    if plan.mission_type == crate::MissionType::Fence && fence_vertices(plan).len() < 3 {
        warnings.push(Warning {
            seq: None,
            reason: WarningReason::TooFewFenceVertices,
        });
    }

    warnings
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
        assert_eq!(warnings[0].seq, Some(0));
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

    /// A vertex of the polygon of a fence.
    fn vertex(command: MavCmd) -> MissionItem {
        let mut item = MissionItem::new(command);
        item.x = -35.0;
        item.y = 149.0;
        item
    }

    #[test]
    fn a_fence_with_three_vertices_says_nothing() {
        let mut plan = Plan::new(crate::MissionType::Fence);
        plan.push(vertex(mavcmd::CMD_FENCE_VERTEX_INCLUSION));
        plan.push(vertex(mavcmd::CMD_FENCE_VERTEX_INCLUSION));
        plan.push(vertex(mavcmd::CMD_FENCE_VERTEX_INCLUSION));

        assert!(validate(&plan).is_empty());
    }

    #[test]
    fn a_fence_with_fewer_than_three_vertices_is_warned_about() {
        let mut plan = Plan::new(crate::MissionType::Fence);
        plan.push(vertex(mavcmd::CMD_FENCE_VERTEX_INCLUSION));
        plan.push(vertex(mavcmd::CMD_FENCE_VERTEX_INCLUSION));

        let warnings = validate(&plan);

        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].reason, WarningReason::TooFewFenceVertices);
        // The warning is about the whole fence, and not about one of its points.
        assert_eq!(warnings[0].seq, None);
    }

    #[test]
    fn the_points_that_are_excluded_do_not_count_as_vertices() {
        let mut plan = Plan::new(crate::MissionType::Fence);
        plan.push(vertex(mavcmd::CMD_FENCE_VERTEX_INCLUSION));
        plan.push(vertex(mavcmd::CMD_FENCE_VERTEX_INCLUSION));
        plan.push(vertex(mavcmd::CMD_FENCE_VERTEX_EXCLUSION));

        let warnings = validate(&plan);

        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].reason, WarningReason::TooFewFenceVertices);
    }

    #[test]
    fn a_mission_with_two_waypoints_is_not_a_fence() {
        // The rule is about fences: a mission of two waypoints is a short mission, not a
        // broken fence.
        let mut plan = Plan::new(crate::MissionType::Mission);
        plan.push(waypoint(100.0));
        plan.push(waypoint(100.0));

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
        assert_eq!(warnings[0].seq, Some(1));
        assert_eq!(warnings[1].seq, Some(2));
    }
}
