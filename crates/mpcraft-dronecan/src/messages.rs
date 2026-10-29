//! The few DroneCAN messages a station asks of a node, and the parameters it reads or writes.
//!
//! Each one is a data type of the DroneCAN standard, and each is defined by a DSDL file whose
//! published signature is what a bus checks a multi-frame transfer against. The signatures below
//! were computed from that DSDL by a second implementation (`pydronecan`'s
//! `get_data_type_signature()`), and are named here as constants rather than derived at run time:
//! they are facts about the standard, not about this crate.
//!
//! The fields follow the DSDL of each type, packed bit by bit as [`crate::bits`] describes. Two of
//! them deserve a note because they are where a station goes wrong:
//!
//! * `param.GetSet` sends `index` as thirteen bits, which is why the three-bit tag of the value that
//!   follows lands on a byte boundary — the comment in the standard's own DSDL says so.
//! * a variable-length array that ends a message carries **no length**: the length of the transfer
//!   says how long it is, and that is the only case where a node does not read a count first.

use crate::bits::{BitReader, BitWriter};

/// The DroneCAN data types this crate names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    /// How a node is, broadcast once a second.
    NodeStatus,
    /// What a node is: its versions, its unique id and its name.
    GetNodeInfo,
    /// Read or write one parameter of a node.
    ParameterGetSet,
    /// Save or erase the parameters a node keeps.
    ParameterExecuteOpcode,
    /// Restart a node.
    RestartNode,
}

impl DataType {
    /// The message or service id, as the DSDL file names it.
    #[must_use]
    pub const fn id(self) -> u16 {
        match self {
            Self::GetNodeInfo => 1,
            Self::RestartNode => 5,
            Self::ParameterExecuteOpcode => 10,
            Self::ParameterGetSet => 11,
            Self::NodeStatus => 341,
        }
    }

    /// The 64-bit signature of the type, computed from its DSDL by `pydronecan`.
    #[must_use]
    pub const fn signature(self) -> u64 {
        match self {
            Self::NodeStatus => 0x6CC9_93F8_0F6D_F80F,
            Self::GetNodeInfo => 0xEEF6_53B1_7FE6_493C,
            Self::ParameterGetSet => 0xA71A_635C_F734_A0DA,
            Self::ParameterExecuteOpcode => 0x38F4_8215_6692_9FDC,
            Self::RestartNode => 0x72BE_7031_1CD2_E2C3,
        }
    }

    /// Whether the type is a service, which is addressed to one node, rather than a message.
    #[must_use]
    pub const fn is_service(self) -> bool {
        !matches!(self, Self::NodeStatus)
    }

    /// The signature of a type by its id, for reading a bus that carries more than one.
    #[must_use]
    pub const fn signature_of(id: u16) -> Option<u64> {
        let found = match id {
            1 => Self::GetNodeInfo,
            5 => Self::RestartNode,
            10 => Self::ParameterExecuteOpcode,
            11 => Self::ParameterGetSet,
            341 => Self::NodeStatus,
            _ => return None,
        };

        Some(found.signature())
    }
}

/// How a node says it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeStatus {
    /// Seconds since the node started.
    pub uptime_sec: u32,
    /// 0 healthy, 1 warning, 2 error, 3 critical.
    pub health: u8,
    /// 0 operational, 1 initialization, 2 maintenance, 3 software update, 7 offline.
    pub mode: u8,
    /// A mode the node's own vendor defines.
    pub sub_mode: u8,
    /// Whatever the node's vendor puts here.
    pub vendor_specific_status_code: u16,
}

impl NodeStatus {
    /// Reads what a node broadcast.
    ///
    /// # Errors
    ///
    /// Returns `None` when the payload is shorter than the seven bytes the type takes.
    pub fn decode(payload: &[u8]) -> Option<Self> {
        let mut reader = BitReader::new(payload);

        Some(Self {
            uptime_sec: reader.uint(32)? as u32,
            health: reader.uint(2)? as u8,
            mode: reader.uint(3)? as u8,
            sub_mode: reader.uint(3)? as u8,
            vendor_specific_status_code: reader.uint(16)? as u16,
        })
    }
}

