//! The MPCraft desktop application.
//!
//! This binary is deliberately **thin**. It wires Tauri commands to [`mpcraft_app`] and owns
//! nothing else: no business logic, no protocol knowledge, no state.
//!
//! If logic appears here, it belongs in `mpcraft-app` — which keeps the core headless and
//! testable without a display server.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use mpcraft_app::{
    AppState, CommandResult, ConnectRequest, ConnectResponse, ConnectionStatus, LinkSnapshot,
    SessionState,
};
use tauri::{AppHandle, Emitter, Manager, State};

/// The event that carries the state of the link and the telemetry.
const STATE_EVENT: &str = "link-state";

/// Opens a link to a vehicle.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the target is invalid or the link cannot
/// be opened.
#[tauri::command]
async fn connect(
    app: AppHandle,
    state: State<'_, AppState>,
    req: ConnectRequest,
) -> CommandResult<ConnectResponse> {
    let handle = app.clone();

    state
        .supervisor
        .connect(&req.target, move |snapshot| {
            if let Err(error) = handle.emit(STATE_EVENT, StateEvent::from(snapshot)) {
                tracing::warn!("could not publish the state: {error}");
            }
        })
        .await
}

/// Closes the link to the vehicle.
///
/// # Errors
///
/// Returns a `CommandError` when the link cannot be closed.
#[tauri::command]
async fn disconnect(app: AppHandle, state: State<'_, AppState>) -> CommandResult<ConnectResponse> {
    let response = state.supervisor.disconnect().await?;
    let _ = app.emit(STATE_EVENT, StateEvent::default());
    Ok(response)
}

/// The current connection status.
///
/// # Errors
///
/// Never fails.
#[tauri::command]
async fn connection_status(state: State<'_, AppState>) -> CommandResult<ConnectionStatus> {
    Ok(state.supervisor.status())
}

/// The connection profiles the user has saved.
///
/// The fields are `name`, `target` and `autoConnect`, like every other payload the shell
/// emits.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the profiles cannot be read.
#[tauri::command]
async fn list_connection_profiles(state: State<'_, AppState>) -> CommandResult<Vec<ProfileEvent>> {
    let profiles = state.connection_profiles()?;

    Ok(profiles
        .iter()
        .map(|profile| ProfileEvent {
            name: profile.name.clone(),
            target: profile.target.clone(),
            auto_connect: profile.auto_connect,
        })
        .collect())
}

/// A connection profile, as the interface sees it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileEvent {
    /// The name the profile is listed under.
    name: String,
    /// The connection target.
    target: String,
    /// Whether the profile is opened when the application starts.
    auto_connect: bool,
}

/// The payload of the state event.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StateEvent {
    /// The connection state.
    status: ConnectionStatus,
    /// The vehicles seen on the link.
    vehicles: Vec<VehicleEvent>,
    /// The telemetry of the primary vehicle.
    telemetry: TelemetryEvent,
    /// The loss statistics of the link.
    loss: LossEvent,
}

/// A vehicle on the link.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct VehicleEvent {
    /// The system id.
    system_id: u8,
    /// The component id.
    component_id: u8,
    /// The vehicle family, when it is known.
    kind: Option<String>,
}

/// The telemetry of the primary vehicle.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryEvent {
    /// The position, when it is known.
    position: Option<PositionEvent>,
    /// The attitude, when it is known.
    attitude: Option<AttitudeEvent>,
    /// The velocity, when it is known.
    velocity: Option<VelocityEvent>,
    /// The battery, when it is known.
    battery: Option<BatteryEvent>,
    /// The flight mode, when it is known.
    mode: Option<String>,
    /// Whether the vehicle is armed.
    armed: Option<bool>,
    /// The distance to home, in metres.
    distance_to_home: Option<f64>,
}

/// A position.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PositionEvent {
    /// The latitude, in degrees.
    lat: f64,
    /// The longitude, in degrees.
    lng: f64,
    /// The altitude above mean sea level, in metres.
    alt: f64,
}

/// An attitude.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AttitudeEvent {
    /// The roll, in degrees.
    roll: f64,
    /// The pitch, in degrees.
    pitch: f64,
    /// The yaw, in degrees.
    yaw: f64,
}

/// A velocity.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct VelocityEvent {
    /// The ground speed, in metres per second.
    ground_speed: f64,
}

/// A battery.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BatteryEvent {
    /// The voltage, in volts.
    voltage: f64,
    /// The current, in amperes.
    current: f64,
    /// The remaining charge, as a percentage.
    remaining: f64,
}

/// The loss statistics of the link.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LossEvent {
    /// The number of packets received.
    received: u64,
    /// The number of packets lost.
    lost: u64,
}

