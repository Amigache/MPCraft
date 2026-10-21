//! The board that is on the other end of the link.
//!
//! A board says what it is when it starts: the line of its own output carries the name
//! of the board. That output is captured by the link — it is the text that is not part
//! of a MAVLink frame — so the board can be told from what the vehicle said about
//! itself.
//!
//! The names are the ones the manifest of the firmware uses, so that the board that is
//! detected is the same one that the firmware is chosen for.

/// The boards that are known, with the names the manifest gives them.
const KNOWN_BOARDS: &[&str] = &[
    "CubeBlack",
    "CubeBlackCopter",
    "CubeOrange",
    "CubeOrange+",
    "CubeOrangeCopter",
    "CubePilot",
    "CubeOrangeCopter",
    "BeastH7",
    "BeastH7Copter",
    "KakuteF7",
    "KakuteF4",
    "KakuteH7",
    "KakuteF4Mini",
    "Pixhawk4",
    "Pixhawk4S",
    "PixhawkCube",
    "Pixhawk4Cube",
    "Pixhawk",
    "Pixracer",
    "Revo-mini",
    "OmniNiagara",
    "mRoX21",
    "MatekF405",
    "MatekF405-Wing",
    "HolybroKakuteF4",
    "HolybroKakuteF7",
    "HolybroKakuteH7",
    "3DRControlZeroG",
    "FurunoX10",
    "Pilot-RC-01",
    "Pilot-RC-02",
    "ARK_CANNODE",
];

/// The board that the output of the vehicle names, when it names one.
///
/// The lines of the output are looked at one by one, and the board whose name is in a
/// line is the board. The names are compared without the spaces and without the case:
/// a board says `Cube Black` as well as `CubeBlack`.
#[must_use]
pub fn detect(lines: &[String]) -> Option<String> {
    for line in lines {
        let haystack = plain(line);

        for board in KNOWN_BOARDS {
            if haystack.contains(&plain(board)) {
                return Some((*board).to_owned());
            }
        }
    }

    None
}

/// A piece of text, without the spaces and without the case.
fn plain(text: &str) -> String {
    text.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

/// The boards that are known, for the interface to show.
#[must_use]
pub fn known_boards() -> Vec<String> {
    KNOWN_BOARDS
        .iter()
        .map(|board| (*board).to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_board_is_found_in_the_output_of_the_vehicle() {
        let lines = vec![
            "ArduPilot V4.4.0 (abc1234) 64-bit Linux".to_owned(),
            "HW: CubeBlack".to_owned(),
            "NMEA: GPS 1 RMC=3".to_owned(),
        ];

        assert_eq!(detect(&lines).as_deref(), Some("CubeBlack"));
    }

    #[test]
    fn the_board_is_found_however_it_is_written() {
        let lines = vec!["ChibiOS: Starting on  cube black  board".to_owned()];
        assert_eq!(detect(&lines).as_deref(), Some("CubeBlack"));
    }

    #[test]
    fn a_vehicle_that_does_not_say_its_board_leaves_it_unknown() {
        let lines = vec![
            "ArduPilot V4.4.0 (abc1234) 64-bit Linux".to_owned(),
            "NMEA: GPS 1 RMC=3".to_owned(),
        ];

        assert_eq!(detect(&lines), None);
    }

    #[test]
    fn an_empty_output_leaves_the_board_unknown() {
        assert_eq!(detect(&[]), None);
    }
}
