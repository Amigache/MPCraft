//! The relays of the vehicle, over a real socket.
//!
//! The vehicle reports its relays with `RELAY_STATUS`, which carries two masks: which relays it
//! has and which of them are on. Two things are checked here.
//!
//! The masks are read into the list the interface draws, so that nothing above the link has to
//! know that a relay is a bit — the numbers come out in the order their bits are counted from the
//! first, and a bit that is only in the second mask is not a relay the vehicle has.
//!
//! And a relay the vehicle has said it does not have is refused before anything is sent: that is
//! not something this board would do at another moment, it is something it cannot do. A vehicle
//! that has never reported its relays is asked anyway, because the report is not streamed unless
//! the station asks for it, and refusing on silence would refuse a board that can do it.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpcraft_link::{LinkError, Session, SessionOptions, VehicleId};
use mpcraft_mavlink::dialect::messages::{
    COMMAND_ACK_DATA, COMMAND_LONG_DATA, HEARTBEAT_DATA, RELAY_STATUS_DATA,
};
use mpcraft_mavlink::frame::{encode_v2, FrameParser, MavHeader};
use mpcraft_mavlink::{MavAutopilot, MavCmd, MavMessage, MavResult, MavType, Message};

/// The commands the vehicle received, so the test can assert on them.
type Received = Arc<Mutex<Vec<COMMAND_LONG_DATA>>>;

/// A simulated vehicle, with or without a report of its relays.
struct Vehicle {
    socket: tokio::net::UdpSocket,
    sequence: u8,
    /// What the vehicle says about its relays, when it says anything.
    report: Option<(u16, u16)>,
    /// Whether the vehicle says its relays on its own. It does not, unless the station asks; the
    /// field is here so that the test can tell the two cases apart.
    announces: bool,
    /// The commands the vehicle received.
    received: Received,
}

impl Vehicle {
    /// Starts the vehicle on its own port.
    async fn start(report: Option<(u16, u16)>, announces: bool) -> (SocketAddr, Received) {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();

        let received: Received = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&received);

        let mut vehicle = Self {
            socket,
            sequence: 0,
            report,
            announces,
            received: log,
        };

        tokio::spawn(async move {
            vehicle.run().await;
        });

        (address, received)
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

                // The ground station announces itself with a heartbeat when it connects. It is
                // not part of what the test is about; it is how the vehicle learns where to send.
                if matches!(decoded, MavMessage::HEARTBEAT(_)) {
                    self.announce(peer).await;
                    continue;
                }

                let MavMessage::COMMAND_LONG(command) = decoded else {
                    continue;
                };

                if let Ok(mut log) = self.received.lock() {
                    log.push(command.clone());
                }

                // A request for a message is answered with the message, and only then. This is the
                // way the station learns what the vehicle has, because the report is not streamed on
                // its own.
                if command.command == MavCmd::MAV_CMD_REQUEST_MESSAGE
                    && command.param1 as u32 == mpcraft_link::commands::RELAY_STATUS_ID
                {
                    if let Some((present, on)) = self.report {
                        self.send(
                            1,
                            peer,
                            &MavMessage::RELAY_STATUS(RELAY_STATUS_DATA {
                                time_boot_ms: 3_000,
                                present,
                                on,
                            }),
                        )
                        .await;
                    }
                }

