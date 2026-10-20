//! MAVLink wire protocol for MPCraft.
//!
//! This crate owns everything at the byte level of the MAVLink protocol:
//!
//! * **Framing** — a buffered, state-machine parser for MAVLink v1 and v2 that resynchronises
//!   after corruption instead of desynchronising the whole stream.
//! * **Validation** — CRC-16/MCRF4XX with the per-message CRC_EXTRA seed, and MAVLink 2
//!   signature verification.
//! * **Bindings** — re-exports of the generated `mavlink` dialect types.
//!
//! # Scope
//!
//! This crate knows about *frames*, not about *vehicles*. It does not know what a heartbeat
//! means or how a mission is transferred; that belongs to [`mpcraft-link`] and
//! [`mpcraft-mission`].
//!
//! # Example
//!
//! ```
//! use mpcraft_mavlink::frame::FrameParser;
//!
//! let mut parser = FrameParser::new();
//! let mut sink = Vec::new();
//!
//! // Feed a truncated buffer; the parser must not panic and must not emit a partial frame.
//! parser.feed(&[0xFD, 0x00], &mut sink).unwrap();
//! assert!(sink.is_empty());
//! ```
//!
//! [`mpcraft-link`]: ../mpcraft_link/index.html
//! [`mpcraft-mission`]: ../mpcraft_mission/index.html

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod dialect;
pub mod error;
pub mod frame;
pub mod inspector;
pub mod signing;

pub use dialect::{
    CharArray, MavAutopilot, MavCmd, MavFrame, MavGpsFixType, MavMessage, MavMissionResult,
    MavMissionType, MavModeFlag, MavParamType, MavProtocolCapability, MavResult, MavSeverity,
    MavState, MavSysStatusSensor, MavType, Message,
};
pub use error::{ProtocolError, Result};
pub use frame::{Frame, FrameParser, MavlinkVersion};
pub use inspector::{MessageStats, PacketInspector};
pub use signing::{SigningKey, SigningKeyring};

/// The MAVLink protocol version implemented by this crate.
pub const PROTOCOL_VERSION: &str = "2.0";
