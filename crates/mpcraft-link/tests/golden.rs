//! The golden-file test.
//!
//! `tests/fixtures/stream.tlog` is produced by `scripts/generate-test-stream.py`, which is a
//! **second, independent implementation** of the MAVLink wire format. The Rust parser is
//! therefore checked against something other than itself: when the two disagree, one of them
//! has a bug in the framing, the header layout or the CRC.
//!
//! It is not a substitute for a capture from a real vehicle — that is added by the SITL job
//! in CI — but it catches the class of bug where an implementation is self-consistent and
//! wrong. It already did, twice: the message id was being read from the wrong offset, and
//! MAVLink 2's trailing-zero payload truncation was not accounted for.
//!
//! # What the fixture exercises
//!
//! * MAVLink 2 framing with the message id at offset seven.
//! * The CRC with each message's own CRC_EXTRA seed.
//! * **Payload truncation**: MAVLink 2 removes the trailing zero bytes of a payload. The
//!   `ATTITUDE` and `HOME_POSITION` messages in the fixture are truncated, so the parser is
//!   tested against the shorter payload it will really receive.

use std::time::Duration;

use mpcraft_link::{Session, SessionOptions};
use mpcraft_mavlink::dialect::messages::{
    ATTITUDE_DATA, GLOBAL_POSITION_INT_DATA, HEARTBEAT_DATA, HOME_POSITION_DATA, SYS_STATUS_DATA,
};
use mpcraft_mavlink::frame::{encode_v2, FrameParser, MavHeader};
use mpcraft_mavlink::{
    MavAutopilot, MavMessage, MavModeFlag, MavState, MavSysStatusSensor, MavType, Message,
};
use mpcraft_transport::ConnectionSpec;

/// The fixture, as committed.
const FIXTURE: &[u8] = include_bytes!("fixtures/stream.tlog");

/// The number of frames in the fixture.
const FRAME_COUNT: usize = 12;

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

/// Parses the fixture into frames.
fn parse_fixture() -> Vec<mpcraft_mavlink::Frame> {
    let mut parser = FrameParser::new();
    let mut frames = Vec::new();
    parser.feed(FIXTURE, &mut frames).unwrap();
    frames
}

#[test]
fn every_frame_in_the_fixture_parses_and_validates() {
    let mut parser = FrameParser::new();
    let mut frames = Vec::new();

    parser.feed(FIXTURE, &mut frames).unwrap();

    assert_eq!(
        frames.len(),
        FRAME_COUNT,
        "every frame should have been parsed"
    );
    assert_eq!(parser.stats().frames_parsed, FRAME_COUNT as u64);
    assert_eq!(parser.stats().crc_errors, 0, "no frame should fail its CRC");
    assert_eq!(parser.stats().unknown_messages, 0);
    assert_eq!(parser.stats().bytes_discarded, 0);
    assert_eq!(parser.stats().resyncs, 0);

    for frame in &frames {
        assert!(
            FrameParser::verify_crc(frame).is_ok(),
            "frame {} failed its CRC",
            frame.message_id
        );
    }
}

#[test]
fn the_fixture_has_the_expected_message_sequence() {
    let frames = parse_fixture();

    let ids: Vec<u32> = frames.iter().map(|f| f.message_id).collect();

    assert_eq!(
        ids,
        vec![
            0,   // HEARTBEAT
            0,   // HEARTBEAT
            33,  // GLOBAL_POSITION_INT
            30,  // ATTITUDE
            1,   // SYS_STATUS
            33,  // GLOBAL_POSITION_INT
            30,  // ATTITUDE
            1,   // SYS_STATUS
            33,  // GLOBAL_POSITION_INT
            30,  // ATTITUDE
            1,   // SYS_STATUS
            242, // HOME_POSITION
        ]
    );
}

