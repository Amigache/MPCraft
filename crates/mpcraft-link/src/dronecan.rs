//! The nodes of a DroneCAN bus.
//!
//! A vehicle with a CAN bus has devices on it that are not the autopilot, and the bus says
//! what they are: each node reports how long it has been up and how it is, and the ground
//! station can ask for what it says about itself — its name, its hardware and software
//! versions, and the number its build carries.
//!
//! These two messages are carried by the compiled dialect — which is generated from the canonical
//! XML at build time, not from an older snapshot — but their health and mode are enumerations there,
//! and the generator refuses a number the dictionary does not give: a status with `health = 5` is
//! answered `Invalid enum value for enum type "UavcanNodeHealth", got 5`, so that node would never
//! reach the sheet. The payload is therefore read from the bytes, as upstream reads it
//! (`ToStructure<mavlink_uavcan_node_status_t>`), and the order used here is the order the seed is
//! worked out over — the one pymavlink's own generator agrees on, which is what makes it a fact
//! rather than a guess.
//!
//! ```text
//! UAVCAN_NODE_STATUS (310): time_usec, uptime_sec, vendor_specific_status_code, health, mode, sub_mode
//! UAVCAN_NODE_INFO   (311): time_usec, uptime_sec, sw_vcs_commit, name[80], hw_version_major,
//!                           hw_version_minor, hw_unique_id[16], sw_version_major, sw_version_minor
//! ```
//!
//! Upstream does the same thing (`MAVLinkInterface.cs`, line 5271 onwards): when a status
//! arrives from a pair the station has not seen, it marks that pair as a CAN node and asks it
//! for its information, and the answer fills the name. It does not wait for an acknowledgement
//! — the dictionary says the answers can be lost and that the receiver notices by seeing a
//! status with no matching information, which is what the sheet shows.

use std::collections::BTreeMap;

use serde::Serialize;

use mpcraft_vehicle::VehicleId;

use crate::commands::{encode_command_long, COMMAND_LONG_ID};
use crate::error::LinkError;
use crate::session::Session;

/// The message a node reports with.
pub const NODE_STATUS_ID: u32 = 310;

/// The message a node says what it is with.
pub const NODE_INFO_ID: u32 = 311;

/// The command that asks the vehicle to answer with one information message per node.
///
/// The number comes from the published dictionary, which reserves the range [5200, 5210) for
/// UAVCAN; the test asserts the compiled dialect gives the same number rather than trusting the
/// constant written here. It travels as it was written, through the raw path, because the dictionary
/// marks this command superseded — by `MAV_CMD_REQUEST_MESSAGE`, since 2026-05 — and the code cannot
/// name a variant the dictionary says is superseded without turning that into a warning. Upstream
/// sends the same number, and the bus answers to it.
pub const GET_NODE_INFO_COMMAND: u32 = 5200;

/// What a node says about how long it has been up and how it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeStatus {
    /// The instant the node said this, in microseconds.
    pub time_usec: u64,
    /// How many seconds the node has been up.
    pub uptime_sec: u32,
    /// What the node says about itself, as the dictionary numbers it.
    pub health: u8,
    /// What the node is doing, as the dictionary numbers it.
    pub mode: u8,
    /// The sub-mode, which the dictionary does not define.
    pub sub_mode: u8,
    /// What the vendor says, which is the node's own number.
    pub vendor_specific_status_code: u16,
}

/// What a node says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeInfo {
    /// The instant the node said this, in microseconds.
    pub time_usec: u64,
    /// How many seconds the node has been up, the same number the status carries.
    pub uptime_sec: u32,
    /// The name the node gives itself.
    pub name: String,
    /// The hardware version the node reports.
    pub hw_version_major: u8,
    /// The hardware version the node reports.
    pub hw_version_minor: u8,
    /// The number that identifies this particular board.
    pub hw_unique_id: [u8; 16],
    /// The software version the node reports.
    pub sw_version_major: u8,
    /// The software version the node reports.
    pub sw_version_minor: u8,
    /// The commit the build came from, as the node says it.
    pub sw_vcs_commit: u32,
}