                self.send(
                    command.target_component,
                    peer,
                    &MavMessage::COMMAND_ACK(COMMAND_ACK_DATA {
                        command: command.command,
                        result: MavResult::MAV_RESULT_ACCEPTED,
                        ..Default::default()
                    }),
                )
                .await;
            }
        }
    }

    /// Announces itself and says what its relays are, once.
    async fn announce(&mut self, peer: SocketAddr) {
        let header = MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.wrapping_add(1);

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

        if self.announces {
            if let Some((present, on)) = self.report {
                let header = MavHeader {
                    system_id: 1,
                    component_id: 1,
                    sequence: self.sequence,
                };
                self.sequence = self.sequence.wrapping_add(1);

                let report = MavMessage::RELAY_STATUS(RELAY_STATUS_DATA {
                    time_boot_ms: 3_000,
                    present,
                    on,
                });

                let _ = self.socket.send_to(&encode_v2(&report, header), peer).await;
            }
        }
    }

    async fn send(&mut self, component_id: u8, peer: SocketAddr, message: &MavMessage) {
        // The answer comes from the component the command was addressed to, which is what a
        // station matches an answer to.
        let header = MavHeader {
            system_id: 1,
            component_id,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.wrapping_add(1);

        let bytes = encode_v2(message, header);
        let _ = self.socket.send_to(&bytes, peer).await;
    }
}

/// Opens a session to the vehicle.
async fn session_to(address: SocketAddr) -> Session {
    let spec = mpcraft_transport::ConnectionSpec::parse(&format!("udp:{address}")).unwrap();
    Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap()
}

/// The autopilot the session talks to.
fn vehicle() -> VehicleId {
    VehicleId::new(1, 1)
}

