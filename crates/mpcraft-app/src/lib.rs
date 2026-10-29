//! Application orchestration for MPCraft.
//!
//! This is the only crate that knows about the application as a whole: the session
//! supervisor, the event bus, the command surface, and the auto-reconnect policy.
//!
//! It is the only library crate permitted to depend on Tauri types — and even here, the Tauri
//! dependency lives in `apps/desktop`, so `mpcraft-app` stays headless and testable.
//!
//! # The IPC contract
//!
//! Every command returns `Result<T, CommandError>`, where [`CommandError`] carries a stable
//! snake_case `code`. The frontend branches on the code, never on the message text: the text
//! is for humans and may be localised or reworded.
//!
//! Telemetry is pushed as **one batched snapshot per tick**, never as one event per field.
//! At 20 Hz with forty fields, per-field events would mean eight hundred IPC messages per
//! second.
//!
//! # The link
//!
//! The [`SessionSupervisor`] owns the link to the vehicle. It opens it, publishes its state
//! to the interface, and — when the link drops — follows the [`ReconnectPolicy`]: how many
//! attempts, with what backoff, and how long a single attempt may take. The policy is data,
//! and the mechanism it drives is injected, so the whole thing is tested without a network
//! peer and without waiting.
//!
//! # The operations
//!
//! The things an operator asks of a vehicle — arm, take off, change the mode, transfer a
//! plan — live in [`operations`], on the supervisor. They live there rather than in the
//! shell so that the rule that a command needs an open link is written once, and tested
//! without a display server.
//!
//! # The settings
//!
//! The connection profiles live in [`mpcraft_settings`], in a TOML file the user can edit by
//! hand. [`AppState`] owns the store and the commands that read and write it.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use mpcraft_geo::terrain_service::TerrainService;
use mpcraft_geo::tiles::TileService;
use mpcraft_settings::{PluginEntry, SettingsStore};
use serde::{Deserialize, Serialize};

pub mod adsb_relay;
pub mod apj;
pub mod board;
pub mod bootloader;
pub mod checklist;
pub mod coordinates;
pub mod dfu;
pub mod diagnostics;
pub mod dronecan;
pub mod error;
pub mod firmware;
pub mod georef;
pub mod graphs;
pub mod http;
pub mod live_kml;
pub mod logs;
pub mod mirrors;
pub mod ntrip;
pub mod operations;
pub mod plugins;
pub mod scp;
pub mod secure;
pub mod signing;
pub mod streams;
pub mod supervisor;
pub mod swarm;
pub mod terrain;
pub mod transponder;
pub mod ulog;

pub use checklist::{ChecklistEntry, ChecklistItem, Condition, Source};
pub use coordinates::{Coordinates, Utm};
pub use error::{CommandError, CommandResult};
pub use graphs::{GraphHistory, PresetInfo, SeriesData, SeriesInfo, SeriesPoints};
pub use live_kml::LiveTrack;
pub use mpcraft_link::SessionState;
pub use mpcraft_settings::ConnectionProfile;
pub use operations::CommandAnswer;
pub use operations::OutputFunction;
pub use operations::StreamAnswer;
pub use operations::{flight_modes, ModeInfo, PlanInfo, PlanItem, PlanKind, VehicleLog};
pub use supervisor::{
    ConnectionStatus, Connector, LinkSnapshot, ReconnectPolicy, SessionSupervisor, Sleeper,
    TokioSleeper, TransportConnector,
};

/// Identifies a vehicle on a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VehicleKey {
    /// The system id of the vehicle.
    pub system_id: u8,
    /// The component id of the vehicle.
    pub component_id: u8,
}

impl VehicleKey {
    /// Creates a vehicle key.
    #[must_use]
    pub const fn new(system_id: u8, component_id: u8) -> Self {
        Self {
            system_id,
            component_id,
        }
    }
}

/// The request payload of the `connect` command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectRequest {
    /// The connection target, for example `udp:127.0.0.1:14550`.
    pub target: String,
}

/// The result of a command that changed the connection state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectResponse {
    /// The new connection state.
    pub status: ConnectionStatus,
    /// The vehicle that answered, if any.
    pub vehicle: Option<VehicleKey>,
}

/// The state the application keeps between commands.
#[derive(Clone)]
pub struct AppState {
    /// The supervisor that owns the link.
    pub supervisor: SessionSupervisor,

    /// The persisted settings, when the platform has a configuration directory.
    ///
    /// It is `None` on a machine that has neither a configuration directory nor a home
    /// directory. The application carries on without persisted settings there; it simply
    /// cannot remember anything between two runs.
    pub settings: Option<SettingsStore>,

    /// The history the graphs are drawn from.
    pub graphs: GraphHistory,

    /// The terrain, fetched from the service and kept on the disk.
    pub terrain: TerrainService,

    ///
    /// The tiles of the map, fetched and kept on the disk.
    ///
    /// A map that is drawn without a link is a map that was drawn before the
    /// flight: the tiles of the field are fetched and kept, and the map reads
    /// them from the disk.
    pub tiles: TileService,

    ///
    /// The flight that is being played back, when one is.
    ///
    /// A log can be played back as if it were happening: the window shows the
    /// vehicle where it was, the graphs show what it said and the map shows the
    /// track it left.
    pub playback: std::sync::Arc<std::sync::Mutex<logs::Playback>>,

    ///
    /// The track of the flight, when it is being streamed to a file.
    ///
    /// The track is written as the vehicle flies, so that another program can follow
    /// the flight as it happens. It is fed from the state the link publishes, which is
    /// the only place the position of the vehicle is known.
    pub live_track: std::sync::Arc<std::sync::Mutex<Option<live_kml::LiveTrack>>>,

    ///
    /// The folder of DTED cells that is open, when there is one.
    ///
    /// The cells are the heights of the ground in the military format, and they are what
    /// a survey of a field is planned with: the vehicle flies at a constant height over
    /// the ground, and the ground is read from these files.
    pub dted: std::sync::Arc<std::sync::Mutex<Option<mpcraft_geo::dted::DtedLibrary>>>,

