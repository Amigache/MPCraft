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
    flight_modes, AppState, CommandError, CommandResult, ConnectRequest, ConnectResponse,
    ConnectionStatus, LinkSnapshot, ModeInfo, PlanInfo, PlanKind, PresetInfo, SeriesData,
    SessionState,
};
use mpcraft_link::VehicleId;
use mpcraft_mission::{MissionType, Plan};
use mpcraft_transport::SerialTransport;
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
    let history = state.graphs.clone();

    state
        .supervisor
        .connect(&req.target, move |snapshot| {
            // The graphs are drawn from the history of the telemetry, and the history is
            // fed from the state the link publishes.
            if let Some(session) = &snapshot.session {
                history.record(&session.telemetry);
            }

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

/// The flight modes the interface offers.
///
/// # Errors
///
/// Never fails.
#[tauri::command]
async fn list_flight_modes() -> CommandResult<Vec<ModeInfo>> {
    Ok(flight_modes())
}

/// The serial ports this machine has.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the ports cannot be listed.
#[tauri::command]
async fn list_serial_ports() -> CommandResult<Vec<SerialPortEvent>> {
    let ports = SerialTransport::available_ports().map_err(|error| {
        CommandError::internal(format!("cannot list the serial ports: {error}"))
    })?;

    Ok(ports
        .into_iter()
        .map(|port| SerialPortEvent {
            device: port.device,
            description: port.description,
        })
        .collect())
}

/// A serial port, as the interface sees it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SerialPortEvent {
    /// The device name, for example `COM3` or `/dev/ttyUSB0`.
    device: String,
    /// A human-readable description of the device.
    description: Option<String>,
}

/// The parameters the vehicle has reported.
///
/// The store is filled as the vehicle reports them, so the list grows as the vehicle talks.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link.
#[tauri::command]
async fn list_parameters(state: State<'_, AppState>) -> CommandResult<Vec<ParameterEvent>> {
    let Some(session) = state.supervisor.state() else {
        return Err(CommandError::not_connected());
    };

    Ok(session
        .parameters
        .iter()
        .map(|parameter| ParameterEvent {
            name: parameter.name.clone(),
            value: parameter.value,
            reported_type: parameter.reported_type.as_mavlink() as u8,
        })
        .collect())
}

/// Asks the vehicle for every parameter it has, and waits for the answers.
///
/// The parameters the vehicle volunteers are not enough: a vehicle only reports the ones
/// that change. The full list has to be asked for.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the vehicle
/// does not answer.
#[tauri::command]
async fn read_parameters(
    state: State<'_, AppState>,
    req: VehicleRequest,
) -> CommandResult<Vec<ParameterEvent>> {
    let vehicle = resolve_vehicle(&state, &req)?;

    let session = state
        .supervisor
        .session()
        .ok_or_else(CommandError::not_connected)?;

    let store = session
        .download_parameters(vehicle, std::time::Duration::from_secs(30))
        .await
        .map_err(|error| CommandError::rejected(error.to_string()))?;

    Ok(store
        .iter()
        .map(|parameter| ParameterEvent {
            name: parameter.name.clone(),
            value: parameter.value,
            reported_type: parameter.reported_type.as_mavlink() as u8,
        })
        .collect())
}

/// Writes a parameter and waits for the vehicle to confirm it.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the vehicle
/// refuses the value.
#[tauri::command]
async fn write_parameter(
    state: State<'_, AppState>,
    req: VehicleRequest,
    name: String,
    value: f64,
) -> CommandResult<ParameterEvent> {
    let vehicle = resolve_vehicle(&state, &req)?;

    let session = state
        .supervisor
        .session()
        .ok_or_else(CommandError::not_connected)?;

    let written = session
        .write_parameter(vehicle, &name, value, std::time::Duration::from_secs(10))
        .await
        .map_err(|error| CommandError::rejected(error.to_string()))?;

    Ok(ParameterEvent {
        name: written.name,
        value: written.value,
        reported_type: written.reported_type.as_mavlink() as u8,
    })
}

/// A parameter, as the interface sees it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ParameterEvent {
    /// The name of the parameter.
    name: String,
    /// The value.
    value: f64,
    /// The type the vehicle reported, as the MAVLink enumeration.
    reported_type: u8,
}

