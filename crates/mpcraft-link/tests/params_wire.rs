//! The parameter protocol over a real socket.
//!
//! The other parameter tests drive a session over a file, which cannot answer a request: a
//! replay is finite and read-only. This suite runs a **simulated vehicle on a UDP socket** and
//! talks to it over the wire, which exercises the whole path — the request leaves the session,
//! the vehicle answers, and the answer is matched back to the waiting transaction.
//!
//! That path is the one the whole actor model exists for, so it is worth testing for real.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpcraft_link::{Session, SessionOptions, VehicleId};
use mpcraft_mavlink::dialect::messages::{HEARTBEAT_DATA, PARAM_VALUE_DATA};
use mpcraft_mavlink::frame::{encode_v2, FrameParser, MavHeader};
use mpcraft_mavlink::{MavAutopilot, MavMessage, MavType, Message};
use mpcraft_params::ParamType;

/// The message ids the vehicle received.
type Received = Arc<Mutex<Vec<u32>>>;

/// The message id of `PARAM_REQUEST_READ`.
const PARAM_REQUEST_READ_ID: u32 = 20;
/// The message id of `PARAM_REQUEST_LIST`.
const PARAM_REQUEST_LIST_ID: u32 = 21;
/// The message id of `PARAM_SET`.
const PARAM_SET_ID: u32 = 23;

/// A simulated vehicle that answers parameter requests over UDP.
struct Vehicle {
    socket: tokio::net::UdpSocket,
    /// The parameters, as (name, value, reported type).
    parameters: Vec<(String, f32, ParamType)>,
    sequence: u8,
    /// The message ids the vehicle received.
    received: Received,
}

impl Vehicle {
    /// Starts a vehicle and returns its address and the log of what it received.
    async fn start(parameters: Vec<(&str, f32, ParamType)>) -> (SocketAddr, Received) {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();
        let received: Received = Arc::new(Mutex::new(Vec::new()));

        let parameters = parameters
            .into_iter()
            .map(|(name, value, param_type)| (name.to_owned(), value, param_type))
            .collect();

        let mut vehicle = Self {
            socket,
            parameters,
            sequence: 0,
            received: Arc::clone(&received),
        };

        tokio::spawn(async move {
            vehicle.run().await;
        });

        (address, received)
    }

