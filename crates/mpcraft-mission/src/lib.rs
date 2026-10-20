//! Mission model for MPCraft.
//!
//! A mission is an ordered list of [`MissionItem`]s. Each item names a `MAV_CMD`, the frame its
//! coordinates are expressed in, and seven parameter slots whose meaning depends on the
//! command.
//!
//! # The contract
//!
//! **Round-trip fidelity.** A mission downloaded from a vehicle and uploaded back must be
//! byte-identical on the wire. This is enforced by property tests over the whole `MAV_CMD`
//! catalogue — see `tests/roundtrip.rs`.
//!
//! # Parameter slots
//!
//! Every `MAV_CMD` has seven meaningful slots: P1, P2, P3, P4, X, Y, Z. The
//! [`mavcmd`](crate::mavcmd) module states, per command, which slots are latitude, longitude,
//! altitude, speed, distance or angle — and the frontend generates its editors from that
//! metadata rather than hardcoding per-command logic.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};

pub mod mavcmd;

/// A stable identifier for a mission item.
///
/// Items need identity that survives edits, reordering and re-download: the frontend uses it
/// as a React key, and the editor uses it to track which row is selected while the list is
/// being mutated.
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
}

/// The role of one of a mission item's seven parameter slots.
///
/// This mirrors the vocabulary used by Mission Planner's `mavcmd.xml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ParamRole {
    /// The slot is not used by this command.
    Empty,
    /// The slot holds a latitude in degrees.
    Latitude,
    /// The slot holds a longitude in degrees.
    Longitude,
    /// The slot holds an altitude in metres.
    Altitude,
    /// The slot holds a speed in metres per second.
    Speed,
    /// The slot holds a distance in metres.
    Distance,
    /// The slot holds an angle in degrees.
    Angle,
    /// The slot holds a value whose meaning is specific to the command.
    Other,
}

/// The unit of a parameter slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ParamUnit {
    /// The value is unitless.
    None,
    /// Degrees.
    Degrees,
    /// Metres.
    Metres,
    /// Metres per second.
    MetresPerSecond,
    /// Metres per second squared.
    MetresPerSecondSquared,
}

/// A single item in a mission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionItem {
    /// Stable identity of this item.
    pub id: MissionItemId,
    /// The MAVLink command to execute.
    pub command: u32,
    /// The coordinate frame the position fields are expressed in.
    pub frame: u32,
    /// The four generic parameters P1 to P4.
    pub params: [f64; 4],
    /// The X slot — latitude for location commands.
    pub x: f64,
    /// The Y slot — longitude for location commands.
    pub y: f64,
    /// The Z slot — altitude for location commands.
    pub z: f64,
    /// The mission type this item belongs to.
    pub mission_type: u32,
    /// Whether the vehicle should continue to the next item automatically.
    pub autocontinue: bool,
    /// Whether this is the item the vehicle is currently executing.
    pub current: bool,
    /// An optional user-supplied label.
    pub display_name: Option<String>,
}

impl MissionItem {
    /// Creates an item with default parameters.
    #[must_use]
    pub fn new(id: MissionItemId, command: u32) -> Self {
        Self {
            id,
            command,
            frame: crate::mavcmd::DEFAULT_FRAME,
            params: [0.0; 4],
            x: 0.0,
            y: 0.0,
            z: 0.0,
            mission_type: crate::mavcmd::DEFAULT_MISSION_TYPE,
            autocontinue: true,
            current: false,
            display_name: None,
        }
    }

    /// Whether this command carries a geographic position.
    ///
    /// Location commands scale their coordinates when the integer mission encoding is used;
    /// other commands do not. Getting this wrong corrupts every non-location command in a
    /// mission uploaded with `MISSION_ITEM_INT`.
    #[must_use]
    pub fn is_location_command(&self) -> bool {
        crate::mavcmd::is_location_command(self.command)
    }
}

/// A complete mission: an ordered list of items.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Mission {
    /// The items, in execution order.
    pub items: Vec<MissionItem>,
    /// The type of mission this is — `MISSION`, `FENCE` or `RALLY`.
    pub mission_type: u32,
    /// The total item count reported by the vehicle.
    pub total: u16,
}

impl Mission {
    /// Creates an empty mission of the given type.
    #[must_use]
    pub fn new(mission_type: u32) -> Self {
        Self {
            items: Vec::new(),
            mission_type,
            total: 0,
        }
    }

    /// The number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the mission has no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Appends an item, renumbering the total.
    pub fn push(&mut self, mut item: MissionItem) {
        item.mission_type = self.mission_type;
        self.items.push(item);
        self.total = self.items.len() as u16;
    }

    /// Removes the item at `index`, renumbering the total.
    ///
    /// Returns the removed item, or `None` when the index was out of range.
    pub fn remove(&mut self, index: usize) -> Option<MissionItem> {
        if index >= self.items.len() {
            return None;
        }
        let item = self.items.remove(index);
        self.total = self.items.len() as u16;
        Some(item)
    }

    /// The index of the item currently being executed.
    #[must_use]
    pub fn current_index(&self) -> Option<usize> {
        self.items.iter().position(|item| item.current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pushing_updates_the_total() {
        let mut mission = Mission::new(crate::mavcmd::DEFAULT_MISSION_TYPE);
        assert!(mission.is_empty());

        mission.push(MissionItem::new(MissionItemId::new(1), 16));
        mission.push(MissionItem::new(MissionItemId::new(2), 16));

        assert_eq!(mission.len(), 2);
        assert_eq!(mission.total, 2);
    }

    #[test]
    fn removing_updates_the_total() {
        let mut mission = Mission::new(crate::mavcmd::DEFAULT_MISSION_TYPE);
        mission.push(MissionItem::new(MissionItemId::new(1), 16));
        mission.push(MissionItem::new(MissionItemId::new(2), 16));

        assert!(mission.remove(0).is_some());
        assert_eq!(mission.len(), 1);
        assert_eq!(mission.total, 1);
    }

    #[test]
    fn removing_out_of_range_is_a_no_op() {
        let mut mission = Mission::new(crate::mavcmd::DEFAULT_MISSION_TYPE);
        assert!(mission.remove(5).is_none());
        assert_eq!(mission.total, 0);
    }

    #[test]
    fn items_inherit_the_mission_type() {
        let mut mission = Mission::new(1);
        let mut item = MissionItem::new(MissionItemId::new(1), 16);
        item.mission_type = 99;
        mission.push(item);
        assert_eq!(mission.items[0].mission_type, 1);
    }
}
