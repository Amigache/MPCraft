//! The session: a transport, a reader task, and the state that flows from them.
//!
//! # The concurrency model
//!
//! Mission Planner has **two** consumers competing for the same socket: a background
//! `SerialReader` loop and every blocking request/response transaction. They are arbitrated
//! by a cooperative boolean flag (`giveComport`) and a semaphore. A transaction that forgets
//! the flag deadlocks the entire link.
//!
//! MPCraft replaces this with an actor model:
//!
//! * **One reader task per session** owns the byte stream. It parses frames, updates the
//!   vehicle state, and serves queued writes.
//! * **Transactions** register a matcher and await a dedicated `oneshot` responder. They
//!   never rely on channel buffering, which is lossy under load.
//! * **Telemetry aggregation** happens in the reader task, and the result is published
//!   through an atomic swap.
//!
//! Concurrent transactions are therefore safe by construction, and a slow transaction cannot
//! starve telemetry.
//!
//! # Why writes are queued rather than concurrent
//!
//! A serial port cannot be read and written at the same time: the underlying handle is
//! exclusive. The reader task therefore owns the transport outright and serves queued writes
//! between reads. To keep the delay between a command and its transmission small, a read is
//! bounded by [`SessionOptions::poll_interval`]; the default of ten milliseconds is well
//! below what a pilot would notice.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use mpcraft_mavlink::dialect::Dialect;
use mpcraft_mavlink::{Frame, FrameParser, MavMessage, MavlinkVersion, Message};
use mpcraft_transport::{ConnectionSpec, Transport};
use tokio::sync::{mpsc, oneshot, watch};

use crate::error::LinkError;
use crate::heartbeat::{Heartbeat, HeartbeatCollector};
use crate::loss::{LossAccountant, LossStats};
use crate::plaintext::PlaintextSniffer;
use crate::telemetry::TelemetryAggregator;
use crate::vehicle::{Vehicle, VehicleId, VehicleKind, VehicleRegistry};

/// The size of the read buffer.
const READ_BUFFER_SIZE: usize = 4096;

/// The tunables of a session.
#[derive(Debug, Clone, Copy)]
pub struct SessionOptions {
    /// How long the reader waits for data before checking for queued work.
    ///
    /// This bounds the delay between a command being issued and it reaching the wire.
    pub poll_interval: Duration,
    /// How long to wait for a vehicle to answer during the handshake.
    pub connect_timeout: Duration,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(10),
            connect_timeout: Duration::from_secs(30),
        }
    }
}

/// A message that arrived on the link.
#[derive(Debug, Clone)]
pub struct ReceivedMessage {
    /// The system id of the sender.
    pub system_id: u8,
    /// The component id of the sender.
    pub component_id: u8,
    /// The message id.
    pub message_id: u32,
    /// The name of the message, when the dialect knows it.
    pub message_name: String,
    /// The decoded message.
    pub message: Dialect,
}

impl ReceivedMessage {
    /// The identity of the sender.
    #[must_use]
    pub fn vehicle(&self) -> VehicleId {
        VehicleId::new(self.system_id, self.component_id)
    }
}

/// A command sent to the reader task.
enum Command {
    /// Write bytes to the link.
    Write {
        /// The bytes to write.
        bytes: Vec<u8>,
        /// Where to report the outcome.
        respond: Option<oneshot::Sender<Result<(), LinkError>>>,
    },
    /// Wait for a message that matches a predicate.
    Request {
        /// The predicate a message must satisfy.
        matcher: Box<dyn Fn(&ReceivedMessage) -> bool + Send>,
        /// Where to deliver the matching message.
        respond: oneshot::Sender<Result<ReceivedMessage, LinkError>>,
    },
    /// Stop the session.
    Stop,
}

