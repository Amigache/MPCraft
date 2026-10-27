//! What a plugin may be given.
//!
//! A plugin is code from somebody else. It is not trusted, so it does not get authority over the
//! application: it gets a list of things it may ask for, written down where a person can read it.
//! A capability is decided when the plugin is loaded and cannot be asked for afterwards — a module
//! that imports something it was not given is never instantiated at all, which is stronger than
//! checking each call.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::PluginError;

/// One thing a plugin may be allowed to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Capability {
    /// What the vehicle last said: altitude, speed, heading, battery.
    ReadTelemetry,

    /// Asking the vehicle to do something. The asking goes through the link like anything else,
    /// so a plugin cannot hold the link while it waits for an answer.
    SendCommands,

    /// Reading the plan the vehicle holds. Writing to it is not a capability: a plugin that wants
    /// to change a plan has to say so to the operator, who decides.
    MissionAccess,

    /// The parameters the vehicle has reported. Reading them and changing one are two names, because
    /// a plugin that can read a value is not a plugin that can change it.
    ReadParams,

    /// Changing a parameter. The asking is queued rather than sent from inside the sandbox, so that a
    /// plugin cannot hold the link while it waits for the vehicle to echo what it stored.
    WriteParams,

    /// Saying something to the operator, which shows up in the message console.
    Notify,
}

impl Capability {
    /// The name the manifest uses for a capability.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::ReadTelemetry => "read_telemetry",
            Self::SendCommands => "send_commands",
            Self::MissionAccess => "mission_access",
            Self::ReadParams => "read_params",
            Self::WriteParams => "write_params",
            Self::Notify => "notify",
        }
    }
}

/// The name a message uses for a capability, which is the same spelling a manifest writes it in.
impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Capability {
    type Err = PluginError;

    /// A capability that is not one of these is refused rather than guessed at, because a manifest
    /// that says `telemetry` when it means `read_telemetry` is a manifest that promises something
    /// the sandbox cannot grant.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.trim() {
            "read_telemetry" | "telemetry_read" => Ok(Self::ReadTelemetry),
            "send_commands" | "command_send" => Ok(Self::SendCommands),
            "mission_access" => Ok(Self::MissionAccess),
            "read_params" | "params_read" => Ok(Self::ReadParams),
            "write_params" | "params_write" => Ok(Self::WriteParams),
            "notify" => Ok(Self::Notify),
            other => Err(PluginError::UnknownCapability {
                name: other.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capability_names_itself_the_way_a_manifest_writes_it() {
        assert_eq!(Capability::ReadTelemetry.name(), "read_telemetry");
        assert_eq!(
            Capability::from_str("read_telemetry").unwrap(),
            Capability::ReadTelemetry
        );
    }

    #[test]
    fn reading_a_parameter_and_changing_one_are_two_names() {
        assert_eq!(Capability::ReadParams.name(), "read_params");
        assert_eq!(
            Capability::from_str("read_params").unwrap(),
            Capability::ReadParams
        );
        assert_eq!(Capability::WriteParams.name(), "write_params");
        assert_eq!(
            Capability::from_str("write_params").unwrap(),
            Capability::WriteParams
        );

        // A plugin given the read is not a plugin given the write, and the two are not one name with
        // two spellings: `params` says nothing that the sandbox can grant.
        assert!(matches!(
            Capability::from_str("params").unwrap_err(),
            PluginError::UnknownCapability { .. }
        ));
    }

    #[test]
    fn a_capability_that_is_not_one_of_these_is_refused() {
        let error = Capability::from_str("write_parameters").unwrap_err();

        assert!(
            matches!(error, PluginError::UnknownCapability { .. }),
            "{error}"
        );
    }
}
