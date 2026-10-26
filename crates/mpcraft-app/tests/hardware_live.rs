//! A real board on the other end of the port, read as a vehicle.
//!
//! A simulator cannot say what a board says about itself. It does not name its hardware, it does not fail to
//! find a sensor, it does not choose an address for itself, it does not have a card that failed to start, and it
//! has no filesystem to reach. What this area asserts is the half that only hardware can answer for: the pair the
//! board introduces itself with, the capabilities it declares, the family its heartbeat names, the words it says
//! about itself, the answer it gives when asked for the logs it holds, and the number its file system gives when
//! it cannot work.
//!
//! ```text
//! MPCRAFT_HARDWARE_TARGET="serial:COM5:115200" cargo test -p mpcraft-app --features hardware-tests
//! ```
//!
//! The target is a connection specification rather than a socket, because a board is reached through a port;
//! and one run holds one connection, the same rule the simulator areas follow — a board serves one station, so
//! a second binary would find nobody answering.
//!
//! The numbers are read from the published dictionary rather than copied into the test. That is what keeps this
//! a second implementation: the bits below are the values `MAV_PROTOCOL_CAPABILITY` gives each capability, and
//! the decisions asserted afterwards are the ones the code makes from them.

#![cfg(feature = "hardware-tests")]

use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

use mpcraft_app::board::detect;
use mpcraft_link::ftp::{falls_back_from, FtpClient, FtpError, FtpMessage, Opcode};
use mpcraft_link::vehicle::VehicleKind;
use mpcraft_link::{LinkError, Session, SessionOptions, VehicleId};
use mpcraft_params::ParamEncoding;
use mpcraft_transport::ConnectionSpec;

/// How long a board is allowed to take to introduce itself.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// How long one transaction is allowed to take.
const TRANSACTION_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the board is allowed to take for its whole parameter list. This board answers 1389 values in about
/// forty seconds at the rate its port uses, so the window is wider than that rather than the same.
const PARAMETER_WINDOW: Duration = Duration::from_secs(120);

/// How long the board is allowed to take to print something outside of the frames. It prints its pre-arm lines
/// once a second while it waits for the things it does not have, so this is a few chances rather than a wait on
/// a wall clock.
const TEXT_WINDOW: Duration = Duration::from_secs(25);

/// The capability bits, as the published dictionary numbers them.
mod dictionary {
    /// Autopilot supports the `MISSION_ITEM` float message type.
    pub const MISSION_ITEM_FLOAT: u64 = 1;
    /// Autopilot supports the new param float message type.
    pub const PARAM_FLOAT: u64 = 2;
    /// Autopilot supports `MISSION_ITEM_INT` scaled integer message type.
    pub const MISSION_INT: u64 = 4;
    /// Autopilot supports `COMMAND_INT` scaled integer message type.
    pub const COMMAND_INT: u64 = 8;
    /// Autopilot supports the File Transfer Protocol v1.
    pub const FILE_TRANSFER: u64 = 32;
    /// Autopilot supports commanding position and velocity targets in global scaled integers.
    pub const GLOBAL_POSITION_TARGETS: u64 = 256;
    /// Autopilot supports terrain protocol / data handling.
    pub const TERRAIN: u64 = 512;
    /// Autopilot supports onboard compass calibration.
    pub const COMPASS_CALIBRATION: u64 = 4096;
    /// Autopilot supports MAVLink version 2.
    pub const MAVLINK_V2: u64 = 8192;
    /// Autopilot supports mission fence protocol.
    pub const FENCE: u64 = 16384;
    /// Autopilot supports mission rally point protocol.
    pub const RALLY: u64 = 32768;
}

/// The number ArduPilot's file-system mapping gives for a drive that cannot work (`FR_NOT_READY` becomes
/// `EBUSY`), read from the published source rather than copied from what the board answered. This crate cannot
/// name such a number — it belongs to the vehicle's filesystem, not to the protocol dictionary — so the golden
/// value lives here, next to the capability bits that are read the same way.
const EBUSY: u8 = 16;

