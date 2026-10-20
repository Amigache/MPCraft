//! Mission items on the wire.
//!
//! # The two encodings
//!
//! A mission item travels either as a `MISSION_ITEM` or as a `MISSION_ITEM_INT`. The
//! integer form scales latitude and longitude by `1e7`; the float form does not.
//!
//! **Only the coordinates of a location command are scaled.** A command whose X and Y are
//! not a position — `CONDITION_DELAY`, say — keeps them as they are. Scaling those would
//! multiply them by ten million.
//!
//! # The capability
//!
//! A vehicle that advertises `MAV_PROTOCOL_CAPABILITY::MISSION_INT` wants the integer form.
//! Mission Planner does the same, and MPCraft keeps it: the two forms are not
//! interchangeable, and sending the wrong one makes the vehicle fly to the wrong place.
//!
//! # The mission type
//!
//! `MAV_MISSION_TYPE` is a **MAVLink 2 extension field**, which means it is absent from a
//! MAVLink 1 frame and truncated away when it is zero. A plain mission therefore travels in
//! the short form, and the fence and rally plans travel with the extension present.

#[allow(deprecated)]
use mpcraft_mavlink::dialect::messages::{MISSION_ITEM_DATA, MISSION_ITEM_INT_DATA};
use mpcraft_mavlink::MavFrame;
use serde::{Deserialize, Serialize};

use crate::MissionItem;

/// The kind of plan a mission belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum MissionType {
    /// The mission the vehicle flies.
    #[default]
    /// The mission the vehicle flies.
    Mission = 0,
    /// The geofence.
    Fence = 1,
    /// The rally points.
    Rally = 2,
}

impl MissionType {
    /// The MAVLink wire value.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// The mission type a wire value corresponds to.
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Fence,
            2 => Self::Rally,
            _ => Self::Mission,
        }
    }
}

impl MissionItem {
    /// Encodes the item as a `MISSION_ITEM`.
    ///
    /// The float form is deprecated in the MAVLink specification in favour of the integer
    /// form, but a vehicle that does not advertise `MISSION_INT` still expects it.
    #[must_use]
    #[allow(deprecated)]
    pub fn to_float_message(&self, seq: u16) -> MISSION_ITEM_DATA {
        MISSION_ITEM_DATA {
            param1: self.params[0],
            param2: self.params[1],
            param3: self.params[2],
            param4: self.params[3],
            x: self.x as f32,
            y: self.y as f32,
            z: self.z as f32,
            seq,
            command: self.command,
            target_system: 0,
            target_component: 0,
            frame: frame_of(self.frame),
            current: u8::from(self.current),
            autocontinue: u8::from(self.autocontinue),
        }
    }

    /// Encodes the item as a `MISSION_ITEM_INT`.
    ///
    /// The coordinates are scaled by `1e7`, but only for a command that carries a position.
    #[must_use]
    pub fn to_int_message(&self, seq: u16) -> MISSION_ITEM_INT_DATA {
        let (x, y) = if self.is_location_command() {
            ((self.x * 1e7) as i32, (self.y * 1e7) as i32)
        } else {
            (self.x as i32, self.y as i32)
        };

        MISSION_ITEM_INT_DATA {
            param1: self.params[0],
            param2: self.params[1],
            param3: self.params[2],
            param4: self.params[3],
            x,
            y,
            z: self.z as f32,
            seq,
            command: self.command,
            target_system: 0,
            target_component: 0,
            frame: frame_of(self.frame),
            current: u8::from(self.current),
            autocontinue: u8::from(self.autocontinue),
        }
    }

    /// Decodes a `MISSION_ITEM`.
    #[must_use]
    #[allow(deprecated)]
    pub fn from_float_message(message: &MISSION_ITEM_DATA) -> Self {
        Self {
            id: crate::MissionItemId::next(),
            command: message.command,
            frame: message.frame as u32,
            params: [
                message.param1,
                message.param2,
                message.param3,
                message.param4,
            ],
            x: f64::from(message.x),
            y: f64::from(message.y),
            z: f64::from(message.z),
            current: message.current != 0,
            autocontinue: message.autocontinue != 0,
            display_name: None,
        }
    }

