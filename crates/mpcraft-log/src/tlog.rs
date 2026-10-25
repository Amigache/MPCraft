//! The telemetry log of MAVLink: the `.tlog`.
//!
//! # What this is
//!
//! A telemetry log is a stream of the frames that crossed the link, each of them with the
//! instant it was seen. Upstream writes it in `MAVLinkInterface.SaveToTlog`: the
//! timestamp comes first — eight bytes of microseconds since the epoch, written the
//! other way round — and then the frame exactly as it went over the wire.
//!
//! The log is what a flight is remembered by. A ground station that cannot read it
//! cannot say anything about a flight that has already happened.
//!
//! # What it is for
//!
//! The log is read twice: once to know what it holds, and once for what the operator
//! asks about it. Both passes stream: a log of a long flight does not fit in memory,
//! and the ground station has to be able to open a log of any size.

use std::io::Read;

use mpcraft_mavlink::dialect::Dialect;
use mpcraft_mavlink::frame::{FrameParser, IFLAG_SIGNED, IFLAG_TARGET_32};
use mpcraft_mavlink::{MavMessage, Message};

/// The marker of a MAVLink 1 frame.
const STX_V1: u8 = 0xFE;

/// The marker of a MAVLink 2 frame.
const STX_V2: u8 = 0xFD;

/// The size of the timestamp that comes before every record.
const TIMESTAMP_LEN: usize = 8;

/// The header of a MAVLink 1 frame.
const HEADER_V1: usize = 6;

/// The header of a MAVLink 2 frame.
const HEADER_V2: usize = 10;

/// The number of bytes of the checksum.
const CRC_LEN: usize = 2;

/// The size of the signature block of a signed frame.
const SIGNATURE_LEN: usize = 13;

/// The size of the target system and component, when the frame carries them.
const TARGET_LEN: usize = 4;

/// What can go wrong while a log is read.
#[derive(Debug, thiserror::Error)]
pub enum LogError {
    /// The file could not be read.
    #[error("the log could not be read: {0}")]
    Io(#[from] std::io::Error),
    /// The log does not hold what a log holds.
    #[error("the file is not a telemetry log: {0}")]
    NotALog(String),
    /// A frame of the log is not a frame.
    #[error("a frame of the log is not a frame: {0}")]
    Frame(#[from] mpcraft_mavlink::ProtocolError),
    /// A message of the log is not a message.
    ///
    /// The log is not broken: the message is one this reader does not understand,
    /// and the rest of the log is still good.
    #[error("a message of the log is malformed: {0}")]
    Malformed(String),
    /// The log holds something this reader cannot make sense of.
    ///
    /// A log of PX4 says when it holds a feature that breaks the format, and such a
    /// log is not to be read as if it were an ordinary one.
    #[error("the log cannot be read: {0}")]
    Unsupported(String),
}

/// A record of a log: a frame, the instant it was seen, and the vehicle that sent it.
#[derive(Debug, Clone)]
pub struct Record {
    /// When the frame was seen, in microseconds since the epoch.
    pub timestamp: u64,
    /// The message the frame carries.
    pub message: MavMessage,
    /// The pair that identifies the sender, read from the header of the frame.
    ///
    /// A log of a link that carried several vehicles keeps the frames of all of them, and
    /// the header is the only place that says which board a frame came from. Without it the
    /// reader would mix two flights into one state.
    pub sender: mpcraft_vehicle::VehicleId,
}

/// A reader of telemetry logs.
///
/// The reader is a stream: it hands out one record at a time and never holds more than
/// the record it is on.
pub struct TlogReader<R: Read> {
    /// The bytes that have not been read yet.
    reader: R,
    /// The parser of the frames.
    parser: FrameParser,
    /// The frames the parser has produced and that have not been read yet.
    frames: Vec<mpcraft_mavlink::Frame>,
    /// Whether the end of the file has been reached.
    finished: bool,
    /// The instant of the record that is being read.
    timestamp: u64,
}

impl<R: Read> TlogReader<R> {
    /// A reader of the log in the given source.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            parser: FrameParser::new(),
            frames: Vec::new(),
            finished: false,
            timestamp: 0,
        }
    }

    /// The next record of the log, or nothing when the log has ended.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read, or when it does not hold a log.
    pub fn next_record(&mut self) -> Result<Option<Record>, LogError> {
        loop {
            if let Some(frame) = self.frames.first() {
                let frame = frame.clone();
                self.frames.remove(0);

                let message = Dialect::parse(frame.version, frame.message_id, &frame.payload)
                    .map_err(|error| LogError::NotALog(error.to_string()))?;

                return Ok(Some(Record {
                    timestamp: self.timestamp,
                    message,
                    sender: mpcraft_vehicle::VehicleId::new(frame.system_id, frame.component_id),
                }));
            }

            if self.finished {
                return Ok(None);
            }

            self.read_next()?;
        }
    }

