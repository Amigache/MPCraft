//! The partial update of a plan, over a real socket.
//!
//! The suite runs a **simulated vehicle on a UDP socket** and writes a range of a plan
//! into it. The exit criterion is the one the protocol has: the vehicle announces the
//! range, asks for the items of it one at a time, and the whole range is written only
//! when the vehicle acknowledges it.

// The float form of the mission messages is deprecated in favour of the integer one,
// but a vehicle that does not advertise MISSION_INT still expects it.
#![allow(deprecated)]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpcraft_link::{PartialUpdatePolicy, Session, SessionOptions, VehicleId};
use mpcraft_mavlink::dialect::messages::{
    AUTOPILOT_VERSION_DATA, HEARTBEAT_DATA, MISSION_ACK_DATA, MISSION_REQUEST_DATA,
    MISSION_REQUEST_INT_DATA, PARAM_VALUE_DATA,
};
use mpcraft_mavlink::frame::{encode_v2, FrameParser, MavHeader};
use mpcraft_mavlink::{
    MavAutopilot, MavCmd, MavMessage, MavMissionResult, MavProtocolCapability, MavType, Message,
};
use mpcraft_mission::{MissionItem, MissionType};
use mpcraft_params::ParamType;

/// What the vehicle was told.
#[derive(Debug, Default, Clone)]
struct VehicleLog {
    /// The range that was announced, as `(start, end)`.
    range: Option<(i16, i16)>,
    /// The items that were written, with the sequence number they carried.
    items: Vec<(u16, MissionItem)>,
    /// Whether the items arrived in the integer form.
    integer_form: bool,
    /// The type of the plan that was announced.
    mission_type: u8,
}

/// A simulated vehicle that writes part of a plan.
struct Vehicle {
    socket: tokio::net::UdpSocket,
    sequence: u8,
    /// Whether the vehicle wants the integer form of the items.
    use_int: bool,
    /// Whether the vehicle refuses the range.
    refuse: bool,
    /// Whether the vehicle acknowledges the range instead of asking for the items.
    push_style: bool,
    /// The sequence number the vehicle asks for instead of the one of the plan.
    ask_for: Option<u16>,
    log: Arc<Mutex<VehicleLog>>,
}

impl Vehicle {
    /// Starts the vehicle and returns its address and its log.
    async fn start(use_int: bool) -> (SocketAddr, Arc<Mutex<VehicleLog>>) {
        Self::start_with(use_int, false, false, None).await
    }

