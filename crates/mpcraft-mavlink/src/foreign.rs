//! The seeds MPCraft works out from the specification, checked against the values the generator
//! publishes.
//!
//! The dialect comes from the `mavlink` crate, which generates its message definitions from the
//! canonical XML at build time. As it stands the generator carries every message in the table
//! below, so the table is not what the runtime uses: `extra_crc` takes the seed from the generator
//! first, and reaches the table only for an id it does not carry. What the table is for is the
//! comparison — the seed written here is *computed* from the name of the message and the types of
//! its fields, by a rule written from the specification rather than copied from the generator, and
//! the tests say the two agree. That agreement is what makes the field order a fact rather than a
//! guess, because the seed is worked out over the fields in the order the wire writes them.
//!
//! ```text
//! crc = x25crc(name + " ")
//! for each field, in wire order:
//!     crc.accumulate(type name)
//!     if the field is an array: crc.accumulate(length)
//! seed = (crc & 0xFF) ^ (crc >> 8)
//! ```
//!
//! The wire order is the order the fields are written in a frame: the widest first, and
//! the order of the definition kept between fields of the same width.

/// The messages whose seeds MPCraft computes, with the values the generator publishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignMessage {
    /// The message id, which is what a frame carries.
    pub id: u32,
    /// The canonical name of the message.
    pub name: &'static str,
    /// The seed of the checksum, fixed by the specification.
    pub crc_extra: u8,
}

/// The messages this module checks, in the order the ids sort in.
pub const FOREIGN_MESSAGES: &[ForeignMessage] = &[
    ForeignMessage {
        id: 110,
        name: "FILE_TRANSFER_PROTOCOL",
        crc_extra: 84,
    },
    ForeignMessage {
        id: 310,
        name: "UAVCAN_NODE_STATUS",
        crc_extra: 28,
    },
    ForeignMessage {
        id: 311,
        name: "UAVCAN_NODE_INFO",
        crc_extra: 95,
    },
];

/// The message of the given id, when it is one of the foreign ones.
#[must_use]
pub fn foreign(id: u32) -> Option<&'static ForeignMessage> {
    FOREIGN_MESSAGES.iter().find(|message| message.id == id)
}

/// The message of the given name, when it is one of the foreign ones.
#[must_use]
pub fn foreign_by_name(name: &str) -> Option<&'static ForeignMessage> {
    FOREIGN_MESSAGES.iter().find(|message| message.name == name)
}

/// The type of a field of a message, as the specification spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    /// An unsigned byte.
    U8,
    /// An unsigned 16-bit integer.
    U16,
    /// An unsigned 32-bit integer.
    U32,
    /// An unsigned 64-bit integer.
    U64,
    /// A signed byte.
    I8,
    /// A signed 16-bit integer.
    I16,
    /// A signed 32-bit integer.
    I32,
    /// A signed 64-bit integer.
    I64,
    /// A 32-bit float.
    F32,
    /// A 64-bit float.
    F64,
    /// A character.
    Char,
}

impl FieldType {
    /// The name the specification writes the type with.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::U8 => "uint8_t",
            Self::U16 => "uint16_t",
            Self::U32 => "uint32_t",
            Self::U64 => "uint64_t",
            Self::I8 => "int8_t",
            Self::I16 => "int16_t",
            Self::I32 => "int32_t",
            Self::I64 => "int64_t",
            Self::F32 => "float",
            Self::F64 => "double",
            Self::Char => "char",
        }
    }

    /// How many bytes the type takes on the wire.
    #[must_use]
    pub const fn size(self) -> usize {
        match self {
            Self::U8 | Self::I8 | Self::Char => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
            Self::U64 | Self::I64 | Self::F64 => 8,
        }
    }
}

/// The CRC_EXTRA seed of a message, worked out from its definition.
///
/// The fields are given with their name, their type and their array length — zero for a
/// field that is not an array — in the order of the definition.
#[must_use]
pub fn crc_extra_of(name: &str, fields: &[(&str, FieldType, usize)]) -> u8 {
    // The seed is over the name of the message and, for every field, its type and its
    // name: in the order the fields are written, the widest first, and the definition's
    // order kept between fields of the same width.
    let mut ordered: Vec<(&str, FieldType, usize)> = fields.to_vec();
    ordered.sort_by_key(|(_, field, _)| std::cmp::Reverse(field.size()));

    let mut crc: u16 = 0xFFFF;
    crc = accumulate(crc, name);
    crc = accumulate(crc, " ");

    for (field_name, field_type, length) in ordered {
        crc = accumulate(crc, field_type.name());
        crc = accumulate(crc, " ");
        crc = accumulate(crc, field_name);
        crc = accumulate(crc, " ");

        if length > 0 {
            crc = crate::frame::crc_accumulate(u8::try_from(length).unwrap_or(u8::MAX), crc);
        }
    }

    ((crc & 0xFF) ^ (crc >> 8)) as u8
}

