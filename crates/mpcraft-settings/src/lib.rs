//! Persisted configuration for MPCraft.
//!
//! Settings are stored in TOML under the platform configuration directory. The format is
//! human-editable on purpose: a user whose GCS will not start can fix the file by hand.
//!
//! Two rules follow from that, and they shape the whole crate:
//!
//! * A file that does not exist is **not an error**. It means nothing has been configured
//!   yet, which is the state of a fresh installation.
//! * A file that cannot be parsed **is an error**, and it is never quietly replaced by an
//!   empty one. A user who mistyped a target has to be told which file is wrong and what
//!   is wrong with it — not shown an empty list, with their profiles quietly gone.
//!
//! # The format
//!
//! ```toml
//! [[connection_profiles]]
//! name = "SITL"
//! target = "udp:127.0.0.1:14550"
//! auto_connect = true
//!
//! [[connection_profiles]]
//! name = "Flight controller"
//! target = "serial:COM3:57600"
//!
//! [[plugins]]
//! name = "alarm"
//! enabled = true
//!
//! [[swarm_slots]]
//! vehicle = "2:1"
//! y = 60.0
//! ```
//!
//! A key the application does not know is an error rather than something to ignore: the
//! file is short, and a misspelled `auto_connect` would otherwise change what the
//! application does without anybody noticing.
//!
//! # Where the file lives
//!
//! | Platform | Directory |
//! | --- | --- |
//! | Windows | `%APPDATA%\MPCraft` |
//! | Linux | `$XDG_CONFIG_HOME/MPCraft`, or `~/.config/MPCraft` |
//! | macOS | `~/Library/Application Support/MPCraft` |
//!
//! The directory is found from the environment, so that no crate has to know every
//! platform's conventions. A machine with neither a configuration directory nor a home
//! directory has no settings — the application carries on without them.
//!
//! # Writing
//!
//! A write is atomic: the file is written under a temporary name in the same directory and
//! then renamed over the target. An interrupted write leaves either the old file or the
//! new one, never half of either.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
mod keys;
mod mirrors;
mod plugins;
mod poi;
mod profiles;
mod store;
mod swarm;

pub use error::SettingsError;
pub use keys::SigningKeyEntry;
pub use mirrors::{MirrorDirection, MirrorProtocol, MirrorSpec};
pub use plugins::PluginEntry;
pub use poi::Poi;
pub use profiles::ConnectionProfile;
pub use store::{platform_config_dir, Settings, SettingsStore};
pub use swarm::SwarmSlot;
