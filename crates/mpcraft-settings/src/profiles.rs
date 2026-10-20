//! The connection profiles the user can come back to.

use serde::{Deserialize, Serialize};

/// A named connection the user can come back to.
///
/// The profile is deliberately small: a name to show in a list, the connection target, and
/// whether the link is opened when the application starts. Everything else about a
/// connection belongs to the target itself, which is a string the transport layer
/// understands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ConnectionProfile {
    /// The name the profile is listed under.
    pub name: String,

    /// The connection target, for example `udp:127.0.0.1:14550`.
    pub target: String,

    /// Whether the profile is opened when the application starts.
    ///
    /// The key is optional in the file: a profile that does not mention it is not opened at
    /// startup. A user who writes a profile by hand should not have to spell out the
    /// default to have a valid file.
    ///
    /// The camel-case spelling is accepted as well, because the file is meant to be edited
    /// by hand and by tools that do not agree on the convention.
    #[serde(default, alias = "autoConnect")]
    pub auto_connect: bool,
}

impl ConnectionProfile {
    /// A profile that is not opened at startup.
    #[must_use]
    pub fn new(name: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            target: target.into(),
            auto_connect: false,
        }
    }

    /// A profile that is opened at startup.
    #[must_use]
    pub fn auto_connect(name: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            target: target.into(),
            auto_connect: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_without_the_auto_connect_key_is_valid() {
        let profile: ConnectionProfile =
            toml::from_str("name = \"SITL\"\ntarget = \"udp:127.0.0.1:14550\"\n").unwrap();

        assert_eq!(profile.name, "SITL");
        assert!(!profile.auto_connect);
    }

    #[test]
    fn a_misspelled_key_is_rejected() {
        let error = toml::from_str::<ConnectionProfile>(
            "name = \"SITL\"\ntarget = \"udp:127.0.0.1:14550\"\nautoconnect = true\n",
        )
        .unwrap_err();

        assert!(error.to_string().contains("autoconnect"), "{error}");
    }
}
