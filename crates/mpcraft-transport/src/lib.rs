//! Byte transports for MPCraft.
//!
//! A transport moves bytes and knows nothing about MAVLink. The protocol layer sits on top and
//! is responsible for framing.
//!
//! # Contract
//!
//! Implementations must:
//!
//! * never block the async runtime,
//! * return `Ok(0)` on a clean end-of-stream rather than an error,
//! * accept a partial write and let the caller loop (see [`Transport::write`]),
//! * survive a reconnect where the medium allows it.
//!
//! # What this crate deliberately does not know
//!
//! A transport does not parse MAVLink, does not know what a heartbeat is, and does not
//! interpret the contents of a file it replays. The log-format-aware replay lives in
//! `mpcraft-log`, which implements the same trait.
//!
//! The device layer ([`device`]) is the counterpart of a transport for a medium that answers
//! requests rather than carrying a stream: there is nothing to read until something is asked. It knows
//! the shape of a request and the shape of the functional descriptor, and nothing about states, blocks
//! or checksums — those are rules of the protocol and live above.
//!
//! # Example
//!
//! ```
//! use mpcraft_transport::{ConnectionSpec, TransportKind};
//!
//! let spec = ConnectionSpec::parse("udp:127.0.0.1:14550").unwrap();
//! assert_eq!(spec.scheme, mpcraft_transport::ConnectionScheme::Udp);
//! assert_eq!(spec.port(), Some(14550));
//!
//! let _ = TransportKind::Udp;
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::fmt;

use async_trait::async_trait;

pub mod device;
pub mod file;
pub mod serial;
pub mod spec;
pub mod tcp;
pub mod udp;

pub use device::{
    read_string, Capability, Device, DeviceError, DeviceResult, FunctionalDescriptor,
};
pub use file::FileTransport;
pub use serial::{SerialPortInfo, SerialTransport};
pub use spec::{ConnectionOptions, ConnectionScheme, ConnectionSpec, SpecError};
pub use tcp::TcpTransport;
pub use udp::UdpTransport;

/// The kind of a transport, used for UI labelling and capability checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TransportKind {
    /// A serial port.
    Serial,
    /// UDP, in either client or server mode.
    Udp,
    /// TCP, in either client or server mode.
    Tcp,
    /// A file replaying recorded traffic.
    File,
    /// An NTRIP correction source.
    Ntrip,
}

impl fmt::Display for TransportKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Serial => "serial",
            Self::Udp => "udp",
            Self::Tcp => "tcp",
            Self::File => "file",
            Self::Ntrip => "ntrip",
        };
        f.write_str(name)
    }
}

/// A bidirectional byte stream.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Reads up to `buf.len()` bytes.
    ///
    /// Returns `Ok(0)` when the peer closed the stream cleanly. A zero-length read is not an
    /// error: it is how a session learns that the link ended.
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>;

    /// Writes *some* of `buf`, returning how many bytes were accepted.
    ///
    /// A transport is allowed to accept fewer bytes than were offered. Callers use
    /// [`Transport::write`] to write a whole buffer.
    async fn write_some(&mut self, buf: &[u8]) -> std::io::Result<usize>;

    /// The kind of this transport.
    fn kind(&self) -> TransportKind;

    /// Whether the transport is currently usable.
    fn is_open(&self) -> bool;

    /// Attempts to restore the connection after a failure.
    ///
    /// The retry policy lives in the supervisor, not here: a transport restores the medium,
    /// and the application decides whether restoring it is the right thing to do.
    async fn reconnect(&mut self) -> std::io::Result<()>;

    /// A human-readable description, shown in the UI.
    fn describe(&self) -> String;

    /// Writes the whole buffer, looping over partial writes.
    ///
    /// A transport that writes only part of the buffer and returns `Ok` would silently
    /// truncate the frame. Every write path goes through this method.
    async fn write(&mut self, buf: &[u8]) -> std::io::Result<()> {
        let mut written = 0;

        while written < buf.len() {
            let n = self.write_some(&buf[written..]).await?;
            if n == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "the transport accepted no bytes",
                ));
            }
            written += n;
        }

        Ok(())
    }
}

