//! The plan of a vehicle, in the file the vehicle keeps it in.
//!
//! The vehicle can hold a plan in a file of its own, and the file transfer protocol is
//! how a program reaches it. The format is the one ArduPilot uses: a header of twelve
//! bytes, and then the items, each one the payload of a `MISSION_ITEM_INT` message.
//!
//! ```text
//! ┌────────┬──────────┬─────────┬────────┬───────────┬──────────┐
//! │ magic  │ data type│ options │ start  │ how many  │ reserved │
//! │ 2 bytes│ 2 bytes  │ 2 bytes │ 2 bytes│ 2 bytes   │ 2 bytes  │
//! └────────┴──────────┴─────────┴────────┴───────────┴──────────┘
//! ┌────────────────────────────────────────────────────────────┐
//! │ the items, each one the payload of a MISSION_ITEM_INT      │
//! └────────────────────────────────────────────────────────────┘
//! ```
//!
//! The items travel in the order MAVLink 2 puts them in, which is not the order they
//! are declared in: the widest fields come first, and the fields that are extensions
//! come last, unsorted.

use crate::wire::MissionType;
use crate::{MissionItem, Plan};
use num_traits::{FromPrimitive, ToPrimitive};

/// The number that says the file holds a plan.
pub const MISSION_MAGIC: u16 = 0x763D;

/// The length of the header of the file, in bytes.
pub const HEADER_LEN: usize = 12;

/// The length of one item of the file, in bytes.
pub const ITEM_LEN: usize = 38;

/// The folder of the vehicle where the plans are kept.
const MISSION_FOLDER: &str = "@MISSION";

/// The name of the file of a kind of plan.
const fn file_name(mission_type: MissionType) -> &'static str {
    match mission_type {
        MissionType::Mission => "mission.dat",
        MissionType::Fence => "fence.dat",
        MissionType::Rally => "rally.dat",
    }
}

/// The path of the file where the vehicle keeps a kind of plan.
#[must_use]
pub fn path_of(mission_type: MissionType) -> String {
    format!("{MISSION_FOLDER}/{}", file_name(mission_type))
}

/// Why a file of a plan could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissionPckError {
    /// The file is shorter than the header.
    TooShort,
    /// The number that says the file holds a plan is not there.
    WrongMagic(u16),
    /// The file says how many items it holds, and it does not hold them.
    BadCount {
        /// How many items the file says it holds.
        said: usize,
        /// How many items the file holds.
        got: usize,
    },
    /// The command of an item is not one the protocol knows.
    UnknownCommand(u16),
}

impl std::fmt::Display for MissionPckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "the file is shorter than the header of a plan"),
            Self::WrongMagic(magic) => {
                write!(f, "the file does not hold a plan: the number is {magic}")
            }
            Self::BadCount { said, got } => {
                write!(f, "the file says it holds {said} items, and it holds {got}")
            }
            Self::UnknownCommand(command) => {
                write!(f, "the command {command} is not one the protocol knows")
            }
        }
    }
}

impl std::error::Error for MissionPckError {}

/// Packs a plan into the bytes of the file the vehicle keeps it in.
///
/// # Errors
///
/// Returns an error when the file cannot hold the plan.
pub fn pack(plan: &Plan) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(HEADER_LEN + plan.items.len() * ITEM_LEN);

    bytes.extend_from_slice(&MISSION_MAGIC.to_le_bytes());
    bytes.extend_from_slice(&(plan.mission_type.as_u8() as u16).to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&(plan.items.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());

    for (index, item) in plan.items.iter().enumerate() {
        bytes.extend_from_slice(&encode_item(item, index as u16, plan.mission_type));
    }

    bytes
}

