//! What the board printed on the wire, and what the interface is told about it.
//!
//! A port that answers with nothing is almost always a configuration problem, and the only thing
//! that says which one is the text the board prints outside of MAVLink frames. The link captures
//! it, so three things have to hold for the operator to read it:
//!
//! * an attempt that gave up keeps its words, because the operator reads them after the attempt is
//!   gone — this is what upstream's connection error does when it appends the buffer;
//! * an open link shows what it is capturing, not what an earlier attempt left behind;
//! * a new link starts with an empty capture, so the words of a link that has since been opened
//!   are not shown on it.
//!
//! The transports here are scripted: they give one block of bytes once. A board that prints its
//! banner and never answers has nothing more to give, so the read reports the link closed; a board
//! that answers as a vehicle stays on the wire until the test tells it to stop, because otherwise
//! the link would be recorded as dropped before the state it captured could be read.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;

use mpcraft_app::supervisor::{
    ConnectionStatus, Connector, LinkSnapshot, ReconnectPolicy, SessionSupervisor, TokioSleeper,
};
use mpcraft_link::{LinkError, Session, SessionOptions};
use mpcraft_mavlink::dialect::messages::HEARTBEAT_DATA;
use mpcraft_mavlink::frame::{encode_v2, MavHeader};
use mpcraft_mavlink::{MavAutopilot, MavMessage, MavType};
use mpcraft_transport::{ConnectionSpec, Transport, TransportKind};