impl NodeStatus {
    /// Reads a status payload, in the order the wire writes it.
    #[must_use]
    pub fn decode(payload: &[u8]) -> Self {
        let mut at = 0;

        let time_usec = read_u64(payload, &mut at);
        let uptime_sec = read_u32(payload, &mut at);
        let vendor_specific_status_code = read_u16(payload, &mut at);
        let health = read_u8(payload, &mut at);
        let mode = read_u8(payload, &mut at);
        let sub_mode = read_u8(payload, &mut at);

        Self {
            time_usec,
            uptime_sec,
            health,
            mode,
            sub_mode,
            vendor_specific_status_code,
        }
    }
}

impl NodeInfo {
    /// Reads an information payload, in the order the wire writes it.
    #[must_use]
    pub fn decode(payload: &[u8]) -> Self {
        let mut at = 0;

        let time_usec = read_u64(payload, &mut at);
        let uptime_sec = read_u32(payload, &mut at);
        let sw_vcs_commit = read_u32(payload, &mut at);
        let name = read_name(payload, &mut at, NAME_LEN);
        let hw_version_major = read_u8(payload, &mut at);
        let hw_version_minor = read_u8(payload, &mut at);
        let mut hw_unique_id = [0_u8; 16];
        for byte in hw_unique_id.iter_mut() {
            *byte = read_u8(payload, &mut at);
        }
        let sw_version_major = read_u8(payload, &mut at);
        let sw_version_minor = read_u8(payload, &mut at);

        Self {
            time_usec,
            uptime_sec,
            name,
            hw_version_major,
            hw_version_minor,
            hw_unique_id,
            sw_version_major,
            sw_version_minor,
            sw_vcs_commit,
        }
    }
}

/// How many bytes the message has for a name.
const NAME_LEN: usize = 80;

/// The health of a node, named as the dictionary names it.
///
/// `None` for a number the dictionary does not give, which is said rather than drawn as a
/// word the vehicle did not say.
#[must_use]
pub fn health_name(health: u8) -> Option<&'static str> {
    match health {
        0 => Some("OK"),
        1 => Some("WARNING"),
        2 => Some("ERROR"),
        3 => Some("CRITICAL"),
        _ => None,
    }
}

/// What a node is doing, named as the dictionary names it.
#[must_use]
pub fn mode_name(mode: u8) -> Option<&'static str> {
    match mode {
        0 => Some("OPERATIONAL"),
        1 => Some("INITIALIZATION"),
        2 => Some("MAINTENANCE"),
        3 => Some("SOFTWARE_UPDATE"),
        7 => Some("OFFLINE"),
        _ => None,
    }
}

/// One node the bus has reported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DronecanNode {
    /// The pair that identifies the node, which is what upstream keeps it under.
    pub vehicle: VehicleId,
    /// How many seconds the node has been up, the last time it said.
    pub uptime_sec: u32,
    /// What the node says about itself, as the dictionary numbers it.
    pub health: u8,
    /// The name the dictionary gives that number, when it gives one.
    pub health_name: Option<String>,
    /// What the node is doing, as the dictionary numbers it.
    pub mode: u8,
    /// The name the dictionary gives that number, when it gives one.
    pub mode_name: Option<String>,
    /// The sub-mode, which the dictionary does not define.
    pub sub_mode: u8,
    /// What the vendor says, which is the node's own number.
    pub vendor_specific_status_code: u16,
    /// What the node says about itself, when it has said.
    pub name: Option<String>,
    /// The hardware version the node reports, when it has reported.
    pub hw_version_major: Option<u8>,
    /// The hardware version the node reports, when it has reported.
    pub hw_version_minor: Option<u8>,
    /// The number that identifies this board, as the bytes say it.
    pub hw_unique_id: Option<String>,
    /// The software version the node reports, when it has reported.
    pub sw_version_major: Option<u8>,
    /// The software version the node reports, when it has reported.
    pub sw_version_minor: Option<u8>,
    /// The commit the build came from, when the node has said it.
    pub sw_vcs_commit: Option<String>,
    /// Whether the node has answered with what it is.
    ///
    /// A status with no matching information is what the dictionary says has to be asked for
    /// again, so this is shown rather than assumed away.
    pub info_missing: bool,
}

