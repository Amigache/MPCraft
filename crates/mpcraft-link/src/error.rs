//! The error type of the link layer.

use std::time::Duration;

use mpcraft_mavlink::ProtocolError;

/// The failures a link operation can produce.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LinkError {
    /// The transport failed.
    #[error("transport: {0}")]
    Transport(String),

    /// The vehicle did not answer in time.
    #[error("no answer within {0:?}")]
    Timeout(Duration),

    /// The operation was cancelled.
    #[error("cancelled")]
    Cancelled,

    /// No vehicle is present on the link.
    #[error("no vehicle is present on the link")]
    NoVehicle,

    /// A frame was malformed.
    #[error("protocol: {0}")]
    Protocol(#[from] ProtocolError),

    /// The vehicle understood the request and refused it.
    #[error("the vehicle rejected the request: {0}")]
    Rejected(String),

    /// The vehicle refused a command.
    ///
    /// The command error carries the result the vehicle reported, so the caller can tell
    /// a refusal from a vehicle that never answered.
    #[error("the vehicle refused the command: {0}")]
    Command(#[from] crate::commands::CommandError),

    /// The session has been closed.
    #[error("the session is closed")]
    SessionClosed,
}

impl LinkError {
    /// Whether the operation may usefully be retried.
    ///
    /// A timeout is worth retrying; a rejection is not.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::Timeout(_) | Self::Transport(_))
    }

    /// A stable, machine-readable code for the error.
    ///
    /// The IPC layer maps these onto its own codes; the frontend branches on the code, never
    /// on the message text.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Transport(_) => "transport",
            Self::Timeout(_) => "link_timeout",
            Self::Cancelled => "cancelled",
            Self::NoVehicle => "no_vehicle",
            Self::Protocol(_) => "protocol",
            // A refused command is a refusal: the frontend already knows how to present
            // one, and the message says which command it was.
            Self::Rejected(_) | Self::Command(_) => "rejected_by_vehicle",
            Self::SessionClosed => "not_connected",
        }
    }
}

impl From<std::io::Error> for LinkError {
    fn from(error: std::io::Error) -> Self {
        Self::Transport(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeouts_are_retryable_and_rejections_are_not() {
        assert!(LinkError::Timeout(Duration::from_secs(1)).is_retryable());
        assert!(!LinkError::Rejected("no".to_owned()).is_retryable());
    }

    #[test]
    fn every_error_has_a_stable_code() {
        assert_eq!(
            LinkError::Timeout(Duration::from_secs(1)).code(),
            "link_timeout"
        );
        assert_eq!(LinkError::NoVehicle.code(), "no_vehicle");
        assert_eq!(LinkError::SessionClosed.code(), "not_connected");
    }
}
