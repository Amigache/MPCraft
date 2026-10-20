//! The mission protocol over a real socket.
//!
//! The suite runs a **simulated vehicle on a UDP socket** and flies a plan through it. The
//! exit criterion is the one the protocol has to meet: a plan downloaded from the vehicle and
//! uploaded back must be byte-identical on the wire.

// The float forms of the mission messages are deprecated in favour of the integer
// ones, but a vehicle that does not advertise MISSION_INT still expects them.
#![allow(deprecated)]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpcraft_link::{Session, SessionOptions, VehicleId};
#[allow(deprecated)]
use mpcraft_mavlink::dialect::messages::{
    MISSION_ACK_DATA, MISSION_COUNT_DATA, MISSION_REQUEST_DATA, MISSION_REQUEST_INT_DATA,
};
use mpcraft_mavlink::frame::{encode_v2, FrameParser, MavHeader};
use mpcraft_mavlink::{MavCmd, MavFrame, MavMessage, MavMissionResult, Message};
use mpcraft_mission::{MissionItem, MissionType};

/// The plan the vehicle was sent.
type Uploaded = Arc<Mutex<Vec<MissionItem>>>;

/// The message id of `MISSION_ITEM`.
#[allow(dead_code)]
const MISSION_ITEM_ID: u32 = 39;
/// The message id of `MISSION_REQUEST`.
#[allow(dead_code)]
const MISSION_REQUEST_ID: u32 = 40;
/// The message id of `MISSION_REQUEST_LIST`.
#[allow(dead_code)]
const MISSION_REQUEST_LIST_ID: u32 = 43;
/// The message id of `MISSION_COUNT`.
#[allow(dead_code)]
const MISSION_COUNT_ID: u32 = 44;
/// The message id of `MISSION_ACK`.
#[allow(dead_code)]
const MISSION_ACK_ID: u32 = 47;
/// The message id of `MISSION_REQUEST_INT`.
#[allow(dead_code)]
const MISSION_REQUEST_INT_ID: u32 = 51;
/// The message id of `MISSION_ITEM_INT`.
#[allow(dead_code)]
const MISSION_ITEM_INT_ID: u32 = 73;

/// A simulated vehicle that answers mission requests over UDP.
struct Vehicle {
    socket: tokio::net::UdpSocket,
    /// The plan the vehicle holds.
    plan: Vec<MissionItem>,
    sequence: u8,
    /// Whether the vehicle wants the integer form of the items.
    use_int: bool,
    /// The plans the vehicle was sent, for the test to assert on.
    uploaded: Uploaded,
    /// How many items the ground station announced.
    expected: u16,
}

impl Vehicle {
    async fn start(plan: Vec<MissionItem>, use_int: bool) -> (SocketAddr, Uploaded) {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();

        let uploaded: Uploaded = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&uploaded);

        let mut vehicle = Self {
            socket,
            plan,
            sequence: 0,
            use_int,
            uploaded: log,
            expected: 0,
        };

        tokio::spawn(async move {
            vehicle.run().await;
        });

        (address, uploaded)
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

