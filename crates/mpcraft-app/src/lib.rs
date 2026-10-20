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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpcraft_link::{Session, SessionOptions};
use serde::{Deserialize, Serialize};

pub mod error;

pub use error::{CommandError, CommandResult};
pub use mpcraft_link::SessionState;

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

/// How often the supervisor publishes the state to the interface.
const PUBLISH_INTERVAL: Duration = Duration::from_millis(200);

/// A running session and the flag that stops it.
#[derive(Clone)]
struct SessionHandle {
    session: Arc<Session>,
    stopped: Arc<AtomicBool>,
}

/// Owns the link to the vehicle and publishes its state.
///
/// The supervisor is deliberately free of any interface type: it calls a callback with the
/// state, and the shell decides how to deliver it. That keeps this crate headless.
#[derive(Clone)]
pub struct SessionSupervisor {
    handle: Arc<Mutex<Option<SessionHandle>>>,
}

impl Default for SessionSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionSupervisor {
    /// Creates a supervisor with no link open.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Opens a link to a vehicle.
    ///
    /// The state is published through `publish` every [`PUBLISH_INTERVAL`], so the interface
    /// sees the link come up and the telemetry arrive.
    ///
    /// # Errors
    ///
    /// Returns an error when the target cannot be parsed or the link cannot be opened.
    pub async fn connect<F>(&self, target: &str, mut publish: F) -> CommandResult<ConnectResponse>
    where
        F: FnMut(&SessionState) + Send + 'static,
    {
        let target = target.trim();
        if target.is_empty() {
            return Err(CommandError::invalid_request(
                "the connection target is empty",
            ));
        }

        let spec = mpcraft_transport::ConnectionSpec::parse(target)
            .map_err(|error| CommandError::invalid_request(error.to_string()))?;

        publish(&SessionState::disconnected());

        let session = Session::connect(&spec, SessionOptions::default())
            .await
            .map_err(|error| CommandError::internal(error.to_string()))?;

        let session = Arc::new(session);
        let stopped = Arc::new(AtomicBool::new(false));

        // The publisher runs for as long as the session lives.
        let publisher_session = Arc::clone(&session);
        let publisher_stopped = Arc::clone(&stopped);
        let handle_stopped = Arc::clone(&stopped);
        let handle_session = Arc::clone(&session);

        tokio::spawn(async move {
            let mut last_vehicles = 0usize;

            while !publisher_stopped.load(Ordering::Relaxed) {
                let state = publisher_session.state();

                // The vehicle appearing on the link is worth a line in the log: it is the
                // moment the operator was waiting for.
                if state.vehicles.len() != last_vehicles {
                    last_vehicles = state.vehicles.len();
                    for vehicle in &state.vehicles {
                        tracing::info!(
                            "vehicle {}/{} ({}) is on the link",
                            vehicle.id.system_id,
                            vehicle.id.component_id,
                            vehicle.kind.map_or("unknown", |kind| kind.as_str()),
                        );
                    }
                }

                publish(&state);
                tokio::time::sleep(PUBLISH_INTERVAL).await;
            }
        });

        // Stopping the session stops the publisher too.
        let stop_watcher_session = Arc::clone(&handle_session);
        let stop_watcher_stopped = Arc::clone(&handle_stopped);
        tokio::spawn(async move {
            while !stop_watcher_stopped.load(Ordering::Relaxed) {
                if stop_watcher_session.is_closed() {
                    stop_watcher_stopped.store(true, Ordering::Relaxed);
                    break;
                }
                tokio::time::sleep(PUBLISH_INTERVAL).await;
            }
        });

        if let Ok(mut slot) = self.handle.lock() {
            *slot = Some(SessionHandle {
                session,
                stopped: handle_stopped,
            });
        }

        Ok(ConnectResponse {
            status: ConnectionStatus::Connecting,
            vehicle: None,
        })
    }

    /// Closes the link.
    pub async fn disconnect(&self) -> CommandResult<ConnectResponse> {
        // The handle is taken out under the lock and awaited outside it: a guard must never
        // be held across an await point.
        let handle = self.handle.lock().ok().and_then(|mut slot| slot.take());

        if let Some(handle) = handle {
            handle.stopped.store(true, Ordering::Relaxed);
            handle.session.stop().await;
        }

        Ok(ConnectResponse {
            status: ConnectionStatus::Disconnected,
            vehicle: None,
        })
    }

    /// The state of the current session, if there is one.
    #[must_use]
    pub fn state(&self) -> Option<SessionState> {
        self.handle
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(|handle| handle.session.state()))
    }
}

/// The state the application keeps between commands.
#[derive(Clone, Default)]
pub struct AppState {
    /// The supervisor that owns the link.
    pub supervisor: SessionSupervisor,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_empty_target_is_rejected() {
        let supervisor = SessionSupervisor::new();
        let error = supervisor
            .connect("   ", |_| {})
            .await
            .expect_err("the target should be rejected");

        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);
    }

    #[tokio::test]
    async fn a_malformed_target_is_rejected() {
        let supervisor = SessionSupervisor::new();
        let error = supervisor
            .connect("carrier-pigeon:COM3", |_| {})
            .await
            .expect_err("the target should be rejected");

        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);
    }

    #[tokio::test]
    async fn disconnecting_without_a_link_is_harmless() {
        let supervisor = SessionSupervisor::new();
        let response = supervisor.disconnect().await.unwrap();
        assert_eq!(response.status, ConnectionStatus::Disconnected);
    }

    #[test]
    fn the_error_codes_are_stable() {
        assert_eq!(CommandError::CODE_TIMEOUT, "link_timeout");
        assert_eq!(CommandError::CODE_NO_VEHICLE, "no_vehicle");
        assert_eq!(CommandError::CODE_NOT_CONNECTED, "not_connected");
    }
}
