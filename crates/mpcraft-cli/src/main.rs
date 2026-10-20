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

    let mut transport = mpcraft_transport::open(&spec)
        .await
        .map_err(|e| format!("cannot open the link: {e}"))?;

    println!("link open: {}", transport.describe());

    if !monitor {
        return Ok(());
    }

    // The telemetry aggregation is wired up in phase 0, task 4. Until then the monitor
    // reports the raw traffic so the link itself can be verified.
    println!("monitoring (the telemetry decoder is not wired up yet)");

    let mut buffer = vec![0u8; 4096];
    loop {
        match transport.read(&mut buffer).await {
            Ok(0) => {
                println!("the link ended");
                return Ok(());
            }
            Ok(len) => println!("{} bytes received", len),
            Err(error) => {
                println!("the link failed: {error}");
                return Err(error.to_string());
            }
        }
    }
}
