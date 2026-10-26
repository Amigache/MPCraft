//! The binary log of DataFlash: the `.bin`.
//!
//! # What this is
//!
//! The same log as the text one, in binary: a message is a marker of two bytes, the
//! number of the message and its values. The declarations of the messages are in the
//! log as well, in the message whose number is 128.
//!
//! The values are read with the types the declaration gives, and the sizes are the
//! sizes of the autopilot: a byte, a half word, a word, a double word — and the
//! scaled ones, which are read as they are and said in the unit they are in.
//!
//! # What it is for
//!
//! The binary log is what the autopilot writes when it is asked to, and it is smaller
//! and faster to write than the text one. A ground station that reads only the text
//! one cannot read the log of a flight that was recorded with the other.

use std::collections::HashMap;
use std::io::Read;

use crate::dataflash::{FieldKind, MessageFormat, Value};
use crate::tlog::LogError;

/// The first byte of the marker that comes before every message.
const HEAD_BYTE1: u8 = 0xA3;

/// The second byte of the marker that comes before every message.
const HEAD_BYTE2: u8 = 0x95;

/// The number of the message that declares the other messages.
const FORMAT_ID: u8 = 0x80;

/// The size of the declaration of a message, the marker not counted.
const FORMAT_LENGTH: usize = 86;

/// A reader of the binary logs of DataFlash.
///
/// The reader streams: a log of a long flight does not fit in memory.
pub struct BinaryReader<R: Read> {
    /// The bytes of the log.
    reader: R,
    /// What the log says about its messages.
    formats: HashMap<u8, MessageFormat>,
    /// How many bytes of the log have been read.
    position: usize,
    /// Where the log was broken.
    damage: Vec<BinaryDamage>,
}

/// Where the log was broken, and what the reader could not do there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryDamage {
    /// Where it happened, counted from the start of the log.
    pub offset: usize,
    /// Why the reader could not read it.
    pub reason: String,
}

