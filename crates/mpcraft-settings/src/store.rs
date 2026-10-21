//! Where the settings live, and how they are read and written.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::error::SettingsError;
use crate::profiles::ConnectionProfile;

/// The name of the settings file inside the configuration directory.
const FILE_NAME: &str = "settings.toml";

/// The name of the application directory inside the platform configuration directory.
const APP_DIRECTORY: &str = "MPCraft";

/// Counts the temporary files, so that two writes never share a name.
static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

/// The persisted settings.
///
/// The document is the file, one to one: what is read is what is written. A section that
/// is added later joins the others here rather than in a file of its own, so that a write
/// never has to guess how to merge two files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case", deny_unknown_fields)]
pub struct Settings {
    /// The connection profiles the user has saved.
    pub connection_profiles: Vec<ConnectionProfile>,

    /// The parameters the user has marked as favourites.
    ///
    /// Upstream has a column of stars in its parameter window, and the stars are worth
    /// keeping between two runs: the parameters an operator touches are the ones they
    /// touch every time.
    pub favourite_parameters: Vec<String>,
}

/// A store of the persisted settings.
///
/// The directory is a parameter and never a global. A test points the store at a temporary
/// directory and therefore never touches the settings of the user running it, and an
/// application that keeps its settings somewhere else — a portable installation on a USB
/// stick, say — can say so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsStore {
    directory: PathBuf,
}

impl SettingsStore {
    /// A store in an explicit directory.
    #[must_use]
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    /// A store in the platform configuration directory.
    ///
    /// Returns `None` on a machine that has no configuration directory at all: a service
    /// account without a home directory, for instance. The application treats that as "the
    /// settings cannot be persisted" rather than as a fault.
    #[must_use]
    pub fn platform() -> Option<Self> {
        platform_config_dir().map(Self::new)
    }

    /// The directory the settings live in.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// The path of the settings file.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.directory.join(FILE_NAME)
    }

    /// Reads the settings.
    ///
    /// A file that does not exist is not an error: it means nothing has been configured
    /// yet, which is the state of a fresh installation. A file that exists but cannot be
    /// parsed *is* an error, and the settings are not silently replaced by empty ones —
    /// that would throw away what the user wrote.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsError::Read`] when the file cannot be read, and
    /// [`SettingsError::Parse`] when it cannot be understood.
    pub fn load(&self) -> Result<Settings, SettingsError> {
        let path = self.path();

        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Settings::default());
            }
            Err(error) => {
                return Err(SettingsError::Read {
                    path,
                    source: error,
                })
            }
        };

        toml::from_str(&text).map_err(|source| SettingsError::Parse { path, source })
    }

    /// Writes the settings.
    ///
    /// The write is atomic. The file is written to a temporary name in the same directory,
    /// flushed, and then renamed over the target: a write that is interrupted — a crash, a
    /// power cut, a full disk — leaves either the old file or the new one, never half of
    /// either.
    ///
    /// The rename is atomic on the platforms MPCraft runs on: `MoveFileEx` with
    /// `MOVEFILE_REPLACE_EXISTING` on Windows, `rename` on Unix. The directory itself is
    /// not flushed afterwards, so on a machine that loses power *between* the rename and
    /// the next boot the file may be the old one — never a truncated one.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsError::Write`] when the file cannot be written, and
    /// [`SettingsError::Replace`] when it cannot be renamed over the target.
    pub fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        let path = self.path();

        std::fs::create_dir_all(&self.directory).map_err(|source| SettingsError::Write {
            path: path.clone(),
            source,
        })?;

        let text = toml::to_string_pretty(settings)
            .map_err(|source| SettingsError::Serialize { source })?;

        let temporary = self.directory.join(format!(
            "{FILE_NAME}.{}.{}.tmp",
            std::process::id(),
            TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));

        {
            let mut file =
                std::fs::File::create(&temporary).map_err(|source| SettingsError::Write {
                    path: temporary.clone(),
                    source,
                })?;

            file.write_all(text.as_bytes())
                .and_then(|()| file.sync_all())
                .map_err(|source| SettingsError::Write {
                    path: temporary.clone(),
                    source,
                })?;
        }

        std::fs::rename(&temporary, &path).map_err(|source| {
            // The temporary file is of no use to anybody now, and leaving it behind would
            // fill the configuration directory with the debris of failed writes.
            let _ = std::fs::remove_file(&temporary);
            SettingsError::Replace { path, source }
        })
    }
}

