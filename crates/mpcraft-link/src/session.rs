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

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use mpcraft_mavlink::dialect::Dialect;
use mpcraft_mavlink::frame::{MavHeader, SIGNATURE_LEN};
use mpcraft_mavlink::signing::{SigningKey, SigningKeyring, TimestampSource};
use mpcraft_mavlink::{Frame, FrameParser, MavMessage, MavType, MavlinkVersion, Message};
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

/// How many messages the session keeps.
///
/// A vehicle that talks a great deal must not grow the state without limit, and the
/// console only ever shows the recent past.
const MAX_MESSAGES: usize = 200;

/// The heartbeat that says this end is a ground station.
///
/// The ground station identifies itself with the system and component ids that Mission
/// Planner uses, because the vehicle checks that the items it receives are addressed to
/// it.
fn ground_station_heartbeat() -> MavMessage {
    use mpcraft_mavlink::dialect::messages::HEARTBEAT_DATA;
    use mpcraft_mavlink::{MavAutopilot, MavModeFlag, MavState, MavType};

    MavMessage::HEARTBEAT(HEARTBEAT_DATA {
        custom_mode: 0,
        mavtype: MavType::MAV_TYPE_GCS,
        autopilot: MavAutopilot::MAV_AUTOPILOT_GENERIC,
        base_mode: MavModeFlag::empty(),
        system_status: MavState::MAV_STATE_ACTIVE,
        mavlink_version: 3,
    })
}

/// How often the ground station says that it is there.
///
/// A vehicle that only streams telemetry sends its traffic to the last ground station it
/// heard from, so a ground station that announced itself once and then went silent could
/// never pick up a vehicle that came up later, and could not recover a link that had been
/// half open. Upstream sends its heartbeat regularly for the same reason.
const ANNOUNCE_INTERVAL: Duration = Duration::from_secs(1);

/// The tunables of a session.
#[derive(Debug, Clone)]
pub struct SessionOptions {
    /// How long the reader waits for data before checking for queued work.
    ///
    /// This bounds the delay between a command being issued and it reaching the wire.
    pub poll_interval: Duration,
    /// How long to wait for a vehicle to answer during the handshake.
    pub connect_timeout: Duration,
    /// How long a link that has heard the vehicle may stay quiet.
    ///
    /// A port stays open when the board stops answering — a board that was restarted, or one
    /// that left MAVLink for its bootloader, keeps the port the operating system gave it — so
    /// without this the reader waits for a vehicle that is no longer there, and the station goes
    /// on saying it is connected. It is only counted once a vehicle has been heard: a station
    /// listening on a UDP port for a vehicle that has not arrived yet has not gone quiet, it has
    /// not started. `Duration::ZERO` turns the check off.
    pub silence_timeout: Duration,
    /// The keys of the link, when the link is signed.
    pub signing: Option<SigningSetup>,
}

/// The keys of a link.
///
/// A session that has a key signs every frame it sends, and every frame it receives is
/// verified against the keys it knows. A link with no keys is an ordinary link.
#[derive(Debug, Clone, Default)]
pub struct SigningSetup {
    /// The key the outgoing frames are signed with.
    pub key: Option<SigningKey>,
    /// The keys the incoming frames are verified against.
    pub keyring: SigningKeyring,
    /// The id of this station on the link.
    pub link_id: u8,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(10),
            connect_timeout: Duration::from_secs(30),
            // Mission Planner treats five seconds without a heartbeat as a link that has gone.
            // Six leaves room for a slow one without making the operator wait.
            silence_timeout: Duration::from_secs(6),
            signing: None,
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

/// A frame whose message the compiled dialect does not carry.
///
/// The vehicle speaks more than the bindings know: the file transfer is the oldest
/// example. Such a frame cannot be decoded into a [`Dialect`], so it is handed over with
/// its payload as it came, and the module that speaks the message reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawMessage {
    /// The system id of the sender.
    pub system_id: u8,
    /// The component id of the sender.
    pub component_id: u8,
    /// The message id.
    pub message_id: u32,
    /// The payload, as it arrived: MAVLink 2 trims the zeros at its end, so a fixed-size
    /// message is shorter than its definition says and the reader pads it.
    pub payload: Vec<u8>,
}

impl RawMessage {
    /// The payload, padded with zeros to the length the definition gives it.
    #[must_use]
    pub fn padded(&self, length: usize) -> Vec<u8> {
        let mut payload = self.payload.clone();
        payload.resize(length.max(self.payload.len()), 0);
        payload
    }
}