/// What a node is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeInfo {
    /// How it is, as the information message begins with a status.
    pub status: NodeStatus,
    /// The major version of its software.
    pub software_major: u8,
    /// The minor version of its software.
    pub software_minor: u8,
    /// The commit its software was built from, when it says one.
    pub vcs_commit: Option<u32>,
    /// The hash of its image, when it says one.
    pub image_crc: Option<u64>,
    /// The major version of its hardware.
    pub hardware_major: u8,
    /// The minor version of its hardware.
    pub hardware_minor: u8,
    /// The 128-bit id that is unique to it.
    pub unique_id: [u8; 16],
    /// The name it gives itself, as a reversed domain name.
    pub name: String,
}

impl NodeInfo {
    /// Reads a node's information.
    ///
    /// The two optional fields are always encoded — their flags only say whether the numbers mean
    /// anything — which is why the layout does not change with them.
    ///
    /// # Errors
    ///
    /// Returns `None` when the payload does not carry the whole type.
    pub fn decode(payload: &[u8]) -> Option<Self> {
        let mut reader = BitReader::new(payload);

        let status = NodeStatus {
            uptime_sec: reader.uint(32)? as u32,
            health: reader.uint(2)? as u8,
            mode: reader.uint(3)? as u8,
            sub_mode: reader.uint(3)? as u8,
            vendor_specific_status_code: reader.uint(16)? as u16,
        };

        let software_major = reader.uint(8)? as u8;
        let software_minor = reader.uint(8)? as u8;
        let flags = reader.uint(8)? as u8;
        let vcs_commit = reader.uint(32)? as u32;
        let image_crc = reader.uint(64)?;

        let hardware_major = reader.uint(8)? as u8;
        let hardware_minor = reader.uint(8)? as u8;

        let mut unique_id = [0_u8; 16];

        for byte in &mut unique_id {
            *byte = reader.uint(8)? as u8;
        }

        let certificate = reader.length(255)?;
        reader.skip(certificate * 8)?;

        // The name ends the message, so its length is the length of what is left.
        let remaining = payload.len() * 8 - reader.bits();
        let characters = remaining / 8;
        let name = reader.bytes(characters)?;
        let name = String::from_utf8_lossy(&name)
            .trim_end_matches('\0')
            .to_owned();

        Some(Self {
            status,
            software_major,
            software_minor,
            vcs_commit: (flags & 0x01 != 0).then_some(vcs_commit),
            image_crc: (flags & 0x02 != 0).then_some(image_crc),
            hardware_major,
            hardware_minor,
            unique_id,
            name,
        })
    }
}

/// A value a parameter holds.
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterValue {
    /// Nothing: the parameter does not exist, or the answer said nothing.
    Empty,
    /// A whole number.
    Integer(i64),
    /// A real number.
    Real(f32),
    /// A byte the node uses as a flag.
    Boolean(u8),
    /// Text.
    Text(String),
}

impl ParameterValue {
    /// Writes the three-bit tag and the field it selects.
    fn encode(&self, writer: &mut BitWriter) {
        match self {
            Self::Empty => writer.uint(0, 3),
            Self::Integer(value) => {
                writer.uint(1, 3);
                writer.int(*value, 64);
            }
            Self::Real(value) => {
                writer.uint(2, 3);
                writer.uint(value.to_bits().into(), 32);
            }
            Self::Boolean(value) => {
                writer.uint(3, 3);
                writer.uint(u64::from(*value), 8);
            }
            Self::Text(text) => {
                writer.uint(4, 3);
                let bytes = text.as_bytes();
                writer.length(bytes.len().min(128), 128);
                writer.bytes(&bytes[..bytes.len().min(128)]);
            }
        }
    }