impl From<&LinkSnapshot> for StateEvent {
    fn from(snapshot: &LinkSnapshot) -> Self {
        let state = snapshot.session.as_ref();

        Self {
            status: snapshot.status,
            vehicles: state
                .map(|state| {
                    state
                        .vehicles
                        .iter()
                        .map(|vehicle| VehicleEvent {
                            system_id: vehicle.id.system_id,
                            component_id: vehicle.id.component_id,
                            kind: vehicle.kind.map(|kind| kind.as_str().to_owned()),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            telemetry: state.map(telemetry_event).unwrap_or_default(),
            loss: state
                .map(|state| LossEvent {
                    received: state.loss.packets_received,
                    lost: state.loss.packets_lost,
                })
                .unwrap_or_default(),
        }
    }
}

/// The telemetry of a session, as the interface sees it.
fn telemetry_event(state: &SessionState) -> TelemetryEvent {
    let telemetry = &state.telemetry;

    TelemetryEvent {
        position: telemetry.position.map(|position| PositionEvent {
            lat: position.lat,
            lng: position.lng,
            alt: position.alt,
        }),
        attitude: telemetry.attitude.map(|attitude| AttitudeEvent {
            roll: attitude.roll.to_degrees(),
            pitch: attitude.pitch.to_degrees(),
            yaw: attitude.yaw.to_degrees(),
        }),
        velocity: telemetry.velocity.as_ref().map(|velocity| VelocityEvent {
            ground_speed: velocity.ground_speed,
        }),
        battery: telemetry.battery.map(|battery| BatteryEvent {
            voltage: battery.voltage,
            current: battery.current,
            remaining: battery.remaining,
        }),
        mode: telemetry.mode.as_ref().map(|mode| {
            mode.name
                .clone()
                .unwrap_or_else(|| format!("mode {}", mode.custom_mode))
        }),
        armed: telemetry.armed,
        distance_to_home: telemetry.distance_to_home,
    }
}

impl StateEvent {
    /// The link state, as a label.
    fn status_label(&self) -> &'static str {
        match self.status {
            ConnectionStatus::Disconnected => "desconectado",
            ConnectionStatus::Connecting => "conectando",
            ConnectionStatus::Connected => "conectado",
            ConnectionStatus::Reconnecting => "reconectando",
            ConnectionStatus::Failed => "fallo",
        }
    }
}

impl Default for StateEvent {
    fn default() -> Self {
        Self::from(&LinkSnapshot {
            status: ConnectionStatus::Disconnected,
            session: None,
        })
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

    // A ground station is often launched from a shortcut that points at a particular
    // vehicle, so the link target can be given on the command line.
    let target = target_from_arguments();

    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            connect,
            disconnect,
            connection_status,
            list_connection_profiles
        ])
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("MPCraft");
            }

            // The state is cloned rather than borrowed: the borrow of the application
            // cannot outlive the setup closure.
            let state = app.state::<AppState>().inner().clone();

            // The saved profiles reach the supervisor too: a profile marked for automatic
            // connection is opened when the command line does not ask for a link.
            let profiles = match state.connection_profiles() {
                Ok(profiles) => profiles,
                Err(error) => {
                    tracing::warn!("could not read the connection profiles: {error}");
                    Vec::new()
                }
            };

            let target = mpcraft_app::launch_target(target.as_deref(), &profiles);

            // Connect to the vehicle the launch asked for, once the window is up.
            if let Some(target) = target {
                let handle = app.handle().clone();

                tauri::async_runtime::spawn(async move {
                    let emitter = handle.clone();
                    let titler = handle.clone();
                    let result = state
                        .supervisor
                        .connect(&target, move |snapshot| {
                            // The title carries the link state, so it is visible even
                            // without the interface in front.
                            if let Some(window) = titler.get_webview_window("main") {
                                let _ = window.set_title(&format!(
                                    "MPCraft \u{2014} {}",
                                    StateEvent::from(snapshot).status_label()
                                ));
                            }

                            if let Err(error) =
                                emitter.emit(STATE_EVENT, StateEvent::from(snapshot))
                            {
                                tracing::warn!("could not publish the state: {error}");
                            }
                        })
                        .await;

                    match result {
                        Ok(response) => {
                            tracing::info!("connecting to {target}: {:?}", response.status)
                        }
                        Err(error) => tracing::warn!("could not connect to {target}: {error}"),
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start the MPCraft application");
}

/// The link target given on the command line, if there is one.
fn target_from_arguments() -> Option<String> {
    let mut arguments = std::env::args().skip(1);

    while let Some(argument) = arguments.next() {
        if argument == "--target" {
            return arguments.next();
        }
        if let Some(value) = argument.strip_prefix("--target=") {
            return Some(value.to_owned());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snapshot_without_a_session_has_no_vehicles() {
        let event = StateEvent::default();

        assert_eq!(event.status, ConnectionStatus::Disconnected);
        assert!(event.vehicles.is_empty());
        assert!(event.telemetry.position.is_none());
    }

    #[test]
    fn the_state_of_a_link_is_carried_by_the_event() {
        let event = StateEvent::from(&LinkSnapshot {
            status: ConnectionStatus::Reconnecting,
            session: None,
        });

        assert_eq!(event.status, ConnectionStatus::Reconnecting);
        assert_eq!(event.status_label(), "reconectando");
    }
}
