//! The text log of DataFlash: the `.log`.
//!
//! # What this is
//!
//! ArduPilot writes its flight log as text: a line per sample, with the name of the
//! message first and its values after it. The first lines of the log say what every
//! message is — how many values it has, what type each of them is and what it is
//! called — and the lines that follow carry the samples.
//!
//! The declarations are the `FMT` lines:
//!
//! ```text
//! FMT, 128, 89, FMT, BBnNZ, Type,Length,Name,Format
//! FMT, 130, 45, GPS, BIHBcLLeeEefI, Status,TimeMS,Week,NSats,HDop,Lat,Lng,...
//! GPS, 3, 130040903, 1769, 10, 0.00, -35.3547178, 149.1696673, 885.52, ...
//! ```
//!
//! The `FMT` line says the number of the message, the length of it, its name, the type
//! of each of its values and the name of each of them. The samples that follow are
//! read with those types: a value that is not read with the type it was declared with
//! is a value the operator would read wrong.
//!
//! # What it is for
//!
//! A flight of an ArduPilot is in this format more often than in any other, and a
//! ground station that cannot read it cannot say anything about the flight that has
//! already happened.

use std::collections::HashMap;
use std::io::BufRead;

use crate::tlog::LogError;

/// One of the types a value of a DataFlash message can have.
///
/// The types are the ones the autopilot declares in its `FMT` lines, and they are the
/// same in the text log and in the binary one. The scaled types are scaled the same way
/// in both: a value of the type `e` is a hundredth, and the reader says the hundredth
/// it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// An eight-bit unsigned number.
    U8,
    /// An eight-bit signed number.
    I8,
    /// A sixteen-bit unsigned number.
    U16,
    /// A sixteen-bit signed number.
    I16,
    /// A thirty-two-bit unsigned number.
    U32,
    /// A thirty-two-bit signed number.
    I32,
    /// A sixty-four-bit unsigned number.
    U64,
    /// A sixty-four-bit signed number.
    I64,
    /// A single-precision number.
    F32,
    /// A double-precision number.
    F64,
    /// A number in hundredths.
    Cents,
    /// A position in millionths of a degree.
    Location,
    /// A half-precision number.
    Half,
    /// A text of the given length.
    Text(usize),
}

impl FieldKind {
    /// The kind a format character means.
    pub fn of(character: char) -> Option<Self> {
        match character {
            'B' => Some(Self::U8),
            'b' => Some(Self::I8),
            'H' => Some(Self::U16),
            'h' => Some(Self::I16),
            'I' => Some(Self::U32),
            'i' => Some(Self::I32),
            'Q' => Some(Self::U64),
            'q' => Some(Self::I64),
            'e' | 'E' => Some(Self::Cents),
            'f' => Some(Self::F32),
            'd' => Some(Self::F64),
            'L' => Some(Self::Location),
            'g' => Some(Self::Half),
            'n' => Some(Self::Text(4)),
            'N' => Some(Self::Text(16)),
            'M' => Some(Self::Text(1)),
            'Z' => Some(Self::Text(64)),
            'a' => Some(Self::Text(128)),
            _ => None,
        }
    }
}

/// The value of one field of one sample.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A number.
    Number(f64),
    /// A text.
    Text(String),
}

impl Value {
    /// The number, when the value is one.
    #[must_use]
    pub fn number(&self) -> Option<f64> {
        match self {
            Value::Number(value) => Some(*value),
            Value::Text(_) => None,
        }
    }
}

/// What the log says about one of its messages.
#[derive(Debug, Clone, PartialEq)]
pub struct MessageFormat {
    /// The number of the message.
    pub id: u8,
    /// How long the message is, the header counted.
    ///
    /// The text log does not use it ? a line is a line ? but the binary one does: the
    /// values of a message are three bytes shorter than the message, because the marker
    /// and the number of the message are part of it.
    pub length: u8,
    /// The name of the message.
    pub name: String,
    /// The name of each of its values.
    pub fields: Vec<String>,
    /// The type of each of its values.
    pub kinds: Vec<FieldKind>,
}

impl MessageFormat {
    /// The number of the field with that name, when the message has one.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<usize> {
        self.fields
            .iter()
            .position(|field| field.eq_ignore_ascii_case(name))
    }
}