/// The address the board is on.
fn target() -> String {
    std::env::var("MPCRAFT_HARDWARE_TARGET").expect("the gate names the port the board is on")
}

#[tokio::test]
async fn a_board_says_what_a_simulator_cannot() {
    let spec = ConnectionSpec::parse(&target()).expect("the target is a connection specification");
    // The client for the transfer protocol is built over the session the link holds, so the session is shared
    // rather than borrowed; the areas that read it take it as they always did.
    let session = Arc::new(
        Session::connect(&spec, SessionOptions::default())
            .await
            .expect("the board can be reached"),
    );

    let vehicle = session
        .wait_for_any_vehicle(CONNECT_TIMEOUT)
        .await
        .expect("a board answers its heartbeat");

    reads_the_family_it_names(&session, vehicle);
    declares_what_it_can(&session, vehicle).await;
    keeps_the_number_it_holds_for_itself(&session, vehicle).await;
    its_words_reach_the_interface(&session).await;
    says_it_has_no_logs(&session, vehicle).await;
    reaches_its_own_files(&session, vehicle).await;

    session.stop().await;
}

/// The family the board's heartbeat names. A simulator is read as a copter because that is what it was started
/// as; this board says airplane, and the family has to come from what it said rather than from what the run
/// assumed.
fn reads_the_family_it_names(session: &Session, vehicle: VehicleId) {
    let state = session.state();
    let seen = state
        .vehicles
        .iter()
        .find(|entry| entry.id == vehicle)
        .expect("the board that answered is on the registry");

    println!(
        "seen: {} | kind {:?} | mav_type {:?} | autopilot {:?}",
        seen.display_name(),
        seen.kind,
        seen.mav_type,
        seen.autopilot
    );

    assert_eq!(
        seen.kind,
        Some(VehicleKind::ArduPlane),
        "the board is named by the family its heartbeat names",
    );
}

/// What the board declares, and the decisions those declarations drive.
///
/// The station cannot assume these: they decide which encoding the values travel in and which plan kinds the
/// vehicle will take. This board answers with acceptance and then with its own report, and the report says what
/// it is — not what a script was written to say.
async fn declares_what_it_can(session: &Session, vehicle: VehicleId) {
    session
        .request_capabilities(vehicle, TRANSACTION_TIMEOUT)
        .await
        .expect("the board answers the ask for its capabilities");

    let flags = session
        .state()
        .capabilities
        .expect("the board sent its report");

    println!("capabilities: {flags} (0x{flags:x})");

    for bit in [
        dictionary::MISSION_ITEM_FLOAT,
        dictionary::PARAM_FLOAT,
        dictionary::MISSION_INT,
        dictionary::COMMAND_INT,
        dictionary::FILE_TRANSFER,
        dictionary::GLOBAL_POSITION_TARGETS,
        dictionary::TERRAIN,
        dictionary::COMPASS_CALIBRATION,
        dictionary::MAVLINK_V2,
        dictionary::FENCE,
        dictionary::RALLY,
    ] {
        assert!(
            flags & bit != 0,
            "the board declares the capability the dictionary numbers {bit}",
        );
    }

    // The two decisions the declarations drive, read from the code that makes them.
    assert_eq!(
        session.param_encoding(),
        ParamEncoding::Float,
        "the board declares the float encoding for its values",
    );

    assert!(
        session.uses_mission_int(),
        "the board declares the scaled integer form for the items of a plan",
    );

    /*
     * What this board cannot say is written down rather than assumed: it has no camera and no gimbal, so it
     * never sends `CAMERA_CAP_FLAGS` or `GIMBAL_DEVICE_CAP_FLAGS`. The rule that refuses an angle a device
     * cannot reach can be shown refusing on a scripted report, but it cannot be shown accepting on this
     * hardware — there is nothing on the board to be capable of.
     */
}

