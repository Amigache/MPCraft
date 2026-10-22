//! Buffered MAVLink frame parsing.
//!
//! # Why this exists
//!
//! Mission Planner's parser reads **one byte at a time** from the socket and hunts for the STX
//! marker by testing each byte. That is one syscall per byte on a link that can carry 50 frames
//! per second at 400 bytes each.
//!
//! MPCraft uses a **buffered state machine**: bytes are appended to an internal buffer and a
//! cursor advances over them. A complete frame is emitted as soon as it is available; a
//! truncated frame stays buffered until the rest arrives.
//!
//! # Resynchronisation
//!
//! When a frame fails CRC validation, the parser does not give up and it does not discard the
//! whole buffer. It advances past the corrupt frame and **scans forward for the next plausible
//! STX marker**, trying each candidate. This is the same strategy upstream uses, but bounded and
//! observable: every resync increments [`FrameParser::resync_count`].
//!
//! # Safety
//!
//! The parser never panics on malformed input and never allocates based on an unvalidated
//! wire-supplied length. The payload length field is a single byte, so the maximum possible
//! frame is bounded by the protocol constants.

use crate::error::{ProtocolError, Result};

/// MAVLink 1 start-of-frame marker.
pub const STX_V1: u8 = 0xFE;
/// MAVLink 2 start-of-frame marker.
pub const STX_V2: u8 = 0xFD;

/// Size of the MAVLink 1 header, including the STX marker.
pub const V1_HEADER_LEN: usize = 6;
/// Size of the MAVLink 2 header, including the STX marker.
pub const V2_HEADER_LEN: usize = 10;
/// Maximum payload length permitted by the protocol.
pub const MAX_PAYLOAD_LEN: usize = 255;
/// Size of the CRC trailer.
pub const CRC_LEN: usize = 2;
/// Size of the MAVLink 2 signature trailer.
pub const SIGNATURE_LEN: usize = 13;

/// The largest possible MAVLink 1 frame: header, maximum payload and CRC.
pub const MAX_FRAME_SIZE_V1: usize = V1_HEADER_LEN + MAX_PAYLOAD_LEN + CRC_LEN;
/// The largest possible MAVLink 2 frame: header, maximum payload, CRC and signature.
pub const MAX_FRAME_SIZE_V2: usize = V2_HEADER_LEN + MAX_PAYLOAD_LEN + CRC_LEN + SIGNATURE_LEN;

/// Incompatibility flag: the frame carries a signature.
pub const IFLAG_SIGNED: u8 = 0x01;
/// Incompatibility flag: the system id is 32 bits wide.
pub const IFLAG_SYSID_32: u8 = 0x02;
/// Incompatibility flag: a 32-bit target system id is present.
pub const IFLAG_TARGET_32: u8 = 0x04;

/// The MAVLink protocol version of a frame.
///
/// This is the version enum of the upstream `mavlink` crate, re-exported so that the frame
/// layer and the dialect layer agree on one type.
pub use mavlink::MavlinkVersion;

/// The header of a MAVLink frame: who sent it, and in which sequence.
pub use mavlink::MavHeader;

/// Encodes a message into a MAVLink 2 frame.
///
/// The frame carries the STX marker, the length, the flags, the header, the payload, the
/// CRC and, when the message is signed, the signature block. The CRC is computed with the
/// message's own CRC_EXTRA seed, so the result is a frame any conformant parser accepts.
///
/// # Panics
///
/// Panics if the message cannot be serialised, which the generated bindings never do.
#[must_use]
pub fn encode_v2<M: mavlink::Message>(message: &M, header: MavHeader) -> Vec<u8> {
    let mut frame = mavlink::MAVLinkV2MessageRaw::new();
    frame.serialize_message(header, message);
    frame.raw_bytes().to_vec()
}

/// Encodes a message into a signed MAVLink 2 frame.
///
/// The frame carries the signature block: the id of the link, the timestamp of the
/// signature and the signature itself. The signature is computed over the frame without
/// the block, with the flag that says the frame is signed already set — which is what
/// the other end signs over as well.
///
/// # Panics
///
/// Panics if the message cannot be serialised, which the generated bindings never do.
#[must_use]
pub fn encode_v2_signed<M: mavlink::Message>(
    message: &M,
    header: MavHeader,
    keyring: &crate::signing::SigningKeyring,
    key: &crate::signing::SigningKey,
    link_id: u8,
    timestamp: u64,
) -> Vec<u8> {
    let mut frame = encode_v2(message, header);

    // The flag that says the frame is signed. It is set before the checksum is worked
    // out, because the checksum covers the header.
    frame[2] |= IFLAG_SIGNED;

    // The checksum is worked out again, because the flag is part of what it covers.
    if let Some(seed) = crate::dialect::extra_crc(message.message_id()) {
        let crc_end = frame.len() - CRC_LEN;
        let crc = calculate_crc(&frame[1..crc_end], seed);
        frame[crc_end..].copy_from_slice(&crc.to_le_bytes());
    }

    let signature = keyring.sign(key, &frame, link_id, timestamp);

    frame.push(link_id);
    frame.extend_from_slice(&timestamp.to_le_bytes()[..6]);
    frame.extend_from_slice(&signature);

    frame
}