    ///
    /// The request of the terrain that has been answered.
    ///
    /// The vehicle asks for the terrain again and again, and the same request is
    /// answered once.
    answered_terrain: std::sync::Arc<std::sync::Mutex<Option<mpcraft_link::TerrainRequest>>>,

    ///
    /// The web server, when it is running.
    ///
    /// The server lets another program follow the flight: the state of the vehicle and
    /// the track of the flight are served as they are now.
    pub http: std::sync::Arc<std::sync::Mutex<Option<crate::http::WebServer>>>,

    ///
    /// The corrections of a base station, when they are being sent to the vehicle.
    ///
    /// The corrections come from a caster, and they are for the receiver of the vehicle:
    /// the service holds the client and the task that reads them.
    pub ntrip: crate::ntrip::NtripService,

    ///
    /// The transponder of the vehicle, when it is being told what to say.
    ///
    /// The vehicle relays the message of the transponder, and the aircraft around it see
    /// it.
    pub transponder: crate::transponder::TransponderService,

    ///
    /// The traffic of the air, sent back onto the link.
    ///
    /// The aircraft the station read from a source of its own are not on the link, so the
    /// relay puts them there: the other stations listening to this link see them.
    pub adsb_relay: crate::adsb_relay::AdsbRelayService,

    ///
    /// The plugins the operator chose to run.
    ///
    /// The folder is read, and nothing runs until somebody says so in the settings: a program
    /// that ran whatever it found in a folder would be running code nobody agreed to. What a
    /// plugin asked for is queued here and run afterwards, because a call inside the sandbox
    /// must never wait for the wire.
    pub plugins: std::sync::Arc<std::sync::Mutex<crate::plugins::PluginRuntime>>,

    ///
    /// The formation, and the loop that runs it.
    ///
    /// The places and the decision to run them are read from the settings, so that a fleet is set
    /// up once and flown many times. The loop asks the supervisor for the link on every tick
    /// rather than holding one, because a link that was restored after dropping is the link the
    /// orders have to go to.
    pub swarm: crate::swarm::SwarmService,

    ///
    /// The nodes of a DroneCAN bus, and what the station has asked for them.
    ///
    /// The link keeps what the bus reported; this holds the decision about when the ask goes out —
    /// once at open, and once more for a node that reported how it is without saying what it is. The
    /// count belongs to the link it was made on, because a link that was restored after dropping is a
    /// new bus.
    pub dronecan: crate::dronecan::DronecanService,

    ///
    /// The link copied to another program.
    ///
    /// The rows are kept between two runs, but nothing is started by a row alone: a mirror is a
    /// decision the operator makes, the way a plugin is.
    pub mirrors: crate::mirrors::MirrorService,
}

impl Default for AppState {
    fn default() -> Self {
        let tiles = TileService::new(tile_cache_dir());
        let swarm = crate::swarm::SwarmService::new();
        let mirrors = crate::mirrors::MirrorService::default();

        // What the operator wrote is read once, so that the map draws with the folder it was
        // given and the formation starts with the places it was left with.
        if let Some(store) = SettingsStore::platform() {
            if let Ok(settings) = store.load() {
                // What the last run left is read once, for both of them: the map draws with the
                // folder it was given, and the formation starts with the places it was given.
                //
                // The loop is not started here: the decision is remembered, but there is no link
                // yet, and a task that ticks before anything is connected would decide from
                // nothing. The first moment it can decide is the moment a link is open.
                swarm.apply(&settings);
                mirrors.set_specs(settings.mirrors.clone());

                if let Some(folder) = settings.map_injection_folder {
                    tiles.set_injection(Some(std::path::PathBuf::from(folder)));
                }
            }
        }

        Self {
            supervisor: SessionSupervisor::new(),
            settings: SettingsStore::platform(),
            graphs: GraphHistory::new(),
            terrain: TerrainService::new(terrain_cache_dir()),
            tiles,
            playback: std::sync::Arc::new(std::sync::Mutex::new(logs::Playback::default())),
            live_track: std::sync::Arc::new(std::sync::Mutex::new(None)),
            dted: std::sync::Arc::new(std::sync::Mutex::new(None)),
            answered_terrain: std::sync::Arc::new(std::sync::Mutex::new(None)),
            http: std::sync::Arc::new(std::sync::Mutex::new(None)),
            ntrip: crate::ntrip::NtripService::default(),
            transponder: crate::transponder::TransponderService::default(),
            adsb_relay: crate::adsb_relay::AdsbRelayService::default(),
            plugins: std::sync::Arc::new(std::sync::Mutex::new(
                crate::plugins::PluginRuntime::new(plugin_folder()),
            )),
            swarm,
            dronecan: crate::dronecan::DronecanService::new(),
            mirrors,
        }
    }
}

/// The folder the plugins are read from.
///
/// A plugin is somebody else's code, so it lives in a folder of its own beside the settings, where an
/// operator can see what is in it. A machine with no configuration directory has nowhere to keep a
/// plugin between two runs, so the temporary directory is the best that can be done there.
fn plugin_folder() -> std::path::PathBuf {
    mpcraft_settings::platform_config_dir()
        .map(|directory| directory.join("plugins"))
        .unwrap_or_else(|| std::env::temp_dir().join("mpcraft-plugins"))
}

/// The directory the tiles of the terrain are kept in.
///
/// The tiles are kept beside the settings, in the configuration directory of the
/// application. A machine that has no configuration directory keeps them in the
/// temporary directory, which is the best that can be done there.
/// The directory the tiles of the map are kept in.
///
/// The tiles are the pictures of the ground, and there are a great many of them: the
/// folder is beside the one of the terrain.
fn tile_cache_dir() -> std::path::PathBuf {
    mpcraft_settings::platform_config_dir()
        .map(|directory| directory.join("tiles"))
        .unwrap_or_else(|| std::env::temp_dir().join("mpcraft-tiles"))
}

