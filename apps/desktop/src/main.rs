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
    ConnectionProfile, ConnectionStatus, LinkSnapshot, ModeInfo, OutputFunction, PlanInfo,
    PlanKind, PresetInfo, SeriesData, SessionState,
};
use mpcraft_link::{Calibration, OsdSlot, OsdSlotConfig, VehicleId};
use mpcraft_mission::{MissionType, Plan};
use mpcraft_transport::SerialTransport;
use mpcraft_vehicle::Telemetry;
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
    let feeder = state.inner().clone();

    // The link is signed with the keys that are saved, when there are any.
    state.apply_signing();

    state
        .supervisor
        .connect(&req.target, move |snapshot| {
            // The graphs are drawn from the history of the telemetry, and the track
            // that is being streamed to a file is written from the same state: the
            // place of the vehicle is only known here.
            if let Some(session) = &snapshot.session {
                if let Err(error) = feeder.feed(session) {
                    tracing::warn!("cannot write the track: {error}");
                }
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

/// Writes the parameters of the vehicle to a file.
///
/// The format is upstream's: a line per parameter, the name and the value separated by
/// a comma.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the file
/// cannot be written.
#[tauri::command]
async fn export_parameters(state: State<'_, AppState>, path: String) -> CommandResult<()> {
    let Some(session) = state.supervisor.state() else {
        return Err(CommandError::not_connected());
    };

    let text = mpcraft_params::param_file::write(&session.parameters);

    std::fs::write(&path, text)
        .map_err(|error| CommandError::internal(format!("the file could not be written: {error}")))
}

/// Reads the parameters of a file.
///
/// The parameters are returned, and not written to the vehicle: the operator sees them
/// in the table and decides.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the file cannot be read, or when it
/// holds a value that is not a number.
#[tauri::command]
async fn import_parameters(path: String) -> CommandResult<Vec<ParameterEvent>> {
    let text = std::fs::read_to_string(&path).map_err(|error| {
        CommandError::invalid_request(format!("the file could not be read: {error}"))
    })?;

    let parameters = mpcraft_params::param_file::read(&text)
        .map_err(|error| CommandError::invalid_request(error.to_string()))?;

    Ok(parameters
        .into_iter()
        .map(|parameter| ParameterEvent {
            name: parameter.name,
            value: parameter.value,
            reported_type: mpcraft_params::ParamType::Real64.as_mavlink() as u8,
        })
        .collect())
}

/// Compares the parameters of the vehicle with the ones of a file.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, when the file
/// cannot be read, or when it holds a value that is not a number.
#[tauri::command]
async fn compare_parameters(
    state: State<'_, AppState>,
    path: String,
) -> CommandResult<Vec<ParameterDifferenceEvent>> {
    let Some(session) = state.supervisor.state() else {
        return Err(CommandError::not_connected());
    };

    let text = std::fs::read_to_string(&path).map_err(|error| {
        CommandError::invalid_request(format!("the file could not be read: {error}"))
    })?;

    let other = mpcraft_params::param_file::read(&text)
        .map_err(|error| CommandError::invalid_request(error.to_string()))?;

    let mut store = mpcraft_params::ParameterStore::new();
    for parameter in other {
        store.insert(parameter);
    }

    Ok(
        mpcraft_params::param_file::differences(&session.parameters, &store)
            .into_iter()
            .map(|difference| ParameterDifferenceEvent {
                name: difference.name,
                current: difference.current,
                other: difference.other,
            })
            .collect(),
    )
}

/// How the value of a parameter differs between the vehicle and a file.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ParameterDifferenceEvent {
    /// The name of the parameter.
    name: String,
    /// The value the vehicle has, when it has one.
    current: Option<f64>,
    /// The value the file has, when it has one.
    other: Option<f64>,
}

/// Asks the vehicle to write its parameters to its own memory.
///
/// A parameter the vehicle has in its memory but not in the flash is lost when the
/// vehicle loses power.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the
/// vehicle refuses the command.
#[tauri::command]
async fn write_parameters_to_flash(
    state: State<'_, AppState>,
    req: VehicleRequest,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;
    state.supervisor.write_parameters_to_flash(vehicle).await
}

/// Where the vehicle is, in the systems an operator reads.
///
/// The decimal degrees, the universal transverse Mercator grid and the military grid
/// reference system, all of them from the same position.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the
/// vehicle has not said where it is.
#[tauri::command]
async fn vehicle_coordinates(state: State<'_, AppState>) -> CommandResult<CoordinatesEvent> {
    let Some(session) = state.supervisor.state() else {
        return Err(CommandError::not_connected());
    };

    let Some(position) = session.telemetry.position else {
        return Err(CommandError::not_connected());
    };

    let coordinates = mpcraft_app::coordinates::of(position.lat, position.lng, position.alt);

    Ok(CoordinatesEvent {
        lat: coordinates.lat,
        lng: coordinates.lng,
        alt: coordinates.alt,
        utm: coordinates.utm.map(|utm| UtmEvent {
            zone: utm.zone,
            band: utm.band.to_string(),
            easting: utm.easting,
            northing: utm.northing,
        }),
        mgrs: coordinates.mgrs,
    })
}

/// Where the vehicle is.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CoordinatesEvent {
    /// The latitude in decimal degrees, positive north.
    lat: f64,
    /// The longitude in decimal degrees, positive east.
    lng: f64,
    /// The altitude above mean sea level, in metres.
    alt: f64,
    /// The position in the universal transverse Mercator grid.
    utm: Option<UtmEvent>,
    /// The position in the military grid reference system.
    mgrs: Option<String>,
}

/// A position in the universal transverse Mercator grid.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct UtmEvent {
    /// The zone number, 1 to 60.
    zone: u8,
    /// The latitude band letter.
    band: String,
    /// The easting, in metres.
    easting: f64,
    /// The northing, in metres.
    northing: f64,
}

/// What the outputs of the vehicle are for.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link.
#[tauri::command]
async fn output_functions(state: State<'_, AppState>) -> CommandResult<Vec<OutputFunction>> {
    state.supervisor.output_functions()
}

/// Runs the motors of the vehicle, to see whether they turn.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the
/// vehicle refuses the command.
#[tauri::command]
async fn motor_test(
    state: State<'_, AppState>,
    req: VehicleRequest,
    motor: u8,
    throttle: f32,
    seconds: f32,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;

    state
        .supervisor
        .motor_test(vehicle, motor, throttle, seconds)
        .await
}

/// Asks the vehicle to calibrate one of its sensors.
///
/// The calibration is carried out by the vehicle: the order says which sensor, and the
/// vehicle answers when it has begun.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, when the
/// calibration is not one this station knows, or when the vehicle does not answer.
#[tauri::command]
async fn calibrate(
    state: State<'_, AppState>,
    req: VehicleRequest,
    kind: String,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;

    let calibration = match kind.as_str() {
        "accelerometer" => Calibration::Accelerometer,
        "level" => Calibration::AccelerometerLevel,
        "compass" => Calibration::Compass,
        other => {
            return Err(CommandError::new(
                "invalid.calibration",
                format!("the calibration '{other}' is not one this station knows"),
            ));
        }
    };

    state.supervisor.calibrate(vehicle, calibration).await
}

/// Takes the radio channels over, one at a time.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the link
/// cannot carry the message.
#[tauri::command]
async fn rc_override(
    state: State<'_, AppState>,
    req: VehicleRequest,
    channels: Vec<Option<u16>>,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;

    let mut given = [None; 8];

    for (index, channel) in channels.into_iter().enumerate().take(8) {
        given[index] = channel;
    }

    state.supervisor.rc_override(vehicle, given).await
}

/// Lets go of the radio channels.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the link
/// cannot carry the message.
#[tauri::command]
async fn rc_override_release(state: State<'_, AppState>, req: VehicleRequest) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;

    state.supervisor.rc_override_release(vehicle).await
}

/// Flies the vehicle with the sticks of the ground station.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the link
/// cannot carry the message.
#[tauri::command]
async fn manual_control(
    state: State<'_, AppState>,
    req: VehicleRequest,
    roll: f32,
    pitch: f32,
    thrust: f32,
    yaw: f32,
    buttons: u16,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;

    state
        .supervisor
        .manual_control(vehicle, roll, pitch, thrust, yaw, buttons)
        .await
}

