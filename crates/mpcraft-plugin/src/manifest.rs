//! What a plugin says about itself.
//!
//! The manifest is a file a person can read and edit: the name, the module, what the plugin wants to
//! be allowed to do, and how often it wants to be called. The capabilities are not a wish list — they
//! are what the sandbox grants, and a manifest that asks for something else is refused at reading
//! time rather than being quietly ignored.
//!
//! A capability is resolved from the name the file writes, which is why the file is read as strings
//! first: a name the sandbox cannot grant has to become an error that names it, and a parser that
//! refuses an unknown variant only says "unknown variant", which is not enough for somebody fixing
//! the file.

use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::capability::Capability;
use crate::error::PluginError;

/// The highest rate a plugin is called at, in times a second.
///
/// A plugin that asks for more is not faster: it is a plugin that will starve the telemetry tick,
/// which is the thing the whole application is built around.
pub const MAX_LOOP_HZ: f64 = 20.0;

/// A plugin as its own file describes it.
// A manifest holds a rate, which is a number that cannot be compared for equality, so this derives
// `PartialEq` and not `Eq`: two manifests with the same rate are the same manifest, and nothing here
// needs them to be a key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct PluginManifest {
    /// The name the plugin is listed under.
    pub name: String,

    /// The version the plugin says it is.
    pub version: String,

    /// Who wrote it, which is what an operator needs before deciding to run it.
    #[serde(default)]
    pub author: String,

    /// The WebAssembly module the plugin is read from, relative to the folder of the manifest.
    pub module: PathBuf,

    /// What the plugin is allowed to do.
    #[serde(default)]
    pub capabilities: Vec<Capability>,

    /// How often the plugin is called, once per tick.
    #[serde(default = "default_loop_hz")]
    pub loop_hz: f64,
}

fn default_loop_hz() -> f64 {
    1.0
}

/// The same file before its capability names are resolved, so that a name that is not one can be
/// reported as itself rather than as a parse failure.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
struct RawManifest {
    name: String,
    version: String,
    #[serde(default)]
    author: String,
    module: PathBuf,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default = "default_loop_hz")]
    loop_hz: f64,
}

impl PluginManifest {
    /// Reads a manifest and says which file it came from when it is not one.
    pub fn from_text(text: &str, path: PathBuf) -> Result<Self, PluginError> {
        let raw: RawManifest = toml::from_str(text).map_err(|source| PluginError::Parse {
            path: path.clone(),
            source,
        })?;

        if raw.loop_hz < 0.0 || raw.loop_hz > MAX_LOOP_HZ {
            return Err(PluginError::BadLoopRate {
                plugin: raw.name,
                hz: raw.loop_hz,
                max: MAX_LOOP_HZ,
            });
        }

        let capabilities = raw
            .capabilities
            .iter()
            .map(|name| Capability::from_str(name))
            .collect::<Result<Vec<Capability>, PluginError>>()?;

        Ok(Self {
            name: raw.name,
            version: raw.version,
            author: raw.author,
            module: raw.module,
            capabilities,
            loop_hz: raw.loop_hz,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manifest_is_read_as_a_person_writes_it() {
        let manifest = PluginManifest::from_text(
            "name = \"alarm\"\nversion = \"1.0.0\"\nauthor = \"Ana\"\nmodule = \"alarm.wasm\"\ncapabilities = [\"read_telemetry\", \"notify\"]\n",
            PathBuf::from("alarm.toml"),
        )
        .unwrap();

        assert_eq!(manifest.name, "alarm");
        assert_eq!(
            manifest.capabilities,
            vec![Capability::ReadTelemetry, Capability::Notify]
        );
        assert_eq!(manifest.loop_hz, 1.0);
    }

    #[test]
    fn a_capability_the_sandbox_cannot_grant_is_refused_while_reading() {
        let error = PluginManifest::from_text(
            "name = \"alarm\"\nversion = \"1.0.0\"\nmodule = \"alarm.wasm\"\ncapabilities = [\"write_parameters\"]\n",
            PathBuf::from("alarm.toml"),
        )
        .unwrap_err();

        assert_eq!(error.code(), "plugin_capability", "{error}");
        // The name the file wrote is kept, because that is what the person editing the file has to
        // change.
        assert!(error.to_string().contains("write_parameters"), "{error}");
    }

    #[test]
    fn a_rate_above_the_tick_is_refused() {
        let error = PluginManifest::from_text(
            "name = \"alarm\"\nversion = \"1.0.0\"\nmodule = \"alarm.wasm\"\nloop_hz = 60\n",
            PathBuf::from("alarm.toml"),
        )
        .unwrap_err();

        assert_eq!(error.code(), "plugin_rate", "{error}");
    }

    #[test]
    fn a_key_that_is_not_part_of_a_manifest_is_rejected() {
        let error = PluginManifest::from_text(
            "name = \"alarm\"\nversion = \"1.0.0\"\nmodule = \"alarm.wasm\"\nauthor = \"Ana\"\ntrusted = true\n",
            PathBuf::from("alarm.toml"),
        )
        .unwrap_err();

        assert!(error.to_string().contains("trusted"), "{error}");
    }
}
