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

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use mpcraft_mavlink::dialect::Dialect;
use mpcraft_mavlink::frame::MavHeader;
use mpcraft_mavlink::{Frame, FrameParser, MavMessage, MavlinkVersion, Message};
use mpcraft_params::ParameterStore;
use mpcraft_transport::{ConnectionSpec, Transport};
use tokio::sync::{mpsc, oneshot, watch};

use crate::error::LinkError;
use crate::heartbeat::{Heartbeat, HeartbeatCollector};
use crate::loss::{LossAccountant, LossStats};
use crate::plaintext::PlaintextSniffer;
use crate::telemetry::TelemetryAggregator;
use crate::vehicle::{VehicleId, VehicleKind, VehicleRegistry};

/// The size of the read buffer.
const READ_BUFFER_SIZE: usize = 4096;

/// The system id the ground station uses.
const GCS_SYSTEM_ID: u8 = 255;

/// The component id the ground station uses.
///
/// Mission Planner uses the same value, and the vehicle checks that the items it receives
/// are addressed to it.
const GCS_COMPONENT_ID: u8 = 190;

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

/// A predicate that decides whether a message matches a request.
type Matcher = Box<dyn Fn(&ReceivedMessage) -> bool + Send>;

/// A predicate that decides whether a collection of messages is complete.
type Completion = Box<dyn Fn(&[ReceivedMessage]) -> bool + Send>;

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
        matcher: Matcher,
        /// Where to deliver the matching message.
        respond: oneshot::Sender<Result<ReceivedMessage, LinkError>>,
    },
    /// Collect messages that match a predicate until the collection is complete.
    Collect {
        /// The predicate a message must satisfy to be collected.
        matcher: Matcher,
        /// Whether the collection is complete, given what has been collected so far.
        complete: Completion,
        /// Where to deliver the collected messages.
        respond: oneshot::Sender<Result<Vec<ReceivedMessage>, LinkError>>,
    },
    /// Stop the session.
    Stop,
}

/// The state a session publishes.
#[derive(Debug, Clone)]
pub struct SessionState {
    /// The vehicles seen on the link.
    pub vehicles: Vec<crate::vehicle::Vehicle>,
    /// The telemetry of the primary vehicle.
    pub telemetry: mpcraft_vehicle::Telemetry,
    /// The loss statistics of the primary vehicle.
    pub loss: LossStats,
    /// The protocol capabilities the vehicle advertised.
    pub capabilities: Option<u64>,
    /// The parameters the vehicle has reported.
    pub parameters: ParameterStore,
    /// The lines of board output captured outside of MAVLink frames.
    pub plaintext: Vec<String>,
    /// Whether the board's output looks like a DroneCAN bus.
    pub looks_like_can_bus: bool,
    /// Whether the link is still open.
    pub connected: bool,
}

/// The mutable state the reader task owns.
///
/// Bundling it into one struct keeps the reader task's functions from taking a dozen
/// arguments, and makes it obvious that the state is only ever touched by one task.
#[derive(Debug, Default)]
struct SessionContext {
    registry: VehicleRegistry,
    heartbeats: HeartbeatCollector,
    aggregator: TelemetryAggregator,
    plaintext: PlaintextSniffer,
    accountants: HashMap<VehicleId, LossAccountant>,
    last_packet_at: Option<Instant>,
    loss: LossStats,
    capabilities: Option<u64>,
    parameters: ParameterStore,
}

