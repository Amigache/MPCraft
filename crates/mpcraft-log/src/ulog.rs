//! The log of PX4: the `.ulg`.
//!
//! # What this is
//!
//! PX4 writes its flight log in a format of its own. The file begins with a header,
//! and what follows is a stream of messages: each of them says how long it is and
//! what it is, and then carries its own content.
//!
//! The log says what every topic is — its name, its fields and the type of each of
//! them — and the samples that follow carry the values:
//!
//! ```text
//! ULog\x01\x12\x35 | version | the instant the logging started
//! F  vehicle_gps_position:uint64_t timestamp;int32_t lat;int32_t lon;...
//! A  vehicle_gps_position -> the subscription 0
//! D  the subscription 0 | the values of a sample
//! ```
//!
//! The declaration of a topic is the message `F`, the announcement of a topic is the
//! message `A`, and a sample is the message `D`. The log also carries the
//! information of the vehicle (`I`), its parameters (`P`), the strings it printed
//! (`L`) and the places where messages were lost (`O`).
//!
//! # What it is for
//!
//! A flight of a PX4 is in this format, and a ground station that cannot read it
//! cannot say anything about the flight that has already happened. Upstream Mission
//! Planner ships a reader of it that is never wired to anything; this one is.
//!
//! # The units
//!
//! The log says the type of every value but not its unit: PX4 writes a latitude as a
//! whole number of hundred-millionths of a degree in the topics that were made when a
//! double was too much to carry, and as a double of degrees in the ones that came
//! later. The reader knows the convention of PX4 and gives the values in the unit
//! they belong to, so that whoever uses the crate reads degrees and metres and not
//! the numbers the autopilot happened to write. See [`units`].
//!
//! # What is not in the log
//!
//! The log does not say what a whole number means. The table of [`units`] is the
//! convention of PX4, and it is applied only to the fields of a whole number, because
//! a value of a floating type is already in its unit.

use std::collections::{BTreeMap, HashMap};
use std::io::Read;

use crate::dataflash::Value;
use crate::tlog::LogError;

pub mod units;

pub use units::FieldUnit;

/// The marker that begins the header of every log of PX4.
pub const MAGIC: [u8; 7] = [0x55, 0x4c, 0x6f, 0x67, 0x01, 0x12, 0x35];

/// The marker of the message that lets a reader find its way back after a corrupt one.
pub const SYNC: [u8; 8] = [0x2f, 0x73, 0x13, 0x20, 0x25, 0x0c, 0xbb, 0x12];

/// The size of the header of a log of PX4.
const HEADER_LEN: usize = 16;

/// The size of the header that comes before every message.
const MESSAGE_HEADER_LEN: usize = 3;

/// The biggest message a log of PX4 can hold.
///
/// The size of a message is a sixteen bit number, so a corrupt one cannot ask for
/// more than this.
const MAX_MESSAGE_LEN: usize = u16::MAX as usize;

/// How deep a message of a message may go.
///
/// The log says a message of a message cannot be circular, but a corrupt log can say
/// anything, and a reader that loops over a corrupt log is a reader that never
/// returns.
const MAX_NESTING: usize = 8;

/// The header of a log of PX4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ULogHeader {
    /// The version of the format the log is written in.
    pub version: u8,
    /// When the logging started, in microseconds since the vehicle booted.
    pub start_timestamp_us: u64,
}

/// The type of a value of a log of PX4.
///
/// The types are the ones of C, and they are the same in every topic: a position is
/// a double, a flag is a `bool`, and a text is a number of characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ULogType {
    /// A yes or a no.
    Bool,
    /// One character.
    Char,
    /// An eight-bit signed whole number.
    Int8,
    /// An eight-bit whole number.
    UInt8,
    /// A sixteen-bit signed whole number.
    Int16,
    /// A sixteen-bit whole number.
    UInt16,
    /// A thirty-two-bit signed whole number.
    Int32,
    /// A thirty-two-bit whole number.
    UInt32,
    /// A sixty-four-bit signed whole number.
    Int64,
    /// A sixty-four-bit whole number.
    UInt64,
    /// A single-precision number.
    Float,
    /// A double-precision number.
    Double,
    /// A topic of the log used as the type of a field.
    Nested(String),
    /// A fixed number of values of the same type.
    Array(Box<ULogType>, usize),
}

impl ULogType {
    /// The size in bytes of one value of the type, when it has one.
    ///
    /// A topic used as a type has no size of its own: it has the size of the fields
    /// it is made of.
    #[must_use]
    pub fn size(&self) -> Option<usize> {
        match self {
            ULogType::Bool | ULogType::Char | ULogType::Int8 | ULogType::UInt8 => Some(1),
            ULogType::Int16 | ULogType::UInt16 => Some(2),
            ULogType::Int32 | ULogType::UInt32 | ULogType::Float => Some(4),
            ULogType::Int64 | ULogType::UInt64 | ULogType::Double => Some(8),
            ULogType::Array(inner, count) => inner.size().map(|size| size * count),
            ULogType::Nested(_) => None,
        }
    }

    /// Whether the type is a whole number.
    #[must_use]
    pub fn is_integer(&self) -> bool {
        match self {
            ULogType::Int8
            | ULogType::UInt8
            | ULogType::Int16
            | ULogType::UInt16
            | ULogType::Int32
            | ULogType::UInt32
            | ULogType::Int64
            | ULogType::UInt64 => true,
            ULogType::Array(inner, _) => inner.is_integer(),
            ULogType::Bool
            | ULogType::Char
            | ULogType::Float
            | ULogType::Double
            | ULogType::Nested(_) => false,
        }
    }

    /// The name of the type, as the log writes it.
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            ULogType::Bool => "bool".to_owned(),
            ULogType::Char => "char".to_owned(),
            ULogType::Int8 => "int8_t".to_owned(),
            ULogType::UInt8 => "uint8_t".to_owned(),
            ULogType::Int16 => "int16_t".to_owned(),
            ULogType::UInt16 => "uint16_t".to_owned(),
            ULogType::Int32 => "int32_t".to_owned(),
            ULogType::UInt32 => "uint32_t".to_owned(),
            ULogType::Int64 => "int64_t".to_owned(),
            ULogType::UInt64 => "uint64_t".to_owned(),
            ULogType::Float => "float".to_owned(),
            ULogType::Double => "double".to_owned(),
            ULogType::Nested(name) => name.clone(),
            ULogType::Array(inner, count) => format!("{}[{}]", inner.name(), count),
        }
    }

    /// The type the log writes, when it is one of the known ones.
    fn of_name(name: &str) -> Option<Self> {
        match name {
            "bool" => Some(ULogType::Bool),
            "char" => Some(ULogType::Char),
            "int8_t" => Some(ULogType::Int8),
            "uint8_t" => Some(ULogType::UInt8),
            "int16_t" => Some(ULogType::Int16),
            "uint16_t" => Some(ULogType::UInt16),
            "int32_t" => Some(ULogType::Int32),
            "uint32_t" => Some(ULogType::UInt32),
            "int64_t" => Some(ULogType::Int64),
            "uint64_t" => Some(ULogType::UInt64),
            "float" => Some(ULogType::Float),
            "double" => Some(ULogType::Double),
            _ => None,
        }
    }
}