fn terrain_cache_dir() -> std::path::PathBuf {
    mpcraft_settings::platform_config_dir()
        .map(|directory| directory.join("terrain"))
        .unwrap_or_else(|| std::env::temp_dir().join("mpcraft-terrain"))
}

impl AppState {
    /// The values of a graph.
    ///
    /// A log that is being played back is the source of the values: the graph then shows
    /// the flight that was recorded, and the play-head says where in it the window is.
    /// The grid of the samples, the graph and the map are then the same moment seen
    /// from three sides.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no graph with that name.
    pub fn graph_series(
        &self,
        name: &str,
        since: Option<std::time::Duration>,
    ) -> CommandResult<crate::graphs::SeriesData> {
        let playback = self
            .playback
            .lock()
            .map_err(|_| CommandError::internal("the play-back is poisoned"))?;

        if !playback.is_empty() {
            let history = playback.history();

            // The window of a log is the whole flight: the play-head is what says where
            // in it the window is.
            let since = if since.is_some() { None } else { since };

            return crate::graphs::series_of(&history, name, since);
        }

        self.graphs.series(name, since)
    }

    /// Feeds the state the link publishes into the parts of the application that keep
    /// a history of the flight.
    ///
    /// The graphs are drawn from the history of the telemetry, and the track that is
    /// being streamed to a file is written from the place of the vehicle. Both are fed
    /// here, so that every way of opening a link feeds them the same way.
    ///
    /// # Errors
    ///
    /// Returns an error when the track that is being written cannot be written. The
    /// history of the graphs is never a failure: it is a memory.
    pub fn feed(&self, session: &SessionState) -> CommandResult<()> {
        self.graphs.record(&session.telemetry);

        // A plugin reads the same numbers the window shows, so the state the link publishes is what
        // the surface of the plugins is fed from, and the tick of each plugin whose own rate is due is
        // called here.
        let has_intents = {
            let Ok(mut runtime) = self.plugins.lock() else {
                return Ok(());
            };

            runtime.surface().record(session);
            runtime.tick(std::time::Instant::now());
            runtime.surface().has_pending()
        };

        // What a plugin asked for is run outside the sandbox, because a call inside it must never wait
        // for the wire. The asking was kept in the queue, and this is where it goes out.
        if has_intents {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let state = self.clone();

                handle.spawn(async move {
                    if let Err(error) = crate::plugins::run_pending(&state).await {
                        tracing::warn!("cannot run what a plugin asked for: {error}");
                    }
                });
            }
        }

        // The vehicle asks for the terrain of the ground it is over, and the answer is
        // read from the files of the terrain.
        self.answer_terrain(session);

        let Some(position) = session.telemetry.position else {
            return Ok(());
        };

        // The caster of the corrections needs to know where the vehicle is.
        self.ntrip.set_position(position);
        // The relay asks the source for the traffic around the place the vehicle is, and it
        // sorts the aircraft by how far they are from it.
        self.adsb_relay.set_position(position);
        // The transponder says where the vehicle is and how it is going.
        let heading = session
            .telemetry
            .attitude
            .map(|attitude| attitude.yaw)
            .unwrap_or(0.0);

        let (speed, vertical_speed) = session
            .telemetry
            .velocity
            .map(|velocity| (velocity.ground_speed, -velocity.down))
            .unwrap_or((0.0, 0.0));

        self.transponder
            .set_vehicle(position, heading, speed, vertical_speed);

        let Ok(mut cell) = self.live_track.lock() else {
            return Ok(());
        };

        let Some(track) = cell.as_mut() else {
            return Ok(());
        };

        track.push(position.lat, position.lng, position.alt);
        track.write()?;