/// One sample of a message.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// The name of the message.
    pub name: String,
    /// The values, in the order the message declares them.
    pub values: Vec<Value>,
}

impl Sample {
    /// The value of the field with that name, when the sample has one.
    ///
    /// The fields of a sample are not named ? the names are in the declaration of the
    /// message ? so this is only useful for the samples that carry a name of their own.
    #[must_use]
    pub fn value(&self, name: &str) -> Option<&Value> {
        self.values.iter().find(|value| match value {
            Value::Text(text) => text == name,
            Value::Number(_) => false,
        })
    }
}

/// A reader of the text logs of DataFlash.
///
/// The reader streams: a log of a long flight does not fit in memory.
pub struct DataFlashReader<R: BufRead> {
    /// The lines of the log.
    reader: R,
    /// What the log says about its messages.
    formats: HashMap<String, MessageFormat>,
    /// How many lines have been read.
    lines: u64,
}

impl<R: BufRead> DataFlashReader<R> {
    /// A reader of the log in the given source.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            formats: HashMap::new(),
            lines: 0,
        }
    }

    /// What the log says about its messages.
    #[must_use]
    pub fn formats(&self) -> &HashMap<String, MessageFormat> {
        &self.formats
    }

    /// The next sample of the log, or nothing when the log has ended.
    ///
    /// The declarations of the log are read as they come, and a sample of a message that
    /// has not been declared is skipped: a value that is not read with the type it was
    /// declared with is a value the operator would read wrong.
    ///
    /// # Errors
    ///
    /// Returns an error when the line cannot be read.
    pub fn next_sample(&mut self) -> Result<Option<Sample>, LogError> {
        let mut line = String::new();

        loop {
            line.clear();

            let read = self.reader.read_line(&mut line)?;

            if read == 0 {
                return Ok(None);
            }

            self.lines += 1;

            let trimmed = line.trim_end_matches(['\r', '\n']);

            if trimmed.is_empty() {
                continue;
            }

            let items: Vec<&str> = trimmed.split([',', ':']).map(str::trim).collect();

            let Some(name) = items.first().copied() else {
                continue;
            };

            if name.eq_ignore_ascii_case("FMT") {
                self.read_format(&items);
                continue;
            }

            let Some(format) = self.formats.get(name) else {
                continue;
            };

            return Ok(Some(Sample {
                name: name.to_owned(),
                values: read_values(format, &items[1..]),
            }));
        }
    }

    /// Reads the declaration of a message.
    fn read_format(&mut self, items: &[&str]) {
        // FMT, <id>, <length>, <name>, <types>, <field names...>
        if items.len() < 5 {
            return;
        }

        let Some(id) = items[1].parse::<u8>().ok() else {
            return;
        };

        let name = items[3].to_owned();
        let kinds: Vec<FieldKind> = items[4].chars().filter_map(FieldKind::of).collect();

        let fields: Vec<String> = items[5..].iter().map(|field| (*field).to_owned()).collect();

        self.formats.insert(
            name.clone(),
            MessageFormat {
                id,
                length: 0,
                name,
                fields,
                kinds,
            },
        );
    }
}

/// Reads the values of a sample with the types the message declares.
fn read_values(format: &MessageFormat, items: &[&str]) -> Vec<Value> {
    let mut values = Vec::with_capacity(items.len());

    for (index, item) in items.iter().enumerate() {
        let kind = format.kinds.get(index).copied();

        values.push(match kind {
            Some(FieldKind::Text(_)) => Value::Text((*item).to_owned()),
            Some(kind) => Value::Number(read_number(item, kind)),
            None => Value::Text((*item).to_owned()),
        });
    }

    values
}

