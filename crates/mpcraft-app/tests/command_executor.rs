//! The generic executor, as the interface reaches it.
//!
//! The named actions — the arm, the mode change, the motor test — are this executor with the slots
//! written down for one command. Row 3.5 is the entry point that lets the operator choose the command
//! instead of the name, so three things have to hold across it:
//!
//! * the command chosen from the catalogue reaches the wire with the slots as they were typed;
//! * what comes back is what the vehicle reported, not a word this project chose for it, and a refusal
//!   is kept apart from a board that never answered;
//! * a value the catalogue does not describe is refused before anything is sent, because the entry
//!   point is the catalogue: without the table the interface cannot say what a slot means. Upstream
//!   sends such a command anyway, because its dropdown is a list of names rather than a table with
//!   meanings.
//!
//! Upstream reaches the same executor through its actions dropdown, where the names are written out by
//! hand and the parameters are decided name by name; MPCraft reaches it through the table, so there is
//! no list of names to keep in step with the dialect.
//!
//! The board is scripted: two heartbeats — one sighting is not enough for the handshake to call a board
//! a vehicle — and then the answer the test says it answers, given only after the command was written,
//! which is what an acknowledgement is.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use mpcraft_app::supervisor::{Connector, ReconnectPolicy, SessionSupervisor, TokioSleeper};
use mpcraft_link::{LinkError, Session, SessionOptions};
use mpcraft_mavlink::dialect::messages::{COMMAND_ACK_DATA, HEARTBEAT_DATA};
use mpcraft_mavlink::frame::{encode_v2, FrameParser, MavHeader};
use mpcraft_mavlink::{MavAutopilot, MavMessage, MavResult, MavType, Message};
use mpcraft_transport::{ConnectionSpec, Transport, TransportKind};

/// `MAV_CMD_NAV_WAYPOINT`: the catalogue says its slots are a heading, a distance, and then latitude,
/// longitude and altitude.
const WAYPOINT: u32 = 16;

/// The seven slots as an operator would type them for that command. The two places are written so that
/// a float carries them exactly — `-33.875` and `151.25` are exact in binary — so what the test asserts
/// is that the executor sends what it was given, without rescaling it.
fn slots() -> [f32; 7] {
    [30.0, 250.0, 100.0, 0.0, -33.875, 151.25, 60.0]
}

/// One heartbeat, as the board would send it.
fn heartbeat() -> Vec<u8> {
    let message = MavMessage::HEARTBEAT(HEARTBEAT_DATA {
        custom_mode: 0,
        mavtype: MavType::MAV_TYPE_QUADROTOR,
        autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
        ..Default::default()
    });

    encode_v2(
        &message,
        MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: 0,
        },
    )
}

/// One board: what it puts on the wire when the port is opened, and what it answers a command with.
#[derive(Debug, Clone, Default)]
struct Board {
    hello: Vec<u8>,
    /// Nothing means a board that never answers, which is a timeout rather than a refusal.
    answer: Option<MavResult>,
    /// What the ground station wrote, decoded, so that the test can read what reached the wire.
    written: Arc<Mutex<Vec<MavMessage>>>,
    closing: Arc<AtomicBool>,
}