#[test]
fn the_frames_carry_the_expected_values() {
    let frames = parse_fixture();

    let heartbeat = &frames[0];
    assert_eq!(heartbeat.system_id, 1);
    assert_eq!(heartbeat.component_id, 1);
    assert_eq!(heartbeat.sequence, 0);
    assert_eq!(heartbeat.payload.len(), 9, "HEARTBEAT is nine bytes");

    let position = &frames[2];
    assert_eq!(position.message_id, 33);
    assert_eq!(
        position.payload.len(),
        28,
        "GLOBAL_POSITION_INT is 28 bytes and its last field is not zero"
    );

    // MAVLink 2 removes the trailing zero bytes of the payload. The angular rates of the
    // attitude are zero, so the payload is cut short.
    let attitude = &frames[3];
    assert_eq!(attitude.message_id, 30);
    assert_eq!(
        attitude.payload.len(),
        16,
        "the trailing zero bytes are truncated in MAVLink 2"
    );

    // The home position ends with three zero floats, so it is truncated too.
    let home = &frames[11];
    assert_eq!(home.message_id, 242);
    assert_eq!(home.payload.len(), 28);
}

#[test]
fn the_truncated_payload_is_padded_back_when_it_is_decoded() {
    let frames = parse_fixture();

    // The attitude message is decoded from a sixteen byte payload; the receiver must pad it
    // back to the message's declared length before reading the fields.
    let attitude = &frames[3];
    let decoded = MavMessage::parse(
        mpcraft_mavlink::MavlinkVersion::V2,
        attitude.message_id,
        &attitude.payload,
    )
    .expect("the truncated attitude should decode");

    let MavMessage::ATTITUDE(values) = decoded else {
        panic!("the frame should decode as an attitude");
    };

    assert!((values.roll - 0.05).abs() < 1e-6);
    assert!((values.pitch - -0.02).abs() < 1e-6);
    assert!((values.yaw - 1.57).abs() < 1e-6);
    // The truncated fields come back as zero.
    assert!((values.rollspeed - 0.0).abs() < 1e-6);
    assert!((values.yawspeed - 0.0).abs() < 1e-6);
}

#[tokio::test]
async fn the_fixture_decodes_into_telemetry() {
    let path = std::env::temp_dir().join("mpcraft-golden-stream.tlog");
    std::fs::write(&path, FIXTURE).unwrap();

    let spec = ConnectionSpec::parse(&format!("file:{}", path.display())).unwrap();
    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap();

    let got = wait_for(&session, Duration::from_secs(5), |state| {
        state.telemetry.position.is_some() && state.telemetry.home.is_some()
    })
    .await;

    assert!(got, "the fixture did not decode into telemetry");

    let state = session.state();

    let position = state.telemetry.position.expect("a position");
    assert!((position.lat - 35.2809).abs() < 1e-6);
    assert!((position.lng - 149.1300).abs() < 1e-6);
    assert!((position.alt - 584.0).abs() < 0.01);

    let attitude = state.telemetry.attitude.expect("an attitude");
    assert!((attitude.roll - 0.05).abs() < 1e-6);
    assert!((attitude.pitch - -0.02).abs() < 1e-6);
    assert!((attitude.yaw - 1.57).abs() < 1e-6);

    let battery = state.telemetry.battery.expect("a battery");
    assert!((battery.voltage - 12.34).abs() < 1e-6);
    assert!((battery.remaining - 87.0).abs() < 1e-6);

    // The home position was reported, so the distance to home is known.
    let distance = state
        .telemetry
        .distance_to_home
        .expect("a distance to home");
    assert!(distance < 1.0, "the distance to home was {distance}");

    // The vehicle was identified as an ArduCopter.
    let vehicle = state.vehicles.first().expect("a vehicle");
    assert_eq!(vehicle.kind, Some(mpcraft_link::VehicleKind::ArduCopter));
    assert_eq!(vehicle.mav_type, Some(MavType::MAV_TYPE_QUADROTOR));

    // The sequence numbers in the fixture are contiguous, so nothing was lost.
    assert_eq!(state.loss.packets_received, FRAME_COUNT as u64);
    assert_eq!(state.loss.packets_lost, 0);
}

