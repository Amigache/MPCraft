//! A board on the port, read as the bootloader reads it — and nothing erased.
//!
//! The half that erases cannot be read from this machine, and what is asserted here is only the half that
//! answers questions: the number the board gives for itself when it is asked, and the number it gives for
//! what its flash holds — which is the number this project's reader works out with the rule, and the two are
//! not the same implementation. Against the board on the bench the two agree: revision 5, board 9, flash
//! 2 080 768, crc `b690e491`, read from the board rather than from the file.
//!
//! A board is on one side of the protocol at a time, and the command that decides which side is the reboot
//! command with the action that holds the board in its bootloader — the only way from this machine, because
//! this board has no button and taking the power away does not carry the flag the loader reads. What the gate
//! asserts about that command is the bytes the helper puts on the wire, not the answer; what it records is
//! the answer kept apart from silence. On the bench, with the number the published dictionary gives the
//! command (246), the board answered `MAV_RESULT_ACCEPTED` and then answered as its own bootloader, which is
//! what the published handler does with that action. An earlier probe read a refusal for the same action, and
//! that refusal was an answer about a number the dictionary does not have (42760) rather than about the
//! action — which is why the number is checked against the dictionary in the wire test.
//!
//! ```text
//! MPCRAFT_HARDWARE_TARGET="serial:COM5:115200" cargo test -p mpcraft-app --features hardware-tests
//! ```
//!
//! The golden numbers come from an implementation that was not built from this one: python's own base64
//! and zlib for the document, and the CRC rule worked out from the published polynomial, whose table was
//! then compared entry by entry with the table the reference clone carries. They are written down before
//! anything is called, so a green means the two agree rather than that one agreed with itself.
//!
//! One run holds one connection, the same rule the other live areas follow: a board serves one station,
//! and a second binary would find nobody answering — which is why the test that records the answer to the
//! reboot command is run alone (`--test-threads=1`), and why it sends only when someone says so: a board that
//! answers it stays on the bootloader side until somebody tells it to start again or takes the power away,
//! and the other live areas read this same board as a vehicle. And nothing that erases is ever sent here,
//! because an erase is the half that cannot be taken back.
//!
//! ```text
//! MPCRAFT_HARDWARE_TARGET="serial:COM5:115200" cargo test -p mpcraft-app --features hardware-tests
//! ```
//!
//! The golden numbers come from an implementation that was not built from this one: python's own base64
//! and zlib for the document, and the CRC rule worked out from the published polynomial, whose table was
//! then compared entry by entry with the table the reference clone carries. They are written down before
//! anything is called, so a green means the two agree rather than that one agreed with itself.
//!
//! One run holds one connection, the same rule the other live areas follow: a board serves one station,
//! and a second binary would find nobody answering. And nothing that erases is ever sent here, because
//! an erase is the half that cannot be taken back.

#![cfg(feature = "hardware-tests")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::time::timeout;

use mpcraft_app::apj::FirmwareImage;
use mpcraft_app::bootloader::{Bootloader, Info, Port};
use mpcraft_link::{CommandError, LinkError, Session, SessionOptions};
use mpcraft_transport::{ConnectionSpec, SerialTransport};

/// The file the published manifest points at for the board on the bench (ArduPlane 4.7.1, fmuv3).
const PUBLISHED_FILE: &str =
    "https://firmware.ardupilot.org/Plane/stable-4.7.1/fmuv3/arduplane.apj";

/// What the independent implementation says about that document, written down before anything is called.
const GOLDEN_BOARD_ID: u32 = 9;
const GOLDEN_IMAGE_SIZE: usize = 1_535_384;
const GOLDEN_FLASH: u32 = 2_080_768;
const GOLDEN_CRC: u32 = 0xb690_e491;

/// How long the port is watched before anything is asked of it. A board that is running as a vehicle is
/// talking all the time; a board in its bootloader says nothing until it is asked.
const LOOK_WINDOW: Duration = Duration::from_secs(3);

/// How long one read of the port is given before it is taken as "the port had nothing to say". A real port
/// waits for bytes rather than answering with nothing, so a quiet board has to be watched against a wall
/// clock: without this the run waits on a board that will not speak until it is asked, which is exactly the
/// state the gate is here to tell apart.
const READ_SLICE: Duration = Duration::from_secs(1);

/// How long the board is given to answer the question that asks whether it is there. Booting takes a
/// while, so this is several chances rather than a wait on a wall clock.
const WAKE_WINDOW: Duration = Duration::from_secs(20);

/// The byte a MAVLink v2 frame starts with — what a board speaks when it is a vehicle.
const FRAME_START: u8 = 0xFD;

/// How long a board is allowed to take to introduce itself, so that the answer to a command can be read as
/// the board's own rather than as an answer addressed to an address somebody assumes.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// How long one transaction is allowed to take.
const TRANSACTION_TIMEOUT: Duration = Duration::from_secs(10);

/// The address the board is on.
fn target() -> String {
    std::env::var("MPCRAFT_HARDWARE_TARGET").expect("the gate names the port the board is on")
}

