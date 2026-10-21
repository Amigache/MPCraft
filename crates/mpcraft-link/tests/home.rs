//! The home position of a vehicle, over a real socket.
//!
//! The suite runs a **simulated vehicle on a UDP socket**: it answers the request for
//! its home, and it takes the home that is sent to it.

// `SET_HOME_POSITION` is deprecated in the specification in favour of the command
// `MAV_CMD_DO_SET_HOME`, and the suite exercises both.
#![allow(deprecated)]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpcraft_link::{HomeRequest, LinkError, Session, SessionOptions, VehicleId};
use mpcraft_mavlink::dialect::messages::{
    COMMAND_ACK_DATA, COMMAND_INT_DATA, HOME_POSITION_DATA, SET_HOME_POSITION_DATA,
};
use mpcraft_mavlink::frame::{encode_v2, FrameParser, MavHeader};
use mpcraft_mavlink::{MavCmd, MavMessage, MavResult, Message};
use mpcraft_vehicle::GeoPoint;

/// The home the simulated vehicle holds.
const HOME: (f64, f64, f64) = (-35.2809, 149.1300, 584.0);

/// What the vehicle was told.
#[derive(Debug, Default, Clone)]
struct VehicleLog {
    /// The message that moved the home, if it was one.
    message: Option<SET_HOME_POSITION_DATA>,
    /// The command that moved the home, if it was one.
    command: Option<COMMAND_INT_DATA>,
    /// How many times the vehicle was asked for its home.
    requests: u32,
}

/// A simulated vehicle that holds a home position.
struct Vehicle {
    socket: tokio::net::UdpSocket,
    sequence: u8,
    /// The home the vehicle reports.
    home: (f64, f64, f64),
    /// Whether the vehicle keeps the home it has when it is asked to move it.
    keep: bool,
    log: Arc<Mutex<VehicleLog>>,
}

impl Vehicle {
    /// Starts the vehicle and returns its address and its log.
    async fn start(keep: bool) -> (SocketAddr, Arc<Mutex<VehicleLog>>) {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();

        let log = Arc::new(Mutex::new(VehicleLog::default()));
        let shared = Arc::clone(&log);

        let mut vehicle = Self {
            socket,
            sequence: 0,
            home: HOME,
            keep,
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
            // The ground station says that it is there. The vehicle reports the home
            // it holds, the way a vehicle does when it is asked and the way it does
            // when the home is moved.
            MavMessage::HEARTBEAT(_) => {
                self.send_home(peer).await;
            }

            // The ground station asks for the home.
            MavMessage::COMMAND_LONG(command)
                if command.command == MavCmd::MAV_CMD_GET_HOME_POSITION
                    || command.command == MavCmd::MAV_CMD_REQUEST_MESSAGE =>
            {
                {
                    let mut log = self.log.lock().expect("the log");
                    log.requests += 1;
                }

                self.send_home(peer).await;
            }

            // The ground station moves the home with the command.
            MavMessage::COMMAND_INT(command) if command.command == MavCmd::MAV_CMD_DO_SET_HOME => {
                {
                    let mut log = self.log.lock().expect("the log");
                    log.command = Some(command.clone());
                }

                if !self.keep {
                    self.home = (
                        f64::from(command.x) / 1e7,
                        f64::from(command.y) / 1e7,
                        f64::from(command.z),
                    );
                }

                self.send(
                    peer,
                    MavMessage::COMMAND_ACK(COMMAND_ACK_DATA {
                        command: MavCmd::MAV_CMD_DO_SET_HOME,
                        result: MavResult::MAV_RESULT_ACCEPTED,
                        ..Default::default()
                    }),
                )
                .await;

                self.send_home(peer).await;
            }

            // The ground station moves the home with the message.
            MavMessage::SET_HOME_POSITION(message) => {
                {
                    let mut log = self.log.lock().expect("the log");
                    log.message = Some(message.clone());
                }

                if !self.keep {
                    self.home = (
                        f64::from(message.latitude) / 1e7,
                        f64::from(message.longitude) / 1e7,
                        f64::from(message.altitude) / 1e3,
                    );
                }

                // The vehicle reports the home it now holds.
                self.send_home(peer).await;
            }

            _ => {}
        }
    }

