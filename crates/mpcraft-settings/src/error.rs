//! The failures the settings store can produce.

use std::path::PathBuf;

/// A failure while reading or writing the settings.
///
/// Every variant names the file it is about. A user who edits the settings by hand — which
/// is the whole point of the format — has to be told *which* file is wrong and *what* is
/// wrong with it, without having to guess.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SettingsError {
    /// The settings file could not be read.
    #[error("cannot read the settings file {path}: {source}")]
    Read {
        /// The file that could not be read.
        path: PathBuf,
        /// The underlying failure.
        #[source]
        source: std::io::Error,
    },

    /// The settings file is not valid TOML, or does not have the expected shape.
    #[error("the settings file {path} cannot be read: {source}")]
    Parse {
        /// The file that could not be parsed.
        path: PathBuf,
        /// The underlying failure.
        #[source]
        source: toml::de::Error,
    },

    /// The settings could not be turned into TOML.
    #[error("the settings cannot be written as TOML: {source}")]
    Serialize {
        /// The underlying failure.
        #[source]
        source: toml::ser::Error,
    },

    /// The settings file could not be written.
    #[error("cannot write the settings file {path}: {source}")]
    Write {
        /// The file that could not be written.
        path: PathBuf,
        /// The underlying failure.
        #[source]
        source: std::io::Error,
    },

    /// The settings file could not be replaced by the newly written one.
    #[error("cannot replace the settings file {path}: {source}")]
    Replace {
        /// The file that could not be replaced.
        path: PathBuf,
        /// The underlying failure.
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_parse_failure_names_the_file() {
        let error = SettingsError::Parse {
            path: PathBuf::from("/home/pilot/.config/MPCraft/settings.toml"),
            source: toml::from_str::<toml::Value>("not = [toml").unwrap_err(),
        };

        let message = error.to_string();
        assert!(message.contains("settings.toml"), "{message}");
        assert!(message.contains("cannot be read"), "{message}");
    }
}