/// Tells the vehicle to go to a place.
///
/// This is the guided mode: the vehicle flies to the place and stays there.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the link
/// cannot carry the message.
#[tauri::command]
async fn set_position_target(
    state: State<'_, AppState>,
    req: VehicleRequest,
    latitude: f64,
    longitude: f64,
    altitude: f32,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;

    state
        .supervisor
        .set_position_target(vehicle, latitude, longitude, altitude)
        .await
}

/// Asks the vehicle to restart.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the link
/// cannot carry the command.
#[tauri::command]
async fn reboot(state: State<'_, AppState>, req: VehicleRequest) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;

    state.supervisor.reboot(vehicle).await
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

/// A change to a parameter: the value it is given.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParamChange {
    /// The name of the parameter.
    name: String,
    /// The value it is given.
    value: f64,
}

/// Writes several parameters, in the order the vehicle needs them.
///
/// The order is the autopilots' knowledge: the parameters that turn something on are
/// written last, so that what they turn on is already configured when it is turned on.
///
/// The answer carries the value each parameter ended up with, which is not always the
/// value that was asked for: the vehicle clamps to its own range, and what the vehicle
/// holds is what is said.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the
/// vehicle refuses a write.
#[tauri::command]
async fn write_parameters_batch(
    state: State<'_, AppState>,
    req: VehicleRequest,
    changes: Vec<ParamChange>,
) -> CommandResult<Vec<ParameterEvent>> {
    let vehicle = resolve_vehicle(&state, &req)?;

    let session = state
        .supervisor
        .session()
        .ok_or_else(CommandError::not_connected)?;

    let names: Vec<String> = changes.iter().map(|change| change.name.clone()).collect();
    let ordered = mpcraft_params::ordering::write_order(&names);

    let mut written = Vec::with_capacity(changes.len());

    for name in ordered {
        let Some(change) = changes.iter().find(|change| change.name == name) else {
            continue;
        };

        let parameter = session
            .write_parameter(
                vehicle,
                &name,
                change.value,
                std::time::Duration::from_secs(10),
            )
            .await
            .map_err(|error| CommandError::rejected(error.to_string()))?;

        written.push(ParameterEvent {
            name: parameter.name,
            value: parameter.value,
            reported_type: parameter.reported_type.as_mavlink() as u8,
        });
    }

    Ok(written)
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
                mpcraft_mission::validate::WarningReason::TooFewFenceVertices => {
                    "tooFewFenceVertices".to_owned()
                }
            },
        })
        .collect())
}

/// A warning about an item of a plan.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanWarningEvent {
    /// The position of the item in the plan, when the warning is about one.
    seq: Option<usize>,
    /// What is wrong with it.
    reason: String,
}

/// The elevation of the terrain along a plan.
///
/// The profile is the shape of the ground the plan flies over: the elevation of the
/// terrain at a point every few metres, and the height the plan asks for.
///
/// The height of the plan is written in the frame each of its places says, and the
/// profile draws the plan and the ground on the same scale, so the height of every
/// place is told against the sea before it is drawn.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the plan holds no place, or when
/// the terrain cannot be reached.
#[tauri::command]
async fn plan_terrain_profile(
    state: State<'_, AppState>,
    kind: PlanKind,
    items: Vec<PlanItemRequest>,
) -> CommandResult<Vec<TerrainPointEvent>> {
    let plan = plan_of(kind, &items)?;

    // The places of the plan, in the order they are flown: where they are, the frame
    // their height is written in and the height itself.
    let places: Vec<Place> = plan
        .items
        .iter()
        .filter(|item| mpcraft_mission::mavcmd::is_location_command(item.command))
        .map(|item| Place {
            lat: item.x,
            lng: item.y,
            frame: item.frame,
            height: item.z,
        })
        .collect();

    if places.len() < 2 {
        return Ok(Vec::new());
    }

    // The height of the home, when the vehicle has reported one: a height above the
    // home cannot be told against the sea without it.
    let home = state
        .supervisor
        .state()
        .and_then(|session| session.telemetry.home)
        .map(|home| home.alt);

    // The samples of the path: a point every few metres between the places of the plan.
    const SAMPLES_PER_SEGMENT: usize = 8;

    let mut samples: Vec<Sample> = Vec::new();

    for (index, window) in places.windows(2).enumerate() {
        let from = window[0];
        let to = window[1];

        // The last segment has one more sample, so that the last place of the plan is
        // a sample of its own.
        let last = index + 1 == places.len() - 1;
        let steps = if last {
            SAMPLES_PER_SEGMENT + 1
        } else {
            SAMPLES_PER_SEGMENT
        };

        for step in 0..steps {
            let fraction = step as f64 / SAMPLES_PER_SEGMENT as f64;

            // The places of the plan are the samples that fall on one.
            let place = if step == 0 {
                Some(index)
            } else if last && step == SAMPLES_PER_SEGMENT {
                Some(index + 1)
            } else {
                None
            };

            samples.push(Sample {
                lat: from.lat + (to.lat - from.lat) * fraction,
                lng: from.lng + (to.lng - from.lng) * fraction,
                place,
                fraction,
            });
        }
    }

    // The ground under every sample.
    let mut grounds: Vec<Option<f64>> = Vec::with_capacity(samples.len());

    for sample in &samples {
        let elevation = state
            .terrain
            .elevation(mpcraft_geo::projection::LatLng::new(sample.lat, sample.lng))
            .await;

        // A place the survey never reached, or the sea, has no elevation: the profile
        // says so by leaving the point out, and the plan is still drawn.
        grounds.push(elevation.ok().map(|elevation| elevation.metres));
    }

    // The height of every place of the plan, told against the sea.
    let mut heights: Vec<Option<f64>> = Vec::with_capacity(places.len());

    for (index, place) in places.iter().enumerate() {
        // The ground under the place: the sample that falls on it.
        let ground = samples
            .iter()
            .position(|sample| sample.place == Some(index))
            .and_then(|position| grounds[position]);

        heights.push(mpcraft_mission::altitude::above_sea(
            place.frame,
            place.height,
            home,
            ground,
        ));
    }

    // The points of the profile.
    let mut points = Vec::with_capacity(samples.len());
    let mut distance = 0.0;
    let mut previous: Option<(f64, f64)> = None;

    for (index, sample) in samples.iter().enumerate() {
        if let Some((before_lat, before_lng)) = previous {
            distance += mpcraft_geo::distance_m(
                mpcraft_geo::GeoPoint::new(before_lat, before_lng),
                mpcraft_geo::GeoPoint::new(sample.lat, sample.lng),
            );
        }

        previous = Some((sample.lat, sample.lng));

        // The height of the plan at this point: between two places of the plan it is
        // the straight line that joins them, because that is the path the vehicle flies.
        let height = match sample.place {
            Some(place) => heights.get(place).copied().flatten(),
            None => between(
                heights.get(sample.place_index()),
                heights.get(sample.place_index() + 1),
                sample.fraction,
            ),
        };

        points.push(TerrainPointEvent {
            distance,
            elevation: grounds[index].unwrap_or(f64::NAN),
            height,
            waypoint: sample.place.is_some(),
        });
    }

    Ok(points)
}

/// One place of a plan.
#[derive(Debug, Clone, Copy)]
struct Place {
    /// The latitude, in degrees.
    lat: f64,
    /// The longitude, in degrees.
    lng: f64,
    /// The frame the height is written in.
    frame: u32,
    /// The height, in the frame it is written in.
    height: f64,
}

/// One sample of the path of a plan.
struct Sample {
    /// The latitude, in degrees.
    lat: f64,
    /// The longitude, in degrees.
    lng: f64,
    /// The place of the plan the sample falls on, when it falls on one.
    place: Option<usize>,
    /// How far along the segment the sample is, from zero to one.
    fraction: f64,
}

impl Sample {
    /// The place of the plan the sample comes after.
    fn place_index(&self) -> usize {
        self.place.map_or(0, |place| place.saturating_sub(1))
    }
}

/// The height of the plan between two of its places.
///
/// The plan flies in a straight line between two places, and the height on that line
/// changes the same way.
fn between(from: Option<&Option<f64>>, to: Option<&Option<f64>>, fraction: f64) -> Option<f64> {
    match (from.and_then(|value| *value), to.and_then(|value| *value)) {
        (Some(from), Some(to)) => Some(from + (to - from) * fraction),
        (Some(from), None) | (None, Some(from)) => Some(from),
        (None, None) => None,
    }
}

