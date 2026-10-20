//! The concurrency guarantees of the session.
//!
//! Two properties matter, and both are what the actor model exists for:
//!
//! * A transaction that is waiting for an answer does **not** hold up the telemetry. The
//!   vehicle streams its position while the ground station waits for a parameter list
//!   that never comes, and the position keeps arriving.
//! * Two transactions can be in flight at once without treading on each other. A
//!   parameter read and a mission download run side by side, and both answers reach the
//!   transaction that asked for them.

// The vehicle does not advertise `MISSION_INT`, so the float form of the mission
// messages is the one it expects.
#![allow(deprecated)]

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use mpcraft_link::{Session, SessionOptions, VehicleId};
use mpcraft_mavlink::dialect::messages::{
    GLOBAL_POSITION_INT_DATA, HEARTBEAT_DATA, MISSION_COUNT_DATA, MISSION_ITEM_DATA,
    PARAM_VALUE_DATA,
};
use mpcraft_mavlink::frame::{encode_v2, FrameParser, MavHeader};
use mpcraft_mavlink::{MavCmd, MavMessage, MavType, Message};
use mpcraft_mission::MissionType;
use mpcraft_params::ParamType;

/// The message id of `PARAM_REQUEST_READ`.
const PARAM_REQUEST_READ_ID: u32 = 20;
/// The message id of `MISSION_REQUEST_LIST`.
const MISSION_REQUEST_LIST_ID: u32 = 43;

/// A vehicle that streams telemetry and answers the requests it understands.
struct Vehicle {
    socket: tokio::net::UdpSocket,
    sequence: u8,
    /// How many telemetry reports the vehicle has sent.
    reports: Arc<AtomicU32>,
    /// Whether the vehicle answers the parameter requests.
    answers_parameters: bool,
    /// The peer the vehicle talks to.
    peer: Option<SocketAddr>,
}

impl Vehicle {
    async fn start(answers_parameters: bool) -> (SocketAddr, Arc<AtomicU32>) {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();

        let reports = Arc::new(AtomicU32::new(0));
        let counter = Arc::clone(&reports);

        let mut vehicle = Self {
            socket,
            sequence: 0,
            reports: counter,
            answers_parameters,
            peer: None,
        };

        tokio::spawn(async move {
            vehicle.run().await;
        });

        (address, reports)
    }