        Ok(())
    }

    /// The connection profiles the user has saved.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform has no configuration directory, or when the
    /// settings file cannot be read. A file that cannot be read is reported as it is: the
    /// profiles are never quietly replaced by an empty list.
    pub fn connection_profiles(&self) -> CommandResult<Vec<ConnectionProfile>> {
        let store = self.store()?;
        let settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        Ok(settings.connection_profiles)
    }

    /// Saves the connection profiles.
    ///
    /// The other settings are left alone: the file is read, the profiles are replaced, and
    /// the file is written back.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform has no configuration directory, when the settings
    /// file cannot be read, or when it cannot be written.
    pub fn set_connection_profiles(&self, profiles: &[ConnectionProfile]) -> CommandResult<()> {
        let store = self.store()?;

        let mut settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        settings.connection_profiles = profiles.to_vec();

        store
            .save(&settings)
            .map_err(|error| CommandError::settings(error.to_string()))
    }

    /// Answers the request of the terrain the vehicle has made.
    ///
    /// The heights are read from the files of the terrain, and the answer goes out on
    /// the link. The same request is answered once: the vehicle asks again and again,
    /// and the answer of the same one is the same.
    fn answer_terrain(&self, session: &SessionState) {
        let Some(request) = session.terrain_request else {
            return;
        };

        // The same request is answered once: the vehicle asks again and again, and the
        // answer of the same one is the same.
        if self
            .answered_terrain
            .lock()
            .ok()
            .and_then(|answered| *answered)
            == Some(request)
        {
            return;
        }

        let Ok(library) = self.dted.lock() else {
            return;
        };

        let Some(library) = library.as_ref() else {
            // There is no ground to read the heights from. The request is not answered,
            // and the vehicle asks again when the files are there.
            return;
        };

        let mut squares = Vec::new();

        for (bit, _) in request.squares() {
            let origin = request.square_of(bit);

            let mut heights = [0_i16; mpcraft_link::terrain::GRID_POINTS];
            let mut complete = true;

            for (index, height) in heights.iter_mut().enumerate() {
                let x = index % mpcraft_link::terrain::GRID_SIDE;
                let y = index / mpcraft_link::terrain::GRID_SIDE;

                let (latitude, longitude) = request.point_of(origin, x, y);

                // The height is read from the cell of the terrain that covers the
                // place: a place outside every cell has no height.
                let found = library
                    .cell_at(latitude, longitude)
                    .and_then(|cell| cell.elevation_at(latitude, longitude).ok());

                match found {
                    Some(found) => *height = found as i16,
                    None => {
                        // A square with a height missing is not answered: the vehicle
                        // asks again, and the file may be there the next time.
                        complete = false;
                        break;
                    }
                }
            }

            if !complete {
                continue;
            }

            squares.push(mpcraft_link::TerrainSquare {
                latitude: request.latitude,
                longitude: request.longitude,
                grid_spacing: request.grid_spacing,
                bit: bit as u8,
                heights,
            });
        }

        tracing::debug!("the terrain is answered with {} squares", squares.len());

        let Some(link) = self.supervisor.session() else {
            return;
        };

        // The request is answered now, and it is not answered again.
        if let Ok(mut answered) = self.answered_terrain.lock() {
            *answered = Some(request);
        }

        tokio::spawn(async move {
            for square in squares {
                let message = mpcraft_mavlink::dialect::MavMessage::TERRAIN_DATA(square.message());

                if let Err(error) = link.write_message(&message).await {
                    tracing::warn!("the terrain could not be answered: {error}");
                    return;
                }
            }
        });
    }

    /// Chooses which vehicle the station speaks to.
    ///
    /// The choice is recorded by the link, because the link is what knows which boards answered:
    /// the interface cannot disagree with it about which vehicle is the one it is reading, in the
    /// same way it cannot disagree with the keyring about which key signs. What is decided here is
    /// only that there has to be a link to make a choice on.
    ///
    /// A board that was never seen is refused, and the choice that stands is left where it was —
    /// an address somebody typed is not a vehicle.
    ///
    /// # Errors
    ///
    /// Returns `not_connected` when there is no open link, and `invalid_request` for a board that
    /// the link has never heard.
    pub async fn choose_vehicle(&self, key: VehicleKey) -> CommandResult<VehicleKey> {
        let Some(link) = self.supervisor.session() else {
            return Err(CommandError::not_connected());
        };

        link.select_vehicle(mpcraft_link::VehicleId::new(
            key.system_id,
            key.component_id,
        ))
        .await
        .map_err(|error| CommandError::new(error.code(), error.to_string()))?;

        Ok(key)
    }

    /// What the keyring says about what the operator wrote.
    ///
    /// The decision about a secret — whether it can be read as thirty-two bytes — is made by the
    /// keyring, so this is where the interface is told which name was left out rather than being
    /// shown a list that quietly dropped it.
    ///
    /// # Errors
    ///
    /// Returns an error when this machine has no configuration directory, or when the settings file
    /// cannot be read.
    pub fn signing_status(&self) -> CommandResult<crate::signing::SigningStatus> {
        let settings = self.load_settings()?;

        Ok(crate::signing::read(&settings).status())
    }

    /// Saves the keys of the signed link.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform has no configuration directory, when the
    /// settings file cannot be read, or when it cannot be written, and answers with what
    /// the keyring makes of the list that was sent: which secrets could not be read, and
    /// which one the frames are signed with.
    pub fn set_signing_keys(
        &self,
        keys: &[mpcraft_settings::SigningKeyEntry],
    ) -> CommandResult<crate::signing::SigningStatus> {
        let store = self.store()?;

        let mut settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        settings.signing_keys = keys.to_vec();

        store
            .save(&settings)
            .map_err(|error| CommandError::settings(error.to_string()))?;

        Ok(crate::signing::read(&settings).status())
    }

    /// The folder of tiles the operator has brought, when there is one.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform has no configuration directory, or when the
    /// settings file cannot be read.
    pub fn map_injection_folder(&self) -> CommandResult<Option<String>> {
        let store = self.store()?;
        let settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        Ok(settings.map_injection_folder)
    }

    /// Says which folder of tiles the operator has brought.
    ///
    /// The folder is kept between two runs, and the map is told about it at once.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform has no configuration directory, when the
    /// settings file cannot be read, or when it cannot be written.
    pub fn set_map_injection_folder(&self, folder: Option<String>) -> CommandResult<()> {
        let store = self.store()?;

        let mut settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        settings.map_injection_folder = folder.clone();

        store
            .save(&settings)
            .map_err(|error| CommandError::settings(error.to_string()))?;

        self.tiles
            .set_injection(folder.map(std::path::PathBuf::from));

        Ok(())
    }

    /// The places the operator marked on the map.
    ///
    /// # Errors
    ///
    /// Returns an error when this machine has no configuration directory or when the settings
    /// file cannot be read.
    pub fn pois(&self) -> CommandResult<Vec<mpcraft_settings::Poi>> {
        let store = self.store()?;

        let settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        Ok(settings.pois)
    }

    /// Adds a place, unless the same place with the same name is already there.
    ///
    /// # Errors
    ///
    /// Returns an error when the name is empty, when the place is not a place, or when the
    /// settings cannot be read or written.
    pub fn add_poi(
        &self,
        name: &str,
        latitude: f64,
        longitude: f64,
    ) -> CommandResult<Vec<mpcraft_settings::Poi>> {
        let name = name.trim();

        if name.is_empty() {
            return Err(CommandError::invalid_request(
                "a place of interest needs a name",
            ));
        }

        if !latitude.is_finite()
            || !longitude.is_finite()
            || !(-90.0..=90.0).contains(&latitude)
            || !(-180.0..=180.0).contains(&longitude)
        {
            return Err(CommandError::invalid_request(
                "a place of interest needs a latitude and a longitude",
            ));
        }

        let poi = mpcraft_settings::Poi {
            name: name.to_owned(),
            latitude,
            longitude,
        };

        let store = self.store()?;

        let mut settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        if !settings.pois.iter().any(|known| known.same(&poi)) {
            settings.pois.push(poi);
        }

        store
            .save(&settings)
            .map_err(|error| CommandError::settings(error.to_string()))?;

        Ok(settings.pois)
    }

    /// Takes one place off the map.
    ///
    /// # Errors
    ///
    /// Returns an error when the settings cannot be read or written.
    pub fn remove_poi(
        &self,
        poi: &mpcraft_settings::Poi,
    ) -> CommandResult<Vec<mpcraft_settings::Poi>> {
        let store = self.store()?;

        let mut settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        settings.pois.retain(|known| !known.same(poi));

        store
            .save(&settings)
            .map_err(|error| CommandError::settings(error.to_string()))?;

        Ok(settings.pois)
    }

    /// Takes every place off the map.
    ///
    /// # Errors
    ///
    /// Returns an error when the settings cannot be read or written.
    pub fn clear_pois(&self) -> CommandResult<Vec<mpcraft_settings::Poi>> {
        let store = self.store()?;

        let mut settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        settings.pois.clear();

        store
            .save(&settings)
            .map_err(|error| CommandError::settings(error.to_string()))?;

        Ok(settings.pois)
    }

    /// What the mirrors are doing.
    #[must_use]
    pub fn mirrors_status(&self) -> Vec<crate::mirrors::MirrorStatus> {
        self.mirrors.status()
    }

    /// Saves the rows of the mirrors and stops the ones that are no longer there.
    ///
    /// # Errors
    ///
    /// Returns an error when the settings cannot be read or written.
    pub fn set_mirrors(
        &self,
        specs: &[mpcraft_settings::MirrorSpec],
    ) -> CommandResult<Vec<crate::mirrors::MirrorStatus>> {
        let store = self.store()?;

        let mut settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        settings.mirrors = specs.to_vec();

        store
            .save(&settings)
            .map_err(|error| CommandError::settings(error.to_string()))?;

        self.mirrors.set_specs(specs.to_vec());

        Ok(self.mirrors.status())
    }

    /// Starts one of the saved rows against the link that is open.
    ///
    /// # Errors
    ///
    /// Returns an error when the row is not one of the saved ones, when there is no link, or when
    /// the endpoint cannot be opened.
    pub async fn start_mirror(
        &self,
        key: &str,
    ) -> CommandResult<Vec<crate::mirrors::MirrorStatus>> {
        let spec = self.mirrors.find(key).ok_or_else(|| {
            CommandError::invalid_request(format!("there is no mirror called {key}"))
        })?;

        let session = self
            .supervisor
            .session()
            .ok_or_else(CommandError::not_connected)?;

        self.mirrors
            .start(session, &spec)
            .await
            .map_err(|error| CommandError::new(error.code(), error.to_string()))?;

        Ok(self.mirrors.status())
    }

    /// Stops one of the rows.
    #[must_use]
    pub fn stop_mirror(&self, key: &str) -> Vec<crate::mirrors::MirrorStatus> {
        self.mirrors.stop(key);

        self.mirrors.status()
    }

    /// Puts the saved rows into the service without starting anything.
    ///
    /// # Errors
    ///
    /// Returns an error when this machine has no configuration directory or when the settings file
    /// cannot be read.
    pub fn apply_mirrors(&self) -> CommandResult<()> {
        let store = self.store()?;

        let settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        self.mirrors.set_specs(settings.mirrors);

        Ok(())
    }

    /// What the formation is doing.
    #[must_use]
    pub fn swarm_status(&self) -> crate::swarm::SwarmStatus {
        self.swarm.status()
    }

    /// Saves what the operator wrote for the formation and puts it to work.
    ///
    /// The places are saved as they were written, including a name that cannot be read as a
    /// vehicle: the file has to say what the operator wrote, and the interface says which of those
    /// names it could not use. The loop is started or stopped by the decision and not by the
    /// places, because writing a fleet is not the same thing as sending orders to a fleet.
    ///
    /// # Errors
    ///
    /// Returns an error when this machine has no configuration directory, when the settings file
    /// cannot be read or written, or when the loop cannot be started.
    pub fn set_swarm(&self, req: &crate::swarm::SwarmRequest) -> CommandResult<()> {
        let store = self.store()?;

        let mut settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        settings.swarm_leader = req.leader.clone();
        settings.swarm_enabled = req.enabled;
        settings.swarm_slots = req.slots.clone();

        store
            .save(&settings)
            .map_err(|error| CommandError::settings(error.to_string()))?;

        self.swarm.apply(&settings);

        if req.enabled && !self.swarm.status().running {
            self.swarm.start(&self.supervisor)?;
        } else if !req.enabled {
            self.swarm.stop();
        }

        Ok(())
    }

    /// Puts the settings into the service and starts the loop with the decision that was kept.
    ///
    /// This is what a run that remembers does: the places are read again, and the loop is started
    /// only when the operator had decided to run it. A file that holds slots and no decision is a
    /// formation that is described, not one that sends orders.
    ///
    /// # Errors
    ///
    /// Returns an error when this machine has no configuration directory, when the settings file
    /// cannot be read, or when the loop cannot be started.
    pub fn apply_swarm(&self) -> CommandResult<()> {
        let store = self.store()?;

        let settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        self.swarm.apply(&settings);

        if settings.swarm_enabled && !self.swarm.status().running {
            self.swarm.start(&self.supervisor)?;
        }

        Ok(())
    }

    /// Gives the supervisor the keys of the link, so that the link that is opened next
    /// is signed.
    ///
    /// A key whose secret is not hexadecimal is left out: a link that is signed with a
    /// key that is not the one the vehicle has is a link that does not work at all, and
    /// the operator has to be told which key is wrong. The reading of a secret is done by
    /// the keyring, once, so that this and the answer the interface gets cannot disagree
    /// about which key is the one that signs.
    pub fn apply_signing(&self) {
        let settings = self.load_settings().unwrap_or_default();
        let keyring = crate::signing::read(&settings);

        for name in &keyring.refused {
            tracing::warn!("the key {name} is not hexadecimal and is not used");
        }

        self.supervisor.set_signing(Some(keyring.setup()));
    }

    /// The settings store, when there is one.
    fn store(&self) -> CommandResult<&SettingsStore> {
        self.settings.as_ref().ok_or_else(|| {
            CommandError::settings(
                "this machine has no configuration directory to keep the settings in",
            )
        })
    }

    /// The settings as the file holds them.
    fn load_settings(&self) -> CommandResult<mpcraft_settings::Settings> {
        self.store()?
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))
    }

    /// The plugins the folder holds, with what the operator chose.
    ///
    /// The folder is read, the plugins that were chosen are run through their first stage, and what
    /// the driver sees is returned. A plugin that could not be read stands failed with its own code,
    /// which is what the sheet lists beside its name.
    ///
    /// # Errors
    ///
    /// Returns an error when the settings file cannot be read, or when the folder of plugins cannot be
    /// read.
    pub fn list_plugins(&self) -> CommandResult<Vec<crate::plugins::PluginInfo>> {
        let store = self.store()?;
        let settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        let Ok(mut runtime) = self.plugins.lock() else {
            return Err(CommandError::internal(
                "the driver of the plugins is locked",
            ));
        };

        runtime.scan(&settings)?;
        runtime.open()
    }

    /// Marks a plugin as run, or leaves it alone, and keeps the decision for the next run.
    ///
    /// The other settings are left alone: the file is read, the choice is written, and the file is
    /// written back. The plugin that was chosen is run now, and the one that was left alone is told
    /// that the application is closing.
    ///
    /// # Errors
    ///
    /// Returns an error when the settings file cannot be read or written, when the plugin was not
    /// seen, or when the one that was chosen could not be read.
    pub fn set_plugin_enabled(&self, name: &str, enabled: bool) -> CommandResult<()> {
        let store = self.store()?;

        let mut settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        settings.plugins.retain(|entry| entry.name != name);
        settings.plugins.push(PluginEntry {
            name: name.to_owned(),
            enabled,
        });

        store
            .save(&settings)
            .map_err(|error| CommandError::settings(error.to_string()))?;

        let Ok(mut runtime) = self.plugins.lock() else {
            return Err(CommandError::internal(
                "the driver of the plugins is locked",
            ));
        };

        runtime.set_enabled(name, enabled)
    }

    /// What the plugins said, most recent last, which is what the message console reads.
    #[must_use]
    pub fn plugin_notes(&self) -> Vec<String> {
        let Ok(runtime) = self.plugins.lock() else {
            return Vec::new();
        };

        runtime.surface().notes()
    }

    /// Calls the tick of each plugin whose own rate is due.
    ///
    /// The driver is called on the tick of the telemetry, and a plugin that asked for two times a
    /// second is not called twenty times a second.
    pub fn tick_plugins(&self, now: std::time::Instant) -> usize {
        let Ok(mut runtime) = self.plugins.lock() else {
            return 0;
        };

        runtime.tick(now)
    }

    /// What the plugins asked for, which is run outside the sandbox.
    ///
    /// A call inside the sandbox is never allowed to wait for the wire, so the asking is kept here and
    /// carried by the link afterwards.
    pub fn pending_plugin_commands(&self) -> Vec<mpcraft_plugin::CommandIntent> {
        let Ok(runtime) = self.plugins.lock() else {
            return Vec::new();
        };

        runtime.surface().take_pending()
    }

    /// Tells every plugin that is running that the link is open.
    pub fn plugin_link_opened(&self) {
        if let Ok(mut runtime) = self.plugins.lock() {
            runtime.link_opened();
        }
    }

    /// The folder the plugins are read from, which is what the sheet says so that an operator knows
    /// where to put one.
    #[must_use]
    pub fn plugin_folder(&self) -> String {
        let Ok(runtime) = self.plugins.lock() else {
            return String::new();
        };

        runtime.folder().to_string_lossy().to_string()
    }

    /// What a plugin said, which is what the message console reads.
    ///
    /// # Errors
    ///
    /// Returns an error when the driver is locked.
    pub fn plugin_note(&self, text: &str) -> CommandResult<()> {
        let runtime = match self.plugins.lock() {
            Ok(runtime) => runtime,
            Err(_) => {
                return Err(CommandError::internal(
                    "the driver of the plugins is locked",
                ));
            }
        };

        runtime.surface().say(text).map_err(CommandError::from)
    }

    /// Tells every plugin that is running that the application is closing.
    pub fn close_plugins(&self) -> Vec<crate::plugins::PluginInfo> {
        match self.plugins.lock() {
            Ok(mut runtime) => runtime.close(),
            Err(_) => Vec::new(),
        }
    }

    /// The parameters the user has marked as favourites.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform has no configuration directory, or when the
    /// settings file cannot be read.
    pub fn favourite_parameters(&self) -> CommandResult<Vec<String>> {
        let store = self.store()?;
        let settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        Ok(settings.favourite_parameters)
    }

    /// Marks a parameter as a favourite, or takes the mark away.
    ///
    /// The other settings are left alone: the file is read, the favourites are replaced,
    /// and the file is written back.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform has no configuration directory, when the settings
    /// file cannot be read, or when it cannot be written.
    pub fn set_favourite_parameter(&self, name: &str, favourite: bool) -> CommandResult<()> {
        let store = self.store()?;

        let mut settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        settings.favourite_parameters.retain(|known| known != name);

        if favourite {
            settings.favourite_parameters.push(name.to_owned());
        }

        store
            .save(&settings)
            .map_err(|error| CommandError::settings(error.to_string()))
    }
}