/// One point of the profile of the terrain.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TerrainPointEvent {
    /// The distance from the start of the plan, in metres.
    distance: f64,
    /// The elevation of the terrain, in metres above the sea.
    elevation: f64,
    /// The height of the plan above the sea, when it is known.
    height: Option<f64>,
    /// Whether the point is a place of the plan.
    waypoint: bool,
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
    let plan = match format.as_str() {
        // A drawing is not text: the file is read as the bytes it is, and the projection
        // written beside it — the `.prj` of the same name — is what says whether the
        // coordinates of the drawing are degrees or metres of a zone.
        "shp" => {
            let bytes = std::fs::read(&path)
                .map_err(|error| CommandError::internal(format!("cannot read {path}: {error}")))?;
            let prj =
                std::fs::read_to_string(std::path::Path::new(&path).with_extension("prj")).ok();
            let drawing = mpcraft_geo::Drawing::read_shapefile(&bytes, prj.as_deref())
                .map_err(|error| CommandError::invalid_request(error.to_string()))?;

            mpcraft_mission::plan_from_places(&drawing.places)
        }
        "dxf" => {
            let text = read_text(&path)?;
            let drawing = mpcraft_geo::Drawing::read_dxf(&text)
                .map_err(|error| CommandError::invalid_request(error.to_string()))?;

            mpcraft_mission::plan_from_places(&drawing.places)
        }
        "kml" => mpcraft_mission::kml::read_kml(&read_text(&path)?)
            .map_err(|error| CommandError::invalid_request(error.to_string()))?,
        "waypoints" | "wpl" | "qgc" => mpcraft_mission::qgc::read_waypoints(&read_text(&path)?)
            .map_err(|error| CommandError::invalid_request(error.to_string()))?,
        other => {
            return Err(CommandError::invalid_request(format!(
                "unknown format {other:?}; use waypoints, kml, shp or dxf"
            )));
        }
    };

    Ok(PlanInfo::from(&plan))
}

/// Reads a plan file that is text.
fn read_text(path: &str) -> Result<String, CommandError> {
    std::fs::read_to_string(path)
        .map_err(|error| CommandError::internal(format!("cannot read {path}: {error}")))
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
            canonical_name: mpcraft_mission::mavcmd::command_name(entry.command),
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
    /// The name the command is shown with, for example `Waypoint`.
    name: String,
    /// The name the specification gives it, for example `MAV_CMD_NAV_WAYPOINT`.
    ///
    /// The plan of the vehicle names its commands the way the specification does,
    /// and the interface has to name them the same way when it adds one.
    canonical_name: String,
    /// The value the command travels with.
    value: u32,
    /// The role of each of the seven slots, in order.
    roles: Vec<&'static str>,
    /// The unit of each of the seven slots, in order.
    units: Vec<&'static str>,
}

/// The metadata of the parameters: what they do, their units and their range.
///
/// The catalogue is the knowledge of the autopilots, so the interface asks for it and
/// shows what it says. A parameter the catalogue does not know has no metadata, and that
/// is not an error: the vehicle may have a parameter no catalogue describes.
///
/// # Errors
///
/// Never fails.
#[tauri::command]
async fn parameter_metadata(names: Vec<String>) -> CommandResult<Vec<Option<ParameterMetaEvent>>> {
    Ok(names
        .iter()
        .map(|name| {
            mpcraft_params::metadata::of(name).map(|meta| ParameterMetaEvent {
                name: meta.name.clone(),
                display_name: meta.display_name.clone(),
                group: meta.group.clone(),
                default: meta.default.clone(),
                description: meta.description.clone(),
                long_description: meta.long_description.clone(),
                unit: meta.unit.clone(),
                minimum: meta.minimum.clone(),
                maximum: meta.maximum.clone(),
                values: meta
                    .values
                    .iter()
                    .map(|choice| ParamChoiceEvent {
                        code: choice.code,
                        text: choice.text.clone(),
                    })
                    .collect(),
                bitmask: meta
                    .bitmask
                    .iter()
                    .map(|flag| ParamFlagEvent {
                        mask: flag.mask,
                        text: flag.text.clone(),
                    })
                    .collect(),
                reboot_required: meta.needs_reboot(),
                vehicles: meta.vehicles.clone(),
            })
        })
        .collect())
}

/// What a log of a flight holds.
///
/// The log is read once, and what it says is the list of the messages it holds and how
/// many of each there are.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the file cannot be read, or when it
/// does not hold a log.
#[tauri::command]
async fn log_summary(path: String) -> CommandResult<mpcraft_app::logs::LogSummary> {
    mpcraft_app::logs::summarise(std::path::Path::new(&path))
}

/// The values of a graph over a log.
///
/// The graph is the same one the live view draws, read from the log instead of from the
/// link.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the file cannot be read, when it
/// does not hold a log, or when there is no graph with that name.
#[tauri::command]
async fn log_series(path: String, name: String) -> CommandResult<mpcraft_app::graphs::SeriesData> {
    mpcraft_app::logs::series(std::path::Path::new(&path), &name)
}

/// Writes the track of a log as a KML document.
///
/// The document is the one Google Earth reads: the path of the flight as a line, and
/// every place of it as a mark.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the file cannot be read, when it
/// does not hold a log, or when the document cannot be written.
#[tauri::command]
async fn log_export_kml(log: String, output: String) -> CommandResult<()> {
    let text = mpcraft_app::logs::track_kml(std::path::Path::new(&log))?;

    std::fs::write(&output, text)
        .map_err(|error| CommandError::internal(format!("cannot write {output}: {error}")))?;

    Ok(())
}

/// Writes the samples of a log as a CSV document.
///
/// The columns are the series of the graph that is chosen, and the rows are the
/// instants where the log has something to say.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the file cannot be read, when it
/// does not hold a log, when there is no graph with that name, or when the document
/// cannot be written.
#[tauri::command]
async fn log_export_csv(log: String, name: String, output: String) -> CommandResult<()> {
    let text = mpcraft_app::logs::samples_csv(std::path::Path::new(&log), &name)?;

    std::fs::write(&output, text)
        .map_err(|error| CommandError::internal(format!("cannot write {output}: {error}")))?;

    Ok(())
}

/// Starts writing the track of the flight to a file.
///
/// The file is a KML document, the one Google Earth reads, and it is written again and
/// again while the vehicle flies. The answer is the name of the file.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the file cannot be written.
#[tauri::command]
async fn start_live_kml(state: State<'_, AppState>, path: String) -> CommandResult<String> {
    let mut cell = state
        .live_track
        .lock()
        .map_err(|_| CommandError::internal("the track is locked"))?;

    let mut track = mpcraft_app::LiveTrack::new(std::path::PathBuf::from(&path));
    track.flush()?;
    *cell = Some(track);

    Ok(path)
}

/// Stops writing the track of the flight to a file.
///
/// The file is written one last time, so that it holds the whole flight. The answer is
/// the name of the file.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no track being written, or
/// when the file cannot be written.
#[tauri::command]
async fn stop_live_kml(state: State<'_, AppState>) -> CommandResult<String> {
    let mut cell = state
        .live_track
        .lock()
        .map_err(|_| CommandError::internal("the track is locked"))?;

    let Some(mut track) = cell.take() else {
        return Err(CommandError::invalid_request(
            "there is no track being written to a file",
        ));
    };

    let path = track.path().display().to_string();
    track.flush()?;

    Ok(path)
}

/// Asks the vehicle where its home is.
///
/// The vehicle is asked with the command that upstream sends, and the answer is the
/// message the vehicle sends when its home is known.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the
/// vehicle does not answer in time.
#[tauri::command]
async fn home_position(
    state: State<'_, AppState>,
    req: VehicleRequest,
) -> CommandResult<PositionEvent> {
    let vehicle = resolve_vehicle(&state, &req)?;
    let home = state.supervisor.home_position(vehicle).await?;

    Ok(PositionEvent {
        lat: home.lat,
        lng: home.lng,
        alt: home.alt,
    })
}

/// Moves the home of the vehicle.
///
/// The vehicle is told where the home is, and it confirms the move by reporting the
/// home it now holds.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, when the vehicle
/// does not confirm the move, or when it reports a different home.
#[tauri::command]
async fn set_home_position(
    state: State<'_, AppState>,
    req: VehicleRequest,
    position: PositionEvent,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;

    state
        .supervisor
        .set_home_position(
            vehicle,
            mpcraft_vehicle::GeoPoint {
                lat: position.lat,
                lng: position.lng,
                alt: position.alt,
            },
        )
        .await
}