#[tokio::test]
async fn a_board_on_the_port_answers_as_it_is_running() {
    let spec = ConnectionSpec::parse(&target()).expect("the target is a connection specification");
    let mut port = SerialTransport::open(&spec.target, spec.options.baud_rate, false)
        .expect("the port can be opened");

    // What the port carries before anything is asked says which side of the protocol the board is on. A
    // board in its bootloader is quiet until it is asked; a board that is running speaks.
    let mut seen = Vec::new();
    let mut buffer = [0_u8; 256];
    let start = Instant::now();

    while start.elapsed() < LOOK_WINDOW {
        match timeout(READ_SLICE, port.read_some(&mut buffer)).await {
            Ok(Ok(count)) => seen.extend_from_slice(&buffer[..count]),
            Ok(Err(error)) => panic!("the port can be read: {error}"),
            Err(_) => {}
        }
    }

    println!(
        "the port carried {} bytes before anything was asked: {}",
        seen.len(),
        hex(&seen[..seen.len().min(40)])
    );

    // A board on the bootloader side says nothing until it is asked, so a port that carried nothing is one
    // of the two states this area is here to tell apart rather than a failure. What is asserted is what the
    // board says when it is asked.
    let speaks_as_a_vehicle = seen.contains(&FRAME_START);

    let mut board = Bootloader::new(port);

    match board.wake_up(WAKE_WINDOW).await {
        Ok(()) => {
            // The board is in its bootloader, so it can be asked what it is and what its flash holds.
            // These are the numbers the rule and the published document give, read from the board rather
            // than assumed from the file.
            let revision = board.device(Info::BlRev).await.expect("the board answers");
            let id = board
                .device(Info::BoardId)
                .await
                .expect("the board answers");
            let flash = board
                .device(Info::FlashSize)
                .await
                .expect("the board answers");
            let holds = board.crc().await.expect("the board answers");

            println!("the board answered as a bootloader: rev {revision} | board {id} | flash {flash} | crc {holds:08x}");

            assert_eq!(id, GOLDEN_BOARD_ID, "the board says what the document says");
            assert_eq!(
                flash, GOLDEN_FLASH,
                "the board says the flash the document gives"
            );
            assert_eq!(
                holds, GOLDEN_CRC,
                "the board's number is the number the rule gives"
            );
        }
        Err(error) => {
            println!("the board did not answer the question of the bootloader: {error}");

            assert!(
                speaks_as_a_vehicle,
                "a board that does not answer the bootloader is speaking as a vehicle",
            );
        }
    }
}

#[tokio::test]
async fn what_the_board_answers_to_the_reboot_command_is_kept_apart_from_silence() {
    // The bytes are asserted by the wire test, not here. What this run does is read what comes back and keep
    // an answer apart from nothing answering, which is what a station can tell apart: an answer that arrives
    // is the vehicle's, and the absence of one is not a refusal. On the bench the board answered
    // `MAV_RESULT_ACCEPTED` for the action that holds it in its bootloader, and then answered as its own
    // bootloader; a raw reader on the same port read nothing, because the device went away at the moment of
    // the restart, so the gate records rather than asserts which of the two a run will see.
    //
    // It sends only when someone says so, because a board that answers this command stays on the bootloader
    // side until somebody tells it to start again or takes the power away, and the other live areas read this
    // same board as a vehicle. Run it alone: a board serves one station, and a second binary on the port
    // finds nobody answering.
    if std::env::var("MPCRAFT_HARDWARE_REBOOT").is_err() {
        println!(
            "not sent here: MPCRAFT_HARDWARE_REBOOT is not set, and the command decides which side of the protocol the board speaks"
        );
        return;
    }

    let spec = ConnectionSpec::parse(&target()).expect("the target is a connection specification");
    let session = Arc::new(
        Session::connect(&spec, SessionOptions::default())
            .await
            .expect("the board can be reached"),
    );

    let vehicle = session
        .wait_for_any_vehicle(CONNECT_TIMEOUT)
        .await
        .expect("a board answers its heartbeat");

    println!(
        "sent to {}:{} — the number the published dictionary gives the command is 246, and the action that holds the board in its bootloader is 3",
        vehicle.system_id, vehicle.component_id
    );

    match session
        .reboot_into_bootloader(vehicle, TRANSACTION_TIMEOUT)
        .await
    {
        Ok(result) => println!("what came back was the board's own answer: {result:?}"),
        Err(LinkError::Command(CommandError::Refused { command, result })) => {
            println!("the board refused as its own: {command:?} -> {result:?}");
        }
        Err(LinkError::Timeout(window)) => {
            println!("nothing answered within {window:?}, which is not the same as a refusal");
        }
        Err(error) => println!("what came back was not an answer: {error}"),
    }

    session.stop().await;
}

#[tokio::test]
async fn the_reader_agrees_with_the_numbers_the_independent_implementation_gives() {
    // The same document, read by this reader and compared with the numbers written down above. This is the
    // half that can be checked without a board on the port, and it is the half the live run compares with
    // what the board says when it is in its bootloader.
    let response = reqwest::get(PUBLISHED_FILE)
        .await
        .expect("the published file can be fetched");

    assert!(response.status().is_success(), "the file answered");

    let bytes = response.bytes().await.expect("the file can be read");
    let file = FirmwareImage::read(&bytes).expect("the published file is the document");

    assert_eq!(file.board_id, GOLDEN_BOARD_ID);
    assert_eq!(file.image_size, GOLDEN_IMAGE_SIZE);

    // The image in the published document is already a multiple of four, so the padding this reader adds
    // is nothing — which is what the independent reader found, and it is why the number below depends on
    // the size the board gives rather than on the file alone.
    assert_eq!(file.image.len(), GOLDEN_IMAGE_SIZE);
    assert_eq!(file.expected_crc(GOLDEN_FLASH), GOLDEN_CRC);
}

/// Says what the bytes look like, without reading them for meaning.
fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}