/// Unpacks a plan from the bytes of the file the vehicle keeps it in.
///
/// # Errors
///
/// Returns an error when the file does not hold a plan, or when it is cut short.
pub fn unpack(bytes: &[u8]) -> Result<Plan, MissionPckError> {
    if bytes.len() < HEADER_LEN {
        return Err(MissionPckError::TooShort);
    }

    let magic = u16::from_le_bytes([bytes[0], bytes[1]]);
    if magic != MISSION_MAGIC {
        return Err(MissionPckError::WrongMagic(magic));
    }

    let mission_type = MissionType::from_u8(bytes[2]);
    let said = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;

    let body = &bytes[HEADER_LEN..];
    let got = body.len() / ITEM_LEN;

    if said != got {
        return Err(MissionPckError::BadCount { said, got });
    }

    let mut plan = Plan::new(mission_type);

    for index in 0..got {
        let chunk = &body[index * ITEM_LEN..(index + 1) * ITEM_LEN];
        plan.push(decode_item(chunk)?);
    }

    Ok(plan)
}

/// Encodes one item the way it travels in the payload of a `MISSION_ITEM_INT`.
///
/// The order is the one MAVLink 2 uses: the four parameters, the two coordinates and
/// the height, all of them four bytes wide; then the two numbers that are two bytes
/// wide; then the bytes; and the kind of the plan last, because it is an extension.
fn encode_item(item: &MissionItem, seq: u16, mission_type: MissionType) -> [u8; ITEM_LEN] {
    let mut bytes = [0_u8; ITEM_LEN];

    for (index, param) in item.params.iter().enumerate() {
        bytes[index * 4..(index + 1) * 4].copy_from_slice(&param.to_le_bytes());
    }

    // The coordinates are scaled by a hundred millionths of a degree, but only for a
    // command that carries a position.
    let (x, y) = if item.is_location_command() {
        ((item.x * 1e7) as i32, (item.y * 1e7) as i32)
    } else {
        (item.x as i32, item.y as i32)
    };

    bytes[16..20].copy_from_slice(&x.to_le_bytes());
    bytes[20..24].copy_from_slice(&y.to_le_bytes());
    bytes[24..28].copy_from_slice(&(item.z as f32).to_le_bytes());
    bytes[28..30].copy_from_slice(&seq.to_le_bytes());
    bytes[30..32]
        .copy_from_slice(&(item.command.to_u64().unwrap_or_default() as u16).to_le_bytes());
    bytes[32] = 0;
    bytes[33] = 0;
    bytes[34] = crate::wire::frame_of(item.frame) as u8;
    bytes[35] = u8::from(item.current);
    bytes[36] = u8::from(item.autocontinue);
    bytes[37] = mission_type.as_u8();

    bytes
}