impl Board {
    fn new(answer: Option<MavResult>) -> Self {
        Self {
            hello: [heartbeat(), heartbeat()].concat(),
            answer,
            written: Arc::new(Mutex::new(Vec::new())),
            closing: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// A transport that gives its block of bytes once and then answers the commands it is sent.
struct Scripted {
    board: Board,
    pending: VecDeque<Vec<u8>>,
    reader: FrameParser,
    given: bool,
}

#[async_trait]
impl Transport for Scripted {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if !self.given {
            self.given = true;

            let length = self.board.hello.len().min(buf.len());
            buf[..length].copy_from_slice(&self.board.hello[..length]);

            return Ok(length);
        }

        if let Some(bytes) = self.pending.pop_front() {
            let length = bytes.len().min(buf.len());
            buf[..length].copy_from_slice(&bytes[..length]);

            return Ok(length);
        }

        // A board that is still on the wire cannot be reported as closed, because the link would be
        // recorded as dropped before the answer it gave could be read.
        while !self.board.closing.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        Ok(0)
    }

    async fn write_some(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut frames = Vec::new();

        if self.reader.feed(buf, &mut frames).is_ok() {
            for frame in frames {
                let Ok(message) = mpcraft_mavlink::dialect::Dialect::parse(
                    frame.version,
                    frame.message_id,
                    &frame.payload,
                ) else {
                    continue;
                };

                if let MavMessage::COMMAND_LONG(command) = &message {
                    self.record(&message);

                    // The acknowledgement answers the command the board was sent, not one it has not
                    // seen, so an announcement of the ground station is not answered.
                    if let Some(result) = self.board.answer {
                        let ack = MavMessage::COMMAND_ACK(COMMAND_ACK_DATA {
                            command: command.command,
                            result,
                            ..Default::default()
                        });

                        self.pending.push_back(encode_v2(
                            &ack,
                            MavHeader {
                                system_id: 1,
                                component_id: 1,
                                sequence: 0,
                            },
                        ));
                    }
                }
            }
        }

        Ok(buf.len())
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Serial
    }

    fn is_open(&self) -> bool {
        !self.given || !self.board.closing.load(Ordering::Relaxed)
    }

    async fn reconnect(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn describe(&self) -> String {
        "a board that answers the commands it is sent".to_owned()
    }
}

impl Scripted {
    fn new(board: Board) -> Self {
        Self {
            board,
            pending: VecDeque::new(),
            reader: FrameParser::new(),
            given: false,
        }
    }

    /// Keeps what the ground station sent, as the message it decodes to.
    fn record(&self, message: &MavMessage) {
        if let Ok(mut written) = self.board.written.lock() {
            written.push(message.clone());
        }
    }
}

/// A connector that hands out one scripted transport per call.
struct ScriptedConnector {
    board: Board,
}

impl ScriptedConnector {
    fn new(board: Board) -> Self {
        Self { board }
    }
}

#[async_trait]
impl Connector for ScriptedConnector {
    async fn connect(
        &self,
        _spec: &ConnectionSpec,
        options: SessionOptions,
    ) -> Result<Session, LinkError> {
        Ok(Session::with_transport(
            Box::new(Scripted::new(self.board.clone())),
            options,
        ))
    }
}

/// Opens a link to one board and keeps the link open for the test.
async fn open(board: Board) -> (SessionSupervisor, Arc<AtomicBool>) {
    let closing = Arc::clone(&board.closing);

    let supervisor = SessionSupervisor::with_parts(
        ReconnectPolicy {
            attempts: 0,
            initial_backoff: Duration::ZERO,
            growth: 1.0,
            max_backoff: Duration::from_millis(50),
            attempt_timeout: Duration::from_millis(400),
            surface_failure: true,
        },
        Arc::new(ScriptedConnector::new(board)) as Arc<dyn Connector>,
        Arc::new(TokioSleeper),
    );

    supervisor
        .connect("serial:COM0", |_| {})
        .await
        .expect("the board is seen twice, so the link opens");

    (supervisor, closing)
}

#[tokio::test]
async fn the_command_of_the_catalogue_reaches_the_wire_with_the_slots_as_they_were_typed() {
    let board = Board::new(Some(MavResult::MAV_RESULT_ACCEPTED));
    let written = Arc::clone(&board.written);

    let (supervisor, closing) = open(board).await;

    let answer = supervisor
        .run_command(mpcraft_link::VehicleId::new(1, 1), WAYPOINT, slots())
        .await
        .expect("the vehicle answered");

    // The names are the ones the protocol gives, which is what the interface shows.
    assert_eq!(answer.command, "MAV_CMD_NAV_WAYPOINT");
    assert_eq!(answer.result, "MAV_RESULT_ACCEPTED");

    let recorded = what_reached_the_wire(&written);
    let command = recorded
        .first()
        .expect("the ground station sent a COMMAND_LONG");

    let MavMessage::COMMAND_LONG(sent) = command else {
        panic!("what was sent is not a COMMAND_LONG");
    };

    assert_eq!(sent.command as u32, WAYPOINT);
    assert_eq!(sent.param1, 30.0);
    assert_eq!(sent.param2, 250.0);
    assert_eq!(sent.param3, 100.0);
    assert_eq!(sent.param4, 0.0);

    // The places travel as the float form carries them. The integer form, which is where a latitude and
    // a longitude keep their precision, belongs to the plan editor; this entry point sends what was
    // typed, as upstream's generic action does.
    assert_eq!(sent.param5, -33.875);
    assert_eq!(sent.param6, 151.25);
    assert_eq!(sent.param7, 60.0);

    closing.store(true, Ordering::Relaxed);
    supervisor.disconnect().await.expect("the link was open");
}

#[tokio::test]
async fn a_refusal_keeps_the_result_the_vehicle_reported_and_is_not_a_timeout() {
    let board = Board::new(Some(MavResult::MAV_RESULT_DENIED));

    let (supervisor, closing) = open(board).await;

    let error = supervisor
        .run_command(mpcraft_link::VehicleId::new(1, 1), WAYPOINT, slots())
        .await
        .expect_err("the vehicle refused the command");

    // Upstream's executor returns a bool, so a refusal and silence look alike to the caller. Here the
    // code says which one it was, and the message carries the result the vehicle reported.
    assert_eq!(error.code, "rejected_by_vehicle");
    assert!(error.message.contains("DENIED"));

    closing.store(true, Ordering::Relaxed);
    supervisor.disconnect().await.expect("the link was open");
}

#[tokio::test]
async fn a_command_the_catalogue_does_not_describe_is_refused_before_anything_is_sent() {
    // The hand: there is no link at all, so a rule that asked for the link first would say
    // `not_connected`. The request names something this station cannot send — 42650 is the bootloader
    // command, which the dialect names but the table does not describe, so its slots have no meaning
    // the interface could show — and the request itself is what is refused, before any wire is touched.
    let supervisor = SessionSupervisor::new();

    let error = supervisor
        .run_command(mpcraft_link::VehicleId::new(1, 1), 42650, [0.0; 7])
        .await
        .expect_err("the catalogue does not describe this command");

    assert_eq!(error.code, "invalid_request");
    assert!(error.message.contains("42650"));
}

/// Reads what the board recorded. The write happened while the command was being sent, so there is
/// nothing to wait for here.
fn what_reached_the_wire(written: &Arc<Mutex<Vec<MavMessage>>>) -> Vec<MavMessage> {
    written
        .lock()
        .expect("the board keeps what it was sent")
        .clone()
}
