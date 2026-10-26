//! The startup text of a real board, kept as it was captured.
//!
//! A board names its model when it starts, and that text is the only place the name appears. The numbers the
//! vehicle reports in `AUTOPILOT_VERSION` do not name it: this board answers with vendor `4617` and product
//! `22337`, which are `0x1209` and `0x5741` — the pair ArduPilot uses as the default for non-composite boards,
//! so they say the family and nothing about the model. That is why the detection reads the text rather than the
//! identifier, and why the expected name below is taken from the hardware list the firmware manifest follows
//! rather than from the table in the module being checked.
//!
//! The fixture is what the port carried while the board booted. The bytes between the lines were frames; the
//! rule the capture uses — printable runs of four characters or more — is what turned them into lines, so the
//! file is the shape the interface reads, not a tidy transcript.
//!
//! ```text
//! cargo test -p mpcraft-app --test board_boot_text
//! ```

use mpcraft_app::board::{detect, known_boards};

/// The startup text of the board on COM5, captured on 2026-10-26.
const STARTUP: &str = include_str!("fixtures/pixhawk_startup.txt");

#[test]
fn the_board_names_itself_in_its_startup_text() {
    let lines: Vec<String> = STARTUP.lines().map(str::to_owned).collect();

    // The name is the one the manifest of the firmware uses for this hardware, which is what makes the board
    // that is detected the board the firmware is chosen for.
    assert_eq!(detect(&lines).as_deref(), Some("Pixhawk"));

    assert!(
        known_boards().contains(&"Pixhawk".to_owned()),
        "the board found in the output is one the application knows by the name the manifest gives it",
    );
}

#[test]
fn the_numbers_a_vehicle_reports_cannot_name_its_model() {
    // The same run read vendor 4617 and product 22337 from the vehicle's report. Read as text they name the
    // default pair ArduPilot uses for non-composite boards — a family, not a board.
    let ids = vec!["vendor 4617 product 22337".to_owned()];

    assert_eq!(detect(&ids), None);
}

#[test]
fn the_capture_keeps_the_lines_that_explain_the_vehicle() {
    let lines: Vec<String> = STARTUP.lines().map(str::to_owned).collect();

    /*
     * The same board answered the list of its logs with an entry that says there are none. The reason is in the
     * startup text: the card did not start. A capture that kept only the line naming the board would lose the
     * half that makes the empty answer readable, so both lines have to be there.
     */
    assert!(
        lines
            .iter()
            .any(|line| line.contains("SDCard failed to start")),
        "the capture keeps what the board said about the card it could not start",
    );

    assert!(
        lines.iter().any(|line| line.contains("no sensor found")),
        "the capture keeps what the board said about the sensor it did not find",
    );
}