/// A battery report with every sensor mask empty.
///
/// The generated default sets the first bit of each sensor mask; the generator writes zeros.
/// Both are valid, but the two implementations must be given the same input to be compared.
fn sys_status() -> MavMessage {
    MavMessage::SYS_STATUS(SYS_STATUS_DATA {
        onboard_control_sensors_present: MavSysStatusSensor::empty(),
        onboard_control_sensors_enabled: MavSysStatusSensor::empty(),
        onboard_control_sensors_health: MavSysStatusSensor::empty(),
        voltage_battery: 12_340,
        current_battery: -1_500,
        battery_remaining: 87,
        ..Default::default()
    })
}

#[test]
fn the_rust_encoder_and_the_python_generator_agree_on_every_frame() {
    // The two implementations must produce byte-identical frames. If they do not, one of them
    // has a bug in the framing, the header layout, the CRC or the payload truncation.
    let heartbeat = MavMessage::HEARTBEAT(HEARTBEAT_DATA {
        custom_mode: 5,
        mavtype: MavType::MAV_TYPE_QUADROTOR,
        autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
        base_mode: MavModeFlag::empty(),
        system_status: MavState::MAV_STATE_ACTIVE,
        mavlink_version: 3,
    });

    let position = MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
        time_boot_ms: 0,
        lat: (35.2809 * 1e7) as i32,
        lon: (149.1300 * 1e7) as i32,
        alt: 584_000,
        relative_alt: 100_000,
        vx: 250,
        vy: 0,
        vz: 0,
        hdg: 9_000,
    });

    let attitude = MavMessage::ATTITUDE(ATTITUDE_DATA {
        time_boot_ms: 0,
        roll: 0.05,
        pitch: -0.02,
        yaw: 1.57,
        rollspeed: 0.0,
        pitchspeed: 0.0,
        yawspeed: 0.0,
    });

    let home = MavMessage::HOME_POSITION(HOME_POSITION_DATA {
        latitude: (35.2809 * 1e7) as i32,
        longitude: (149.1300 * 1e7) as i32,
        altitude: 584_000,
        // A zero rotation is (1, 0, 0, 0) in the MAVLink quaternion convention.
        q: [1.0, 0.0, 0.0, 0.0],
        ..Default::default()
    });

    // The messages the generator sends, in order.
    let expected: Vec<(MavMessage, u8)> = vec![
        (heartbeat.clone(), 0),
        (heartbeat.clone(), 1),
        (position.clone(), 2),
        (attitude.clone(), 3),
        (sys_status(), 4),
        (position.clone(), 5),
        (attitude.clone(), 6),
        (sys_status(), 7),
        (position.clone(), 8),
        (attitude.clone(), 9),
        (sys_status(), 10),
        (home.clone(), 11),
    ];

    let mut offset = 0;

    for (message, sequence) in &expected {
        let header = MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: *sequence,
        };

        let encoded = encode_v2(message, header);

        assert_eq!(
            &FIXTURE[offset..offset + encoded.len()],
            &encoded[..],
            "the Rust encoder and the Python generator disagree on message {message_id}",
            message_id = message.message_id()
        );

        offset += encoded.len();
    }

    assert_eq!(offset, FIXTURE.len(), "the fixture has trailing bytes");
}

#[test]
fn the_fixture_is_not_accepted_after_a_single_corruption() {
    // Flip a bit in the payload of the first frame.
    let mut corrupted = FIXTURE.to_vec();
    corrupted[12] ^= 0x01;

    let mut parser = FrameParser::new();
    let mut frames = Vec::new();
    parser.feed(&corrupted, &mut frames).unwrap();

    assert_eq!(
        parser.stats().crc_errors,
        1,
        "the corrupted frame should have been rejected"
    );
    assert_eq!(
        frames.len(),
        FRAME_COUNT - 1,
        "the other frames should still be parsed"
    );
}