/// Encodes a message into a MAVLink 1 frame.
///
/// MAVLink 1 message ids are limited to 255; a message with a larger id cannot be encoded
/// this way.
///
/// # Panics
///
/// Panics if the message id does not fit in a byte, or the message cannot be serialised.
#[must_use]
pub fn encode_v1<M: mavlink::Message>(message: &M, header: MavHeader) -> Vec<u8> {
    assert!(
        message.message_id() <= u32::from(u8::MAX),
        "message id {} does not fit in a MAVLink 1 frame",
        message.message_id()
    );

    let mut frame = mavlink::MAVLinkV1MessageRaw::new();
    frame.serialize_message(header, message);
    frame.raw_bytes().to_vec()
}

/// A parsed MAVLink frame.
///
/// The payload is owned, so a `Frame` outlives the parser buffer it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The protocol version of the frame.
    pub version: MavlinkVersion,
    /// Incompatibility flags (MAVLink 2 only; zero for v1).
    pub incompat_flags: u8,
    /// Compatibility flags (MAVLink 2 only; zero for v1).
    pub compat_flags: u8,
    /// Packet sequence number.
    pub sequence: u8,
    /// Sending system id.
    pub system_id: u8,
    /// Sending component id.
    pub component_id: u8,
    /// Message id.
    pub message_id: u32,
    /// Optional target system id (MAVLink 2 only).
    pub target_system_id: Option<u8>,
    /// The message payload.
    pub payload: Vec<u8>,
    /// The CRC carried in the frame.
    pub crc: u16,
    /// The signature block, when the frame is signed.
    pub signature: Option<Signature>,
    /// The raw frame bytes, including the header, payload, CRC and any signature.
    ///
    /// Kept so the CRC and the signature can be re-verified over exactly the bytes that
    /// arrived, and so a frame can be logged verbatim.
    pub raw: Vec<u8>,
}

/// The MAVLink 2 signature trailer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature {
    /// Link id of the signing station.
    pub link_id: u8,
    /// Timestamp of the signature, in 100 microsecond units.
    pub timestamp: u64,
    /// The 48-bit signature value.
    pub value: [u8; 6],
}

/// What the parser does with a frame whose CRC does not validate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrcPolicy {
    /// Drop the frame and count it as a corruption.
    ///
    /// This is the default: a frame that fails its checksum carries arbitrary payload
    /// bytes, and letting it reach a transaction matcher risks acting on garbage.
    #[default]
    Reject,
    /// Emit the frame regardless, leaving validation to the caller.
    ///
    /// Useful for the packet inspector, which wants to show *that* a message arrived even
    /// when it was corrupted.
    PassThrough,
}

/// Statistics describing the state of the frame parser.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParserStats {
    /// Number of frames successfully parsed and accepted.
    pub frames_parsed: u64,
    /// Number of frames rejected because of a CRC mismatch.
    pub crc_errors: u64,
    /// Number of frames rejected because their message id is not in the dialect.
    pub unknown_messages: u64,
    /// Number of times the parser had to resynchronise after corruption.
    pub resyncs: u64,
    /// Number of bytes discarded while hunting for a frame boundary.
    pub bytes_discarded: u64,
}

/// A buffered, incremental MAVLink frame parser.
///
/// Feed it bytes; it yields complete frames.
///
/// # Example
///
/// ```
/// use mpcraft_mavlink::frame::{FrameParser, STX_V2};
///
/// let mut parser = FrameParser::new();
///
/// // A lone STX is not a frame: nothing is emitted and nothing is lost.
/// let mut out = Vec::new();
/// parser.feed(&[STX_V2], &mut out).unwrap();
/// assert!(out.is_empty());
/// ```
#[derive(Debug)]
pub struct FrameParser {
    buffer: Vec<u8>,
    cursor: usize,
    stats: ParserStats,
    policy: CrcPolicy,
    /// The bytes that were discarded while hunting for a frame boundary.
    plaintext: Vec<u8>,
}