/// The warnings about a plan, before it is flown.
///
/// The rule is the autopilot's, so it is evaluated in the model and the interface only
/// shows what it says.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the plan holds a command the ground
/// station does not know.
#[tauri::command]
async fn validate_mission(
    kind: PlanKind,
    items: Vec<PlanItemRequest>,
) -> CommandResult<Vec<PlanWarningEvent>> {
    let plan = plan_of(kind, &items)?;

    Ok(mpcraft_mission::validate::validate(&plan)
        .into_iter()
        .map(|warning| PlanWarningEvent {
            seq: warning.seq,
            reason: match warning.reason {
                mpcraft_mission::validate::WarningReason::ZeroAltitude => "zeroAltitude".to_owned(),
            },
        })
        .collect())
}

/// A warning about an item of a plan.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanWarningEvent {
    /// The position of the item in the plan.
    seq: usize,
    /// What is wrong with it.
    reason: String,
}

/// Writes a plan to a file.
///
/// The format is the one Mission Planner uses: `waypoints` for the QGC file and `kml` for
/// the one Google Earth reads.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the plan holds a command the ground
/// station does not know, or when the file cannot be written.
#[tauri::command]
async fn export_plan(
    kind: PlanKind,
    items: Vec<PlanItemRequest>,
    format: String,
    path: String,
) -> CommandResult<()> {
    let plan = plan_of(kind, &items)?;

    let text = match format.as_str() {
        "kml" => mpcraft_mission::kml::write_kml(&plan),
        "waypoints" | "wpl" | "qgc" => mpcraft_mission::qgc::write_waypoints(&plan),
        other => {
            return Err(CommandError::invalid_request(format!(
                "unknown format {other:?}; use waypoints or kml"
            )));
        }
    };

    std::fs::write(&path, text)
        .map_err(|error| CommandError::internal(format!("cannot write {path}: {error}")))?;

    Ok(())
}

/// Reads a plan from a file.
///
/// The format is the one Mission Planner uses: `waypoints` for the QGC file and `kml` for
/// the one Google Earth reads.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the file cannot be read, or when it
/// is not a plan this ground station understands.
#[tauri::command]
async fn import_plan(format: String, path: String) -> CommandResult<PlanInfo> {
    let text = std::fs::read_to_string(&path)
        .map_err(|error| CommandError::internal(format!("cannot read {path}: {error}")))?;

    let plan = match format.as_str() {
        "kml" => mpcraft_mission::kml::read_kml(&text)
            .map_err(|error| CommandError::invalid_request(error.to_string()))?,
        "waypoints" | "wpl" | "qgc" => mpcraft_mission::qgc::read_waypoints(&text)
            .map_err(|error| CommandError::invalid_request(error.to_string()))?,
        other => {
            return Err(CommandError::invalid_request(format!(
                "unknown format {other:?}; use waypoints or kml"
            )));
        }
    };

    Ok(PlanInfo::from(&plan))
}

/// The commands a plan can hold, and what their parameters mean.
///
/// The table is the one Mission Planner's `mavcmd.xml` describes, ported into the model.
/// The interface must not write it out again: the meaning of a parameter belongs to the
/// autopilot, not to the screen.
///
/// # Errors
///
/// Never fails.
#[tauri::command]
async fn list_mav_commands() -> CommandResult<Vec<MavCommandEvent>> {
    Ok(mpcraft_mission::mavcmd::COMMANDS
        .iter()
        .map(|entry| MavCommandEvent {
            name: entry.name.to_owned(),
            value: entry.command as u32,
            roles: entry.roles.iter().map(|role| role_name(*role)).collect(),
            units: entry.units.iter().map(|unit| unit_name(*unit)).collect(),
        })
        .collect())
}

/// The name of the role of a parameter slot.
fn role_name(role: mpcraft_mission::mavcmd::ParamRole) -> &'static str {
    use mpcraft_mission::mavcmd::ParamRole;

    match role {
        ParamRole::Empty => "empty",
        ParamRole::Latitude => "latitude",
        ParamRole::Longitude => "longitude",
        ParamRole::Altitude => "altitude",
        ParamRole::Speed => "speed",
        ParamRole::Distance => "distance",
        ParamRole::Angle => "angle",
        ParamRole::Other => "other",
    }
}