impl SessionContext {
    /// Handles one frame: updates the vehicle state and the telemetry.
    fn handle_frame(&mut self, frame: &Frame, now: Instant) {
        let id = VehicleId::new(frame.system_id, frame.component_id);
        let vehicle = self.registry.observe(id);

        // The loss accounting is per vehicle, and follows the sequence number.
        let since_previous = self
            .last_packet_at
            .is_some_and(|last| now.saturating_duration_since(last).as_secs_f64() < 1.0);
        let accountant = self.accountants.entry(id).or_default();
        accountant.observe(frame.sequence, since_previous);
        self.loss = accountant.stats();
        self.last_packet_at = Some(now);

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

            let _ = self.heartbeats.observe(Heartbeat {
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

        // The protocol capabilities arrive in AUTOPILOT_VERSION and decide how the vehicle
        // encodes its parameter values.
        if let MavMessage::AUTOPILOT_VERSION(version) = &message {
            self.capabilities = Some(version.capabilities.bits());
        }

        // A PARAM_VALUE updates the parameter store, whether it was asked for or the vehicle
        // changed the value on its own.
        if let MavMessage::PARAM_VALUE(value) = &message {
            if value.param_index != crate::params::PX4_PARAM_HASH_INDEX {
                let name = crate::params::param_name_of(value);
                if !name.is_empty() {
                    let reported_type = mpcraft_params::ParamType::from_mavlink(value.param_type)
                        .unwrap_or(mpcraft_params::ParamType::Real32);
                    self.parameters.set_total_reported(value.param_count);
                    self.parameters.insert(mpcraft_params::Parameter::new(
                        name,
                        value.param_value as f64,
                        reported_type,
                    ));
                }
            }
        }

        self.aggregator.apply(&message);
    }

    /// The snapshot of the current state.
    fn snapshot(&self, connected: bool) -> SessionState {
        SessionState {
            vehicles: self.registry.vehicles().into_iter().cloned().collect(),
            telemetry: self.aggregator.telemetry().clone(),
            loss: self.loss,
            capabilities: self.capabilities,
            parameters: self.parameters.clone(),
            plaintext: self.plaintext.lines(),
            looks_like_can_bus: self.plaintext.looks_like_can_bus(),
            connected,
        }
    }
}

impl SessionState {
    /// The state of a link that is not open.
    #[must_use]
    pub fn disconnected() -> Self {
        Self {
            vehicles: Vec::new(),
            telemetry: mpcraft_vehicle::Telemetry::default(),
            loss: LossStats::default(),
            capabilities: None,
            parameters: mpcraft_params::ParameterStore::new(),
            plaintext: Vec::new(),
            looks_like_can_bus: false,
            connected: false,
        }
    }
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
        let session = Self::with_transport(transport, options);

        // The ground station announces itself. A vehicle that streams telemetry needs to
        // know where to send it, and the first frame it sees from the ground station is
        // what tells it.
        session.announce().await;

        Ok(session)
    }

    /// Sends the heartbeat that identifies this end as a ground station.
    ///
    /// A vehicle that only streams telemetry never learns the ground station's address
    /// otherwise, and its traffic would go nowhere.
    pub async fn announce(&self) {
        use mpcraft_mavlink::dialect::messages::HEARTBEAT_DATA;
        use mpcraft_mavlink::{MavAutopilot, MavModeFlag, MavState, MavType};

        let message = MavMessage::HEARTBEAT(HEARTBEAT_DATA {
            custom_mode: 0,
            mavtype: MavType::MAV_TYPE_GCS,
            autopilot: MavAutopilot::MAV_AUTOPILOT_GENERIC,
            base_mode: MavModeFlag::empty(),
            system_status: MavState::MAV_STATE_ACTIVE,
            mavlink_version: 3,
        });

        if let Err(error) = self.write_message(&message).await {
            tracing::warn!("could not announce the ground station: {error}");
        }
    }

    /// Opens a session on an already-open transport.
    #[must_use]
    pub fn with_transport(transport: Box<dyn Transport>, options: SessionOptions) -> Self {
        let (commands, receiver) = mpsc::channel::<Command>(64);
        let (state_tx, state_rx) = watch::channel(SessionContext::default().snapshot(true));

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

    /// Registers a matcher for a single message.
    ///
    /// The matcher is registered *before* the request that triggers it is sent. That order
    /// matters: a vehicle on a fast link can answer before the request has even left the
    /// session, and an answer that arrives before anybody is listening is lost.
    ///
    /// # Errors
    ///
    /// Returns an error when the session has closed.
    pub async fn begin_request<F>(&self, matcher: F) -> Result<RequestHandle, LinkError>
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

        Ok(RequestHandle { wait })
    }

    /// Registers a collector for the messages that match a predicate.
    ///
    /// The collector is registered *before* the request that triggers it is sent. That order
    /// matters: a vehicle on a fast link can answer before the request has even left the
    /// session, and an answer that arrives before anybody is listening is lost.
    ///
    /// # Errors
    ///
    /// Returns an error when the session has closed.
    pub async fn begin_collect<F, C>(&self, matcher: F, complete: C) -> Result<Collector, LinkError>
    where
        F: Fn(&ReceivedMessage) -> bool + Send + 'static,
        C: Fn(&[ReceivedMessage]) -> bool + Send + 'static,
    {
        let (respond, wait) = oneshot::channel();
        self.commands
            .send(Command::Collect {
                matcher: Box::new(matcher),
                complete: Box::new(complete),
                respond,
            })
            .await
            .map_err(|_| LinkError::SessionClosed)?;

        Ok(Collector { wait })
    }

    /// Collects the messages that match a predicate until the collection is complete.
    ///
    /// This is what a streaming protocol needs: the parameter list is a stream of
    /// `PARAM_VALUE` messages that ends when every parameter has arrived, not when one
    /// matching message does.
    ///
    /// Use [`Session::begin_collect`] when the request has to be sent *after* the collector
    /// is registered.
    ///
    /// # Errors
    ///
    /// Returns an error when the session closes before the collection is complete, or when
    /// the timeout expires.
    pub async fn collect<F, C>(
        &self,
        timeout: Duration,
        matcher: F,
        complete: C,
    ) -> Result<Vec<ReceivedMessage>, LinkError>
    where
        F: Fn(&ReceivedMessage) -> bool + Send + 'static,
        C: Fn(&[ReceivedMessage]) -> bool + Send + 'static,
    {
        let collector = self.begin_collect(matcher, complete).await?;
        collector.wait(timeout).await
    }

    /// Encodes a message and writes it to the link.
    ///
    /// The ground station identifies itself with the system and component ids that Mission
    /// Planner uses, because the vehicle checks that the items it receives are addressed to
    /// it.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is closed or the write fails.
    pub async fn write_message(&self, message: &MavMessage) -> Result<(), LinkError> {
        let header = MavHeader {
            system_id: GCS_SYSTEM_ID,
            component_id: GCS_COMPONENT_ID,
            sequence: next_sequence(),
        };

        self.write(mpcraft_mavlink::frame::encode_v2(message, header))
            .await
    }

    /// Stops the session.
    pub async fn stop(&self) {
        let _ = self.commands.send(Command::Stop).await;
    }
}

/// A registered matcher, waiting for the message it is waiting for.
///
/// Created by [`Session::begin_request`]. Registering the matcher before the request is
/// sent is what stops a fast answer from being lost.
#[derive(Debug)]
pub struct RequestHandle {
    wait: oneshot::Receiver<Result<ReceivedMessage, LinkError>>,
}

impl RequestHandle {
    /// Waits for the message.
    ///
    /// # Errors
    ///
    /// Returns an error when the session closes before the message arrives, or when the
    /// timeout expires.
    pub async fn wait(self, timeout: Duration) -> Result<ReceivedMessage, LinkError> {
        match tokio::time::timeout(timeout, self.wait).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(LinkError::SessionClosed),
            Err(_) => Err(LinkError::Timeout(timeout)),
        }
    }
}