/// Reads one number with the type it was declared with.
fn read_number(text: &str, kind: FieldKind) -> f64 {
    let trimmed = text.trim();

    match kind {
        FieldKind::U8 => trimmed.parse::<u8>().map(f64::from).unwrap_or(0.0),
        FieldKind::I8 => trimmed.parse::<i8>().map(f64::from).unwrap_or(0.0),
        FieldKind::U16 => trimmed.parse::<u16>().map(f64::from).unwrap_or(0.0),
        FieldKind::I16 => trimmed.parse::<i16>().map(f64::from).unwrap_or(0.0),
        FieldKind::U32 => trimmed.parse::<u32>().map(f64::from).unwrap_or(0.0),
        FieldKind::I32 => trimmed.parse::<i32>().map(f64::from).unwrap_or(0.0),
        FieldKind::U64 => trimmed
            .parse::<u64>()
            .map(|value| value as f64)
            .unwrap_or(0.0),
        FieldKind::I64 => trimmed
            .parse::<i64>()
            .map(|value| value as f64)
            .unwrap_or(0.0),
        FieldKind::F32 => trimmed.parse::<f32>().map(f64::from).unwrap_or(0.0),
        FieldKind::F64 => trimmed.parse::<f64>().unwrap_or(0.0),
        FieldKind::Cents => trimmed
            .parse::<f64>()
            .map(|value| value / 100.0)
            .unwrap_or(0.0),
        FieldKind::Location => trimmed
            .parse::<f64>()
            .map(|value| value / 1e7)
            .unwrap_or(0.0),
        FieldKind::Half => trimmed.parse::<f64>().unwrap_or(0.0),
        FieldKind::Text(_) => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A log of a short flight, in the format DataFlash writes.
    const LOG: &str = "FMT, 128, 89, FMT, BBnNZ, Type,Length,Name,Format\n\
FMT, 130, 45, GPS, QhLLee, TimeUS,Week,Lat,Lng,Alt,Spd\n\
GPS, 1000, 2100, -353547178, 1491696673, 88552, 245\n\
GPS, 2000, 2101, -353547180, 1491696680, 88652, 250\n\
FMT, 131, 20, MODE, QI, TimeUS,Mode\n\
MODE, 3000, 5\n";

    #[test]
    fn the_declarations_of_the_log_are_read() {
        let reader = DataFlashReader::new(LOG.as_bytes());

        let formats = reader.formats();

        assert!(formats.is_empty());
    }

    #[test]
    fn the_samples_of_the_log_are_read() {
        let mut reader = DataFlashReader::new(LOG.as_bytes());

        let mut samples = Vec::new();

        while let Some(sample) = reader.next_sample().expect("the log is read") {
            samples.push(sample);
        }

        assert_eq!(samples.len(), 3);

        let gps = &samples[0];
        assert_eq!(gps.name, "GPS");
        assert_eq!(gps.values.len(), 6);
        assert_eq!(gps.values[0].number(), Some(1000.0));
        assert!((gps.values[2].number().unwrap() - -35.3547178).abs() < 1e-9);
        assert!((gps.values[4].number().unwrap() - 885.52).abs() < 1e-6);

        let mode = &samples[2];
        assert_eq!(mode.name, "MODE");
        assert_eq!(mode.values[1].number(), Some(5.0));
    }

    #[test]
    fn the_declarations_are_kept() {
        let mut reader = DataFlashReader::new(LOG.as_bytes());

        while reader.next_sample().expect("the log is read").is_some() {}

        let formats = reader.formats();

        assert_eq!(formats.len(), 3);

        let gps = formats.get("GPS").expect("the log declares the GPS");
        assert_eq!(gps.id, 130);
        assert_eq!(
            gps.fields,
            vec!["TimeUS", "Week", "Lat", "Lng", "Alt", "Spd"]
        );
        assert_eq!(gps.field("lat"), Some(2));
    }

    #[test]
    fn a_sample_of_a_message_that_is_not_declared_is_skipped() {
        let log = "FMT, 130, 45, GPS, QhLLee, TimeUS,Week,Lat,Lng,Alt,Spd\n\
ATT, 1, 2, 3\n\
GPS, 1000, 2100, -35.3547178, 149.1696673, 885.52, 2.45\n";

        let mut reader = DataFlashReader::new(log.as_bytes());

        let sample = reader
            .next_sample()
            .expect("the log is read")
            .expect("a sample");

        assert_eq!(sample.name, "GPS");
    }

    #[test]
    fn an_empty_log_has_no_samples() {
        let mut reader = DataFlashReader::new("".as_bytes());

        assert!(reader.next_sample().expect("the log is read").is_none());
    }
}