impl<R: Read> BinaryReader<R> {
    /// A reader of the log in the given source.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            formats: HashMap::new(),
            position: 0,
            damage: Vec::new(),
        }
    }

    /// What the log says about its messages.
    #[must_use]
    pub fn formats(&self) -> &HashMap<u8, MessageFormat> {
        &self.formats
    }

    /// Where the log was broken.
    ///
    /// A log that ends in the middle of a message keeps what came before it, and the reader
    /// says where it stopped: a head fetched from a card is often cut in the middle of a
    /// sample, which is what the fixture of the golden test is.
    #[must_use]
    pub fn damage(&self) -> &[BinaryDamage] {
        &self.damage
    }

    /// Reads `size` bytes, or says where the log ends in the middle of them.
    fn fill(&mut self, size: usize) -> Result<Option<Vec<u8>>, LogError> {
        let start = self.position;
        let mut buffer = vec![0_u8; size];
        let mut read = 0_usize;

        while read < size {
            match self.reader.read(&mut buffer[read..]) {
                Ok(0) => {
                    self.damage.push(BinaryDamage {
                        offset: start,
                        reason: format!("the log ends in the middle of a record of {size} bytes"),
                    });

                    return Ok(None);
                }
                Ok(got) => read += got,
                Err(error) => return Err(LogError::Io(error)),
            }
        }

        self.position += read;

        Ok(Some(buffer))
    }

    /// The next sample of the log, or nothing when the log has ended.
    ///
    /// # Errors
    ///
    /// Returns an error when the log cannot be read, or when it is not a log.
    pub fn next_sample(&mut self) -> Result<Option<BinarySample>, LogError> {
        loop {
            let Some(marker) = self.read_marker()? else {
                return Ok(None);
            };

            if marker == FORMAT_ID {
                self.read_format()?;
                continue;
            }

            let Some(format) = self.formats.get(&marker).cloned() else {
                // A message whose declaration has not arrived: the log is not in the
                // order the reader expects, and the values cannot be read.
                continue;
            };

            // The length of the message counts the marker and the number of the
            // message, so the values are three bytes shorter.
            let size = format.length.saturating_sub(3) as usize;
            let start = self.position;

            let Some(payload) = self.fill(size)? else {
                return Ok(None);
            };

            // A declaration whose types do not fill its own length cannot be read: the bytes that
            // belong to the next message would be read as values of this one, so the message is
            // passed over and what the declaration says is written down.
            let needed = value_bytes(&format);

            if needed != size {
                self.damage.push(BinaryDamage {
                    offset: start,
                    reason: format!(
                        "the declaration says {size} bytes of values and its types need {needed}"
                    ),
                });

                continue;
            }

            // A field whose type the table does not give has no size, so none of the values of
            // this message can be read with the type it was declared with: the message is
            // passed over, and the names it could not read stay with its declaration.
            if !format.untyped().is_empty() {
                continue;
            }

            let values = read_values(&format, &payload);

            return Ok(Some(BinarySample {
                name: format.name.clone(),
                values,
            }));
        }
    }

    /// Reads the marker that comes before a message.
    fn read_marker(&mut self) -> Result<Option<u8>, LogError> {
        let mut byte = [0_u8; 1];

        loop {
            match self.reader.read(&mut byte) {
                Ok(0) => return Ok(None),
                Ok(_) => self.position += 1,
                Err(error) => return Err(LogError::Io(error)),
            }

            if byte[0] != HEAD_BYTE1 {
                continue;
            }

            // A log that ends after the first byte of a marker keeps what came before it and says
            // where it stopped.
            let Some(second) = self.fill(1)? else {
                return Ok(None);
            };

            if second[0] != HEAD_BYTE2 {
                continue;
            }

            let Some(kind) = self.fill(1)? else {
                return Ok(None);
            };

            return Ok(Some(kind[0]));
        }
    }

    /// Reads the declaration of a message.
    fn read_format(&mut self) -> Result<(), LogError> {
        let Some(payload) = self.fill(FORMAT_LENGTH)? else {
            return Ok(());
        };

        let kind = payload[0];
        let length = payload[1];
        let name = text_of(&payload[2..6]);
        let format = text_of(&payload[6..22]);
        let labels = text_of(&payload[22..86]);

        let fields: Vec<String> = labels
            .split(',')
            .map(str::trim)
            .filter(|field| !field.is_empty())
            .map(str::to_owned)
            .collect();

        // One kind per field, so a character the table does not describe keeps its place and
        // is named by `untyped` instead of shifting the values that come after it.
        let kinds: Vec<FieldKind> = format
            .chars()
            .take(fields.len())
            .map(|character| FieldKind::of(character).unwrap_or(FieldKind::Unknown))
            .collect();

        self.formats.insert(
            kind,
            MessageFormat {
                id: kind,
                length,
                name,
                fields,
                kinds,
            },
        );

        Ok(())
    }
}

/// One sample of a message of a binary log.
#[derive(Debug, Clone, PartialEq)]
pub struct BinarySample {
    /// The name of the message.
    pub name: String,
    /// The values, in the order the message declares them.
    pub values: Vec<Value>,
}

/// The text of a field of the declaration, without the zeroes.
fn text_of(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|byte| *byte as char)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Reads the values of a sample with the types the message declares.
fn read_values(format: &MessageFormat, payload: &[u8]) -> Vec<Value> {
    let mut values = Vec::with_capacity(format.kinds.len());
    let mut offset = 0_usize;

    for kind in &format.kinds {
        let (value, size) = read_value(*kind, payload, offset);
        values.push(value);
        offset += size;
    }

    values
}

/// How many bytes the values of a message take when read with the types its declaration gives.
///
/// When this does not equal what the declaration says the message is, the declaration and the log
/// disagree, and the reader cannot know which of the two it is reading.
fn value_bytes(format: &MessageFormat) -> usize {
    format.kinds.iter().copied().map(size_of).sum()
}