    /// Reports the home the vehicle holds.
    async fn send_home(&mut self, peer: SocketAddr) {
        let message = MavMessage::HOME_POSITION(HOME_POSITION_DATA {
            latitude: (self.home.0 * 1e7).round() as i32,
            longitude: (self.home.1 * 1e7).round() as i32,
            altitude: (self.home.2 * 1e3).round() as i32,
            ..Default::default()
        });

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

#[tokio::test]
async fn the_home_position_is_read_from_the_vehicle() {
    let (address, _log) = Vehicle::start(false).await;
    let session = session_to(address).await;

    let home = session
        .request_home_position(
            VehicleId::new(1, 1),
            HomeRequest::Command,
            Duration::from_secs(5),
        )
        .await
        .expect("the home position should be read");

    assert!((home.lat - HOME.0).abs() < 1e-7);
    assert!((home.lng - HOME.1).abs() < 1e-7);
    assert!((home.alt - HOME.2).abs() < 1e-3);
}

#[tokio::test]
async fn the_home_position_is_asked_for_with_the_message_request() {
    let (address, log) = Vehicle::start(false).await;
    let session = session_to(address).await;

    let home = session
        .request_home_position(
            VehicleId::new(1, 1),
            HomeRequest::MessageRequest,
            Duration::from_secs(5),
        )
        .await
        .expect("the home position should be read");

    assert!((home.lat - HOME.0).abs() < 1e-7);
    assert!(log.lock().expect("the log").requests >= 1);
}

#[tokio::test]
async fn the_home_position_reaches_the_telemetry() {
    let (address, _log) = Vehicle::start(false).await;
    let session = session_to(address).await;

    let reported = wait_for(
        || session.state().telemetry.home.is_some(),
        Duration::from_secs(5),
    )
    .await;

    assert!(reported, "the home never reached the telemetry");

    let home = session.state().telemetry.home.expect("the home");
    assert!((home.lat - HOME.0).abs() < 1e-7);
    assert!((home.lng - HOME.1).abs() < 1e-7);
    assert!((home.alt - HOME.2).abs() < 1e-3);
}

#[tokio::test]
async fn the_home_position_is_written_to_the_vehicle() {
    let (address, log) = Vehicle::start(false).await;
    let session = session_to(address).await;

    let asked = GeoPoint {
        lat: -35.3636,
        lng: 149.1626,
        alt: 590.5,
    };

    session
        .set_home_position(VehicleId::new(1, 1), asked, Duration::from_secs(5))
        .await
        .expect("the home should be moved");

    let log = log.lock().expect("the log").clone();
    let message = log.message.expect("the message that moved the home");

    // The coordinates travel in hundred millionths of a degree, and the altitude in
    // millimetres.
    assert_eq!(message.latitude, (-35.3636_f64 * 1e7_f64).round() as i32);
    assert_eq!(message.longitude, (149.1626_f64 * 1e7_f64).round() as i32);
    assert_eq!(message.altitude, 590_500);
    assert_eq!(message.target_system, 1);
}

#[tokio::test]
async fn a_vehicle_that_keeps_another_home_is_an_error() {
    // The vehicle refuses the move and reports the home it kept.
    let (address, _log) = Vehicle::start(true).await;
    let session = session_to(address).await;

    let asked = GeoPoint {
        lat: -35.3636,
        lng: 149.1626,
        alt: 590.5,
    };

    let error = session
        .set_home_position(VehicleId::new(1, 1), asked, Duration::from_secs(5))
        .await
        .expect_err("the home should not have been moved");

    assert!(
        matches!(error, LinkError::Rejected(_)),
        "the error was {error}"
    );
    assert!(
        error.to_string().contains("kept the home"),
        "the error was {error}"
    );
}

#[tokio::test]
async fn the_home_is_moved_with_the_command() {
    let (address, log) = Vehicle::start(false).await;
    let session = session_to(address).await;

    let asked = GeoPoint {
        lat: -35.3636,
        lng: 149.1626,
        alt: 590.0,
    };

    let result = session
        .set_home_position_by_command(VehicleId::new(1, 1), asked, Duration::from_secs(5))
        .await
        .expect("the home should be moved");

    assert_eq!(result, MavResult::MAV_RESULT_ACCEPTED);

    let log = log.lock().expect("the log").clone();
    let command = log.command.expect("the command that moved the home");

    assert_eq!(command.command, MavCmd::MAV_CMD_DO_SET_HOME);
    // The position travels in the command, scaled the same way.
    assert_eq!(command.x, (-35.3636_f64 * 1e7_f64).round() as i32);
    assert_eq!(command.y, (149.1626_f64 * 1e7_f64).round() as i32);
    assert!((command.z - 590.0).abs() < 1e-3);
}

#[tokio::test]
async fn a_vehicle_that_does_not_answer_is_a_timeout() {
    // A socket that nothing listens on: the request is never answered.
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();

    let session = session_to(address).await;

    let error = session
        .request_home_position(
            VehicleId::new(1, 1),
            HomeRequest::Command,
            Duration::from_millis(50),
        )
        .await
        .expect_err("the request should not be answered");

    assert!(
        matches!(error, LinkError::Timeout(_)),
        "the error was {error}"
    );
}