/// The state a session publishes.
#[derive(Debug, Clone)]
pub struct SessionState {
    /// The vehicles seen on the link.
    pub vehicles: Vec<Vehicle>,
    /// The telemetry of the primary vehicle.
    pub telemetry: mpcraft_vehicle::Telemetry,
    /// The loss statistics of the primary vehicle.
    pub loss: LossStats,
    /// The lines of board output captured outside of MAVLink frames.
    pub plaintext: Vec<String>,
    /// Whether the board's output looks like a DroneCAN bus.
    pub looks_like_can_bus: bool,
    /// Whether the link is still open.
    pub connected: bool,
}

/// A live link to a vehicle.
///
/// The session owns a reader task that holds the transport. Everything else — telemetry,
/// the vehicle registry, transactions — reads from the state the task publishes.
#[derive(Debug)]
pub struct Session {
    commands: mpsc::Sender<Command>,
    state: watch::Receiver<SessionState>,
    options: SessionOptions,
    stopped: Arc<AtomicBool>,
}

impl Session {
    /// Opens a session on the given connection.
    ///
    /// # Errors
    ///
    /// Returns an error when the transport cannot be opened.
    pub async fn connect(
        spec: &ConnectionSpec,
        options: SessionOptions,
    ) -> Result<Self, LinkError> {
        let transport = mpcraft_transport::open(spec).await?;
        Ok(Self::with_transport(transport, options))
    }

    /// Opens a session on an already-open transport.
    #[must_use]
    pub fn with_transport(transport: Box<dyn Transport>, options: SessionOptions) -> Self {
        let (commands, receiver) = mpsc::channel::<Command>(64);
        let (state_tx, state_rx) = watch::channel(SessionState {
            vehicles: Vec::new(),
            telemetry: mpcraft_vehicle::Telemetry::default(),
            loss: LossStats::default(),
            plaintext: Vec::new(),
            looks_like_can_bus: false,
            connected: true,
        });

        let stopped = Arc::new(AtomicBool::new(false));
        let task_stopped = Arc::clone(&stopped);

        tokio::spawn(async move {
            run_reader(transport, receiver, state_tx, options, task_stopped).await;
        });

        Self {
            commands,
            state: state_rx,
            options,
            stopped,
        }
    }

    /// The current state of the session.
    #[must_use]
    pub fn state(&self) -> SessionState {
        self.state.borrow().clone()
    }

    /// The tunables this session was opened with.
    #[must_use]
    pub fn options(&self) -> SessionOptions {
        self.options
    }

    /// Waits until the session has stopped.
    pub async fn wait_closed(&mut self) {
        while self.state.borrow().connected {
            if self.state.changed().await.is_err() {
                break;
            }
        }
    }

    /// Whether the session has stopped.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.stopped.load(Ordering::Relaxed) || !self.state.borrow().connected
    }

    /// Writes bytes to the link.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is closed or the write fails.
    pub async fn write(&self, bytes: Vec<u8>) -> Result<(), LinkError> {
        let (respond, wait) = oneshot::channel();
        self.commands
            .send(Command::Write {
                bytes,
                respond: Some(respond),
            })
            .await
            .map_err(|_| LinkError::SessionClosed)?;

        wait.await.map_err(|_| LinkError::SessionClosed)?
    }

    /// Waits for a message that satisfies the predicate.
    ///
    /// # Errors
    ///
    /// Returns an error when the session closes before a matching message arrives, or when
    /// the timeout expires.
    pub async fn request<F>(
        &self,
        timeout: Duration,
        matcher: F,
    ) -> Result<ReceivedMessage, LinkError>
    where
        F: Fn(&ReceivedMessage) -> bool + Send + 'static,
    {
        let (respond, wait) = oneshot::channel();
        self.commands
            .send(Command::Request {
                matcher: Box::new(matcher),
                respond,
            })
            .await
            .map_err(|_| LinkError::SessionClosed)?;

        match tokio::time::timeout(timeout, wait).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(LinkError::SessionClosed),
            Err(_) => Err(LinkError::Timeout(timeout)),
        }
    }

    /// Stops the session.
    pub async fn stop(&self) {
        let _ = self.commands.send(Command::Stop).await;
    }
}