/// The commands the vehicle received.
fn commands_of(received: &Received) -> Vec<COMMAND_LONG_DATA> {
    received.lock().map(|log| log.clone()).unwrap_or_default()
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
async fn the_report_of_the_relays_is_read_into_the_list_the_interface_draws() {
    // The hand: the vehicle says it has relays 0 and 2 — the mask 5 is 101 in binary, with the
    // bits at positions 0 and 2 — and that three relays are on: the mask 7 is 111. The bit at
    // position 1 is in the second mask but not in the first, so it is not a relay this vehicle
    // has, and the list holds relays 0 and 2, both on.
    let (address, _) = Vehicle::start(Some((0b0101, 0b0111)), true).await;
    let session = session_to(address).await;

    let seen = wait_for(&session, |state| {
        state
            .telemetries
            .get(&vehicle())
            .and_then(|state| state.relays.as_ref())
            .is_some_and(|relays| !relays.instances.is_empty())
    })
    .await;

    assert!(seen, "the report of the vehicle should have been read");

    let state = session.state();
    let relays = state
        .telemetries
        .get(&vehicle())
        .and_then(|state| state.relays.as_ref())
        .expect("the report of the relays");

    assert_eq!(relays.instances.len(), 2);
    assert_eq!(relays.instances[0].number, 0);
    assert!(relays.instances[0].on);
    assert_eq!(relays.instances[1].number, 2);
    assert!(relays.instances[1].on);
}

#[tokio::test]
async fn a_relay_the_vehicle_said_it_does_not_have_is_refused_before_anything_is_sent() {
    // The vehicle has relays 0 and 2. Relay 1 is not one of them, so the station cannot ask for
    // it, and it does not wait for the vehicle to say so — the refusal is the station's own.
    let (address, received) = Vehicle::start(Some((0b0101, 0b0101)), true).await;
    let session = session_to(address).await;

    let seen = wait_for(&session, |state| {
        state
            .telemetries
            .get(&vehicle())
            .and_then(|state| state.relays.as_ref())
            .is_some_and(|relays| !relays.instances.is_empty())
    })
    .await;

    assert!(seen, "the report of the vehicle should have been read");

    let error = session
        .set_relay(vehicle(), 1, true, Duration::from_secs(2))
        .await
        .expect_err("the vehicle has no relay 1");

    assert!(matches!(error, LinkError::Unsupported(_)));

    // Nothing went out: the refusal happened before the command was built.
    assert!(commands_of(&received).is_empty());
}

#[tokio::test]
async fn a_relay_the_vehicle_has_reaches_the_wire_as_it_was_asked() {
    let (address, received) = Vehicle::start(Some((0b0101, 0b0101)), true).await;
    let session = session_to(address).await;

    session
        .set_relay(vehicle(), 2, true, Duration::from_secs(5))
        .await
        .expect("the vehicle acknowledged the relay");

    let sent = commands_of(&received);
    let command = sent.first().expect("the vehicle received the command");

    assert_eq!(command.command, MavCmd::MAV_CMD_DO_SET_RELAY);
    // The instance is the first parameter and the state is the second, as the specification says:
    // one for on and zero for off.
    assert_eq!(command.param1, 2.0);
    assert_eq!(command.param2, 1.0);
    assert_eq!(command.target_system, 1);
    assert_eq!(command.target_component, 1);

    // And turning it off sends the same command with the second parameter at zero.
    session
        .set_relay(vehicle(), 2, false, Duration::from_secs(5))
        .await
        .expect("the vehicle acknowledged the relay");

    let sent = commands_of(&received);
    let off = sent.last().expect("the vehicle received the command");
    assert_eq!(off.param1, 2.0);
    assert_eq!(off.param2, 0.0);
}

#[tokio::test]
async fn a_vehicle_that_has_never_reported_its_relays_is_asked_anyway() {
    // The report is not streamed unless the station asks for it, so silence is not a reason to
    // refuse: a board that has relays and has said nothing about them yet can still be asked.
    let (address, received) = Vehicle::start(None, true).await;
    let session = session_to(address).await;

    session
        .set_relay(vehicle(), 0, true, Duration::from_secs(5))
        .await
        .expect("the vehicle acknowledged the relay");

    let sent = commands_of(&received);
    let command = sent.first().expect("the vehicle received the command");

    assert_eq!(command.command, MavCmd::MAV_CMD_DO_SET_RELAY);
    assert_eq!(command.param1, 0.0);
    assert_eq!(command.param2, 1.0);

    // The station never claimed to know what this vehicle has: the state says nothing about its
    // relays, which is what the sheet shows as an empty list rather than as relays at zero.
    assert!(session
        .state()
        .telemetries
        .get(&vehicle())
        .and_then(|state| state.relays.as_ref())
        .is_none_or(|relays| relays.instances.is_empty()));
}

#[tokio::test]
async fn the_vehicle_says_what_it_has_once_the_station_asks() {
    // A vehicle that has relays and does not say them on its own, which is what the specification
    // says of this message: it is streamed only when it is asked for. So the station asks, and what
    // the sheet draws afterwards is the state the event carries.
    let (address, received) = Vehicle::start(Some((0b0101, 0b0101)), false).await;
    let session = session_to(address).await;

    // Give the link the time to announce itself, which is how the vehicle learns where to send.
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(session
        .state()
        .telemetries
        .get(&vehicle())
        .and_then(|state| state.relays.as_ref())
        .is_none());

    let report = session
        .request_relay_status(vehicle(), Duration::from_secs(5))
        .await
        .expect("the vehicle answered the request");

    // The hand: the mask 5 is 101 in binary, so the vehicle has relays 0 and 2, and the mask 5
    // again says both of them are on.
    assert_eq!(report.instances.len(), 2);
    assert_eq!(report.instances[0].number, 0);
    assert_eq!(report.instances[1].number, 2);

    let sent = commands_of(&received);
    let request = sent.first().expect("the vehicle received the request");

    assert_eq!(request.command, MavCmd::MAV_CMD_REQUEST_MESSAGE);
    // The parameter names the message that is wanted, and the number here agrees with the one the
    // dialect gives for RELAY_STATUS.
    assert_eq!(request.param1, 376.0);

    // The same answer reads into the state of the vehicle, which is what the sheet shows.
    let seen = wait_for(&session, |state| {
        state
            .telemetries
            .get(&vehicle())
            .and_then(|state| state.relays.as_ref())
            .is_some_and(|relays| relays.instances.len() == 2)
    })
    .await;

    assert!(
        seen,
        "the report should have read into the state of the vehicle"
    );
}
