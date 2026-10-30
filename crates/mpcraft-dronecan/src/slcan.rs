//! SLCAN: the bus written as lines over a serial port.
//!
//! A CAN adapter of the "Lawicel" family — the SLCAN protocol — is reached over a serial port, and it
//! takes frames as text: one line per frame, a letter for the kind, the identifier in hexadecimal, the
//! number of bytes and then the bytes, ended by a carriage return. A DroneCAN frame is always an
//! extended frame, so the letter is `T` and the identifier is eight hexadecimal digits:
//!
//! ```text
//! T12345678 8 01 02 03 04 05 06 07 08\r
//! ```
//!
//! (the spaces are written here for the eye; the wire carries none). The adapter is opened with a
//! speed and then told to open the bus:
//!
//! ```text
//! S6\r      the speed, by the code the protocol numbers (500 000 bits per second)
//! O\r       open the bus
//! ```
//!
//! What comes back is not only frames: the adapter answers with a version, a serial number, status
//! flags, and a bell for a frame that was not accepted. A reader that took every line for a frame
//! would read its own configuration as traffic, so the decoder keeps what it can read and lets
//! everything else pass — and never fails a bus because a line was not for it.

use crate::can::Frame;

/// The number of bits per second each code of the protocol means.
///
/// The table is the one the protocol publishes; a rate that is not in it cannot be asked for, and a
/// station that guessed a code would tune the adapter to a bus nobody is on.
pub const SPEEDS: [(u32, char); 9] = [
    (10_000, '0'),
    (20_000, '1'),
    (50_000, '2'),
    (100_000, '3'),
    (125_000, '4'),
    (250_000, '5'),
    (500_000, '6'),
    (800_000, '7'),
    (1_000_000, '8'),
];

/// The code that asks for a rate, when the protocol has one.
#[must_use]
pub fn speed_code(bits_per_second: u32) -> Option<char> {
    SPEEDS
        .iter()
        .find(|(rate, _)| *rate == bits_per_second)
        .map(|(_, code)| *code)
}

/// The lines that open the bus at a rate.
///
/// The speed goes first and the bus is opened after it, which is the order the adapter expects: a bus
/// opened before the rate is a bus at whatever rate it was left in.
#[must_use]
pub fn open_lines(bits_per_second: u32) -> Option<Vec<u8>> {
    let code = speed_code(bits_per_second)?;

    Some(format!("S{code}\rO\r").into_bytes())
}

/// The line that closes the bus.
#[must_use]
pub fn close_line() -> Vec<u8> {
    b"C\r".to_vec()
}

/// One frame as the line the adapter takes.
///
/// Only extended frames are written, because a DroneCAN identifier has 29 bits; an identifier that
/// fits in eleven would be a different bus, and is written as extended anyway so that what the
/// adapter reads is one shape.
#[must_use]
pub fn encode(frame: &Frame) -> Vec<u8> {
    let length = frame.data.len().min(8);
    let mut line = format!("T{:08X}{length:X}", frame.id & 0x1FFF_FFFF);

    for byte in &frame.data[..length] {
        line.push_str(&format!("{byte:02X}"));
    }

    line.push('\r');

    line.into_bytes()
}

/// What the decoder is doing with the bytes it has been given.
#[derive(Debug, Default)]
pub struct Decoder {
    line: Vec<u8>,
    /// Set when a line ran past the buffer: everything up to its end belongs to that line, and the
    /// next frame begins only after it.
    discarding: bool,
    /// How many lines were thrown away because they were not a frame this reader knows.
    ignored: usize,
}

impl Decoder {
    /// An empty decoder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many lines arrived that were not a frame.
    ///
    /// The adapter's own answers — its version, its serial number, its status — are counted here
    /// rather than being an error: a bus that also says what it is has not failed.
    #[must_use]
    pub const fn ignored(&self) -> usize {
        self.ignored
    }

    /// Feeds bytes in, and hands back every frame they completed.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Frame> {
        let mut frames = Vec::new();

        for byte in bytes {
            match byte {
                b'\r' | b'\n' => {
                    if self.discarding {
                        self.discarding = false;
                        self.line.clear();
                        continue;
                    }

                    if !self.line.is_empty() {
                        if let Some(frame) = decode_line(&self.line) {
                            frames.push(frame);
                        } else {
                            self.ignored += 1;
                        }

                        self.line.clear();
                    }
                }
                // The bell is what the adapter writes when it could not take a frame; it is not part
                // of any line, and the next line begins after it.
                0x07 => {
                    self.line.clear();
                    self.ignored += 1;
                }
                byte => {
                    if self.discarding {
                        continue;
                    }

                    // A line that never ends is not a line: the buffer is bounded so a port that
                    // speaks nonsense cannot grow the state of the station. What was collected is
                    // thrown away, and everything up to the end of that line with it.
                    if self.line.len() < 64 {
                        self.line.push(*byte);
                    } else {
                        self.line.clear();
                        self.discarding = true;
                        self.ignored += 1;
                    }
                }
            }
        }