/// A predicate that decides whether a raw message matches a request.
type RawMatcher = Box<dyn Fn(&RawMessage) -> bool + Send>;

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
    /// Wait for a frame whose message the dialect does not carry.
    RequestRaw {
        /// The id of the message that is waited for.
        message_id: u32,
        /// The predicate the payload must satisfy.
        matcher: RawMatcher,
        /// Where to deliver the matching frame.
        respond: oneshot::Sender<Result<RawMessage, LinkError>>,
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
    /// Chooses which vehicle the station speaks to.
    Select {
        /// The board that was chosen.
        vehicle: VehicleId,
        /// Where to report the outcome.
        respond: oneshot::Sender<Result<(), LinkError>>,
    },
    /// Receives every chunk of bytes the reader takes off the transport.
    ///
    /// A mirror is a byte-level copy, so what it receives is the stream as it arrived rather than
    /// the frames after parsing: the other program gets the same bytes the vehicle sent.
    Mirror {
        /// Where the chunks are delivered.
        respond: oneshot::Sender<mpsc::Receiver<Vec<u8>>>,
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
    /// The telemetry of each vehicle seen on the link, keyed by the pair that identifies it.
    ///
    /// `telemetry` is the primary vehicle's snapshot; this holds every vehicle's own state,
    /// so a formation or a companion computer can be read without mixing it into the
    /// primary's.
    pub telemetries: HashMap<VehicleId, mpcraft_vehicle::Telemetry>,
    /// The vehicle the telemetry above belongs to, once the link has selected one.
    ///
    /// Before a selection the snapshot is the mixed view of whatever arrived; after it, the
    /// snapshot is this vehicle's own state. The interface needs to know which of the two it
    /// is reading.
    pub selected: Option<VehicleId>,
    /// The loss statistics of the primary vehicle.
    pub loss: LossStats,
    /// The protocol capabilities the vehicle advertised.
    pub capabilities: Option<u64>,
    /// The parameters the vehicle has reported.
    pub parameters: ParameterStore,
    /// What the board printed on the wire outside of MAVLink frames.
    pub output: crate::plaintext::BoardOutput,
    /// The messages the vehicle has sent, most recent last.
    ///
    /// The list is bounded: a vehicle that talks a great deal must not grow the state
    /// without limit.
    pub messages: Vec<mpcraft_vehicle::VehicleMessage>,
    /// The traffic that the vehicle has heard.
    pub adsb: Vec<crate::adsb::AdsbVehicle>,
    /// The gimbals the vehicle has reported.
    pub gimbals: Vec<crate::gimbal::GimbalDevice>,
    /// The cameras the vehicle has reported.
    pub cameras: Vec<crate::camera::CameraDevice>,
    /// The nodes of the DroneCAN bus the vehicle has reported.
    pub dronecan: Vec<crate::dronecan::DronecanNode>,
    /// The request of the terrain the vehicle has made, when it has made one.
    pub terrain_request: Option<crate::terrain::TerrainRequest>,
    /// Whether the link is still open.
    pub connected: bool,
}

/// The time the protocol counts the marks of time of the signatures in.
///
/// It is the time since the first of January of 2015, in hundredths of a microsecond.
fn signature_now() -> u64 {
    /// The first of January of 2015, in seconds since the epoch.
    const EPOCH_2015: u64 = 1_420_070_400;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    now.as_secs().saturating_sub(EPOCH_2015) * 10_000 + u64::from(now.subsec_micros()) / 100
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
    /// The traffic that the vehicle has heard.
    adsb: crate::adsb::AdsbRegistry,
    /// The gimbals the vehicle has reported.
    gimbals: crate::gimbal::GimbalRegistry,
    /// The cameras the vehicle has reported.
    cameras: crate::camera::CameraRegistry,
    /// The nodes of the DroneCAN bus the vehicle has reported.
    dronecan: crate::dronecan::DronecanRegistry,
    /// The request of the terrain the vehicle has made.
    terrain_request: Option<crate::terrain::TerrainRequest>,
    accountants: HashMap<VehicleId, LossAccountant>,
    last_packet_at: Option<Instant>,
    loss: LossStats,
    capabilities: Option<u64>,
    parameters: ParameterStore,
    messages: VecDeque<mpcraft_vehicle::VehicleMessage>,
    /// The keys the incoming frames are verified against, when the link is signed.
    keyring: SigningKeyring,
}

impl SessionContext {
    /// Whether a frame may be believed.
    ///
    /// On a link that is not signed, every frame is believed. On a link that is signed,
    /// a frame has to carry a signature that one of the keys accounts for: a frame that
    /// does not is not from the vehicle.
    fn verify(&mut self, frame: &Frame) -> bool {
        if self.keyring.is_empty() {
            return true;
        }

        let Some(signature) = frame.signature else {
            tracing::warn!("a frame arrived without a signature");
            return false;
        };

        // The signature is over the frame without its own block.
        let signed = &frame.raw[..frame.raw.len().saturating_sub(SIGNATURE_LEN)];

        match self.keyring.verify(
            signed,
            signature.link_id,
            signature.timestamp,
            &signature.value,
        ) {
            Ok(name) => {
                tracing::debug!("the frame was signed with the key {name}");
                true
            }
            Err(error) => {
                tracing::warn!("the frame was not signed properly: {error}");
                false
            }
        }
    }

    /// Handles one frame: updates the vehicle state and the telemetry.
    fn handle_frame(&mut self, frame: &Frame, now: Instant) {
        let id = VehicleId::new(frame.system_id, frame.component_id);

        // The loss accounting is per vehicle, and follows the sequence number.
        let since_previous = self
            .last_packet_at
            .is_some_and(|last| now.saturating_duration_since(last).as_secs_f64() < 1.0);
        let accountant = self.accountants.entry(id).or_default();
        accountant.observe(frame.sequence, since_previous);
        self.loss = accountant.stats();
        self.last_packet_at = Some(now);

        // The two messages of a DroneCAN bus are read from the bytes, not through the dialect. The
        // compiled dialect does carry them — it is generated from the canonical XML at build time —
        // but it turns their health and mode into enumerations and *refuses* a number the dictionary
        // does not give, so a node reporting a value outside the dictionary would never reach the
        // sheet. Upstream reads the same payload as bytes (`ToStructure`), and the order used here is
        // the order the seed is worked out over, which pymavlink's own generator agrees on (28 and 95).
        if frame.message_id == crate::dronecan::NODE_STATUS_ID {
            let status = crate::dronecan::NodeStatus::decode(&frame.payload);
            self.dronecan.observe_status(id, &status);
            return;
        }

        if frame.message_id == crate::dronecan::NODE_INFO_ID {
            let info = crate::dronecan::NodeInfo::decode(&frame.payload);
            self.dronecan.observe_info(id, &info);
            return;
        }

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

            // A ground station on the wire is not a vehicle. A second station, or the router that
            // joins several simulated vehicles into one link, announces itself as one, and counting
            // it says the link carries more boards than it does. The selection already skips a
            // station; the registry did not, so the window listed it as a vehicle with no state.
            if mav_type == Some(MavType::MAV_TYPE_GCS) {
                return;
            }

            let _ = self.heartbeats.observe(Heartbeat {
                system_id: frame.system_id,
                component_id: frame.component_id,
                message_id: frame.message_id,
                mav_type,
            });

            // The selection is made **once**. A second vehicle can satisfy the rule as well as
            // the first — an autopilot seen twice on component one — but the link keeps the
            // vehicle it chose, which is what upstream does with `MAVList.GetID`: the choice
            // does not move because another board happened to talk more. Before the selection
            // every packet reads into the mixed view; from here on the view belongs to the
            // primary vehicle alone, and what another board sends stays in its own slot.
            let chosen = self.registry.primary().map(|primary| primary.id);

            let vehicle = self.registry.observe(id);

            if let MavMessage::HEARTBEAT(heartbeat) = &message {
                vehicle.mav_type = Some(heartbeat.mavtype);
                vehicle.autopilot = Some(heartbeat.autopilot);
                vehicle.kind = Some(VehicleKind::from_heartbeat(
                    heartbeat.autopilot,
                    heartbeat.mavtype,
                ));
                vehicle.custom_mode = Some(heartbeat.custom_mode);
            }

            if chosen.is_none_or(|primary| primary == id) && !vehicle.primary {
                let selected = self
                    .heartbeats
                    .select()
                    .map(|heartbeat| VehicleId::new(heartbeat.system_id, heartbeat.component_id));

                if selected == Some(id) {
                    vehicle.primary = true;
                    self.aggregator.set_primary(id);
                }
            }
        }

        // The traffic the vehicle has heard is kept: it is drawn on the map.
        if let MavMessage::ADSB_VEHICLE(traffic) = &message {
            if traffic
                .flags
                .contains(mpcraft_mavlink::dialect::AdsbFlags::ADSB_FLAGS_VALID_COORDS)
            {
                let callsign: Vec<u8> = traffic.callsign.iter().copied().collect();

                self.adsb.observe(
                    crate::adsb::AdsbVehicle {
                        icao_address: traffic.ICAO_address,
                        callsign: crate::adsb::callsign_of(&callsign),
                        squawk: traffic.squawk,
                        latitude: f64::from(traffic.lat) / 1e7,
                        longitude: f64::from(traffic.lon) / 1e7,
                        altitude: f64::from(traffic.altitude) / 1000.0,
                        heading: f64::from(traffic.heading) / 100.0,
                        speed: f64::from(traffic.hor_velocity) / 100.0,
                        vertical_speed: f64::from(traffic.ver_velocity) / 100.0,
                        // The vehicle sent it, so the relay does not send it back.
                        source: crate::adsb::AdsbSource::Vehicle,
                    },
                    now,
                );
            }
        }

        // The gimbals and the cameras are kept because the operator controls them: what a
        // device can do decides whether a control is offered, and where it points or what it
        // is capturing is what the sheet shows.
        match &message {
            MavMessage::GIMBAL_MANAGER_INFORMATION(information) => {
                self.gimbals
                    .observe_information(frame.component_id, information);
            }
            MavMessage::GIMBAL_MANAGER_STATUS(status) => {
                self.gimbals.observe_status(status);
            }
            MavMessage::GIMBAL_DEVICE_ATTITUDE_STATUS(attitude) => {
                self.gimbals.observe_attitude(attitude);
            }
            MavMessage::CAMERA_INFORMATION(information) => {
                self.cameras
                    .observe_information(frame.component_id, information);
            }
            // The number of pictures a camera has taken is a MAVLink v2 extension field, and
            // the bindings this project compiles do not carry it, so only the mode and the
            // state of the captures are kept — both read from the component that sent them,
            // because these messages name no target.
            MavMessage::CAMERA_SETTINGS(settings) => {
                self.cameras.observe_settings(frame.component_id, settings);
            }
            MavMessage::CAMERA_CAPTURE_STATUS(status) => {
                self.cameras
                    .observe_capture_status(frame.component_id, status);
            }
            _ => {}
        }

        // The vehicle asks for the terrain of the ground it is over.
        if let MavMessage::TERRAIN_REQUEST(request) = &message {
            self.terrain_request = Some(crate::terrain::TerrainRequest {
                latitude: f64::from(request.lat) / 1e7,
                longitude: f64::from(request.lon) / 1e7,
                grid_spacing: request.grid_spacing,
                mask: request.mask,
            });
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

        self.aggregator.apply(&message, id);

        // The messages the vehicle sends are what it says about itself: a pre-arm
        // failure, a mode it refused, a waypoint it reached. They are the first thing an
        // operator wants to read.
        if let MavMessage::STATUSTEXT(text) = &message {
            let line = text.text.to_str().unwrap_or_default().trim();

            if !line.is_empty() {
                self.messages.push_back(mpcraft_vehicle::VehicleMessage {
                    severity: text.severity as u8,
                    text: line.to_owned(),
                    id: text.id,
                });

                while self.messages.len() > MAX_MESSAGES {
                    self.messages.pop_front();
                }
            }
        }
    }

    /// The snapshot of the current state.
    fn snapshot(&self, connected: bool) -> SessionState {
        SessionState {
            vehicles: self.registry.vehicles().into_iter().cloned().collect(),
            telemetry: self.aggregator.telemetry().clone(),
            telemetries: self.aggregator.telemetries(),
            selected: self.aggregator.selected(),
            loss: self.loss,
            capabilities: self.capabilities,
            parameters: self.parameters.clone(),
            output: self.plaintext.output(),
            messages: self.messages.iter().cloned().collect(),
            adsb: self.adsb.vehicles(),
            gimbals: self.gimbals.devices(),
            cameras: self.cameras.cameras(),
            dronecan: self.dronecan.nodes(),
            terrain_request: self.terrain_request,
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
            telemetries: HashMap::new(),
            selected: None,
            loss: LossStats::default(),
            capabilities: None,
            parameters: mpcraft_params::ParameterStore::new(),
            output: crate::plaintext::BoardOutput::default(),
            messages: Vec::new(),
            adsb: Vec::new(),
            gimbals: Vec::new(),
            cameras: Vec::new(),
            dronecan: Vec::new(),
            terrain_request: None,
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
    /// The marks of time of the signatures, which have to increase.
    timestamps: std::sync::Mutex<TimestampSource>,
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
        if let Err(error) = self.write_message(&ground_station_heartbeat()).await {
            tracing::warn!("could not announce the ground station: {error}");
        }
    }

    /// Opens a session on an already-open transport.
    #[must_use]
    pub fn with_transport(transport: Box<dyn Transport>, options: SessionOptions) -> Self {
        let (commands, receiver) = mpsc::channel::<Command>(64);

        // The reader verifies the frames that arrive, so it needs the keys of the link.
        let mut context = SessionContext::default();
        if let Some(signing) = &options.signing {
            context.keyring = signing.keyring.clone();
        }

        let (state_tx, state_rx) = watch::channel(context.snapshot(true));

        let stopped = Arc::new(AtomicBool::new(false));
        let task_stopped = Arc::clone(&stopped);

        let reader_options = options.clone();

        tokio::spawn(async move {
            run_reader(transport, receiver, state_tx, reader_options, task_stopped).await;
        });

        Self {
            commands,
            state: state_rx,
            options,
            stopped,
            timestamps: std::sync::Mutex::new(TimestampSource::new()),
        }
    }

    /// The current state of the session.
    #[must_use]
    pub fn state(&self) -> SessionState {
        self.state.borrow().clone()
    }

    /// Waits until a vehicle has identified itself.
    ///
    /// Some of the protocol depends on what the vehicle says about itself: the encoding
    /// of its parameter values, for one, follows the autopilot it reports. A ground
    /// station that reads a parameter before the heartbeat has arrived guesses the
    /// encoding, and the guess reads the wrong numbers.
    ///
    /// Waiting for the vehicle to *appear* is not enough: it appears on the first frame
    /// it sends, whatever that frame is, and a vehicle that sends telemetry before its
    /// heartbeat is still unidentified at that point.
    ///
    /// The wait is bounded by the timeout, and the timeout also covers the operation that
    /// follows, so an operation takes at most twice as long as the timeout it was given.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle does not identify itself within the timeout.
    pub async fn wait_for_vehicle(
        &self,
        vehicle: VehicleId,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let deadline = Instant::now() + timeout;

        loop {
            // The autopilot is what the heartbeat carries, and it is what the parameter
            // encoding depends on.
            let identified = self
                .state
                .borrow()
                .vehicles
                .iter()
                .any(|seen| seen.id == vehicle && seen.autopilot.is_some());

            if identified {
                return Ok(());
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(LinkError::Timeout(timeout));
            }

            tokio::time::sleep(remaining.min(Duration::from_millis(20))).await;
        }
    }

    /// Waits until a vehicle has said what it is.
    ///
    /// The wait is for *any* vehicle: it is what the automatic rate needs, because the
    /// rate is chosen before anything is known about the vehicle on the other end.
    ///
    /// # Errors
    ///
    /// Returns an error when no vehicle identifies itself within the timeout.
    pub async fn wait_for_any_vehicle(&self, timeout: Duration) -> Result<VehicleId, LinkError> {
        let deadline = Instant::now() + timeout;

        loop {
            let identified = self
                .state
                .borrow()
                .vehicles
                .iter()
                .find(|seen| seen.autopilot.is_some())
                .map(|seen| seen.id);

            if let Some(id) = identified {
                return Ok(id);
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(LinkError::Timeout(timeout));
            }

            tokio::time::sleep(remaining.min(Duration::from_millis(20))).await;
        }
    }

    /// The tunables this session was opened with.
    #[must_use]
    pub fn options(&self) -> SessionOptions {
        self.options.clone()
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

    /// Chooses which vehicle the station speaks to.
    ///
    /// The choice is made from the boards the link has seen. A pair that has never spoken cannot be
    /// chosen, because there is no state behind it — the numbers say an address, not a vehicle.
    ///
    /// # Errors
    ///
    /// Returns `UnknownVehicle` for a board that was never seen, and `SessionClosed` when the link
    /// is not open.
    pub async fn select_vehicle(&self, vehicle: VehicleId) -> Result<(), LinkError> {
        let (respond, wait) = oneshot::channel();
        self.commands
            .send(Command::Select { vehicle, respond })
            .await
            .map_err(|_| LinkError::SessionClosed)?;

        wait.await.map_err(|_| LinkError::SessionClosed)?
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

    /// Receives every chunk of bytes the reader takes off the transport.
    ///
    /// This is what a mirror copies: the bytes as they arrived, before any frame is parsed. The
    /// receiver is closed when the session stops, so a task that reads it ends with the link.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is closed.
    pub async fn mirror_bytes(&self) -> Result<mpsc::Receiver<Vec<u8>>, LinkError> {
        let (respond, wait) = oneshot::channel();

        self.commands
            .send(Command::Mirror { respond })
            .await
            .map_err(|_| LinkError::SessionClosed)?;

        wait.await.map_err(|_| LinkError::SessionClosed)
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

        let bytes = match &self.options.signing {
            Some(signing) => match signing.key.as_ref() {
                Some(key) => {
                    // The mark of time of the signature has to increase on every frame.
                    let timestamp = {
                        let mut source = self.timestamps.lock().map_err(|_| {
                            LinkError::Protocol(mpcraft_mavlink::ProtocolError::Malformed(
                                "the marks of time of the signatures are locked",
                            ))
                        })?;
                        source.next(signature_now())
                    };

                    mpcraft_mavlink::frame::encode_v2_signed(
                        message,
                        header,
                        &signing.keyring,
                        key,
                        signing.link_id,
                        timestamp,
                    )
                }
                // The link has keys to verify with, but none to sign with.
                None => mpcraft_mavlink::frame::encode_v2(message, header),
            },
            None => mpcraft_mavlink::frame::encode_v2(message, header),
        };

        self.write(bytes).await
    }

    /// Encodes a message the dialect does not carry, signing it when the link is signed.
    fn encode_raw(
        &self,
        message_id: u32,
        payload: &[u8],
        header: MavHeader,
    ) -> Result<Vec<u8>, LinkError> {
        // A frame whose seed is unknown would be one the vehicle refuses; saying so here is
        // better than a silent drop on the other end.
        if mpcraft_mavlink::dialect::extra_crc(message_id).is_none() {
            return Err(LinkError::Protocol(
                mpcraft_mavlink::ProtocolError::Malformed(
                    "the message has no checksum seed and cannot be sent",
                ),
            ));
        }

        let bytes = match &self.options.signing {
            Some(signing) => match signing.key.as_ref() {
                Some(key) => {
                    // The mark of time of the signature has to increase on every frame.
                    let timestamp = {
                        let mut source = self.timestamps.lock().map_err(|_| {
                            LinkError::Protocol(mpcraft_mavlink::ProtocolError::Malformed(
                                "the marks of time of the signatures are locked",
                            ))
                        })?;
                        source.next(signature_now())
                    };

                    mpcraft_mavlink::frame::encode_v2_bytes_signed(
                        message_id,
                        payload,
                        header,
                        &signing.keyring,
                        key,
                        signing.link_id,
                        timestamp,
                    )
                }
                None => mpcraft_mavlink::frame::encode_v2_bytes(message_id, payload, header),
            },
            None => mpcraft_mavlink::frame::encode_v2_bytes(message_id, payload, header),
        };

        Ok(bytes)
    }

    /// Writes a message the dialect does not carry.
    ///
    /// The payload is written as it is, with the zeros at its end trimmed the way MAVLink 2
    /// trims them, and the checksum is the one of the message's own seed.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is closed, the write fails, or the seed of the
    /// message is not known.
    pub async fn write_raw(&self, message_id: u32, payload: &[u8]) -> Result<(), LinkError> {
        let header = MavHeader {
            system_id: GCS_SYSTEM_ID,
            component_id: GCS_COMPONENT_ID,
            sequence: next_sequence(),
        };

        let bytes = self.encode_raw(message_id, payload, header)?;

        self.write(bytes).await
    }

    /// Registers a matcher for a frame whose message the dialect does not carry.
    ///
    /// The matcher is registered *before* the request that triggers it is sent: a vehicle on
    /// a fast link can answer before the request has even left the session.
    ///
    /// # Errors
    ///
    /// Returns an error when the session has closed.
    pub async fn begin_raw<F>(&self, message_id: u32, matcher: F) -> Result<RawHandle, LinkError>
    where
        F: Fn(&RawMessage) -> bool + Send + 'static,
    {
        let (respond, wait) = oneshot::channel();
        self.commands
            .send(Command::RequestRaw {
                message_id,
                matcher: Box::new(matcher),
                respond,
            })
            .await
            .map_err(|_| LinkError::SessionClosed)?;

        Ok(RawHandle { wait })
    }

    /// Sends a message the dialect does not carry and waits for the frame that answers it.
    ///
    /// Like [`Session::exchange`], the answer is waited for on a matcher registered before
    /// the write, and the payload is written a second time when the first goes unanswered
    /// by the middle of the timeout.
    ///
    /// # Errors
    ///
    /// Returns an error when the session closes, the write fails, or the timeout expires.
    pub(crate) async fn exchange_raw<F>(
        &self,
        timeout: Duration,
        message_id: u32,
        payload: &[u8],
        matcher: F,
    ) -> Result<RawMessage, LinkError>
    where
        F: Fn(&RawMessage) -> bool + Send + 'static,
    {
        let mut handle = self.begin_raw(message_id, matcher).await?;

        let start = Instant::now();

        let mut attempt = 0u8;
        loop {
            self.write_raw(message_id, payload).await?;
            attempt += 1;

            // The first attempt gets half of the budget, the retry the other half.
            let share = if attempt == 1 { timeout / 2 } else { timeout };
            let budget = share.saturating_sub(start.elapsed());

            match handle.wait_for(budget).await {
                Ok(message) => return Ok(message),
                Err(LinkError::Timeout(_)) if attempt < 2 => continue,
                Err(error) => return Err(error),
            }
        }
    }

    /// Sends a message and waits for the answer that completes the exchange.
    ///
    /// The matcher is registered before the message is written, because a vehicle on a
    /// fast link can answer before the request has even left the session.
    ///
    /// The message is written once. If the vehicle has not answered by the middle of the
    /// timeout it is written a second time: a lost frame is the common failure on a noisy
    /// link, and one repeat is enough to tell a silent vehicle from a lost answer. The
    /// repeat carries a higher `confirmation` number where the message has such a field,
    /// so the vehicle can tell it apart from the first attempt.
    ///
    /// # Errors
    ///
    /// Returns an error when the session closes or the timeout expires.
    pub(crate) async fn exchange(
        &self,
        timeout: Duration,
        matcher: impl Fn(&ReceivedMessage) -> bool + Send + 'static,
        complete: impl Fn(&[ReceivedMessage]) -> bool + Send + 'static,
        message: impl Fn(u8) -> MavMessage,
    ) -> Result<Vec<ReceivedMessage>, LinkError> {
        let mut collector = self.begin_collect(matcher, complete).await?;

        // The budget is measured from the start of the exchange rather than turned into a
        // deadline: `Instant + Duration` panics on an absurd duration, and a ground
        // station that passes one must not take the process down with it.
        let start = Instant::now();

        let mut attempt = 0u8;
        loop {
            self.write_message(&message(attempt)).await?;
            attempt += 1;

            // The first attempt gets half of the budget, the retry the other half.
            let share = if attempt == 1 { timeout / 2 } else { timeout };
            let budget = share.saturating_sub(start.elapsed());

            match collector.wait_for(budget).await {
                Ok(messages) => return Ok(messages),
                Err(LinkError::Timeout(_)) if attempt < 2 => continue,
                Err(error) => return Err(error),
            }
        }
    }

    /// Stops the session.
    pub async fn stop(&self) {
        let _ = self.commands.send(Command::Stop).await;
    }
}

/// A registered matcher, waiting for a frame whose message the dialect does not carry.
///
/// Created by [`Session::begin_raw`]. Registering the matcher before the request is sent
/// is what stops a fast answer from being lost.
#[derive(Debug)]
pub struct RawHandle {
    wait: oneshot::Receiver<Result<RawMessage, LinkError>>,
}

impl RawHandle {
    /// Waits for the message.
    ///
    /// # Errors
    ///
    /// Returns an error when the session closes before the message arrives, or when the
    /// timeout expires.
    pub async fn wait(mut self, timeout: Duration) -> Result<RawMessage, LinkError> {
        self.wait_for(timeout).await
    }

    /// Waits for the message, for at most the given budget.
    ///
    /// The handle is **not** consumed, so a transaction that re-sends its request can keep
    /// waiting on the same matcher.
    ///
    /// # Errors
    ///
    /// Returns an error when the session closes before the message arrives, or when the
    /// budget runs out.
    pub async fn wait_for(&mut self, budget: Duration) -> Result<RawMessage, LinkError> {
        match tokio::time::timeout(budget, &mut self.wait).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(LinkError::SessionClosed),
            Err(_) => Err(LinkError::Timeout(budget)),
        }
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
    pub async fn wait(mut self, timeout: Duration) -> Result<ReceivedMessage, LinkError> {
        self.wait_for(timeout).await
    }

    /// Waits for the message, for at most the given budget.
    ///
    /// The handle is **not** consumed, so a transaction that re-sends its request can
    /// keep waiting on the same matcher. Registering a fresh matcher for the retry
    /// would leave a window in which the vehicle's answer arrives with nobody
    /// listening — the very race the matcher exists to prevent.
    ///
    /// # Errors
    ///
    /// Returns an error when the session closes before the message arrives, or when
    /// the budget runs out.
    pub async fn wait_for(&mut self, budget: Duration) -> Result<ReceivedMessage, LinkError> {
        match tokio::time::timeout(budget, &mut self.wait).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(LinkError::SessionClosed),
            Err(_) => Err(LinkError::Timeout(budget)),
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
    pub async fn wait(mut self, timeout: Duration) -> Result<Vec<ReceivedMessage>, LinkError> {
        self.wait_for(timeout).await
    }

    /// Waits for the collection to complete, for at most the given budget.
    ///
    /// The collector is **not** consumed, so a transaction that re-sends its request can
    /// keep waiting on the same matcher. Registering a fresh matcher for the retry would
    /// leave a window in which the vehicle's answer arrives with nobody listening — the
    /// very race the collector exists to prevent.
    ///
    /// # Errors
    ///
    /// Returns an error when the session closes before the collection is complete, or when
    /// the budget runs out.
    pub async fn wait_for(&mut self, budget: Duration) -> Result<Vec<ReceivedMessage>, LinkError> {
        match tokio::time::timeout(budget, &mut self.wait).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(LinkError::SessionClosed),
            Err(_) => Err(LinkError::Timeout(budget)),
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

/// A pending request waiting for a frame whose message the dialect does not carry.
struct PendingRaw {
    message_id: u32,
    matcher: RawMatcher,
    respond: oneshot::Sender<Result<RawMessage, LinkError>>,
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
    let mut pending_raw: Vec<PendingRaw> = Vec::new();
    let mut collections: Vec<PendingCollection> = Vec::new();
    let mut mirror_sinks: Vec<mpsc::Sender<Vec<u8>>> = Vec::new();

    let mut last_announce = Instant::now();

    loop {
        // 0. Say that the ground station is there, every now and then.
        if last_announce.elapsed() >= ANNOUNCE_INTERVAL {
            last_announce = Instant::now();

            let header = MavHeader {
                system_id: GCS_SYSTEM_ID,
                component_id: GCS_COMPONENT_ID,
                sequence: next_sequence(),
            };

            let bytes = mpcraft_mavlink::frame::encode_v2(&ground_station_heartbeat(), header);
            let _ = transport.write(&bytes).await;
        }

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
                Command::RequestRaw {
                    message_id,
                    matcher,
                    respond,
                } => {
                    pending_raw.push(PendingRaw {
                        message_id,
                        matcher,
                        respond,
                    });
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
                Command::Select { vehicle, respond } => {
                    if context.registry.choose(vehicle) {
                        // The view belongs to the chosen vehicle from here. `set_primary` keeps what
                        // had already been read as its starting point, so a board that has been
                        // reporting does not lose its state at the moment it becomes the one shown.
                        context.aggregator.set_primary(vehicle);
                        let _ = respond.send(Ok(()));
                        let _ = state_tx.send(context.snapshot(true));
                    } else {
                        let _ = respond.send(Err(LinkError::UnknownVehicle(vehicle)));
                    }
                }
                Command::Mirror { respond } => {
                    let (sink, receiver) = mpsc::channel(32);
                    mirror_sinks.push(sink);
                    let _ = respond.send(receiver);
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
            Err(_elapsed) => {
                // A link that has heard the vehicle and then gone quiet is not a link. The port
                // stays open because the operating system has no reason to close it, so a board
                // that was restarted — or one that left MAVLink for its bootloader — would leave
                // the station saying it is connected to a vehicle that is no longer speaking.
                if !options.silence_timeout.is_zero() {
                    if let Some(last) = context.last_packet_at {
                        if last.elapsed() >= options.silence_timeout {
                            tracing::warn!(
                                silent_for = ?last.elapsed(),
                                "the vehicle has gone quiet: the link is closed"
                            );
                            stopped.store(true, Ordering::Relaxed);
                            let _ = state_tx.send(context.snapshot(false));
                            return;
                        }
                    }
                }

                continue;
            }
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

                // The mirrors receive the stream as it arrived, before it is parsed: a program that
                // follows the link gets the same bytes the vehicle sent, not a re-encoding of the
                // frames. A mirror that has stopped reading is dropped rather than allowed to grow
                // without a bound; a slow one is allowed to fall behind by the channel's capacity.
                if !mirror_sinks.is_empty() {
                    let chunk = buffer[..count].to_vec();

                    mirror_sinks.retain(|sink| {
                        matches!(
                            sink.try_send(chunk.clone()),
                            Ok(()) | Err(mpsc::error::TrySendError::Full(_))
                        )
                    });
                }

                frames.clear();
                if parser.feed(&buffer[..count], &mut frames).is_ok() {
                    for frame in &frames {
                        if !context.verify(frame) {
                            continue;
                        }

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
                if !pending_raw.is_empty() {
                    deliver_raw(&mut pending_raw, &frames);
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
            // The caller has given up — it timed out, or the session is shutting down.
            // Keeping the matcher would leak it, and every message that arrives would be
            // tested against a predicate nobody is waiting for.
            None if request.respond.is_closed() => {}
            None => remaining.push(request),
        }
    }

    *pending = remaining;
}

/// Delivers frames whose messages the dialect does not carry.
fn deliver_raw(pending: &mut Vec<PendingRaw>, frames: &[Frame]) {
    let mut remaining = Vec::with_capacity(pending.len());

    for request in pending.drain(..) {
        let mut matched = None;

        for frame in frames {
            if frame.message_id != request.message_id {
                continue;
            }

            let raw = RawMessage {
                system_id: frame.system_id,
                component_id: frame.component_id,
                message_id: frame.message_id,
                payload: frame.payload.clone(),
            };

            if (request.matcher)(&raw) {
                matched = Some(raw);
                break;
            }
        }

        match matched {
            Some(raw) => {
                let _ = request.respond.send(Ok(raw));
            }
            None if request.respond.is_closed() => {}
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
        } else if collection.respond.is_closed() {
            // The caller has given up. See `deliver` for why this matters.
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