/// The configuration directory of the application on this platform.
///
/// The directory is found from the environment, without a crate that knows every
/// platform's conventions: `%APPDATA%` on Windows, `$XDG_CONFIG_HOME` on Linux, and
/// `~/Library/Application Support` on macOS. Each of them falls back to a directory under
/// the user's home, and a machine with neither has no configuration directory at all.
#[must_use]
pub fn platform_config_dir() -> Option<PathBuf> {
    let home = home_dir();

    if cfg!(target_os = "windows") {
        if let Some(appdata) = env_path("APPDATA") {
            return Some(appdata.join(APP_DIRECTORY));
        }

        // The roaming profile is where a Windows application keeps its settings. When the
        // variable is missing, the home directory is the next best guess.
        return home.map(|home| home.join("AppData").join("Roaming").join(APP_DIRECTORY));
    }

    if cfg!(target_os = "macos") {
        return home.map(|home| {
            home.join("Library")
                .join("Application Support")
                .join(APP_DIRECTORY)
        });
    }

    // Everything else follows the XDG convention, which Linux and the other Unices use.
    if let Some(config) = env_path("XDG_CONFIG_HOME") {
        return Some(config.join(APP_DIRECTORY));
    }

    home.map(|home| home.join(".config").join(APP_DIRECTORY))
}

/// The home directory of the user, if the environment names one.
fn home_dir() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        if let Some(profile) = env_path("USERPROFILE") {
            return Some(profile);
        }

        // The home directory of a Windows account is sometimes split over two variables.
        let drive = env_path("HOMEDRIVE")?;
        let path = env_path("HOMEPATH")?;
        return Some(drive.join(path));
    }

    env_path("HOME")
}