    /// Decodes a `MISSION_ITEM_INT`.
    ///
    /// The coordinates are unscaled, but only for a command that carries a position.
    #[must_use]
    pub fn from_int_message(message: &MISSION_ITEM_INT_DATA) -> Self {
        let mut item = Self {
            id: crate::MissionItemId::next(),
            command: message.command,
            frame: message.frame as u32,
            params: [
                message.param1,
                message.param2,
                message.param3,
                message.param4,
            ],
            x: f64::from(message.x) / 1e7,
            y: f64::from(message.y) / 1e7,
            z: f64::from(message.z),
            current: message.current != 0,
            autocontinue: message.autocontinue != 0,
            display_name: None,
        };

        if !item.is_location_command() {
            item.x = f64::from(message.x);
            item.y = f64::from(message.y);
        }

        item
    }
}

/// The coordinate frame of an item, as the MAVLink enumeration.
///
/// The frames that matter for a plan are the global ones. The `_INT` variants are deprecated
/// synonyms and are mapped onto their modern equivalent.
fn frame_of(frame: u32) -> MavFrame {
    match frame {
        0 => MavFrame::MAV_FRAME_GLOBAL,
        1 => MavFrame::MAV_FRAME_LOCAL_NED,
        2 => MavFrame::MAV_FRAME_MISSION,
        3 => MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT,
        4 => MavFrame::MAV_FRAME_LOCAL_ENU,
        5 => MavFrame::MAV_FRAME_GLOBAL,
        6 => MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT,
        7 => MavFrame::MAV_FRAME_LOCAL_OFFSET_NED,
        10 => MavFrame::MAV_FRAME_GLOBAL_TERRAIN_ALT,
        11 => MavFrame::MAV_FRAME_GLOBAL_TERRAIN_ALT,
        _ => MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpcraft_mavlink::MavCmd;

    fn waypoint(lat: f64, lng: f64, alt: f64) -> MissionItem {
        MissionItem {
            command: MavCmd::MAV_CMD_NAV_WAYPOINT,
            x: lat,
            y: lng,
            z: alt,
            ..MissionItem::default()
        }
    }

    #[test]
    fn a_float_item_round_trips() {
        let item = waypoint(-35.2809, 149.1300, 584.0);
        let message = item.to_float_message(3);
        let decoded = MissionItem::from_float_message(&message);

        assert_eq!(decoded.command, item.command);
        assert_eq!(decoded.frame, item.frame);
        assert!((decoded.x - item.x).abs() < 1e-4);
        assert!((decoded.y - item.y).abs() < 1e-4);
        assert!((decoded.z - item.z).abs() < 1e-3);
        assert_eq!(decoded.current, item.current);
        assert_eq!(decoded.autocontinue, item.autocontinue);
    }

    #[test]
    fn an_int_item_round_trips() {
        let item = waypoint(-35.2809, 149.1300, 584.0);
        let message = item.to_int_message(3);
        let decoded = MissionItem::from_int_message(&message);

        assert!((decoded.x - item.x).abs() < 1e-6);
        assert!((decoded.y - item.y).abs() < 1e-6);
        assert!((decoded.z - item.z).abs() < 1e-3);
    }

    #[test]
    fn the_integer_encoding_scales_the_coordinates() {
        let item = waypoint(-35.2809, 149.1300, 584.0);
        let message = item.to_int_message(0);

        assert_eq!(message.x, (-35.2809 * 1e7) as i32);
        assert_eq!(message.y, (149.1300 * 1e7) as i32);
    }

    #[test]
    fn a_command_without_a_position_is_not_scaled() {
        // `CONDITION_DELAY` has no position, so its X and Y must not be multiplied by ten
        // million.
        let mut item = MissionItem::new(MavCmd::MAV_CMD_CONDITION_DELAY);
        item.x = 5.0;
        item.y = 7.0;

        assert!(!item.is_location_command());

        let message = item.to_int_message(0);
        assert_eq!(message.x, 5);
        assert_eq!(message.y, 7);

        let decoded = MissionItem::from_int_message(&message);
        assert_eq!(decoded.x, 5.0);
        assert_eq!(decoded.y, 7.0);
    }

    #[test]
    #[allow(deprecated)]
    fn the_sequence_number_is_carried_by_the_message() {
        let item = waypoint(1.0, 2.0, 3.0);
        assert_eq!(item.to_float_message(17).seq, 17);
        assert_eq!(item.to_int_message(17).seq, 17);
    }

    #[test]
    fn the_mission_types_map_to_their_wire_values() {
        assert_eq!(MissionType::Mission.as_u8(), 0);
        assert_eq!(MissionType::Fence.as_u8(), 1);
        assert_eq!(MissionType::Rally.as_u8(), 2);
        assert_eq!(MissionType::from_u8(2), MissionType::Rally);
        assert_eq!(MissionType::from_u8(9), MissionType::Mission);
    }
}
