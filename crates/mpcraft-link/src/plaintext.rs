//! Capturing the boot output of a board.
//!
//! When a board is misconfigured — the wrong baud rate, or the wrong port — it says so on
//! the wire in plain text, not in MAVLink. Mission Planner captures that text and shows it
//! to the user, which turns a mystifying "no heartbeat received" into "the board is printing
//! its banner at 115200, not 57600".
//!
//! That diagnostic is genuinely valuable, so MPCraft keeps it. The capture is bounded: a
//! board that prints continuously must not exhaust memory.

use std::collections::VecDeque;

/// The number of lines kept.
const MAX_LINES: usize = 52;

/// The minimum length of a line worth keeping.
const MIN_LINE_LENGTH: usize = 4;

/// Captures printable text arriving outside of MAVLink frames.
#[derive(Debug, Clone)]
pub struct PlaintextSniffer {
    /// The lines captured, newest first.
    lines: VecDeque<String>,
    /// The line currently being assembled.
    current: String,
    /// The number of lines dropped because the buffer was full.
    dropped: u64,
}

impl Default for PlaintextSniffer {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaintextSniffer {
    /// Creates an empty sniffer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            lines: VecDeque::with_capacity(MAX_LINES),
            current: String::new(),
            dropped: 0,
        }
    }

    /// Feeds the bytes that were not part of a MAVLink frame.
    ///
    /// Printable characters and line terminators are captured; everything else is ignored.
    pub fn observe(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            match byte {
                b'\r' | b'\n' => {
                    // A line terminator closes the current line.
                    if self.current.len() >= MIN_LINE_LENGTH {
                        self.lines.push_front(std::mem::take(&mut self.current));
                        while self.lines.len() > MAX_LINES {
                            self.lines.pop_back();
                            self.dropped += 1;
                        }
                    } else {
                        self.current.clear();
                    }
                }
                0x20..=0x7E => self.current.push(byte as char),
                _ => {
                    // A control character that is not a line terminator ends the line: it
                    // marks the boundary of the board's output.
                    self.current.clear();
                }
            }
        }
    }

    /// The captured lines, newest first.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.lines.iter().cloned().collect()
    }

    /// The captured text as a single block, newest first.
    #[must_use]
    pub fn text(&self) -> String {
        self.lines.iter().cloned().collect::<Vec<_>>().join("\n")
    }

    /// The number of lines dropped because the buffer was full.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Whether the captured text looks like a DroneCAN bus.
    ///
    /// A DroneCAN node prints a frame identifier of the form `T` followed by a hexadecimal
    /// payload. Mission Planner detects this and tells the user to use the DroneCAN screen
    /// instead, which is a much better answer than "no heartbeat received".
    #[must_use]
    pub fn looks_like_can_bus(&self) -> bool {
        self.lines.iter().any(|line| {
            let bytes = line.as_bytes();
            // The line must start with 'T' followed by nine to thirty-two hexadecimal
            // characters, and end there.
            bytes.len() >= 10
                && bytes.len() <= 33
                && bytes[0] == b'T'
                && bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
        })
    }

    /// Clears the capture.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.current.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_text_is_captured_line_by_line() {
        let mut sniffer = PlaintextSniffer::new();
        sniffer.observe(b"ArduCopter V4.3.7 (0123abcd)\r\n");

        let lines = sniffer.lines();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "ArduCopter V4.3.7 (0123abcd)");
    }

    #[test]
    fn short_lines_are_not_kept() {
        let mut sniffer = PlaintextSniffer::new();
        sniffer.observe(b"ok\r\n");

        assert!(sniffer.lines().is_empty());
    }

    #[test]
    fn the_capture_is_bounded() {
        let mut sniffer = PlaintextSniffer::new();

        for index in 0..200 {
            sniffer.observe(format!("line {index} of output\r\n").as_bytes());
        }

        assert_eq!(sniffer.lines().len(), MAX_LINES);
        assert!(sniffer.dropped() > 0);
        // The newest line is kept first.
        assert_eq!(sniffer.lines()[0], "line 199 of output");
    }

    #[test]
    fn a_dronecan_banner_is_recognised() {
        let mut sniffer = PlaintextSniffer::new();
        sniffer.observe(b"T0123456789ABCDEF\r\n");

        assert!(sniffer.looks_like_can_bus());
    }

    #[test]
    fn an_ordinary_banner_is_not_mistaken_for_a_can_bus() {
        let mut sniffer = PlaintextSniffer::new();
        sniffer.observe(b"ArduCopter V4.3.7 (0123abcd)\r\n");

        assert!(!sniffer.looks_like_can_bus());
    }

    #[test]
    fn the_captured_text_is_joined_for_display() {
        let mut sniffer = PlaintextSniffer::new();
        sniffer.observe(b"first line\r\n");
        sniffer.observe(b"second line\r\n");

        let text = sniffer.text();
        assert!(text.contains("first line"));
        assert!(text.contains("second line"));
    }
}
