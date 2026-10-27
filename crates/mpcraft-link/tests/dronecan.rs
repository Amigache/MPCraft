//! The nodes of a DroneCAN bus, over a real socket.
//!
//! Two things are checked here, and they are different things.
//!
//! The first is the wire. The two frames below are the bytes pymavlink's own generator wrote for
//! fixed values, so the reader this project uses is checked against something other than itself:
//! if the field order were wrong, or the seed, the bytes would not match. The seeds the rule
//! computes (28 and 95) are the ones pymavlink publishes, and the order the fields are read in is
//! the order the seed is worked out over, so the agreement on the number is also an agreement on
//! the layout.
//!
//! The second is what the station does with them: it keeps a node under the pair that reported it,
//! asks once for what a node is when its status arrives from a pair it has not seen, and keeps a
//! node that has never said what it is as one that has not — which is the case the dictionary says
//! has to be asked for again. The command goes out with the number the published dictionary gives it
//! (5200), addressed to the pair the link chose.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpcraft_link::{Session, SessionOptions, VehicleId};
use mpcraft_mavlink::frame::{encode_v2_bytes, FrameParser, MavHeader};

/// What the vehicle received, as bytes, so the test can assert on a command the bindings cannot
/// decode.
type Received = Arc<Mutex<Vec<Vec<u8>>>>;

/// The frame pymavlink's generator wrote for a status, for system 2 component 1, sequence 0.
const STATUS_FRAME: [u8; 29] = [
    0xFD, 0x11, 0, 0, 0, 2, 1, 0x36, 1, 0, // the header, with the id at offset seven
    0x00, 0x40, 0x1E, 0x18, 0x24, 0x0A, 0x06, 0x00, // time_usec
    0x31, 0xD4, 0x00, 0x00, // uptime_sec
    0x34, 0x12, // the vendor's number, before the three single bytes
    0x01, 0x00, 0x05, // health, mode, sub_mode
    0xDD, 0xEE, // the checksum with the seed of 28
];

/// The frame pymavlink's generator wrote for the same node's information.
fn info_frame() -> Vec<u8> {
    let mut name = [0_u8; 80];
    for (index, byte) in b"MPCRAFT UCA-1".iter().enumerate() {
        name[index] = *byte;
    }

    let payload = [
        0x00, 0x40, 0x1E, 0x18, 0x24, 0x0A, 0x06, 0x00, // time_usec
        0x31, 0xD4, 0x00, 0x00, // uptime_sec
        0x11, 0xEE, 0xBB, 0xDD, // the commit, which the wire writes before the name
    ]
    .into_iter()
    .chain(name.iter().copied())
    .chain([2, 3].iter().copied())
    .chain(
        [
            0xDE, 0xAD, 0xBE, 0xEF, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
        ]
        .iter()
        .copied(),
    )
    .chain([4, 7].iter().copied())
    .collect::<Vec<u8>>();

    encode_v2_bytes(
        311,
        &payload,
        MavHeader {
            sequence: 0,
            system_id: 2,
            component_id: 1,
        },
    )
}

#[test]
fn the_frame_of_a_node_is_read_with_the_seed_the_rule_gives() {
    let mut parser = FrameParser::new();
    let mut frames = Vec::new();
    parser
        .feed(&STATUS_FRAME, &mut frames)
        .expect("the frame is read");

    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].message_id, mpcraft_link::NODE_STATUS_ID);
    assert_eq!(
        mpcraft_mavlink::dialect::message_name(310),
        Some("UAVCAN_NODE_STATUS")
    );
    assert_eq!(mpcraft_mavlink::dialect::extra_crc(310), Some(28));

    // The payload is what the generator wrote, byte for byte.
    assert_eq!(frames[0].payload, STATUS_FRAME[10..27].to_vec());

    // And this project's own helper puts the same bytes on the wire.
    let rebuilt = encode_v2_bytes(
        310,
        &STATUS_FRAME[10..27],
        MavHeader {
            sequence: 0,
            system_id: 2,
            component_id: 1,
        },
    );
    assert_eq!(rebuilt, STATUS_FRAME.to_vec());
}

#[test]
fn the_information_of_a_node_is_read_in_the_order_the_wire_writes_it() {
    let frame = info_frame();

    let mut parser = FrameParser::new();
    let mut frames = Vec::new();
    parser.feed(&frame, &mut frames).expect("the frame is read");

    assert_eq!(frames[0].message_id, mpcraft_link::NODE_INFO_ID);
    assert_eq!(
        mpcraft_mavlink::dialect::message_name(311),
        Some("UAVCAN_NODE_INFO")
    );
    assert_eq!(mpcraft_mavlink::dialect::extra_crc(311), Some(95));

    let decoded = mpcraft_link::DronecanNodeInfo::decode(&frames[0].payload);
    assert_eq!(decoded.name, "MPCRAFT UCA-1");
    assert_eq!(decoded.sw_vcs_commit, 0xDDBBEE11);
    assert_eq!(decoded.hw_unique_id[0], 0xDE);
}