impl Default for FrameParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameParser {
    /// Creates a parser that drops frames whose CRC does not validate.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(1024),
            cursor: 0,
            stats: ParserStats::default(),
            policy: CrcPolicy::Reject,
            plaintext: Vec::new(),
        }
    }

    /// Creates a parser with an explicit CRC policy.
    #[must_use]
    pub fn with_policy(policy: CrcPolicy) -> Self {
        Self {
            policy,
            ..Self::new()
        }
    }

    /// Feeds bytes to the parser, appending every complete frame to `out`.
    ///
    /// Frames that fail CRC validation are discarded and the parser resynchronises; they are
    /// counted in [`ParserStats::crc_errors`] rather than returned as errors, because a corrupt
    /// link is a condition to observe, not to propagate.
    ///
    /// # Errors
    ///
    /// Returns an error only if the parser is in an unrecoverable state, which currently cannot
    /// happen — the signature exists to document that callers must handle failure.
    pub fn feed(&mut self, bytes: &[u8], out: &mut Vec<Frame>) -> Result<()> {
        self.buffer.extend_from_slice(bytes);
        self.parse_available(out)?;
        Ok(())
    }

    /// Returns the parser statistics.
    #[must_use]
    pub fn stats(&self) -> ParserStats {
        self.stats
    }

    /// Number of times the parser resynchronised after corruption.
    #[must_use]
    pub fn resync_count(&self) -> u64 {
        self.stats.resyncs
    }

    /// Number of bytes currently held in the internal buffer awaiting more data.
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.buffer.len() - self.cursor
    }

    /// Discards all buffered bytes and resets the statistics.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.plaintext.clear();
        self.stats = ParserStats::default();
    }

    /// Takes the bytes that were discarded while hunting for a frame boundary.
    ///
    /// These are the bytes that were *not* part of a MAVLink frame. A board that is printing
    /// its banner produces them, and they are the difference between "no heartbeat
    /// received" and "the board is printing at the wrong baud rate".
    pub fn take_plaintext(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.plaintext)
    }

    /// Verifies the CRC of a frame against the dialect's CRC_EXTRA seed.
    ///
    /// The CRC covers the frame bytes from the payload-length field through the end of the
    /// payload — that is, everything between the STX marker and the CRC trailer — and the
    /// CRC_EXTRA seed for the message id is accumulated afterwards.
    ///
    /// A message id that is not in the compiled dialect cannot be validated. Rather than
    /// assuming a seed of zero and reporting a false corruption, this returns
    /// [`ProtocolError::UnknownMessage`].
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::UnknownMessage`] when the message id is not in the dialect, and
    /// [`ProtocolError::CrcMismatch`] when the frame is corrupt.
    pub fn verify_crc(frame: &Frame) -> Result<()> {
        let Some(seed) = crate::dialect::extra_crc(frame.message_id) else {
            return Err(ProtocolError::UnknownMessage(frame.message_id));
        };

        // The CRC trailer sits immediately before the signature block.
        let crc_len = CRC_LEN;
        let sig_len = frame.signature.map_or(0, |_| SIGNATURE_LEN);
        let crc_end = frame.raw.len() - sig_len;
        let crc_start = crc_end - crc_len;

        // The CRC is computed over everything from the payload-length field to the payload end.
        let computed = calculate_crc(&frame.raw[1..crc_start], seed);

        if computed != frame.crc {
            return Err(ProtocolError::CrcMismatch {
                expected: frame.crc,
                actual: computed,
            });
        }

        Ok(())
    }

    /// Verifies the CRC of a frame, returning `false` instead of an error.
    ///
    /// Convenient in hot paths where a corrupt frame should simply be dropped and counted.
    #[must_use]
    pub fn is_crc_valid(frame: &Frame) -> bool {
        Self::verify_crc(frame).is_ok()
    }

    /// Parses as many complete frames as the buffer holds.
    fn parse_available(&mut self, out: &mut Vec<Frame>) -> Result<()> {
        loop {
            let (frame, consumed) = match self.try_parse_one() {
                ParseOutcome::Frame(frame, consumed) => (frame, consumed),
                ParseOutcome::Incomplete => break,
                ParseOutcome::Corrupt { skip } => {
                    self.stats.crc_errors += 1;
                    self.stats.resyncs += 1;
                    self.stats.bytes_discarded += skip as u64;
                    self.advance(skip);
                    continue;
                }
            };

            // The frame is structurally sound. Decide whether it is trustworthy.
            match self.classify(&frame) {
                FrameStatus::Accepted => {
                    self.stats.frames_parsed += 1;
                    out.push(frame);
                    self.advance(consumed);
                }
                FrameStatus::BadCrc => {
                    self.stats.crc_errors += 1;
                    self.stats.resyncs += 1;
                    self.stats.bytes_discarded += consumed as u64;
                    self.advance(consumed);
                }
                FrameStatus::UnknownMessage => {
                    self.stats.unknown_messages += 1;
                    self.stats.bytes_discarded += consumed as u64;
                    self.advance(consumed);
                }
            }
        }

        // Reclaim consumed bytes so the buffer does not grow without bound on a long-lived link.
        if self.cursor > 0 {
            self.buffer.drain(..self.cursor);
            self.cursor = 0;
        }

        Ok(())
    }

    /// Decides whether a structurally valid frame should be emitted.
    fn classify(&self, frame: &Frame) -> FrameStatus {
        if self.policy == CrcPolicy::PassThrough {
            return FrameStatus::Accepted;
        }

        match Self::verify_crc(frame) {
            Ok(()) => FrameStatus::Accepted,
            Err(ProtocolError::UnknownMessage(_)) => FrameStatus::UnknownMessage,
            Err(_) => FrameStatus::BadCrc,
        }
    }

    /// Attempts to parse a single frame at the cursor.
    fn try_parse_one(&mut self) -> ParseOutcome {
        // Locate the next STX without holding a borrow across the parse calls.
        let stx_pos = {
            let buf = &self.buffer[self.cursor..];
            buf.iter().position(|&b| b == STX_V1 || b == STX_V2)
        };

        let Some(stx_pos) = stx_pos else {
            // No frame boundary in the buffer at all: discard everything but the last byte,
            // which could be the start of an STX split across two feeds.
            let discard = (self.buffer.len() - self.cursor).saturating_sub(1);
            return if discard == 0 {
                ParseOutcome::Incomplete
            } else {
                self.stats.bytes_discarded += discard as u64;
                self.plaintext
                    .extend_from_slice(&self.buffer[self.cursor..self.cursor + discard]);
                ParseOutcome::Corrupt { skip: discard }
            };
        };

        if stx_pos > 0 {
            self.stats.bytes_discarded += stx_pos as u64;
            self.plaintext
                .extend_from_slice(&self.buffer[self.cursor..self.cursor + stx_pos]);
            self.advance(stx_pos);
        }

        let stx = self.buffer[self.cursor];
        let parsed = match stx {
            STX_V1 => Self::parse_v1(&self.buffer[self.cursor..]),
            STX_V2 => Self::parse_v2(&self.buffer[self.cursor..]),
            // Unreachable: `stx` was matched by the position search above.
            _ => return ParseOutcome::Corrupt { skip: 1 },
        };

        match parsed {
            // The cursor is advanced by the caller, which owns the decision of whether the
            // frame is accepted, rejected as corrupt, or rejected as unknown.
            Some((frame, len)) => ParseOutcome::Frame(frame, len),
            None => ParseOutcome::Incomplete,
        }
    }

    /// Parses a MAVLink 1 frame at the start of `buf`.
    ///
    /// Returns the frame and the number of bytes it occupies, or `None` when more bytes are
    /// needed.
    fn parse_v1(buf: &[u8]) -> Option<(Frame, usize)> {
        if buf.len() < V1_HEADER_LEN {
            return None;
        }

        let payload_len = buf[1] as usize;
        let total = V1_HEADER_LEN + payload_len + CRC_LEN;

        if buf.len() < total {
            return None;
        }

        let sequence = buf[2];
        let system_id = buf[3];
        let component_id = buf[4];
        let message_id = buf[5] as u32;
        let payload = buf[V1_HEADER_LEN..V1_HEADER_LEN + payload_len].to_vec();
        let crc = u16::from_le_bytes([buf[total - 2], buf[total - 1]]);

        let frame = Frame {
            version: MavlinkVersion::V1,
            incompat_flags: 0,
            compat_flags: 0,
            sequence,
            system_id,
            component_id,
            message_id,
            target_system_id: None,
            payload,
            crc,
            signature: None,
            raw: buf[..total].to_vec(),
        };

        Some((frame, total))
    }

    /// Parses a MAVLink 2 frame at the start of `buf`.
    ///
    /// Returns the frame and the number of bytes it occupies, or `None` when more bytes are
    /// needed.
    ///
    /// # The header layout
    ///
    /// ```text
    /// 0      1      2      3      4      5      6      7 8 9
    /// STX    len    incomp compat seq    sysid  compid msgid
    /// ```
    ///
    /// The message id occupies three bytes at offset seven, and the payload starts at
    /// offset ten. With the `SYSID32` flag the system id widens to four bytes and
    /// everything after it shifts by three.
    fn parse_v2(buf: &[u8]) -> Option<(Frame, usize)> {
        if buf.len() < V2_HEADER_LEN {
            return None;
        }

        let payload_len = buf[1] as usize;
        let incompat_flags = buf[2];
        let compat_flags = buf[3];
        let sequence = buf[4];

        // The system id widens to 32 bits when the flag is set, which shifts everything
        // that follows it.
        let (system_id, component_id, message_id, mut header_len) =
            if incompat_flags & IFLAG_SYSID_32 != 0 {
                if buf.len() < V2_HEADER_LEN + 3 {
                    return None;
                }
                let id = u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);
                let component = buf[9];
                let id3 = u32::from_le_bytes([buf[10], buf[11], buf[12], 0]);
                (id, component, id3, V2_HEADER_LEN + 3)
            } else {
                let id = u32::from(buf[5]);
                let component = buf[6];
                let id3 = u32::from_le_bytes([buf[7], buf[8], buf[9], 0]);
                (id, component, id3, V2_HEADER_LEN)
            };

        // An optional 32-bit target system id follows the message id.
        let mut target_system_id = None;
        if incompat_flags & IFLAG_TARGET_32 != 0 {
            if buf.len() < header_len + 4 {
                return None;
            }
            target_system_id = Some(buf[header_len]);
            header_len += 4;
        }

        let signature_len = if incompat_flags & IFLAG_SIGNED != 0 {
            SIGNATURE_LEN
        } else {
            0
        };

        let total = header_len + payload_len + CRC_LEN + signature_len;
        if buf.len() < total {
            return None;
        }

        let payload = buf[header_len..header_len + payload_len].to_vec();
        let crc_offset = header_len + payload_len;
        let crc = u16::from_le_bytes([buf[crc_offset], buf[crc_offset + 1]]);

        let signature = if signature_len > 0 {
            let s = &buf[crc_offset + CRC_LEN..total];
            let mut value = [0u8; 6];
            value.copy_from_slice(&s[7..13]);
            Some(Signature {
                link_id: s[0],
                timestamp: u64::from_le_bytes([s[1], s[2], s[3], s[4], s[5], s[6], 0, 0]),
                value,
            })
        } else {
            None
        };

        let frame = Frame {
            version: MavlinkVersion::V2,
            incompat_flags,
            compat_flags,
            sequence,
            system_id: system_id as u8,
            component_id,
            message_id,
            target_system_id,
            payload,
            crc,
            signature,
            raw: buf[..total].to_vec(),
        };

        Some((frame, total))
    }

    /// Advances the cursor by `n` bytes.
    fn advance(&mut self, n: usize) {
        self.cursor = (self.cursor + n).min(self.buffer.len());
    }
}