    /// Reads the tag and the field it selects.
    fn decode(reader: &mut BitReader<'_>) -> Option<Self> {
        match reader.uint(3)? {
            0 => Some(Self::Empty),
            1 => Some(Self::Integer(reader.int(64)?)),
            2 => Some(Self::Real(f32::from_bits(reader.uint(32)? as u32))),
            3 => Some(Self::Boolean(reader.uint(8)? as u8)),
            4 => {
                let count = reader.length(128)?;

                Some(Self::Text(
                    String::from_utf8_lossy(&reader.bytes(count)?).into_owned(),
                ))
            }
            // A tag the type does not define is not a value this station can show.
            _ => None,
        }
    }

    /// What the sheet writes for it.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Integer(value) => value.to_string(),
            Self::Real(value) => value.to_string(),
            Self::Boolean(value) => (*value != 0).to_string(),
            Self::Text(text) => text.clone(),
        }
    }
}

/// The request for one parameter, by name or by index.
#[derive(Debug, Clone, PartialEq)]
pub struct GetSetRequest {
    /// The index, which is ignored when a name is given.
    pub index: u16,
    /// The value to write, or empty to read.
    pub value: ParameterValue,
    /// The name, which wins over the index when it is not empty.
    pub name: String,
}

impl GetSetRequest {
    /// Reads one parameter by name.
    #[must_use]
    pub fn read(name: &str) -> Self {
        Self {
            index: 0,
            value: ParameterValue::Empty,
            name: name.to_owned(),
        }
    }

    /// Writes one parameter by name.
    #[must_use]
    pub fn write(name: &str, value: ParameterValue) -> Self {
        Self {
            index: 0,
            value,
            name: name.to_owned(),
        }
    }

    /// Lists the parameters from one index onwards.
    #[must_use]
    pub const fn list(index: u16) -> Self {
        Self {
            index,
            value: ParameterValue::Empty,
            name: String::new(),
        }
    }

    /// The payload, as the request goes on the bus.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = BitWriter::new();

        writer.uint(u64::from(self.index & 0x1FFF), 13);
        self.value.encode(&mut writer);

        let name = self.name.as_bytes();

        // The name is the last field, so it carries no length of its own.
        writer.bytes(&name[..name.len().min(92)]);

        writer.finish()
    }
}

/// What a node answered about one parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct GetSetResponse {
    /// The value it holds now, which for a write is what it stored rather than what was asked.
    pub value: ParameterValue,
    /// The value it would have if nobody changed it.
    pub default_value: ParameterValue,
    /// The largest value it accepts, when it says one.
    pub max_value: ParameterValue,
    /// The smallest value it accepts, when it says one.
    pub min_value: ParameterValue,
    /// Its name, which is empty when there is no such parameter.
    pub name: String,
}

impl GetSetResponse {
    /// Reads the answer of a node.
    ///
    /// # Errors
    ///
    /// Returns `None` when the payload does not carry the whole type.
    pub fn decode(payload: &[u8]) -> Option<Self> {
        let mut reader = BitReader::new(payload);

        let value = ParameterValue::decode(&mut reader)?;

        // The standard pads between these fields so each one starts where the wire is cheap.
        reader.skip(5)?;

        let default_value = ParameterValue::decode(&mut reader)?;
        reader.skip(6)?;

        let max_value = numeric(&mut reader)?;
        reader.skip(6)?;

        let min_value = numeric(&mut reader)?;
        reader.skip(6)?;

        let remaining = payload.len() * 8 - reader.bits();
        let characters = remaining / 8;
        let name = reader.bytes(characters)?;

        Some(Self {
            value,
            default_value,
            max_value,
            min_value,
            name: String::from_utf8_lossy(&name)
                .trim_end_matches('\0')
                .to_owned(),
        })
    }
}