/// The number the board holds for its own identity, compared with the key the station builds for it.
///
/// This is the only way to know whether the key came from the wire: the parameter is the board saying what
/// address it uses, and the registry is the station building one. When the two disagree the station invented an
/// address, which is what an assumption of the reserved one would have done.
async fn keeps_the_number_it_holds_for_itself(session: &Session, vehicle: VehicleId) {
    let store = session
        .download_parameters(vehicle, PARAMETER_WINDOW)
        .await
        .expect("the board answers its parameter list");

    // ArduPilot renamed this parameter: the board reports `MAV_SYSID`, while the catalogue in this project
    // carries the older `SYSID_THISMAV`. A parameter the catalogue does not know still arrives as the vehicle
    // said it, which is why the value is read without the metadata — and the gap itself is recorded in the
    // parity matrix rather than left as a missing entry.
    let says = store
        .get("MAV_SYSID")
        .expect("the board answered its own identity");

    println!(
        "the board says {} = {}; the station keys it as {}",
        says.name, says.value, vehicle
    );

    assert_eq!(
        says.value,
        f64::from(u32::from(vehicle.system_id)),
        "the key the station builds is the number the board holds for itself",
    );

    assert_eq!(
        vehicle.component_id, 1,
        "an autopilot reports on the component the protocol reserves for it",
    );
}

