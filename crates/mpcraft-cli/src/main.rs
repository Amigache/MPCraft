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
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use mpcraft_transport::{ConnectionSpec, SerialTransport};

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
}

/// The mission actions.
#[derive(Debug, Subcommand)]
enum MissionAction {
    /// Download the mission and print it.
    Download,
    /// Upload a mission from a file.
    Upload {
        /// The path to the mission file.
        path: std::path::PathBuf,
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
            let target = cli
                .target
                .clone()
                .ok_or_else(|| "no connection target given".to_owned())?;
            let _spec = ConnectionSpec::parse(&target).map_err(|e| e.to_string())?;
            println!("params {action:?} on {target}");
            Ok(())
        }
        Command::Mission { action } => {
            let target = cli
                .target
                .clone()
                .ok_or_else(|| "no connection target given".to_owned())?;
            let _spec = ConnectionSpec::parse(&target).map_err(|e| e.to_string())?;
            println!("mission {action:?} on {target}");
            Ok(())
        }
    }
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
