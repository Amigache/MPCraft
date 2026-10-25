//! The command protocol over a real socket.
//!
//! The suite runs a **simulated vehicle on a UDP socket** and flies it through the
//! operations a ground station needs. The exit criterion is the one the protocol has to
//! meet: a command the vehicle accepts is reported as accepted, and a command the
//! vehicle refuses is an error that says so.

// `SET_MODE` is deprecated in favour of `MAV_CMD_DO_SET_MODE`, but the ground station
// sends both: the legacy message carries the custom mode exactly.
#![allow(deprecated)]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpcraft_link::{CommandError, LinkError, Session, SessionOptions, VehicleId};
use mpcraft_mavlink::dialect::messages::COMMAND_ACK_DATA;
use mpcraft_mavlink::frame::{encode_v2, FrameParser, MavHeader};
use mpcraft_mavlink::{MavCmd, MavMessage, MavModeFlag, MavResult, Message};

/// The frames the vehicle received, so the test can assert on the bytes.
type Received = Arc<Mutex<Vec<MavMessage>>>;

/// A simulated vehicle that acknowledges the commands it receives.
struct Vehicle {
    socket: tokio::net::UdpSocket,
    sequence: u8,
    /// The result the vehicle reports for every command.
    result: MavResult,
    /// Whether the vehicle answers at all.
    silent: bool,
    /// The commands the vehicle received.
    received: Received,
}

impl Vehicle {
    /// Starts a vehicle that answers with the given result.
    async fn start(result: MavResult) -> (SocketAddr, Received) {
        Self::start_with(result, false).await
    }

    /// Starts a vehicle, silent or not.
    async fn start_with(result: MavResult, silent: bool) -> (SocketAddr, Received) {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();

        let received: Received = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&received);

        let mut vehicle = Self {
            socket,
            sequence: 0,
            result,
            silent,
            received: log,
        };

        tokio::spawn(async move {
            vehicle.run().await;
        });

        (address, received)
    }

    async fn run(&mut self) {
        let mut buffer = [0u8; 2048];

        loop {
            let Ok((len, peer)) = self.socket.recv_from(&mut buffer).await else {
                return;
            };

            let Some(start) = buffer[..len].iter().position(|&b| b == 0xFD || b == 0xFE) else {
                continue;
            };

            let mut parser = FrameParser::new();
            let mut frames = Vec::new();
            if parser.feed(&buffer[start..len], &mut frames).is_err() {
                continue;
            }

            for frame in &frames {
                let Ok(decoded) =
                    MavMessage::parse(frame.version, frame.message_id, &frame.payload)
                else {
                    continue;
                };

                // The ground station announces itself with a heartbeat when it connects.
                // It is not part of what the test is about.
                if matches!(decoded, MavMessage::HEARTBEAT(_)) {
                    continue;
                }

                if let Ok(mut log) = self.received.lock() {
                    log.push(decoded.clone());
                }

                self.answer(&decoded, peer).await;
            }
        }
    }

    /// Answers a command with an acknowledgement.
    async fn answer(&mut self, request: &MavMessage, peer: SocketAddr) {
        if self.silent {
            return;
        }

        let command = match request {
            MavMessage::COMMAND_LONG(command) => command.command,
            MavMessage::COMMAND_INT(command) => command.command,
            _ => return,
        };

        self.send(
            peer,
            MavMessage::COMMAND_ACK(COMMAND_ACK_DATA {
                command,
                result: self.result,
                ..Default::default()
            }),
        )
        .await;
    }

    async fn send(&mut self, peer: SocketAddr, message: MavMessage) {
        let header = MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.wrapping_add(1);

        let bytes = encode_v2(&message, header);
        let _ = self.socket.send_to(&bytes, peer).await;
    }
}

/// Opens a session to a vehicle.
async fn session_to(address: SocketAddr) -> Session {
    let spec = mpcraft_transport::ConnectionSpec::parse(&format!("udp:{address}")).unwrap();
    Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap()
}

/// The vehicle the session talks to.
fn vehicle() -> VehicleId {
    VehicleId::new(1, 1)
}