    async fn run(&mut self) {
        let mut buffer = [0u8; 2048];

        loop {
            let Ok((len, peer)) = self.socket.recv_from(&mut buffer).await else {
                return;
            };
            self.peer = Some(peer);

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

                self.answer(&decoded, peer).await;
            }
        }
    }

    async fn answer(&mut self, request: &MavMessage, peer: SocketAddr) {
        match request {
            MavMessage::HEARTBEAT(_) => {
                self.send_heartbeat(peer).await;
                self.stream_telemetry(peer).await;
            }

            MavMessage::PARAM_REQUEST_LIST(_) => {
                if self.answers_parameters {
                    self.send_parameter(peer, "ARMING_CHECK", 1.0).await;
                }
            }

            MavMessage::PARAM_REQUEST_READ(read) => {
                if self.answers_parameters {
                    let name = read_name(&read.param_id);
                    self.send_parameter(peer, &name, 42.0).await;
                }
            }

            MavMessage::MISSION_REQUEST_LIST(_) => {
                self.send(
                    peer,
                    MavMessage::MISSION_COUNT(MISSION_COUNT_DATA {
                        count: 1,
                        ..Default::default()
                    }),
                )
                .await;
            }

            MavMessage::MISSION_REQUEST(request) => {
                self.send(
                    peer,
                    MavMessage::MISSION_ITEM(MISSION_ITEM_DATA {
                        command: MavCmd::MAV_CMD_NAV_WAYPOINT,
                        seq: request.seq,
                        x: -35.2809,
                        y: 149.13,
                        z: 100.0,
                        ..Default::default()
                    }),
                )
                .await;
            }

            _ => {}
        }
    }

    /// Sends a heartbeat, so the ground station knows the vehicle is there.
    async fn send_heartbeat(&mut self, peer: SocketAddr) {
        self.send(
            peer,
            MavMessage::HEARTBEAT(HEARTBEAT_DATA {
                custom_mode: 5,
                mavtype: MavType::MAV_TYPE_QUADROTOR,
                ..Default::default()
            }),
        )
        .await;
    }

    /// Streams a position report, the way a vehicle in flight does.
    async fn stream_telemetry(&mut self, peer: SocketAddr) {
        for index in 0..40 {
            self.send(
                peer,
                MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
                    time_boot_ms: index * 100,
                    lat: (35.2809 * 1e7) as i32,
                    lon: (149.1300 * 1e7) as i32,
                    alt: 584_000,
                    ..Default::default()
                }),
            )
            .await;

            self.reports.fetch_add(1, Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Sends one parameter value.
    async fn send_parameter(&mut self, peer: SocketAddr, name: &str, value: f32) {
        self.send(
            peer,
            MavMessage::PARAM_VALUE(PARAM_VALUE_DATA {
                param_value: value,
                param_count: 1,
                param_index: 0,
                param_id: param_id(name),
                param_type: ParamType::Real32.as_mavlink(),
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

/// The name carried by a `param_id` field.
fn read_name(id: &mpcraft_mavlink::CharArray<16>) -> String {
    let bytes: Vec<u8> = id.iter().copied().collect();
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// The `param_id` field of a message: the name, padded to sixteen bytes.
fn param_id(name: &str) -> mpcraft_mavlink::CharArray<16> {
    let mut buffer = [0u8; 16];
    let bytes = name.as_bytes();
    let length = bytes.len().min(16);
    buffer[..length].copy_from_slice(&bytes[..length]);
    mpcraft_mavlink::CharArray::new(buffer)
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

#[tokio::test]
async fn a_slow_transaction_does_not_delay_the_telemetry() {
    // The vehicle streams telemetry and ignores the parameter request. The transaction
    // waits for an answer that never comes, and the telemetry must keep arriving while
    // it waits: a transaction that blocks the reader would starve the whole link.
    let (address, reports) = Vehicle::start(false).await;
    let session = session_to(address).await;

    // The telemetry has to be flowing before the transaction starts.
    tokio::time::sleep(Duration::from_millis(150)).await;
    let before = reports.load(Ordering::Relaxed);
    assert!(before > 0, "the vehicle never streamed anything");

    // A transaction that will time out.
    let slow = session.download_parameters(vehicle(), Duration::from_secs(2));

    // While it waits, the telemetry keeps arriving.
    tokio::time::sleep(Duration::from_millis(400)).await;
    let during = reports.load(Ordering::Relaxed);

    assert!(
        during > before,
        "the telemetry stopped while the transaction was waiting: {before} then {during}"
    );

    // The telemetry the session published was updated too.
    let state = session.state();
    assert!(
        state.telemetry.position.is_some(),
        "the position was never decoded"
    );

    let _ = slow.await;
}

#[tokio::test]
async fn a_parameter_read_and_a_mission_download_do_not_interfere() {
    // Two transactions in flight at once: each answer must reach the transaction that
    // asked for it.
    let (address, _) = Vehicle::start(true).await;
    let session = session_to(address).await;

    let parameter = session.read_parameter(vehicle(), "RTL_ALT", Duration::from_secs(5));
    let mission = session.download_mission(vehicle(), MissionType::Mission, Duration::from_secs(5));

    let (parameter, plan) = tokio::join!(parameter, mission);

    let parameter = parameter.expect("the parameter should be read");
    assert_eq!(parameter.name, "RTL_ALT");
    assert!((parameter.value - 42.0).abs() < 1e-6);

    let plan = plan.expect("the plan should download");
    assert_eq!(plan.len(), 1);
    assert_eq!(plan.items[0].command, MavCmd::MAV_CMD_NAV_WAYPOINT);
    assert!((plan.items[0].x - -35.2809).abs() < 1e-4);
}

#[tokio::test]
async fn the_message_ids_the_transactions_use_are_the_ones_the_protocol_says() {
    assert_eq!(PARAM_REQUEST_READ_ID, 20);
    assert_eq!(MISSION_REQUEST_LIST_ID, 43);
}
