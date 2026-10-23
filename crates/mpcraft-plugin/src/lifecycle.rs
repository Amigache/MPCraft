//! The stages a plugin is run through.
//!
//! Upstream calls a plugin four times: once when it is read, once when the link is open, once per
//! tick, and once when the application closes. These are the same four, with two things changed on
//! purpose: the tick has a rate that the manifest says and the sandbox caps, and a stage that says
//! it did not work is an error rather than a boolean somebody forgets to check.

use std::fmt;

use serde::{Deserialize, Serialize};

/// One stage of the lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// The plugin is read. It has no link yet, so this is where it decides whether it can work at
    /// all. This stage is required: a plugin without it is not a plugin.
    Init,

    /// The link is open. Optional: a plugin that only works on its own has nothing to do here.
    Loaded,

    /// A tick. Optional, because plenty of plugins have nothing to do every second.
    Loop,

    /// The application is closing. Optional, but a plugin that holds a file has to be told.
    Exit,
}

impl Stage {
    /// The export a module has to have for this stage.
    #[must_use]
    pub fn export(self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Loaded => "loaded",
            Self::Loop => "loop",
            Self::Exit => "exit",
        }
    }

    /// The name a message calls this stage by.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Init => "Init",
            Self::Loaded => "Loaded",
            Self::Loop => "Loop",
            Self::Exit => "Exit",
        }
    }

    /// Whether a plugin can be run without this stage.
    #[must_use]
    pub fn is_required(self) -> bool {
        matches!(self, Self::Init)
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_stage_is_required() {
        assert!(Stage::Init.is_required());

        for stage in [Stage::Loaded, Stage::Loop, Stage::Exit] {
            assert!(!stage.is_required(), "{stage} should be optional");
        }
    }

    #[test]
    fn a_stage_names_the_export_it_looks_for() {
        assert_eq!(Stage::Init.export(), "init");
        assert_eq!(Stage::Loop.export(), "loop");
    }
}