/// A line the board printed, as it arrives on the wire.
fn line(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.extend_from_slice(b"\r\n");
    bytes
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

/// One board: the block of bytes it puts on the wire, and whether it stays there.
#[derive(Debug, Clone, Default)]
struct Board {
    bytes: Vec<u8>,
    /// Whether the board answers as a vehicle, which is what keeps the link open.
    answers: bool,
}

impl Board {
    fn new(bytes: Vec<u8>, answers: bool) -> Self {
        Self { bytes, answers }
    }
}

/// A transport that gives one block of bytes once and then either closes or waits.
struct Scripted {
    board: Board,
    given: bool,
    /// Set when the test wants the board off the wire.
    closing: Arc<AtomicBool>,
}

#[async_trait]
impl Transport for Scripted {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if !self.given {
            self.given = true;

            let length = self.board.bytes.len().min(buf.len());
            buf[..length].copy_from_slice(&self.board.bytes[..length]);

            return Ok(length);
        }

        if self.board.answers {
            // A board that answers is still on the wire; a read that reports it closed would
            // record the link as dropped before anything could be read from it.
            while !self.closing.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }

        Ok(0)
    }

    async fn write_some(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Serial
    }

    fn is_open(&self) -> bool {
        !self.given || self.board.answers
    }

    async fn reconnect(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn describe(&self) -> String {
        "a board that prints its banner and answers with nothing".to_owned()
    }
}

/// A connector that hands out one scripted transport per call, in the order the boards were given.
struct ScriptedConnector {
    boards: Mutex<Vec<Board>>,
    closing: Arc<AtomicBool>,
}

impl ScriptedConnector {
    fn new(boards: Vec<Board>) -> Self {
        Self {
            boards: Mutex::new(boards),
            closing: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl Connector for ScriptedConnector {
    async fn connect(
        &self,
        _spec: &ConnectionSpec,
        options: SessionOptions,
    ) -> Result<Session, LinkError> {
        // Each call gets the next board, which is what a rate being tried one after another means:
        // the first rate hears one board, the second rate hears the next.
        let board = self
            .boards
            .lock()
            .map(|mut boards| {
                if boards.is_empty() {
                    Board::new(Vec::new(), false)
                } else {
                    boards.remove(0)
                }
            })
            .unwrap_or_default();

        Ok(Session::with_transport(
            Box::new(Scripted {
                board,
                given: false,
                closing: Arc::clone(&self.closing),
            }),
            options,
        ))
    }
}

/// A policy that gives up on the first attempt, with a wait long enough for the reader to read what
/// the board printed before the attempt is abandoned, and short enough that trying every rate does
/// not make the test slow.
fn policy() -> ReconnectPolicy {
    ReconnectPolicy {
        attempts: 0,
        initial_backoff: Duration::ZERO,
        growth: 1.0,
        max_backoff: Duration::from_millis(50),
        attempt_timeout: Duration::from_millis(120),
        surface_failure: true,
    }
}

/// Collects the states the supervisor published, with a bounded deadline.
async fn published(
    receiver: &mut mpsc::UnboundedReceiver<LinkSnapshot>,
    wanted: ConnectionStatus,
) -> Vec<LinkSnapshot> {
    let collected = tokio::time::timeout(Duration::from_secs(10), async {
        let mut snapshots = Vec::new();

        loop {
            let snapshot = receiver
                .recv()
                .await
                .expect("the supervisor should keep publishing");

            let reached = snapshot.status == wanted;
            snapshots.push(snapshot);

            if reached {
                return snapshots;
            }
        }
    })
    .await;

    collected.expect("the supervisor should reach the wanted state")
}

#[tokio::test]
async fn an_attempt_that_gave_up_keeps_what_the_board_printed() {
    // The hand: the port is opened at the first rate, the board prints its banner, nothing answers
    // as a vehicle, and the attempt gives up. The words are what tells the operator whether the
    // rate was wrong or the port was, so they must still be there when the failure is published.
    let connector = Arc::new(ScriptedConnector::new(vec![Board::new(
        line("ArduCopter V4.3.7 (0123abcd)"),
        false,
    )]));

    let supervisor = SessionSupervisor::with_parts(
        policy(),
        Arc::clone(&connector) as Arc<dyn Connector>,
        Arc::new(TokioSleeper),
    );

    let (sender, mut receiver) = mpsc::unbounded_channel::<LinkSnapshot>();

    supervisor
        .connect("serial:COM0|baud=AUTO", move |snapshot| {
            let _ = sender.send(snapshot.clone());
        })
        .await
        .expect_err("nothing answered at any rate");

    let snapshots = published(&mut receiver, ConnectionStatus::Failed).await;

    let failed = snapshots
        .last()
        .expect("the failure is the last state published");
    assert_eq!(failed.output.lines.len(), 1);
    assert_eq!(failed.output.lines[0], "ArduCopter V4.3.7 (0123abcd)");
}

#[tokio::test]
async fn an_open_link_shows_what_it_captured_rather_than_what_an_earlier_attempt_left_behind() {
    // The hand: the first rate hears a banner and nothing else, so that attempt gives up and keeps
    // its words; the second rate hears a different banner and a vehicle, and the link opens. The
    // console shows what the open link captured — the second banner — and not the words of the
    // attempt that came before it.
    //
    // The board is seen twice because the handshake asks for a group of heartbeats rather than a
    // single one: one sighting is not enough to call a board a vehicle. The board that answers is
    // held on the wire so that the link is still open when its state is read.
    let first = Board::new(line("the banner of the attempt that gave up"), false);

    let mut second_bytes = line("the banner of the link that opened");
    second_bytes.extend(heartbeat());
    second_bytes.extend(heartbeat());
    let second = Board::new(second_bytes, true);

    let connector = Arc::new(ScriptedConnector::new(vec![first, second]));

    let supervisor = SessionSupervisor::with_parts(
        policy(),
        Arc::clone(&connector) as Arc<dyn Connector>,
        Arc::new(TokioSleeper),
    );

    let (sender, mut receiver) = mpsc::unbounded_channel::<LinkSnapshot>();

    supervisor
        .connect("serial:COM0|baud=AUTO", move |snapshot| {
            let _ = sender.send(snapshot.clone());
        })
        .await
        .expect("the link should open at the second rate");

    let snapshots = published(&mut receiver, ConnectionStatus::Connected).await;

    let open = snapshots
        .last()
        .expect("the open link is the last state published");
    assert_eq!(open.output.lines.len(), 1);
    assert_eq!(open.output.lines[0], "the banner of the link that opened");

    // The board is taken off the wire and the link is closed, so that the task the supervisor
    // started does not outlive the test.
    connector.closing.store(true, Ordering::Relaxed);
    supervisor.disconnect().await.expect("the link was open");
}

#[tokio::test]
async fn the_words_of_an_attempt_are_forgotten_when_a_new_link_is_opened() {
    // The hand: the first attempt prints a banner and gives up, so its words are kept; the operator
    // then asks for another link, and the boards given run out, so every rate on that link hears
    // nothing. The state published while that second link is being opened has to be empty — it is
    // the one that would have shown the words of the first attempt if they had not been forgotten.
    let connector = Arc::new(ScriptedConnector::new(vec![Board::new(
        line("ArduCopter V4.3.7 (0123abcd)"),
        false,
    )]));

    let supervisor = SessionSupervisor::with_parts(
        policy(),
        Arc::clone(&connector) as Arc<dyn Connector>,
        Arc::new(TokioSleeper),
    );

    let (sender, mut receiver) = mpsc::unbounded_channel::<LinkSnapshot>();
    let first_sender = sender.clone();

    supervisor
        .connect("serial:COM0|baud=AUTO", move |snapshot| {
            let _ = first_sender.send(snapshot.clone());
        })
        .await
        .expect_err("nothing answered at any rate");

    let first = published(&mut receiver, ConnectionStatus::Failed).await;
    assert_eq!(first.last().expect("the failure").output.lines.len(), 1);

    supervisor
        .disconnect()
        .await
        .expect("no link was open to close");

    supervisor
        .connect("serial:COM1|baud=AUTO", move |snapshot| {
            let _ = sender.send(snapshot.clone());
        })
        .await
        .expect_err("nothing answered at any rate");

    let second = published(&mut receiver, ConnectionStatus::Failed).await;

    let opening = second
        .iter()
        .find(|snapshot| snapshot.status == ConnectionStatus::Connecting)
        .expect("the second link was opened");

    assert!(opening.output.lines.is_empty());
}
