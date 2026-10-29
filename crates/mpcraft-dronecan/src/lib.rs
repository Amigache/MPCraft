//! DroneCAN as a station speaks it: frames on a CAN bus, and the messages a ground station asks of a
//! node.
//!
//! A vehicle with a CAN bus can be asked to forward every frame on it over MAVLink
//! (`MAV_CMD_CAN_FORWARD`), and the frames then arrive as `CAN_FRAME` messages. That is the path
//! Mission Planner takes (`ConfigDroneCAN.cs`), and it is the path this crate is the wire for: it
//! knows nothing about MAVLink, nothing about a serial port and nothing about the interface.
//!
//! Two checksums hold it together, and they answer different questions: the CRC-16 of one transfer,
//! which catches a frame that was lost or changed, and the CRC-64 of a data type, which decides
//! whether two nodes may exchange that type at all. A payload of up to seven bytes travels in one
//! frame and carries neither, because the bus already checks the frame; a longer one is split, and
//! the first frame carries the transfer checksum seeded with the type's signature.
//!
//! What is carried today is the transport, written from the specification and from libcanard's
//! reading of the identifier, and checked against the values both publish. The messages themselves —
//! who is on the bus, what it is, and the parameters it holds — are the next layer, and they are
//! where the two signatures of a station's own asks will be named.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod bits;
pub mod can;
pub mod crc;
pub mod messages;

pub use can::NOMINAL_PRIORITY;
pub use can::{encode, signature_bytes, CanId, Frame, Kind, Reassembler, Received, Tail};
pub use messages::{
    DataType, ExecuteOpcode, GetSetRequest, GetSetResponse, NodeInfo, NodeStatus, ParameterValue,
    RestartNode,
};
