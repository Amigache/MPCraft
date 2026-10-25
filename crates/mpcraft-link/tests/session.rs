//! End-to-end tests of the session.
//!
//! These drive a real session over a real transport — a file replaying a recorded byte
//! stream — and assert that the vehicle state comes out the other end.

use std::time::Duration;

use mpcraft_link::{Session, SessionOptions};
use mpcraft_mavlink::dialect::messages::{
    GLOBAL_POSITION_INT_DATA, HEARTBEAT_DATA, SYS_STATUS_DATA,
};
use mpcraft_mavlink::frame::{encode_v2, MavHeader};
use mpcraft_mavlink::{MavAutopilot, MavMessage, MavType};
use mpcraft_transport::ConnectionSpec;

/// Builds a MAVLink 2 frame for a message.
fn frame(message: MavMessage, sequence: u8) -> Vec<u8> {
    let header = MavHeader {
        system_id: 1,
        component_id: 1,
        sequence,
    };
    encode_v2(&message, header)
}

/// A heartbeat from a quadcopter.
fn heartbeat(custom_mode: u32) -> MavMessage {
    MavMessage::HEARTBEAT(HEARTBEAT_DATA {
        custom_mode,
        mavtype: MavType::MAV_TYPE_QUADROTOR,
        autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
        ..Default::default()
    })
}

/// A global position report.
fn global_position() -> MavMessage {
    MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
        time_boot_ms: 1_000,
        lat: (35.2809 * 1e7) as i32,
        lon: (149.1300 * 1e7) as i32,
        alt: 584_000,
        relative_alt: 100_000,
        vx: 250,
        vy: 0,
        vz: 0,
        hdg: 9_000,
    })
}

/// Writes a stream to a temporary file and returns its path.
fn temp_stream(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("mpcraft-session-{name}.tlog"));
    std::fs::write(&path, bytes).expect("write the temporary stream");
    path
}