/// Reads one value with the type it was declared with.
fn read_value(kind: FieldKind, payload: &[u8], offset: usize) -> (Value, usize) {
    let size = size_of(kind);
    let bytes = payload.get(offset..offset + size).unwrap_or(&[]);

    let number = match kind {
        FieldKind::U8 => bytes.first().map(|byte| f64::from(*byte)).unwrap_or(0.0),
        FieldKind::I8 => bytes
            .first()
            .map(|byte| f64::from(*byte as i8))
            .unwrap_or(0.0),
        FieldKind::U16 => {
            let value = u16::from_le_bytes([
                bytes.first().copied().unwrap_or(0),
                *bytes.get(1).unwrap_or(&0),
            ]);
            f64::from(value)
        }
        FieldKind::I16 => {
            let value = i16::from_le_bytes([
                bytes.first().copied().unwrap_or(0),
                *bytes.get(1).unwrap_or(&0),
            ]);
            f64::from(value)
        }
        FieldKind::U32 => {
            let value = u32::from_le_bytes([
                *bytes.first().unwrap_or(&0),
                *bytes.get(1).unwrap_or(&0),
                *bytes.get(2).unwrap_or(&0),
                *bytes.get(3).unwrap_or(&0),
            ]);
            f64::from(value)
        }
        FieldKind::I32 => {
            let value = i32::from_le_bytes([
                *bytes.first().unwrap_or(&0),
                *bytes.get(1).unwrap_or(&0),
                *bytes.get(2).unwrap_or(&0),
                *bytes.get(3).unwrap_or(&0),
            ]);
            f64::from(value)
        }
        FieldKind::U64 => {
            let mut array = [0_u8; 8];
            let length = bytes.len().min(8);
            array[..length].copy_from_slice(&bytes[..length]);
            u64::from_le_bytes(array) as f64
        }
        FieldKind::I64 => {
            let mut array = [0_u8; 8];
            let length = bytes.len().min(8);
            array[..length].copy_from_slice(&bytes[..length]);
            i64::from_le_bytes(array) as f64
        }
        FieldKind::F32 => {
            let mut array = [0_u8; 4];
            let length = bytes.len().min(4);
            array[..length].copy_from_slice(&bytes[..length]);
            f64::from(f32::from_le_bytes(array))
        }
        FieldKind::F64 => {
            let mut array = [0_u8; 8];
            let length = bytes.len().min(8);
            array[..length].copy_from_slice(&bytes[..length]);
            f64::from_le_bytes(array)
        }
        FieldKind::CentsI16 => {
            let value = i16::from_le_bytes([
                bytes.first().copied().unwrap_or(0),
                *bytes.get(1).unwrap_or(&0),
            ]);
            f64::from(value) / 100.0
        }
        FieldKind::CentsU16 => {
            let value = u16::from_le_bytes([
                bytes.first().copied().unwrap_or(0),
                *bytes.get(1).unwrap_or(&0),
            ]);
            f64::from(value) / 100.0
        }
        FieldKind::CentsI32 => {
            let mut array = [0_u8; 4];
            let length = bytes.len().min(4);
            array[..length].copy_from_slice(&bytes[..length]);
            f64::from(i32::from_le_bytes(array)) / 100.0
        }
        FieldKind::CentsU32 => {
            let mut array = [0_u8; 4];
            let length = bytes.len().min(4);
            array[..length].copy_from_slice(&bytes[..length]);
            f64::from(u32::from_le_bytes(array)) / 100.0
        }
        FieldKind::Location => {
            let mut array = [0_u8; 4];
            let length = bytes.len().min(4);
            array[..length].copy_from_slice(&bytes[..length]);
            f64::from(i32::from_le_bytes(array)) / 1e7
        }
        FieldKind::Half => {
            let value = u16::from_le_bytes([
                bytes.first().copied().unwrap_or(0),
                *bytes.get(1).unwrap_or(&0),
            ]);
            f64::from(half_to_single(value))
        }
        FieldKind::Text(_) => 0.0,
        FieldKind::Unknown => 0.0,
    };

    let value = match kind {
        FieldKind::Text(_) => Value::Text(text_of(bytes)),
        // Reached only when a caller asks for the value of a field the table does not
        // describe, which is what the reader of a binary log does not do: it passes the whole
        // message over when it has one.
        FieldKind::Unknown => Value::Text(String::new()),
        _ => Value::Number(number),
    };

    (value, size)
}