                self.answer(&decoded, peer).await;
            }
        }
    }

    async fn answer(&mut self, request: &MavMessage, peer: SocketAddr) {
        match request {
            // The vehicle reports how many items it has.
            MavMessage::MISSION_REQUEST_LIST(_) => {
                let count = self.plan.len() as u16;
                self.send(
                    peer,
                    MavMessage::MISSION_COUNT(MISSION_COUNT_DATA {
                        count,
                        target_system: 0,
                        target_component: 0,
                        ..Default::default()
                    }),
                )
                .await;
            }

            // The vehicle asks for one item.
            MavMessage::MISSION_REQUEST(request) => {
                self.send_item(peer, request.seq).await;
            }
            MavMessage::MISSION_REQUEST_INT(request) => {
                self.send_item(peer, request.seq).await;
            }

            // The ground station announces how many items will follow.
            MavMessage::MISSION_COUNT(count) => {
                if let Ok(mut uploaded) = self.uploaded.lock() {
                    uploaded.clear();
                }
                self.expected = count.count;
                // The vehicle now asks for the items, one at a time.
                self.send(
                    peer,
                    if self.use_int {
                        MavMessage::MISSION_REQUEST_INT(MISSION_REQUEST_INT_DATA {
                            seq: 0,
                            target_system: 0,
                            target_component: 0,
                            ..Default::default()
                        })
                    } else {
                        MavMessage::MISSION_REQUEST(MISSION_REQUEST_DATA {
                            seq: 0,
                            target_system: 0,
                            target_component: 0,
                            ..Default::default()
                        })
                    },
                )
                .await;
            }

            MavMessage::MISSION_ITEM(item) => {
                if let Ok(mut uploaded) = self.uploaded.lock() {
                    uploaded.push(MissionItem::from_float_message(item));
                }
                self.continue_upload(peer).await;
            }
            MavMessage::MISSION_ITEM_INT(item) => {
                if let Ok(mut uploaded) = self.uploaded.lock() {
                    uploaded.push(MissionItem::from_int_message(item));
                }
                self.continue_upload(peer).await;
            }

            _ => {}
        }
    }

    /// Asks for the next item, or acknowledges when the plan is complete.
    async fn continue_upload(&mut self, peer: SocketAddr) {
        let next = self
            .uploaded
            .lock()
            .map(|uploaded| uploaded.len() as u16)
            .unwrap_or(0);

        if next >= self.expected {
            self.send(
                peer,
                MavMessage::MISSION_ACK(MISSION_ACK_DATA {
                    target_system: 0,
                    target_component: 0,
                    mavtype: MavMissionResult::MAV_MISSION_ACCEPTED,
                    ..Default::default()
                }),
            )
            .await;
        } else {
            self.send(
                peer,
                if self.use_int {
                    MavMessage::MISSION_REQUEST_INT(MISSION_REQUEST_INT_DATA {
                        seq: next,
                        target_system: 0,
                        target_component: 0,
                        ..Default::default()
                    })
                } else {
                    MavMessage::MISSION_REQUEST(MISSION_REQUEST_DATA {
                        seq: next,
                        target_system: 0,
                        target_component: 0,
                        ..Default::default()
                    })
                },
            )
            .await;
        }
    }

    /// Sends one item of the plan.
    async fn send_item(&mut self, peer: SocketAddr, seq: u16) {
        let Some(item) = self.plan.get(seq as usize).cloned() else {
            return;
        };

        let message = if self.use_int {
            MavMessage::MISSION_ITEM_INT(item.to_int_message(seq, MissionType::Mission))
        } else {
            MavMessage::MISSION_ITEM(item.to_float_message(seq, MissionType::Mission))
        };

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

/// A plan with a few items of different kinds.
fn sample_plan() -> Vec<MissionItem> {
    vec![
        MissionItem {
            command: MavCmd::MAV_CMD_NAV_TAKEOFF,
            params: [15.0, 0.0, 0.0, 0.0],
            x: -35.2809,
            y: 149.1300,
            z: 584.0,
            ..MissionItem::default()
        },
        MissionItem {
            command: MavCmd::MAV_CMD_NAV_WAYPOINT,
            params: [0.0, 0.0, 0.0, 0.0],
            x: -35.2815,
            y: 149.1320,
            z: 200.0,
            ..MissionItem::default()
        },
        MissionItem {
            // A command without a position: its X and Y must not be scaled.
            command: MavCmd::MAV_CMD_CONDITION_DELAY,
            params: [5.0, 0.0, 0.0, 0.0],
            x: 0.0,
            y: 0.0,
            z: 0.0,
            ..MissionItem::default()
        },
        MissionItem {
            command: MavCmd::MAV_CMD_NAV_LAND,
            params: [0.0, 0.0, 0.0, 0.0],
            x: -35.2820,
            y: 149.1340,
            z: 0.0,
            ..MissionItem::default()
        },
    ]
}

/// The wire bytes of a plan, as the vehicle would send them.
fn plan_bytes(items: &[MissionItem], use_int: bool) -> Vec<Vec<u8>> {
    items
        .iter()
        .enumerate()
        .map(|(seq, item)| {
            let header = MavHeader {
                system_id: 1,
                component_id: 1,
                sequence: seq as u8,
            };
            if use_int {
                encode_v2(
                    &MavMessage::MISSION_ITEM_INT(
                        item.to_int_message(seq as u16, MissionType::Mission),
                    ),
                    header,
                )
            } else {
                #[allow(deprecated)]
                encode_v2(
                    &MavMessage::MISSION_ITEM(
                        item.to_float_message(seq as u16, MissionType::Mission),
                    ),
                    header,
                )
            }
        })
        .collect()
}

#[tokio::test]
async fn a_plan_is_downloaded_from_the_vehicle() {
    let items = sample_plan();
    let (address, _) = Vehicle::start(items.clone(), false).await;

    let session = session_to(address).await;

    let plan = session
        .download_mission(
            VehicleId::new(1, 1),
            MissionType::Mission,
            Duration::from_secs(10),
        )
        .await
        .expect("the plan should download");

    assert_eq!(plan.len(), items.len());
    assert_eq!(plan.mission_type, MissionType::Mission);

    for (downloaded, original) in plan.items.iter().zip(items.iter()) {
        assert_eq!(downloaded.command, original.command);
        assert_eq!(downloaded.frame, original.frame);
        assert!((downloaded.x - original.x).abs() < 1e-4);
        assert!((downloaded.y - original.y).abs() < 1e-4);
        assert!((downloaded.z - original.z).abs() < 1e-3);
    }
}

#[tokio::test]
async fn a_plan_downloaded_and_uploaded_is_byte_identical() {
    let items = sample_plan();
    let (address, _) = Vehicle::start(items.clone(), false).await;

    let session = session_to(address).await;

    let plan = session
        .download_mission(
            VehicleId::new(1, 1),
            MissionType::Mission,
            Duration::from_secs(10),
        )
        .await
        .expect("the plan should download");

    let result = session
        .upload_mission(VehicleId::new(1, 1), &plan, Duration::from_secs(10))
        .await
        .expect("the plan should upload");

    assert_eq!(result, MavMissionResult::MAV_MISSION_ACCEPTED);

    // The plan the vehicle now holds must be the same plan, item for item, on the wire.
    let original = plan_bytes(&items, false);
    let uploaded = plan_bytes(&plan.items, false);

    assert_eq!(original.len(), uploaded.len());
    for (before, after) in original.iter().zip(uploaded.iter()) {
        assert_eq!(
            before, after,
            "the uploaded item is not byte-identical to the downloaded one"
        );
    }
}

#[tokio::test]
async fn the_integer_encoding_is_used_when_the_vehicle_asks_for_it() {
    let items = sample_plan();
    let (address, _) = Vehicle::start(items.clone(), true).await;

    let session = session_to(address).await;

    let plan = session
        .download_mission(
            VehicleId::new(1, 1),
            MissionType::Mission,
            Duration::from_secs(10),
        )
        .await
        .expect("the plan should download");

    // The integer encoding scales the coordinates of a location command.
    let waypoint = plan
        .items
        .iter()
        .find(|item| item.command == MavCmd::MAV_CMD_NAV_WAYPOINT)
        .expect("the waypoint");

    assert!((waypoint.x - -35.2815).abs() < 1e-6);
    assert!((waypoint.y - 149.1320).abs() < 1e-6);

    // But not of a command that has no position.
    let delay = plan
        .items
        .iter()
        .find(|item| item.command == MavCmd::MAV_CMD_CONDITION_DELAY)
        .expect("the delay");

    assert_eq!(delay.x, 0.0);
    assert_eq!(delay.y, 0.0);
}

#[tokio::test]
async fn the_frame_of_an_item_survives_the_round_trip() {
    let mut items = sample_plan();
    // A terrain-relative frame must not become a global one.
    items[1].frame = MavFrame::MAV_FRAME_GLOBAL_TERRAIN_ALT as u32;

    let (address, _) = Vehicle::start(items.clone(), false).await;
    let session = session_to(address).await;

    let plan = session
        .download_mission(
            VehicleId::new(1, 1),
            MissionType::Mission,
            Duration::from_secs(10),
        )
        .await
        .expect("the plan should download");

    assert_eq!(
        plan.items[1].frame,
        MavFrame::MAV_FRAME_GLOBAL_TERRAIN_ALT as u32,
        "the frame was changed on the way"
    );
}

#[tokio::test]
async fn the_capability_decides_the_encoding() {
    // A vehicle that advertises MISSION_INT gets the integer form.
    let (address, _) = Vehicle::start(sample_plan(), true).await;
    let session = session_to(address).await;

    // The capability arrives in AUTOPILOT_VERSION, which this vehicle does not send, so the
    // session must fall back to the float form.
    assert!(!session.uses_mission_int());
}