/// Waits until the vehicle has received at least `count` messages.
async fn wait_for_messages(received: &Received, count: usize) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);

    while std::time::Instant::now() < deadline {
        if received.lock().map(|log| log.len()).unwrap_or(0) >= count {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    false
}

/// The commands the vehicle received.
fn commands_of(received: &Received) -> Vec<MavCmd> {
    received
        .lock()
        .map(|log| {
            log.iter()
                .filter_map(|message| match message {
                    MavMessage::COMMAND_LONG(command) => Some(command.command),
                    MavMessage::COMMAND_INT(command) => Some(command.command),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn a_command_the_vehicle_acknowledges_is_accepted() {
    let (address, received) = Vehicle::start(MavResult::MAV_RESULT_ACCEPTED).await;
    let session = session_to(address).await;

    let result = session
        .command_long(
            vehicle(),
            MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
            [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            Duration::from_secs(5),
        )
        .await
        .expect("the command should be acknowledged");

    assert_eq!(result, MavResult::MAV_RESULT_ACCEPTED);

    // The command that went out is the one that was asked for.
    assert!(wait_for_messages(&received, 1).await);
    assert_eq!(
        commands_of(&received),
        vec![MavCmd::MAV_CMD_COMPONENT_ARM_DISARM]
    );
}

#[tokio::test]
async fn a_command_the_vehicle_refuses_is_an_error_that_carries_the_result() {
    let (address, _) = Vehicle::start(MavResult::MAV_RESULT_DENIED).await;
    let session = session_to(address).await;

    let error = session
        .arm(vehicle(), Duration::from_secs(5))
        .await
        .expect_err("the vehicle refused the command");

    match error {
        LinkError::Command(CommandError::Refused { command, result }) => {
            assert_eq!(command, MavCmd::MAV_CMD_COMPONENT_ARM_DISARM);
            assert_eq!(result, MavResult::MAV_RESULT_DENIED);
        }
        other => panic!("the refusal was not reported as one: {other:?}"),
    }
}

#[tokio::test]
async fn a_vehicle_that_does_not_answer_is_a_timeout() {
    let (address, _) = Vehicle::start_with(MavResult::MAV_RESULT_ACCEPTED, true).await;
    let session = session_to(address).await;

    let error = session
        .disarm(vehicle(), Duration::from_millis(400))
        .await
        .expect_err("the vehicle never answered");

    assert!(
        matches!(error, LinkError::Timeout(_)),
        "the silence was not reported as a timeout: {error:?}"
    );
}

#[tokio::test]
async fn the_arm_command_carries_the_arm_parameter() {
    let (address, received) = Vehicle::start(MavResult::MAV_RESULT_ACCEPTED).await;
    let session = session_to(address).await;

    session
        .arm(vehicle(), Duration::from_secs(5))
        .await
        .expect("the vehicle should accept the command");

    assert!(wait_for_messages(&received, 1).await);

    let log = received.lock().unwrap();
    let Some(MavMessage::COMMAND_LONG(command)) = log.first() else {
        panic!("the vehicle did not receive a command");
    };

    assert_eq!(command.command, MavCmd::MAV_CMD_COMPONENT_ARM_DISARM);
    assert_eq!(command.param1, 1.0, "the arm parameter was not set");
    assert_eq!(command.target_system, 1);
    assert_eq!(command.target_component, 1);
}

#[tokio::test]
async fn the_disarm_command_carries_the_disarm_parameter() {
    let (address, received) = Vehicle::start(MavResult::MAV_RESULT_ACCEPTED).await;
    let session = session_to(address).await;

    session
        .disarm(vehicle(), Duration::from_secs(5))
        .await
        .expect("the vehicle should accept the command");

    assert!(wait_for_messages(&received, 1).await);

    let log = received.lock().unwrap();
    let Some(MavMessage::COMMAND_LONG(command)) = log.first() else {
        panic!("the vehicle did not receive a command");
    };

    assert_eq!(command.param1, 0.0, "the disarm parameter was not set");
}

#[tokio::test]
async fn the_takeoff_command_carries_the_altitude() {
    let (address, received) = Vehicle::start(MavResult::MAV_RESULT_ACCEPTED).await;
    let session = session_to(address).await;

    session
        .takeoff(vehicle(), 25.0, Duration::from_secs(5))
        .await
        .expect("the vehicle should accept the command");

    assert!(wait_for_messages(&received, 1).await);

    let log = received.lock().unwrap();
    let Some(MavMessage::COMMAND_LONG(command)) = log.first() else {
        panic!("the vehicle did not receive a command");
    };

    assert_eq!(command.command, MavCmd::MAV_CMD_NAV_TAKEOFF);
    assert_eq!(command.param1, 25.0, "the altitude was not carried");
}

#[tokio::test]
async fn the_mode_change_sends_both_the_message_and_the_command() {
    let (address, received) = Vehicle::start(MavResult::MAV_RESULT_ACCEPTED).await;
    let session = session_to(address).await;

    session
        .set_mode(
            vehicle(),
            5,
            MavModeFlag::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED,
            Duration::from_secs(5),
        )
        .await
        .expect("the vehicle should accept the command");

    assert!(wait_for_messages(&received, 2).await);

    let log = received.lock().unwrap();

    // The legacy message carries the custom mode exactly.
    let set_mode = log
        .iter()
        .find_map(|message| match message {
            MavMessage::SET_MODE(mode) => Some(mode),
            _ => None,
        })
        .expect("the vehicle did not receive a SET_MODE");

    assert_eq!(set_mode.custom_mode, 5);
    assert_eq!(set_mode.target_system, 1);

    // The command is what the vehicle acknowledges.
    let command = log
        .iter()
        .find_map(|message| match message {
            MavMessage::COMMAND_LONG(command) => Some(command),
            _ => None,
        })
        .expect("the vehicle did not receive a command");

    assert_eq!(command.command, MavCmd::MAV_CMD_DO_SET_MODE);
    assert_eq!(command.param2, 5.0, "the custom mode was not carried");
}

#[tokio::test]
async fn the_integer_command_carries_the_position() {
    let (address, received) = Vehicle::start(MavResult::MAV_RESULT_ACCEPTED).await;
    let session = session_to(address).await;

    let result = session
        .command_int(
            vehicle(),
            MavCmd::MAV_CMD_NAV_TAKEOFF,
            [0, 0, 0, 0, -35_362_909, 149_130_000, 100],
            Duration::from_secs(5),
        )
        .await
        .expect("the vehicle should acknowledge the command");

    assert_eq!(result, MavResult::MAV_RESULT_ACCEPTED);

    assert!(wait_for_messages(&received, 1).await);

    let log = received.lock().unwrap();
    let Some(MavMessage::COMMAND_INT(command)) = log.first() else {
        panic!("the vehicle did not receive an integer command");
    };

    assert_eq!(command.x, -35_362_909);
    assert_eq!(command.y, 149_130_000);
    assert_eq!(command.z, 100.0);
}

#[tokio::test]
async fn an_acknowledgement_for_another_command_is_ignored() {
    // A vehicle that answers the arming command with an acknowledgement for a different
    // command must not be taken for an answer: the transaction keeps waiting.
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();

    tokio::spawn(async move {
        let mut buffer = [0u8; 2048];
        let mut sequence = 0u8;

        loop {
            let Ok((len, peer)) = socket.recv_from(&mut buffer).await else {
                return;
            };

            let mut parser = FrameParser::new();
            let mut frames = Vec::new();
            if parser.feed(&buffer[..len], &mut frames).is_err() {
                continue;
            }

            for frame in &frames {
                let Ok(MavMessage::COMMAND_LONG(command)) =
                    MavMessage::parse(frame.version, frame.message_id, &frame.payload)
                else {
                    continue;
                };

                // The acknowledgement is for the wrong command on purpose.
                let header = MavHeader {
                    system_id: 1,
                    component_id: 1,
                    sequence,
                };
                sequence = sequence.wrapping_add(1);

                let bytes = encode_v2(
                    &MavMessage::COMMAND_ACK(COMMAND_ACK_DATA {
                        command: MavCmd::MAV_CMD_NAV_TAKEOFF,
                        result: MavResult::MAV_RESULT_ACCEPTED,
                        ..Default::default()
                    }),
                    header,
                );
                let _ = socket.send_to(&bytes, peer).await;
                let _ = command;
            }
        }
    });

    let session = session_to(address).await;

    let error = session
        .arm(vehicle(), Duration::from_millis(400))
        .await
        .expect_err("the acknowledgement was for another command");

    assert!(
        matches!(error, LinkError::Timeout(_)),
        "the acknowledgement for another command was taken for an answer: {error:?}"
    );
}

#[tokio::test]
async fn an_interim_acknowledgement_is_not_the_answer() {
    // The vehicle says the command is running and then reports the final result.
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();

    tokio::spawn(async move {
        let mut buffer = [0u8; 2048];
        let mut sequence = 0u8;

        loop {
            let Ok((len, peer)) = socket.recv_from(&mut buffer).await else {
                return;
            };

            let mut parser = FrameParser::new();
            let mut frames = Vec::new();
            if parser.feed(&buffer[..len], &mut frames).is_err() {
                continue;
            }

            for frame in &frames {
                let Ok(MavMessage::COMMAND_LONG(command)) =
                    MavMessage::parse(frame.version, frame.message_id, &frame.payload)
                else {
                    continue;
                };

                for result in [
                    MavResult::MAV_RESULT_IN_PROGRESS,
                    MavResult::MAV_RESULT_ACCEPTED,
                ] {
                    let header = MavHeader {
                        system_id: 1,
                        component_id: 1,
                        sequence,
                    };
                    sequence = sequence.wrapping_add(1);

                    let bytes = encode_v2(
                        &MavMessage::COMMAND_ACK(COMMAND_ACK_DATA {
                            command: command.command,
                            result,
                            ..Default::default()
                        }),
                        header,
                    );
                    let _ = socket.send_to(&bytes, peer).await;
                }
            }
        }
    });

    let session = session_to(address).await;

    let result = session
        .command_long(
            vehicle(),
            MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
            [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            Duration::from_secs(5),
        )
        .await
        .expect("the final acknowledgement should be the answer");

    assert_eq!(result, MavResult::MAV_RESULT_ACCEPTED);
}

#[tokio::test]
async fn a_command_is_sent_again_when_the_vehicle_stays_silent() {
    // A lost frame is the common failure on a noisy link. The command is written a second
    // time before the operation gives up.
    let (address, received) = Vehicle::start_with(MavResult::MAV_RESULT_ACCEPTED, true).await;
    let session = session_to(address).await;

    let _ = session
        .arm(vehicle(), Duration::from_millis(400))
        .await
        .expect_err("the vehicle never answered");

    assert!(wait_for_messages(&received, 2).await);
    assert_eq!(
        commands_of(&received),
        vec![
            MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
            MavCmd::MAV_CMD_COMPONENT_ARM_DISARM
        ],
        "the command was not repeated"
    );
}

#[tokio::test]
async fn the_message_ids_are_the_ones_the_protocol_says() {
    // The ids are part of the contract with the vehicle; a change would break the link
    // silently.
    assert_eq!(mpcraft_link::commands::COMMAND_LONG_ID, 76);
    assert_eq!(mpcraft_link::commands::COMMAND_INT_ID, 75);
    assert_eq!(mpcraft_link::commands::COMMAND_ACK_ID, 77);
    assert_eq!(mpcraft_link::commands::SET_MODE_ID, 11);
    assert_eq!(mpcraft_link::commands::SET_ATTITUDE_TARGET_ID, 82);
    // The report of the relays is message 376, and the dialect agrees with the number written
    // down here: the seed of its checksum is computed from its fields, so a frame of this message
    // is checked against the same seed the vehicle uses.
    assert_eq!(mpcraft_link::commands::RELAY_STATUS_ID, 376);
    assert_eq!(
        mpcraft_mavlink::dialect::message_id("RELAY_STATUS"),
        Some(mpcraft_link::commands::RELAY_STATUS_ID)
    );
}

#[tokio::test]
async fn the_answer_the_formation_decides_reaches_the_vehicle_as_bytes() {
    // A silent vehicle: this message has no acknowledgement, so nothing comes back and there is
    // nothing to wait for. What the vehicle received is the decision itself, and what is asserted
    // here are the hand values — the right angle about the x axis and the floor of the thrust
    // demand — rather than the code that produced them.
    let (address, received) = Vehicle::start_with(MavResult::MAV_RESULT_ACCEPTED, true).await;
    let session = session_to(address).await;

    let decision = mpcraft_vehicle::formation::PlaneDecision {
        target: mpcraft_vehicle::GeoPoint {
            lat: 0.0,
            lng: 0.0,
            alt: 0.0,
        },
        roll_deg: 90.0,
        pitch_deg: 0.0,
        yaw_error_deg: 0.0,
        thrust: 0.1,
    };

    session
        .send_attitude_target(vehicle(), &decision.attitude_command())
        .await
        .expect("the link should carry the answer");

    assert!(wait_for_messages(&received, 1).await);

    let log = received.lock().unwrap();
    let Some(MavMessage::SET_ATTITUDE_TARGET(target)) = log.first() else {
        panic!("the vehicle did not receive the attitude target");
    };

    let root_half = std::f64::consts::FRAC_1_SQRT_2;

    assert!(
        (target.q[0] as f64 - root_half).abs() < 1e-7,
        "q1 was {}",
        target.q[0]
    );
    assert!(
        (target.q[1] as f64 - root_half).abs() < 1e-7,
        "q2 was {}",
        target.q[1]
    );
    assert!(target.q[2].abs() < 1e-9, "q3 was {}", target.q[2]);
    assert!(target.q[3].abs() < 1e-9, "q4 was {}", target.q[3]);

    // The mask as upstream's arithmetic leaves it, with the two bits the current dialect does
    // not name still set. Read through `from_bits_truncate` they would have been dropped and this
    // byte would have come back as 32.
    assert_eq!(
        target.type_mask.bits(),
        0x38,
        "the mask was {:?}",
        target.type_mask
    );

    assert!(
        (target.thrust - 0.1).abs() < 1e-7,
        "thrust was {}",
        target.thrust
    );
    assert_eq!(target.target_system, 1);
    assert_eq!(target.target_component, 1);
}
