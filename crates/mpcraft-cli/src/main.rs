//! The `mpcraft` command line interface.
//!
//! The CLI exists for three reasons:
//!
//! * it proves the core crates are usable without a display server,
//! * it gives CI a way to drive SITL without the GUI,
//! * it gives advanced users a scriptable interface, the way Mission Planner's scripting
//!   host did.
//!
//! ```text
//! mpcraft-cli ports
//! mpcraft-cli connect udp:127.0.0.1:14550 --monitor
//! mpcraft-cli --target udp:127.0.0.1:14550 params list
//! mpcraft-cli --target udp:127.0.0.1:14550 mission download --output plan.waypoints
//! mpcraft-cli --target udp:127.0.0.1:14550 mission upload plan.waypoints
//! mpcraft-cli --target udp:127.0.0.1:14550 arm
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use mpcraft_mission::{MissionType, Plan};
use mpcraft_transport::{ConnectionSpec, SerialTransport};

/// How long to wait for the vehicle to answer.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// The MPCraft command line interface.
#[derive(Debug, Parser)]
#[command(
    name = "mpcraft-cli",
    version,
    about = "MPCraft command line interface"
)]
struct Cli {
    /// The connection target, for example `udp:127.0.0.1:14550`.
    ///
    /// This is a global option so that the subcommand reads naturally:
    /// `mpcraft-cli --target udp:127.0.0.1:14550 params list`.
    #[arg(short, long, global = true)]
    target: Option<String>,

    /// The vehicle to talk to, as `system/component`.
    #[arg(short, long, global = true, default_value = "1/1")]
    vehicle: String,

    /// The action to perform.
    #[command(subcommand)]
    command: Command,
}

/// The available actions.
#[derive(Debug, Subcommand)]
enum Command {
    /// List the serial ports available on this machine.
    Ports,

    /// Open a link to a vehicle.
    Connect {
        /// The connection target, overriding the global option.
        target: Option<String>,

        /// Print live telemetry as it arrives.
        #[arg(long)]
        monitor: bool,
    },

    /// Work with the vehicle's parameters.
    Params {
        /// The parameter action to perform.
        #[command(subcommand)]
        action: ParamsAction,
    },

    /// Work with the vehicle's mission.
    Mission {
        /// The mission action to perform.
        #[command(subcommand)]
        action: MissionAction,
    },

    /// Arm the vehicle.
    Arm,

    /// Disarm the vehicle.
    Disarm,

    /// Change the flight mode.
    Mode {
        /// The mode, by name or by its custom mode number.
        mode: String,
    },

    /// Take off to an altitude, relative to the home position.
    Takeoff {
        /// The altitude in metres.
        altitude: f32,
    },
}

/// The parameter actions.
#[derive(Debug, Subcommand)]
enum ParamsAction {
    /// List every parameter on the vehicle.
    List {
        /// Print only parameters whose name contains this text.
        #[arg(long)]
        filter: Option<String>,
    },
    /// Read a single parameter.
    Get {
        /// The parameter name.
        name: String,
    },
    /// Write a single parameter.
    Set {
        /// The parameter name.
        name: String,
        /// The new value.
        value: f64,
    },
    /// Write the parameters of the vehicle to a file.
    Export {
        /// The path of the file.
        path: std::path::PathBuf,
    },
    /// Write the parameters of a file to the vehicle.
    Import {
        /// The path of the file.
        path: std::path::PathBuf,
    },
    /// Say how the parameters of the vehicle differ from the ones of a file.
    Diff {
        /// The path of the file.
        path: std::path::PathBuf,
    },
}

/// The mission actions.
#[derive(Debug, Subcommand)]
enum MissionAction {
    /// Download the mission and print it.
    Download {
        /// Write the mission to this file instead of only printing it.
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,

        /// The format of the file: `waypoints` or `kml`.
        #[arg(short, long, default_value = "waypoints")]
        format: String,

        /// The kind of plan: `mission`, `fence` or `rally`.
        #[arg(short, long, default_value = "mission")]
        kind: String,
    },

    /// Upload a mission from a file.
    Upload {
        /// The path to the mission file.
        path: std::path::PathBuf,

        /// The kind of plan: `mission`, `fence` or `rally`.
        #[arg(short, long, default_value = "mission")]
        kind: String,
    },