/// A pending request waiting for a matching message.
struct PendingRequest {
    matcher: Box<dyn Fn(&ReceivedMessage) -> bool + Send>,
    respond: oneshot::Sender<Result<ReceivedMessage, LinkError>>,
}

/// The reader task: the single owner of the transport.
async fn run_reader(
    mut transport: Box<dyn Transport>,
    mut commands: mpsc::Receiver<Command>,
    state_tx: watch::Sender<SessionState>,
    options: SessionOptions,
    stopped: Arc<AtomicBool>,
) {
    let mut parser = FrameParser::new();
    let mut buffer = vec![0u8; READ_BUFFER_SIZE];
    let mut frames: Vec<Frame> = Vec::new();

    let mut registry = VehicleRegistry::new();
    let mut heartbeats = HeartbeatCollector::new();
    let mut plaintext = PlaintextSniffer::new();
    let mut aggregator = TelemetryAggregator::new();
    let mut loss: LossStats = LossStats::default();

    // The loss accounting is per vehicle.
    let mut accountants: std::collections::HashMap<VehicleId, LossAccountant> =
        std::collections::HashMap::new();
    let mut last_packet_at: Option<Instant> = None;

    let mut pending: Vec<PendingRequest> = Vec::new();

    loop {
        // 1. Serve the queued writes and requests first.
        while let Ok(command) = commands.try_recv() {
            match command {
                Command::Write { bytes, respond } => {
                    let result = transport.write(&bytes).await;
                    if let Some(respond) = respond {
                        let _ = respond.send(result.map_err(LinkError::from));
                    }
                }
                Command::Request { matcher, respond } => {
                    pending.push(PendingRequest { matcher, respond });
                }
                Command::Stop => {
                    stopped.store(true, Ordering::Relaxed);
                    publish(&state_tx, &registry, &aggregator, &plaintext, loss, false);
                    return;
                }
            }
        }

        // 2. Read, bounded so that a queued write is not stuck behind a pending read.
        let read = tokio::time::timeout(options.poll_interval, transport.read(&mut buffer)).await;

        match read {
            Err(_elapsed) => continue,
            Ok(Ok(0)) => {
                // The peer closed the link cleanly. Any board output that arrived with the
                // last read is collected before the state is published.
                let discarded = parser.take_plaintext();
                if !discarded.is_empty() {
                    plaintext.observe(&discarded);
                }

                stopped.store(true, Ordering::Relaxed);
                publish(&state_tx, &registry, &aggregator, &plaintext, loss, false);
                return;
            }
            Ok(Ok(count)) => {
                let now = Instant::now();

                frames.clear();
                if parser.feed(&buffer[..count], &mut frames).is_ok() {
                    for frame in &frames {
                        handle_frame(
                            frame,
                            now,
                            &mut registry,
                            &mut heartbeats,
                            &mut aggregator,
                            &mut accountants,
                            &mut last_packet_at,
                            &mut loss,
                        );
                    }
                }

                // The bytes that were not part of a frame are the board's own output. They
                // are collected after the feed, because that is when the parser knows which
                // bytes they were.
                let discarded = parser.take_plaintext();
                if !discarded.is_empty() {
                    plaintext.observe(&discarded);
                }

                // Deliver any message that satisfies a pending request.
                if !pending.is_empty() {
                    deliver(&mut pending, &frames);
                }

                publish(&state_tx, &registry, &aggregator, &plaintext, loss, true);
            }
            Ok(Err(error)) => {
                tracing::warn!("the link failed: {error}");
                stopped.store(true, Ordering::Relaxed);
                publish(&state_tx, &registry, &aggregator, &plaintext, loss, false);
                return;
            }
        }
    }
}