impl DronecanNode {
    /// A node as its first status describes it.
    #[must_use]
    pub fn of(vehicle: VehicleId, status: &NodeStatus) -> Self {
        Self {
            vehicle,
            uptime_sec: status.uptime_sec,
            health: status.health,
            health_name: health_name(status.health).map(str::to_owned),
            mode: status.mode,
            mode_name: mode_name(status.mode).map(str::to_owned),
            sub_mode: status.sub_mode,
            vendor_specific_status_code: status.vendor_specific_status_code,
            name: None,
            hw_version_major: None,
            hw_version_minor: None,
            hw_unique_id: None,
            sw_version_major: None,
            sw_version_minor: None,
            sw_vcs_commit: None,
            info_missing: true,
        }
    }

    /// Fills in what the node said about itself.
    pub fn apply_info(&mut self, info: &NodeInfo) {
        self.name = Some(info.name.clone());
        self.hw_version_major = Some(info.hw_version_major);
        self.hw_version_minor = Some(info.hw_version_minor);
        self.hw_unique_id = Some(hex_of(&info.hw_unique_id));
        self.sw_version_major = Some(info.sw_version_major);
        self.sw_version_minor = Some(info.sw_version_minor);
        self.sw_vcs_commit = Some(format!("{:08x}", info.sw_vcs_commit));
        self.info_missing = false;
    }
}

/// The nodes the bus has reported, by the pair that identifies them.
#[derive(Debug, Clone, Default)]
pub struct DronecanRegistry {
    nodes: BTreeMap<VehicleId, DronecanNode>,
}

impl DronecanRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Keeps a status, and says whether this is the first time the node has been seen — which
    /// is the moment upstream asks it for what it is.
    pub fn observe_status(&mut self, vehicle: VehicleId, status: &NodeStatus) -> bool {
        let first = !self.nodes.contains_key(&vehicle);

        if first {
            self.nodes
                .insert(vehicle, DronecanNode::of(vehicle, status));
            return true;
        }

        if let Some(node) = self.nodes.get_mut(&vehicle) {
            node.uptime_sec = status.uptime_sec;
            node.health = status.health;
            node.health_name = health_name(status.health).map(str::to_owned);
            node.mode = status.mode;
            node.mode_name = mode_name(status.mode).map(str::to_owned);
            node.sub_mode = status.sub_mode;
            node.vendor_specific_status_code = status.vendor_specific_status_code;
        }

        false
    }

    /// Keeps what a node said about itself.
    pub fn observe_info(&mut self, vehicle: VehicleId, info: &NodeInfo) {
        let node = self
            .nodes
            .entry(vehicle)
            .or_insert_with(|| DronecanNode::of(vehicle, &zero_status(info)));

        node.apply_info(info);
    }

    /// The nodes, in the order the pairs sort in.
    #[must_use]
    pub fn nodes(&self) -> Vec<DronecanNode> {
        self.nodes.values().cloned().collect()
    }

    /// The nodes that have reported but never said what they are: the ones worth asking again.
    #[must_use]
    pub fn missing_info(&self) -> Vec<VehicleId> {
        self.nodes
            .values()
            .filter(|node| node.info_missing)
            .map(|node| node.vehicle)
            .collect()
    }
}

impl Session {
    /// Asks the vehicle for what its nodes are.
    ///
    /// The command goes to the pair the link chose for the vehicle, not to an address somebody
    /// assumes, and nothing is waited for: the dictionary says these answers can be lost and that
    /// a station notices by seeing a status with no matching information, which is what the sheet
    /// shows, and upstream sends the command without waiting for an acknowledgement
    /// (`MAVLinkInterface.cs`, line 5289).
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the command.
    pub async fn send_node_info_request(&self, vehicle: VehicleId) -> Result<(), LinkError> {
        let payload = encode_command_long(
            vehicle.system_id,
            vehicle.component_id,
            GET_NODE_INFO_COMMAND,
            0,
            [0.0; 7],
        );

        self.write_raw(COMMAND_LONG_ID, &payload).await
    }
}