/// Decodes one item from the bytes it travels in.
///
/// # Errors
///
/// Returns an error when the command of the item is not one the protocol knows.
fn decode_item(bytes: &[u8]) -> Result<MissionItem, MissionPckError> {
    let command = u16::from_le_bytes([bytes[30], bytes[31]]);

    let Some(command) = mpcraft_mavlink::MavCmd::from_u64(u64::from(command)) else {
        return Err(MissionPckError::UnknownCommand(command));
    };

    let mut item = MissionItem {
        id: crate::MissionItemId::next(),
        command,
        frame: u32::from(bytes[34]),
        params: [
            f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            f32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            f32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            f32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
        ],
        x: f64::from(i32::from_le_bytes([
            bytes[16], bytes[17], bytes[18], bytes[19],
        ])),
        y: f64::from(i32::from_le_bytes([
            bytes[20], bytes[21], bytes[22], bytes[23],
        ])),
        z: f64::from(f32::from_le_bytes([
            bytes[24], bytes[25], bytes[26], bytes[27],
        ])),
        current: bytes[35] != 0,
        autocontinue: bytes[36] != 0,
        display_name: None,
    };

    // The coordinates are scaled by a hundred millionths of a degree, but only for a
    // command that carries a position.
    if item.is_location_command() {
        item.x /= 1e7;
        item.y /= 1e7;
    }

    Ok(item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MissionItem;
    use mpcraft_mavlink::MavCmd;

    fn waypoint(latitude: f64, longitude: f64, altitude: f64) -> MissionItem {
        MissionItem {
            command: MavCmd::MAV_CMD_NAV_WAYPOINT,
            frame: 6,
            params: [1.0, 2.0, 3.0, 4.0],
            x: latitude,
            y: longitude,
            z: altitude,
            current: false,
            autocontinue: true,
            display_name: None,
            ..MissionItem::default()
        }
    }

    #[test]
    fn a_plan_survives_the_file_it_is_kept_in() {
        let mut plan = Plan::new(MissionType::Mission);
        plan.push(waypoint(-35.2809, 149.1300, 584.0));
        plan.push(waypoint(-35.2811, 149.1304, 610.0));

        let bytes = pack(&plan);
        assert_eq!(bytes.len(), HEADER_LEN + 2 * ITEM_LEN);

        let read = unpack(&bytes).expect("the file holds a plan");
        assert_eq!(read.mission_type, MissionType::Mission);
        assert_eq!(read.items.len(), 2);
        assert_eq!(read.items[0].x, -35.2809);
        assert_eq!(read.items[0].y, 149.1300);
        assert_eq!(read.items[0].z, 584.0);
        assert_eq!(read.items[1].params, [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn the_file_of_a_plan_has_the_number_that_says_so() {
        let plan = Plan::new(MissionType::Fence);
        let bytes = pack(&plan);

        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), MISSION_MAGIC);
        assert_eq!(bytes[2], MissionType::Fence.as_u8());
        assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 0);
    }

    #[test]
    fn a_file_that_is_not_a_plan_is_refused() {
        let error = unpack(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap_err();
        assert_eq!(error, MissionPckError::WrongMagic(0));
    }

    #[test]
    fn a_file_that_is_cut_short_is_refused() {
        let mut plan = Plan::new(MissionType::Mission);
        plan.push(waypoint(1.0, 2.0, 3.0));

        let mut bytes = pack(&plan);
        bytes.truncate(HEADER_LEN + 10);

        let error = unpack(&bytes).unwrap_err();
        assert_eq!(error, MissionPckError::BadCount { said: 1, got: 0 });
    }

    #[test]
    fn the_items_travel_the_way_the_protocol_puts_them() {
        // The bytes of an item are the payload of a MISSION_ITEM_INT, so they are the
        // bytes the dialect itself would write.
        use mpcraft_mavlink::{MavMessage, MavlinkVersion, Message};

        let item = waypoint(-35.2809, 149.1300, 584.0);
        let ours = encode_item(&item, 3, MissionType::Mission);

        let message = MavMessage::MISSION_ITEM_INT(item.to_int_message(3, MissionType::Mission));
        let mut theirs = [0_u8; 64];
        let length = message.ser(MavlinkVersion::V2, &mut theirs);

        // The dialect writes the kind of the plan as an extension, which is the last
        // byte of the payload — and it trims the zeros at the end of the payload, so
        // the kind of the mission is trimmed away with them.
        assert_eq!(length, ITEM_LEN - 1);
        assert_eq!(&ours[..36], &theirs[..36]);
        assert_eq!(ours[37], MissionType::Mission.as_u8());

        // The fence is written whole, because its kind is not zero.
        let message = MavMessage::MISSION_ITEM_INT(item.to_int_message(3, MissionType::Fence));
        let length = message.ser(MavlinkVersion::V2, &mut theirs);
        assert_eq!(length, ITEM_LEN);
        assert_eq!(&ours[..37], &theirs[..37]);
    }

    #[test]
    fn each_kind_of_plan_is_kept_in_a_file_of_its_own() {
        assert_eq!(path_of(MissionType::Mission), "@MISSION/mission.dat");
        assert_eq!(path_of(MissionType::Fence), "@MISSION/fence.dat");
        assert_eq!(path_of(MissionType::Rally), "@MISSION/rally.dat");
    }
}
