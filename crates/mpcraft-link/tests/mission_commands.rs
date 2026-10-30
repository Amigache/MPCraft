//! The mission commands over a real socket.
//!
//! The suite runs a **simulated vehicle on a UDP socket** and checks the bytes that go
//! out: a fence plan must be asked for as a fence plan, and the messages that change the
//! plan must carry the values they are supposed to carry.

// The float form of the mission messages and `MISSION_SET_CURRENT` are deprecated in
// favour of their modern equivalents, but a vehicle that does not advertise
// `MISSION_INT` still expects them.
#![allow(deprecated)]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpcraft_link::{Session, SessionOptions, VehicleId};
use mpcraft_mavlink::dialect::messages::{
    AUTOPILOT_VERSION_DATA, COMMAND_ACK_DATA, MISSION_ACK_DATA, MISSION_COUNT_DATA,
    MISSION_REQUEST_DATA,
};
use mpcraft_mavlink::frame::{encode_v2, FrameParser, MavHeader};
use mpcraft_mavlink::{MavCmd, MavMessage, MavMissionResult, MavMissionType, MavResult, Message};
use mpcraft_mission::{MissionItem, MissionType, Plan};

/// The frames the vehicle received, so the test can assert on the bytes.
type Received = Arc<Mutex<Vec<MavMessage>>>;

/// The message id of `MISSION_SET_CURRENT`.
const MISSION_SET_CURRENT_ID: u32 = 41;
/// The message id of `MISSION_CURRENT`.
const MISSION_CURRENT_ID: u32 = 42;
/// The message id of `MISSION_CLEAR_ALL`.
const MISSION_CLEAR_ALL_ID: u32 = 45;

/// A simulated vehicle that holds a plan.
struct Vehicle {
    socket: tokio::net::UdpSocket,
    sequence: u8,
    /// The plan the vehicle holds.
    plan: Vec<MissionItem>,
    /// How many items the ground station announced.
    expected: u16,
    /// The frames the vehicle received.
    received: Received,
}

impl Vehicle {
    /// Starts a vehicle holding the given plan.
    async fn start(plan: Vec<MissionItem>) -> (SocketAddr, Received) {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();

        let received: Received = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&received);