/// Opens a folder of DTED cells.
///
/// The cells are the heights of the ground in the military format, and they are what a
/// survey of a field is planned with: the vehicle flies at a constant height over the
/// ground, and the ground is read from these files.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the folder cannot be read, or when
/// it holds no cell.
#[tauri::command]
async fn open_dted_folder(
    state: State<'_, AppState>,
    path: String,
) -> CommandResult<mpcraft_app::terrain::DtedLibraryInfo> {
    let (info, library) = mpcraft_app::terrain::open_dted(std::path::Path::new(&path))?;

    let mut cell = state
        .dted
        .lock()
        .map_err(|_| CommandError::internal("the cells are locked"))?;
    *cell = Some(library);

    Ok(info)
}

/// The height of the ground at a place, in metres above the sea.
///
/// The height comes from the folder of DTED cells that is open.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when no folder is open, or when the
/// height of that place is not known.
#[tauri::command]
async fn dted_elevation(
    state: State<'_, AppState>,
    latitude: f64,
    longitude: f64,
) -> CommandResult<f64> {
    let cell = state
        .dted
        .lock()
        .map_err(|_| CommandError::internal("the cells are locked"))?;

    let Some(library) = cell.as_ref() else {
        return Err(CommandError::invalid_request(
            "there is no folder of DTED cells open",
        ));
    };

    mpcraft_app::terrain::dted_elevation(library, latitude, longitude)
}

/// The zigzag of a survey that follows the ground.
///
/// The strips run across the line that joins the two places, and the vehicle flies at a
/// constant height above the ground.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when no folder is open, or when the
/// ground under the path is not known.
#[tauri::command]
async fn dted_path(
    state: State<'_, AppState>,
    from: PositionEvent,
    to: PositionEvent,
    strip_length: f64,
    strip_spacing: f64,
    clearance: f64,
    sample_step: f64,
) -> CommandResult<Vec<mpcraft_app::terrain::TerrainPathPoint>> {
    let cell = state
        .dted
        .lock()
        .map_err(|_| CommandError::internal("the cells are locked"))?;

    let Some(library) = cell.as_ref() else {
        return Err(CommandError::invalid_request(
            "there is no folder of DTED cells open",
        ));
    };

    let options = mpcraft_geo::terrain_path::TerrainPathOptions {
        strip_length_m: strip_length,
        strip_spacing_m: strip_spacing,
        clearance_m: clearance,
        sample_step_m: sample_step,
    };

    mpcraft_app::terrain::terrain_path(library, (from.lat, from.lng), (to.lat, to.lng), options)
}

/// The manifest of the firmware: the vehicles and the versions of their firmware.
///
/// The manifest is published by ArduPilot, and it is the list of what can be installed.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the manifest cannot be fetched, or
/// when it is not the document that was expected.
#[tauri::command]
async fn firmware_manifest() -> CommandResult<mpcraft_app::firmware::FirmwareManifest> {
    mpcraft_app::firmware::download_manifest(mpcraft_app::firmware::MANIFEST_URL).await
}

/// Fetches the file of a version of the firmware and writes it down.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the file cannot be fetched or
/// cannot be written.
#[tauri::command]
async fn firmware_download(url: String, output: String) -> CommandResult<u64> {
    mpcraft_app::firmware::download_file(&url, std::path::Path::new(&output)).await
}

/// Writes a bundle with what the application knows about itself.
///
/// The bundle is a zip with a file per subject: the machine, the connection profiles
/// and the state of the link. It is what is sent to whoever is asked to look at a
/// failure.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the bundle cannot be written.
#[tauri::command]
async fn diagnostics_bundle(state: State<'_, AppState>, output: String) -> CommandResult<String> {
    let entries = mpcraft_app::diagnostics::gather(&state);

    mpcraft_app::diagnostics::write_bundle(std::path::Path::new(&output), &entries)?;

    Ok(output)
}

/// Writes a firmware to a board, over the port the board is on.
///
/// The board has to be in its bootloader: it answers the questions of the protocol
/// instead of the MAVLink of a flight. The file is the one that the manifest of the
/// firmware points at.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the port cannot be opened, when the
/// file cannot be read, or when the board does not answer.
#[tauri::command]
async fn firmware_write_to_board(
    port: String,
    file: String,
) -> CommandResult<mpcraft_app::bootloader::BoardInfo> {
    let transport = SerialTransport::open(port, 115200, false).map_err(|error| {
        CommandError::internal(format!("the port could not be opened: {error}"))
    })?;

    let firmware = std::fs::read(&file).map_err(|error| {
        CommandError::invalid_request(format!("the file could not be read: {error}"))
    })?;

    mpcraft_app::bootloader::write_firmware(transport, &firmware, |_, _| {}).await
}

/// Erases the firmware of a board, over the port the board is on.
///
/// The board has to be in its bootloader. It is left without a firmware, which is what
/// the operator wants when the board does not start.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the port cannot be opened, or when
/// the board does not answer.
#[tauri::command]
async fn firmware_erase_board(port: String) -> CommandResult<mpcraft_app::bootloader::BoardInfo> {
    let transport = SerialTransport::open(port, 115200, false).map_err(|error| {
        CommandError::internal(format!("the port could not be opened: {error}"))
    })?;

    mpcraft_app::bootloader::erase_firmware(transport).await
}

/// Starts the web server, so that another program can follow the flight.
///
/// The server answers three things: the page that shows the flight, the state of the
/// vehicle as JSON, and the track of the flight as a KML document.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the port cannot be opened.
#[tauri::command]
async fn http_server_start(state: State<'_, AppState>, port: Option<u16>) -> CommandResult<u16> {
    // A server that is already running is stopped first: there is only one port. The
    // lock is let go before the server is opened, so that the task of the server is
    // never waiting for it.
    {
        let mut cell = state
            .http
            .lock()
            .map_err(|_| CommandError::internal("the server is locked"))?;

        if let Some(server) = cell.take() {
            server.stop();
        }
    }

    let port = port.unwrap_or(mpcraft_app::http::DEFAULT_PORT);
    let server =
        mpcraft_app::http::WebServer::start(port, std::sync::Arc::new(state.inner().clone()))
            .await?;

    let listening = server.port();

    let mut cell = state
        .http
        .lock()
        .map_err(|_| CommandError::internal("the server is locked"))?;
    *cell = Some(server);

    Ok(listening)
}

/// Stops the web server.
///
/// # Errors
///
/// Never fails.
#[tauri::command]
async fn http_server_stop(state: State<'_, AppState>) -> CommandResult<()> {
    let mut cell = state
        .http
        .lock()
        .map_err(|_| CommandError::internal("the server is locked"))?;

    if let Some(server) = cell.take() {
        server.stop();
    }

    Ok(())
}

/// Reboots the vehicle into its bootloader, so that a firmware can be written to it.
///
/// The vehicle is told to restart in the bootloader. The link is closed afterwards: the
/// board stops talking MAVLink when it is in the bootloader, and the port is then opened
/// again in the manner of the bootloader.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the vehicle
/// refuses the command.
#[tauri::command]
async fn flash_bootloader(state: State<'_, AppState>, req: VehicleRequest) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;

    state.supervisor.flash_bootloader(vehicle).await
}

/// Starts sending the corrections of a base station to the vehicle.
///
/// The corrections come from a caster, and the address of the caster is the one the
/// operator gives: `http://user:password@host:port/mount`. They go to the port of the
/// GPS of the vehicle.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, when the caster
/// cannot be reached, or when it refuses.
#[tauri::command]
async fn ntrip_start(
    state: State<'_, AppState>,
    req: VehicleRequest,
    url: String,
    gps_port: u8,
    version1: bool,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;

    let session = state
        .supervisor
        .session()
        .ok_or_else(CommandError::not_connected)?;

    state
        .ntrip
        .start(
            session,
            vehicle,
            mpcraft_app::ntrip::NtripSettings {
                url,
                gps_port,
                version1,
            },
        )
        .await
        .map_err(|error| CommandError::internal(error.to_string()))
}

/// Stops sending the corrections of a base station.
///
/// # Errors
///
/// Never fails.
#[tauri::command]
async fn ntrip_stop(state: State<'_, AppState>) -> CommandResult<()> {
    state.ntrip.stop();
    Ok(())
}

/// What the corrections of a base station are doing.
///
/// # Errors
///
/// Never fails.
#[tauri::command]
async fn ntrip_status(
    state: State<'_, AppState>,
) -> CommandResult<mpcraft_app::ntrip::NtripStatus> {
    Ok(state.ntrip.status())
}