/// The name of the unit of a parameter slot.
fn unit_name(unit: mpcraft_mission::mavcmd::ParamUnit) -> &'static str {
    use mpcraft_mission::mavcmd::ParamUnit;

    match unit {
        ParamUnit::None => "",
        ParamUnit::Degrees => "°",
        ParamUnit::Metres => "m",
        ParamUnit::MetresPerSecond => "m/s",
    }
}

/// A command a plan can hold.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MavCommandEvent {
    /// The name of the command, for example `MAV_CMD_NAV_WAYPOINT`.
    name: String,
    /// The value the command travels with.
    value: u32,
    /// The role of each of the seven slots, in order.
    roles: Vec<&'static str>,
    /// The unit of each of the seven slots, in order.
    units: Vec<&'static str>,
}

/// The graphs the user can choose.
///
/// # Errors
///
/// Never fails.
#[tauri::command]
async fn list_graphs(state: State<'_, AppState>) -> CommandResult<Vec<PresetInfo>> {
    Ok(state.graphs.presets())
}

/// The values of a graph over the history of the telemetry.
///
/// The `since` argument is how long back to look, in seconds. A live graph only wants the
/// recent past.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no graph with that name.
#[tauri::command]
async fn graph_series(
    state: State<'_, AppState>,
    name: String,
    since: Option<f64>,
) -> CommandResult<SeriesData> {
    let since = since.map(std::time::Duration::from_secs_f64);
    state.graphs.series(&name, since)
}

/// The vehicle a command is for.
///
/// The system and component ids are optional: a request that names no vehicle is for
/// the primary one on the link.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct VehicleRequest {
    /// The system id of the vehicle.
    system_id: Option<u8>,
    /// The component id of the vehicle.
    component_id: Option<u8>,
}

/// Arms the vehicle.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the vehicle
/// refuses the command.
#[tauri::command]
async fn arm(state: State<'_, AppState>, req: VehicleRequest) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;
    state.supervisor.arm(vehicle).await
}

/// Disarms the vehicle.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the vehicle
/// refuses the command.
#[tauri::command]
async fn disarm(state: State<'_, AppState>, req: VehicleRequest) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;
    state.supervisor.disarm(vehicle).await
}

/// Takes off to an altitude, relative to the home position.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the vehicle
/// refuses the command.
#[tauri::command]
async fn takeoff(
    state: State<'_, AppState>,
    req: VehicleRequest,
    altitude: f32,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;
    state.supervisor.takeoff(vehicle, altitude).await
}

/// Changes the flight mode.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the vehicle
/// refuses the command.
#[tauri::command]
async fn set_mode(
    state: State<'_, AppState>,
    req: VehicleRequest,
    custom_mode: u32,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;
    state.supervisor.set_mode(vehicle, custom_mode).await
}

/// Downloads the plan the vehicle holds.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the vehicle
/// does not answer.
#[tauri::command]
async fn download_mission(
    state: State<'_, AppState>,
    req: VehicleRequest,
    kind: PlanKind,
) -> CommandResult<PlanInfo> {
    let vehicle = resolve_vehicle(&state, &req)?;
    let plan = state
        .supervisor
        .download_mission(vehicle, mission_type_of(kind))
        .await?;

    Ok(PlanInfo::from(&plan))
}

/// Uploads a plan to the vehicle.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the vehicle
/// refuses the plan.
#[tauri::command]
async fn upload_mission(
    state: State<'_, AppState>,
    req: VehicleRequest,
    kind: PlanKind,
    items: Vec<PlanItemRequest>,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;
    let plan = plan_of(kind, &items)?;
    state.supervisor.upload_mission(vehicle, &plan).await
}

/// Says which item of the plan the vehicle is executing.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the vehicle
/// does not answer.
#[tauri::command]
async fn set_current_item(
    state: State<'_, AppState>,
    req: VehicleRequest,
    seq: u16,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;
    state.supervisor.set_current_item(vehicle, seq).await
}

/// Empties one of the vehicle's plans.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the vehicle
/// refuses.
#[tauri::command]
async fn clear_mission(
    state: State<'_, AppState>,
    req: VehicleRequest,
    kind: PlanKind,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;
    state
        .supervisor
        .clear_mission(vehicle, mission_type_of(kind))
        .await
}

/// Makes the vehicle jump to an item of the plan.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the vehicle
/// refuses the command.
#[tauri::command]
async fn jump_to_item(
    state: State<'_, AppState>,
    req: VehicleRequest,
    seq: u16,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;
    state.supervisor.jump_to_item(vehicle, seq).await
}

