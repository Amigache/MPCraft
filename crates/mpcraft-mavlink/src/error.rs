//! Errors produced by the MAVLink protocol layer.

use thiserror::Error;

/// Result alias for protocol operations.
pub type Result<T> = std::result::Result<T, ProtocolError>;

/// A failure in the MAVLink wire protocol.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProtocolError {
    /// The frame carried a CRC that did not match its payload.
    #[error("CRC mismatch: expected 0x{expected:04x}, computed 0x{actual:04x}")]
    CrcMismatch {
        /// CRC value carried in the frame.
        expected: u16,
        /// CRC value computed over the frame contents.
        actual: u16,
    },

    /// The frame was structurally invalid.
    #[error("malformed frame: {0}")]
    Malformed(&'static str),

    /// The frame used a protocol version this crate does not implement.
    #[error("unsupported MAVLink version: {0}")]
    UnsupportedVersion(u8),

    /// A signed frame failed signature verification.
    #[error("signature verification failed")]
    SignatureInvalid,

    /// The frame's payload length exceeded the protocol maximum.
    #[error("payload length {0} exceeds the maximum of {1}")]
    PayloadTooLarge(usize, usize),

    /// The message id is not present in the compiled dialect.
    #[error("unknown message id {0}")]
    UnknownMessage(u32),

    /// The frame was signed but no key was configured for the peer.
    #[error("frame is signed but no verification key is configured")]
    MissingSigningKey,
}