/// A status made from what an information message carries, for a node that answered with what
/// it is before it said how it is.
fn zero_status(info: &NodeInfo) -> NodeStatus {
    NodeStatus {
        time_usec: info.time_usec,
        uptime_sec: info.uptime_sec,
        health: 0,
        mode: 0,
        sub_mode: 0,
        vendor_specific_status_code: 0,
    }
}

/// The bytes of a board's number, as the file says them.
fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn read_u64(payload: &[u8], at: &mut usize) -> u64 {
    let window = window(payload, *at, 8);
    *at += 8;
    u64::from_le_bytes([
        window[0], window[1], window[2], window[3], window[4], window[5], window[6], window[7],
    ])
}

fn read_u32(payload: &[u8], at: &mut usize) -> u32 {
    let window = window(payload, *at, 4);
    *at += 4;
    u32::from_le_bytes([window[0], window[1], window[2], window[3]])
}

fn read_u16(payload: &[u8], at: &mut usize) -> u16 {
    let window = window(payload, *at, 2);
    *at += 2;
    u16::from_le_bytes([window[0], window[1]])
}

fn read_u8(payload: &[u8], at: &mut usize) -> u8 {
    let window = window(payload, *at, 1);
    *at += 1;
    window[0]
}

fn read_name(payload: &[u8], at: &mut usize, length: usize) -> String {
    let text = payload
        .get(*at..*at + length)
        .unwrap_or(&[])
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|byte| char::from(*byte))
        .collect::<String>();

    *at += length;

    text
}