/// A number a node bounds a parameter with: its own union, whose tag is two bits.
fn numeric(reader: &mut BitReader<'_>) -> Option<ParameterValue> {
    match reader.uint(2)? {
        0 => Some(ParameterValue::Empty),
        1 => Some(ParameterValue::Integer(reader.int(64)?)),
        2 => Some(ParameterValue::Real(
            f32::from_bits(reader.uint(32)? as u32),
        )),
        _ => None,
    }
}

/// The request that saves or erases what a node keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecuteOpcode {
    /// Keep the parameters it holds across a power cycle.
    Save,
    /// Erase what it keeps.
    Erase,
}

impl ExecuteOpcode {
    /// The opcode as the standard numbers it.
    #[must_use]
    pub const fn opcode(self) -> u8 {
        match self {
            Self::Save => 0,
            Self::Erase => 1,
        }
    }

    /// The payload, as the request goes on the bus.
    #[must_use]
    pub fn encode(self) -> Vec<u8> {
        let mut writer = BitWriter::new();

        writer.uint(u64::from(self.opcode()), 8);
        writer.int(0, 48);

        writer.finish()
    }
}

/// The request that restarts a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartNode;

impl RestartNode {
    /// The number the standard requires for the request to be accepted.
    pub const MAGIC: u64 = 0x00AC_CE55_1B1E;