/// A registered collector, waiting for the messages it is collecting.
///
/// Created by [`Session::begin_collect`]. Registering the collector before the request is
/// sent is what stops a fast answer from being lost.
#[derive(Debug)]
pub struct Collector {
    wait: oneshot::Receiver<Result<Vec<ReceivedMessage>, LinkError>>,
}

impl Collector {
    /// Waits for the collection to complete.
    ///
    /// # Errors
    ///
    /// Returns an error when the session closes before the collection is complete, or when
    /// the timeout expires.
    pub async fn wait(self, timeout: Duration) -> Result<Vec<ReceivedMessage>, LinkError> {
        match tokio::time::timeout(timeout, self.wait).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(LinkError::SessionClosed),
            Err(_) => Err(LinkError::Timeout(timeout)),
        }
    }
}

/// The sequence number of the next outgoing frame.
///
/// The sequence number is per system and component, and increments for every frame sent.
fn next_sequence() -> u8 {
    use std::sync::atomic::{AtomicU8, Ordering};
    static SEQUENCE: AtomicU8 = AtomicU8::new(0);
    SEQUENCE.fetch_add(1, Ordering::Relaxed)
}

/// A pending request waiting for a matching message.
struct PendingRequest {
    matcher: Matcher,
    respond: oneshot::Sender<Result<ReceivedMessage, LinkError>>,
}

