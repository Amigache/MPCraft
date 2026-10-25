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

    /// A vehicle was named that the link has never seen.
    ///
    /// The choice of a vehicle is made from the boards that answered, so a pair that has never
    /// spoken cannot be chosen: there is no state behind it to read.
    #[error("the vehicle {0} was not seen on the link")]
    UnknownVehicle(mpcraft_vehicle::VehicleId),

    /// A frame was malformed.
    #[error("protocol: {0}")]
    Protocol(#[from] ProtocolError),

    /// The vehicle understood the request and refused it.
    #[error("the vehicle rejected the request: {0}")]
    Rejected(String),

    /// The vehicle cannot do what was asked, rather than refusing it at this moment.
    ///
    /// A capability the device does not have is a permanent refusal, and it is worth
    /// telling apart from a command the vehicle would have taken at another time: upstream
    /// checks the capability flags before a gimbal or camera command is sent at all.
    #[error("the vehicle cannot do this: {0}")]
    Unsupported(String),

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
            // A capability the device does not have is not a refusal at this moment: it is
            // something the vehicle can never do.
            Self::Unsupported(_) => "unsupported_by_vehicle",
            // A board that never spoke is not a vehicle to speak to: the request named something
            // that is not on the link, which is the frontend's mistake rather than the vehicle's.
            Self::UnknownVehicle(_) => "invalid_request",
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
        use mpcraft_vehicle::VehicleId;

        assert_eq!(
            LinkError::Timeout(Duration::from_secs(1)).code(),
            "link_timeout"
        );
        assert_eq!(LinkError::NoVehicle.code(), "no_vehicle");
        assert_eq!(LinkError::SessionClosed.code(), "not_connected");
        assert_eq!(
            LinkError::Unsupported("no pitch axis".to_owned()).code(),
            "unsupported_by_vehicle"
        );
        // A pair somebody typed is the caller's mistake, not the vehicle's, so it is the code the
        // frontend already branches on for a malformed request.
        assert_eq!(
            LinkError::UnknownVehicle(VehicleId::new(9, 1)).code(),
            "invalid_request"
        );
    }
}