/// The link the application should open at launch.
///
/// The command line wins over the saved profiles: an argument is a deliberate choice made
/// at the moment of the launch, while a profile is a default that was set up days ago. With
/// no argument, the first profile marked for automatic connection is opened.
#[must_use]
pub fn launch_target(target: Option<&str>, profiles: &[ConnectionProfile]) -> Option<String> {
    if let Some(target) = target.map(str::trim).filter(|target| !target.is_empty()) {
        return Some(target.to_owned());
    }

    profiles
        .iter()
        .find(|profile| profile.auto_connect)
        .map(|profile| profile.target.clone())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    /// Counts the temporary directories, so that two tests never share one.
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// A directory that is removed when the test ends.
    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        /// A directory of its own, under the temporary directory of the system.
        fn new(name: &str) -> Self {
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "mpcraft-app-{name}-{}-{unique}",
                std::process::id()
            ));

            std::fs::create_dir_all(&path).expect("the temporary directory should be creatable");

            Self { path }
        }

        /// The path of the directory.
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    /// The driver lives in the state of the application, which is what the shell hands to a command
    /// through a lock, so it has to be something one task can hand to another.
    fn assert_send<T: Send>() {}

    #[test]
    fn the_driver_of_the_plugins_can_be_handed_between_tasks() {
        assert_send::<crate::plugins::PluginRuntime>();
    }

    #[test]
    fn the_error_codes_are_stable() {
        assert_eq!(CommandError::CODE_TIMEOUT, "link_timeout");
        assert_eq!(CommandError::CODE_NO_VEHICLE, "no_vehicle");
        assert_eq!(CommandError::CODE_NOT_CONNECTED, "not_connected");
        assert_eq!(CommandError::CODE_SETTINGS, "settings_error");
    }

    #[test]
    fn the_pair_crosses_the_boundary_with_the_spelling_the_interface_reads() {
        // The same rule as the vehicle crate, read from the side that answers the commands: the contract
        // reads `systemId` and `componentId`, so the document the shell writes for a request that names a
        // vehicle has to write them that way. What the interface declares and what the boundary writes were
        // two spellings of one field, and the disagreement could only be found by reading both sides.
        let json = serde_json::to_string(&VehicleKey::new(2, 1)).unwrap();

        assert_eq!(json, r#"{"systemId":2,"componentId":1}"#);
    }

    #[test]
    fn the_profiles_are_read_from_the_settings_store() {
        let directory = TempDir::new("profiles");
        let store = SettingsStore::new(directory.path());

        let state = AppState {
            supervisor: SessionSupervisor::new(),
            settings: Some(store),
            graphs: GraphHistory::new(),
            terrain: TerrainService::new(terrain_cache_dir()),
            tiles: TileService::new(tile_cache_dir()),
            playback: std::sync::Arc::new(std::sync::Mutex::new(logs::Playback::default())),
            live_track: std::sync::Arc::new(std::sync::Mutex::new(None)),
            dted: std::sync::Arc::new(std::sync::Mutex::new(None)),
            answered_terrain: std::sync::Arc::new(std::sync::Mutex::new(None)),
            http: std::sync::Arc::new(std::sync::Mutex::new(None)),
            ntrip: crate::ntrip::NtripService::default(),
            transponder: crate::transponder::TransponderService::default(),
            adsb_relay: crate::adsb_relay::AdsbRelayService::default(),
            plugins: std::sync::Arc::new(std::sync::Mutex::new(
                crate::plugins::PluginRuntime::new(plugin_folder()),
            )),
            swarm: crate::swarm::SwarmService::new(),
            dronecan: crate::dronecan::DronecanService::new(),
            mirrors: crate::mirrors::MirrorService::default(),
        };

        assert!(state
            .connection_profiles()
            .expect("the profiles should be readable")
            .is_empty());

        let profiles = vec![ConnectionProfile::auto_connect(
            "SITL",
            "udp:127.0.0.1:14550",
        )];
        state
            .set_connection_profiles(&profiles)
            .expect("the profiles should be saved");

        assert_eq!(
            state
                .connection_profiles()
                .expect("the profiles should be readable"),
            profiles
        );
    }

    #[test]
    fn the_keyring_says_which_secret_it_could_not_read_and_which_one_signs() {
        let directory = TempDir::new("keyring");
        let store = SettingsStore::new(directory.path());

        let state = AppState {
            supervisor: SessionSupervisor::new(),
            settings: Some(store),
            graphs: GraphHistory::new(),
            terrain: TerrainService::new(terrain_cache_dir()),
            tiles: TileService::new(tile_cache_dir()),
            playback: std::sync::Arc::new(std::sync::Mutex::new(logs::Playback::default())),
            live_track: std::sync::Arc::new(std::sync::Mutex::new(None)),
            dted: std::sync::Arc::new(std::sync::Mutex::new(None)),
            answered_terrain: std::sync::Arc::new(std::sync::Mutex::new(None)),
            http: std::sync::Arc::new(std::sync::Mutex::new(None)),
            ntrip: crate::ntrip::NtripService::default(),
            transponder: crate::transponder::TransponderService::default(),
            adsb_relay: crate::adsb_relay::AdsbRelayService::default(),
            plugins: std::sync::Arc::new(std::sync::Mutex::new(
                crate::plugins::PluginRuntime::new(plugin_folder()),
            )),
            swarm: crate::swarm::SwarmService::new(),
            dronecan: crate::dronecan::DronecanService::new(),
            mirrors: crate::mirrors::MirrorService::default(),
        };

        // The hand: sixty-four characters are thirty-two bytes, and `zz` is not a pair of the
        // hexadecimal numbers, so the second line is not a key. Both stay in the answer because the
        // file is read by hand, and the answer names which one was left out.
        let readable = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        assert_eq!(
            readable.len(),
            64,
            "sixty-four characters are thirty-two bytes"
        );

        let status = state
            .set_signing_keys(&[
                mpcraft_settings::SigningKeyEntry::new("station", readable),
                mpcraft_settings::SigningKeyEntry::new("typo", "zz"),
            ])
            .expect("the keys should be saved");

        assert_eq!(
            status.keys.len(),
            2,
            "both lines are kept as they were written"
        );
        assert_eq!(status.refused, vec!["typo".to_owned()]);
        assert_eq!(status.signed_with.as_deref(), Some("station"));

        // What the keyring says is what the link is opened with: the key that could be read signs,
        // and the one that could not is not in the ring the incoming frames are checked against.
        state.apply_signing();

        let setup = state
            .supervisor
            .signing()
            .expect("the keyring reaches the link");

        assert!(
            setup.key.is_some(),
            "the readable key is the one that signs"
        );
        assert!(
            !setup.keyring.is_empty(),
            "the ring holds what could be read"
        );

        // A file whose only secret cannot be read is a link that is not signed — not a link signed
        // with a key nobody wrote.
        state
            .set_signing_keys(&[mpcraft_settings::SigningKeyEntry::new("typo", "zz")])
            .expect("the keys should be saved");
        state.apply_signing();

        let alone = state
            .supervisor
            .signing()
            .expect("the keyring reaches the link");

        assert!(alone.key.is_none());
        assert!(
            alone.keyring.is_empty(),
            "nothing was read, so nothing is verified against"
        );
    }

    #[test]
    fn the_formation_keeps_what_was_written_and_says_which_name_it_could_not_use() {
        let directory = TempDir::new("formation");
        let store = SettingsStore::new(directory.path());
        let path = store.path().clone();

        let state = AppState {
            supervisor: SessionSupervisor::new(),
            settings: Some(store),
            graphs: GraphHistory::new(),
            terrain: TerrainService::new(terrain_cache_dir()),
            tiles: TileService::new(tile_cache_dir()),
            playback: std::sync::Arc::new(std::sync::Mutex::new(logs::Playback::default())),
            live_track: std::sync::Arc::new(std::sync::Mutex::new(None)),
            dted: std::sync::Arc::new(std::sync::Mutex::new(None)),
            answered_terrain: std::sync::Arc::new(std::sync::Mutex::new(None)),
            http: std::sync::Arc::new(std::sync::Mutex::new(None)),
            ntrip: crate::ntrip::NtripService::default(),
            transponder: crate::transponder::TransponderService::default(),
            adsb_relay: crate::adsb_relay::AdsbRelayService::default(),
            plugins: std::sync::Arc::new(std::sync::Mutex::new(
                crate::plugins::PluginRuntime::new(plugin_folder()),
            )),
            swarm: crate::swarm::SwarmService::new(),
            dronecan: crate::dronecan::DronecanService::new(),
            mirrors: crate::mirrors::MirrorService::default(),
        };

        let request = crate::swarm::SwarmRequest {
            leader: Some("1:1".to_owned()),
            enabled: false,
            slots: vec![
                mpcraft_settings::SwarmSlot::with_offset("2:1", 0.0, 60.0, 0.0),
                mpcraft_settings::SwarmSlot::new("board two"),
            ],
        };

        state
            .set_swarm(&request)
            .expect("the formation should be saved");

        // The file keeps what the operator wrote, including the name that could not be read:
        // somebody fixing the file has to see what they actually wrote, not what an interface
        // guessed from it.
        let text = std::fs::read_to_string(&path).expect("the file should be readable");
        assert!(text.contains("board two"), "{text}");

        // And the service says which of those names it left out, without pretending that an
        // unreadable name is a vehicle.
        let status = state.swarm_status();
        assert_eq!(status.slots.len(), 1);
        assert_eq!(status.refused, vec!["board two".to_owned()]);
        assert!(!status.running, "writing places does not start a fleet");
    }

    #[test]
    fn a_machine_without_a_configuration_directory_says_so() {
        let state = AppState {
            supervisor: SessionSupervisor::new(),
            settings: None,
            graphs: GraphHistory::new(),
            terrain: TerrainService::new(terrain_cache_dir()),
            tiles: TileService::new(tile_cache_dir()),
            playback: std::sync::Arc::new(std::sync::Mutex::new(logs::Playback::default())),
            live_track: std::sync::Arc::new(std::sync::Mutex::new(None)),
            dted: std::sync::Arc::new(std::sync::Mutex::new(None)),
            answered_terrain: std::sync::Arc::new(std::sync::Mutex::new(None)),
            http: std::sync::Arc::new(std::sync::Mutex::new(None)),
            ntrip: crate::ntrip::NtripService::default(),
            transponder: crate::transponder::TransponderService::default(),
            adsb_relay: crate::adsb_relay::AdsbRelayService::default(),
            plugins: std::sync::Arc::new(std::sync::Mutex::new(
                crate::plugins::PluginRuntime::new(plugin_folder()),
            )),
            swarm: crate::swarm::SwarmService::new(),
            dronecan: crate::dronecan::DronecanService::new(),
            mirrors: crate::mirrors::MirrorService::default(),
        };

        let error = state
            .connection_profiles()
            .expect_err("there is nowhere to read the profiles from");

        assert_eq!(error.code, CommandError::CODE_SETTINGS);
    }

    #[test]
    fn the_command_line_wins_over_the_saved_profiles() {
        let profiles = vec![ConnectionProfile::auto_connect(
            "SITL",
            "udp:127.0.0.1:14550",
        )];

        let target = launch_target(Some("serial:COM3:57600"), &profiles);

        assert_eq!(target.as_deref(), Some("serial:COM3:57600"));
    }

    #[test]
    fn a_profile_is_opened_at_launch_when_there_is_no_argument() {
        let profiles = vec![
            ConnectionProfile::new("Flight controller", "serial:COM3:57600"),
            ConnectionProfile::auto_connect("SITL", "udp:127.0.0.1:14550"),
        ];

        let target = launch_target(None, &profiles);

        assert_eq!(target.as_deref(), Some("udp:127.0.0.1:14550"));
    }

    #[test]
    fn nothing_is_opened_when_nothing_asks_for_it() {
        let profiles = vec![ConnectionProfile::new(
            "Flight controller",
            "serial:COM3:57600",
        )];

        assert!(launch_target(None, &profiles).is_none());
        assert!(launch_target(Some("  "), &profiles).is_none());
    }
}
