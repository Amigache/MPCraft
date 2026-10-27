#![forbid(unsafe_code)]

//! The plugin layer: what a plugin says about itself, what it is allowed to do, and the runtime that
//! holds it.
//!
//! Upstream runs plugins as C# assemblies inside the same process as everything else, which means a
//! plugin can reach the socket, the disk, and the settings, and a plugin that hangs takes the window
//! with it. MPCraft keeps the lifecycle — read, link open, tick, close — and changes where the code
//! runs: a plugin is a WebAssembly module inside a sandbox with a list of what it may ask for and a
//! ceiling on what it may use.
//!
//! The pieces are:
//!
//! - [`manifest`]: the file a person can read and edit, which names the module and the capabilities;
//! - [`capability`]: the things the sandbox can grant, and nothing else;
//! - [`api`]: the surface a plugin is plugged into, which the application implements;
//! - [`lifecycle`]: the four stages, in order;
//! - [`sandbox`]: the runtime, the limits, and the call.
//!
//! A plugin cannot be given authority over the application. It is given names, and a capability it
//! was not given is a function its module cannot import, so it fails at load rather than at some
//! call somebody hoped would not happen.

pub mod api;
pub mod capability;
pub mod error;
pub mod lifecycle;
pub mod manifest;
pub mod sandbox;

pub use api::{CommandIntent, HostApi, ParamIntent, TelemetryField};
pub use capability::Capability;
pub use error::PluginError;
pub use lifecycle::Stage;
pub use manifest::PluginManifest;
pub use sandbox::{Limits, Plugin, Sandbox};
