//! The messages the vehicle sends, captured by the session.
//!
//! A vehicle reports what it is doing with `STATUSTEXT`, and the console in the interface
//! shows them. The capture has to survive the two things that go wrong with a text field:
//! the payload is truncated at the first zero, and a vehicle that talks a great deal must
//! not grow the state without limit.

use std::net::SocketAddr;
use std::time::Duration;

use mpcraft_link::{Session, SessionOptions, VehicleId};
use mpcraft_mavlink::dialect::messages::{HEARTBEAT_DATA, STATUSTEXT_DATA};
use mpcraft_mavlink::frame::{encode_v2, MavHeader};
use mpcraft_mavlink::{MavAutopilot, MavMessage, MavSeverity, MavType};

/// A vehicle that says a few things and then stops.
struct Vehicle {
    socket: tokio::net::UdpSocket,
    sequence: u8,
}

impl Vehicle {
    /// Starts a vehicle that announces itself and says what it has to say.
    ///
    /// The vehicle learns where the ground station is from the frame it receives: a
    /// vehicle that only sends would never know where to send, exactly as in the field.
    async fn start(lines: Vec<(&'static str, MavSeverity)>) -> SocketAddr {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let bound = socket.local_addr().unwrap();

        tokio::spawn(async move {
            let mut vehicle = Self {
                socket,
                sequence: 0,
            };

            vehicle.run(lines).await;
        });

        bound
    }

    /// Waits for the ground station, then announces itself and says the lines.
    async fn run(&mut self, lines: Vec<(&'static str, MavSeverity)>) {
        let mut buffer = [0u8; 2048];

        // The ground station announces itself when it opens the link.
        let Ok((_, peer)) = self.socket.recv_from(&mut buffer).await else {
            return;
        };

        let header = MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: 0,
        };

        let heartbeat = MavMessage::HEARTBEAT(HEARTBEAT_DATA {
            custom_mode: 0,
            mavtype: MavType::MAV_TYPE_QUADROTOR,
            autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
            ..Default::default()
        });

        let _ = self
            .socket
            .send_to(&encode_v2(&heartbeat, header), peer)
            .await;

        for (text, severity) in lines {
            self.sequence = self.sequence.wrapping_add(1);

            // The text field is a fixed fifty characters, padded with zeroes.
            let mut data = [0u8; 50];
            let bytes = text.as_bytes();
            let length = bytes.len().min(data.len());
            data[..length].copy_from_slice(&bytes[..length]);

            let message = MavMessage::STATUSTEXT(STATUSTEXT_DATA {
                severity,
                text: mpcraft_mavlink::dialect::types::CharArray::new(data),
                id: 0,
                chunk_seq: 0,
            });

            let header = MavHeader {
                system_id: 1,
                component_id: 1,
                sequence: self.sequence,
            };

            let _ = self
                .socket
                .send_to(&encode_v2(&message, header), peer)
                .await;
        }
    }
}

/// Waits until the predicate holds, or the time runs out.
async fn wait_for<F>(session: &Session, predicate: F) -> bool
where
    F: Fn(&mpcraft_link::SessionState) -> bool,
{
    for _ in 0..100 {
        if predicate(&session.state()) {
            return true;
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    false
}

#[tokio::test]
async fn the_messages_the_vehicle_sends_are_captured() {
    let vehicle = Vehicle::start(vec![
        (
            "PreArm: Compass not calibrated",
            MavSeverity::MAV_SEVERITY_CRITICAL,
        ),
        ("ARMING checks passed", MavSeverity::MAV_SEVERITY_NOTICE),
    ])
    .await;

    let spec = mpcraft_transport::ConnectionSpec::parse(&format!("udp:{vehicle}")).unwrap();
    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap();

    let captured = wait_for(&session, |state| state.messages.len() >= 2).await;

    assert!(captured, "the messages should have been captured");

    let state = session.state();

    assert_eq!(state.messages.len(), 2);
    assert_eq!(state.messages[0].text, "PreArm: Compass not calibrated");
    assert_eq!(state.messages[0].severity_name(), "CRITICAL");
    assert_eq!(state.messages[1].text, "ARMING checks passed");
    assert_eq!(state.messages[1].severity_name(), "NOTICE");

    // The messages belong to the vehicle, not to the ground station.
    assert_eq!(state.vehicles.len(), 1);
    assert_eq!(state.vehicles[0].id, VehicleId::new(1, 1));
}

#[tokio::test]
async fn an_empty_message_is_not_captured() {
    let vehicle = Vehicle::start(vec![("", MavSeverity::MAV_SEVERITY_INFO)]).await;

    let spec = mpcraft_transport::ConnectionSpec::parse(&format!("udp:{vehicle}")).unwrap();
    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap();

    // Give the vehicle the time to say its piece.
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert!(session.state().messages.is_empty());
}
