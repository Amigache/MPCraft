//! The vehicle the station speaks to, over a real link.
//!
//! The rule that picks a vehicle from the heartbeats is one thing, and the operator's choice is
//! another: the rule runs once, and from then on the choice stands. What this proves is that the
//! choice reaches the state the interface reads — the view, and the mark that says which entry is
//! the one being read — and that a board that talks more afterwards does not take it back, which
//! is the same reason the rule's own choice does not move.
//!
//! It also proves the boundary an operator cannot cross: a pair that has never spoken is not a
//! vehicle. There is no state behind it, so choosing it would be choosing an address.

use std::time::Duration;

use mpcraft_link::{Session, SessionOptions, VehicleId};
use mpcraft_mavlink::dialect::messages::{GLOBAL_POSITION_INT_DATA, HEARTBEAT_DATA};
use mpcraft_mavlink::frame::{encode_v2, MavHeader};
use mpcraft_mavlink::{MavAutopilot, MavMessage, MavType};
use mpcraft_transport::ConnectionSpec;

/// A copter that reports on the autopilot's component.
fn copter() -> MavMessage {
    MavMessage::HEARTBEAT(HEARTBEAT_DATA {
        mavtype: MavType::MAV_TYPE_QUADROTOR,
        autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
        ..Default::default()
    })
}

/// A plane on the same component, so that the two boards can be told apart by what they report.
fn plane() -> MavMessage {
    MavMessage::HEARTBEAT(HEARTBEAT_DATA {
        mavtype: MavType::MAV_TYPE_FIXED_WING,
        autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
        ..Default::default()
    })
}

/// A position, at a place that is different for each board.
fn position(lat: f64, lng: f64) -> MavMessage {
    MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
        lat: (lat * 1e7).round() as i32,
        lon: (lng * 1e7).round() as i32,
        alt: 584_000,
        ..Default::default()
    })
}

/// Writes one frame to the ground station, from the board named by `system_id`.
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
async fn the_choice_stands_and_the_view_moves_with_it() {
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();
    let spec = ConnectionSpec::parse(&format!("udp:{address}")).unwrap();

    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap();

    // The ground station announces itself; what the boards answer goes back to wherever the
    // station spoke from.
    let mut buffer = [0u8; 2048];
    let (_, peer) = socket.recv_from(&mut buffer).await.unwrap();

    let mut sequence = 0u8;

    // Board 1 is the autopilot seen at least twice, so it is the one the rule takes. Board 2
    // qualifies through the other branch of the rule, and the choice has to fall on the board that
    // qualified first.
    for _ in 0..2 {
        send(&socket, peer, 1, &copter(), sequence).await;
        sequence += 1;
    }

    for _ in 0..4 {
        send(&socket, peer, 2, &plane(), sequence).await;
        sequence += 1;
    }

    send(&socket, peer, 1, &position(40.4406, -3.6075), sequence).await;
    sequence += 1;
    send(&socket, peer, 2, &position(-33.8688, 151.2093), sequence).await;
    sequence += 1;

    let deadline = std::time::Instant::now() + Duration::from_secs(5);

    loop {
        let state = session.state();

        if state.selected == Some(VehicleId::new(1, 1)) && state.telemetries.len() == 2 {
            break;
        }

        if std::time::Instant::now() > deadline {
            panic!("board 1 was never chosen by the rule: {state:?}");
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // The operator chooses board 2.
    session.select_vehicle(VehicleId::new(2, 1)).await.unwrap();

    let state = session.state();

    assert_eq!(state.selected, Some(VehicleId::new(2, 1)));

    // The mark is on one entry only: choosing a vehicle takes the mark off the one the rule had
    // made, otherwise the interface would show two vehicles as the one it is reading.
    assert_eq!(
        state
            .vehicles
            .iter()
            .filter(|vehicle| vehicle.primary)
            .count(),
        1
    );

    // The view belongs to the chosen board — and what board 1 had already reported stays in its own
    // slot rather than being thrown away when the choice moves.
    let view = state.telemetry.position.expect("the view holds a position");
    assert!(
        (view.lat - -33.8688).abs() < 1e-4,
        "the view did not move with the choice"
    );

    let first = state
        .telemetries
        .get(&VehicleId::new(1, 1))
        .expect("board 1 keeps its own state");
    let first_position = first.position.expect("board 1 had reported a place");
    assert!((first_position.lat - 40.4406).abs() < 1e-4);

    // A board that talks more afterwards does not take the choice back. Board 1 is heard enough
    // times to satisfy the rule again, and the reader is known to have processed it because its
    // new position arrives; once that has happened, the choice is still board 2's.
    for _ in 0..6 {
        send(&socket, peer, 1, &copter(), sequence).await;
        sequence += 1;
    }

    send(&socket, peer, 1, &position(41.3888, 2.1159), sequence).await;

    let until = std::time::Instant::now() + Duration::from_secs(5);

    loop {
        let state = session.state();

        let moved = state
            .telemetries
            .get(&VehicleId::new(1, 1))
            .and_then(|vehicle| vehicle.position)
            .is_some_and(|point| (point.lat - 41.3888).abs() < 1e-4);

        if moved {
            assert_eq!(
                state.selected,
                Some(VehicleId::new(2, 1)),
                "the choice moved back to the board that talked more"
            );
            break;
        }

        if std::time::Instant::now() > until {
            panic!("board 1 was never read after the choice: {state:?}");
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // A pair that has never spoken is not a vehicle: there is no state behind it. The choice that
    // stands is left where it was.
    let error = session
        .select_vehicle(VehicleId::new(9, 1))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_request");

    assert_eq!(session.state().selected, Some(VehicleId::new(2, 1)));

    session.stop().await;
}