/// Opens the transport a specification describes.
///
/// # Errors
///
/// Returns an error when the specification is invalid for the platform, or when the transport
/// cannot be opened.
pub async fn open(spec: &ConnectionSpec) -> std::io::Result<Box<dyn Transport>> {
    match spec.scheme {
        ConnectionScheme::Serial => {
            let port = spec.port_name().unwrap_or_default();
            let transport =
                SerialTransport::open(port, spec.options.baud_rate, spec.options.dtr_on_open)?;
            Ok(Box::new(transport))
        }
        ConnectionScheme::Udp => {
            let port = spec.port().unwrap_or_default();
            let host = spec.host().unwrap_or_default();

            if spec.options.server {
                let local = resolve(host, port)?;
                Ok(Box::new(UdpTransport::listen(local).await?))
            } else {
                let remote = resolve(host, port)?;
                Ok(Box::new(UdpTransport::connect(remote).await?))
            }
        }
        ConnectionScheme::Tcp => {
            let port = spec.port().unwrap_or_default();
            let host = spec.host().unwrap_or_default();

            if spec.options.server {
                let local = resolve(host, port)?;
                Ok(Box::new(TcpTransport::listen(local).await?))
            } else {
                let remote = resolve(host, port)?;
                Ok(Box::new(TcpTransport::connect(remote).await?))
            }
        }
        ConnectionScheme::File => Ok(Box::new(
            FileTransport::open(&spec.target, spec.options.replay_rate).await?,
        )),
    }
}

/// Resolves a host and port into a socket address.
///
/// An empty host means "every interface", which is what a listening transport binds.
///
/// # Errors
///
/// Returns an error when the host cannot be resolved.
fn resolve(host: &str, port: u16) -> std::io::Result<std::net::SocketAddr> {
    use std::net::{IpAddr, ToSocketAddrs};

    let host = if host.is_empty() { "0.0.0.0" } else { host };

    // A literal address is used as given. Resolving it through the resolver would replace
    // the requested port with the one the resolver happened to pick.
    if let Ok(address) = host.parse::<IpAddr>() {
        return Ok(std::net::SocketAddr::new(address, port));
    }

    (host, port).to_socket_addrs()?.next().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("cannot resolve {host}:{port}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A writer that accepts at most `chunk` bytes per call, proving the write loop.
    struct ChunkedWriter {
        chunk: usize,
        buffer: Vec<u8>,
    }

    #[async_trait]
    impl Transport for ChunkedWriter {
        async fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Ok(0)
        }

        async fn write_some(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let n = buf.len().min(self.chunk);
            self.buffer.extend_from_slice(&buf[..n]);
            Ok(n)
        }

        fn kind(&self) -> TransportKind {
            TransportKind::File
        }

        fn is_open(&self) -> bool {
            true
        }

        async fn reconnect(&mut self) -> std::io::Result<()> {
            Ok(())
        }

        fn describe(&self) -> String {
            "chunked".to_owned()
        }
    }

    #[tokio::test]
    async fn write_loops_over_partial_writes() {
        let mut writer = ChunkedWriter {
            chunk: 3,
            buffer: Vec::new(),
        };
        writer.write(b"0123456789").await.unwrap();
        assert_eq!(writer.buffer, b"0123456789");
    }

    #[tokio::test]
    async fn a_transport_that_accepts_nothing_is_an_error_not_a_hang() {
        struct Stalled;

        #[async_trait]
        impl Transport for Stalled {
            async fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
                Ok(0)
            }

            async fn write_some(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Ok(0)
            }

            fn kind(&self) -> TransportKind {
                TransportKind::File
            }

            fn is_open(&self) -> bool {
                true
            }

            async fn reconnect(&mut self) -> std::io::Result<()> {
                Ok(())
            }

            fn describe(&self) -> String {
                "stalled".to_owned()
            }
        }

        let mut stalled = Stalled;
        let error = stalled.write(b"data").await.unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::WriteZero);
    }

    #[test]
    fn transport_kind_displays_a_stable_label() {
        assert_eq!(TransportKind::Serial.to_string(), "serial");
        assert_eq!(TransportKind::Udp.to_string(), "udp");
        assert_eq!(TransportKind::Tcp.to_string(), "tcp");
        assert_eq!(TransportKind::File.to_string(), "file");
    }

    #[test]
    fn an_empty_host_binds_every_interface() {
        let address = resolve("", 14550).unwrap();
        assert_eq!(address.port(), 14550);
        assert!(address.ip().is_unspecified());
    }
}