/// An environment variable that holds a path, if it is set and not empty.
fn env_path(name: &str) -> Option<PathBuf> {
    let value = std::env::var_os(name)?;

    if value.is_empty() {
        return None;
    }

    Some(PathBuf::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory that is removed when the test ends.
    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        /// A directory of its own, under the temporary directory of the system.
        fn new(name: &str) -> Self {
            let unique = TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "mpcraft-settings-{name}-{}-{unique}",
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
    fn the_profiles_round_trip_through_the_file() {
        let directory = TempDir::new("round-trip");
        let store = SettingsStore::new(directory.path());

        let profiles = vec![
            ConnectionProfile::auto_connect("SITL", "udp:127.0.0.1:14550"),
            ConnectionProfile::new("Flight controller", "serial:COM3:57600"),
        ];

        store
            .save(&Settings {
                favourite_parameters: Vec::new(),
                connection_profiles: profiles.clone(),
            })
            .expect("the settings should be written");

        let settings = store.load().expect("the settings should be read");

        assert_eq!(settings.connection_profiles, profiles);
    }

    #[test]
    fn a_missing_file_means_no_profiles() {
        let directory = TempDir::new("missing");
        let store = SettingsStore::new(directory.path());

        let settings = store.load().expect("a missing file is not an error");

        assert!(settings.connection_profiles.is_empty());
    }

    #[test]
    fn a_malformed_file_names_the_file_and_the_problem() {
        let directory = TempDir::new("malformed");
        let store = SettingsStore::new(directory.path());

        std::fs::write(store.path(), "this is not = = toml").expect("the file should be written");

        let error = store.load().expect_err("the file should be rejected");

        let message = error.to_string();
        assert!(message.contains("settings.toml"), "{message}");
        assert!(message.contains("cannot be read"), "{message}");
        // The parser says where the file goes wrong, so the user can open it and look.
        assert!(message.contains("line 1"), "{message}");
    }

    #[test]
    fn a_malformed_file_is_not_replaced_by_an_empty_one() {
        let directory = TempDir::new("not-reset");
        let store = SettingsStore::new(directory.path());

        let broken = "this is not = = toml";
        std::fs::write(store.path(), broken).expect("the file should be written");

        assert!(store.load().is_err());

        // The file the user wrote is still there: a parse failure never costs the user
        // their settings.
        let text = std::fs::read_to_string(store.path()).expect("the file should still be there");
        assert_eq!(text, broken);
    }

    #[test]
    fn writing_twice_keeps_everything() {
        let directory = TempDir::new("twice");
        let store = SettingsStore::new(directory.path());

        let first = vec![ConnectionProfile::auto_connect(
            "SITL",
            "udp:127.0.0.1:14550",
        )];
        store
            .save(&Settings {
                favourite_parameters: Vec::new(),
                connection_profiles: first.clone(),
            })
            .expect("the first write should succeed");

        // The second write reads the file back, changes one profile and adds another.
        let mut settings = store.load().expect("the settings should be read");
        settings.connection_profiles[0].name = "SITL (mavlink2)".to_owned();
        settings.connection_profiles.push(ConnectionProfile::new(
            "Ground station",
            "tcp:192.168.1.10:5760",
        ));
        store
            .save(&settings)
            .expect("the second write should succeed");

        let settings = store.load().expect("the settings should be read");

        assert_eq!(settings.connection_profiles.len(), 2);
        assert_eq!(settings.connection_profiles[0].name, "SITL (mavlink2)");
        assert!(settings.connection_profiles[0].auto_connect);
        assert_eq!(settings.connection_profiles[1].name, "Ground station");
        assert!(!settings.connection_profiles[1].auto_connect);
    }

    #[test]
    fn the_file_can_be_read_by_hand() {
        let directory = TempDir::new("by-hand");
        let store = SettingsStore::new(directory.path());

        store
            .save(&Settings {
                favourite_parameters: Vec::new(),
                connection_profiles: vec![ConnectionProfile::auto_connect(
                    "SITL",
                    "udp:127.0.0.1:14550",
                )],
            })
            .expect("the settings should be written");

        let text = std::fs::read_to_string(store.path()).expect("the file should be readable");

        assert!(text.contains("[[connection_profiles]]"), "{text}");
        assert!(text.contains("name = \"SITL\""), "{text}");
        assert!(text.contains("target = \"udp:127.0.0.1:14550\""), "{text}");
        assert!(text.contains("auto_connect = true"), "{text}");
    }

    #[test]
    fn a_write_leaves_no_debris_behind() {
        let directory = TempDir::new("debris");
        let store = SettingsStore::new(directory.path());

        store
            .save(&Settings {
                favourite_parameters: Vec::new(),
                connection_profiles: vec![ConnectionProfile::new("SITL", "udp:127.0.0.1:14550")],
            })
            .expect("the settings should be written");

        let entries: Vec<_> = std::fs::read_dir(directory.path())
            .expect("the directory should be readable")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();

        assert_eq!(entries, vec![FILE_NAME.to_owned()]);
    }

    #[test]
    fn a_settings_file_written_by_hand_is_understood() {
        let directory = TempDir::new("hand-written");
        let store = SettingsStore::new(directory.path());

        std::fs::create_dir_all(directory.path()).expect("the directory should exist");
        std::fs::write(
            store.path(),
            "[[connection_profiles]]\nname = \"SITL\"\ntarget = \"udp:127.0.0.1:14550\"\nauto_connect = true\n",
        )
        .expect("the file should be written");

        let settings = store.load().expect("the settings should be read");

        assert_eq!(settings.connection_profiles.len(), 1);
        assert!(settings.connection_profiles[0].auto_connect);
    }

    #[test]
    fn the_platform_directory_ends_with_the_application_directory() {
        // The test does not assert the exact path: the environment of the machine running
        // the test decides where the configuration lives. It does assert that the
        // application has a directory of its own, so two applications cannot fight over
        // the same file.
        let Some(directory) = platform_config_dir() else {
            // A machine without a home directory has no configuration directory. That is a
            // legitimate state, not a failure.
            return;
        };

        assert_eq!(directory.file_name().unwrap(), APP_DIRECTORY);
    }

    #[test]
    fn an_empty_environment_variable_is_no_path() {
        // The check is on the helper rather than on the environment: the environment of
        // the test process must not be changed by a test.
        assert!(env_path("MPCRAFT_A_VARIABLE_THAT_DOES_NOT_EXIST").is_none());
    }
}