/// One field of a topic of a log of PX4.
///
/// A field is one column of the samples of the topic. A field of a list of values is
/// one column per value, and its name says which one it is: `q[0]`, `q[1]`, and so
/// on.
#[derive(Debug, Clone, PartialEq)]
pub struct ULogField {
    /// The name of the field.
    pub name: String,
    /// The type of the values of the field.
    pub kind: ULogType,
    /// The unit of the values, when it is known.
    pub unit: Option<String>,
    /// The factor the value the log holds was multiplied by.
    pub scale: f64,
    /// What was added to the value once it was multiplied.
    pub offset: f64,
}

/// What a log of PX4 says about one of its topics.
#[derive(Debug, Clone, PartialEq)]
pub struct ULogFormat {
    /// The name of the topic.
    pub name: String,
    /// The fields of the topic, one for each column of its samples.
    pub fields: Vec<ULogField>,
    /// The format as the log wrote it.
    pub format: String,
}

impl ULogFormat {
    /// The number of the field with that name, when the topic has one.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<usize> {
        self.fields
            .iter()
            .position(|field| field.name.eq_ignore_ascii_case(name))
    }

    /// The value of the field with that name in the given sample.
    #[must_use]
    pub fn value<'a>(&self, sample: &'a ULogSample, name: &str) -> Option<&'a Value> {
        self.field(name).and_then(|index| sample.values.get(index))
    }
}

/// One sample of a topic of a log of PX4.
#[derive(Debug, Clone, PartialEq)]
pub struct ULogSample {
    /// The name of the topic the sample belongs to.
    pub name: String,
    /// Which instance of the topic the sample is.
    ///
    /// A topic can be logged more than once — two sensors of the same kind, for
    /// instance — and the instances are told apart by this number.
    pub multi_id: u8,
    /// When the sample was taken, in microseconds since the vehicle booted.
    pub timestamp_us: u64,
    /// The values, in the order the format declares them.
    pub values: Vec<Value>,
}

/// A piece of information the log carries.
#[derive(Debug, Clone, PartialEq)]
pub struct ULogInfo {
    /// The name of the key, e.g. `ver_sw`.
    pub name: String,
    /// The type of the value.
    pub kind: ULogType,
    /// The value.
    pub value: Value,
}

/// A parameter of the vehicle, as the log records it.
#[derive(Debug, Clone, PartialEq)]
pub struct ULogParameter {
    /// The name of the parameter, e.g. `MPC_XY_PID`.
    pub name: String,
    /// The type of the value.
    pub kind: ULogType,
    /// The value.
    pub value: Value,
    /// The groups the default value belongs to, for a default parameter.
    ///
    /// One is the default of the system, two the default of the setup of the
    /// vehicle, and the two are told apart in the log.
    pub default_types: u8,
}

/// A string the vehicle printed.
#[derive(Debug, Clone, PartialEq)]
pub struct ULogMessage {
    /// How serious the message is, from zero to seven.
    pub level: u8,
    /// The tag of the message, when it has one.
    pub tag: Option<u16>,
    /// When it was printed, in microseconds since the vehicle booted.
    pub timestamp_us: u64,
    /// The text of the message.
    pub text: String,
}

impl ULogMessage {
    /// The name of the level of the message.
    ///
    /// The levels are the ones of the kernel of Linux, and PX4 uses the same ones.
    #[must_use]
    pub fn level_name(&self) -> &'static str {
        match self.level {
            0 => "EMERG",
            1 => "ALERT",
            2 => "CRIT",
            3 => "ERR",
            4 => "WARNING",
            5 => "NOTICE",
            6 => "INFO",
            _ => "DEBUG",
        }
    }
}

/// A place where the vehicle lost messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ULogDropout {
    /// How long the messages were lost, in milliseconds.
    pub duration_ms: u16,
}

/// A place of the log that could not be read.
///
/// A log of a vehicle that crashed ends in the middle of a message, and a log that
/// was written by a faulty disk has holes in it. Neither is a reason to throw the
/// rest of the log away, so the reader says where the log is broken and carries on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ULogDamage {
    /// Where in the log it is, in bytes from the beginning of the file.
    pub offset: u64,
    /// What is wrong with it.
    pub reason: String,
}

/// A topic that is logged, as the log announces it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Subscription {
    /// The name of the topic.
    topic: String,
    /// Which instance of the topic it is.
    multi_id: u8,
}

/// A field as the log declares it.
#[derive(Debug, Clone, PartialEq)]
struct DeclaredField {
    /// The name of the field.
    name: String,
    /// The type of the values.
    kind: ULogType,
}

/// A topic as the log declares it.
#[derive(Debug, Clone, PartialEq)]
struct DeclaredFormat {
    /// The name of the topic.
    name: String,
    /// The format as the log wrote it.
    format: String,
    /// The fields of the topic, as they were declared.
    fields: Vec<DeclaredField>,
}

/// One column of the samples of a topic.
#[derive(Debug, Clone)]
struct Column {
    /// The name of the column, e.g. `q[0]`.
    name: String,
    /// The type of the value.
    kind: ULogType,
    /// How many bytes the value takes.
    size: usize,
    /// Whether the column is only there to keep the alignment of the values.
    padding: bool,
    /// The unit of the value, when it is known.
    unit: Option<&'static str>,
    /// The factor the value the log holds is multiplied by.
    scale: f64,
    /// What is added to the value once it is multiplied.
    offset: f64,
}

/// A reader of the logs of PX4.
///
/// The reader is a stream: it hands out one sample at a time and never holds more
/// than the message it is on. The declarations of the topics, the information of the
/// vehicle and its parameters are kept as they are met, and the samples of a topic
/// are handed out as they come.
pub struct ULogReader<R: Read> {
    /// The bytes of the log.
    reader: R,
    /// The bytes that have been read and not consumed yet.
    buffer: Vec<u8>,
    /// Where the bytes that have not been consumed begin.
    start: usize,
    /// How many bytes of the log have been consumed.
    position: u64,
    /// The message that is being read.
    payload: Vec<u8>,
    /// The header of the log, once it has been read.
    header: Option<ULogHeader>,
    /// The topics as the log declares them.
    declared: HashMap<String, DeclaredFormat>,
    /// What the log says about its topics.
    formats: HashMap<String, ULogFormat>,
    /// The columns of the samples of each topic.
    layouts: HashMap<String, Vec<Column>>,
    /// The topics that are logged, by the number that identifies them.
    subscriptions: HashMap<u16, Subscription>,
    /// The information the log carries.
    info: BTreeMap<String, ULogInfo>,
    /// The information the log carries more than once.
    multi_info: Vec<ULogInfo>,
    /// The parameters the vehicle holds.
    parameters: BTreeMap<String, ULogParameter>,
    /// The default values of the parameters.
    defaults: BTreeMap<String, ULogParameter>,
    /// The strings the vehicle printed.
    messages: Vec<ULogMessage>,
    /// The places where messages were lost.
    dropouts: Vec<ULogDropout>,
    /// The places of the log that could not be read.
    damage: Vec<ULogDamage>,
    /// Whether the end of the log has been reached.
    finished: bool,
}

