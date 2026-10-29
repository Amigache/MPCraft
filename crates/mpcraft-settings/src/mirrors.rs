//! The ways the link can be mirrored to another program.

use serde::{Deserialize, Serialize};

/// How a mirror reaches the other program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MirrorProtocol {
    /// A serial port of this computer.
    Serial,
    /// A TCP connection.
    Tcp,
    /// A UDP socket.
    Udp,
}

impl MirrorProtocol {
    /// The name the file writes and the interface reads.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Serial => "serial",
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }

    /// Reads the name the file writes.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "serial" => Some(Self::Serial),
            "tcp" => Some(Self::Tcp),
            "udp" => Some(Self::Udp),
            _ => None,
        }
    }
}

/// Which side opens the connection.
///
/// An outbound mirror reaches the other program, and an inbound one waits for it. A serial
/// port has no such choice: the port is opened here, and the other side is whatever is on the
/// cable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MirrorDirection {
    /// This station connects to the other program.
    Outbound,
    /// This station waits for the other program.
    Inbound,
}

impl MirrorDirection {
    /// The name the file writes and the interface reads.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Outbound => "outbound",
            Self::Inbound => "inbound",
        }
    }

    /// Reads the name the file writes.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "outbound" => Some(Self::Outbound),
            "inbound" => Some(Self::Inbound),
            _ => None,
        }
    }
}

/// One way the link is copied to another program.
///
/// Every packet the station reads from the vehicle is written to the mirror, which is what lets
/// another program follow the flight without a link of its own. When `write` is set, what the
/// mirror receives is written back to the vehicle as well, which is how a second station on the
/// same cable can talk to the board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorSpec {
    /// How the mirror reaches the other program.
    pub protocol: MirrorProtocol,
    /// Which side opens the connection.
    pub direction: MirrorDirection,
    /// The port: the name of a serial port, or the number a TCP or UDP socket uses.
    pub port: String,
    /// The rest of the address: the baud rate of a serial port, or the host of an outbound TCP
    /// or UDP mirror. An inbound TCP or UDP mirror listens on every interface, so it says
    /// nothing here.
    pub extra: String,
    /// Whether what the mirror receives is written back to the vehicle.
    #[serde(default)]
    pub write: bool,
}

impl MirrorSpec {
    /// The name a row is known by.
    ///
    /// The same name the row was started with is the one that stops it, so the name is built
    /// from the settings and not from the place the row happens to sit in the list. The `write`
    /// switch is not part of it: changing it does not make it another mirror.
    #[must_use]
    pub fn key(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.protocol.as_str(),
            self.direction.as_str(),
            self.port.trim(),
            self.extra.trim()
        )
    }
}