/// Adds a string to a checksum.
fn accumulate(mut crc: u16, text: &str) -> u16 {
    for byte in text.bytes() {
        crc = crate::frame::crc_accumulate(byte, crc);
    }

    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seed_of_a_message_the_dialect_carries_is_the_one_the_specification_says() {
        // HEARTBEAT, declared as type, autopilot, base_mode, custom_mode, system_status
        // and mavlink_version: the 32-bit custom mode is written first.
        let heartbeat = crc_extra_of(
            "HEARTBEAT",
            &[
                ("type", FieldType::U8, 0),
                ("autopilot", FieldType::U8, 0),
                ("base_mode", FieldType::U8, 0),
                ("custom_mode", FieldType::U32, 0),
                ("system_status", FieldType::U8, 0),
                ("mavlink_version", FieldType::U8, 0),
            ],
        );
        assert_eq!(heartbeat, 50, "HEARTBEAT keeps its seed of 50");

        // ATTITUDE: a 32-bit time and six floats.
        let attitude = crc_extra_of(
            "ATTITUDE",
            &[
                ("time_boot_ms", FieldType::U32, 0),
                ("roll", FieldType::F32, 0),
                ("pitch", FieldType::F32, 0),
                ("yaw", FieldType::F32, 0),
                ("rollspeed", FieldType::F32, 0),
                ("pitchspeed", FieldType::F32, 0),
                ("yawspeed", FieldType::F32, 0),
            ],
        );
        assert_eq!(attitude, 39, "ATTITUDE keeps its seed of 39");
    }

    #[test]
    fn the_seed_of_the_file_transfer_is_the_one_ardupilot_uses() {
        // target_network, target_system, target_component and the payload of 251 bytes.
        let seed = crc_extra_of(
            "FILE_TRANSFER_PROTOCOL",
            &[
                ("target_network", FieldType::U8, 0),
                ("target_system", FieldType::U8, 0),
                ("target_component", FieldType::U8, 0),
                ("payload", FieldType::U8, 251),
            ],
        );

        assert_eq!(seed, 84);
        assert_eq!(foreign(110).map(|message| message.crc_extra), Some(seed));
    }

    #[test]
    fn the_seeds_of_the_dronecan_nodes_are_the_ones_the_rule_and_a_second_reader_give() {
        // The rule, written from the specification, and pymavlink's own generator give the
        // same two numbers without being built from each other. That agreement settles the
        // order the fields are written in as well, because the seed is worked out over the
        // fields in that order: a different order would give a different number.
        let status = crc_extra_of(
            "UAVCAN_NODE_STATUS",
            &[
                ("time_usec", FieldType::U64, 0),
                ("uptime_sec", FieldType::U32, 0),
                ("health", FieldType::U8, 0),
                ("mode", FieldType::U8, 0),
                ("sub_mode", FieldType::U8, 0),
                ("vendor_specific_status_code", FieldType::U16, 0),
            ],
        );
        assert_eq!(status, 28, "the node status keeps its seed of 28");
        assert_eq!(foreign(310).map(|message| message.crc_extra), Some(status));

        let info = crc_extra_of(
            "UAVCAN_NODE_INFO",
            &[
                ("time_usec", FieldType::U64, 0),
                ("uptime_sec", FieldType::U32, 0),
                ("name", FieldType::Char, 80),
                ("hw_version_major", FieldType::U8, 0),
                ("hw_version_minor", FieldType::U8, 0),
                ("hw_unique_id", FieldType::U8, 16),
                ("sw_version_major", FieldType::U8, 0),
                ("sw_version_minor", FieldType::U8, 0),
                ("sw_vcs_commit", FieldType::U32, 0),
            ],
        );
        assert_eq!(info, 95, "the node information keeps its seed of 95");
        assert_eq!(foreign(311).map(|message| message.crc_extra), Some(info));
    }

    #[test]
    fn the_generator_publishes_the_same_seeds_as_the_rule() {
        // Two implementations that were not built from each other: the rule written from the
        // specification, and the code generated from the canonical XML. They agree on every message
        // in the table, which is what makes the order the fields are read in a fact — a different
        // order would give a different number.
        for message in FOREIGN_MESSAGES {
            assert_eq!(
                crate::dialect::extra_crc(message.id),
                Some(message.crc_extra),
                "the generator disagrees with the rule for {}",
                message.name
            );
        }
    }

    #[test]
    fn a_message_of_the_table_is_found_by_its_id_and_by_its_name() {
        assert_eq!(
            foreign(110).map(|message| message.name),
            Some("FILE_TRANSFER_PROTOCOL")
        );
        assert_eq!(
            foreign_by_name("FILE_TRANSFER_PROTOCOL").map(|message| message.id),
            Some(110)
        );
        assert!(foreign(111).is_none());
    }

    #[test]
    fn the_fields_are_ordered_by_width_not_by_their_name() {
        // The order the fields are declared in does not change the seed: the protocol
        // writes the widest first, and so does this.
        let declared = crc_extra_of(
            "SOMETHING",
            &[("a", FieldType::U8, 0), ("b", FieldType::U32, 0)],
        );
        let sorted = crc_extra_of(
            "SOMETHING",
            &[("b", FieldType::U32, 0), ("a", FieldType::U8, 0)],
        );

        assert_eq!(declared, sorted);
    }

    #[test]
    fn the_name_of_a_field_is_part_of_the_seed() {
        // The seed changes when a field is renamed, which is what lets a vehicle notice
        // that its idea of a message and ours are not the same one.
        let first = crc_extra_of("SOMETHING", &[("a", FieldType::U32, 0)]);
        let second = crc_extra_of("SOMETHING", &[("b", FieldType::U32, 0)]);

        assert_ne!(first, second);
    }
}