impl<R: Read> ULogReader<R> {
    /// A reader of the log in the given source.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
            start: 0,
            position: 0,
            payload: Vec::new(),
            header: None,
            declared: HashMap::new(),
            formats: HashMap::new(),
            layouts: HashMap::new(),
            subscriptions: HashMap::new(),
            info: BTreeMap::new(),
            multi_info: Vec::new(),
            parameters: BTreeMap::new(),
            defaults: BTreeMap::new(),
            messages: Vec::new(),
            dropouts: Vec::new(),
            damage: Vec::new(),
            finished: false,
        }
    }

    /// The header of the log, once it has been read.
    #[must_use]
    pub fn header(&self) -> Option<ULogHeader> {
        self.header
    }

    /// What the log says about its topics.
    #[must_use]
    pub fn formats(&self) -> &HashMap<String, ULogFormat> {
        &self.formats
    }

    /// The information the log carries.
    #[must_use]
    pub fn info(&self) -> &BTreeMap<String, ULogInfo> {
        &self.info
    }

    /// The information the log carries more than once.
    #[must_use]
    pub fn multi_info(&self) -> &[ULogInfo] {
        &self.multi_info
    }

    /// The parameters the vehicle holds.
    #[must_use]
    pub fn parameters(&self) -> &BTreeMap<String, ULogParameter> {
        &self.parameters
    }

    /// The default values of the parameters.
    #[must_use]
    pub fn defaults(&self) -> &BTreeMap<String, ULogParameter> {
        &self.defaults
    }

    /// The strings the vehicle printed.
    #[must_use]
    pub fn messages(&self) -> &[ULogMessage] {
        &self.messages
    }

    /// The places where messages were lost.
    #[must_use]
    pub fn dropouts(&self) -> &[ULogDropout] {
        &self.dropouts
    }

    /// The places of the log that could not be read.
    #[must_use]
    pub fn damage(&self) -> &[ULogDamage] {
        &self.damage
    }

    /// The next sample of the log, or nothing when the log has ended.
    ///
    /// The messages that are not samples are read and kept, so that they are there
    /// when the log has been read whole.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read, or when it does not hold a log
    /// of PX4.
    pub fn next_sample(&mut self) -> Result<Option<ULogSample>, LogError> {
        if self.finished {
            return Ok(None);
        }

        if self.header.is_none() {
            self.read_header()?;
        }

        loop {
            let Some(kind) = self.read_message()? else {
                return Ok(None);
            };

            match kind {
                b'F' => self.read_format(),
                b'A' => self.read_subscription(),
                b'R' => self.read_unsubscription(),
                b'I' => self.read_info(),
                b'M' => self.read_multi_info(),
                b'P' => self.read_parameter(),
                b'Q' => self.read_default_parameter(),
                b'L' => self.read_logged_string(),
                b'C' => self.read_tagged_string(),
                b'O' => self.read_dropout(),
                b'S' => {}
                b'B' => self.read_flag_bits()?,
                b'D' => {
                    if let Some(sample) = self.decode_data() {
                        return Ok(Some(sample));
                    }
                }
                _ => {}
            }
        }
    }

    /// Reads the header of the log.
    fn read_header(&mut self) -> Result<(), LogError> {
        if !self.fill(HEADER_LEN)? {
            return Err(LogError::NotALog(
                "the file is too short to hold the header of a log".to_owned(),
            ));
        }

        let head = self.unread();

        if head[..MAGIC.len()] != MAGIC {
            return Err(LogError::NotALog(
                "the file does not begin with the marker of a log of PX4".to_owned(),
            ));
        }

        let version = head[7];
        let timestamp = u64::from_le_bytes([
            head[8], head[9], head[10], head[11], head[12], head[13], head[14], head[15],
        ]);

        self.consume(HEADER_LEN);

        self.header = Some(ULogHeader {
            version,
            start_timestamp_us: timestamp,
        });

        Ok(())
    }

    /// Reads the next message of the log into the buffer of the reader.
    fn read_message(&mut self) -> Result<Option<u8>, LogError> {
        if !self.fill(MESSAGE_HEADER_LEN)? {
            self.finished = true;
            return Ok(None);
        }

        let head = self.unread();
        let size = u16::from_le_bytes([head[0], head[1]]) as usize;
        let kind = head[2];
        let total = MESSAGE_HEADER_LEN + size;

        if total > MAX_MESSAGE_LEN + MESSAGE_HEADER_LEN {
            self.damage.push(ULogDamage {
                offset: self.position,
                reason: format!("a message of {size} bytes is too long to be one"),
            });
            self.finished = true;
            return Ok(None);
        }

        if !self.fill(total)? {
            // The log ends in the middle of a message. Everything that came before is
            // still good, and the reader says where the log is broken.
            self.damage.push(ULogDamage {
                offset: self.position,
                reason: format!("the log ends in the middle of a message of {size} bytes"),
            });
            self.finished = true;
            return Ok(None);
        }

        self.payload.clear();
        self.payload
            .extend_from_slice(&self.buffer[self.start + MESSAGE_HEADER_LEN..self.start + total]);
        self.consume(total);

        Ok(Some(kind))
    }

    /// Reads the declaration of a topic.
    fn read_format(&mut self) {
        let Ok(declared) = parse_format(&self.payload) else {
            return;
        };

        self.declared.insert(declared.name.clone(), declared);
        self.rebuild();
    }

    /// Reads the announcement of a topic.
    fn read_subscription(&mut self) {
        if self.payload.len() < 3 {
            return;
        }

        let multi_id = self.payload[0];
        let id = u16::from_le_bytes([self.payload[1], self.payload[2]]);
        let topic = text_of(&self.payload[3..]);

        if topic.is_empty() {
            return;
        }

        self.subscriptions
            .insert(id, Subscription { topic, multi_id });
    }

    /// Reads the announcement that a topic is not logged any more.
    fn read_unsubscription(&mut self) {
        if self.payload.len() < 2 {
            return;
        }

        let id = u16::from_le_bytes([self.payload[0], self.payload[1]]);
        self.subscriptions.remove(&id);
    }

    /// Reads a piece of information about the vehicle.
    fn read_info(&mut self) {
        let Some((name, kind, value)) = read_key_value(&self.payload, 0) else {
            return;
        };

        self.info
            .insert(name.clone(), ULogInfo { name, kind, value });
    }

    /// Reads a piece of information that comes in more than one part.
    fn read_multi_info(&mut self) {
        let Some((name, kind, value)) = read_key_value(&self.payload, 1) else {
            return;
        };

        self.multi_info.push(ULogInfo { name, kind, value });
    }

    /// Reads a parameter of the vehicle.
    fn read_parameter(&mut self) {
        let Some((name, kind, value)) = read_key_value(&self.payload, 0) else {
            return;
        };

        self.parameters.insert(
            name.clone(),
            ULogParameter {
                name,
                kind,
                value,
                default_types: 0,
            },
        );
    }

    /// Reads the default value of a parameter.
    fn read_default_parameter(&mut self) {
        if self.payload.len() < 2 {
            return;
        }

        let default_types = self.payload[0];

        let Some((name, kind, value)) = read_key_value(&self.payload, 1) else {
            return;
        };

        self.defaults.insert(
            name.clone(),
            ULogParameter {
                name,
                kind,
                value,
                default_types,
            },
        );
    }

    /// Reads a string the vehicle printed.
    fn read_logged_string(&mut self) {
        if self.payload.len() < 9 {
            return;
        }

        let level = self.payload[0];
        let timestamp = u64::from_le_bytes([
            self.payload[1],
            self.payload[2],
            self.payload[3],
            self.payload[4],
            self.payload[5],
            self.payload[6],
            self.payload[7],
            self.payload[8],
        ]);

        self.messages.push(ULogMessage {
            level,
            tag: None,
            timestamp_us: timestamp,
            text: text_of(&self.payload[9..]),
        });
    }

    /// Reads a string the vehicle printed, with the tag of the process that printed it.
    fn read_tagged_string(&mut self) {
        if self.payload.len() < 11 {
            return;
        }

        let level = self.payload[0];
        let tag = u16::from_le_bytes([self.payload[1], self.payload[2]]);
        let timestamp = u64::from_le_bytes([
            self.payload[3],
            self.payload[4],
            self.payload[5],
            self.payload[6],
            self.payload[7],
            self.payload[8],
            self.payload[9],
            self.payload[10],
        ]);

        self.messages.push(ULogMessage {
            level,
            tag: Some(tag),
            timestamp_us: timestamp,
            text: text_of(&self.payload[11..]),
        });
    }

    /// Reads the place where the vehicle lost messages.
    fn read_dropout(&mut self) {
        if self.payload.len() < 2 {
            return;
        }

        let duration = u16::from_le_bytes([self.payload[0], self.payload[1]]);
        self.dropouts.push(ULogDropout {
            duration_ms: duration,
        });
    }

    /// Reads the flags that say what the log holds.
    fn read_flag_bits(&mut self) -> Result<(), LogError> {
        if self.payload.len() < 16 {
            return Ok(());
        }

        // The first eight bytes are the flags every reader understands, and the
        // second eight are the ones it does not. A log that sets a flag of the second
        // group holds something that breaks the format, and reading it would be
        // reading something else than what the log says it is.
        let unknown = self.payload[8..16]
            .iter()
            .enumerate()
            .filter(|(index, byte)| **byte != 0 && (*index != 0 || **byte & 0xFE != 0))
            .map(|(index, byte)| (index, *byte))
            .collect::<Vec<_>>();

        if !unknown.is_empty() {
            return Err(LogError::Unsupported(format!(
                "the log sets the flags {unknown:?}, which this reader does not understand"
            )));
        }

        Ok(())
    }

    /// Reads a sample of a topic.
    fn decode_data(&mut self) -> Option<ULogSample> {
        if self.payload.len() < 2 {
            return None;
        }

        let id = u16::from_le_bytes([self.payload[0], self.payload[1]]);

        let subscription = self.subscriptions.get(&id)?;

        let topic = subscription.topic.clone();
        let multi_id = subscription.multi_id;

        let columns = self.layouts.get(&topic)?;

        let values = decode_columns(columns, &self.payload[2..]);

        let timestamp_us = timestamp_of(columns, &values);

        Some(ULogSample {
            name: topic,
            multi_id,
            timestamp_us,
            values,
        })
    }

    /// Builds the columns and the description of every topic of the log.
    ///
    /// A topic can use another topic as the type of one of its fields, and the other
    /// one can be declared after it, so the columns of a topic are built again every
    /// time a topic is declared.
    fn rebuild(&mut self) {
        let declared = &self.declared;

        self.layouts.clear();
        self.formats.clear();

        for (name, format) in declared {
            let mut columns = Vec::new();

            if !build_columns(format, declared, name, "", 0, &mut columns) {
                continue;
            }

            self.layouts.insert(name.clone(), columns.clone());
            self.formats
                .insert(name.clone(), describe(format, &columns));
        }
    }

    /// The bytes that have been read and not consumed yet.
    fn unread(&self) -> &[u8] {
        &self.buffer[self.start..]
    }

    /// Makes sure that at least that many bytes are ready to be read.
    ///
    /// Says whether there are enough bytes: the end of the log is not an error.
    fn fill(&mut self, wanted: usize) -> Result<bool, LogError> {
        while self.unread().len() < wanted {
            if self.start > 0 {
                self.buffer.drain(..self.start);
                self.start = 0;
            }

            let mut chunk = [0_u8; 16 * 1024];
            let read = self.reader.read(&mut chunk)?;

            if read == 0 {
                return Ok(false);
            }

            self.buffer.extend_from_slice(&chunk[..read]);
        }

        Ok(true)
    }

    /// Throws away that many bytes of the log.
    fn consume(&mut self, count: usize) {
        self.start += count;
        self.position += count as u64;

        if self.start == self.buffer.len() {
            self.buffer.clear();
            self.start = 0;
        }
    }
}