/// A pending request that collects messages until it is complete.
struct PendingCollection {
    matcher: Matcher,
    complete: Completion,
    messages: Vec<ReceivedMessage>,
    respond: oneshot::Sender<Result<Vec<ReceivedMessage>, LinkError>>,
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

    let mut context = SessionContext::default();
    let mut pending: Vec<PendingRequest> = Vec::new();
    let mut collections: Vec<PendingCollection> = Vec::new();

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
                Command::Collect {
                    matcher,
                    complete,
                    respond,
                } => {
                    collections.push(PendingCollection {
                        matcher,
                        complete,
                        messages: Vec::new(),
                        respond,
                    });
                }
                Command::Stop => {
                    stopped.store(true, Ordering::Relaxed);
                    let _ = state_tx.send(context.snapshot(false));
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
                    context.plaintext.observe(&discarded);
                }

                stopped.store(true, Ordering::Relaxed);
                let _ = state_tx.send(context.snapshot(false));
                return;
            }
            Ok(Ok(count)) => {
                let now = Instant::now();

                frames.clear();
                if parser.feed(&buffer[..count], &mut frames).is_ok() {
                    for frame in &frames {
                        context.handle_frame(frame, now);
                    }
                }

                // The bytes that were not part of a frame are the board's own output. They
                // are collected after the feed, because that is when the parser knows which
                // bytes they were.
                let discarded = parser.take_plaintext();
                if !discarded.is_empty() {
                    context.plaintext.observe(&discarded);
                }

                // Deliver any message that satisfies a pending request.
                if !pending.is_empty() {
                    deliver(&mut pending, &frames);
                }
                if !collections.is_empty() {
                    deliver_collections(&mut collections, &frames);
                }

                let _ = state_tx.send(context.snapshot(true));
            }
            Ok(Err(error)) => {
                // A UDP socket reports the previous datagram's "port unreachable" as a
                // connection reset on the next read. That is a transient condition, not a
                // broken link: the vehicle may simply not be listening yet.
                if error.kind() == std::io::ErrorKind::ConnectionReset {
                    tracing::debug!("the peer is unreachable at the moment: {error}");
                    continue;
                }

                tracing::warn!("the link failed: {error}");
                stopped.store(true, Ordering::Relaxed);
                let _ = state_tx.send(context.snapshot(false));
                return;
            }
        }
    }
}

/// Delivers the messages that satisfy a pending request.
fn deliver(pending: &mut Vec<PendingRequest>, frames: &[Frame]) {
    let mut remaining = Vec::with_capacity(pending.len());

    for request in pending.drain(..) {
        let mut matched = None;

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

/// Delivers the messages that satisfy a pending collection.
fn deliver_collections(collections: &mut Vec<PendingCollection>, frames: &[Frame]) {
    let mut remaining = Vec::with_capacity(collections.len());

    for mut collection in collections.drain(..) {
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

            if (collection.matcher)(&received) {
                collection.messages.push(received);
            }
        }

        if (collection.complete)(&collection.messages) {
            let _ = collection.respond.send(Ok(collection.messages));
        } else {
            remaining.push(collection);
        }
    }

    *collections = remaining;
}

/// The MAVLink version a frame used, for the frame parser.
#[must_use]
pub fn frame_version(frame: &Frame) -> MavlinkVersion {
    frame.version
}
