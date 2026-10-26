//! The ask for the streams the flight-data window reads, as the interface reaches it.
//!
//! A vehicle sends by the rates configured on the port it is talking on, and those rates can all be
//! zero, so a station that does not ask sees nothing. Three things have to hold across the ask:
//!
//! * the wire carries what the table says — the message the dialect names, and the interval its rate
//!   turns into;
//! * the answer of the vehicle is kept for each of the asks rather than stopped at the first refusal:
//!   a board that refuses one message is not a board that refuses all of them, and an operator has to
//!   see which screens will stay empty;
//! * a station that is not connected asks nothing, the way the tab of the outputs asks nothing until
//!   the link is open.
//!
//! The numbers are read from the dialect rather than copied into the test, which is what keeps the
//! test a second implementation rather than a mirror of the code it checks.

use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use mpcraft_app::streams::table;
use mpcraft_app::supervisor::{Connector, ReconnectPolicy, SessionSupervisor, TokioSleeper};
use mpcraft_link::{LinkError, Session, SessionOptions};
use mpcraft_mavlink::dialect::{message_id, messages::COMMAND_ACK_DATA, messages::HEARTBEAT_DATA};
use mpcraft_mavlink::frame::{encode_v2, FrameParser, MavHeader};
use mpcraft_mavlink::{MavAutopilot, MavCmd, MavMessage, MavResult, MavType, Message};
use mpcraft_transport::{ConnectionSpec, Transport, TransportKind};

/// `MAV_CMD_SET_MESSAGE_INTERVAL`: the command the ask travels on. The number is the one the published
/// dictionary gives the command — which is not the number its retired neighbour had — so the test reads it
/// from the dictionary and the dialect is checked against it below rather than assumed.
const SET_MESSAGE_INTERVAL: u32 = 511;

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

/// One board: what it puts on the wire when the port is opened, and what it answers each ask with.
#[derive(Debug, Clone)]
struct Board {
    hello: Vec<u8>,
    /// The messages this board refuses.
    denied: HashSet<u32>,
    /// What the ground station wrote, decoded, so that the test can read what reached the wire.
    written: Arc<Mutex<Vec<MavMessage>>>,
    closing: Arc<AtomicBool>,
}