        frames
    }
}

/// Reads one line, when it is an extended frame of the protocol.
fn decode_line(line: &[u8]) -> Option<Frame> {
    let text = std::str::from_utf8(line).ok()?;

    // `T` is a frame with a 29-bit identifier and no timestamp. The variant with a timestamp is not
    // one a station asks for, so it is not read here either.
    let rest = text.strip_prefix('T')?;

    if rest.len() < 9 {
        return None;
    }

    let id = u32::from_str_radix(&rest[..8], 16).ok()?;
    let length = usize::from_str_radix(&rest[8..9], 16).ok()?;

    if length > 8 || rest.len() != 9 + length * 2 {
        return None;
    }

    let mut data = Vec::with_capacity(length);

    for index in 0..length {
        let at = 9 + index * 2;
        data.push(u8::from_str_radix(&rest[at..at + 2], 16).ok()?);
    }

    Some(Frame { id, data })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_the_line_the_adapter_takes() {
        let frame = Frame {
            id: 0x1234_5678,
            data: vec![0x01, 0x02, 0x03, 0xAA, 0xBB],
        };

        assert_eq!(encode(&frame), b"T123456785010203AABB\r".to_vec());
    }

    #[test]
    fn a_frame_of_eight_bytes_is_written_with_its_length() {
        let frame = Frame {
            id: 0x0000_001F,
            data: vec![0xFF; 8],
        };

        let line = encode(&frame);

        assert_eq!(line, b"T0000001F8FFFFFFFFFFFFFFFF\r".to_vec());
    }

    #[test]
    fn a_line_the_adapter_writes_is_read_back_as_the_frame() {
        let mut decoder = Decoder::new();
        let frames = decoder.push(b"T123456785010203AABB\r");

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].id, 0x1234_5678);
        assert_eq!(frames[0].data, vec![0x01, 0x02, 0x03, 0xAA, 0xBB]);
    }

    #[test]
    fn frames_that_arrive_together_are_read_one_by_one() {
        let mut decoder = Decoder::new();

        // Two frames in one read, which is what a port hands over when they arrive close together.
        let frames = decoder.push(b"T0000001F1FF\rT000000201C0\r");

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].id, 0x1F);
        assert_eq!(frames[1].id, 0x20);
        assert_eq!(frames[1].data, vec![0xC0]);
    }

    #[test]
    fn a_frame_split_across_two_reads_is_read_when_it_ends() {
        let mut decoder = Decoder::new();

        assert!(decoder.push(b"T0000001F1").is_empty());

        let frames = decoder.push(b"FF\r");

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, vec![0xFF]);
    }

    #[test]
    fn what_the_adapter_says_about_itself_is_not_a_frame() {
        let mut decoder = Decoder::new();

        // A version, a serial number, status flags and an accepted-frame bell, as an adapter writes
        // them when it is opened.
        let frames = decoder.push(b"V1013\rN1234\rF00\r\x07T0000001F1FF\r");

        assert_eq!(frames.len(), 1, "one frame and no more");
        assert_eq!(frames[0].id, 0x1F);
        assert_eq!(decoder.ignored(), 4);
    }

    #[test]
    fn a_line_that_says_a_length_it_does_not_carry_is_not_read() {
        let mut decoder = Decoder::new();

        // The line says three bytes and carries two.
        assert!(decoder.push(b"T0000001F301\r").is_empty());
        assert_eq!(decoder.ignored(), 1);

        // A length the protocol does not have.
        assert!(decoder.push(b"T0000001F9FFFFFFFFFFFFFFFFF\r").is_empty());
    }

    #[test]
    fn a_standard_identifier_frame_is_not_read_as_an_extended_one() {
        // `t` is an eleven-bit identifier; a DroneCAN identifier does not fit in it, and reading one
        // as the other would put a frame on the wrong part of the bus.
        let mut decoder = Decoder::new();

        assert!(decoder.push(b"t1F81FF\r").is_empty());
        assert_eq!(decoder.ignored(), 1);
    }

    #[test]
    fn a_port_that_never_ends_a_line_cannot_grow_the_buffer() {
        let mut decoder = Decoder::new();

        let mut noise = vec![b'x'; 200];
        noise.push(b'\r');

        assert!(decoder.push(&noise).is_empty());

        // Whatever was said was thrown away with the line it was on, and the next frame still reads.
        let frames = decoder.push(b"T0000001F1FF\r");

        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn a_rate_is_asked_for_by_the_code_the_protocol_numbers_it_with() {
        assert_eq!(speed_code(1_000_000), Some('8'));
        assert_eq!(speed_code(500_000), Some('6'));
        assert_eq!(speed_code(125_000), Some('4'));
        assert_eq!(speed_code(123_456), None);

        assert_eq!(open_lines(500_000), Some(b"S6\rO\r".to_vec()));
        assert_eq!(open_lines(1), None);
        assert_eq!(close_line(), b"C\r".to_vec());
    }
}