/// A whole log of PX4, read and kept.
///
/// The reader of the log is the one that should be used for a log that does not fit
/// in memory: this one keeps every sample of every topic, and a log of a long flight
/// does not fit in memory.
pub struct ULog {
    /// The header of the log.
    header: ULogHeader,
    /// The topics the log holds.
    topics: Vec<ULogTopic>,
    /// The information the log carries.
    info: BTreeMap<String, ULogInfo>,
    /// The information the log carries more than once.
    multi_info: Vec<ULogInfo>,
    /// The parameters the vehicle holds.
    parameters: BTreeMap<String, ULogParameter>,
    /// The default values of the parameters.
    defaults: BTreeMap<String, ULogParameter>,
    /// The strings the vehicle printed.
    messages: Vec<ULogMessage>,
    /// The places where messages were lost.
    dropouts: Vec<ULogDropout>,
    /// The places of the log that could not be read.
    damage: Vec<ULogDamage>,
}

impl ULog {
    /// Reads a log of PX4 and keeps it whole.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read, or when it does not hold a log
    /// of PX4.
    pub fn read<R: Read>(reader: R) -> Result<Self, LogError> {
        let mut reader = ULogReader::new(reader);
        let mut topics: Vec<ULogTopic> = Vec::new();
        let mut index: HashMap<(String, u8), usize> = HashMap::new();

        while let Some(sample) = reader.next_sample()? {
            let key = (sample.name.clone(), sample.multi_id);

            let position = *index.entry(key).or_insert_with(|| {
                let format = reader
                    .formats()
                    .get(&sample.name)
                    .cloned()
                    .unwrap_or_else(|| ULogFormat {
                        name: sample.name.clone(),
                        fields: Vec::new(),
                        format: String::new(),
                    });

                topics.push(ULogTopic {
                    topic: sample.name.clone(),
                    multi_id: sample.multi_id,
                    format,
                    samples: Vec::new(),
                });

                topics.len() - 1
            });

            topics[position].samples.push(sample);
        }

        let header = reader.header().unwrap_or(ULogHeader {
            version: 0,
            start_timestamp_us: 0,
        });

        Ok(Self {
            header,
            topics,
            info: reader.info().clone(),
            multi_info: reader.multi_info().to_vec(),
            parameters: reader.parameters().clone(),
            defaults: reader.defaults().clone(),
            messages: reader.messages().to_vec(),
            dropouts: reader.dropouts().to_vec(),
            damage: reader.damage().to_vec(),
        })
    }

