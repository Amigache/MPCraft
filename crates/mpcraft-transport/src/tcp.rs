//! TCP transport.
//!
//! # Reconnection
//!
//! Mission Planner reconnects a dropped TCP link automatically, at most once every five
//! seconds. MPCraft keeps the capability but separates the two concerns:
//!
//! * the transport implements [`Transport::reconnect`], which re-establishes the socket;
//! * the supervisor in `mpcraft-app` decides *whether* to reconnect, how many times to try,
//!   and whether to surface the failure to the user.
//!
//! Splitting them keeps the retry policy out of the protocol layer, where it would be
//! impossible to test without a real network peer.

use std::net::SocketAddr;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::{Transport, TransportKind};

/// A TCP transport.
pub struct TcpTransport {
    stream: Option<TcpStream>,
    listener: Option<TcpListener>,
    address: SocketAddr,
    server: bool,
}

impl TcpTransport {
    /// Connects to a remote address.
    ///
    /// # Errors
    ///
    /// Returns an error when the connection cannot be established.
    pub async fn connect(address: SocketAddr) -> std::io::Result<Self> {
        let stream = TcpStream::connect(address).await?;
        stream.set_nodelay(true)?;

        Ok(Self {
            stream: Some(stream),
            listener: None,
            address,
            server: false,
        })
    }

    /// Listens on a local address, accepting the first connection.
    ///
    /// The connection is established lazily, on the first read or write, so that a vehicle
    /// connecting to a listening station does not have to race the listener's startup.
    ///
    /// # Errors
    ///
    /// Returns an error when the socket cannot be bound.
    pub async fn listen(address: SocketAddr) -> std::io::Result<Self> {
        let listener = TcpListener::bind(address).await?;

        Ok(Self {
            stream: None,
            listener: Some(listener),
            address,
            server: true,
        })
    }

    /// Accepts a pending connection, if this transport is listening.
    ///
    /// # Errors
    ///
    /// Returns an error when the connection cannot be accepted.
    pub async fn accept(&mut self) -> std::io::Result<SocketAddr> {
        let Some(listener) = self.listener.as_mut() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "this transport is not listening",
            ));
        };

        let (stream, peer) = listener.accept().await?;
        stream.set_nodelay(true)?;
        self.stream = Some(stream);

        Ok(peer)
    }

    /// The address this transport was opened against.
    #[must_use]
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// The address the socket is actually bound to.
    ///
    /// When the transport was opened on port zero this is the port the operating system
    /// chose, which is what a test or a second peer needs to know.
    ///
    /// # Errors
    ///
    /// Returns an error when the socket's address cannot be read.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        match self.listener.as_ref() {
            Some(listener) => listener.local_addr(),
            None => self
                .stream
                .as_ref()
                .map_or(Ok(self.address), |stream| stream.local_addr()),
        }
    }
}

#[async_trait]
impl Transport for TcpTransport {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // A listening transport accepts its peer before the first read.
        if self.stream.is_none() && self.listener.is_some() {
            self.accept().await?;
        }

        let Some(stream) = self.stream.as_mut() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "the TCP connection is closed",
            ));
        };

        let read = stream.read(buf).await?;

        // A read of zero bytes means the peer closed the connection cleanly.
        if read == 0 {
            self.stream = None;
        }

        Ok(read)
    }

    async fn write_some(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.stream.is_none() && self.listener.is_some() {
            self.accept().await?;
        }

        let Some(stream) = self.stream.as_mut() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "the TCP connection is closed",
            ));
        };

        stream.write(buf).await
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Tcp
    }

    fn is_open(&self) -> bool {
        self.stream.is_some()
    }

    async fn reconnect(&mut self) -> std::io::Result<()> {
        if self.server {
            // A listening transport rebinds its socket. The listener is already bound, so
            // there is nothing to do beyond dropping a dead connection.
            self.stream = None;
            return Ok(());
        }

        let stream = TcpStream::connect(self.address).await?;
        stream.set_nodelay(true)?;
        self.stream = Some(stream);

        Ok(())
    }

    fn describe(&self) -> String {
        if self.server {
            format!("tcp server on {}", self.address)
        } else {
            format!("tcp client to {}", self.address)
        }
    }
}

impl std::fmt::Debug for TcpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TcpTransport")
            .field("address", &self.address)
            .field("server", &self.server)
            .field("connected", &self.stream.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bytes_round_trip_over_a_tcp_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut stream = stream;
            let mut buf = [0u8; 32];
            let len = stream.read(&mut buf).await.unwrap();
            stream.write_all(&buf[..len]).await.unwrap();
        });

        let mut client = TcpTransport::connect(address).await.unwrap();
        client.write(b"ping").await.unwrap();

        let mut buf = [0u8; 32];
        let len = client.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..len], b"ping");

        server.await.unwrap();
    }

    #[tokio::test]
    async fn a_closed_peer_is_reported_as_end_of_stream() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            drop(stream);
        });

        let mut client = TcpTransport::connect(address).await.unwrap();
        server.await.unwrap();

        let mut buf = [0u8; 32];
        let len = client.read(&mut buf).await.unwrap();

        // Zero bytes is a clean end of stream, not an error.
        assert_eq!(len, 0);
        assert!(!client.is_open());
    }

    #[tokio::test]
    async fn a_listening_transport_accepts_a_peer() {
        // The transport binds port zero and reports the address the system chose.
        let mut server = TcpTransport::listen("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let address = server.local_addr().unwrap();
        assert!(!server.is_open());

        let client = tokio::spawn(async move {
            let mut stream = TcpStream::connect(address).await.unwrap();
            stream.write_all(b"hi").await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        });

        let mut buf = [0u8; 32];
        let len = server.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..len], b"hi");

        client.await.unwrap();
    }

    #[tokio::test]
    async fn reconnecting_restores_a_dropped_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        let mut client = TcpTransport::connect(address).await.unwrap();
        assert!(client.is_open());

        client.reconnect().await.unwrap();
        assert!(client.is_open());
    }

    #[tokio::test]
    async fn accepting_on_a_client_transport_is_an_error() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        let mut transport = TcpTransport::connect(address).await.unwrap();

        let error = transport.accept().await.unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
}