    /// The payload, as the request goes on the bus.
    #[must_use]
    pub fn encode(self) -> Vec<u8> {
        let mut writer = BitWriter::new();

        writer.uint(Self::MAGIC, 40);

        writer.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signatures_are_the_ones_a_second_implementation_computes() {
        // pydronecan's `get_data_type_signature()` over the DSDL of each type.
        assert_eq!(DataType::NodeStatus.signature(), 0x6CC9_93F8_0F6D_F80F);
        assert_eq!(DataType::GetNodeInfo.signature(), 0xEEF6_53B1_7FE6_493C);
        assert_eq!(DataType::ParameterGetSet.signature(), 0xA71A_635C_F734_A0DA);
        assert_eq!(
            DataType::ParameterExecuteOpcode.signature(),
            0x38F4_8215_6692_9FDC
        );
        assert_eq!(DataType::RestartNode.signature(), 0x72BE_7031_1CD2_E2C3);

        // And the ids, as the DSDL files name them.
        assert_eq!(DataType::GetNodeInfo.id(), 1);
        assert_eq!(DataType::RestartNode.id(), 5);
        assert_eq!(DataType::ParameterExecuteOpcode.id(), 10);
        assert_eq!(DataType::ParameterGetSet.id(), 11);
        assert_eq!(DataType::NodeStatus.id(), 341);

        assert_eq!(
            DataType::signature_of(11),
            Some(DataType::ParameterGetSet.signature())
        );
        assert_eq!(DataType::signature_of(999), None);
    }

    #[test]
    fn a_status_is_seven_bytes_and_reads_back() {
        let mut writer = BitWriter::new();
        writer.uint(4_242, 32);
        writer.uint(1, 2);
        writer.uint(0, 3);
        writer.uint(0, 3);
        writer.uint(0x1234, 16);

        let payload = writer.finish();
        assert_eq!(payload.len(), 7);

        let status = NodeStatus::decode(&payload).expect("the status is read");

        assert_eq!(status.uptime_sec, 4_242);
        assert_eq!(status.health, 1);
        assert_eq!(status.vendor_specific_status_code, 0x1234);
    }

    #[test]
    fn a_request_that_names_nothing_is_the_index_and_the_name() {
        // Index zero is thirteen bits and the empty value is a three-bit tag, so the two make two
        // bytes of nothing; the name is the last field and carries no length.
        let request = GetSetRequest::read("X");

        assert_eq!(request.encode(), vec![0x00, 0x00, b'X']);
    }

    #[test]
    fn a_write_names_the_parameter_and_carries_the_value() {
        let request = GetSetRequest::write("P", ParameterValue::Integer(1));
        let payload = request.encode();

        // Thirteen bits of index, three bits of tag for the integer, sixty-four bits of it, and one
        // byte of name: eighty-eight bits, so eleven bytes.
        assert_eq!(payload.len(), 11);

        let mut reader = BitReader::new(&payload);
        assert_eq!(reader.uint(13), Some(0));
        assert_eq!(reader.uint(3), Some(1));
        assert_eq!(reader.int(64), Some(1));
        assert_eq!(reader.bytes(1), Some(b"P".to_vec()));
    }

    #[test]
    fn an_answer_carries_the_value_its_default_and_its_bounds() {
        let mut writer = BitWriter::new();

        writer.uint(1, 3);
        writer.int(42, 64);
        writer.padding(5);
        writer.uint(2, 3);
        writer.uint(0.5_f32.to_bits().into(), 32);
        writer.padding(6);
        writer.uint(1, 2);
        writer.int(100, 64);
        writer.padding(6);
        writer.uint(0, 2);
        writer.padding(6);
        writer.bytes(b"ESC_RATE");

        let payload = writer.finish();
        let answer = GetSetResponse::decode(&payload).expect("the answer is read");

        assert_eq!(answer.value, ParameterValue::Integer(42));
        assert_eq!(answer.default_value, ParameterValue::Real(0.5));
        assert_eq!(answer.max_value, ParameterValue::Integer(100));
        assert_eq!(answer.min_value, ParameterValue::Empty);
        assert_eq!(answer.name, "ESC_RATE");
    }

    #[test]
    fn a_node_name_ends_the_information_and_carries_no_length() {
        let mut writer = BitWriter::new();

        // The status: seven bytes.
        writer.uint(10, 32);
        writer.uint(0, 2);
        writer.uint(0, 3);
        writer.uint(0, 3);
        writer.uint(0, 16);

        // The software version: major, minor, flags with both optional fields set, the commit and
        // the hash of the image.
        writer.uint(1, 8);
        writer.uint(2, 8);
        writer.uint(3, 8);
        writer.uint(0xABCD, 32);
        writer.uint(0x0123_4567_89AB_CDEF, 64);

        // The hardware version: major, minor, the unique id and an empty certificate.
        writer.uint(4, 8);
        writer.uint(5, 8);

        for byte in 0..16_u8 {
            writer.uint(u64::from(byte), 8);
        }

        writer.length(0, 255);

        writer.bytes(b"com.example.node");

        let payload = writer.finish();
        let info = NodeInfo::decode(&payload).expect("the information is read");

        assert_eq!(info.status.uptime_sec, 10);
        assert_eq!(info.software_major, 1);
        assert_eq!(info.software_minor, 2);
        assert_eq!(info.vcs_commit, Some(0xABCD));
        assert_eq!(info.image_crc, Some(0x0123_4567_89AB_CDEF));
        assert_eq!(info.hardware_major, 4);
        assert_eq!(info.hardware_minor, 5);
        assert_eq!(info.unique_id[15], 15);
        assert_eq!(info.name, "com.example.node");
    }

    #[test]
    fn a_restart_carries_the_magic_the_standard_asks_for() {
        let payload = RestartNode.encode();

        assert_eq!(payload.len(), 5);

        let mut reader = BitReader::new(&payload);
        assert_eq!(reader.uint(40), Some(RestartNode::MAGIC));
    }

    #[test]
    fn a_save_carries_its_opcode_and_an_empty_argument() {
        let payload = ExecuteOpcode::Save.encode();

        // One byte of opcode and forty-eight bits of argument: seven bytes.
        assert_eq!(payload.len(), 7);

        let mut reader = BitReader::new(&payload);
        assert_eq!(reader.uint(8), Some(0));
        assert_eq!(reader.int(48), Some(0));
    }
}
