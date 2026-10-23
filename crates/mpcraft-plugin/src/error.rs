//! The failures the plugin sandbox can produce.
//!
//! A plugin is somebody else's code, so a failure has to say which plugin and what went wrong in a
//! way the interface can act on. The codes are stable: the frontend branches on the code, never on
//! the text.

use std::path::PathBuf;

use thiserror::Error;

use crate::capability::Capability;

/// A failure in the plugin layer.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PluginError {
    /// The manifest could not be read.
    #[error("cannot read the plugin manifest {path}: {source}")]
    Parse {
        /// The file that could not be read.
        path: PathBuf,
        /// The underlying failure.
        #[source]
        source: toml::de::Error,
    },

    /// The manifest names something that is not a capability.
    #[error("the manifest asks for `{name}`, which is not a capability the sandbox can grant")]
    UnknownCapability {
        /// What the manifest said.
        name: String,
    },

    /// The manifest asks for a rate the sandbox will not run at.
    #[error("{plugin} asks to be called {hz} times a second, which is more than {max}")]
    BadLoopRate {
        /// The plugin that asked.
        plugin: String,
        /// The rate the manifest says.
        hz: f64,
        /// The highest rate the sandbox runs a plugin at.
        max: f64,
    },

    /// The plugin imports something it was not given.
    #[error("{plugin} is not allowed to {capability}, so it cannot be loaded")]
    NotGranted {
        /// The plugin that asked.
        plugin: String,
        /// The capability its import needs.
        capability: Capability,
    },

    /// The plugin module could not be built. A module that is not valid WebAssembly fails here,
    /// and the runtime's own words are kept because they are what tells an operator which file is
    /// broken.
    #[error("the module of {plugin} cannot be built: {message}")]
    Module {
        /// The plugin whose module failed.
        plugin: String,
        /// What the runtime said.
        message: String,
    },

    /// The plugin module could not be instantiated.
    #[error("the module of {plugin} cannot be run: {message}")]
    Instantiate {
        /// The plugin that could not be run.
        plugin: String,
        /// What the runtime said.
        message: String,
    },

    /// The module asked for more of a resource than the sandbox gives it. This is caught while the
    /// module is being built, so nothing of its code ran.
    #[error("{plugin} asked for more {resource} than the sandbox gives it ({max} bytes)")]
    Denied {
        /// The plugin that asked.
        plugin: String,
        /// What it asked for.
        resource: &'static str,
        /// The ceiling the sandbox has for it.
        max: usize,
    },

    /// An export the lifecycle needs is not there.
    #[error("{plugin} does not have `{export}`, which the lifecycle needs")]
    MissingExport {
        /// The plugin that is missing it.
        plugin: String,
        /// The export that is not there.
        export: String,
    },

    /// A stage said it did not work.
    #[error("{plugin} said the stage {stage} failed (code {code})")]
    StageFailed {
        /// The plugin that said so.
        plugin: String,
        /// Which stage it was.
        stage: &'static str,
        /// The code the plugin returned.
        code: i32,
    },

    /// The plugin went deeper than the stack it was given, which is what a call that never returns
    /// does. The runtime stops it, so the application is not the thing that hangs.
    #[error("{plugin} went deeper than the {max_wasm_stack_bytes} bytes of stack it was given")]
    TooDeep {
        /// The plugin that went too deep.
        plugin: String,
        /// The stack the sandbox gives a plugin.
        max_wasm_stack_bytes: usize,
    },

    /// The plugin ran out of the work it was allowed to do. This is how a loop that never ends is
    /// stopped without stopping the application.
    #[error("{plugin} used up the {fuel} steps it was given")]
    OutOfFuel {
        /// The plugin that ran out.
        plugin: String,
        /// How many steps it had.
        fuel: u64,
    },

    /// The plugin trapped while it was running.
    #[error("{plugin} stopped while it was running: {message}")]
    Trap {
        /// The plugin that stopped.
        plugin: String,
        /// What the runtime said.
        message: String,
    },
}

impl PluginError {
    /// The stable code the interface branches on.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Parse { .. } => "plugin_parse",
            Self::UnknownCapability { .. } => "plugin_capability",
            Self::BadLoopRate { .. } => "plugin_rate",
            Self::NotGranted { .. } => "plugin_not_granted",
            Self::Module { .. } => "plugin_module",
            Self::Instantiate { .. } => "plugin_instantiate",
            Self::Denied { .. } => "plugin_limit",
            Self::MissingExport { .. } => "plugin_export",
            Self::StageFailed { .. } => "plugin_stage",
            Self::TooDeep { .. } => "plugin_depth",
            Self::OutOfFuel { .. } => "plugin_fuel",
            Self::Trap { .. } => "plugin_trap",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_parse_failure_names_the_file() {
        let error = PluginError::Parse {
            path: PathBuf::from("/home/pilot/.config/MPCraft/plugins/alarm.toml"),
            source: toml::from_str::<toml::Value>("not = [toml").unwrap_err(),
        };

        let message = error.to_string();
        assert!(message.contains("alarm.toml"), "{message}");
        assert_eq!(error.code(), "plugin_parse");
    }

    #[test]
    fn a_capability_that_was_not_granted_says_which_one_it_was() {
        let error = PluginError::NotGranted {
            plugin: "alarm".to_string(),
            capability: Capability::SendCommands,
        };

        assert_eq!(error.code(), "plugin_not_granted");
        assert!(error.to_string().contains("send_commands"), "{error}");
    }
}