    /// Reads the next record of the log into the parser.
    fn read_next(&mut self) -> Result<(), LogError> {
        let mut timestamp = [0_u8; TIMESTAMP_LEN];

        if !read_exact_or_eof(&mut self.reader, &mut timestamp)? {
            self.finished = true;
            return Ok(());
        }

        // The timestamp is written the other way round: the bytes of a number of the
        // machine are turned round before they are written.
        self.timestamp = u64::from_be_bytes(timestamp);

        // The frame is read whole: its length is in its own header.
        let mut head = [0_u8; HEADER_V2];
        let read = read_exact_or_eof(&mut self.reader, &mut head[..1])?;

        if !read {
            self.finished = true;
            return Ok(());
        }

        let stx = head[0];

        if stx != STX_V1 && stx != STX_V2 {
            return Err(LogError::NotALog(format!(
                "the frame does not start with a marker (0x{stx:02X})"
            )));
        }

        let length = if stx == STX_V1 {
            read_exact_or_eof(&mut self.reader, &mut head[1..HEADER_V1])?;
            HEADER_V1
        } else {
            read_exact_or_eof(&mut self.reader, &mut head[1..HEADER_V2])?;
            HEADER_V2
        };

        let payload_len = usize::from(head[1]);
        let mut frame = head[..length].to_vec();

        let mut payload = vec![0_u8; payload_len];
        self.reader.read_exact(&mut payload)?;
        frame.extend_from_slice(&payload);

        let mut crc = [0_u8; CRC_LEN];
        self.reader.read_exact(&mut crc)?;
        frame.extend_from_slice(&crc);

        // A MAVLink 2 frame may carry the system and the component it is aimed at, and
        // it may be signed. Both are told by the flags of its header.
        if stx == STX_V2 {
            let flags = head[2];

            if flags & IFLAG_TARGET_32 != 0 {
                let mut target = [0_u8; TARGET_LEN];
                self.reader.read_exact(&mut target)?;
                frame.extend_from_slice(&target);
            }

            if flags & IFLAG_SIGNED != 0 {
                let mut signature = [0_u8; SIGNATURE_LEN];
                self.reader.read_exact(&mut signature)?;
                frame.extend_from_slice(&signature);
            }
        }

        self.parser.feed(&frame, &mut self.frames)?;

        Ok(())
    }
}

/// Reads as many bytes as the buffer holds, and says whether there were any.
///
/// The end of the file is not an error: a log that ends in the middle of nothing is a
/// log that has ended.
fn read_exact_or_eof<R: Read>(reader: &mut R, buffer: &mut [u8]) -> Result<bool, LogError> {
    let mut filled = 0;

    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(LogError::Io(error)),
        }
    }

    if filled == 0 {
        return Ok(false);
    }

    if filled < buffer.len() {
        return Err(LogError::NotALog(
            "the log ends in the middle of a record".to_owned(),
        ));
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpcraft_mavlink::dialect::messages::HEARTBEAT_DATA;
    use mpcraft_mavlink::frame::MavHeader;
    use mpcraft_mavlink::{MavAutopilot, MavModeFlag, MavState, MavType};

    /// A frame of a heartbeat, as it goes over the wire.
    fn heartbeat_frame() -> Vec<u8> {
        let message = MavMessage::HEARTBEAT(HEARTBEAT_DATA {
            custom_mode: 0,
            mavtype: MavType::MAV_TYPE_QUADROTOR,
            autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
            base_mode: MavModeFlag::empty(),
            system_status: MavState::MAV_STATE_ACTIVE,
            mavlink_version: 3,
        });

        mpcraft_mavlink::frame::encode_v2(
            &message,
            MavHeader {
                system_id: 1,
                component_id: 1,
                sequence: 0,
            },
        )
    }

    /// A log with the given frames, each of them with its timestamp.
    fn tlog(records: &[(u64, Vec<u8>)]) -> Vec<u8> {
        let mut bytes = Vec::new();

        for (timestamp, frame) in records {
            bytes.extend_from_slice(&timestamp.to_be_bytes());
            bytes.extend_from_slice(frame);
        }

        bytes
    }

    #[test]
    fn a_record_carries_the_instant_and_the_message() {
        let frame = heartbeat_frame();
        let bytes = tlog(&[(1_700_000_000_000_000, frame)]);

        let mut reader = TlogReader::new(&bytes[..]);
        let record = reader
            .next_record()
            .expect("the log is read")
            .expect("there is a record");

        assert_eq!(record.timestamp, 1_700_000_000_000_000);
        assert!(matches!(record.message, MavMessage::HEARTBEAT(_)));
    }

    #[test]
    fn the_records_come_in_the_order_they_were_written() {
        let frame = heartbeat_frame();
        let bytes = tlog(&[(10, frame.clone()), (20, frame.clone()), (30, frame)]);

        let mut reader = TlogReader::new(&bytes[..]);

        let mut timestamps = Vec::new();

        while let Some(record) = reader.next_record().expect("the log is read") {
            timestamps.push(record.timestamp);
        }

        assert_eq!(timestamps, vec![10, 20, 30]);
    }

    #[test]
    fn an_empty_log_has_no_records() {
        let mut reader = TlogReader::new(&[][..]);

        assert!(reader.next_record().expect("the log is read").is_none());
    }

    #[test]
    fn a_file_that_is_not_a_log_is_said_so() {
        let bytes = tlog(&[(1, vec![0x00, 0x01, 0x02])]);

        let mut reader = TlogReader::new(&bytes[..]);

        assert!(matches!(reader.next_record(), Err(LogError::NotALog(_))));
    }
}