        let mut vehicle = Self {
            socket,
            sequence: 0,
            plan,
            expected: 0,
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

    async fn answer(&mut self, request: &MavMessage, peer: SocketAddr) {
        match request {
            // The vehicle reports how many items it has.
            MavMessage::MISSION_REQUEST_LIST(request) => {
                self.send(
                    peer,
                    MavMessage::MISSION_COUNT(MISSION_COUNT_DATA {
                        count: self.plan.len() as u16,
                        target_system: 0,
                        target_component: 0,
                        mission_type: request.mission_type,
                        ..Default::default()
                    }),
                )
                .await;
            }

            // The vehicle is asked for one item.
            MavMessage::MISSION_REQUEST(request) => {
                self.send_item(peer, request.seq, request.mission_type)
                    .await;
            }

            // The ground station announces an upload. The vehicle asks for the first item.
            MavMessage::MISSION_COUNT(count) => {
                self.expected = count.count;
                self.send(
                    peer,
                    MavMessage::MISSION_REQUEST(MISSION_REQUEST_DATA {
                        seq: 0,
                        target_system: 0,
                        target_component: 0,
                        ..Default::default()
                    }),
                )
                .await;
            }

            // The vehicle is told which item is the current one.
            MavMessage::MISSION_SET_CURRENT(request) => {
                self.send(
                    peer,
                    MavMessage::MISSION_CURRENT(
                        mpcraft_mavlink::dialect::messages::MISSION_CURRENT_DATA {
                            seq: request.seq,
                            total: self.plan.len() as u16,
                            ..Default::default()
                        },
                    ),
                )
                .await;
            }

            // The vehicle is told to forget its plan.
            MavMessage::MISSION_CLEAR_ALL(request) => {
                self.send(
                    peer,
                    MavMessage::MISSION_ACK(MISSION_ACK_DATA {
                        mavtype: MavMissionResult::MAV_MISSION_ACCEPTED,
                        mission_type: request.mission_type,
                        ..Default::default()
                    }),
                )
                .await;
            }

            // The vehicle acknowledges the commands it is given, and answers the one that asks what
            // it is capable of: a plan is read in the encoding the report names, so a station asks
            // for it before the first item.
            MavMessage::COMMAND_LONG(command) => {
                self.send(
                    peer,
                    MavMessage::COMMAND_ACK(COMMAND_ACK_DATA {
                        command: command.command,
                        result: MavResult::MAV_RESULT_ACCEPTED,
                        ..Default::default()
                    }),
                )
                .await;

                if command.command == MavCmd::MAV_CMD_REQUEST_MESSAGE
                    && command.param1 == mpcraft_link::commands::AUTOPILOT_VERSION_ID as f32
                {
                    self.send(
                        peer,
                        MavMessage::AUTOPILOT_VERSION(AUTOPILOT_VERSION_DATA::default()),
                    )
                    .await;
                }
            }

            _ => {}
        }
    }

    /// Sends one item of the plan.
    async fn send_item(&mut self, peer: SocketAddr, seq: u16, mission_type: MavMissionType) {
        let Some(item) = self.plan.get(seq as usize).cloned() else {
            return;
        };

        let message = MavMessage::MISSION_ITEM(
            item.to_float_message(seq, MissionType::from_u8(mission_type as u8)),
        );

        self.send(peer, message).await;
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

/// A plan with one waypoint.
fn sample_plan() -> Vec<MissionItem> {
    vec![MissionItem {
        command: MavCmd::MAV_CMD_NAV_WAYPOINT,
        params: [0.0, 0.0, 0.0, 0.0],
        x: -35.2809,
        y: 149.1300,
        z: 100.0,
        ..MissionItem::default()
    }]
}

#[tokio::test]
async fn the_mission_type_travels_with_the_request_for_the_list() {
    let (address, received) = Vehicle::start(sample_plan()).await;
    let session = session_to(address).await;

    let _ = session
        .download_mission(vehicle(), MissionType::Fence, Duration::from_secs(5))
        .await;

    assert!(wait_for_messages(&received, 1).await);

    // The list is not the first thing the vehicle hears: the station asks what it is capable of
    // first, because that answer decides the encoding of the items. What this test is about is
    // the plan type on the request, wherever in the exchange it sits.
    let log = received.lock().unwrap();
    let Some(MavMessage::MISSION_REQUEST_LIST(request)) = log
        .iter()
        .find(|message| matches!(message, MavMessage::MISSION_REQUEST_LIST(_)))
    else {
        panic!("the vehicle did not receive a request for the list");
    };

    assert_eq!(
        request.mission_type,
        MavMissionType::MAV_MISSION_TYPE_FENCE,
        "the fence was not asked for as a fence"
    );
}

#[tokio::test]
async fn the_mission_type_travels_with_the_request_for_an_item() {
    let (address, received) = Vehicle::start(sample_plan()).await;
    let session = session_to(address).await;

    let _ = session
        .download_mission(vehicle(), MissionType::Rally, Duration::from_secs(5))
        .await;

    // The request for the list, and the request for the item.
    assert!(wait_for_messages(&received, 2).await);

    let log = received.lock().unwrap();
    let Some(MavMessage::MISSION_REQUEST(request)) = log
        .iter()
        .find(|message| matches!(message, MavMessage::MISSION_REQUEST(_)))
    else {
        panic!("the vehicle did not receive a request for an item");
    };

    assert_eq!(
        request.mission_type,
        MavMissionType::MAV_MISSION_TYPE_RALLY,
        "the rally point was not asked for as a rally point"
    );
}

#[tokio::test]
async fn a_fence_plan_downloads_as_a_fence_plan() {
    let (address, _) = Vehicle::start(sample_plan()).await;
    let session = session_to(address).await;

    let plan = session
        .download_mission(vehicle(), MissionType::Fence, Duration::from_secs(5))
        .await
        .expect("the plan should download");

    assert_eq!(plan.mission_type, MissionType::Fence);
    assert_eq!(plan.len(), 1);
    assert_eq!(plan.items[0].command, MavCmd::MAV_CMD_NAV_WAYPOINT);
}

#[tokio::test]
async fn the_mission_type_travels_with_the_announcement_of_an_upload() {
    let (address, received) = Vehicle::start(Vec::new()).await;
    let session = session_to(address).await;

    let mut plan = Plan::new(MissionType::Fence);
    plan.push(sample_plan().remove(0));

    // The vehicle never asks for the item, so the upload times out — but the
    // announcement has already gone out, and that is what this test is about.
    let _ = session
        .upload_mission(vehicle(), &plan, Duration::from_millis(400))
        .await;

    assert!(wait_for_messages(&received, 1).await);

    // As above: the ask about the capabilities comes before the plan, and what is asserted here is
    // the type the announcement carries.
    let log = received.lock().unwrap();
    let Some(MavMessage::MISSION_COUNT(count)) = log
        .iter()
        .find(|message| matches!(message, MavMessage::MISSION_COUNT(_)))
    else {
        panic!("the vehicle did not receive the announcement");
    };

    assert_eq!(count.count, 1);
    assert_eq!(
        count.mission_type,
        MavMissionType::MAV_MISSION_TYPE_FENCE,
        "the fence was not announced as a fence"
    );
}

#[tokio::test]
async fn the_mission_type_travels_with_every_item_of_an_upload() {
    let (address, received) = Vehicle::start(Vec::new()).await;
    let session = session_to(address).await;

    let mut plan = Plan::new(MissionType::Rally);
    plan.push(sample_plan().remove(0));

    // The vehicle asks for the item, the ground station answers it, and the item carries
    // the type of the plan it belongs to.
    let _ = session
        .upload_mission(vehicle(), &plan, Duration::from_millis(600))
        .await;

    assert!(wait_for_messages(&received, 2).await);

    let log = received.lock().unwrap();
    let Some(MavMessage::MISSION_ITEM(item)) = log
        .iter()
        .find(|message| matches!(message, MavMessage::MISSION_ITEM(_)))
    else {
        panic!("the vehicle did not receive the item");
    };

    assert_eq!(
        item.mission_type,
        MavMissionType::MAV_MISSION_TYPE_RALLY,
        "the rally point was not sent as a rally point"
    );
}

#[tokio::test]
async fn setting_the_current_item_sends_the_sequence_number() {
    let (address, received) = Vehicle::start(sample_plan()).await;
    let session = session_to(address).await;

    session
        .set_current_item(vehicle(), 3, Duration::from_secs(5))
        .await
        .expect("the vehicle should confirm the change");

    assert!(wait_for_messages(&received, 1).await);

    let log = received.lock().unwrap();
    let Some(MavMessage::MISSION_SET_CURRENT(request)) = log.first() else {
        panic!("the vehicle did not receive the request");
    };

    assert_eq!(request.seq, 3, "the sequence number was not carried");
    assert_eq!(request.target_system, 1);
    assert_eq!(request.target_component, 1);
}

#[tokio::test]
async fn clearing_the_plan_sends_the_mission_type() {
    let (address, received) = Vehicle::start(sample_plan()).await;
    let session = session_to(address).await;

    let result = session
        .clear_mission(vehicle(), MissionType::Fence, Duration::from_secs(5))
        .await
        .expect("the vehicle should accept the request");

    assert_eq!(result, MavMissionResult::MAV_MISSION_ACCEPTED);

    assert!(wait_for_messages(&received, 1).await);

    let log = received.lock().unwrap();
    let Some(MavMessage::MISSION_CLEAR_ALL(request)) = log.first() else {
        panic!("the vehicle did not receive the request");
    };

    assert_eq!(
        request.mission_type,
        MavMissionType::MAV_MISSION_TYPE_FENCE,
        "the fence was not cleared as a fence"
    );
    assert_eq!(request.target_system, 1);
}

#[tokio::test]
async fn jumping_to_an_item_sends_the_sequence_number() {
    let (address, received) = Vehicle::start(sample_plan()).await;
    let session = session_to(address).await;

    session
        .jump_to_item(vehicle(), 7, Duration::from_secs(5))
        .await
        .expect("the vehicle should accept the command");

    assert!(wait_for_messages(&received, 1).await);

    let log = received.lock().unwrap();
    let Some(MavMessage::COMMAND_LONG(command)) = log.first() else {
        panic!("the vehicle did not receive the command");
    };

    assert_eq!(command.command, MavCmd::MAV_CMD_DO_JUMP);
    assert_eq!(command.param1, 7.0, "the sequence number was not carried");
}

#[tokio::test]
async fn the_progress_of_the_plan_is_kept_by_the_session() {
    let (address, _) = Vehicle::start(sample_plan()).await;
    let session = session_to(address).await;

    session
        .set_current_item(vehicle(), 0, Duration::from_secs(5))
        .await
        .expect("the vehicle should confirm the change");

    // The session aggregates the `MISSION_CURRENT` the vehicle streams.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline && session.mission_current().is_none() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let progress = session.mission_current().expect("the progress of the plan");

    assert_eq!(progress.seq, 0);
    assert_eq!(progress.total, 1);
}

/// The message ids the mission commands use.
#[test]
fn the_message_ids_are_the_ones_the_protocol_says() {
    assert_eq!(mpcraft_link::mission::MISSION_SET_CURRENT_ID, 41);
    assert_eq!(mpcraft_link::mission::MISSION_CURRENT_ID, 42);
    assert_eq!(mpcraft_link::mission::MISSION_CLEAR_ALL_ID, 45);
    assert_eq!(MISSION_SET_CURRENT_ID, 41);
    assert_eq!(MISSION_CURRENT_ID, 42);
    assert_eq!(MISSION_CLEAR_ALL_ID, 45);
}