/// One item of a plan, as the interface sends it.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanItemRequest {
    /// The numeric command value.
    command_value: u32,
    /// The coordinate frame the position is expressed in.
    frame: u32,
    /// The four generic parameters.
    params: [f32; 4],
    /// The latitude, for a command that carries a position.
    x: f64,
    /// The longitude, for a command that carries a position.
    y: f64,
    /// The altitude.
    z: f64,
    /// Whether the item is the one being executed.
    current: bool,
    /// Whether the vehicle continues to the next item automatically.
    autocontinue: bool,
}

/// The vehicle a command is for: the one it names, or the primary one on the link.
fn resolve_vehicle(state: &AppState, req: &VehicleRequest) -> CommandResult<VehicleId> {
    if let (Some(system_id), Some(component_id)) = (req.system_id, req.component_id) {
        return Ok(VehicleId::new(system_id, component_id));
    }

    let primary = state
        .supervisor
        .state()
        .and_then(|state| {
            state
                .vehicles
                .iter()
                .find(|vehicle| vehicle.primary)
                .or_else(|| state.vehicles.first())
                .map(|vehicle| vehicle.id)
        })
        .ok_or_else(|| CommandError::no_vehicle("there is no vehicle on the link"))?;

    Ok(primary)
}

/// The wire kind of a plan.
fn mission_type_of(kind: PlanKind) -> MissionType {
    kind.into()
}

/// Builds a plan from what the interface sent.
fn plan_of(kind: PlanKind, items: &[PlanItemRequest]) -> CommandResult<Plan> {
    let mut plan = Plan::new(mission_type_of(kind));

    for item in items {
        let command =
            mpcraft_mission::mavcmd::command_from_value(item.command_value).ok_or_else(|| {
                CommandError::invalid_request(format!(
                    "{} is not a command this ground station knows",
                    item.command_value
                ))
            })?;

        plan.push(mpcraft_mission::MissionItem {
            id: mpcraft_mission::MissionItemId::next(),
            command,
            frame: item.frame,
            params: item.params,
            x: item.x,
            y: item.y,
            z: item.z,
            current: item.current,
            autocontinue: item.autocontinue,
            display_name: None,
        });
    }

    Ok(plan)
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
    /// The messages the vehicle has sent, most recent last.
    messages: Vec<MessageEvent>,
}

/// A message the vehicle sent.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MessageEvent {
    /// How serious the message is, as the MAVLink enumeration.
    severity: u8,
    /// The name of the severity.
    severity_name: String,
    /// The text of the message.
    text: String,
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
    /// The home position, when it is known.
    home: Option<PositionEvent>,
    /// The bearing to home, in degrees.
    bearing_to_home: Option<f64>,
    /// The progress of the plan, when the vehicle has reported it.
    mission: Option<MissionProgressEvent>,
    /// The state of the global positioning, when it is known.
    gps: Option<GpsEvent>,
}

/// The progress of the plan on the vehicle.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MissionProgressEvent {
    /// The item the vehicle is executing.
    pub seq: u16,
    /// How many items the plan has.
    pub total: u16,
    /// The state of the plan, as the MAVLink enumeration.
    pub state: u8,
    /// Whether the plan is being run.
    pub in_mission_mode: bool,
}