    /// The header of the log.
    #[must_use]
    pub fn header(&self) -> &ULogHeader {
        &self.header
    }

    /// The topics the log holds.
    #[must_use]
    pub fn topics(&self) -> &[ULogTopic] {
        &self.topics
    }

    /// The topic with that name, when the log holds one.
    #[must_use]
    pub fn topic(&self, name: &str) -> Option<&ULogTopic> {
        self.topics.iter().find(|topic| topic.topic == name)
    }

    /// The information the log carries.
    #[must_use]
    pub fn info(&self) -> &BTreeMap<String, ULogInfo> {
        &self.info
    }

    /// The information the log carries more than once.
    #[must_use]
    pub fn multi_info(&self) -> &[ULogInfo] {
        &self.multi_info
    }

    /// The parameters the vehicle holds.
    #[must_use]
    pub fn parameters(&self) -> &BTreeMap<String, ULogParameter> {
        &self.parameters
    }

    /// The default values of the parameters.
    #[must_use]
    pub fn defaults(&self) -> &BTreeMap<String, ULogParameter> {
        &self.defaults
    }

    /// The strings the vehicle printed.
    #[must_use]
    pub fn messages(&self) -> &[ULogMessage] {
        &self.messages
    }

    /// The places where messages were lost.
    #[must_use]
    pub fn dropouts(&self) -> &[ULogDropout] {
        &self.dropouts
    }

    /// The places of the log that could not be read.
    #[must_use]
    pub fn damage(&self) -> &[ULogDamage] {
        &self.damage
    }

    /// The instant of a sample, in microseconds since the epoch.
    ///
    /// The log counts the time from the moment the vehicle booted, and the vehicle
    /// says how far the clock of the board was from the clock of the world. A log
    /// that does not say has no instant but its own.
    #[must_use]
    pub fn epoch_us(&self, timestamp_us: u64) -> Option<u64> {
        let offset = self.info.get("time_ref_utc")?.value.number()?;

        let offset = if offset < 0.0 { 0.0 } else { offset };

        Some(
            self.header
                .start_timestamp_us
                .saturating_add(timestamp_us)
                .saturating_add((offset * 1_000_000.0) as u64),
        )
    }
}

/// A topic of a log of PX4, with the samples of it.
#[derive(Debug, Clone, PartialEq)]
pub struct ULogTopic {
    /// The name of the topic.
    pub topic: String,
    /// Which instance of the topic it is.
    pub multi_id: u8,
    /// What the log says about the topic.
    pub format: ULogFormat,
    /// The samples of the topic.
    pub samples: Vec<ULogSample>,
}

/// Reads the declaration of a topic out of the message that carries it.
fn parse_format(payload: &[u8]) -> Result<DeclaredFormat, LogError> {
    // A log writes the declaration as one line of text: the name of the topic, a
    // colon, and the fields separated by semicolons.
    if let Some(text) = as_text(payload) {
        if text.contains(':') {
            return parse_format_text(&text);
        }
    }

    // The oldest logs wrote the name of the topic and the fields with the length of
    // each of them in front.
    parse_legacy_format(payload)
}

/// Reads the declaration of a topic out of a line of text.
fn parse_format_text(text: &str) -> Result<DeclaredFormat, LogError> {
    let Some((name, body)) = text.split_once(':') else {
        return Err(LogError::Malformed(
            "the declaration of a topic has no name".to_owned(),
        ));
    };

    let name = name.trim();

    if name.is_empty() {
        return Err(LogError::Malformed(
            "the declaration of a topic has no name".to_owned(),
        ));
    }

    let mut fields = Vec::new();

    for field in body.split(';') {
        let field = field.trim();

        if field.is_empty() {
            continue;
        }

        let Some((kind, field_name)) = parse_field(field) else {
            return Err(LogError::Malformed(format!(
                "the declaration of the topic {name:?} has a field that cannot be read: {field:?}"
            )));
        };

        fields.push(DeclaredField {
            name: field_name,
            kind,
        });
    }

    Ok(DeclaredFormat {
        name: name.to_owned(),
        format: text.trim_end_matches(';').to_owned(),
        fields,
    })
}

/// Reads the declaration of a topic out of the oldest way of writing one.
fn parse_legacy_format(payload: &[u8]) -> Result<DeclaredFormat, LogError> {
    if payload.len() < 2 {
        return Err(LogError::Malformed(
            "the declaration of a topic is too short".to_owned(),
        ));
    }

    let name_length = usize::from(payload[0]);

    if payload.len() < 1 + name_length + 1 {
        return Err(LogError::Malformed(
            "the declaration of a topic is too short".to_owned(),
        ));
    }

    let name = text_of(&payload[1..1 + name_length]);
    let format_length = usize::from(payload[1 + name_length]);

    if payload.len() < 2 + name_length + format_length {
        return Err(LogError::Malformed(
            "the declaration of a topic is too short".to_owned(),
        ));
    }

    let format = text_of(&payload[2 + name_length..2 + name_length + format_length]);

    let mut fields = Vec::new();

    for field in format.split(';') {
        let field = field.trim();

        if field.is_empty() {
            continue;
        }

        let Some((kind, field_name)) = parse_field(field) else {
            return Err(LogError::Malformed(format!(
                "the declaration of the topic {name:?} has a field that cannot be read: {field:?}"
            )));
        };

        fields.push(DeclaredField {
            name: field_name,
            kind,
        });
    }

    Ok(DeclaredFormat {
        name,
        format,
        fields,
    })
}

/// Reads one field of a declaration.
///
/// A field is written as the type of its values and the name of the field, and a
/// list of values has the number of them in brackets after the type.
fn parse_field(field: &str) -> Option<(ULogType, String)> {
    let mut parts = field.split_whitespace();

    let type_text = parts.next()?;
    let name = parts.next()?;

    let (base, count) = match type_text.split_once('[') {
        Some((base, rest)) => {
            let count = rest.strip_suffix(']')?;
            (base, Some(count.parse::<usize>().ok()?))
        }
        None => (type_text, None),
    };

    let kind = ULogType::of_name(base).unwrap_or_else(|| ULogType::Nested(base.to_owned()));

    let kind = match count {
        Some(count) => ULogType::Array(Box::new(kind), count),
        None => kind,
    };

    Some((kind, name.to_owned()))
}