    /// Starts a vehicle that misbehaves in a particular way.
    async fn start_with(
        use_int: bool,
        refuse: bool,
        push_style: bool,
        ask_for: Option<u16>,
    ) -> (SocketAddr, Arc<Mutex<VehicleLog>>) {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();

        let log = Arc::new(Mutex::new(VehicleLog::default()));
        let shared = Arc::clone(&log);

        let mut vehicle = Self {
            socket,
            sequence: 0,
            use_int,
            refuse,
            push_style,
            ask_for,
            log: shared,
        };

        tokio::spawn(async move {
            vehicle.run().await;
        });

        (address, log)
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
            // The ground station says that it is there: the vehicle introduces itself,
            // and the capabilities it advertises are what decide the encoding of the
            // items.
            MavMessage::HEARTBEAT(_) => {
                let capabilities = if self.use_int {
                    MavProtocolCapability::MAV_PROTOCOL_CAPABILITY_MISSION_INT
                } else {
                    MavProtocolCapability::empty()
                };

                self.send(
                    peer,
                    MavMessage::HEARTBEAT(HEARTBEAT_DATA {
                        custom_mode: 5,
                        mavtype: MavType::MAV_TYPE_QUADROTOR,
                        autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
                        ..Default::default()
                    }),
                )
                .await;

                self.send(
                    peer,
                    MavMessage::AUTOPILOT_VERSION(AUTOPILOT_VERSION_DATA {
                        capabilities,
                        ..Default::default()
                    }),
                )
                .await;
            }

            // A parameter is read while the range is written.
            MavMessage::PARAM_REQUEST_READ(read) => {
                let name = read_name(&read.param_id);
                self.send(
                    peer,
                    MavMessage::PARAM_VALUE(PARAM_VALUE_DATA {
                        param_value: 42.0,
                        param_count: 1,
                        param_index: 0,
                        param_id: param_id(&name),
                        param_type: ParamType::Real32.as_mavlink(),
                    }),
                )
                .await;
            }

            // The range that is to be written.
            MavMessage::MISSION_WRITE_PARTIAL_LIST(range) => {
                {
                    let mut log = self.log.lock().expect("the log");
                    log.range = Some((range.start_index, range.end_index));
                    log.mission_type = range.mission_type as u8;
                }

                if self.refuse {
                    self.send(
                        peer,
                        MavMessage::MISSION_ACK(MISSION_ACK_DATA {
                            target_system: 0,
                            target_component: 0,
                            mavtype: MavMissionResult::MAV_MISSION_ERROR,
                            mission_type: range.mission_type,
                            ..Default::default()
                        }),
                    )
                    .await;
                    return;
                }

                if self.push_style {
                    // The vehicle acknowledges the range and waits for the items.
                    self.acknowledge(peer).await;
                    return;
                }

                let seq = self.ask_for.unwrap_or(range.start_index as u16);
                self.ask(peer, seq).await;
            }

            // One item of the range.
            MavMessage::MISSION_ITEM(item) => {
                {
                    let mut log = self.log.lock().expect("the log");
                    log.items
                        .push((item.seq, MissionItem::from_float_message(item)));
                }
                self.after_item(peer, item.seq + 1).await;
            }

            MavMessage::MISSION_ITEM_INT(item) => {
                {
                    let mut log = self.log.lock().expect("the log");
                    log.integer_form = true;
                    log.items
                        .push((item.seq, MissionItem::from_int_message(item)));
                }
                self.after_item(peer, item.seq + 1).await;
            }

            _ => {}
        }
    }

    /// Answers the item that was written: the vehicle asks for the next one, or it
    /// acknowledges the range when the last item of it has been taken.
    async fn after_item(&mut self, peer: SocketAddr, next: u16) {
        let end = self
            .log
            .lock()
            .ok()
            .and_then(|log| log.range)
            .map(|(_, end)| end as u16);

        if Some(next) > end || self.push_style {
            self.acknowledge(peer).await;
        } else {
            self.ask(peer, next).await;
        }
    }

    /// Acknowledges the range.
    async fn acknowledge(&mut self, peer: SocketAddr) {
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
    }

    /// Asks for one item of the plan.
    async fn ask(&mut self, peer: SocketAddr, seq: u16) {
        let message = if self.use_int {
            MavMessage::MISSION_REQUEST_INT(MISSION_REQUEST_INT_DATA {
                seq,
                target_system: 0,
                target_component: 0,
                ..Default::default()
            })
        } else {
            MavMessage::MISSION_REQUEST(MISSION_REQUEST_DATA {
                seq,
                target_system: 0,
                target_component: 0,
                ..Default::default()
            })
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

/// Waits for a condition to hold.
async fn wait_for<F>(condition: F, timeout: Duration) -> bool
where
    F: Fn() -> bool,
{
    let deadline = std::time::Instant::now() + timeout;

    while std::time::Instant::now() < deadline {
        if condition() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    condition()
}

/// A range of items.
fn sample_items() -> Vec<MissionItem> {
    vec![
        MissionItem {
            command: MavCmd::MAV_CMD_NAV_WAYPOINT,
            params: [0.0, 0.0, 0.0, 0.0],
            x: -35.2815,
            y: 149.1320,
            z: 200.0,
            ..MissionItem::default()
        },
        MissionItem {
            command: MavCmd::MAV_CMD_NAV_LOITER_UNLIM,
            params: [30.0, 0.0, 0.0, 0.0],
            x: -35.2820,
            y: 149.1340,
            z: 100.0,
            ..MissionItem::default()
        },
        MissionItem {
            command: MavCmd::MAV_CMD_NAV_LAND,
            params: [0.0, 0.0, 0.0, 0.0],
            x: -35.2830,
            y: 149.1360,
            z: 0.0,
            ..MissionItem::default()
        },
    ]
}

#[tokio::test]
async fn a_range_of_the_plan_is_written() {
    let (address, log) = Vehicle::start(false).await;
    let session = session_to(address).await;

    let items = sample_items();
    let report = session
        .upload_mission_partial(
            VehicleId::new(1, 1),
            MissionType::Mission,
            2,
            &items,
            PartialUpdatePolicy::default(),
        )
        .await
        .expect("the range should be written");

    assert_eq!(report.start, 2);
    assert_eq!(report.accepted, vec![2, 3, 4]);

    let log = log.lock().expect("the log").clone();

    // The range is announced, and its last index is the last item of it: the vehicle
    // expects the items from the start to the end, both of them.
    assert_eq!(log.range, Some((2, 4)));

    // The items are written with the sequence numbers of the range.
    assert_eq!(log.items.len(), 3);
    for (index, (seq, item)) in log.items.iter().enumerate() {
        assert_eq!(*seq, 2 + index as u16);
        assert_eq!(item.command, items[index].command);
        assert!((item.x - items[index].x).abs() < 1e-4);
        assert!((item.y - items[index].y).abs() < 1e-4);
        assert!((item.z - items[index].z).abs() < 1e-3);
    }
}

#[tokio::test]
async fn the_range_is_written_with_the_integer_form_when_the_vehicle_asks_for_it() {
    let (address, log) = Vehicle::start(true).await;
    let session = session_to(address).await;

    // The vehicle advertises the capability, and the session has to know it before the
    // first item goes out.
    let advertised = wait_for(|| session.uses_mission_int(), Duration::from_secs(5)).await;
    assert!(advertised, "the capability never arrived");

    let items = sample_items();
    let report = session
        .upload_mission_partial(
            VehicleId::new(1, 1),
            MissionType::Mission,
            0,
            &items,
            PartialUpdatePolicy::default(),
        )
        .await
        .expect("the range should be written");

    assert_eq!(report.accepted, vec![0, 1, 2]);

    let log = log.lock().expect("the log").clone();
    assert!(log.integer_form, "the items were not in the integer form");

    // The integer form scales the coordinates of a location command.
    let (seq, item) = &log.items[0];
    assert_eq!(*seq, 0);
    assert!((item.x - -35.2815).abs() < 1e-6);
    assert!((item.y - 149.1320).abs() < 1e-6);
}

#[tokio::test]
async fn a_vehicle_that_acknowledges_every_item_is_answered() {
    // A vehicle that acknowledges the range instead of asking for the items: the
    // ground station sends the items of the range by itself.
    let (address, log) = Vehicle::start_with(false, false, true, None).await;
    let session = session_to(address).await;

    let items = sample_items();
    let report = session
        .upload_mission_partial(
            VehicleId::new(1, 1),
            MissionType::Mission,
            1,
            &items,
            PartialUpdatePolicy::default(),
        )
        .await
        .expect("the range should be written");

    assert_eq!(report.accepted, vec![1, 2, 3]);

    let log = log.lock().expect("the log").clone();
    assert_eq!(log.items.len(), 3);
    for (index, (seq, _item)) in log.items.iter().enumerate() {
        assert_eq!(*seq, 1 + index as u16);
    }
}

#[tokio::test]
async fn a_range_that_is_refused_is_an_error() {
    let (address, _log) = Vehicle::start_with(false, true, false, None).await;
    let session = session_to(address).await;

    let error = session
        .upload_mission_partial(
            VehicleId::new(1, 1),
            MissionType::Mission,
            0,
            &sample_items(),
            PartialUpdatePolicy::default(),
        )
        .await
        .expect_err("the range should be refused");

    assert!(
        error.to_string().contains("rejected"),
        "the error was {error}"
    );
}

#[tokio::test]
async fn a_range_with_no_items_is_refused() {
    let (address, _log) = Vehicle::start(false).await;
    let session = session_to(address).await;

    let error = session
        .upload_mission_partial(
            VehicleId::new(1, 1),
            MissionType::Mission,
            0,
            &[],
            PartialUpdatePolicy::default(),
        )
        .await
        .expect_err("the range should be refused");

    assert!(
        error.to_string().contains("no items"),
        "the error was {error}"
    );
}

#[tokio::test]
async fn a_vehicle_that_asks_for_another_item_is_an_error() {
    // The vehicle asks for an item that was never announced.
    let (address, _log) = Vehicle::start_with(false, false, false, Some(9)).await;
    let session = session_to(address).await;

    let error = session
        .upload_mission_partial(
            VehicleId::new(1, 1),
            MissionType::Mission,
            0,
            &sample_items(),
            PartialUpdatePolicy::default(),
        )
        .await
        .expect_err("the exchange should fail");

    assert!(
        error.to_string().contains("asked for item 9"),
        "the error was {error}"
    );
}

#[tokio::test]
async fn a_parameter_read_and_a_partial_update_do_not_interfere() {
    // Two transactions in flight at once: the answer of each of them must reach the
    // transaction that asked for it.
    let (address, _log) = Vehicle::start(false).await;
    let session = session_to(address).await;

    let items = sample_items();

    let parameter = session.read_parameter(VehicleId::new(1, 1), "RTL_ALT", Duration::from_secs(5));
    let update = session.upload_mission_partial(
        VehicleId::new(1, 1),
        MissionType::Mission,
        0,
        &items,
        PartialUpdatePolicy::default(),
    );

    let (parameter, update) = tokio::join!(parameter, update);

    let parameter = parameter.expect("the parameter should be read");
    assert_eq!(parameter.name, "RTL_ALT");
    assert!((parameter.value - 42.0).abs() < 1e-6);

    let update = update.expect("the range should be written");
    assert_eq!(update.accepted, vec![0, 1, 2]);
}

#[tokio::test]
async fn a_vehicle_that_says_nothing_is_a_timeout() {
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();

    let session = session_to(address).await;

    let policy = PartialUpdatePolicy {
        item_timeout: Duration::from_millis(50),
        item_retries: 1,
    };

    let error = session
        .upload_mission_partial(
            VehicleId::new(1, 1),
            MissionType::Mission,
            0,
            &sample_items(),
            policy,
        )
        .await
        .expect_err("the exchange should fail");

    assert!(
        matches!(error, mpcraft_link::LinkError::Timeout(_)),
        "the error was {error}"
    );
}
