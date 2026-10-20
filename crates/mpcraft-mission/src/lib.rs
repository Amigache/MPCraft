//! Mission items, plans and the MAV_CMD metadata.
//!
//! # The model
//!
//! A plan is an ordered list of [`MissionItem`]s. Each item names a `MAV_CMD`, the frame its
//! coordinates are expressed in, and seven parameter slots whose meaning depends on the
//! command.
//!
//! The model is the *domain* one: it carries a stable identity and a display name, which the
//! editor needs and the wire does not. The conversions to and from the MAVLink messages live
//! in [`wire`].
//!
//! # Round-trip fidelity
//!
//! A plan downloaded from a vehicle and uploaded back must be byte-identical on the wire.
//! That is enforced by the round-trip property tests over the whole `MAV_CMD` catalogue.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::sync::atomic::{AtomicU64, Ordering};

use mpcraft_mavlink::MavCmd;
use serde::{Deserialize, Serialize};

pub mod mavcmd;
pub mod wire;

pub use wire::MissionType;

/// A stable identity for a mission item.
///
/// Items need identity that survives edits, reordering and re-download: the editor uses it as
/// a key, and it tracks which row is selected while the list is being mutated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MissionItemId(u64);

impl MissionItemId {
    /// Creates an id from a raw counter value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The raw counter value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// The next identity in the sequence.
    ///
    /// The identity is only meaningful within one session, so a plain counter is enough.
    #[must_use]
    pub fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

/// One item of a plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionItem {
    /// Stable identity of this item.
    pub id: MissionItemId,
    /// The command to execute.
    pub command: MavCmd,
    /// The coordinate frame the position fields are expressed in.
    pub frame: u32,
    /// The four generic parameters.
    pub params: [f32; 4],
    /// The X field — latitude for a location command.
    pub x: f64,
    /// The Y field — longitude for a location command.
    pub y: f64,
    /// The Z field — altitude for a location command.
    pub z: f64,
    /// Whether this is the item the vehicle is executing.
    pub current: bool,
    /// Whether the vehicle continues to the next item automatically.
    pub autocontinue: bool,
    /// An optional user-supplied label.
    pub display_name: Option<String>,
}

impl Default for MissionItem {
    fn default() -> Self {
        Self {
            id: MissionItemId::next(),
            command: MavCmd::MAV_CMD_NAV_WAYPOINT,
            frame: mavcmd::DEFAULT_FRAME,
            params: [0.0; 4],
            x: 0.0,
            y: 0.0,
            z: 0.0,
            current: false,
            autocontinue: true,
            display_name: None,
        }
    }
}

impl MissionItem {
    /// Creates an item.
    #[must_use]
    pub fn new(command: MavCmd) -> Self {
        Self {
            command,
            ..Self::default()
        }
    }

    /// Whether the command carries a geographic position.
    #[must_use]
    pub fn is_location_command(&self) -> bool {
        mavcmd::is_location_command(self.command)
    }
}

/// A plan: an ordered list of items.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    /// The kind of plan.
    pub mission_type: MissionType,
    /// The items, in order.
    pub items: Vec<MissionItem>,
}

impl Plan {
    /// Creates an empty plan.
    #[must_use]
    pub fn new(mission_type: MissionType) -> Self {
        Self {
            mission_type,
            items: Vec::new(),
        }
    }

    /// The number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the plan has no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Appends an item.
    pub fn push(&mut self, item: MissionItem) {
        self.items.push(item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_item_has_a_distinct_identity() {
        let first = MissionItem::new(MavCmd::MAV_CMD_NAV_WAYPOINT);
        let second = MissionItem::new(MavCmd::MAV_CMD_NAV_WAYPOINT);
        assert_ne!(first.id, second.id);
    }

    #[test]
    fn a_plan_keeps_its_items_in_order() {
        let mut plan = Plan::new(MissionType::Mission);
        assert!(plan.is_empty());

        plan.push(MissionItem::new(MavCmd::MAV_CMD_NAV_TAKEOFF));
        plan.push(MissionItem::new(MavCmd::MAV_CMD_NAV_WAYPOINT));

        assert_eq!(plan.len(), 2);
        assert_eq!(plan.items[0].command, MavCmd::MAV_CMD_NAV_TAKEOFF);
        assert_eq!(plan.items[1].command, MavCmd::MAV_CMD_NAV_WAYPOINT);
    }

    #[test]
    fn a_waypoint_carries_a_position_and_a_delay_does_not() {
        let waypoint = MissionItem::new(MavCmd::MAV_CMD_NAV_WAYPOINT);
        assert!(waypoint.is_location_command());

        let delay = MissionItem::new(MavCmd::MAV_CMD_CONDITION_DELAY);
        assert!(!delay.is_location_command());
    }
}