/// Builds the columns of the samples of a topic.
///
/// A field of a list of values is one column per value, and a topic used as the type
/// of a field is spread over the columns of its own fields. A field whose name
/// begins with `_padding` is not a column: it is only there to keep the alignment of
/// the values, and it is read but not shown.
fn build_columns(
    format: &DeclaredFormat,
    all: &HashMap<String, DeclaredFormat>,
    topic: &str,
    prefix: &str,
    depth: usize,
    out: &mut Vec<Column>,
) -> bool {
    if depth > MAX_NESTING {
        return false;
    }

    for field in &format.fields {
        let name = format!("{prefix}{}", field.name);

        if field.name.starts_with("_padding") {
            out.push(Column {
                name,
                kind: field.kind.clone(),
                size: field.kind.size().unwrap_or(0),
                padding: true,
                unit: None,
                scale: 1.0,
                offset: 0.0,
            });

            continue;
        }

        match &field.kind {
            ULogType::Array(inner, count) => {
                if matches!(**inner, ULogType::Char) {
                    // A text is one column, not one column per character.
                    out.push(Column {
                        name,
                        kind: field.kind.clone(),
                        size: inner.size().unwrap_or(0) * count,
                        padding: false,
                        unit: None,
                        scale: 1.0,
                        offset: 0.0,
                    });

                    continue;
                }

                if let ULogType::Nested(nested) = &**inner {
                    let Some(nested_format) = all.get(nested) else {
                        return false;
                    };

                    for index in 0..*count {
                        let nested_prefix = format!("{name}[{index}].");

                        if !build_columns(nested_format, all, topic, &nested_prefix, depth + 1, out)
                        {
                            return false;
                        }
                    }

                    continue;
                }

                let size = inner.size().unwrap_or(0);
                let (unit, scale, offset) = units::scale_of(topic, &field.name, inner);

                for index in 0..*count {
                    out.push(Column {
                        name: format!("{name}[{index}]"),
                        kind: (**inner).clone(),
                        size,
                        padding: false,
                        unit,
                        scale,
                        offset,
                    });
                }
            }
            ULogType::Nested(nested) => {
                let Some(nested_format) = all.get(nested) else {
                    return false;
                };

                let nested_prefix = format!("{name}.");

                if !build_columns(nested_format, all, topic, &nested_prefix, depth + 1, out) {
                    return false;
                }
            }
            _ => {
                let size = field.kind.size().unwrap_or(0);
                let (unit, scale, offset) = units::scale_of(topic, &field.name, &field.kind);

                out.push(Column {
                    name,
                    kind: field.kind.clone(),
                    size,
                    padding: false,
                    unit,
                    scale,
                    offset,
                });
            }
        }
    }

    true
}

/// Says what a topic is, out of its declaration and its columns.
fn describe(format: &DeclaredFormat, columns: &[Column]) -> ULogFormat {
    ULogFormat {
        name: format.name.clone(),
        fields: columns
            .iter()
            .filter(|column| !column.padding)
            .map(|column| ULogField {
                name: column.name.clone(),
                kind: column.kind.clone(),
                unit: column.unit.map(str::to_owned),
                scale: column.scale,
                offset: column.offset,
            })
            .collect(),
        format: format.format.clone(),
    }
}

/// Reads the values of a sample out of the bytes of the message that carries it.
fn decode_columns(columns: &[Column], data: &[u8]) -> Vec<Value> {
    let mut values = Vec::new();
    let mut offset = 0_usize;

    for (index, column) in columns.iter().enumerate() {
        let remaining = data.len().saturating_sub(offset);

        if remaining < column.size {
            if column.padding {
                // The padding at the end of a message is not always written.
                break;
            }

            // The sample is shorter than the declaration says. The values that are
            // missing are said as unknown rather than as zero: a zero of a height is
            // a height on the ground.
            for other in &columns[index..] {
                if !other.padding {
                    values.push(Value::Number(f64::NAN));
                }
            }

            break;
        }

        let bytes = &data[offset..offset + column.size];
        values.push(read_value(&column.kind, bytes, column.scale, column.offset));
        offset += column.size;
    }

    values
}

/// Reads one value with the type it was declared with.
fn read_value(kind: &ULogType, bytes: &[u8], scale: f64, offset: f64) -> Value {
    let number = match kind {
        ULogType::Bool => {
            if bytes.first().copied().unwrap_or(0) != 0 {
                1.0
            } else {
                0.0
            }
        }
        ULogType::Char => return Value::Text(text_of(bytes)),
        ULogType::Int8 => f64::from(i8::from_le_bytes([bytes.first().copied().unwrap_or(0)])),
        ULogType::UInt8 => f64::from(bytes.first().copied().unwrap_or(0)),
        ULogType::Int16 => f64::from(i16::from_le_bytes([
            bytes.first().copied().unwrap_or(0),
            *bytes.get(1).unwrap_or(&0),
        ])),
        ULogType::UInt16 => f64::from(u16::from_le_bytes([
            bytes.first().copied().unwrap_or(0),
            *bytes.get(1).unwrap_or(&0),
        ])),
        ULogType::Int32 => f64::from(i32::from_le_bytes([
            bytes.first().copied().unwrap_or(0),
            *bytes.get(1).unwrap_or(&0),
            *bytes.get(2).unwrap_or(&0),
            *bytes.get(3).unwrap_or(&0),
        ])),
        ULogType::UInt32 => f64::from(u32::from_le_bytes([
            bytes.first().copied().unwrap_or(0),
            *bytes.get(1).unwrap_or(&0),
            *bytes.get(2).unwrap_or(&0),
            *bytes.get(3).unwrap_or(&0),
        ])),
        ULogType::Int64 => i64::from_le_bytes([
            bytes.first().copied().unwrap_or(0),
            *bytes.get(1).unwrap_or(&0),
            *bytes.get(2).unwrap_or(&0),
            *bytes.get(3).unwrap_or(&0),
            *bytes.get(4).unwrap_or(&0),
            *bytes.get(5).unwrap_or(&0),
            *bytes.get(6).unwrap_or(&0),
            *bytes.get(7).unwrap_or(&0),
        ]) as f64,
        ULogType::UInt64 => u64::from_le_bytes([
            bytes.first().copied().unwrap_or(0),
            *bytes.get(1).unwrap_or(&0),
            *bytes.get(2).unwrap_or(&0),
            *bytes.get(3).unwrap_or(&0),
            *bytes.get(4).unwrap_or(&0),
            *bytes.get(5).unwrap_or(&0),
            *bytes.get(6).unwrap_or(&0),
            *bytes.get(7).unwrap_or(&0),
        ]) as f64,
        ULogType::Float => f64::from(f32::from_le_bytes([
            bytes.first().copied().unwrap_or(0),
            *bytes.get(1).unwrap_or(&0),
            *bytes.get(2).unwrap_or(&0),
            *bytes.get(3).unwrap_or(&0),
        ])),
        ULogType::Double => f64::from_le_bytes([
            bytes.first().copied().unwrap_or(0),
            *bytes.get(1).unwrap_or(&0),
            *bytes.get(2).unwrap_or(&0),
            *bytes.get(3).unwrap_or(&0),
            *bytes.get(4).unwrap_or(&0),
            *bytes.get(5).unwrap_or(&0),
            *bytes.get(6).unwrap_or(&0),
            *bytes.get(7).unwrap_or(&0),
        ]),
        ULogType::Array(inner, _) => {
            if matches!(**inner, ULogType::Char) {
                return Value::Text(text_of(bytes));
            }

            return Value::Text(text_of(bytes));
        }
        ULogType::Nested(_) => 0.0,
    };

    Value::Number(number * scale + offset)
}