/// The result of attempting to parse one frame.
enum ParseOutcome {
    /// A complete frame was parsed, occupying the given number of bytes.
    Frame(Frame, usize),
    /// More bytes are needed.
    Incomplete,
    /// The bytes are corrupt; skip `skip` bytes and resynchronise.
    Corrupt { skip: usize },
}

/// The trustworthiness of a structurally valid frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameStatus {
    /// The frame is well formed and its CRC validates.
    Accepted,
    /// The frame is well formed but its CRC does not match.
    BadCrc,
    /// The frame's message id is not part of the compiled dialect.
    UnknownMessage,
}

/// Computes the MAVLink CRC-16/MCRF4XX over `data`, seeded with the message's CRC_EXTRA byte.
///
/// The CRC covers the frame bytes *excluding* the STX marker and the CRC trailer, and the
/// CRC_EXTRA seed is accumulated **after** the payload — exactly as the protocol specifies.
///
/// ```
/// use mpcraft_mavlink::frame::calculate_crc;
///
/// // Folding a zero CRC_EXTRA seed into the initial register value 0xFFFF yields 0x0F87.
/// assert_eq!(calculate_crc(&[], 0), 0x0F87);
/// ```
#[must_use]
pub fn calculate_crc(data: &[u8], extra_crc: u8) -> u16 {
    let mut crc: u16 = 0xFFFF;

    for &byte in data {
        let mut tmp = byte ^ (crc as u8);
        tmp ^= tmp << 4;
        let tmp = tmp as u16;
        crc = (crc >> 8) ^ (tmp << 8) ^ (tmp << 3) ^ (tmp >> 4);
    }

    // Accumulate the CRC_EXTRA seed.
    let mut tmp = extra_crc ^ (crc as u8);
    tmp ^= tmp << 4;
    let tmp = tmp as u16;
    crc = (crc >> 8) ^ (tmp << 8) ^ (tmp << 3) ^ (tmp >> 4);

    crc
}

