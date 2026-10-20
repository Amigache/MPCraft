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
//! mpcraft connect udp:127.0.0.1:14550 --monitor
//! mpcraft connect udp:127.0.0.1:14550 -- params list
//! mpcraft connect udp:127.0.0.1:14550 -- params get ARMING_CHECK
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::process::ExitCode;

use clap::{Parser, Subcommand};

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
    /// `mpcraft --target udp:127.0.0.1:14550 params list`.
    #[arg(short, long, global = true)]
    target: Option<String>,

    /// The action to perform.
    #[command(subcommand)]
    command: Command,
}

/// The available actions.
#[derive(Debug, Subcommand)]
enum Command {
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

    let target = match cli.command_target() {
        Some(target) => target,
        None => {
            eprintln!("error: no connection target given");
            eprintln!("usage: mpcraft --target udp:127.0.0.1:14550 <command>");
            return ExitCode::FAILURE;
        }
    };

    match run(&target, &cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

impl Cli {
    /// The connection target, whether given globally or per command.
    fn command_target(&self) -> Option<String> {
        match &self.command {
            Command::Connect { target, .. } => target.clone().or_else(|| self.target.clone()),
            _ => self.target.clone(),
        }
    }
}

/// Runs the requested action.
fn run(target: &str, command: &Command) -> Result<(), String> {
    // The link layer is wired up in phase 0, task 3. Until then the CLI reports the target
    // it would connect to, which keeps the argument parsing and exit codes exercised.
    match command {
        Command::Connect { monitor, .. } => {
            println!("connecting to {target}");
            if *monitor {
                println!("monitoring telemetry (not yet implemented)");
            }
        }
        Command::Params { action } => {
            println!("params {action:?} on {target}");
        }
        Command::Mission { action } => {
            println!("mission {action:?} on {target}");
        }
    }
    Ok(())
}