/// Waits for a condition to hold, polling the session state.
async fn wait_for<F>(session: &Session, timeout: Duration, condition: F) -> bool
where
    F: Fn(&mpcraft_link::SessionState) -> bool,
{
    let deadline = std::time::Instant::now() + timeout;

    while std::time::Instant::now() < deadline {
        if condition(&session.state()) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    condition(&session.state())
}

#[tokio::test]
async fn a_session_decodes_a_stream_into_telemetry() {
    // Two heartbeats, so the vehicle is identified, then a position and a battery report.
    let mut stream = Vec::new();
    stream.extend(frame(heartbeat(5), 0));
    stream.extend(frame(heartbeat(5), 1));
    stream.extend(frame(global_position(), 2));
    stream.extend(frame(
        MavMessage::SYS_STATUS(SYS_STATUS_DATA {
            voltage_battery: 12_340,
            current_battery: -1500,
            battery_remaining: 87,
            ..Default::default()
        }),
        3,
    ));

    let path = temp_stream("telemetry", &stream);
    let spec = ConnectionSpec::parse(&format!("file:{}", path.display())).unwrap();

    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap();

    let got_telemetry = wait_for(&session, Duration::from_secs(5), |state| {
        state.telemetry.position.is_some() && state.telemetry.battery.is_some()
    })
    .await;

    assert!(got_telemetry, "the telemetry was never decoded");

    let state = session.state();

    let position = state.telemetry.position.expect("a position");
    assert!((position.lat - 35.2809).abs() < 1e-4);
    assert!((position.lng - 149.13).abs() < 1e-4);
    assert!((position.alt - 584.0).abs() < 1.0);

    let battery = state.telemetry.battery.expect("a battery");
    assert!((battery.voltage - 12.34).abs() < 1e-6);
    assert!((battery.remaining - 87.0).abs() < 1e-6);

    let velocity = state.telemetry.velocity.expect("a velocity");
    assert!(
        (velocity.north - 2.5).abs() < 1e-6,
        "north was {}",
        velocity.north
    );
    assert!((velocity.ground_speed - 2.5).abs() < 1e-6);

    // The vehicle must have been identified from its heartbeat.
    assert!(!state.vehicles.is_empty(), "no vehicle was identified");
    let vehicle = &state.vehicles[0];
    assert_eq!(vehicle.id.system_id, 1);
    assert_eq!(vehicle.id.component_id, 1);
    assert_eq!(
        vehicle.kind,
        Some(mpcraft_link::VehicleKind::ArduCopter),
        "the vehicle family was not detected"
    );

    // The packets were counted.
    assert_eq!(state.loss.packets_received, 4);
    assert_eq!(state.loss.packets_lost, 0);
}

#[tokio::test]
async fn the_mode_and_the_armed_state_come_from_the_heartbeat() {
    let mut stream = Vec::new();
    stream.extend(frame(heartbeat(6), 0));
    stream.extend(frame(heartbeat(6), 1));

    let path = temp_stream("mode", &stream);
    let spec = ConnectionSpec::parse(&format!("file:{}", path.display())).unwrap();

    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap();

    let got = wait_for(&session, Duration::from_secs(5), |state| {
        state.telemetry.mode.is_some()
    })
    .await;

    assert!(got, "the mode was never decoded");
    assert_eq!(
        session
            .state()
            .telemetry
            .mode
            .as_ref()
            .map(|m| m.custom_mode),
        Some(6)
    );
}

#[tokio::test]
async fn the_board_output_outside_of_frames_is_captured() {
    // A board that prints its banner, followed by a heartbeat.
    let mut stream = b"ArduCopter V4.3.7 (0123456789)\r\n".to_vec();
    stream.extend(frame(heartbeat(5), 0));
    stream.extend(frame(heartbeat(5), 1));

    let path = temp_stream("plaintext", &stream);
    let spec = ConnectionSpec::parse(&format!("file:{}", path.display())).unwrap();

    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap();

    let got = wait_for(&session, Duration::from_secs(5), |state| {
        !state.output.lines.is_empty()
    })
    .await;

    assert!(got, "the board output was not captured");
    let state = session.state();
    assert!(
        state.output.lines[0].contains("ArduCopter"),
        "the captured text was {:?}",
        state.output.lines
    );

    // Nothing was dropped, and the banner is an ordinary banner: the state carries both facts,
    // which is what the console draws.
    assert_eq!(state.output.dropped, 0);
    assert!(!state.output.looks_like_can_bus);
}

#[tokio::test]
async fn a_capture_that_is_full_says_what_it_dropped() {
    // A board that prints more than the window holds, and prints it as a DroneCAN node would:
    // ninety lines, each one a 'T' followed by hexadecimal digits. The window keeps the newest
    // fifty-two — the hand: ninety lines keep fifty-two and drop thirty-eight — and the state
    // says so, because a window that showed only what it kept would look like the whole output.
    let mut stream = Vec::new();
    for index in 0..90 {
        stream.extend(format!("T{:09X}\r\n", index).as_bytes());
    }
    stream.extend(frame(heartbeat(5), 0));
    stream.extend(frame(heartbeat(5), 1));

    let path = temp_stream("dropped-plaintext", &stream);
    let spec = ConnectionSpec::parse(&format!("file:{}", path.display())).unwrap();

    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap();

    let seen = wait_for(&session, Duration::from_secs(5), |state| {
        state.output.lines.len() == 52 && state.output.dropped == 38
    })
    .await;

    assert!(
        seen,
        "the capture should have filled and said what it dropped"
    );

    let state = session.state();
    assert!(state.output.looks_like_can_bus);
}

#[tokio::test]
async fn a_session_over_an_empty_stream_closes_cleanly() {
    let path = temp_stream("empty", b"");
    let spec = ConnectionSpec::parse(&format!("file:{}", path.display())).unwrap();

    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap();

    let closed = wait_for(&session, Duration::from_secs(5), |state| !state.connected).await;

    assert!(closed, "the session did not close on an empty stream");
    assert!(session.state().vehicles.is_empty());
}

#[tokio::test]
async fn a_corrupt_frame_does_not_stop_the_session() {
    // A frame with a broken checksum, followed by a good one.
    let mut stream = Vec::new();

    let mut corrupt = frame(heartbeat(5), 0);
    let last = corrupt.len() - 1;
    corrupt[last] ^= 0xFF;
    stream.extend(corrupt);

    stream.extend(frame(heartbeat(5), 1));
    stream.extend(frame(global_position(), 2));

    let path = temp_stream("corrupt", &stream);
    let spec = ConnectionSpec::parse(&format!("file:{}", path.display())).unwrap();

    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap();

    let got = wait_for(&session, Duration::from_secs(5), |state| {
        state.telemetry.position.is_some()
    })
    .await;

    assert!(got, "the good frame after the corrupt one was not decoded");
}
