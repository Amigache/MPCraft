//! Two vehicles on one link.
//!
//! A link can carry more than one board, and the state of each has to be kept apart: the
//! architecture says the aggregator owns the per-vehicle state, and this is the test that
//! proves it over a real socket rather than over a unit test that calls the aggregator
//! directly.
//!
//! The two vehicles report different places, so a mixed state would show one position. What
//! is asserted is that the selection names a primary vehicle by the rule the collector
//! implements, that the view keeps that vehicle's place, and that the other vehicle keeps its
//! own.

use std::time::Duration;

use mpcraft_link::{Session, SessionOptions, VehicleId};
use mpcraft_mavlink::dialect::messages::{GLOBAL_POSITION_INT_DATA, HEARTBEAT_DATA};
use mpcraft_mavlink::frame::{encode_v2, FrameParser, MavHeader};
use mpcraft_mavlink::{MavAutopilot, MavMessage, MavType};
use mpcraft_transport::ConnectionSpec;

/// A heartbeat from an autopilot.
fn heartbeat() -> MavMessage {
    MavMessage::HEARTBEAT(HEARTBEAT_DATA {
        mavtype: MavType::MAV_TYPE_QUADROTOR,
        autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
        ..Default::default()
    })
}

/// A position report, at a place that is different for each vehicle.
fn position(lat: f64, lng: f64) -> MavMessage {
    MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
        lat: (lat * 1e7) as i32,
        lon: (lng * 1e7) as i32,
        alt: 584_000,
        ..Default::default()
    })
}

/// Writes one frame to the ground station, with the system id of the vehicle that sends it.
async fn send(
    socket: &tokio::net::UdpSocket,
    peer: std::net::SocketAddr,
    system_id: u8,
    message: &MavMessage,
    sequence: u8,
) {
    let header = MavHeader {
        system_id,
        component_id: 1,
        sequence,
    };

    socket
        .send_to(&encode_v2(message, header), peer)
        .await
        .unwrap();
}

#[tokio::test]
async fn two_vehicles_keep_their_own_state_and_one_of_them_is_the_primary() {
    // The test plays both boards on one link.
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();
    let spec = ConnectionSpec::parse(&format!("udp:{address}")).unwrap();

    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap();

    // The ground station announced itself; what the boards answer goes back to wherever the
    // station spoke from.
    let mut buffer = [0u8; 2048];
    let (len, peer) = socket.recv_from(&mut buffer).await.unwrap();

    let start = buffer[..len]
        .iter()
        .position(|&b| b == 0xFD || b == 0xFE)
        .expect("the ground station announced itself with a frame");

    let mut parser = FrameParser::new();
    let mut frames = Vec::new();
    parser.feed(&buffer[start..len], &mut frames).unwrap();

    assert_eq!(frames.len(), 1, "the announcement should be one frame");

    let mut sequence = 0u8;

    // Vehicle 1 is the autopilot seen at least twice, which is what the rule prefers. Vehicle
    // 2 reports on enough sightings to satisfy the other branch of the rule, so both qualify,
    // and the choice has to fall on the one that qualified first.
    for _ in 0..2 {
        send(&socket, peer, 1, &heartbeat(), sequence).await;
        sequence += 1;
    }

    for _ in 0..4 {
        send(&socket, peer, 2, &heartbeat(), sequence).await;
        sequence += 1;
    }

    // The places are reported after the selection, so the view can only hold one of them if
    // the state is kept apart.
    send(&socket, peer, 1, &position(35.2809, 149.1300), sequence).await;
    sequence += 1;
    send(&socket, peer, 2, &position(-33.8688, 151.2093), sequence).await;

    let deadline = std::time::Instant::now() + Duration::from_secs(5);

    loop {
        let state = session.state();

        if state.telemetry.position.is_some() && state.telemetries.len() == 2 {
            break;
        }

        if std::time::Instant::now() > deadline {
            panic!("the two vehicles were never read apart: {state:?}");
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let state = session.state();

    // The choice fell on vehicle 1, and it does not move because vehicle 2 talked more.
    assert_eq!(state.selected, Some(VehicleId::new(1, 1)));

    let primary = state
        .vehicles
        .iter()
        .find(|vehicle| vehicle.primary)
        .expect("a vehicle should have been selected");

    assert_eq!(primary.id, VehicleId::new(1, 1));

    // The view is the primary's place, not the one the other board reported.
    let view = state.telemetry.position.expect("the view holds a position");
    assert!(
        (view.lat - 35.2809).abs() < 1e-4,
        "the view was taken for vehicle 2"
    );

    // And vehicle 2 kept its own.
    let other = state
        .telemetries
        .get(&VehicleId::new(2, 1))
        .expect("vehicle 2 has its own state");
    let other_position = other.position.expect("the second vehicle keeps a position");
    assert!((other_position.lat - -33.8688).abs() < 1e-4);

    session.stop().await;
}

#[tokio::test]
async fn the_view_keeps_the_primary_and_a_later_vehicle_does_not_take_it_over() {
    // A single vehicle on the link: what it reports is the view, and a second board that
    // arrives later does not change which vehicle the view belongs to.
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();
    let spec = ConnectionSpec::parse(&format!("udp:{address}")).unwrap();

    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap();

    let mut buffer = [0u8; 2048];
    let (len, peer) = socket.recv_from(&mut buffer).await.unwrap();

    assert!(
        buffer[..len].iter().any(|&b| b == 0xFD || b == 0xFE),
        "the ground station announced itself with a frame"
    );

    let mut sequence = 0u8;

    for _ in 0..2 {
        send(&socket, peer, 1, &heartbeat(), sequence).await;
        sequence += 1;
    }

    send(&socket, peer, 1, &position(35.2809, 149.1300), sequence).await;
    sequence += 1;

    // The second board comes up after the choice was made and reports its own place.
    for _ in 0..4 {
        send(&socket, peer, 7, &heartbeat(), sequence).await;
        sequence += 1;
    }

    send(&socket, peer, 7, &position(-33.8688, 151.2093), sequence).await;

    let deadline = std::time::Instant::now() + Duration::from_secs(5);

    loop {
        let state = session.state();

        if state.telemetries.len() == 2 && state.selected.is_some() {
            break;
        }

        if std::time::Instant::now() > deadline {
            panic!("the second vehicle was never read apart: {state:?}");
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let state = session.state();

    assert_eq!(state.selected, Some(VehicleId::new(1, 1)));

    let view = state.telemetry.position.expect("the view holds a position");
    assert!(
        (view.lat - 35.2809).abs() < 1e-4,
        "the view was taken for vehicle 7"
    );

    session.stop().await;
}