/// The slots of the on-screen display, and what each of them holds.
///
/// The screens five and six are the ones the operator chooses. The answer is the name
/// of the parameter each slot holds, when the vehicle says it.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the
/// vehicle does not answer.
#[tauri::command]
async fn osd_slots(state: State<'_, AppState>, req: VehicleRequest) -> CommandResult<Vec<OsdSlot>> {
    let vehicle = resolve_vehicle(&state, &req)?;

    state.supervisor.osd_slots(vehicle).await
}

/// What a slot of the on-screen display is to hold, as the interface says it.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct OsdSlotRequest {
    /// The screen the slot is on.
    screen: u8,
    /// The number of the slot on that screen.
    index: u8,
    /// The name of the parameter the slot is to hold.
    name: String,
    /// The lowest value the parameter takes.
    min: f32,
    /// The highest value the parameter takes.
    max: f32,
    /// The step the value moves by.
    increment: f32,
}

/// Tells the vehicle what a slot of the on-screen display is to hold.
///
/// The slot holds a parameter, and the range of the parameter is said with it.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, when the vehicle
/// refuses the change, or when the vehicle does not answer.
#[tauri::command]
async fn osd_slot_config(
    state: State<'_, AppState>,
    req: VehicleRequest,
    slot: OsdSlotRequest,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;

    state
        .supervisor
        .osd_slot_config(
            vehicle,
            OsdSlotConfig {
                screen: slot.screen,
                index: slot.index,
                name: slot.name,
                config_type: mpcraft_mavlink::OsdParamConfigType::OSD_PARAM_NONE,
                min: slot.min,
                max: slot.max,
                increment: slot.increment,
            },
        )
        .await
}

/// The files the vehicle holds in a folder.
///
/// The list is what the vehicle answered with: the name of every entry, whether it is a
/// folder, and how big it is.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, when the vehicle
/// refuses the request, or when the vehicle does not answer.
#[tauri::command]
async fn ftp_list(
    state: State<'_, AppState>,
    path: String,
) -> CommandResult<Vec<mpcraft_link::FtpEntry>> {
    state.supervisor.ftp_list(&path).await
}

/// The content of a file the vehicle holds.
///
/// The bytes are returned as they were read, and the file is written down where the
/// operator says.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, when the vehicle
/// refuses the request, or when the file cannot be written.
#[tauri::command]
async fn ftp_read(
    state: State<'_, AppState>,
    path: String,
    output: Option<String>,
) -> CommandResult<FtpFileEvent> {
    let content = state.supervisor.ftp_read(&path).await?;

    if let Some(output) = output {
        std::fs::write(&output, &content).map_err(|error| {
            CommandError::internal(format!("the file could not be written: {error}"))
        })?;
    }

    Ok(FtpFileEvent {
        path,
        size: content.len(),
        content,
    })
}

/// A file the vehicle holds: its name, its size and its bytes.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FtpFileEvent {
    /// The name of the file.
    path: String,
    /// How many bytes the file has.
    size: usize,
    /// The bytes of the file.
    content: Vec<u8>,
}

/// The keys of the signed link.
///
/// The answer is the list of the keys that are saved, with their names and their
/// secrets.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the settings cannot be read.
#[tauri::command]
async fn list_signing_keys(state: State<'_, AppState>) -> CommandResult<Vec<SigningKeyEvent>> {
    Ok(state
        .signing_keys()?
        .into_iter()
        .map(|key| SigningKeyEvent {
            name: key.name,
            secret: key.secret,
        })
        .collect())
}

/// Saves the keys of the signed link.
///
/// The keys are replaced: the list that is sent is the list that is kept.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the settings cannot be written.
#[tauri::command]
async fn set_signing_keys(
    state: State<'_, AppState>,
    keys: Vec<SigningKeyEvent>,
) -> CommandResult<()> {
    let keys: Vec<mpcraft_settings::SigningKeyEntry> = keys
        .into_iter()
        .map(|key| mpcraft_settings::SigningKeyEntry::new(key.name, key.secret))
        .collect();

    state.set_signing_keys(&keys)?;
    state.apply_signing();

    Ok(())
}

/// A key of the signed link, as the interface sees it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SigningKeyEvent {
    /// The name the key is listed under.
    name: String,
    /// The secret, in hexadecimal.
    secret: String,
}

/// Writes bytes to a serial port of the vehicle.
///
/// The text travels as it is written: the bytes of the text are what the port of the
/// vehicle receives.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the link
/// cannot carry the message.
#[tauri::command]
async fn serial_control(
    state: State<'_, AppState>,
    req: VehicleRequest,
    device: u8,
    data: String,
    baudrate: u32,
    timeout: u16,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;

    state
        .supervisor
        .serial_control(vehicle, device, data.as_bytes(), baudrate, timeout)
        .await
}

/// The event that carries the telemetry of a log that is being played back.
const PLAYBACK_EVENT: &str = "playback-state";

/// What a log that is being played back says about itself.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaybackInfo {
    /// How long the flight lasted, in seconds.
    duration: f64,
    /// How many samples the flight holds.
    samples: usize,
    /// Whether a log is open.
    open: bool,
}

/// Opens a log to be played back.
///
/// The flight is read once, and the window can then be moved through it: the vehicle
/// is where it was, the graphs say what it said and the map shows the track it left.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the file cannot be read, or when it
/// does not hold a log.
#[tauri::command]
async fn playback_open(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> CommandResult<PlaybackInfo> {
    let playback = mpcraft_app::logs::Playback::open(std::path::Path::new(&path))?;

    let info = PlaybackInfo {
        duration: playback.duration(),
        samples: playback.len(),
        open: true,
    };

    {
        let mut held = state
            .playback
            .lock()
            .map_err(|_| CommandError::internal("the play-back is poisoned"))?;

        *held = playback;
    }

    // The window is told that the flight is being played back, so that the telemetry of
    // the link does not fight with it.
    let _ = app.emit(
        PLAYBACK_EVENT,
        PlaybackState {
            open: true,
            position: 0.0,
            duration: info.duration,
            telemetry: telemetry_of(&Telemetry::default()),
        },
    );

    Ok(info)
}

/// Moves the play-back of the log to the given second of the flight.
///
/// The telemetry of that instant is published, and the window is drawn as the flight
/// was at that moment.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when no log is open.
#[tauri::command]
async fn playback_seek(
    app: AppHandle,
    state: State<'_, AppState>,
    seconds: f64,
) -> CommandResult<()> {
    let (telemetry, duration) = {
        let mut held = state
            .playback
            .lock()
            .map_err(|_| CommandError::internal("the play-back is poisoned"))?;

        if held.is_empty() {
            return Err(CommandError::invalid_request(
                "there is no log to play back",
            ));
        }

        (held.seek(seconds), held.duration())
    };

    let _ = app.emit(
        PLAYBACK_EVENT,
        PlaybackState {
            open: true,
            position: seconds,
            duration,
            telemetry: telemetry_of(&telemetry),
        },
    );

    Ok(())
}

/// Closes the log that is being played back.
///
/// The window goes back to the vehicle on the link.
///
/// # Errors
///
/// Never fails.
#[tauri::command]
async fn playback_close(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    {
        let mut held = state
            .playback
            .lock()
            .map_err(|_| CommandError::internal("the play-back is poisoned"))?;

        *held = mpcraft_app::logs::Playback::default();
    }

    let _ = app.emit(
        PLAYBACK_EVENT,
        PlaybackState {
            open: false,
            position: 0.0,
            duration: 0.0,
            telemetry: telemetry_of(&Telemetry::default()),
        },
    );

    Ok(())
}

/// The state of a log that is being played back.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaybackState {
    /// Whether a log is open.
    open: bool,
    /// Where the play-back is, in seconds.
    position: f64,
    /// How long the flight lasted, in seconds.
    duration: f64,
    /// The telemetry of the flight at that instant.
    telemetry: TelemetryEvent,
}

/// The logs the vehicle holds in its own memory.
///
/// A log that is only on the vehicle is a log that is lost when the vehicle is
/// reformatted. The list is what the operator chooses from.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the
/// vehicle does not answer.
#[tauri::command]
async fn vehicle_logs(
    state: State<'_, AppState>,
    req: VehicleRequest,
) -> CommandResult<Vec<mpcraft_app::VehicleLog>> {
    let vehicle = resolve_vehicle(&state, &req)?;

    state.supervisor.vehicle_logs(vehicle).await
}

