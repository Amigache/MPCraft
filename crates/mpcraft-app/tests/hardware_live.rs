//! A real board on the other end of the port, read as a vehicle.
//!
//! A simulator cannot say what a board says about itself. It does not name its hardware, it does not fail to
//! find a sensor, it does not choose an address for itself, and it does not have a card that failed to start.
//! What this area asserts is the half that only hardware can answer for: the pair the board introduces itself
//! with, the capabilities it declares, the family its heartbeat names, the words it says about itself, and the
//! answer it gives when asked for the logs it holds.
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

use std::time::Duration;

use tokio::time::Instant;

use mpcraft_app::board::detect;
use mpcraft_link::vehicle::VehicleKind;
use mpcraft_link::{Session, SessionOptions, VehicleId};
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

/// The address the board is on.
fn target() -> String {
    std::env::var("MPCRAFT_HARDWARE_TARGET").expect("the gate names the port the board is on")
}

#[tokio::test]
async fn a_board_says_what_a_simulator_cannot() {
    let spec = ConnectionSpec::parse(&target()).expect("the target is a connection specification");
    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .expect("the board can be reached");

    let vehicle = session
        .wait_for_any_vehicle(CONNECT_TIMEOUT)
        .await
        .expect("a board answers its heartbeat");

    reads_the_family_it_names(&session, vehicle);
    declares_what_it_can(&session, vehicle).await;
    keeps_the_number_it_holds_for_itself(&session, vehicle).await;
    its_words_reach_the_interface(&session).await;
    says_it_has_no_logs(&session, vehicle).await;

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