/// The size in bytes of a value of each type.
fn size_of(kind: FieldKind) -> usize {
    match kind {
        FieldKind::U8 | FieldKind::I8 => 1,
        FieldKind::U16 | FieldKind::I16 | FieldKind::CentsI16 | FieldKind::CentsU16 => 2,
        FieldKind::U32
        | FieldKind::I32
        | FieldKind::CentsI32
        | FieldKind::CentsU32
        | FieldKind::Location
        | FieldKind::F32 => 4,
        FieldKind::U64 | FieldKind::I64 | FieldKind::F64 => 8,
        FieldKind::Half => 2,
        FieldKind::Text(length) => length,
        // A field the table does not describe has no size, which is why a message that carries
        // one is passed over rather than read.
        FieldKind::Unknown => 0,
    }
}

/// Turns a half-precision number into a single-precision one.
fn half_to_single(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;
    let exponent = (bits >> 10) & 0x1F;
    let mantissa = u32::from(bits & 0x03FF);

    let value = if exponent == 0 {
        if mantissa == 0 {
            0.0
        } else {
            mantissa as f32 * 2f32.powi(-24)
        }
    } else if exponent == 0x1F {
        if mantissa == 0 {
            f32::INFINITY
        } else {
            f32::NAN
        }
    } else {
        mantissa as f32 * 2f32.powi(i32::from(exponent) - 15) + 1.0
    };

    f32::from_bits(sign) * if sign == 0 { value } else { -value }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration of a message, as the binary log writes it.
    fn format_declaration(kind: u8, length: u8, name: &str, types: &str, labels: &str) -> Vec<u8> {
        let mut bytes = vec![0xA3, 0x95, FORMAT_ID, kind, length];

        let mut name_bytes = [0_u8; 4];
        name_bytes[..name.len().min(4)].copy_from_slice(&name.as_bytes()[..name.len().min(4)]);
        bytes.extend_from_slice(&name_bytes);

        let mut type_bytes = [0_u8; 16];
        type_bytes[..types.len().min(16)].copy_from_slice(&types.as_bytes()[..types.len().min(16)]);
        bytes.extend_from_slice(&type_bytes);

        let mut label_bytes = [0_u8; 64];
        let labels = labels.as_bytes();
        label_bytes[..labels.len().min(64)].copy_from_slice(&labels[..labels.len().min(64)]);
        bytes.extend_from_slice(&label_bytes);

        bytes
    }

    /// A message of the log, with its values.
    fn message(kind: u8, values: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0xA3, 0x95, kind];
        bytes.extend_from_slice(values);
        bytes
    }

    #[test]
    fn the_samples_of_the_log_are_read() {
        let mut log = format_declaration(0x81, 8, "MODE", "IB", "Mode,Reason");
        log.extend_from_slice(&message(0x81, &[5, 0, 0, 0, 2]));

        let mut reader = BinaryReader::new(&log[..]);

        let sample = reader
            .next_sample()
            .expect("the log is read")
            .expect("there is a sample");

        assert_eq!(sample.name, "MODE");
        assert_eq!(sample.values[0].number(), Some(5.0));
        assert_eq!(sample.values[1].number(), Some(2.0));
    }

    #[test]
    fn the_declarations_are_kept() {
        let log = format_declaration(0x81, 11, "MODE", "IB", "Mode,Reason");

        let mut reader = BinaryReader::new(&log[..]);

        while reader.next_sample().expect("the log is read").is_some() {}

        let formats = reader.formats();

        assert_eq!(formats.len(), 1);

        let mode = formats.get(&0x81).expect("the log declares the MODE");
        assert_eq!(mode.name, "MODE");
        assert_eq!(mode.fields, vec!["Mode", "Reason"]);
    }

    #[test]
    fn a_scaled_value_is_said_in_its_unit() {
        // A latitude in millionths of a degree.
        let mut log = format_declaration(0x82, 7, "GPS", "L", "Lat");
        log.extend_from_slice(&message(0x82, &(-35_354_718_i32).to_le_bytes()));

        let mut reader = BinaryReader::new(&log[..]);

        let sample = reader
            .next_sample()
            .expect("the log is read")
            .expect("there is a sample");

        let latitude = sample.values[0].number().unwrap();

        assert!((latitude - -3.5354718).abs() < 1e-9);
    }

    #[test]
    fn an_empty_log_has_no_samples() {
        let mut reader = BinaryReader::new("".as_bytes());

        assert!(reader.next_sample().expect("the log is read").is_none());
    }

    #[test]
    fn a_declaration_that_does_not_fill_its_own_length_is_said_and_the_message_is_passed_over() {
        // The declaration says MODE holds four bytes of values while its types need five (`I` is a
        // word and `B` a byte), so the reader cannot know which of the two it is reading: it reads
        // nothing and says what disagrees, then finds the next record by its marker.
        let mut log = format_declaration(0x81, 7, "MODE", "IB", "Mode,Reason");
        log.extend_from_slice(&message(0x81, &[5, 0, 0, 0, 2]));

        // A message that does fill its own length, after the one that does not.
        log.extend_from_slice(&format_declaration(0x82, 11, "RSSI", "If", "TimeUS,Rssi"));
        log.extend_from_slice(&message(0x82, &1_000_u32.to_le_bytes()));
        log.extend_from_slice(&1.0_f32.to_le_bytes());

        let mut reader = BinaryReader::new(&log[..]);

        let sample = reader
            .next_sample()
            .expect("the log is read")
            .expect("the message that can be read is read");

        assert_eq!(sample.name, "RSSI");

        let damage = reader.damage();

        assert_eq!(damage.len(), 1);
        // The record begins at 89 and its values begin at 92, which is what the reader names.
        assert_eq!(damage[0].offset, 92);
        assert_eq!(
            damage[0].reason,
            "the declaration says 4 bytes of values and its types need 5"
        );
    }

    #[test]
    fn a_half_word_in_hundredths_is_said_in_its_unit() {
        // The types `c` and `C`, which the map did not have: a half word and an unsigned half
        // word, both said in hundredths. The declaration's length counts the three-byte marker, so
        // it says seven for a message whose values are four.
        let mut log = format_declaration(0x81, 7, "XKF", "cC", "Roll,Yaw");

        let mut values = (-39_i16).to_le_bytes().to_vec();
        values.extend_from_slice(&13_165_u16.to_le_bytes());
        log.extend_from_slice(&message(0x81, &values));

        let mut reader = BinaryReader::new(&log[..]);

        let sample = reader
            .next_sample()
            .expect("the log is read")
            .expect("there is a sample");

        assert!((sample.values[0].number().unwrap() - -0.39).abs() < 1e-9);
        assert!((sample.values[1].number().unwrap() - 131.65).abs() < 1e-9);
    }

    #[test]
    fn a_field_the_table_does_not_describe_keeps_its_place() {
        // A type the published table does not give has no size, so the message that carries it
        // cannot be read at all — but the field still keeps its place, and the name is said.
        let mut log = format_declaration(0x81, 11, "MYS", "QY", "TimeUS,Mystery");
        log.extend_from_slice(&message(0x81, &1_000_u64.to_le_bytes()));

        let mut reader = BinaryReader::new(&log[..]);

        assert!(reader.next_sample().expect("the log is read").is_none());

        let format = reader.formats().get(&0x81).expect("the log declares MYS");

        assert_eq!(format.kinds.len(), format.fields.len());
        assert_eq!(format.untyped(), vec!["Mystery".to_owned()]);
    }

    #[test]
    fn a_log_that_ends_in_the_middle_of_a_record_says_where_it_broke() {
        let mut log = format_declaration(0x81, 8, "MODE", "IB", "Mode,Reason");
        log.extend_from_slice(&message(0x81, &[5, 0, 0, 0, 2]));

        // The next record begins and then the log stops: two of its five bytes are there.
        log.extend_from_slice(&[0xA3, 0x95, 0x81, 5]);

        let mut reader = BinaryReader::new(&log[..]);

        let sample = reader
            .next_sample()
            .expect("the log is read")
            .expect("the sample that came before is kept");

        assert_eq!(sample.values[0].number(), Some(5.0));

        assert!(reader.next_sample().expect("the log is read").is_none());

        let damage = reader.damage();

        assert_eq!(damage.len(), 1);
        // The declaration takes eighty-nine bytes, the first record eight, the second record's marker three: the
        // place said is where the bytes that could not be read begin.
        assert_eq!(damage[0].offset, 100);
        assert_eq!(
            damage[0].reason,
            "the log ends in the middle of a record of 5 bytes"
        );
    }
}
