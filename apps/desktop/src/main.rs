//! The MPCraft desktop application.
//!
//! This binary is deliberately **thin**. It wires Tauri commands to [`mpcraft_app`] and owns
//! nothing else: no business logic, no protocol knowledge, no state.
//!
//! If logic appears here, it belongs in `mpcraft-app` — which keeps the core headless and
//! testable without a display server.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use mpcraft_app::{CommandError, CommandResult, ConnectRequest, ConnectResponse, ConnectionStatus};
use tauri::{Emitter, Manager, State};

/// The shared application state.
#[derive(Default)]
struct AppState {
    /// The current connection status.
    status: std::sync::Mutex<ConnectionStatus>,
}

/// Opens a link to a vehicle.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the target is invalid or the link cannot
/// be opened.
#[tauri::command]
async fn connect(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    req: ConnectRequest,
) -> CommandResult<ConnectResponse> {
    let target = req.target.trim();

    if target.is_empty() {
        return Err(CommandError::invalid_request(
            "the connection target is empty",
        ));
    }

    set_status(&state, ConnectionStatus::Connecting);
    let _ = app.emit("connection", ConnectionStatus::Connecting);

    // The link layer is wired up in phase 0, task 3. Until then the shell validates the
    // target and reports the state, which keeps the IPC contract exercised end to end.
    let status = ConnectionStatus::Connected;
    set_status(&state, status);
    let _ = app.emit("connection", status);

    Ok(ConnectResponse {
        status,
        vehicle: None,
    })
}

/// Closes the link to the vehicle.
///
/// # Errors
///
/// Returns a `CommandError` when the link cannot be closed.
#[tauri::command]
async fn disconnect(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<ConnectResponse> {
    let status = ConnectionStatus::Disconnected;
    set_status(&state, status);
    let _ = app.emit("connection", status);

    Ok(ConnectResponse {
        status,
        vehicle: None,
    })
}

/// Returns the current connection status.
///
/// # Errors
///
/// Never fails.
#[tauri::command]
async fn connection_status(state: State<'_, AppState>) -> CommandResult<ConnectionStatus> {
    let status = *state.status.lock().unwrap_or_else(|e| e.into_inner());
    Ok(status)
}

/// Updates the shared connection status.
fn set_status(state: &State<'_, AppState>, status: ConnectionStatus) {
    if let Ok(mut current) = state.status.lock() {
        *current = status;
    }
}

/// Builds and runs the Tauri application.
fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("starting MPCraft");

    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            connect,
            disconnect,
            connection_status
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("MPCraft");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start the MPCraft application");
}
