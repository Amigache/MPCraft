//! Re-exports of the generated MAVLink dialect types.
//!
//! The message and enum definitions come from the upstream `mavlink` crate, which generates
//! them from the canonical XML dialects via `mavlink-bindgen`. This module exists so the rest of
//! MPCraft depends on `mpcraft_mavlink` rather than on the upstream crate directly — that keeps
//! the generated API behind a boundary we can evolve.
//!
//! # Generated naming
//!
//! The generator uses `CamelCase` for enums (`MavCmd`) and `<NAME>_DATA` for message structs
//! (`HEARTBEAT_DATA`), with the message enum variant carrying the canonical name
//! (`MavMessage::HEARTBEAT`). The aliases below hide that convention so the rest of MPCraft can
//! use the canonical MAVLink spelling.

pub use mavlink::dialects::ardupilotmega::MavMessage;
pub use mavlink::dialects::ardupilotmega::{
    MavAutopilot, MavCmd, MavFrame, MavMissionResult, MavMissionType, MavModeFlag, MavParamType,
    MavProtocolCapability, MavResult, MavSeverity, MavState, MavSysStatusSensor, MavType,
    SerialControlDev, SerialControlFlag,
};

// The enums the slots of the on-screen display use.
pub use mavlink::dialects::ardupilotmega::{OsdParamConfigError, OsdParamConfigType};

/// The type of the GPS fix.
///
/// The enumeration is called `GPS_FIX_TYPE` in the dialect, and the generator prefixes
/// the names it generates with `Mav`, so the type is re-exported under the name the rest
/// of the codebase uses.
pub use mavlink::dialects::ardupilotmega::GpsFixType as MavGpsFixType;

// The generated message structs live in the dialect module. They are re-exported as a whole
// so that callers can build a message of a specific type — `HEARTBEAT_DATA`, say — without
// having to know the generator's naming convention.
pub use mavlink::dialects::ardupilotmega as messages;

pub use mavlink::types;
pub use mavlink::types::CharArray;
pub use mavlink::{MavlinkVersion, Message};

/// The compiled dialect used throughout MPCraft: ArduPilot Mega plus the common dialect.
///
/// `MavMessage` is the dialect's message enum. It implements [`Message`], so a frame can be
/// decoded generically from a message id and payload.
pub type Dialect = mavlink::dialects::ardupilotmega::MavMessage;

/// Returns the CRC_EXTRA seed for a message id.
///
/// The seed is folded into the CRC **after** the payload. A frame whose CRC does not match with
/// this seed is corrupt.
///
/// Returns `None` for message ids that are not part of the compiled dialect — the caller must
/// treat those as undecodable rather than assume a seed of zero.
///
/// ```
/// use mpcraft_mavlink::dialect::extra_crc;
///
/// // HEARTBEAT is message id 0 and has a well-known CRC_EXTRA of 50.
/// assert_eq!(extra_crc(0), Some(50));
/// assert_eq!(extra_crc(999_999), None);
/// ```
#[must_use]
pub fn extra_crc(message_id: u32) -> Option<u8> {
    // `extra_crc` panics on an unknown id in the generated code, so membership is checked
    // against the message table first.
    if !Dialect::all_messages()
        .iter()
        .any(|(_, id)| *id == message_id)
    {
        // The messages the dialect does not carry, but the vehicle speaks, have their
        // seed written down with them: the checksum of their frames is as real as any.
        return crate::foreign::foreign(message_id).map(|message| message.crc_extra);
    }
    Some(Dialect::extra_crc(message_id))
}

/// Returns the canonical name of a message id, if the dialect knows it.
///
/// ```
/// use mpcraft_mavlink::dialect::message_name;
///
/// assert_eq!(message_name(0), Some("HEARTBEAT"));
/// assert_eq!(message_name(999_999), None);
/// ```
#[must_use]
pub fn message_name(message_id: u32) -> Option<&'static str> {
    if let Some(name) = Dialect::all_messages()
        .iter()
        .find(|(_, id)| *id == message_id)
        .map(|(name, _)| *name)
    {
        return Some(name);
    }

    crate::foreign::foreign(message_id).map(|message| message.name)
}

/// Returns the message id of a canonical message name, if the dialect knows it.
///
/// ```
/// use mpcraft_mavlink::dialect::message_id;
///
/// assert_eq!(message_id("HEARTBEAT"), Some(0));
/// assert_eq!(message_id("NOT_A_MESSAGE"), None);
/// ```
#[must_use]
pub fn message_id(name: &str) -> Option<u32> {
    Dialect::message_id_from_name(name)
        .or_else(|| crate::foreign::foreign_by_name(name).map(|message| message.id))
}

/// The number of messages defined in the compiled dialect.
///
/// Useful as a sanity check in tests: it changes when the dialect XML is updated.
#[must_use]
pub fn message_count() -> usize {
    Dialect::all_messages().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_crc_is_known_for_real_messages() {
        // These seeds are fixed by the MAVLink specification and never change.
        assert_eq!(extra_crc(0), Some(50)); // HEARTBEAT
        assert_eq!(extra_crc(30), Some(39)); // ATTITUDE
    }

    #[test]
    fn extra_crc_is_none_for_unknown_messages() {
        assert_eq!(extra_crc(999_999), None);
    }

    #[test]
    fn names_and_ids_round_trip() {
        let id = message_id("HEARTBEAT").expect("HEARTBEAT is in the dialect");
        assert_eq!(id, 0);
        assert_eq!(message_name(id), Some("HEARTBEAT"));
        assert_eq!(message_id("NOT_A_REAL_MESSAGE"), None);
        assert_eq!(message_name(999_999), None);
    }

    #[test]
    fn dialect_contains_the_messages_mpcraft_relies_on() {
        // Heartbeat, attitude, GPS, global position and the mission protocol messages must all
        // be present, or the GCS cannot function.
        for name in [
            "HEARTBEAT",
            "ATTITUDE",
            "GLOBAL_POSITION_INT",
            "GPS_RAW_INT",
            "SYS_STATUS",
            "MISSION_ITEM",
            "MISSION_ITEM_INT",
            "MISSION_COUNT",
            "MISSION_REQUEST",
            "MISSION_REQUEST_INT",
            "MISSION_ACK",
            "PARAM_VALUE",
            "PARAM_REQUEST_LIST",
            "PARAM_REQUEST_READ",
            "PARAM_SET",
            "COMMAND_LONG",
            "COMMAND_INT",
            "SET_MODE",
            "STATUSTEXT",
        ] {
            assert!(message_id(name).is_some(), "the dialect is missing {name}");
        }
    }

    #[test]
    fn dialect_has_a_plausible_message_count() {
        assert!(message_count() > 200, "dialect looks truncated");
    }
}