/// The instant of a sample, out of the values of it.
fn timestamp_of(columns: &[Column], values: &[Value]) -> u64 {
    let Some(index) = columns
        .iter()
        .position(|column| column.name == "timestamp" && !column.padding)
    else {
        return 0;
    };

    values
        .get(index)
        .and_then(Value::number)
        .map(|number| number.max(0.0) as u64)
        .unwrap_or(0)
}

/// Reads a key and a value out of an information or a parameter message.
///
/// The key says the type of the value and the name of the key, and the value
/// follows it.
fn read_key_value(payload: &[u8], skip: usize) -> Option<(String, ULogType, Value)> {
    let length = usize::from(*payload.get(skip)?);
    let key_start = skip + 1;
    let key_end = key_start + length;

    if payload.len() < key_end {
        return None;
    }

    let key = text_of(&payload[key_start..key_end]);

    let (type_text, name) = key.split_once(' ')?;

    let (kind, count) = match type_text.split_once('[') {
        Some((base, rest)) => {
            let count = rest
                .strip_suffix(']')
                .and_then(|count| count.parse::<usize>().ok());
            (base, count)
        }
        None => (type_text, None),
    };

    let base = ULogType::of_name(kind).unwrap_or_else(|| ULogType::Nested(kind.to_owned()));

    let kind = match count {
        Some(count) => ULogType::Array(Box::new(base), count),
        None => base,
    };

    let value = read_value(&kind, &payload[key_end..], 1.0, 0.0);

    Some((name.trim().to_owned(), kind, value))
}

/// The text of some bytes, without the zeroes that end it.
fn text_of(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());

    String::from_utf8_lossy(&bytes[..end]).trim_end().to_owned()
}

/// The text of some bytes, when they are text.
fn as_text(bytes: &[u8]) -> Option<String> {
    if bytes
        .iter()
        .all(|byte| *byte == 0 || byte.is_ascii_graphic() || *byte == b' ' || *byte == b'\t')
    {
        Some(text_of(bytes))
    } else {
        None
    }
}

