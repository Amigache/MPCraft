//! The ground station announces itself, and keeps announcing itself.
//!
//! A vehicle that only streams telemetry sends its traffic to the last ground station it
//! heard from. A ground station that announced itself once and then went silent could
//! therefore never pick up a vehicle that came up later, and could not recover a link
//! that had been half open.
//!
//! The vehicle here stands in for one that is not listening yet: it ignores the first
//! announcements and only answers once it has heard several. A ground station that
//! announced itself once would never be heard again.

use std::net::SocketAddr;
use std::time::Duration;

use mpcraft_link::{Session, SessionOptions, VehicleId};
use mpcraft_mavlink::dialect::messages::HEARTBEAT_DATA;
use mpcraft_mavlink::frame::{encode_v2, MavHeader};
use mpcraft_mavlink::{MavAutopilot, MavMessage, MavType};

/// How many announcements the vehicle ignores before it answers.
const ANNOUNCEMENTS_BEFORE_ANSWERING: usize = 3;

/// A vehicle that only starts talking once it has heard the ground station a few times.
struct Vehicle {
    socket: tokio::net::UdpSocket,
    sequence: u8,
    heard: usize,
}

impl Vehicle {
    /// Starts a vehicle and returns its address.
    async fn start() -> SocketAddr {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();

        tokio::spawn(async move {
            let mut vehicle = Self {
                socket,
                sequence: 0,
                heard: 0,
            };

            vehicle.run().await;
        });

        address
    }

    /// Waits for the ground station, and answers once it has heard it enough.
    async fn run(&mut self) {
        let mut buffer = [0u8; 2048];

        loop {
            let Ok((_, peer)) = self.socket.recv_from(&mut buffer).await else {
                return;
            };

            self.heard += 1;

            if self.heard < ANNOUNCEMENTS_BEFORE_ANSWERING {
                continue;
            }

            self.sequence = self.sequence.wrapping_add(1);

            let heartbeat = MavMessage::HEARTBEAT(HEARTBEAT_DATA {
                custom_mode: 5,
                mavtype: MavType::MAV_TYPE_QUADROTOR,
                autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
                ..Default::default()
            });

            let header = MavHeader {
                system_id: 1,
                component_id: 1,
                sequence: self.sequence,
            };

            let _ = self
                .socket
                .send_to(&encode_v2(&heartbeat, header), peer)
                .await;
        }
    }
}

#[tokio::test]
async fn a_vehicle_that_comes_up_later_is_picked_up() {
    let vehicle = Vehicle::start().await;

    let spec = mpcraft_transport::ConnectionSpec::parse(&format!("udp:{vehicle}")).unwrap();
    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap();

    // The vehicle has not answered yet: it is still waiting to hear the ground station
    // a few times.
    assert!(
        session.state().vehicles.is_empty(),
        "the vehicle should not have been seen yet"
    );

    // The ground station keeps announcing itself, and the vehicle answers.
    let mut seen = false;

    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(100)).await;

        if session
            .state()
            .vehicles
            .iter()
            .any(|vehicle| vehicle.id == VehicleId::new(1, 1))
        {
            seen = true;
            break;
        }
    }

    assert!(
        seen,
        "the vehicle should have been picked up without restarting the link"
    );
}
