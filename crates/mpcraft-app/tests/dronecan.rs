//! The rule that says when the bus is asked, over a real socket.
//!
//! The link keeps the nodes; this is the policy above it — ask once when the link opens, once more for
//! a node that has reported how it is without saying what it is, and then stop. What is asserted here
//! is that the policy puts the command on the wire exactly those times, and that the count belongs to
//! the link it was made on: a link that was restored after dropping is a new bus, so the rule starts
//! again there.
//!
//! The command is read as bytes at the peer, because the number it carries (5200) is one the interface
//! needs to see rather than a word somebody chose.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mpcraft_app::dronecan::DronecanService;
use mpcraft_link::{Session, SessionOptions, VehicleId};
use mpcraft_mavlink::frame::{encode_v2_bytes, FrameParser, MavHeader};

/// What the scripted vehicle heard, as bytes.
type Received = Arc<Mutex<Vec<Vec<u8>>>>;

/// The payload of a status, in the order the wire writes it.
fn status_payload() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&1_700_000_000_000_000_u64.to_le_bytes());
    payload.extend_from_slice(&54_321_u32.to_le_bytes());
    payload.extend_from_slice(&0x1234_u16.to_le_bytes());
    payload.extend_from_slice([1, 0, 5].as_slice()); // health WARNING, mode OPERATIONAL, sub-mode
    payload
}

/// A vehicle that records the commands it hears and answers with two nodes — one of them says what it
/// is, the other does not.
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
                println!(
                    "peer heard message {} payload {}",
                    frame.message_id,
                    frame
                        .payload
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                );
                // The station announces itself with a heartbeat when it connects; that is how the
                // vehicle learns where to send, and it is not what the test is about.
                if frame.message_id == 0 {
                    announce(&socket, peer).await;
                    continue;
                }

                if frame.message_id != 76 {
                    continue;
                }

                if let Ok(mut log) = log.lock() {
                    log.push(frame.payload.to_vec());
                }

                // One information message per node that is online — so the second node is answered
                // with nothing, which is the case the dictionary says has to be asked for again.
                for component in [40_u8, 41] {
                    send(&socket, component, peer, &status_payload(), 310).await;
                }

                send(&socket, 40, peer, &info_payload(), 311).await;
            }
        }
    });

    (address, received)
}

fn info_payload() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&1_700_000_000_000_000_u64.to_le_bytes());
    payload.extend_from_slice(&54_321_u32.to_le_bytes());
    payload.extend_from_slice(&0xDDBBEE11_u32.to_le_bytes());

    let mut name = [0_u8; 80];
    for (index, byte) in b"MPCRAFT UCA-1".iter().enumerate() {
        name[index] = *byte;
    }

    payload.extend_from_slice(&name);
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
    // The vehicle introduces itself as 2:1, which is the pair the station then speaks to.
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

    let _ = socket.send_to(&bytes, peer).await;
}

async fn session_to(address: SocketAddr) -> Session {
    let spec = mpcraft_transport::ConnectionSpec::parse(&format!("udp:{address}")).unwrap();
    Session::connect(&spec, SessionOptions::default())
        .await
        .expect("the link opens")
}

async fn wait_for_selection(session: &Session) {
    let deadline = Instant::now() + Duration::from_secs(3);

    while Instant::now() < deadline {
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

/// Waits for something the peer can only answer to.
///
/// A write is queued and goes out on the next tick of the reader, so a check made straight after the
/// call is a check made before the wire. The same is true of what the bus answers with.
async fn wait_for(what: &dyn Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);

    while Instant::now() < deadline && !what() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn the_ask_goes_out_once_at_open_and_once_more_for_a_status_with_no_matching_information() {
    let (address, received) = vehicle().await;
    let session = Arc::new(session_to(address).await);

    wait_for_selection(&session).await;

    let service = DronecanService::new();

    // The first reading is the one that asks.
    let report = service
        .scan(&session, VehicleId::new(2, 1))
        .await
        .expect("the ask goes out");
    assert!(report.sent_now, "the first reading asks");
    assert_eq!(report.asked, 1);

    wait_for(&|| !received.lock().unwrap().is_empty()).await;

    let heard = received.lock().unwrap().first().cloned();
    assert!(heard.is_some(), "the vehicle did not hear the ask");

    let command = heard.expect("the ask was recorded");
    assert_eq!(&command[28..30], &5200_u16.to_le_bytes());
    assert_eq!(command[30], 2, "the ask went to the system the link chose");
    assert_eq!(command[31], 1, "and to the component it chose");

    // Two nodes report themselves. One says what it is; the other stays a node whose information is
    // missing, which is what the second reading is for.
    wait_for(&|| {
        let nodes = session.state().dronecan;
        nodes.len() >= 2 && nodes.iter().any(|node| node.info_missing)
    })
    .await;

    let report = service
        .scan(&session, VehicleId::new(2, 1))
        .await
        .expect("the bus is read");

    assert_eq!(report.nodes.len(), 2, "the bus reported two nodes");
    assert!(
        report.sent_now,
        "a status with no matching information is asked for again"
    );
    assert_eq!(report.asked, 2);

    let said = report
        .nodes
        .iter()
        .find(|node| node.vehicle == VehicleId::new(2, 40))
        .expect("the node that answered with what it is");
    assert_eq!(said.name.as_deref(), Some("MPCRAFT UCA-1"));
    assert_eq!(said.health_name.as_deref(), Some("WARNING"));
    assert_eq!(said.mode_name.as_deref(), Some("OPERATIONAL"));
    assert!(!said.info_missing);

    let silent = report
        .nodes
        .iter()
        .find(|node| node.vehicle == VehicleId::new(2, 41))
        .expect("the node that said how it is but not what it is");
    assert!(silent.info_missing);

    // And the rule stops: a third reading puts nothing on the wire, because a bus that will not
    // answer is not answered by asking again.
    wait_for(&|| received.lock().unwrap().len() == 2).await;

    let before = received.lock().unwrap().len();
    let report = service
        .scan(&session, VehicleId::new(2, 1))
        .await
        .expect("the bus is read");

    assert!(!report.sent_now, "the rule does not ask a third time");
    assert_eq!(report.asked, 2);

    // Give the reader a tick to put anything on the wire, then check that nothing appeared.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        received.lock().unwrap().len(),
        before,
        "nothing went out on the third reading"
    );
}

#[tokio::test]
async fn the_count_belongs_to_the_link_it_was_made_on() {
    let first = vehicle().await;
    let second = vehicle().await;

    let service = DronecanService::new();

    let session = Arc::new(session_to(first.0).await);
    wait_for_selection(&session).await;

    let report = service
        .scan(&session, VehicleId::new(2, 1))
        .await
        .expect("the ask goes out");
    assert_eq!(report.asked, 1);
    wait_for(&|| !first.1.lock().unwrap().is_empty()).await;
    assert!(
        !first.1.lock().unwrap().is_empty(),
        "the first bus heard the ask"
    );

    // A different link is a different bus: the rule starts again there rather than carrying the count
    // over, which is what a link that was restored after dropping means. Had the count been carried
    // over, the second bus would have heard nothing.
    let other = Arc::new(session_to(second.0).await);
    wait_for_selection(&other).await;

    let report = service
        .scan(&other, VehicleId::new(2, 1))
        .await
        .expect("the ask goes out");
    assert_eq!(report.asked, 1, "the new link starts with one ask");
    assert!(report.sent_now);

    wait_for(&|| !second.1.lock().unwrap().is_empty()).await;
    assert!(
        !second.1.lock().unwrap().is_empty(),
        "the second bus heard its own ask, which is what a reset count means"
    );
}
