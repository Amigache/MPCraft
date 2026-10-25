//! The error type exposed across the IPC boundary.

use serde::{Deserialize, Serialize};

/// The result of a command.
pub type CommandResult<T> = Result<T, CommandError>;

/// A failure surfaced to the frontend.
///
/// The `code` is a stable, machine-readable identifier. The frontend branches on it; the
/// message is for humans only and may be reworded or localised without breaking anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandError {
    /// The stable error code.
    pub code: String,
    /// A human-readable description.
    pub message: String,
}

impl CommandError {
    /// The link did not answer within the allotted time.
    pub const CODE_TIMEOUT: &'static str = "link_timeout";
    /// No vehicle is present on the link.
    pub const CODE_NO_VEHICLE: &'static str = "no_vehicle";
    /// There is no open link.
    pub const CODE_NOT_CONNECTED: &'static str = "not_connected";
    /// The vehicle understood the request and refused it.
    pub const CODE_REJECTED: &'static str = "rejected_by_vehicle";
    /// The request was malformed.
    pub const CODE_INVALID_REQUEST: &'static str = "invalid_request";
    /// The settings could not be read or written.
    pub const CODE_SETTINGS: &'static str = "settings_error";
    /// Something went wrong that has no more specific code.
    pub const CODE_INTERNAL: &'static str = "internal";

    /// Creates an error with the given code and message.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Creates a `link_timeout` error.
    #[must_use]
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(Self::CODE_TIMEOUT, message)
    }

    /// Creates a `no_vehicle` error.
    #[must_use]
    pub fn no_vehicle(message: impl Into<String>) -> Self {
        Self::new(Self::CODE_NO_VEHICLE, message)
    }

    /// Creates a `not_connected` error.
    #[must_use]
    pub fn not_connected() -> Self {
        Self::new(
            Self::CODE_NOT_CONNECTED,
            "there is no open connection to the vehicle",
        )
    }

    /// Creates a `rejected_by_vehicle` error.
    #[must_use]
    pub fn rejected(message: impl Into<String>) -> Self {
        Self::new(Self::CODE_REJECTED, message)
    }

    /// Creates an `invalid_request` error.
    #[must_use]
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(Self::CODE_INVALID_REQUEST, message)
    }

    /// Creates a `settings_error` error.
    #[must_use]
    pub fn settings(message: impl Into<String>) -> Self {
        Self::new(Self::CODE_SETTINGS, message)
    }

    /// Creates an `internal` error.
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(Self::CODE_INTERNAL, message)
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

impl From<mpcraft_link::LinkError> for CommandError {
    fn from(error: mpcraft_link::LinkError) -> Self {
        Self::new(error.code(), error.to_string())
    }
}

impl From<crate::adsb_relay::RelayError> for CommandError {
    fn from(error: crate::adsb_relay::RelayError) -> Self {
        Self::new(error.code(), error.to_string())
    }
}

impl From<crate::swarm::SwarmError> for CommandError {
    fn from(error: crate::swarm::SwarmError) -> Self {
        Self::new(error.code(), error.to_string())
    }
}

impl From<mpcraft_plugin::PluginError> for CommandError {
    fn from(error: mpcraft_plugin::PluginError) -> Self {
        Self::new(error.code(), error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(CommandError::timeout("x").code, "link_timeout");
        assert_eq!(CommandError::no_vehicle("x").code, "no_vehicle");
        assert_eq!(CommandError::not_connected().code, "not_connected");
        assert_eq!(CommandError::rejected("x").code, "rejected_by_vehicle");
        assert_eq!(CommandError::invalid_request("x").code, "invalid_request");
        assert_eq!(CommandError::settings("x").code, "settings_error");
        assert_eq!(CommandError::internal("x").code, "internal");
    }

    #[test]
    fn errors_serialise_with_a_code_and_a_message() {
        let error = CommandError::timeout("no answer within 2s");
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(json["code"], "link_timeout");
        assert_eq!(json["message"], "no answer within 2s");
    }

    #[test]
    fn a_link_error_keeps_its_code() {
        let error = CommandError::from(mpcraft_link::LinkError::Timeout(
            std::time::Duration::from_secs(1),
        ));
        assert_eq!(error.code, "link_timeout");
    }

    #[test]
    fn a_plugin_failure_keeps_its_code() {
        let error = CommandError::from(mpcraft_plugin::PluginError::UnknownCapability {
            name: "write_parameters".to_string(),
        });

        assert_eq!(error.code, "plugin_capability");
    }
}
