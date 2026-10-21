//! The graphs, over a real link.
//!
//! The chain is the one the interface depends on: a vehicle sends telemetry, the session
//! decodes it, the history keeps it, and the graph draws it. Testing it over a UDP socket
//! exercises the whole of it.

use std::net::SocketAddr;
use std::time::Duration;

use mpcraft_app::GraphHistory;
use mpcraft_link::{Session, SessionOptions};
use mpcraft_mavlink::dialect::messages::{GLOBAL_POSITION_INT_DATA, HEARTBEAT_DATA};
use mpcraft_mavlink::frame::{encode_v2, MavHeader};
use mpcraft_mavlink::{MavAutopilot, MavMessage, MavType};

/// A vehicle that says where it is.
struct Vehicle {
    socket: tokio::net::UdpSocket,
    sequence: u8,
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
            };

            vehicle.run().await;
        });

        address
    }

    /// Waits for the ground station, then sends a position every tenth of a second.
    async fn run(&mut self) {
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
            custom_mode: 5,
            mavtype: MavType::MAV_TYPE_QUADROTOR,
            autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
            ..Default::default()
        });

        let _ = self
            .socket
            .send_to(&encode_v2(&heartbeat, header), peer)
            .await;

        for step in 0..50 {
            self.sequence = self.sequence.wrapping_add(1);

            let position = MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
                time_boot_ms: step * 100,
                lat: (-35_280_900_i32 + step as i32 * 10),
                lon: 149_130_000,
                alt: 584_000,
                relative_alt: 0,
                vx: 500,
                vy: 0,
                vz: 0,
                hdg: 0,
            });

            let header = MavHeader {
                system_id: 1,
                component_id: 1,
                sequence: self.sequence,
            };

            let _ = self
                .socket
                .send_to(&encode_v2(&position, header), peer)
                .await;

            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

#[tokio::test]
async fn a_graph_draws_what_the_vehicle_said() {
    let vehicle = Vehicle::start().await;

    let spec = mpcraft_transport::ConnectionSpec::parse(&format!("udp:{vehicle}")).unwrap();
    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap();

    let history = GraphHistory::new();

    // The history is fed from the state the link publishes, exactly as the shell does it.
    for _ in 0..100 {
        let state = session.state();

        if !state.vehicles.is_empty() {
            history.record(&state.telemetry);
        }

        if history.len() > 5 {
            break;
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    assert!(!history.is_empty(), "the history should have samples");

    // The graph of the ground speed, which the vehicle reports in the position message.
    let data = history
        .series("Speed/Ground Speed", None)
        .expect("the graph exists");

    assert_eq!(data.series.len(), 1);

    let series = &data.series[0];
    assert_eq!(series.name, "Ground speed");
    assert_eq!(series.unit, "m/s");

    assert!(!series.x.is_empty(), "the series should have values");
    assert_eq!(series.x.len(), series.y.len());

    // The vehicle reports five metres per second.
    let value = series.y[0];
    assert!(
        (value - 5.0).abs() < 0.01,
        "the ground speed was {value}, not 5"
    );
}