/// Fetches a log of the vehicle and writes it to a file.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, when the vehicle
/// does not answer, or when the file cannot be written.
#[tauri::command]
async fn download_log(
    state: State<'_, AppState>,
    req: VehicleRequest,
    id: u16,
    size: u32,
    path: String,
) -> CommandResult<()> {
    let vehicle = resolve_vehicle(&state, &req)?;

    state
        .supervisor
        .download_log(vehicle, id, size, std::path::Path::new(&path))
        .await
}

/// The samples of one message of a log.
///
/// The values are the ones the message carries, field by field: the grid the operator
/// reads is the message itself.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the file cannot be read, when it
/// does not hold a log, or when the log holds no message of that name.
#[tauri::command]
async fn log_message_samples(
    path: String,
    name: String,
    limit: usize,
) -> CommandResult<mpcraft_app::logs::MessageSamples> {
    mpcraft_app::logs::message_samples(std::path::Path::new(&path), &name, limit)
}

/// Fetches the tiles of a region and keeps them on the disk.
///
/// The tiles are the pictures of the ground, and a survey is flown where there is no
/// signal: the tiles of the field are fetched before the flight, and the map is drawn
/// with the ones that are there.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when a tile cannot be fetched, or when it
/// cannot be kept.
#[tauri::command]
async fn prefetch_tiles(
    state: State<'_, AppState>,
    template: String,
    provider: String,
    bounds: mpcraft_geo::tiles::Bounds,
    zooms: Vec<u8>,
) -> CommandResult<usize> {
    state
        .tiles
        .prefetch(&template, &provider, bounds, &zooms)
        .await
        .map_err(|error| CommandError::internal(error.to_string()))
}

/// The sources of tiles the map can draw with.
///
/// The sources are the ones that are free and that need neither an account nor a key.
///
/// # Errors
///
/// Never fails.
#[tauri::command]
async fn list_map_providers() -> CommandResult<Vec<mpcraft_geo::tiles::TileProvider>> {
    Ok(mpcraft_geo::tiles::providers())
}

/// One tile of the map.
///
/// The tile is taken from the folder the operator has brought, then from the disk where
/// the tiles that were fetched are kept, and from the service when it is in neither.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the tile cannot be fetched.
#[tauri::command]
async fn map_tile(
    state: State<'_, AppState>,
    provider: String,
    zoom: u8,
    x: u32,
    y: u32,
) -> CommandResult<Vec<u8>> {
    state
        .tiles
        .tile(&provider, zoom, x, y)
        .await
        .map_err(|error| CommandError::internal(error.to_string()))
}

/// The folder of tiles the operator has brought, when there is one.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the settings cannot be read.
#[tauri::command]
async fn map_injection_folder(state: State<'_, AppState>) -> CommandResult<Option<String>> {
    state.map_injection_folder()
}

/// Says which folder of tiles the operator has brought.
///
/// The folder is kept between two runs.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the settings cannot be written.
#[tauri::command]
async fn set_map_injection_folder(
    state: State<'_, AppState>,
    folder: Option<String>,
) -> CommandResult<()> {
    state.set_map_injection_folder(folder)
}

/// The track of a log: the places the vehicle went through.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the file cannot be read, or when it
/// does not hold a log.
#[tauri::command]
async fn log_track(path: String) -> CommandResult<Vec<mpcraft_app::logs::TrackPoint>> {
    mpcraft_app::logs::track(std::path::Path::new(&path))
}

/// The groups the parameters belong to.
///
/// The groups are the subjects the parameters are about — *Radio Calibration*, *EKF2* —
/// and upstream builds its configuration menu out of them.
///
/// # Errors
///
/// Never fails.
#[tauri::command]
async fn list_parameter_groups() -> CommandResult<Vec<String>> {
    Ok(mpcraft_params::metadata::groups()
        .into_iter()
        .map(str::to_owned)
        .collect())
}

/// One of the values a parameter can be given, when it is a choice.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ParamChoiceEvent {
    /// The value the parameter takes when this choice is made.
    code: f64,
    /// What the choice is called.
    text: String,
}

/// One of the flags of a mask.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ParamFlagEvent {
    /// The mask of the flag: the part of the number that is raised when it is on.
    mask: u32,
    /// What the flag is called.
    text: String,
}

/// The metadata of a parameter.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ParameterMetaEvent {
    /// The name of the parameter.
    name: String,
    /// The name it is shown with.
    display_name: String,
    /// The group it belongs to.
    group: String,
    /// The value it takes when it has never been changed, when the documentation knows
    /// it.
    default: String,
    /// What it does, in a line.
    description: String,
    /// What it does, at length.
    long_description: String,
    /// The unit the value is in.
    unit: String,
    /// The smallest value it takes.
    minimum: String,
    /// The largest value it takes.
    maximum: String,
    /// The values it can be given, when it is a choice.
    values: Vec<ParamChoiceEvent>,
    /// The flags of the mask, when the parameter is one.
    bitmask: Vec<ParamFlagEvent>,
    /// Whether a change to the parameter needs a restart to take effect.
    reboot_required: bool,
    /// The vehicle families it belongs to.
    vehicles: Vec<String>,
}

/// The parameters the user has marked as favourites.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the favourites cannot be read.
#[tauri::command]
async fn list_favourite_parameters(state: State<'_, AppState>) -> CommandResult<Vec<String>> {
    state.favourite_parameters()
}

/// Marks a parameter as a favourite, or takes the mark away.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the favourites cannot be written.
#[tauri::command]
async fn set_favourite_parameter(
    state: State<'_, AppState>,
    name: String,
    favourite: bool,
) -> CommandResult<()> {
    state.set_favourite_parameter(&name, favourite)
}

/// The pre-flight checklist, checked against the vehicle.
///
/// The rules are the vehicle's: the interface only shows what they say.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link.
#[tauri::command]
async fn preflight_checklist(
    state: State<'_, AppState>,
) -> CommandResult<Vec<ChecklistEntryEvent>> {
    let session = state
        .supervisor
        .state()
        .ok_or_else(CommandError::not_connected)?;

    let quality = mpcraft_app::checklist::link_quality(
        session.loss.packets_received,
        session.loss.packets_lost,
    );

    let entries = mpcraft_app::checklist::evaluate(
        mpcraft_app::checklist::items(),
        &session.telemetry,
        &session.parameters,
        quality,
    );

    Ok(entries
        .into_iter()
        .map(|entry| ChecklistEntryEvent {
            description: entry.description,
            text: entry.text,
            passed: entry.passed,
            known: entry.known,
            manual: entry.manual,
        })
        .collect())
}

/// One line of the pre-flight checklist.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistEntryEvent {
    /// What the item is about.
    description: String,
    /// The text of the item, with the value in it.
    text: String,
    /// Whether the condition holds.
    passed: bool,
    /// Whether the value was known at all.
    known: bool,
    /// Whether the pilot answers this one by hand.
    manual: bool,
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
    state.graph_series(&name, since)
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

/// Downloads the plan the vehicle keeps in a file of its own.
///
/// The plan is not asked for with the mission protocol: it is read from the file the
/// vehicle keeps it in, over the file transfer protocol.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, when the vehicle
/// refuses the request, or when the file does not hold a plan.
#[tauri::command]
async fn download_mission_ftp(
    state: State<'_, AppState>,
    kind: PlanKind,
) -> CommandResult<PlanInfo> {
    let plan = state
        .supervisor
        .download_plan_ftp(mission_type_of(kind))
        .await?;

    Ok(PlanInfo::from(&plan))
}