/// Handles one frame: updates the vehicle state and the telemetry.
#[allow(clippy::too_many_arguments)]
fn handle_frame(
    frame: &Frame,
    now: Instant,
    registry: &mut VehicleRegistry,
    heartbeats: &mut HeartbeatCollector,
    aggregator: &mut TelemetryAggregator,
    accountants: &mut std::collections::HashMap<VehicleId, LossAccountant>,
    last_packet_at: &mut Option<Instant>,
    loss: &mut LossStats,
) {
    let id = VehicleId::new(frame.system_id, frame.component_id);
    let vehicle = registry.observe(id);

    // The loss accounting is per vehicle, and follows the sequence number.
    let since_previous =
        last_packet_at.is_some_and(|last| now.saturating_duration_since(last).as_secs_f64() < 1.0);
    let accountant = accountants.entry(id).or_default();
    accountant.observe(frame.sequence, since_previous);
    *loss = accountant.stats();
    *last_packet_at = Some(now);

    // Decode the message.
    let Ok(message) = Dialect::parse(frame.version, frame.message_id, &frame.payload) else {
        return;
    };

    // The heartbeat identifies the vehicle.
    if frame.message_id == crate::heartbeat::HEARTBEAT_ID
        || frame.message_id == crate::heartbeat::HIGH_LATENCY2_ID
    {
        let mav_type = match &message {
            MavMessage::HEARTBEAT(heartbeat) => Some(heartbeat.mavtype),
            _ => None,
        };

        let _ = heartbeats.observe(Heartbeat {
            system_id: frame.system_id,
            component_id: frame.component_id,
            message_id: frame.message_id,
            mav_type,
        });

        if let MavMessage::HEARTBEAT(heartbeat) = &message {
            vehicle.mav_type = Some(heartbeat.mavtype);
            vehicle.autopilot = Some(heartbeat.autopilot);
            vehicle.kind = Some(VehicleKind::from_heartbeat(
                heartbeat.autopilot,
                heartbeat.mavtype,
            ));
            vehicle.custom_mode = Some(heartbeat.custom_mode);
        }
    }

    aggregator.apply(&message);
}

/// Delivers the messages that satisfy a pending request.
fn deliver(pending: &mut Vec<PendingRequest>, frames: &[Frame]) {
    let mut remaining = Vec::with_capacity(pending.len());

    for request in pending.drain(..) {
        let mut matched: Option<ReceivedMessage> = None;

        for frame in frames {
            let Ok(message) = Dialect::parse(frame.version, frame.message_id, &frame.payload)
            else {
                continue;
            };

            let received = ReceivedMessage {
                system_id: frame.system_id,
                component_id: frame.component_id,
                message_id: frame.message_id,
                message_name: message.message_name().to_owned(),
                message,
            };

            if (request.matcher)(&received) {
                matched = Some(received);
                break;
            }
        }

        match matched {
            Some(received) => {
                let _ = request.respond.send(Ok(received));
            }
            None => remaining.push(request),
        }
    }

    *pending = remaining;
}

/// Publishes the current state.
fn publish(
    state_tx: &watch::Sender<SessionState>,
    registry: &VehicleRegistry,
    aggregator: &TelemetryAggregator,
    plaintext: &PlaintextSniffer,
    loss: LossStats,
    connected: bool,
) {
    let state = SessionState {
        vehicles: registry.vehicles().into_iter().cloned().collect(),
        telemetry: aggregator.telemetry().clone(),
        loss,
        plaintext: plaintext.lines(),
        looks_like_can_bus: plaintext.looks_like_can_bus(),
        connected,
    };

    // A closed watch channel simply means nobody is listening any more.
    let _ = state_tx.send(state);
}

/// The MAVLink version a frame used, for the frame parser.
#[must_use]
pub fn frame_version(frame: &Frame) -> MavlinkVersion {
    frame.version
}