/// What is known about the unit of a field, for the caller that wants to know.
#[must_use]
pub fn unit_of(topic: &str, field: &str) -> Option<&'static FieldUnit> {
    units::unit_of(topic, field)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The header of a log, as it is written.
    fn header(version: u8, timestamp: u64) -> Vec<u8> {
        let mut bytes = MAGIC.to_vec();
        bytes.push(version);
        bytes.extend_from_slice(&timestamp.to_le_bytes());
        bytes
    }

    /// A message of the log, as it is written.
    fn message(kind: u8, payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(payload.len() + 3);
        bytes.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        bytes.push(kind);
        bytes.extend_from_slice(payload);
        bytes
    }

    /// The declaration of a topic, as it is written.
    fn format(name: &str, fields: &str) -> Vec<u8> {
        message(b'F', format!("{name}:{fields}").as_bytes())
    }

    /// The announcement of a topic, as it is written.
    fn subscription(multi_id: u8, id: u16, name: &str) -> Vec<u8> {
        let mut payload = vec![multi_id];
        payload.extend_from_slice(&id.to_le_bytes());
        payload.extend_from_slice(name.as_bytes());
        message(b'A', &payload)
    }

    /// A sample of a topic, as it is written.
    fn sample(id: u16, data: &[u8]) -> Vec<u8> {
        let mut payload = id.to_le_bytes().to_vec();
        payload.extend_from_slice(data);
        message(b'D', &payload)
    }

    /// A parameter, as it is written.
    fn parameter(kind: &str, name: &str, value: &[u8]) -> Vec<u8> {
        let key = format!("{kind} {name}");
        let mut payload = vec![key.len() as u8];
        payload.extend_from_slice(key.as_bytes());
        payload.extend_from_slice(value);
        message(b'P', &payload)
    }

    /// A piece of information, as it is written.
    fn information(key: &str, value: &[u8]) -> Vec<u8> {
        let mut payload = vec![key.len() as u8];
        payload.extend_from_slice(key.as_bytes());
        payload.extend_from_slice(value);
        message(b'I', &payload)
    }

    #[test]
    fn the_header_of_the_log_is_read() {
        let bytes = [header(1, 20_309_082), format("a", "uint64_t timestamp")].concat();

        let mut reader = ULogReader::new(&bytes[..]);

        let header = reader.header();

        assert_eq!(header, None);

        while reader.next_sample().expect("the log is read").is_some() {}

        let header = reader.header().expect("the log has a header");

        assert_eq!(header.version, 1);
        assert_eq!(header.start_timestamp_us, 20_309_082);
    }

    #[test]
    fn the_declaration_of_a_topic_is_read() {
        let bytes = [
            header(1, 0),
            format(
                "vehicle_gps_position",
                "uint64_t timestamp;int32_t lat;int32_t lon;int32_t alt;uint8_t fix_type",
            ),
        ]
        .concat();

        let mut reader = ULogReader::new(&bytes[..]);

        while reader.next_sample().expect("the log is read").is_some() {}

        let formats = reader.formats();
        let format = formats
            .get("vehicle_gps_position")
            .expect("the log declares the topic");

        assert_eq!(format.name, "vehicle_gps_position");
        assert_eq!(format.fields.len(), 5);
        assert_eq!(format.fields[0].name, "timestamp");
        assert_eq!(format.fields[0].kind, ULogType::UInt64);
        assert_eq!(format.fields[1].kind, ULogType::Int32);
        assert_eq!(format.field("lat"), Some(1));

        // The unit of a position is known, and the value of the log is scaled to it.
        assert_eq!(format.fields[1].unit.as_deref(), Some("deg"));
        assert_eq!(format.fields[1].scale, 1e-7);
        assert_eq!(format.fields[3].unit.as_deref(), Some("m"));
    }

    #[test]
    fn the_samples_of_a_topic_are_read() {
        let mut data = Vec::new();
        data.extend_from_slice(&1_000_000_u64.to_le_bytes());
        data.extend_from_slice(&(-35_354_718_i32).to_le_bytes());
        data.extend_from_slice(&149_169_667_i32.to_le_bytes());
        data.extend_from_slice(&88_552_i32.to_le_bytes());
        data.push(3);

        let bytes = [
            header(1, 0),
            format(
                "vehicle_gps_position",
                "uint64_t timestamp;int32_t lat;int32_t lon;int32_t alt;uint8_t fix_type",
            ),
            subscription(0, 0, "vehicle_gps_position"),
            sample(0, &data),
        ]
        .concat();

        let mut reader = ULogReader::new(&bytes[..]);

        let sample = reader
            .next_sample()
            .expect("the log is read")
            .expect("there is a sample");

        assert_eq!(sample.name, "vehicle_gps_position");
        assert_eq!(sample.multi_id, 0);
        assert_eq!(sample.timestamp_us, 1_000_000);
        assert_eq!(sample.values.len(), 5);

        // The latitude is given in degrees and not in the hundred-millionths of a
        // degree the log holds.
        let latitude = sample.values[1].number().expect("the latitude is a number");
        assert!((latitude - -3.5354718).abs() < 1e-9);

        let longitude = sample.values[2]
            .number()
            .expect("the longitude is a number");
        assert!((longitude - 14.9169667).abs() < 1e-9);

        // The height is given in metres and not in the millimetres the log holds.
        let altitude = sample.values[3].number().expect("the height is a number");
        assert!((altitude - 88.552).abs() < 1e-9);
    }

    #[test]
    fn the_parameters_of_the_vehicle_are_read() {
        let bytes = [
            header(1, 0),
            parameter("int32_t", "ASPD_BETA_GATE", &1_i32.to_le_bytes()),
            parameter("float", "MPC_XY_PID", &(-5.0_f32).to_le_bytes()),
        ]
        .concat();

        let mut reader = ULogReader::new(&bytes[..]);

        while reader.next_sample().expect("the log is read").is_some() {}

        let parameters = reader.parameters();

        let gate = parameters
            .get("ASPD_BETA_GATE")
            .expect("the log holds the parameter");

        assert_eq!(gate.kind, ULogType::Int32);
        assert_eq!(gate.value.number(), Some(1.0));

        let pid = parameters
            .get("MPC_XY_PID")
            .expect("the log holds the parameter");

        assert_eq!(pid.kind, ULogType::Float);
        assert_eq!(pid.value.number(), Some(-5.0));
    }

    #[test]
    fn the_information_of_the_vehicle_is_read() {
        let bytes = [
            header(1, 0),
            information("char[3] sys_name", b"PX4"),
            information("uint32_t ver_sw_release", &0x0104_02FF_u32.to_le_bytes()),
        ]
        .concat();

        let mut reader = ULogReader::new(&bytes[..]);

        while reader.next_sample().expect("the log is read").is_some() {}

        let info = reader.info();

        let name = info.get("sys_name").expect("the log holds the name");
        assert_eq!(name.kind, ULogType::Array(Box::new(ULogType::Char), 3));
        assert_eq!(name.value, Value::Text("PX4".to_owned()));

        let release = info
            .get("ver_sw_release")
            .expect("the log holds the release");
        assert_eq!(release.value.number(), Some(0x0104_02FF_u32 as f64));
    }

    #[test]
    fn the_strings_of_the_vehicle_are_read() {
        let mut payload = vec![6_u8];
        payload.extend_from_slice(&1_000_u64.to_le_bytes());
        payload.extend_from_slice(b"[commander] Takeoff detected");

        let bytes = [header(1, 0), message(b'L', &payload)].concat();

        let mut reader = ULogReader::new(&bytes[..]);

        while reader.next_sample().expect("the log is read").is_some() {}

        let messages = reader.messages();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].level, 6);
        assert_eq!(messages[0].level_name(), "INFO");
        assert_eq!(messages[0].timestamp_us, 1_000);
        assert_eq!(messages[0].text, "[commander] Takeoff detected");
    }

    #[test]
    fn the_places_where_messages_were_lost_are_read() {
        let bytes = [header(1, 0), message(b'O', &30_u16.to_le_bytes())].concat();

        let mut reader = ULogReader::new(&bytes[..]);

        while reader.next_sample().expect("the log is read").is_some() {}

        assert_eq!(reader.dropouts().len(), 1);
        assert_eq!(reader.dropouts()[0].duration_ms, 30);
    }

    #[test]
    fn a_list_of_values_is_one_column_per_value() {
        let bytes = [
            header(1, 0),
            format(
                "vehicle_attitude",
                "uint64_t timestamp;float[4] q;uint8_t[4] _padding0",
            ),
        ]
        .concat();

        let mut reader = ULogReader::new(&bytes[..]);

        while reader.next_sample().expect("the log is read").is_some() {}

        let formats = reader.formats();
        let format = formats
            .get("vehicle_attitude")
            .expect("the log declares the topic");

        let names: Vec<&str> = format
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect();

        assert_eq!(names, vec!["timestamp", "q[0]", "q[1]", "q[2]", "q[3]"]);
    }

    #[test]
    fn the_padding_at_the_end_of_a_message_may_not_be_written() {
        let mut data = Vec::new();
        data.extend_from_slice(&1_000_u64.to_le_bytes());
        data.extend_from_slice(&1.0_f32.to_le_bytes());

        let bytes = [
            header(1, 0),
            format(
                "a_topic",
                "uint64_t timestamp;float value;uint8_t[4] _padding0",
            ),
            subscription(0, 0, "a_topic"),
            sample(0, &data),
        ]
        .concat();

        let mut reader = ULogReader::new(&bytes[..]);

        let sample = reader
            .next_sample()
            .expect("the log is read")
            .expect("there is a sample");

        assert_eq!(sample.values.len(), 2);
        assert_eq!(sample.values[1].number(), Some(1.0));
    }

    #[test]
    fn a_log_that_ends_in_the_middle_of_a_message_is_read_up_to_there() {
        let mut data = Vec::new();
        data.extend_from_slice(&1_000_u64.to_le_bytes());

        let mut bytes = [
            header(1, 0),
            format("a_topic", "uint64_t timestamp;float value"),
            subscription(0, 0, "a_topic"),
            sample(0, &data),
        ]
        .concat();

        // The last message is cut in half.
        bytes.extend_from_slice(&message(b'D', &[0, 0, 1, 2, 3, 4])[..3]);

        let mut reader = ULogReader::new(&bytes[..]);

        let mut samples = 0;

        while reader
            .next_sample()
            .expect("the log is read up to the end")
            .is_some()
        {
            samples += 1;
        }

        assert_eq!(samples, 1);

        // The place where the log is broken is said, and the rest of the log is not
        // thrown away.
        let damage = reader.damage();
        assert_eq!(damage.len(), 1);
        assert!(damage[0].reason.contains("ends in the middle"));
    }

    #[test]
    fn a_file_that_is_not_a_log_is_said_so() {
        let bytes = b"this is not a log of PX4 at all";

        let mut reader = ULogReader::new(&bytes[..]);

        assert!(matches!(reader.next_sample(), Err(LogError::NotALog(_))));
    }

    #[test]
    fn a_log_that_sets_a_flag_it_does_not_know_is_refused() {
        let mut payload = vec![0_u8; 40];
        payload[8] = 0x02;

        let bytes = [header(1, 0), message(b'B', &payload)].concat();

        let mut reader = ULogReader::new(&bytes[..]);

        assert!(matches!(
            reader.next_sample(),
            Err(LogError::Unsupported(_))
        ));
    }

    #[test]
    fn a_whole_log_holds_its_topics_and_their_samples() {
        let mut data = Vec::new();
        data.extend_from_slice(&1_000_u64.to_le_bytes());
        data.extend_from_slice(&2.5_f32.to_le_bytes());

        let bytes = [
            header(1, 0),
            format("a_topic", "uint64_t timestamp;float value"),
            subscription(0, 0, "a_topic"),
            sample(0, &data),
            sample(0, &data),
        ]
        .concat();

        let log = ULog::read(&bytes[..]).expect("the log is read");

        assert_eq!(log.topics().len(), 1);
        assert_eq!(log.topic("a_topic").expect("the topic").samples.len(), 2);
        assert_eq!(log.header().version, 1);
    }
}