    /// The vehicle's answer loop.
    async fn run(&mut self) {
        let mut buffer = [0u8; 2048];

        loop {
            let Ok((len, peer)) = self.socket.recv_from(&mut buffer).await else {
                return;
            };

            // Find the frame in the datagram and decode it.
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

                if let Ok(mut log) = self.received.lock() {
                    log.push(frame.message_id);
                }

                self.answer(&decoded, peer).await;
            }
        }
    }

    /// Answers a request.
    async fn answer(&mut self, request: &MavMessage, peer: SocketAddr) {
        match request {
            // The ground station announces itself with a heartbeat. The vehicle answers
            // with one of its own: that is how it identifies itself, and the ground
            // station waits for the identification before it reads the parameters.
            MavMessage::HEARTBEAT(_) => {
                self.send_heartbeat(peer).await;
            }

            MavMessage::PARAM_REQUEST_LIST(_) => {
                let total = self.parameters.len() as u16;
                for (index, (name, value, param_type)) in self.parameters.clone().iter().enumerate()
                {
                    self.send_param_value(peer, name, *value, *param_type, total, index as u16)
                        .await;
                }
            }

            MavMessage::PARAM_REQUEST_READ(read) => {
                let name = read_name(&read.param_id);
                if let Some((index, (stored, value, param_type))) = self
                    .parameters
                    .iter()
                    .enumerate()
                    .find(|(_, (stored, _, _))| *stored == name)
                {
                    let total = self.parameters.len() as u16;
                    let stored = stored.clone();
                    self.send_param_value(peer, &stored, *value, *param_type, total, index as u16)
                        .await;
                }
            }

            MavMessage::PARAM_SET(set) => {
                let name = read_name(&set.param_id);
                let found = self
                    .parameters
                    .iter()
                    .position(|(stored, _, _)| *stored == name);

                if let Some(index) = found {
                    let (stored, _, param_type) = self.parameters[index].clone();
                    let total = self.parameters.len() as u16;

                    // The vehicle answers with the value it stored.
                    self.send_param_value(
                        peer,
                        &stored,
                        set.param_value,
                        param_type,
                        total,
                        index as u16,
                    )
                    .await;
                }
            }

            _ => {}
        }
    }

    /// Sends the heartbeat that identifies the vehicle.
    async fn send_heartbeat(&mut self, peer: SocketAddr) {
        let header = MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.wrapping_add(1);

        let message = MavMessage::HEARTBEAT(HEARTBEAT_DATA {
            custom_mode: 5,
            mavtype: MavType::MAV_TYPE_QUADROTOR,
            autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
            ..Default::default()
        });

        let bytes = encode_v2(&message, header);
        let _ = self.socket.send_to(&bytes, peer).await;
    }

    /// Sends one `PARAM_VALUE`.
    async fn send_param_value(
        &mut self,
        peer: SocketAddr,
        name: &str,
        value: f32,
        param_type: ParamType,
        count: u16,
        index: u16,
    ) {
        let header = MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.wrapping_add(1);

        let message = MavMessage::PARAM_VALUE(PARAM_VALUE_DATA {
            param_value: value,
            param_count: count,
            param_index: index,
            param_id: param_id(name),
            param_type: param_type.as_mavlink(),
        });

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

#[tokio::test]
async fn the_parameter_list_is_downloaded_over_the_wire() {
    let (address, received) = Vehicle::start(vec![
        ("ARMING_CHECK", 1.0, ParamType::Real32),
        ("RTL_ALT", 100.0, ParamType::Real32),
        ("WP_RADIUS", 250.0, ParamType::Real32),
    ])
    .await;

    let session = session_to(address).await;

    let store = session
        .download_parameters(VehicleId::new(1, 1), Duration::from_secs(10))
        .await
        .expect("the parameter list should download");

    assert_eq!(store.len(), 3);
    assert_eq!(store.total_reported(), 3);
    assert!(store.is_complete());

    assert_eq!(store.get("ARMING_CHECK").map(|p| p.value), Some(1.0));
    assert_eq!(store.get("RTL_ALT").map(|p| p.value), Some(100.0));
    assert_eq!(store.get("WP_RADIUS").map(|p| p.value), Some(250.0));

    // The request really went out over the wire.
    let log = received.lock().unwrap().clone();
    assert!(
        log.contains(&PARAM_REQUEST_LIST_ID),
        "the vehicle did not receive a PARAM_REQUEST_LIST: {log:?}"
    );
}

#[tokio::test]
async fn a_single_parameter_is_read_over_the_wire() {
    let (address, received) = Vehicle::start(vec![
        ("ARMING_CHECK", 1.0, ParamType::Real32),
        ("RTL_ALT", 100.0, ParamType::Real32),
    ])
    .await;

    let session = session_to(address).await;

    let parameter = session
        .read_parameter(VehicleId::new(1, 1), "RTL_ALT", Duration::from_secs(10))
        .await
        .expect("the parameter should be read");

    assert_eq!(parameter.name, "RTL_ALT");
    assert_eq!(parameter.value, 100.0);

    let log = received.lock().unwrap().clone();
    assert!(
        log.contains(&PARAM_REQUEST_READ_ID),
        "the vehicle did not receive a PARAM_REQUEST_READ: {log:?}"
    );
}

#[tokio::test]
async fn a_parameter_is_written_and_the_value_is_confirmed() {
    let (address, received) = Vehicle::start(vec![
        ("ARMING_CHECK", 1.0, ParamType::Real32),
        ("RTL_ALT", 100.0, ParamType::Real32),
    ])
    .await;

    let session = session_to(address).await;

    // The parameter has to be known before it can be written, because its reported type
    // decides how the value is encoded.
    let _ = session
        .download_parameters(VehicleId::new(1, 1), Duration::from_secs(10))
        .await
        .expect("the parameter list should download");

    let written = session
        .write_parameter(
            VehicleId::new(1, 1),
            "RTL_ALT",
            250.0,
            Duration::from_secs(10),
        )
        .await
        .expect("the parameter should be written");

    assert_eq!(written.name, "RTL_ALT");
    assert_eq!(written.value, 250.0);

    let log = received.lock().unwrap().clone();
    assert!(
        log.contains(&PARAM_SET_ID),
        "the vehicle did not receive a PARAM_SET: {log:?}"
    );
}

#[tokio::test]
async fn a_write_of_an_integer_survives_the_round_trip() {
    // A byte-wise vehicle stores an integer. Writing it and reading it back must give the
    // same number, not a denormal.
    let (address, _) = Vehicle::start(vec![("WP_RADIUS", 1000.0, ParamType::UInt16)]).await;

    let session = session_to(address).await;

    let _ = session
        .download_parameters(VehicleId::new(1, 1), Duration::from_secs(10))
        .await
        .expect("the parameter list should download");

    let written = session
        .write_parameter(
            VehicleId::new(1, 1),
            "WP_RADIUS",
            2000.0,
            Duration::from_secs(10),
        )
        .await
        .expect("the parameter should be written");

    assert_eq!(
        written.value, 2000.0,
        "the integer was corrupted on the way"
    );
}
