//! Application orchestration for MPCraft.
//!
//! This is the only crate that knows about the application as a whole: the session
//! supervisor, the event bus, the command surface, and the auto-reconnect policy.
//!
//! It is the only library crate permitted to depend on Tauri types — and even here, the Tauri
//! dependency lives in `apps/desktop`, so `mpcraft-app` stays headless and testable.
//!
//! # The IPC contract
//!
//! Every command returns `Result<T, CommandError>`, where [`CommandError`] carries a stable
//! snake_case `code`. The frontend branches on the code, never on the message text: the text
//! is for humans and may be localised or reworded.
//!
//! Telemetry is pushed as **one batched snapshot per tick**, never as one event per field.
//! At 20 Hz with forty fields, per-field events would mean eight hundred IPC messages per
//! second.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};

pub mod error;

pub use error::{CommandError, CommandResult};

/// The connection state of the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    /// No link is open.
    #[default]
    Disconnected,
    /// A link is being opened.
    Connecting,
    /// A vehicle is answering.
    Connected,
    /// The link dropped and is being restored.
    Reconnecting,
    /// The link could not be opened.
    Failed,
}

/// Identifies a vehicle on a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VehicleKey {
    /// The system id of the vehicle.
    pub system_id: u8,
    /// The component id of the vehicle.
    pub component_id: u8,
}

impl VehicleKey {
    /// Creates a vehicle key.
    #[must_use]
    pub const fn new(system_id: u8, component_id: u8) -> Self {
        Self {
            system_id,
            component_id,
        }
    }
}

/// The request payload of the `connect` command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectRequest {
    /// The connection target, for example `udp:127.0.0.1:14550`.
    pub target: String,
}

/// The result of a command that changed the connection state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectResponse {
    /// The new connection state.
    pub status: ConnectionStatus,
    /// The vehicle that answered, if any.
    pub vehicle: Option<VehicleKey>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_status_serialises_as_snake_case() {
        let json = serde_json::to_string(&ConnectionStatus::Reconnecting).unwrap();
        assert_eq!(json, "\"reconnecting\"");
    }

    #[test]
    fn vehicle_keys_round_trip() {
        let key = VehicleKey::new(1, 1);
        let json = serde_json::to_string(&key).unwrap();
        let restored: VehicleKey = serde_json::from_str(&json).unwrap();
        assert_eq!(key, restored);
    }
}