/// The bytes of one field. MAVLink 2 removes the zeros at the end of a payload, and the
/// receiver pads them back, so a field past what arrived reads as zeros rather than as nothing.
fn window(payload: &[u8], at: usize, length: usize) -> [u8; 8] {
    let mut out = [0_u8; 8];

    for (index, byte) in out.iter_mut().enumerate().take(length) {
        if let Some(value) = payload.get(at + index) {
            *byte = *value;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use mpcraft_mavlink::dialect::{
        messages::{UavcanNodeHealth, UavcanNodeMode},
        Dialect, MavCmd, MavMessage,
    };
    use mpcraft_mavlink::MavlinkVersion;
    use mpcraft_mavlink::Message as _;
    use num_traits::FromPrimitive;

    /// The status as pymavlink's own generator wrote it, for the values recorded in
    /// `tests/dronecan.rs`. The payload is the fields in the order the wire uses them.
    fn status_payload() -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&1_700_000_000_000_000_u64.to_le_bytes());
        payload.extend_from_slice(&54_321_u32.to_le_bytes());
        payload.extend_from_slice(&0x1234_u16.to_le_bytes());
        payload.push(1); // health: WARNING
        payload.push(0); // mode: OPERATIONAL
        payload.push(5); // sub_mode
        payload
    }

    fn info_payload() -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&1_700_000_000_000_000_u64.to_le_bytes());
        payload.extend_from_slice(&54_321_u32.to_le_bytes());
        payload.extend_from_slice(&0xDDBBEE11_u32.to_le_bytes());
        let mut name = [0_u8; 80];
        for (index, byte) in b"MPCRAFT UCA-1".iter().enumerate() {
            name[index] = *byte;
        }
        payload.extend_from_slice(&name);
        payload.push(2);
        payload.push(3);
        payload.extend_from_slice(
            [
                0xDE, 0xAD, 0xBE, 0xEF, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
            ]
            .as_slice(),
        );
        payload.push(4);
        payload.push(7);
        payload
    }

    #[test]
    fn the_status_decodes_in_the_order_the_wire_writes_it() {
        let status = NodeStatus::decode(&status_payload());

        assert_eq!(status.time_usec, 1_700_000_000_000_000);
        assert_eq!(status.uptime_sec, 54_321);
        // The vendor's number comes before the three single bytes, because the wire writes the
        // widest field first even though the definition names it last.
        assert_eq!(status.vendor_specific_status_code, 0x1234);
        assert_eq!(status.health, 1);
        assert_eq!(status.mode, 0);
        assert_eq!(status.sub_mode, 5);
        assert_eq!(health_name(status.health), Some("WARNING"));
        assert_eq!(mode_name(status.mode), Some("OPERATIONAL"));
    }

    #[test]
    fn the_information_decodes_in_the_order_the_wire_writes_it() {
        let info = NodeInfo::decode(&info_payload());

        assert_eq!(info.name, "MPCRAFT UCA-1");
        // The commit is read before the name, which is where the wire puts it.
        assert_eq!(info.sw_vcs_commit, 0xDDBBEE11);
        assert_eq!(info.hw_version_major, 2);
        assert_eq!(info.hw_version_minor, 3);
        assert_eq!(info.hw_unique_id[0], 0xDE);
        assert_eq!(info.sw_version_major, 4);
        assert_eq!(info.sw_version_minor, 7);
    }

    #[test]
    fn a_number_the_dictionary_does_not_give_is_said_as_a_number() {
        // The dictionary gives four health values; a vehicle that says five is not given a word
        // the dictionary does not have.
        assert_eq!(health_name(5), None);
        assert_eq!(mode_name(9), None);
    }

    #[test]
    fn the_numbers_of_the_two_dictionaries_are_the_ones_the_compiled_dictionary_gives() {
        // The words this module writes are checked against the enumerations the generator builds
        // from the canonical XML, so a number is not written twice: if the dictionary changes, this
        // test is what says so.
        assert_eq!(UavcanNodeHealth::UAVCAN_NODE_HEALTH_OK as u32, 0);
        assert_eq!(UavcanNodeHealth::UAVCAN_NODE_HEALTH_WARNING as u32, 1);
        assert_eq!(UavcanNodeHealth::UAVCAN_NODE_HEALTH_ERROR as u32, 2);
        assert_eq!(UavcanNodeHealth::UAVCAN_NODE_HEALTH_CRITICAL as u32, 3);

        assert_eq!(UavcanNodeMode::UAVCAN_NODE_MODE_OPERATIONAL as u32, 0);
        assert_eq!(UavcanNodeMode::UAVCAN_NODE_MODE_INITIALIZATION as u32, 1);
        assert_eq!(UavcanNodeMode::UAVCAN_NODE_MODE_MAINTENANCE as u32, 2);
        assert_eq!(UavcanNodeMode::UAVCAN_NODE_MODE_SOFTWARE_UPDATE as u32, 3);
        assert_eq!(UavcanNodeMode::UAVCAN_NODE_MODE_OFFLINE as u32, 7);

        // The number is read from the dictionary rather than written as an identifier here, because
        // the dictionary also says this command is superseded — by `MAV_CMD_REQUEST_MESSAGE`, since
        // 2026-05 — so the ask travels through the raw path with the number it was given. Upstream
        // sends the same number, and a bus answers to it.
        let named = MavCmd::from_u32(GET_NODE_INFO_COMMAND);

        assert_eq!(
            named.map(|command| command as u32),
            Some(GET_NODE_INFO_COMMAND)
        );
        assert_eq!(format!("{named:?}"), "Some(MAV_CMD_UAVCAN_GET_NODE_INFO)");
    }

    #[test]
    fn the_byte_reader_and_the_generator_agree_on_the_same_payload() {
        // Two implementations that were not built from each other: the reader written from the
        // layout, and the bindings generated from the XML. They agree on every field of the golden
        // payload recorded in `tests/dronecan.rs`.
        let status = NodeStatus::decode(&status_payload());
        let decoded = Dialect::parse(MavlinkVersion::V2, NODE_STATUS_ID, &status_payload())
            .expect("the generator reads the same bytes");

        match decoded {
            MavMessage::UAVCAN_NODE_STATUS(message) => {
                assert_eq!(message.time_usec, status.time_usec);
                assert_eq!(message.uptime_sec, status.uptime_sec);
                assert_eq!(
                    message.vendor_specific_status_code,
                    status.vendor_specific_status_code
                );
                assert_eq!(message.health as u32, status.health as u32);
                assert_eq!(message.mode as u32, status.mode as u32);
                assert_eq!(message.sub_mode, status.sub_mode);
            }
            other => panic!("the generator read it as {other:?}"),
        }

        let info = NodeInfo::decode(&info_payload());
        let decoded = Dialect::parse(MavlinkVersion::V2, NODE_INFO_ID, &info_payload())
            .expect("the generator reads the same bytes");

        match decoded {
            MavMessage::UAVCAN_NODE_INFO(message) => {
                assert_eq!(
                    message.name.to_str().expect("the name is readable as text"),
                    info.name
                );
                assert_eq!(message.sw_vcs_commit, info.sw_vcs_commit);
                assert_eq!(message.hw_unique_id, info.hw_unique_id);
            }
            other => panic!("the generator read it as {other:?}"),
        }
    }

    #[test]
    fn a_health_the_generator_refuses_is_kept_as_what_the_vehicle_said() {
        // The generator answers `Invalid enum value for enum type "UavcanNodeHealth", got 5`, so a
        // node that reports a number outside the dictionary would never reach the sheet. The byte
        // reader keeps it — as a number, without a word the dictionary does not have.
        let mut payload = status_payload();
        payload[14] = 5; // health, at the place the wire puts it after the vendor's number
        payload[15] = 9; // mode

        assert!(
            Dialect::parse(MavlinkVersion::V2, NODE_STATUS_ID, &payload).is_err(),
            "the generator refuses this frame, which is why the reader works from the bytes"
        );

        let status = NodeStatus::decode(&payload);
        assert_eq!(status.health, 5);
        assert_eq!(status.mode, 9);
        assert_eq!(health_name(status.health), None);
        assert_eq!(mode_name(status.mode), None);
    }

    #[test]
    fn a_truncated_payload_pads_back_with_zeros() {
        // MAVLink 2 removes the zeros at the end of a payload. A status that arrives with only
        // its first two fields has no vendor number and no health, and reads as zeros rather
        // than as nothing.
        let mut short = Vec::new();
        short.extend_from_slice(&1_700_000_000_000_000_u64.to_le_bytes());
        short.extend_from_slice(&54_321_u32.to_le_bytes());

        let status = NodeStatus::decode(&short);
        assert_eq!(status.vendor_specific_status_code, 0);
        assert_eq!(status.health, 0);
        assert_eq!(status.sub_mode, 0);
    }

    #[test]
    fn the_first_status_of_a_node_is_the_moment_upstream_asks_for_what_it_is() {
        let mut registry = DronecanRegistry::new();
        let node = VehicleId::new(2, 40);

        assert!(registry.observe_status(node, &NodeStatus::decode(&status_payload())));
        // The same node reporting again is not a new node, so it is not asked again.
        assert!(!registry.observe_status(node, &NodeStatus::decode(&status_payload())));
    }

    #[test]
    fn a_node_that_has_not_said_what_it_is_is_kept_as_one_that_has_not() {
        let mut registry = DronecanRegistry::new();
        let node = VehicleId::new(2, 40);

        registry.observe_status(node, &NodeStatus::decode(&status_payload()));
        assert_eq!(registry.missing_info(), vec![node]);

        registry.observe_info(node, &NodeInfo::decode(&info_payload()));
        assert!(registry.missing_info().is_empty());

        let kept = registry.nodes();
        assert_eq!(kept[0].name.as_deref(), Some("MPCRAFT UCA-1"));
        assert_eq!(
            kept[0].hw_unique_id.as_deref(),
            Some("deadbeef0102030405060708090a0b0c")
        );
    }
}
