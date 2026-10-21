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
use mpcraft_settings::SettingsStore;
use serde::{Deserialize, Serialize};

pub mod checklist;
pub mod coordinates;
pub mod error;
pub mod graphs;
pub mod logs;
pub mod operations;
pub mod supervisor;

pub use checklist::{ChecklistEntry, ChecklistItem, Condition, Source};
pub use coordinates::{Coordinates, Utm};
pub use error::{CommandError, CommandResult};
pub use graphs::{GraphHistory, PresetInfo, SeriesData, SeriesInfo, SeriesPoints};
pub use mpcraft_link::SessionState;
pub use mpcraft_settings::ConnectionProfile;
pub use operations::OutputFunction;
pub use operations::{flight_modes, ModeInfo, PlanInfo, PlanItem, PlanKind};
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
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            supervisor: SessionSupervisor::new(),
            settings: SettingsStore::platform(),
            graphs: GraphHistory::new(),
            terrain: TerrainService::new(terrain_cache_dir()),
        }
    }
}

/// The directory the tiles of the terrain are kept in.
///
/// The tiles are kept beside the settings, in the configuration directory of the
/// application. A machine that has no configuration directory keeps them in the
/// temporary directory, which is the best that can be done there.
fn terrain_cache_dir() -> std::path::PathBuf {
    mpcraft_settings::platform_config_dir()
        .map(|directory| directory.join("terrain"))
        .unwrap_or_else(|| std::env::temp_dir().join("mpcraft-terrain"))
}

impl AppState {
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
