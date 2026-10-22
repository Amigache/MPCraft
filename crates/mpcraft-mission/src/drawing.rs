//! A plan brought in from a drawing.
//!
//! Mission Planner reads a shapefile — or a drawing of a CAD program — and puts its
//! points in the plan as waypoints, so that a route that was drawn on a map can be flown
//! without drawing it again. A drawing carries places and nothing else, so the items are
//! waypoints, the frame is the one the ground station plans in, and the altitude is the
//! one a new waypoint is given.
//!
//! The places come from [`mpcraft_geo::drawing`], which is where a drawing's own
//! projection is turned into degrees.

use mpcraft_mavlink::MavCmd;

use crate::{MissionItem, MissionType, Plan};

/// The altitude an imported waypoint is given, in metres above the home position.
///
/// A drawing says where a place is on the ground, not how high above it to fly, so the
/// items start at a height the operator is expected to check before flying them.
pub const IMPORTED_ALTITUDE: f64 = 100.0;

/// A plan of waypoints, one for each place of the drawing.
///
/// The first item is the one the plan starts with, which is the convention the model
/// keeps for the home of the vehicle.
#[must_use]
pub fn plan_from_places(places: &[(f64, f64)]) -> Plan {
    let mut plan = Plan::new(MissionType::Mission);

    for (lat, lng) in places {
        plan.push(MissionItem {
            command: MavCmd::MAV_CMD_NAV_WAYPOINT,
            x: *lat,
            y: *lng,
            z: IMPORTED_ALTITUDE,
            ..MissionItem::default()
        });
    }

    if let Some(first) = plan.items.first_mut() {
        first.current = true;
    }

    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_place_becomes_a_waypoint() {
        let plan = plan_from_places(&[(-35.28, 149.13), (-35.29, 149.14)]);

        assert_eq!(plan.items.len(), 2);
        assert!((plan.items[0].x + 35.28).abs() < 1e-9);
        assert!((plan.items[0].y - 149.13).abs() < 1e-9);
        assert!((plan.items[1].x + 35.29).abs() < 1e-9);
        assert!(plan.items.iter().all(|item| item.z == IMPORTED_ALTITUDE));
        assert_eq!(plan.items[0].command, MavCmd::MAV_CMD_NAV_WAYPOINT);
    }

    #[test]
    fn the_plan_starts_with_its_first_item() {
        let plan = plan_from_places(&[(-35.28, 149.13)]);

        assert!(plan.items[0].current);
    }

    #[test]
    fn the_items_have_identities_of_their_own() {
        let plan = plan_from_places(&[(-35.28, 149.13), (-35.29, 149.14)]);

        assert_ne!(plan.items[0].id, plan.items[1].id);
    }
}
