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
//! * loop until the whole buffer is written, or report the error,
//! * survive a partial write.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::fmt;

use async_trait::async_trait;

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
    /// Returns `Ok(0)` when the peer closed the stream cleanly.
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>;

    /// Writes the whole buffer.
    ///
    /// Implementations must loop until every byte is written, or return the error.
    async fn write(&mut self, buf: &[u8]) -> std::io::Result<()>;

    /// The kind of this transport.
    fn kind(&self) -> TransportKind;

    /// Attempts to restore the connection after a failure.
    async fn reconnect(&mut self) -> std::io::Result<()>;

    /// A human-readable description, shown in the UI.
    fn describe(&self) -> String;
}

/// Writes a whole buffer, looping over partial writes.
///
/// A transport that writes only part of the buffer and returns `Ok` would silently truncate
/// the message. Every write path goes through this helper.
pub async fn write_all<W>(mut writer: W, buf: &[u8]) -> std::io::Result<()>
where
    W: AsyncWrite,
{
    let mut written = 0;
    while written < buf.len() {
        let n = writer.write(&buf[written..]).await?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "transport accepted no bytes",
            ));
        }
        written += n;
    }
    Ok(())
}

/// The subset of `tokio::io::AsyncWrite` this crate needs.
///
/// Defined locally so the trait can be implemented by test doubles without pulling in the whole
/// `tokio` I/O surface.
#[async_trait]
pub trait AsyncWrite: Send {
    /// Writes some bytes, returning how many were accepted.
    async fn write(&mut self, buf: &[u8]) -> std::io::Result<usize>;
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
    impl AsyncWrite for ChunkedWriter {
        async fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let n = buf.len().min(self.chunk);
            self.buffer.extend_from_slice(&buf[..n]);
            Ok(n)
        }
    }

    #[async_trait]
    impl AsyncWrite for &mut ChunkedWriter {
        async fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let n = buf.len().min(self.chunk);
            self.buffer.extend_from_slice(&buf[..n]);
            Ok(n)
        }
    }

    #[tokio::test]
    async fn write_all_loops_over_partial_writes() {
        let mut writer = ChunkedWriter {
            chunk: 3,
            buffer: Vec::new(),
        };
        write_all(&mut writer, b"0123456789").await.unwrap();
        assert_eq!(writer.buffer, b"0123456789");
    }

    #[test]
    fn transport_kind_displays_a_stable_label() {
        assert_eq!(TransportKind::Serial.to_string(), "serial");
        assert_eq!(TransportKind::Udp.to_string(), "udp");
    }
}