impl Board {
    fn new(denied: &[&str]) -> Self {
        Self {
            hello: [heartbeat(), heartbeat()].concat(),
            denied: denied.iter().filter_map(|name| message_id(name)).collect(),
            written: Arc::new(Mutex::new(Vec::new())),
            closing: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// A transport that gives its block of bytes once and then answers the asks it is sent.
struct Scripted {
    board: Board,
    pending: VecDeque<Vec<u8>>,
    reader: FrameParser,
    given: bool,
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
        // recorded as dropped before the answers it gave could be read.
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
                    if let Ok(mut written) = self.board.written.lock() {
                        written.push(message.clone());
                    }

                    if command.command as u32 != SET_MESSAGE_INTERVAL {
                        continue;
                    }

                    // The first slot carries the message id as a number, so the denial is read from it as
                    // a number — not as the bits of the float, which are a different thing.
                    let result = if self.board.denied.contains(&(command.param1 as u32)) {
                        MavResult::MAV_RESULT_DENIED
                    } else {
                        MavResult::MAV_RESULT_ACCEPTED
                    };

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
        "a board that answers the asks it is sent".to_owned()
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

/// The commands that reached the wire, in the order the station wrote them.
fn what_reached_the_wire(written: &Arc<Mutex<Vec<MavMessage>>>) -> Vec<MavMessage> {
    written
        .lock()
        .map(|messages| messages.clone())
        .unwrap_or_default()
}

#[tokio::test]
async fn the_ask_carries_the_message_and_the_interval_the_table_says() {
    let board = Board::new(&[]);
    let written = Arc::clone(&board.written);

    let (supervisor, closing) = open(board).await;

    let answers = supervisor
        .request_telemetry_streams(mpcraft_link::VehicleId::new(1, 1))
        .await
        .expect("the board answered every ask");

    assert!(
        answers
            .iter()
            .all(|answer| answer.answer == "MAV_RESULT_ACCEPTED"),
        "a board that answers each ask says so, one answer per message"
    );

    // The whole table goes out, one command per message, and nothing else.
    let sent = what_reached_the_wire(&written);
    let rows = table();
    assert_eq!(sent.len(), rows.len(), "the table was asked as a whole");

    for entry in &sent {
        let MavMessage::COMMAND_LONG(command) = entry else {
            panic!("what was sent is not a COMMAND_LONG");
        };

        assert_eq!(command.command as u32, SET_MESSAGE_INTERVAL);

        // The first slot carries the message id as a number, so the number is read back as a number.
        let stream = rows
            .iter()
            .find(|row| row.id == command.param1 as u32)
            .expect("the ask carried a message of the table");

        // The number is the one the dialect gives the message — read from the dialect here rather
        // than copied — and the interval is the rate turned into it.
        assert_eq!(
            command.param1 as u32,
            message_id(stream.name).expect("the dialect names its own messages"),
            "the wire carries the message the table names"
        );
        assert_eq!(
            command.param2,
            stream.interval_us(),
            "the wire carries the rate as an interval"
        );
    }

    // Three of them, worked out by hand: 10 Hz is 100 000 microseconds, 4 Hz is 250 000, and 2 Hz
    // is 500 000. These are the numbers the experiment against a live vehicle found — the attitude
    // and the raw satellites arrived only after they were asked for.
    // The command itself, read from the dictionary: 511. A number written down is a number that can fall
    // out of step with the source it was copied from, so the agreement between the dictionary and the
    // dialect is what the test checks rather than something assumed — 52 is the number of the command this
    // ask replaced, not the number of this one.
    assert_eq!(
        MavCmd::MAV_CMD_SET_MESSAGE_INTERVAL as u32,
        SET_MESSAGE_INTERVAL,
        "the command the ask travels on is the one the dictionary names, and the dialect agrees"
    );

    let attitude = sent
        .iter()
        .find(|entry| matches!(entry, MavMessage::COMMAND_LONG(command) if command.param1 as u32 == 30))
        .expect("the ask for ATTITUDE reached the wire");
    let MavMessage::COMMAND_LONG(attitude) = attitude else {
        panic!("the ask for ATTITUDE is not a COMMAND_LONG");
    };
    assert_eq!(
        attitude.param1 as u32, 30,
        "ATTITUDE is the thirtieth message"
    );
    assert_eq!(attitude.param2, 100_000.0);

    let gps = sent
        .iter()
        .find(|entry| matches!(entry, MavMessage::COMMAND_LONG(command) if command.param1 as u32 == 24))
        .expect("the ask for GPS_RAW_INT reached the wire");
    let MavMessage::COMMAND_LONG(gps) = gps else {
        panic!("the ask for GPS_RAW_INT is not a COMMAND_LONG");
    };
    assert_eq!(
        gps.param1 as u32, 24,
        "GPS_RAW_INT is the twenty-fourth message"
    );
    assert_eq!(gps.param2, 500_000.0);

    closing.store(true, Ordering::Relaxed);
    supervisor.disconnect().await.expect("the link was open");
}

#[tokio::test]
async fn a_refusal_is_kept_apart_and_the_ask_does_not_stop_at_it() {
    // A board that refuses two of the messages and answers the rest. A station that stopped at the
    // first refusal would leave the operator with one word rather than with the fourteen the table
    // asked for, and would not say which screens will stay empty.
    let board = Board::new(&["ATTITUDE", "GPS_RAW_INT"]);
    let written = Arc::clone(&board.written);

    let (supervisor, closing) = open(board).await;

    let answers = supervisor
        .request_telemetry_streams(mpcraft_link::VehicleId::new(1, 1))
        .await
        .expect("the answers are kept even when the vehicle refuses some of them");

    assert_eq!(answers.len(), table().len(), "every ask has its own answer");

    let refused: Vec<String> = answers
        .iter()
        .filter(|answer| answer.answer != "MAV_RESULT_ACCEPTED")
        .map(|answer| answer.name.clone())
        .collect();

    assert_eq!(
        refused,
        vec!["ATTITUDE".to_owned(), "GPS_RAW_INT".to_owned()]
    );

    // The names are the ones the dialect gives, which is what the interface shows.
    for answer in &answers {
        assert!(
            answer.answer == "MAV_RESULT_ACCEPTED" || answer.answer == "MAV_RESULT_DENIED",
            "the answer says what the vehicle said, not what this station wanted"
        );
    }

    let sent = what_reached_the_wire(&written);
    assert_eq!(
        sent.len(),
        table().len(),
        "the asks after a refusal were still made"
    );

    closing.store(true, Ordering::Relaxed);
    supervisor.disconnect().await.expect("the link was open");
}

#[tokio::test]
async fn a_station_that_is_not_connected_asks_nothing() {
    // There is no link at all, so the rule that an operation needs an open link is what answers, and
    // nothing was put on a wire that is not there.
    let supervisor = SessionSupervisor::new();

    let error = supervisor
        .request_telemetry_streams(mpcraft_link::VehicleId::new(1, 1))
        .await
        .expect_err("a station that is not connected cannot ask anything");

    assert_eq!(error.code, "not_connected");

    // The same rule covers the ask for the capabilities, which the plan editor makes the way the tab of
    // the outputs makes its ask: without a link there is nothing to ask, and nothing is sent.
    let error = supervisor
        .read_capabilities(mpcraft_link::VehicleId::new(1, 1))
        .await
        .expect_err("a station that is not connected cannot ask anything");

    assert_eq!(error.code, "not_connected");
}
