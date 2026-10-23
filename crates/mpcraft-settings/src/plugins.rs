//! The plugins the operator has chosen to run.
//!
//! A plugin is somebody else's code. It is not run because its file happens to be in the folder: the
//! operator has to say so here, in the same file that holds the connection profiles. An entry that is
//! not in the list is a plugin that is present and not run, which is the state a fresh installation
//! is in — and it is the safe one, because a program that ran whatever it found in a folder would be
//! running code nobody agreed to.
//!
//! What a plugin *wants* is not kept here. That is in the manifest, which the author wrote. What is
//! kept here is what the operator decided, and the two are kept apart so that a manifest cannot
//! grant itself anything.

use serde::{Deserialize, Serialize};

/// One plugin as the operator's file remembers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct PluginEntry {
    /// The name the manifest says, which is what the management sheet lists the plugin under.
    pub name: String,

    /// Whether the plugin is run.
    ///
    /// The key is optional: a plugin that was seen but never turned on is remembered without being
    /// run, which is what a person who dropped a module into the folder and then thought about it
    /// has written.
    #[serde(default)]
    pub enabled: bool,
}

impl PluginEntry {
    /// A plugin that is remembered and not run.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            enabled: false,
        }
    }

    /// A plugin the operator has turned on.
    #[must_use]
    pub fn enabled(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_without_the_enabled_key_is_valid_and_not_run() {
        let entry: PluginEntry = toml::from_str("name = \"alarm\"\n").unwrap();

        assert_eq!(entry.name, "alarm");
        assert!(!entry.enabled, "a plugin nobody chose is not run");
    }

    #[test]
    fn a_misspelled_key_is_rejected() {
        let error = toml::from_str::<PluginEntry>("name = \"alarm\"\nenabled = tru\n").unwrap_err();

        assert!(error.to_string().contains("enabled"), "{error}");
    }
}
