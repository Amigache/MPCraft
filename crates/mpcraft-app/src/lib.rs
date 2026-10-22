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
use mpcraft_settings::SettingsStore;
use serde::{Deserialize, Serialize};

pub mod board;
pub mod bootloader;
pub mod checklist;
pub mod coordinates;
pub mod diagnostics;
pub mod error;
pub mod firmware;
pub mod graphs;
pub mod http;
pub mod live_kml;
pub mod logs;
pub mod ntrip;
pub mod operations;
pub mod supervisor;
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
pub use operations::OutputFunction;
pub use operations::{flight_modes, ModeInfo, PlanInfo, PlanItem, PlanKind, VehicleLog};
pub use supervisor::{
    ConnectionStatus, Connector, LinkSnapshot, ReconnectPolicy, SessionSupervisor, Sleeper,
    TokioSleeper, TransportConnector,
};

/// Identifies a vehicle on a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
}

impl Default for AppState {
    fn default() -> Self {
        let tiles = TileService::new(tile_cache_dir());

        // The folder of tiles the operator has brought is read from the settings, so
        // that the map is drawn with them from the first moment.
        if let Some(store) = SettingsStore::platform() {
            if let Ok(settings) = store.load() {
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
            http: std::sync::Arc::new(std::sync::Mutex::new(None)),
            ntrip: crate::ntrip::NtripService::default(),
            transponder: crate::transponder::TransponderService::default(),
        }
    }
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

/// The bytes of a hexadecimal text, when it is hexadecimal.
fn hex_to_bytes(text: &str) -> Option<[u8; 32]> {
    let text = text.trim();
    if text.len() != 64 {
        return None;
    }

    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let pair = text.get(index * 2..index * 2 + 2)?;
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }

    Some(bytes)
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

        let Some(position) = session.telemetry.position else {
            return Ok(());
        };

        // The caster of the corrections needs to know where the vehicle is.
        self.ntrip.set_position(position);

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

    /// The keys of the signed link.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform has no configuration directory, or when the
    /// settings file cannot be read.
    pub fn signing_keys(&self) -> CommandResult<Vec<mpcraft_settings::SigningKeyEntry>> {
        let store = self.store()?;
        let settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        Ok(settings.signing_keys)
    }

    /// Saves the keys of the signed link.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform has no configuration directory, when the
    /// settings file cannot be read, or when it cannot be written.
    pub fn set_signing_keys(
        &self,
        keys: &[mpcraft_settings::SigningKeyEntry],
    ) -> CommandResult<()> {
        let store = self.store()?;

        let mut settings = store
            .load()
            .map_err(|error| CommandError::settings(error.to_string()))?;

        settings.signing_keys = keys.to_vec();

        store
            .save(&settings)
            .map_err(|error| CommandError::settings(error.to_string()))
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

    /// Gives the supervisor the keys of the link, so that the link that is opened next
    /// is signed.
    ///
    /// A key whose secret is not hexadecimal is left out: a link that is signed with a
    /// key that is not the one the vehicle has is a link that does not work at all, and
    /// the operator has to be told which key is wrong.
    pub fn apply_signing(&self) {
        let keys = self.signing_keys().unwrap_or_default();

        let mut keyring = mpcraft_link::SigningKeyring::new();
        let mut signing_key = None;

        for entry in &keys {
            let Some(bytes) = hex_to_bytes(&entry.secret) else {
                tracing::warn!("the key {} is not hexadecimal and is not used", entry.name);
                continue;
            };

            let key = mpcraft_link::SigningKey::from_bytes(bytes);
            keyring.insert(entry.name.clone(), key.clone());

            // The first key that can be read is the one the frames are signed with.
            if signing_key.is_none() {
                signing_key = Some(key);
            }
        }

        self.supervisor
            .set_signing(Some(mpcraft_link::SigningSetup {
                key: signing_key,
                keyring,
                link_id: 1,
            }));
    }

    /// The settings store, when there is one.
    fn store(&self) -> CommandResult<&SettingsStore> {
        self.settings.as_ref().ok_or_else(|| {
            CommandError::settings(
                "this machine has no configuration directory to keep the settings in",
            )
        })
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

    #[test]
    fn the_error_codes_are_stable() {
        assert_eq!(CommandError::CODE_TIMEOUT, "link_timeout");
        assert_eq!(CommandError::CODE_NO_VEHICLE, "no_vehicle");
        assert_eq!(CommandError::CODE_NOT_CONNECTED, "not_connected");
        assert_eq!(CommandError::CODE_SETTINGS, "settings_error");
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
            http: std::sync::Arc::new(std::sync::Mutex::new(None)),
            ntrip: crate::ntrip::NtripService::default(),
            transponder: crate::transponder::TransponderService::default(),
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
            http: std::sync::Arc::new(std::sync::Mutex::new(None)),
            ntrip: crate::ntrip::NtripService::default(),
            transponder: crate::transponder::TransponderService::default(),
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
