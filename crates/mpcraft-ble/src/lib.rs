//! A Bluetooth LE device reached through its GATT serial service.
//!
//! A Bluetooth LE module that carries a serial link is not a COM port: the bytes travel as writes
//! to one characteristic and notifications from another. Mission Planner reads the Nordic UART
//! service (`6e400001-...`), and the HM-10 family — the Makeblock module is one — uses `FFE1` for
//! both directions. This crate speaks both, so a module of either kind can carry a link.
//!
//! The GATT stack of the machine is reached through `btleplug` on Windows, which is the platform the
//! service was specified for; on another platform the transport says that it is not carried rather
//! than pretending to be. The trait and the bytes stay in `mpcraft-transport`, and the application
//! opens the device because the platform API is asynchronous in a way a byte carrier does not need
//! to know.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The Nordic UART service, which is what Mission Planner reads.
pub const NUS_SERVICE: &str = "6e400001-b5a3-f393-e0a9-e50e24dcca9e";

/// The characteristic the station writes to on the Nordic UART service.
pub const NUS_WRITE: &str = "6e400002-b5a3-f393-e0a9-e50e24dcca9e";

/// The characteristic the module notifies on the Nordic UART service.
pub const NUS_NOTIFY: &str = "6e400003-b5a3-f393-e0a9-e50e24dcca9e";

/// The HM-10 service and its serial characteristic.
pub const HM10_SERVICE: &str = "0000ffe0-0000-1000-8000-00805f9b34fb";

/// The HM-10 characteristic, which is written and notified on the same handle.
pub const HM10_CHARACTERISTIC: &str = "0000ffe1-0000-1000-8000-00805f9b34fb";

/// How long a scan is given before it is read.
pub const SCAN_TIME: Duration = Duration::from_secs(4);

/// A Bluetooth LE device that carries a serial service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BleDevice {
    /// The address of the device, as the platform writes it.
    pub address: String,
    /// The name the device advertises, when it says one.
    pub name: String,
    /// The serial profile it carries: `nus`, `ffe1`, or `unknown`.
    pub profile: String,
}

/// Why a Bluetooth LE device could not be reached.
#[derive(Debug, thiserror::Error)]
pub enum BleError {
    /// The platform has no Bluetooth LE stack the crate can use.
    #[error("Bluetooth LE is not carried on this platform")]
    Unsupported,
    /// No adapter was found.
    #[error("this machine has no Bluetooth LE adapter")]
    NoAdapter,
    /// The device was not seen in the scan.
    #[error("the device {0} was not found")]
    NotFound(String),
    /// The device does not carry a serial service this crate reads.
    #[error("the device {0} does not carry a serial service")]
    NoProfile(String),
    /// The platform refused a step of the GATT exchange.
    #[error("Bluetooth LE: {0}")]
    Platform(String),
}

impl From<BleError> for std::io::Error {
    fn from(error: BleError) -> Self {
        let kind = match error {
            BleError::NotFound(_) => std::io::ErrorKind::NotFound,
            BleError::Unsupported => std::io::ErrorKind::Unsupported,
            BleError::NoAdapter | BleError::NoProfile(_) | BleError::Platform(_) => {
                std::io::ErrorKind::Other
            }
        };

        Self::new(kind, error.to_string())
    }
}

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{open, scan};

#[cfg(not(windows))]
mod stub;

#[cfg(not(windows))]
pub use stub::{open, scan};
