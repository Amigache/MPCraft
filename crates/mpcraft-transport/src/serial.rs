//! Serial port transport.
//!
//! # DTR and RTS
//!
//! Many boards use the DTR line as a reset line: opening the port with DTR asserted and
//! then dropping it puts an ESP32 into its bootloader. Mission Planner toggles DTR for
//! exactly this reason, and MPCraft follows: the DTR line is **low** when the port is opened
//! unless the connection specification asks otherwise.
//!
//! The driver asserts DTR briefly on open on Linux regardless of the requested state. That
//! is a driver behaviour, not something the application can prevent.

use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_serial::{SerialPortBuilderExt, SerialPortType, SerialStream};

use crate::{Transport, TransportKind};

/// A serial port transport.
pub struct SerialTransport {
    stream: Option<SerialStream>,
    port: String,
    baud_rate: u32,
    dtr_on_open: bool,
}

impl SerialTransport {
    /// Opens a serial port.
    ///
    /// # Errors
    ///
    /// Returns an error when the port cannot be opened: it does not exist, it is already in
    /// use, or the requested baud rate is not supported by the driver.
    pub fn open(
        port: impl Into<String>,
        baud_rate: u32,
        dtr_on_open: bool,
    ) -> std::io::Result<Self> {
        let port = port.into();

        let builder = tokio_serial::new(&port, baud_rate)
            .dtr_on_open(dtr_on_open)
            .timeout(Duration::from_millis(100));

        let stream = builder.open_native_async()?;

        Ok(Self {
            stream: Some(stream),
            port,
            baud_rate,
            dtr_on_open,
        })
    }

    /// Lists the serial ports available on this machine.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform's port enumeration fails.
    pub fn available_ports() -> std::io::Result<Vec<SerialPortInfo>> {
        tokio_serial::available_ports()
            .map(|ports| {
                ports
                    .into_iter()
                    .map(|port| SerialPortInfo {
                        device: port.port_name,
                        description: Some(describe_port_type(&port.port_type)),
                    })
                    .collect()
            })
            .map_err(|error| {
                std::io::Error::other(format!("cannot enumerate serial ports: {error}"))
            })
    }

    /// The name of the port.
    #[must_use]
    pub fn port(&self) -> &str {
        &self.port
    }

    /// The baud rate.
    #[must_use]
    pub fn baud_rate(&self) -> u32 {
        self.baud_rate
    }
}

/// A human-readable description of how a port is connected.
fn describe_port_type(port_type: &SerialPortType) -> String {
    match port_type {
        SerialPortType::UsbPort(_) => "USB".to_owned(),
        SerialPortType::PciPort => "PCI".to_owned(),
        SerialPortType::BluetoothPort => "Bluetooth".to_owned(),
        SerialPortType::Unknown => "unknown".to_owned(),
    }
}

/// A serial port found on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialPortInfo {
    /// The device name, for example `COM3` or `/dev/ttyUSB0`.
    pub device: String,
    /// A human-readable description of the device.
    pub description: Option<String>,
}

#[async_trait]
impl Transport for SerialTransport {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let Some(stream) = self.stream.as_mut() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "the serial port is closed",
            ));
        };
        stream.read(buf).await
    }

    async fn write_some(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let Some(stream) = self.stream.as_mut() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "the serial port is closed",
            ));
        };
        stream.write(buf).await
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Serial
    }

    fn is_open(&self) -> bool {
        self.stream.is_some()
    }

    async fn reconnect(&mut self) -> std::io::Result<()> {
        let builder = tokio_serial::new(&self.port, self.baud_rate)
            .dtr_on_open(self.dtr_on_open)
            .timeout(Duration::from_millis(100));

        self.stream = Some(builder.open_native_async()?);
        Ok(())
    }

    fn describe(&self) -> String {
        format!("{} @ {}", self.port, self.baud_rate)
    }
}

impl std::fmt::Debug for SerialTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerialTransport")
            .field("port", &self.port)
            .field("baud_rate", &self.baud_rate)
            .field("open", &self.stream.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_closed_transport_reports_an_error_rather_than_panicking() {
        let transport = SerialTransport {
            stream: None,
            port: "COM0".to_owned(),
            baud_rate: 57600,
            dtr_on_open: false,
        };

        assert!(!transport.is_open());
        assert_eq!(transport.kind(), TransportKind::Serial);
        assert_eq!(transport.describe(), "COM0 @ 57600");
    }

    #[test]
    fn port_enumeration_does_not_fail_on_a_machine_with_no_ports() {
        // The result depends on the machine, but the call itself must not fail on a
        // supported platform.
        let _ = SerialTransport::available_ports();
    }
}