/// Writes a plan to the file the vehicle keeps it in.
///
/// The plan is not written with the mission protocol: it is written to the file the
/// vehicle keeps it in, over the file transfer protocol.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, or when the
/// vehicle refuses the plan.
#[tauri::command]
async fn upload_mission_ftp(
    state: State<'_, AppState>,
    kind: PlanKind,
    items: Vec<PlanItemRequest>,
) -> CommandResult<()> {
    let plan = plan_of(kind, &items)?;

    state.supervisor.upload_plan_ftp(&plan).await
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

/// Writes a part of a plan to the vehicle, leaving the rest of it alone.
///
/// The vehicle is told which range of the plan is coming, and it asks for the items one
/// at a time. The answer says which items the vehicle took.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when there is no link, when the vehicle
/// refuses the range, or when it does not answer.
#[tauri::command]
async fn upload_mission_partial(
    state: State<'_, AppState>,
    req: VehicleRequest,
    kind: PlanKind,
    start: u16,
    items: Vec<PlanItemRequest>,
) -> CommandResult<PartialUpdateInfo> {
    let vehicle = resolve_vehicle(&state, &req)?;
    let plan = plan_of(kind, &items)?;

    let report = state
        .supervisor
        .upload_mission_partial(vehicle, plan.mission_type, start, &plan.items)
        .await?;

    Ok(PartialUpdateInfo {
        start: report.start,
        accepted: report.accepted,
    })
}

/// What a partial update of a plan was.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PartialUpdateInfo {
    /// The first item that was written.
    start: u16,
    /// The items the vehicle took.
    accepted: Vec<u16>,
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

/// Saves the connection profiles.
///
/// The profiles are the ones the operator has named, and the file is read, the profiles
/// are replaced and the file is written back: the other settings are left alone.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the profiles cannot be written.
#[tauri::command]
async fn set_connection_profiles(
    state: State<'_, AppState>,
    profiles: Vec<ProfileEvent>,
) -> CommandResult<()> {
    let profiles: Vec<ConnectionProfile> = profiles
        .into_iter()
        .map(|profile| ConnectionProfile {
            name: profile.name,
            target: profile.target,
            auto_connect: profile.auto_connect,
        })
        .collect();

    state.set_connection_profiles(&profiles)
}

/// A connection profile, as the interface sees it./// A connection profile, as the interface sees it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    /// The board that is on the other end, when the vehicle has said which one it is.
    board: Option<String>,
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

    /// The state of the second global positioning, when it is known.
    gps2: Option<GpsEvent>,

    /// The wind, when it is known.
    wind: Option<WindEvent>,
    /// What came back from a serial port of the vehicle, when something did.
    passthrough: Option<PassthroughEvent>,
    /// The state of the engine, when the vehicle has told it.
    efi: Option<EfiEvent>,
    /// The identification the vehicle broadcasts, when it broadcasts one.
    drone_id: Option<DroneIdEvent>,
    /// The inertial measurement, when it is known.
    imu: Option<ImuEvent>,
    /// The barometer, when it is known.
    barometer: Option<BarometerEvent>,
    /// The servos and the radio control, when they are known.
    controls: Option<ControlsEvent>,
    /// The air data, when it is known.
    airspeed: Option<AirspeedEvent>,
    /// The state of the estimator, when it is known.
    ekf: Option<EkfEvent>,
    /// The vibration of the airframe, when it is known.
    vibration: Option<VibrationEvent>,
    /// The battery banks the vehicle has reported.
    batteries: Vec<BatteryBankEvent>,
    /// The attitude of the second estimator, when it is known.
    ahrs2: Option<AttitudeEvent>,
    /// What the flight controller is asking for, when it is known.
    controller: Option<ControllerEvent>,
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

/// The inertial measurement of the vehicle.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ImuEvent {
    /// The acceleration on each axis, in metres per second squared.
    acceleration: [f64; 3],
    /// The angular rate on each axis, in radians per second.
    angular_rate: [f64; 3],
    /// The magnetic field on each axis, in gauss.
    magnetic_field: [f64; 3],
    /// The temperature of the sensor, in degrees Celsius.
    temperature: f64,
}

/// The barometer of the vehicle.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BarometerEvent {
    /// The absolute pressure, in hectopascals.
    absolute_pressure: f64,
    /// The differential pressure, in hectopascals.
    differential_pressure: f64,
    /// The temperature, in degrees Celsius.
    temperature: f64,
}

/// The servos and the radio control.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ControlsEvent {
    /// The pulse width of each of the first eight outputs, in microseconds.
    outputs: [Option<f64>; 8],
    /// The value of each of the first eight radio channels.
    inputs: [Option<f64>; 8],
    /// The strength of the radio signal, as a percentage.
    signal: Option<f64>,
}

/// The air data of the vehicle.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AirspeedEvent {
    /// The speed through the air, in metres per second.
    airspeed: Option<f64>,
    /// The speed over the ground, in metres per second.
    ground_speed: f64,
    /// The rate of climb, in metres per second.
    climb: f64,
    /// The throttle, as a percentage.
    throttle: f64,
}

/// The state of the estimator of the vehicle.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct EkfEvent {
    /// The variance of the velocity estimate.
    velocity_variance: f64,
    /// The variance of the horizontal position estimate.
    horizontal_variance: f64,
    /// The variance of the vertical position estimate.
    vertical_variance: f64,
    /// The variance of the compass.
    compass_variance: f64,
    /// The variance of the terrain altitude.
    terrain_variance: f64,
    /// The variance of the airspeed estimate.
    airspeed_variance: f64,
}

/// The vibration of the airframe.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct VibrationEvent {
    /// The vibration on the first axis, in metres per second squared.
    x: f64,
    /// The vibration on the second axis, in metres per second squared.
    y: f64,
    /// The vibration on the third axis, in metres per second squared.
    z: f64,
}

/// The state of one battery bank.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BatteryBankEvent {
    /// The identifier the vehicle gives the bank.
    id: u8,
    /// The voltage of each cell, in volts.
    cells: Vec<f64>,
    /// The current of the bank, in amperes.
    current: f64,
    /// The remaining charge of the bank, as a percentage.
    remaining: f64,
    /// The temperature of the bank, in degrees Celsius.
    temperature: f64,
}

/// What the flight controller is asking the vehicle to do.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ControllerEvent {
    /// The roll the controller asks for, in radians.
    roll: f64,
    /// The pitch the controller asks for, in radians.
    pitch: f64,
    /// The error of the altitude, in metres.
    altitude_error: f64,
    /// The error of the airspeed.
    airspeed_error: f64,
    /// The error of the cross-track, in metres.
    cross_track_error: f64,
    /// The distance to the waypoint the vehicle is flying towards, in metres.
    waypoint_distance: f64,
}

/// The identification a vehicle broadcasts.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DroneIdEvent {
    /// What the identification is: a registration, a serial number, a session.
    kind: Option<String>,
    /// The identification itself.
    id: Option<String>,
    /// What the vehicle is: a multicopter, a fixed wing, a helicopter.
    kind_of_vehicle: Option<String>,
    /// The place the vehicle says it is.
    location: Option<DroneIdLocationEvent>,
    /// What the operator says about the vehicle.
    description: Option<String>,
}

/// The place a vehicle says it is.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DroneIdLocationEvent {
    /// The latitude, in degrees.
    latitude: f64,
    /// The longitude, in degrees.
    longitude: f64,
    /// The height over the sea, in metres.
    altitude: f64,
    /// The height over the ground, in metres.
    height: f64,
    /// The speed over the ground, in metres per second.
    speed: f64,
    /// The speed upwards, in metres per second.
    vertical_speed: f64,
    /// The direction the vehicle is going, in degrees.
    direction: f64,
}

/// The state of the engine of a vehicle that has one.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct EfiEvent {
    /// The revolutions of the engine per minute.
    rpm: f64,
    /// The fuel that has been burnt, in cubic centimetres.
    fuel_consumed: f64,
    /// The fuel that flows now, in cubic centimetres per hour.
    fuel_flow: f64,
    /// The load of the engine, as a percentage.
    engine_load: f64,
    /// The pressure of the barometric, in kilopascals.
    barometric_pressure: f64,
    /// The pressure of the fuel, in kilopascals.
    fuel_pressure: f64,
    /// The temperature of the intake, in degrees Celsius.
    intake_temperature: f64,
    /// The temperature of the head of the cylinder, in degrees Celsius.
    head_temperature: f64,
    /// The temperature of the exhaust, in degrees Celsius.
    exhaust_temperature: f64,
}

/// What came back from a serial port of the vehicle.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PassthroughEvent {
    /// The port the bytes came from.
    device: u8,
    /// The bytes, as they came.
    data: Vec<u8>,
}

/// The wind where the vehicle is.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WindEvent {
    /// The direction the wind comes from, in degrees.
    direction: f64,
    /// The speed of the wind, in metres per second.
    speed: f64,
    /// The vertical speed of the air, in metres per second.
    vertical_speed: f64,
}

/// A position.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
            // The board is told by the output of the vehicle when it starts, and the
            // output is captured by the link.
            board: state.and_then(|state| mpcraft_app::board::detect(&state.plaintext)),
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
    telemetry_of(&state.telemetry)
}