#[tokio::test]
async fn a_node_that_has_not_said_what_it_is_stays_one_that_has_not() {
    let (address, received) = vehicle().await;
    let session = session_to(address).await;

    // The station speaks to the pair the link chose from what answered — here 2:1, which is what
    // the scripted vehicle announces itself as.
    wait_for_selection(&session).await;
    session
        .send_node_info_request(VehicleId::new(2, 1))
        .await
        .expect("the ask goes out");

    let deadline = std::time::Instant::now() + Duration::from_secs(3);

    while std::time::Instant::now() < deadline && received.lock().unwrap().is_empty() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let sent = received.lock().unwrap().first().cloned();
    assert!(sent.is_some(), "the vehicle did not hear the ask");

    // The command travels as the number the published dictionary gives it, at the place the
    // layout puts it: seven floats, then the command, then the pair it is for.
    let payload = sent.expect("the ask was recorded");
    assert_eq!(&payload[28..30], &5200_u16.to_le_bytes());
    assert_eq!(payload[30], 2, "the ask went to the system the link chose");
    assert_eq!(payload[31], 1, "and to the component it chose");

    // Two nodes report themselves. One of them also says what it is; the other does not, and stays
    // as one whose information is missing rather than becoming a node with an invented name.
    let nodes = wait_for_nodes(&session, 2).await;
    assert_eq!(nodes.len(), 2);

    let reported = nodes
        .iter()
        .find(|node| node.vehicle == VehicleId::new(2, 40))
        .expect("the node that answered with what it is");
    assert_eq!(reported.name.as_deref(), Some("MPCRAFT UCA-1"));
    assert_eq!(reported.health_name.as_deref(), Some("WARNING"));
    assert_eq!(reported.mode_name.as_deref(), Some("OPERATIONAL"));
    assert!(!reported.info_missing);

    let silent = nodes
        .iter()
        .find(|node| node.vehicle == VehicleId::new(2, 41))
        .expect("the node that said how it is but not what it is");
    assert_eq!(silent.uptime_sec, 54_321);
    assert!(
        silent.info_missing,
        "a status with no matching information is still missing it"
    );
}

/// A simulated vehicle that records the raw commands it hears and answers with two nodes.
async fn vehicle() -> (SocketAddr, Received) {
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();

    let received: Received = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&received);

    tokio::spawn(async move {
        let mut buffer = [0_u8; 2048];

        loop {
            let Ok((length, peer)) = socket.recv_from(&mut buffer).await else {
                return;
            };

            let Some(start) = buffer[..length]
                .iter()
                .position(|&byte| byte == 0xFD || byte == 0xFE)
            else {
                continue;
            };

            let mut parser = FrameParser::new();
            let mut frames = Vec::new();
            if parser.feed(&buffer[start..length], &mut frames).is_err() {
                continue;
            }

            for frame in &frames {
                // The station announces itself with a heartbeat when it connects; this is how the
                // vehicle learns where to send, and it is not what the test is about.
                if frame.message_id == 0 {
                    announce(&socket, peer).await;
                    continue;
                }

                if frame.message_id == 76 {
                    if let Ok(mut log) = log.lock() {
                        log.push(frame.payload.to_vec());
                    }

                    // The dictionary says one message per node that is online, so the vehicle
                    // answers with the two its bus holds.
                    send(&socket, 40, peer, &STATUS_FRAME[10..27], 310).await;
                    send(&socket, 41, peer, &STATUS_FRAME[10..27], 310).await;
                    let name = "MPCRAFT UCA-1";
                    let payload = info_payload_of(name);
                    send(&socket, 40, peer, &payload, 311).await;
                }
            }
        }
    });

    (address, received)
}

fn info_payload_of(name: &str) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&1_700_000_000_000_000_u64.to_le_bytes());
    payload.extend_from_slice(&54_321_u32.to_le_bytes());
    payload.extend_from_slice(&0xDDBBEE11_u32.to_le_bytes());

    let mut name_field = [0_u8; 80];
    for (index, byte) in name.as_bytes().iter().enumerate().take(80) {
        name_field[index] = *byte;
    }

    payload.extend_from_slice(&name_field);
    payload.extend_from_slice([2, 3].as_slice());
    payload.extend_from_slice(
        [
            0xDE, 0xAD, 0xBE, 0xEF, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
        ]
        .as_slice(),
    );
    payload.extend_from_slice([4, 7].as_slice());

    payload
}

async fn announce(socket: &tokio::net::UdpSocket, peer: SocketAddr) {
    // The vehicle introduces itself as 2:1, which is the pair the station then speaks to. The
    // payload is written in the order the wire uses: the 32-bit mode first, then the bytes.
    let payload = [5_u8, 0, 0, 0, 2, 3, 0, 4, 3];
    send(socket, 1, peer, &payload, 0).await;
}

async fn send(
    socket: &tokio::net::UdpSocket,
    component_id: u8,
    peer: SocketAddr,
    payload: &[u8],
    message_id: u32,
) {
    let bytes = encode_v2_bytes(
        message_id,
        payload,
        MavHeader {
            sequence: 0,
            system_id: 2,
            component_id,
        },
    );

    println!("sent frame {message_id}: {}", hex_of(&bytes));
    let _ = socket.send_to(&bytes, peer).await;
}

fn hex_of(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Opens a session to the vehicle.
async fn session_to(address: SocketAddr) -> Session {
    let spec = mpcraft_transport::ConnectionSpec::parse(&format!("udp:{address}")).unwrap();
    Session::connect(&spec, SessionOptions::default())
        .await
        .expect("the link opens")
}

async fn wait_for_selection(session: &Session) {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);

    while std::time::Instant::now() < deadline {
        if session.state().selected == Some(VehicleId::new(2, 1)) {
            return;
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    panic!(
        "the link did not choose the pair it heard: {:?}",
        session.state().selected
    );
}

async fn wait_for_nodes(session: &Session, expected: usize) -> Vec<mpcraft_link::DronecanNode> {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);

    while std::time::Instant::now() < deadline {
        let nodes = session.state().dronecan;

        if nodes.len() >= expected {
            return nodes;
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    panic!("the bus did not report {} nodes", expected);
}