/// The state of the global positioning.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GpsEvent {
    /// The type of the fix, as the MAVLink enumeration.
    pub fix_type: u8,
    /// The number of satellites in use.
    pub satellites: u8,
    /// The horizontal dilution of precision.
    pub hdop: f64,
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
    /// The northward speed, in metres per second.
    north: f64,
    /// The eastward speed, in metres per second.
    east: f64,
    /// The downward speed, in metres per second.
    down: f64,
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
            messages: state
                .map(|state| {
                    state
                        .messages
                        .iter()
                        .map(|message| MessageEvent {
                            severity: message.severity,
                            severity_name: message.severity_name().to_owned(),
                            text: message.text.clone(),
                        })
                        .collect()
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
            north: velocity.north,
            east: velocity.east,
            down: velocity.down,
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
        home: telemetry.home.map(|position| PositionEvent {
            lat: position.lat,
            lng: position.lng,
            alt: position.alt,
        }),
        bearing_to_home: telemetry.bearing_to_home,
        mission: telemetry
            .mission
            .as_ref()
            .map(|mission| MissionProgressEvent {
                seq: mission.seq,
                total: mission.total,
                state: mission.state as u8,
                in_mission_mode: mission.in_mission_mode,
            }),
        gps: telemetry.gps.as_ref().map(|gps| GpsEvent {
            fix_type: gps.fix_type as u8,
            satellites: gps.satellites,
            hdop: gps.hdop,
        }),
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
            list_connection_profiles,
            list_flight_modes,
            list_serial_ports,
            list_graphs,
            list_mav_commands,
            graph_series,
            list_parameters,
            read_parameters,
            write_parameter,
            validate_mission,
            export_plan,
            import_plan,
            arm,
            disarm,
            takeoff,
            set_mode,
            download_mission,
            upload_mission,
            set_current_item,
            clear_mission,
            jump_to_item
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
                    let history = state.graphs.clone();
                    let result = state
                        .supervisor
                        .connect(&target, move |snapshot| {
                            // The graphs are drawn from the history of the telemetry, and
                            // the history is fed from the state the link publishes.
                            if let Some(session) = &snapshot.session {
                                history.record(&session.telemetry);
                            }

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

    #[test]
    fn a_vehicle_on_the_link_reaches_the_interface() {
        // The interface addresses the vehicle it sees, so a vehicle that is on the link
        // must arrive in the event with everything the interface shows about it.
        let mut state = SessionState::disconnected();
        state.connected = true;
        state.vehicles.push(mpcraft_link::Vehicle {
            id: mpcraft_link::VehicleId::new(1, 1),
            kind: Some(mpcraft_link::VehicleKind::ArduCopter),
            mav_type: Some(mpcraft_mavlink::MavType::MAV_TYPE_QUADROTOR),
            autopilot: Some(mpcraft_mavlink::MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA),
            custom_mode: Some(5),
            primary: true,
        });

        let event = StateEvent::from(&LinkSnapshot {
            status: ConnectionStatus::Connected,
            session: Some(state),
        });

        assert_eq!(event.vehicles.len(), 1);
        assert_eq!(event.vehicles[0].system_id, 1);
        assert_eq!(event.vehicles[0].component_id, 1);
        assert_eq!(event.vehicles[0].kind.as_deref(), Some("ArduCopter"));
    }

    #[test]
    fn the_telemetry_the_interface_needs_is_carried_by_the_event() {
        // The vertical speed dial, the home marker and the plan progress are read from
        // the event, so the fields must be there.
        let mut state = SessionState::disconnected();
        state.connected = true;
        state.telemetry.velocity = Some(mpcraft_vehicle::Velocity {
            north: 1.0,
            east: 2.0,
            down: -0.5,
            ground_speed: 2.236,
        });
        state.telemetry.home = Some(mpcraft_vehicle::GeoPoint {
            lat: -35.28,
            lng: 149.13,
            alt: 584.0,
        });
        state.telemetry.bearing_to_home = Some(42.0);
        state.telemetry.mission = Some(mpcraft_vehicle::MissionProgress {
            seq: 3,
            total: 12,
            state: mpcraft_vehicle::PlanState::Active,
            in_mission_mode: true,
            mission_id: 1,
            fence_id: 0,
            rally_id: 0,
        });
        state.telemetry.gps = Some(mpcraft_vehicle::GpsStatus {
            fix_type: mpcraft_mavlink::MavGpsFixType::GPS_FIX_TYPE_3D_FIX,
            satellites: 14,
            hdop: 1.2,
        });

        let event = StateEvent::from(&LinkSnapshot {
            status: ConnectionStatus::Connected,
            session: Some(state),
        });

        let velocity = event.telemetry.velocity.expect("a velocity");
        assert!((velocity.north - 1.0).abs() < 1e-9);
        assert!((velocity.down + 0.5).abs() < 1e-9);

        assert!(event.telemetry.home.is_some());
        assert_eq!(event.telemetry.bearing_to_home, Some(42.0));

        let mission = event.telemetry.mission.expect("the progress of the plan");
        assert_eq!(mission.seq, 3);
        assert_eq!(mission.total, 12);
        assert!(mission.in_mission_mode);

        let gps = event.telemetry.gps.expect("the state of the positioning");
        assert_eq!(gps.satellites, 14);
        assert!((gps.hdop - 1.2).abs() < 1e-9);
    }
}