/// The telemetry of a vehicle, as the interface sees it.
///
/// The telemetry of a link comes from the state of the session, and the telemetry
/// of a log that is being played back comes from the log. The shape of the event
/// is the same either way: the interface does not have to know where the numbers
/// came from.
fn telemetry_of(telemetry: &Telemetry) -> TelemetryEvent {
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
            // The name of the mode is the one the vehicle calls it: the list of the
            // modes is the backend's, and the number the vehicle says is looked up in
            // it. A mode the list does not know is shown by its number, because the
            // operator would rather see the number than nothing.
            mode.name.clone().unwrap_or_else(|| {
                flight_modes()
                    .iter()
                    .find(|entry| entry.value == mode.custom_mode)
                    .map(|entry| entry.name.clone())
                    .unwrap_or_else(|| format!("mode {}", mode.custom_mode))
            })
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
        gps2: telemetry.gps2.as_ref().map(|gps| GpsEvent {
            fix_type: gps.fix_type as u8,
            satellites: gps.satellites,
            hdop: gps.hdop,
        }),

        passthrough: telemetry
            .passthrough
            .as_ref()
            .map(|passthrough| PassthroughEvent {
                device: passthrough.device,
                data: passthrough.data.clone(),
            }),
        wind: telemetry.wind.map(|wind| WindEvent {
            direction: wind.direction,
            speed: wind.speed,
            vertical_speed: wind.vertical_speed,
        }),

        efi: telemetry.efi.map(|efi| EfiEvent {
            rpm: efi.rpm,
            fuel_consumed: efi.fuel_consumed,
            fuel_flow: efi.fuel_flow,
            engine_load: efi.engine_load,
            barometric_pressure: efi.barometric_pressure,
            fuel_pressure: efi.fuel_pressure,
            intake_temperature: efi.intake_temperature,
            head_temperature: efi.head_temperature,
            exhaust_temperature: efi.exhaust_temperature,
        }),

        drone_id: telemetry.drone_id.as_ref().map(|drone_id| DroneIdEvent {
            kind: drone_id.kind.clone(),
            id: drone_id.id.clone(),
            kind_of_vehicle: drone_id.kind_of_vehicle.clone(),
            description: drone_id.description.clone(),
            location: drone_id.location.map(|location| DroneIdLocationEvent {
                latitude: location.latitude,
                longitude: location.longitude,
                altitude: location.altitude,
                height: location.height,
                speed: location.speed,
                vertical_speed: location.vertical_speed,
                direction: location.direction,
            }),
        }),
        imu: telemetry.imu.map(|imu| ImuEvent {
            acceleration: imu.acceleration,
            angular_rate: imu.angular_rate,
            magnetic_field: imu.magnetic_field,
            temperature: imu.temperature,
        }),
        barometer: telemetry.barometer.map(|barometer| BarometerEvent {
            absolute_pressure: barometer.absolute_pressure,
            differential_pressure: barometer.differential_pressure,
            temperature: barometer.temperature,
        }),
        controls: telemetry.controls.map(|controls| ControlsEvent {
            outputs: controls.outputs,
            inputs: controls.inputs,
            signal: controls.signal,
        }),
        airspeed: telemetry.airspeed.map(|airspeed| AirspeedEvent {
            airspeed: airspeed.airspeed,
            ground_speed: airspeed.ground_speed,
            climb: airspeed.climb,
            throttle: airspeed.throttle,
        }),
        ekf: telemetry.ekf.map(|ekf| EkfEvent {
            velocity_variance: ekf.velocity_variance,
            horizontal_variance: ekf.horizontal_variance,
            vertical_variance: ekf.vertical_variance,
            compass_variance: ekf.compass_variance,
            terrain_variance: ekf.terrain_variance,
            airspeed_variance: ekf.airspeed_variance,
        }),
        vibration: telemetry.vibration.map(|vibration| VibrationEvent {
            x: vibration.x,
            y: vibration.y,
            z: vibration.z,
        }),
        batteries: telemetry
            .batteries
            .iter()
            .map(|bank| BatteryBankEvent {
                id: bank.id,
                cells: bank.cells.clone(),
                current: bank.current,
                remaining: bank.remaining,
                temperature: bank.temperature,
            })
            .collect(),
        ahrs2: telemetry.ahrs2.map(|attitude| AttitudeEvent {
            roll: attitude.roll.to_degrees(),
            pitch: attitude.pitch.to_degrees(),
            yaw: attitude.yaw.to_degrees(),
        }),
        controller: telemetry.controller.map(|controller| ControllerEvent {
            roll: controller.roll,
            pitch: controller.pitch,
            altitude_error: controller.altitude_error,
            airspeed_error: controller.airspeed_error,
            cross_track_error: controller.cross_track_error,
            waypoint_distance: controller.waypoint_distance,
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

    // A panic takes the application down with it, and the reason is only in the
    // message of the panic. It is written down where it can be found afterwards.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "an unknown place".to_owned());

        let message = format!("{info}");

        let path = std::env::temp_dir().join("mpcraft-panic.txt");
        let _ = std::fs::write(&path, format!("{location}\n\n{message}\n"));

        tracing::error!("the application fell over at {location}: {message}");
        previous(info);
    }));

    // A ground station is often launched from a shortcut that points at a particular
    // vehicle, so the link target can be given on the command line.
    let target = target_from_arguments();

    tauri::Builder::default()
        .manage(AppState::default())
        // The file dialog, for the plans an operator saves and loads.
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            connect,
            disconnect,
            connection_status,
            list_connection_profiles,
            set_connection_profiles,
            list_flight_modes,
            list_serial_ports,
            list_graphs,
            preflight_checklist,
            list_favourite_parameters,
            list_parameter_groups,
            export_parameters,
            import_parameters,
            compare_parameters,
            write_parameters_to_flash,
            set_position_target,
            output_functions,
            motor_test,
            calibrate,
            rc_override,
            rc_override_release,
            manual_control,
            reboot,
            plan_terrain_profile,
            vehicle_coordinates,
            set_favourite_parameter,
            parameter_metadata,
            list_mav_commands,
            graph_series,
            log_summary,
            prefetch_tiles,
            list_map_providers,
            map_tile,
            map_injection_folder,
            set_map_injection_folder,
            log_message_samples,
            vehicle_logs,
            download_log,
            playback_open,
            playback_seek,
            playback_close,
            log_series,
            log_track,
            log_export_kml,
            log_export_csv,
            start_live_kml,
            stop_live_kml,
            home_position,
            set_home_position,
            open_dted_folder,
            dted_elevation,
            dted_path,
            firmware_manifest,
            firmware_download,
            firmware_write_to_board,
            firmware_erase_board,
            flash_bootloader,
            ftp_list,
            ftp_read,
            osd_slots,
            osd_slot_config,
            ntrip_start,
            ntrip_stop,
            ntrip_status,
            http_server_start,
            http_server_stop,
            list_signing_keys,
            set_signing_keys,
            serial_control,
            diagnostics_bundle,
            list_parameters,
            read_parameters,
            write_parameter,
            write_parameters_batch,
            validate_mission,
            export_plan,
            import_plan,
            arm,
            disarm,
            takeoff,
            set_mode,
            download_mission,
            download_mission_ftp,
            upload_mission,
            upload_mission_ftp,
            upload_mission_partial,
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
                    let feeder = state.clone();
                    let result = state
                        .supervisor
                        .connect(&target, move |snapshot| {
                            // The graphs are drawn from the history of the telemetry, and
                            // the track that is being streamed to a file is written from
                            // the same state.
                            if let Some(session) = &snapshot.session {
                                if let Err(error) = feeder.feed(session) {
                                    tracing::warn!("cannot write the track: {error}");
                                }
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

    /// The name the model gives a field, in the spelling the interface uses.
    fn camel(name: &str) -> String {
        let mut out = String::new();
        let mut upper = false;

        for character in name.chars() {
            if character == '_' {
                upper = true;
                continue;
            }

            if upper {
                out.extend(character.to_uppercase());
                upper = false;
            } else {
                out.push(character);
            }
        }

        out
    }

    #[test]
    fn the_event_carries_every_field_of_the_telemetry() {
        // The event is written by hand, and a field of the model that is forgotten in it
        // is a value the interface never sees. The model is the specification, and this
        // is what says that the event has caught up with it.
        let model = serde_json::to_value(mpcraft_vehicle::Telemetry::default())
            .expect("the telemetry is written");
        let event = serde_json::to_value(TelemetryEvent::default()).expect("the event is written");

        let model = model.as_object().expect("the telemetry is an object");
        let event = event.as_object().expect("the event is an object");

        for name in model.keys() {
            let name = camel(name);

            assert!(
                event.contains_key(&name),
                "the event does not carry {name}, so the interface never sees it"
            );
        }
    }

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