/// Accumulates a single byte into a running CRC.
///
/// This is the primitive the protocol defines; [`calculate_crc`] uses it.
#[must_use]
pub const fn crc_accumulate(byte: u8, crc: u16) -> u16 {
    let mut tmp = byte ^ (crc as u8);
    tmp ^= tmp << 4;
    let tmp = tmp as u16;
    (crc >> 8) ^ (tmp << 8) ^ (tmp << 3) ^ (tmp >> 4)
}

/// Errors raised when a frame is structurally invalid.
impl From<ProtocolError> for String {
    fn from(e: ProtocolError) -> Self {
        e.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `MavMessage::parse` is a method of the upstream `Message` trait.
    use mavlink::Message as _;

    #[test]
    fn crc_matches_specification() {
        // Folding a zero CRC_EXTRA seed into the initial value 0xFFFF yields 0x0F87. The
        // value 0xFFFF is the *initial* CRC register, not the result.
        assert_eq!(calculate_crc(&[], 0), 0x0F87);
        // The seed is folded in after the payload, so a different seed gives a different
        // result over identical data.
        assert_ne!(calculate_crc(&[], 0), calculate_crc(&[], 1));
        assert_ne!(calculate_crc(&[], 0), calculate_crc(&[1], 0));
    }

    #[test]
    fn crc_matches_the_upstream_mavlink_implementation() {
        // Cross-check against an independent implementation of the same protocol primitive.
        // A round-trip test alone would not catch a symmetrically wrong CRC.
        let payloads: [&[u8]; 5] = [
            &[],
            &[0x00],
            &[0xFF; 9],
            &[0x01, 0x02, 0x03, 0x04, 0x05],
            &[0xDE, 0xAD, 0xBE, 0xEF],
        ];

        for payload in payloads {
            for seed in [0u8, 1, 50, 148, 255] {
                assert_eq!(
                    calculate_crc(payload, seed),
                    mavlink::calculate_crc(payload, seed),
                    "CRC mismatch for payload {payload:?} with seed {seed}"
                );
            }
        }
    }

    #[test]
    fn crc_accumulate_matches_calculate() {
        let data = [0x01, 0x02, 0x03, 0x04];
        let mut crc = 0xFFFFu16;
        for &b in &data {
            crc = crc_accumulate(b, crc);
        }
        crc = crc_accumulate(0x2f, crc);
        assert_eq!(crc, calculate_crc(&data, 0x2f));
    }

    #[test]
    fn incomplete_frame_is_buffered_not_emitted() {
        let mut parser = FrameParser::new();
        let mut out = Vec::new();

        parser.feed(&[STX_V2, 0x00], &mut out).unwrap();
        assert!(out.is_empty());
        assert_eq!(parser.buffered(), 2);
    }

    #[test]
    fn garbage_before_frame_is_discarded() {
        let mut parser = FrameParser::new();
        let mut out = Vec::new();

        let frame = v1_frame(0, &[0u8; 9], None);

        parser.feed(b"hello world junk", &mut out).unwrap();
        assert!(out.is_empty());

        parser.feed(&frame, &mut out).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].system_id, 1);
        assert_eq!(out[0].message_id, 0);
    }

    #[test]
    fn frame_is_split_across_feeds() {
        let mut parser = FrameParser::new();
        let mut out = Vec::new();

        let frame = v1_frame(0, &[1, 2, 3, 4], None);

        for chunk in frame.chunks(3) {
            parser.feed(chunk, &mut out).unwrap();
        }

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].payload, vec![1, 2, 3, 4]);
        assert_eq!(out[0].sequence, 0);
    }

    #[test]
    fn resyncs_after_corrupt_frame() {
        let mut parser = FrameParser::new();
        let mut out = Vec::new();

        let frame = v1_frame(0, &[1, 2, 3, 4], Some(0xDEAD));

        parser.feed(&frame, &mut out).unwrap();

        // The corrupt frame is dropped, counted, and the parser resynchronises.
        assert!(out.is_empty());
        assert_eq!(parser.stats().crc_errors, 1);
        assert_eq!(parser.resync_count(), 1);
        assert_eq!(parser.stats().frames_parsed, 0);
    }

    #[test]
    fn a_good_frame_after_a_corrupt_one_is_still_recovered() {
        let mut parser = FrameParser::new();
        let mut out = Vec::new();

        let mut bytes = v1_frame(0, &[1, 2, 3, 4], Some(0xDEAD));
        bytes.extend_from_slice(&v1_frame(0, &[9, 9, 9, 9], None));

        parser.feed(&bytes, &mut out).unwrap();

        assert_eq!(
            out.len(),
            1,
            "the good frame should survive the corrupt one"
        );
        assert_eq!(out[0].payload, vec![9, 9, 9, 9]);
        assert_eq!(parser.stats().crc_errors, 1);
    }

    #[test]
    fn pass_through_policy_emits_unvalidated_frames() {
        let mut parser = FrameParser::with_policy(CrcPolicy::PassThrough);
        let mut out = Vec::new();

        parser
            .feed(&v1_frame(0, &[1, 2, 3, 4], Some(0xDEAD)), &mut out)
            .unwrap();

        assert_eq!(out.len(), 1);
        assert_eq!(parser.stats().crc_errors, 0);
    }

    #[test]
    fn frames_with_unknown_message_ids_are_dropped() {
        let mut parser = FrameParser::new();
        let mut out = Vec::new();

        // Find a message id below 256 that the ArduPilot Mega dialect does not define.
        let known: Vec<u32> = crate::dialect::Dialect::all_messages()
            .iter()
            .map(|(_, id)| *id)
            .collect();
        let unknown = (1u32..256)
            .find(|id| !known.contains(id))
            .expect("free message id");

        parser
            .feed(&v1_frame(unknown, &[1, 2, 3, 4], None), &mut out)
            .unwrap();

        assert!(out.is_empty());
        assert_eq!(parser.stats().unknown_messages, 1);
    }

    #[test]
    fn a_valid_heartbeat_frame_is_accepted() {
        let mut parser = FrameParser::new();
        let mut out = Vec::new();

        // HEARTBEAT is message id 0 with a CRC_EXTRA of 50.
        let payload = [0u8; 9];
        let frame = v1_frame(0, &payload, None);
        parser.feed(&frame, &mut out).unwrap();

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].message_id, 0);
        assert_eq!(parser.stats().frames_parsed, 1);
        assert_eq!(parser.stats().crc_errors, 0);
    }

    /// Builds a MAVLink 1 frame with the given message id and payload.
    ///
    /// `force_crc` overrides the computed checksum, which is how corrupt frames are built.
    fn v1_frame(message_id: u32, payload: &[u8], force_crc: Option<u16>) -> Vec<u8> {
        let mut frame = vec![STX_V1, payload.len() as u8, 0, 1, 1];
        frame.push(message_id as u8);
        frame.extend_from_slice(payload);

        let crc = force_crc.unwrap_or_else(|| {
            let seed = crate::dialect::extra_crc(message_id).unwrap_or(0);
            calculate_crc(&frame[1..], seed)
        });
        frame.extend_from_slice(&crc.to_le_bytes());
        frame
    }

    #[test]
    fn payload_length_cannot_exceed_protocol_maximum() {
        // The payload length is a single byte, so the frame size is bounded by construction.
        assert_eq!(MAX_PAYLOAD_LEN, u8::MAX as usize);
        assert_eq!(MAX_FRAME_SIZE_V1, 263);
        assert_eq!(MAX_FRAME_SIZE_V2, 280);
    }

    #[test]
    fn reset_clears_state() {
        let mut parser = FrameParser::new();
        let mut out = Vec::new();
        parser.feed(&[STX_V2, 0x10], &mut out).unwrap();
        assert_eq!(parser.buffered(), 2);
        parser.reset();
        assert_eq!(parser.buffered(), 0);
        assert_eq!(parser.stats().frames_parsed, 0);
    }

    #[test]
    fn an_encoded_frame_parses_back_to_the_same_message() {
        let message =
            crate::dialect::MavMessage::HEARTBEAT(crate::dialect::messages::HEARTBEAT_DATA {
                custom_mode: 5,
                mavtype: crate::dialect::MavType::MAV_TYPE_QUADROTOR,
                autopilot: crate::dialect::MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
                ..Default::default()
            });

        let header = MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: 7,
        };

        let bytes = encode_v2(&message, header);

        let mut parser = FrameParser::new();
        let mut frames = Vec::new();
        parser.feed(&bytes, &mut frames).unwrap();

        assert_eq!(frames.len(), 1, "the frame should have been parsed");
        let frame = &frames[0];
        assert_eq!(frame.message_id, 0);
        assert_eq!(frame.system_id, 1);
        assert_eq!(frame.component_id, 1);
        assert_eq!(frame.sequence, 7);
        assert_eq!(frame.version, MavlinkVersion::V2);

        // The CRC must validate against the message's own seed.
        assert!(FrameParser::verify_crc(frame).is_ok());
        assert_eq!(parser.stats().crc_errors, 0);
    }

    #[test]
    fn a_mavlink1_frame_round_trips_too() {
        let message =
            crate::dialect::MavMessage::HEARTBEAT(crate::dialect::messages::HEARTBEAT_DATA {
                custom_mode: 3,
                ..Default::default()
            });

        let header = MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: 2,
        };

        let bytes = encode_v1(&message, header);

        let mut parser = FrameParser::new();
        let mut frames = Vec::new();
        parser.feed(&bytes, &mut frames).unwrap();

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].version, MavlinkVersion::V1);
        assert_eq!(frames[0].sequence, 2);
        assert!(FrameParser::verify_crc(&frames[0]).is_ok());
    }

    /// A `MISSION_COUNT` whose extensions are all zero.
    ///
    /// A plain mission travels in the short form: the extension bytes are zero and are
    /// truncated away, so the payload is only the four bytes of the short form.
    fn plain_mission_count() -> crate::dialect::MavMessage {
        crate::dialect::MavMessage::MISSION_COUNT(crate::dialect::messages::MISSION_COUNT_DATA {
            count: 3,
            target_system: 1,
            target_component: 1,
            ..Default::default()
        })
    }

    #[test]
    fn an_extension_field_survives_the_frame() {
        // The fence and the rally plans travel with a non-zero `MAV_MISSION_TYPE`, and that
        // field is a MAVLink 2 extension. It has to arrive intact.
        let message = crate::dialect::MavMessage::MISSION_COUNT(
            crate::dialect::messages::MISSION_COUNT_DATA {
                count: 5,
                target_system: 1,
                target_component: 1,
                mission_type: crate::dialect::MavMissionType::MAV_MISSION_TYPE_FENCE,
                opaque_id: 0x1234,
            },
        );

        let header = MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: 4,
        };

        let bytes = encode_v2(&message, header);

        let mut parser = FrameParser::new();
        let mut frames = Vec::new();
        parser.feed(&bytes, &mut frames).unwrap();

        assert_eq!(frames.len(), 1);
        assert!(FrameParser::verify_crc(&frames[0]).is_ok());

        let decoded = crate::dialect::MavMessage::parse(
            MavlinkVersion::V2,
            frames[0].message_id,
            &frames[0].payload,
        )
        .expect("the frame should decode");

        match decoded {
            crate::dialect::MavMessage::MISSION_COUNT(count) => {
                assert_eq!(count.count, 5);
                assert_eq!(
                    count.mission_type,
                    crate::dialect::MavMissionType::MAV_MISSION_TYPE_FENCE
                );
                assert_eq!(count.opaque_id, 0x1234);
            }
            other => panic!("expected a mission count, got {other:?}"),
        }
    }

    #[test]
    fn the_extension_bytes_are_truncated_at_the_first_zero() {
        // A plan whose extensions are zero travels in the short form: the payload is the
        // four bytes of the short form and no more.
        let header = MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: 0,
        };

        let bytes = encode_v2(&plain_mission_count(), header);

        // The length is the second byte of the frame.
        assert_eq!(bytes[1], 4, "the short form is four bytes long");

        let mut parser = FrameParser::new();
        let mut frames = Vec::new();
        parser.feed(&bytes, &mut frames).unwrap();

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].payload.len(), 4);

        // The plan is still a plain mission once it arrives.
        let decoded = crate::dialect::MavMessage::parse(
            MavlinkVersion::V2,
            frames[0].message_id,
            &frames[0].payload,
        )
        .expect("the frame should decode");

        match decoded {
            crate::dialect::MavMessage::MISSION_COUNT(count) => {
                assert_eq!(count.count, 3);
                assert_eq!(
                    count.mission_type,
                    crate::dialect::MavMissionType::MAV_MISSION_TYPE_MISSION
                );
            }
            other => panic!("expected a mission count, got {other:?}"),
        }
    }

    #[test]
    fn a_mavlink1_frame_carries_no_extensions() {
        // MAVLink 1 has no extension fields, so a fence plan cannot travel in it. The
        // message still encodes, and the type is simply absent.
        let message = crate::dialect::MavMessage::MISSION_COUNT(
            crate::dialect::messages::MISSION_COUNT_DATA {
                count: 2,
                target_system: 1,
                target_component: 1,
                mission_type: crate::dialect::MavMissionType::MAV_MISSION_TYPE_RALLY,
                ..Default::default()
            },
        );

        let header = MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: 0,
        };

        let bytes = encode_v1(&message, header);

        assert_eq!(bytes[1], 4, "the short form is four bytes long");

        let mut parser = FrameParser::new();
        let mut frames = Vec::new();
        parser.feed(&bytes, &mut frames).unwrap();

        assert_eq!(frames.len(), 1);
        assert!(FrameParser::verify_crc(&frames[0]).is_ok());
    }
}