    /// Say which item of the plan the vehicle is executing.
    SetCurrent {
        /// The sequence number of the item.
        seq: u16,
    },

    /// Empty one of the vehicle's plans.
    Clear {
        /// The kind of plan: `mission`, `fence` or `rally`.
        #[arg(short, long, default_value = "mission")]
        kind: String,
    },

    /// Jump to an item of the plan.
    Jump {
        /// The sequence number of the item.
        seq: u16,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // The CLI drives the same asynchronous session layer the desktop application uses.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("error: cannot start the runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run(&cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Runs the requested action.
async fn run(cli: &Cli) -> Result<(), String> {
    let vehicle = parse_vehicle(&cli.vehicle)?;

    match &cli.command {
        Command::Ports => list_ports(),
        Command::Connect { target, monitor } => {
            let target = target
                .clone()
                .or_else(|| cli.target.clone())
                .ok_or_else(|| "no connection target given".to_owned())?;
            connect(&target, *monitor).await
        }
        Command::Params { action } => {
            let session = open_link(cli).await?;
            params(&session, vehicle, action).await
        }
        Command::Mission { action } => {
            let session = open_link(cli).await?;
            mission(&session, vehicle, action).await
        }
        Command::Arm => {
            let session = open_link(cli).await?;
            session
                .arm(vehicle, DEFAULT_TIMEOUT)
                .await
                .map_err(|error| format!("cannot arm the vehicle: {error}"))?;
            println!("armed");
            Ok(())
        }
        Command::Disarm => {
            let session = open_link(cli).await?;
            session
                .disarm(vehicle, DEFAULT_TIMEOUT)
                .await
                .map_err(|error| format!("cannot disarm the vehicle: {error}"))?;
            println!("disarmed");
            Ok(())
        }
        Command::Mode { mode } => {
            let custom_mode = parse_mode(mode)?;
            let session = open_link(cli).await?;
            session
                .set_mode(
                    vehicle,
                    custom_mode,
                    mpcraft_mavlink::MavModeFlag::empty(),
                    DEFAULT_TIMEOUT,
                )
                .await
                .map_err(|error| format!("cannot change the mode: {error}"))?;
            println!("the mode is now {custom_mode}");
            Ok(())
        }
        Command::Takeoff { altitude } => {
            let session = open_link(cli).await?;
            session
                .takeoff(vehicle, *altitude, DEFAULT_TIMEOUT)
                .await
                .map_err(|error| format!("cannot take off: {error}"))?;
            println!("taking off to {altitude} metres");
            Ok(())
        }
    }
}

/// Parses the `system/component` pair of the vehicle to talk to.
fn parse_vehicle(text: &str) -> Result<mpcraft_link::VehicleId, String> {
    let (system, component) = text
        .split_once('/')
        .ok_or_else(|| format!("the vehicle must be given as system/component, not {text:?}"))?;

    let system = system
        .trim()
        .parse::<u8>()
        .map_err(|_| format!("the system id must be a number, not {system:?}"))?;
    let component = component
        .trim()
        .parse::<u8>()
        .map_err(|_| format!("the component id must be a number, not {component:?}"))?;

    Ok(mpcraft_link::VehicleId::new(system, component))
}

/// Parses a flight mode, by name or by its custom mode number.
///
/// The table lives in the vehicle model, so the command line and the interface offer the
/// same modes.
fn parse_mode(text: &str) -> Result<u32, String> {
    let trimmed = text.trim();

    if let Ok(number) = trimmed.parse::<u32>() {
        return Ok(number);
    }

    mpcraft_vehicle::modes::mode_value(trimmed).ok_or_else(|| {
        let names: Vec<&str> = mpcraft_vehicle::modes::COMMON_MODES
            .iter()
            .map(|entry| entry.name)
            .collect();

        format!(
            "unknown mode {text:?}; give a number, or one of: {}",
            names.join(", ")
        )
    })
}

/// Opens a link to the vehicle.
async fn open_link(cli: &Cli) -> Result<mpcraft_link::Session, String> {
    let target = cli
        .target
        .clone()
        .ok_or_else(|| "no connection target given".to_owned())?;

    let spec = ConnectionSpec::parse(&target).map_err(|error| error.to_string())?;

    mpcraft_link::Session::connect(&spec, mpcraft_link::SessionOptions::default())
        .await
        .map_err(|error| format!("cannot open the link: {error}"))
}

/// Lists the serial ports this machine has.
fn list_ports() -> Result<(), String> {
    let ports = SerialTransport::available_ports().map_err(|e| e.to_string())?;

    if ports.is_empty() {
        println!("no serial ports found");
        return Ok(());
    }

    for port in ports {
        match port.description {
            Some(description) => println!("{:<24} {description}", port.device),
            None => println!("{}", port.device),
        }
    }

    Ok(())
}

/// Opens a link to a vehicle.
async fn connect(target: &str, monitor: bool) -> Result<(), String> {
    let spec = ConnectionSpec::parse(target).map_err(|e| e.to_string())?;

    println!("connecting to {spec}");

    let session = mpcraft_link::Session::connect(&spec, mpcraft_link::SessionOptions::default())
        .await
        .map_err(|e| format!("cannot open the link: {e}"))?;

    println!("link open");

    if !monitor {
        return Ok(());
    }

    monitor_session(&session).await
}

/// Runs a parameter operation against the vehicle.
async fn params(
    session: &mpcraft_link::Session,
    vehicle: mpcraft_link::VehicleId,
    action: &ParamsAction,
) -> Result<(), String> {
    match action {
        ParamsAction::List { filter } => {
            let store = session
                .download_parameters(vehicle, DEFAULT_TIMEOUT)
                .await
                .map_err(|e| format!("cannot read the parameters: {e}"))?;

            println!(
                "{} parameters received, {} reported by the vehicle",
                store.len(),
                store.total_reported()
            );

            for parameter in store.iter() {
                if let Some(filter) = filter {
                    if !parameter.name.contains(filter.as_str()) {
                        continue;
                    }
                }

                println!("{:<24} {}", parameter.name, parameter.value);
            }

            Ok(())
        }

        ParamsAction::Get { name } => {
            let parameter = session
                .read_parameter(vehicle, name, DEFAULT_TIMEOUT)
                .await
                .map_err(|e| format!("cannot read {name}: {e}"))?;

            println!("{:<24} {}", parameter.name, parameter.value);
            Ok(())
        }

        ParamsAction::Set { name, value } => {
            // The reported type of the parameter decides how the value is encoded, so the
            // parameter has to be known before it can be written.
            let current = session
                .read_parameter(vehicle, name, DEFAULT_TIMEOUT)
                .await
                .map_err(|e| format!("cannot read {name}: {e}"))?;

            if current.value == *value {
                println!("{name} is already {value}; nothing to do");
                return Ok(());
            }

            let written = session
                .write_parameter(vehicle, name, *value, DEFAULT_TIMEOUT)
                .await
                .map_err(|e| format!("cannot write {name}: {e}"))?;

            println!("{:<24} {}", written.name, written.value);
            Ok(())
        }

        ParamsAction::Export { path } => {
            let store = session
                .download_parameters(vehicle, DEFAULT_TIMEOUT)
                .await
                .map_err(|e| format!("cannot read the parameters: {e}"))?;

            let text = mpcraft_params::param_file::write(&store);

            std::fs::write(path, text)
                .map_err(|e| format!("cannot write {}: {e}", path.display()))?;

            println!("{} parameters written to {}", store.len(), path.display());
            Ok(())
        }

        ParamsAction::Import { path } => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;

            let parameters = mpcraft_params::param_file::read(&text)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;

            // The parameters of the file are written one by one, because the vehicle
            // confirms each of them: a file of two hundred parameters is two hundred
            // writes, and a vehicle that refuses one of them says so.
            let mut written = 0usize;

            for parameter in &parameters {
                let stored = session
                    .write_parameter(vehicle, &parameter.name, parameter.value, DEFAULT_TIMEOUT)
                    .await
                    .map_err(|e| format!("cannot write {}: {e}", parameter.name))?;

                written += 1;

                if stored.value != parameter.value {
                    println!(
                        "{:<24} the vehicle stored {} instead of {}",
                        stored.name, stored.value, parameter.value
                    );
                }
            }

            println!("{written} parameters written to the vehicle");
            Ok(())
        }

        ParamsAction::Diff { path } => {
            let store = session
                .download_parameters(vehicle, DEFAULT_TIMEOUT)
                .await
                .map_err(|e| format!("cannot read the parameters: {e}"))?;

            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;

            let parameters = mpcraft_params::param_file::read(&text)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;

            let mut other = mpcraft_params::ParameterStore::new();
            for parameter in parameters {
                other.insert(parameter);
            }

            let differences = mpcraft_params::param_file::differences(&store, &other);

            if differences.is_empty() {
                println!("the vehicle and the file agree");
                return Ok(());
            }

            for difference in &differences {
                let current = difference
                    .current
                    .map_or_else(|| "-".to_owned(), |value| value.to_string());
                let other = difference
                    .other
                    .map_or_else(|| "-".to_owned(), |value| value.to_string());

                println!("{:<24} vehicle {current:<14} file {other}", difference.name);
            }

            println!("{} parameters differ", differences.len());
            Ok(())
        }
    }
}

/// Runs a mission operation against the vehicle.
async fn mission(
    session: &mpcraft_link::Session,
    vehicle: mpcraft_link::VehicleId,
    action: &MissionAction,
) -> Result<(), String> {
    match action {
        MissionAction::Download {
            output,
            format,
            kind,
        } => {
            let mission_type = parse_kind(kind)?;

            let plan = session
                .download_mission(vehicle, mission_type, DEFAULT_TIMEOUT)
                .await
                .map_err(|error| format!("cannot download the plan: {error}"))?;

            print_plan(&plan);

            if let Some(path) = output {
                let text = match format.as_str() {
                    "waypoints" | "wpl" | "qgc" => mpcraft_mission::qgc::write_waypoints(&plan),
                    "kml" => mpcraft_mission::kml::write_kml(&plan),
                    other => {
                        return Err(format!("unknown format {other:?}; use waypoints or kml"));
                    }
                };

                std::fs::write(path, text)
                    .map_err(|error| format!("cannot write {}: {error}", path.display()))?;

                println!("the plan was written to {}", path.display());
            }

            Ok(())
        }

        MissionAction::Upload { path, kind } => {
            let mission_type = parse_kind(kind)?;

            let text = std::fs::read_to_string(path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;

            let plan = read_plan(&text, path)?;

            // The kind of the file and the kind that was asked for have to agree: a fence
            // written into the mission plan would be a fence the vehicle never flies.
            if plan.mission_type != mission_type {
                println!(
                    "the file holds a {:?} plan, but a {:?} one was asked for",
                    plan.mission_type, mission_type
                );
            }

            let result = session
                .upload_mission(vehicle, &plan, DEFAULT_TIMEOUT)
                .await
                .map_err(|error| format!("cannot upload the plan: {error}"))?;

            println!("the plan of {} items was accepted: {result:?}", plan.len());
            Ok(())
        }

        MissionAction::SetCurrent { seq } => {
            session
                .set_current_item(vehicle, *seq, DEFAULT_TIMEOUT)
                .await
                .map_err(|error| format!("cannot set the current item: {error}"))?;

            println!("the current item is now {seq}");
            Ok(())
        }

        MissionAction::Clear { kind } => {
            let mission_type = parse_kind(kind)?;

            session
                .clear_mission(vehicle, mission_type, DEFAULT_TIMEOUT)
                .await
                .map_err(|error| format!("cannot empty the plan: {error}"))?;

            println!("the plan was emptied");
            Ok(())
        }

        MissionAction::Jump { seq } => {
            session
                .jump_to_item(vehicle, *seq, DEFAULT_TIMEOUT)
                .await
                .map_err(|error| format!("cannot jump to the item: {error}"))?;

            println!("jumped to item {seq}");
            Ok(())
        }
    }
}

/// Parses the kind of plan.
fn parse_kind(text: &str) -> Result<MissionType, String> {
    match text.trim().to_ascii_lowercase().as_str() {
        "mission" | "plan" => Ok(MissionType::Mission),
        "fence" => Ok(MissionType::Fence),
        "rally" | "rally_points" => Ok(MissionType::Rally),
        other => Err(format!(
            "unknown kind of plan {other:?}; use mission, fence or rally"
        )),
    }
}

/// Reads a plan from a file, choosing the format by its extension.
fn read_plan(text: &str, path: &std::path::Path) -> Result<Plan, String> {
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        "kml" => mpcraft_mission::kml::read_kml(text).map_err(|error| error.to_string()),
        _ => mpcraft_mission::qgc::read_waypoints(text).map_err(|error| error.to_string()),
    }
}

/// Prints a plan, one item per line.
fn print_plan(plan: &Plan) {
    println!(
        "the plan holds {} items and is a {:?} plan",
        plan.len(),
        plan.mission_type
    );

    for (index, item) in plan.items.iter().enumerate() {
        let command = mpcraft_mission::mavcmd::command_name(item.command);

        if item.is_location_command() {
            println!(
                "{index:>3}  {command:<28} {:>11.7}, {:>11.7} {:>8.1} m",
                item.x, item.y, item.z
            );
        } else {
            println!(
                "{index:>3}  {command:<28} params {:>8.3}, {:>8.3}, {:>8.3}, {:>8.3}",
                item.params[0], item.params[1], item.params[2], item.params[3]
            );
        }
    }
}

/// Prints the live telemetry until the link ends.
async fn monitor_session(session: &mpcraft_link::Session) -> Result<(), String> {
    println!("monitoring; press Ctrl-C to stop");

    let mut last_summary = String::new();

    while session.state().connected {
        let state = session.state();
        let summary = summarise(&state);

        // Print when something changed, so the output stays readable but nothing is missed.
        if summary != last_summary {
            println!("{summary}");
            last_summary = summary;
        }

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    // The link ended. The final state is printed whether or not it changed, because the
    // session may have finished before the first poll.
    let state = session.state();
    let summary = summarise(&state);
    if summary != last_summary {
        println!("{summary}");
    }

    // A link that ended without a vehicle is almost always a configuration problem. Say so,
    // and show the board's own output if there was any.
    if state.vehicles.is_empty() {
        eprintln!("no vehicle answered on this link");

        if state.looks_like_can_bus {
            eprintln!(
                "this looks like a DroneCAN port; use the DroneCAN screen rather than the \
                 serial link"
            );
        }

        if !state.plaintext.is_empty() {
            eprintln!("the board printed:");
            for line in &state.plaintext {
                eprintln!("  {line}");
            }
        } else {
            eprintln!("no board output was captured either; check the baud rate and the port");
        }

        return Err("no vehicle answered".to_owned());
    }

    println!("the link ended");
    Ok(())
}

/// Summarises the session state in a single line.
fn summarise(state: &mpcraft_link::SessionState) -> String {
    let telemetry = &state.telemetry;

    let vehicle = state.vehicles.first().map_or_else(
        || "no vehicle".to_owned(),
        |v| {
            let kind = v
                .kind
                .map_or_else(String::new, |k| format!(" ({})", k.as_str()));
            format!("{}{kind}", v.id)
        },
    );

    let position = telemetry.position.map_or_else(
        || "no position".to_owned(),
        |p| format!("{:.7}, {:.7} {:+.1} m", p.lat, p.lng, p.alt),
    );

    let attitude = telemetry.attitude.map_or_else(
        || "no attitude".to_owned(),
        |a| format!("roll {:.1} pitch {:.1} yaw {:.1}", a.roll, a.pitch, a.yaw),
    );

    let mode = telemetry.mode.as_ref().map_or_else(
        || "unknown".to_owned(),
        |m| {
            m.name
                .clone()
                .unwrap_or_else(|| format!("mode {}", m.custom_mode))
        },
    );

    let armed = match telemetry.armed {
        Some(true) => "armed",
        Some(false) => "disarmed",
        None => "arm state unknown",
    };

    format!(
        "{vehicle} | {position} | {attitude} | {mode} | {armed} | lost {} | received {}",
        state.loss.packets_lost, state.loss.packets_received
    )
}