/// The words the board says about itself, read on the channel the interface reads them on.
async fn its_words_reach_the_interface(session: &Session) {
    let deadline = Instant::now() + TEXT_WINDOW;

    while Instant::now() < deadline {
        let state = session.state();

        if state
            .messages
            .iter()
            .any(|message| message.text.contains("PreArm"))
        {
            println!(
                "the board says: {} | capture outside the frames holds {} lines",
                state
                    .messages
                    .first()
                    .map_or(String::new(), |m| m.text.clone()),
                state.output.lines.len(),
            );

            assert!(
                state
                    .messages
                    .iter()
                    .any(|message| message.text.contains("PreArm")),
                "the board's own words reach the console the operator reads",
            );

            /*
             * On a link that answers as a vehicle the words come inside the frames, so the capture of the text
             * outside them stays empty — which is what this run recorded, and it is said rather than turned
             * into a failure. The text that is outside the frames is the startup banner, and that half is
             * asserted against the real capture kept in `tests/fixtures/pixhawk_startup.txt`.
             */
            assert!(state.output.lines.is_empty());

            /*
             * Detection over the lines kept during a running link names nothing, because a board names its
             * model when it starts. The fixture is where that half is read.
             */
            assert_eq!(detect(&state.output.lines), None);

            return;
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    panic!("the board said nothing about itself during the window");
}

/// The answer a board with no logs gives: one entry that says there are none. It is read as an empty list, not
/// as a log of no bytes — which is the divergence this run found, and the rule `logs.rs` now follows.
async fn says_it_has_no_logs(session: &Session, vehicle: VehicleId) {
    let entries = mpcraft_link::logs::list_logs(session, vehicle, TRANSACTION_TIMEOUT)
        .await
        .expect("the board answers the list");

    println!("the board says it holds {} logs", entries.len());

    assert!(
        entries.is_empty(),
        "what the board said is that it has none"
    );
}

/// The board's own filesystem, asked for through the transfer protocol.
///
/// A simulator has no filesystem to reach, so this is the half that only hardware answers for. Three things are
/// read apart here: the command the board answers without touching a filesystem at all, which is where the path
/// is proved before anything is said about the card; the opcode this build of the firmware does not know; and the
/// number the vehicle's file system gives when its drive cannot work.
async fn reaches_its_own_files(session: &Arc<Session>, vehicle: VehicleId) {
    let client = FtpClient::new(session.clone());

    // `ResetSessions` is the one command this board answers without reaching its filesystem, so it is where the
    // path is proved first: the frame, the pair the request carries, and the rule that an answer names the command
    // it is answering. Everything asserted after this depends on those being right.
    let reset = client
        .exchange(ftp_message(Opcode::ResetSessions, "/"))
        .await
        .expect("the board answers the command that never touches a filesystem");

    assert_eq!(reset.opcode, Opcode::Ack);
    assert_eq!(
        reset.req_opcode,
        Opcode::ResetSessions as u8,
        "the answer says which command it is answering",
    );

    /*
     * The listing with the times is the opcode the published dictionary numbers 16, and the client asks for it
     * first. This build of the firmware answers an opcode it does not know with a generic failure that names no
     * number, so the shape the specification gives for an unknown command cannot fire against it — which is the
     * divergence recorded in the parity matrix. It is read here rather than assumed, because the fall-back is
     * written for a shape this board does not produce.
     */
    let unknown = client
        .exchange(ftp_message(Opcode::ListDirectoryWithTime, "/"))
        .await;

    match unknown {
        Err(LinkError::FtpRefused(refusal)) => {
            println!("opcode 16 answered: {refusal}");

            assert_eq!(refusal.req_opcode, Opcode::ListDirectoryWithTime);
            assert_eq!(refusal.code, Some(FtpError::Fail));
            assert_eq!(
                refusal.errno, None,
                "a failure that names no code carries no number"
            );
        }
        other => panic!("the board answered opcode 16 with something else: {other:?}"),
    }

    /*
     * Each exchange takes its own sequence number, which is what keeps these two answers apart: the board re-sends
     * the answer for a number it has already seen, and a probe on the bench that reused one read the same refusal
     * four times.
     */
    let listing = client.list_directory("/").await;

    let refusal = match listing {
        Err(LinkError::FtpRefused(refusal)) => refusal,
        other => panic!("the board answered the listing with something else: {other:?}"),
    };

    println!("the listing read through the client: {refusal}");

    assert_eq!(
        refusal.req_opcode,
        Opcode::ListDirectory,
        "this is the answer to the second ask, so the fall-back ran",
    );

    assert_eq!(refusal.code, Some(FtpError::FailErrno));

    assert_eq!(
        refusal.errno,
        Some(EBUSY),
        "the number is the file system saying that its drive cannot work",
    );

    // The decision about what to do next reads the code rather than the words the message renders, and this
    // refusal is the case where the two differ: its rendering contains the word "Fail" — which is what an earlier
    // rule read — while the code names an error number, and a refusal that names a number is not asked again with
    // another opcode.
    assert!(format!("{refusal}").contains("Fail"));
    assert!(!falls_back_from(&refusal));

    // A refusal reaches the interface as a refusal, with its numbers kept apart from the sentence.
    assert_eq!(LinkError::FtpRefused(refusal).code(), "rejected_by_vehicle");

    /*
     * What the number means is said by the same board in another channel: the startup text keeps the line about
     * the card it could not start, and the mapping ArduPilot's file system uses gives that state the number
     * asserted above. Two channels saying one fact, and neither is read out of prose for meaning.
     */
    let boot = include_str!("fixtures/pixhawk_startup.txt");

    assert!(
        boot.contains("SDCard failed to start"),
        "the card the number says cannot work is the card the board said did not start",
    );

    /*
     * What this area cannot assert is a filesystem. Nothing can be listed, read or parsed while the drive cannot
     * work, so what is proved here is the part that does not depend on a card: that a refusal travels as the
     * numbers the vehicle named. The pair the request carries is proved by the answer arriving at all — this
     * board said nothing on the bench to a request addressed to the reserved pair and answered the one addressed
     * to the pair it chose for itself, which is what `encode_ftp` now writes rather than what it assumed.
     */
    assert_eq!(
        session.state().selected,
        Some(vehicle),
        "the request was addressed to the vehicle the link chose, not to an assumed address",
    );
}

/// A request in the shape the transfer protocol carries it: the sequence number is given by the client when the
/// command goes out, and the path asked for is the data, inside the block of bytes a payload can carry.
fn ftp_message(opcode: Opcode, path: &str) -> FtpMessage {
    FtpMessage {
        seq_number: 0,
        session: 0,
        opcode,
        size: path.len().min(239) as u8,
        req_opcode: 0,
        burst_complete: 0,
        padding: 0,
        offset: 0,
        data: path.as_bytes().to_vec(),
    }
}
