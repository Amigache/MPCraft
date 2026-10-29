//! The platforms where GATT is not carried.

use std::time::Duration;

use mpcraft_transport::Transport;

use crate::{BleDevice, BleError};

/// Opens a Bluetooth LE device.
///
/// # Errors
///
/// Always returns [`BleError::Unsupported`]: the GATT path is carried on Windows, which is where
/// the service was specified.
pub async fn open(_address: &str) -> Result<Box<dyn Transport>, BleError> {
    Err(BleError::Unsupported)
}

/// Scans for Bluetooth LE devices.
///
/// # Errors
///
/// Always returns [`BleError::Unsupported`].
pub async fn scan(_timeout: Duration) -> Result<Vec<BleDevice>, BleError> {
    Err(BleError::Unsupported)
}
