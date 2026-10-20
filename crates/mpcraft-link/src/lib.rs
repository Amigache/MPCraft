//! The link layer for MPCraft.
//!
//! This crate turns a byte stream into vehicle state.
//!
//! # What lives here
//!
//! | Module | Responsibility |
//! | --- | --- |
//! | [`session`] | The session: the reader task that owns the transport, and the actor model around it |
//! | [`heartbeat`] | Identifying the vehicle on a link that carries several heartbeat sources |
//! | [`vehicle`] | The vehicle registry and the vehicle family detection |
//! | [`telemetry`] | Decoding messages into the telemetry snapshot |
//! | [`loss`] | Packet-loss accounting that distinguishes reordering from loss |
//! | [`plaintext`] | Capturing the board's own output, which explains a silent link |
//!
//! # The concurrency model
//!
//! Mission Planner has **two** consumers competing for the same socket: a background
//! `SerialReader` loop and every blocking request/response transaction. They are arbitrated
//! by a cooperative boolean flag (`giveComport`) and a semaphore. A transaction that forgets
//! the flag deadlocks the entire link.
//!
//! MPCraft replaces this with an actor model:
//!
//! * **One reader task per session** owns the byte stream. It parses frames, updates the
//!   vehicle state, and serves queued writes.
//! * **Transactions** register a matcher and await a dedicated `oneshot` responder. They
//!   never rely on channel buffering, which is lossy under load.
//! * **Telemetry aggregation** happens in the reader task, and the result is published
//!   through an atomic swap.
//!
//! Concurrent transactions are therefore safe by construction, and a slow transaction cannot
//! starve telemetry.
//!
//! # Example
//!
//! ```
//! use mpcraft_link::{Session, SessionOptions};
//! use mpcraft_transport::ConnectionSpec;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let spec = ConnectionSpec::parse("udp:127.0.0.1:14550")?;
//! let session = Session::connect(&spec, SessionOptions::default()).await?;
//!
//! // The state is published by the reader task.
//! let state = session.state();
//! assert!(state.connected);
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod heartbeat;
pub mod loss;
pub mod params;
pub mod plaintext;
pub mod session;
pub mod telemetry;
pub mod vehicle;

pub use error::LinkError;
pub use heartbeat::{Heartbeat, HeartbeatCollector, HeartbeatRejection};
pub use loss::{LossAccountant, LossStats};
pub use params::{ParamError, DEFAULT_LIST_TIMEOUT, DEFAULT_READ_TIMEOUT, DEFAULT_WRITE_TIMEOUT};
pub use plaintext::PlaintextSniffer;
pub use session::{ReceivedMessage, Session, SessionOptions, SessionState};
pub use telemetry::TelemetryAggregator;
pub use vehicle::{Vehicle, VehicleId, VehicleKind, VehicleRegistry};
