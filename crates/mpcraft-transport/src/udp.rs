//! UDP transport.
//!
//! # Fan-out
//!
//! Mission Planner's `UdpSerial` keeps a list of remote endpoints and writes every frame to
//! **all** of them. A new endpoint joins the list as soon as a datagram arrives from it.
//!
//! That behaviour is what lets two ground stations watch the same vehicle over a UDP link:
//! the vehicle sends to one address, and the station relays to the other. MPCraft keeps the
//! same semantics.
//!
//! # Server mode
//!
//! In server mode the transport binds a port and waits for the first datagram, learning the
//! peer's address from it. This is the mode a vehicle uses when it streams telemetry to a
//! ground station that is listening.

use std::net::SocketAddr;
use std::time::Duration;

use async_trait::async_trait;
use tokio::net::UdpSocket;

use crate::{Transport, TransportKind};

/// A UDP transport.
pub struct UdpTransport {
    socket: UdpSocket,
    /// The peers this transport writes to.
    peers: Vec<SocketAddr>,
    /// The address the transport was opened against, for reconnection.
    remote: Option<SocketAddr>,
    /// The local address to bind when reconnecting in client mode.
    local: SocketAddr,
    /// Whether the transport waits for the peer instead of connecting out.
    server: bool,
}

impl UdpTransport {
    /// Opens a UDP transport.
    ///
    /// In client mode the socket binds to `local` and remembers `remote` as the peer to
    /// write to. In server mode the socket binds to `local` and waits for the first datagram
    /// to discover the peer.
    ///
    /// # Errors
    ///
    /// Returns an error when the socket cannot be bound.
    pub async fn open(
        local: SocketAddr,
        remote: Option<SocketAddr>,
        server: bool,
    ) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(local).await?;

        // In client mode the remote peer is known from the start, so a frame can be sent
        // before the peer has said anything. In server mode the peer is learned from the
        // first datagram that arrives.
        let peers = match (server, remote) {
            (false, Some(remote)) => vec![remote],
            _ => Vec::new(),
        };

        Ok(Self {
            socket,
            peers,
            remote,
            local,
            server,
        })
    }

    /// Opens a client-mode transport against a remote address.
    ///
    /// # Errors
    ///
    /// Returns an error when the socket cannot be bound.
    pub async fn connect(remote: SocketAddr) -> std::io::Result<Self> {
        Self::open(SocketAddr::from(([0, 0, 0, 0], 0)), Some(remote), false).await
    }

    /// Opens a server-mode transport bound to a local address.
    ///
    /// # Errors
    ///
    /// Returns an error when the socket cannot be bound.
    pub async fn listen(local: SocketAddr) -> std::io::Result<Self> {
        Self::open(local, None, true).await
    }

    /// The peers this transport is currently writing to.
    #[must_use]
    pub fn peers(&self) -> &[SocketAddr] {
        &self.peers
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
        self.socket.local_addr()
    }

    /// Adds a peer to the fan-out list.
    pub fn add_peer(&mut self, peer: SocketAddr) {
        if !self.peers.contains(&peer) {
            self.peers.push(peer);
        }
    }
}

#[async_trait]
impl Transport for UdpTransport {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let (len, from) = self.socket.recv_from(buf).await?;

        // A datagram from an unknown peer joins the fan-out list, which is how a second
        // ground station is picked up automatically.
        if !self.peers.contains(&from) {
            self.peers.push(from);
        }

        Ok(len)
    }

    async fn write_some(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // Write to every known peer. A peer that has gone away must not stop the others
        // from receiving, so individual failures are tolerated.
        if self.peers.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "no UDP peer is known yet",
            ));
        }

        let mut written = 0usize;

        for peer in self.peers.clone() {
            match self.socket.send_to(buf, peer).await {
                Ok(sent) => written = written.max(sent),
                Err(_) => continue,
            }
        }

        Ok(written)
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Udp
    }

    fn is_open(&self) -> bool {
        true
    }

    async fn reconnect(&mut self) -> std::io::Result<()> {
        // Rebinding the socket is the only thing a UDP transport can "reconnect" to: the
        // peers are addressed by name, not by connection.
        let socket = UdpSocket::bind(self.local).await?;
        self.socket = socket;
        self.peers.clear();
        Ok(())
    }

    fn describe(&self) -> String {
        if self.server {
            format!("udp server on {}", self.local)
        } else {
            match self.remote {
                Some(remote) => format!("udp client to {remote}"),
                None => format!("udp client on {}", self.local),
            }
        }
    }
}

impl std::fmt::Debug for UdpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UdpTransport")
            .field("local", &self.local)
            .field("remote", &self.remote)
            .field("server", &self.server)
            .field("peers", &self.peers)
            .finish()
    }
}

/// The delay before a reconnect attempt, growing with each failure.
#[must_use]
pub fn reconnect_delay(attempt: u32) -> Duration {
    // Exponential backoff, capped at five seconds — the same interval Mission Planner uses
    // between automatic TCP reconnect attempts.
    let seconds = 1u64 << attempt.min(3);
    Duration::from_secs(seconds.min(5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn datagrams_round_trip_between_two_sockets() {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server.local_addr().unwrap();

        let mut transport = UdpTransport::connect(server_addr).await.unwrap();

        // The client writes to the server...
        transport.write(b"hello").await.unwrap();

        let mut buf = [0u8; 32];
        let (len, from) = server.recv_from(&mut buf).await.unwrap();
        assert_eq!(&buf[..len], b"hello");

        // ...and the server replies.
        server.send_to(b"world", from).await.unwrap();

        let len = transport.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..len], b"world");
    }

    #[tokio::test]
    async fn a_new_sender_joins_the_fan_out_list() {
        // The transport binds port zero and reports the address the system chose, so the
        // test never collides with another test's port.
        let mut transport = UdpTransport::listen("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let server_addr = transport.local_addr().unwrap();

        assert!(
            transport.peers().is_empty(),
            "a server starts with no peers"
        );

        // A datagram from an unknown peer is discovered.
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        sender.send_to(b"ping", server_addr).await.unwrap();

        let mut buf = [0u8; 32];
        transport.read(&mut buf).await.unwrap();

        assert_eq!(transport.peers().len(), 1);
        assert_eq!(transport.peers()[0], sender.local_addr().unwrap());
    }

    #[tokio::test]
    async fn writing_without_a_peer_is_an_error_rather_than_a_panic() {
        let mut transport = UdpTransport::listen("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();

        let error = transport.write(b"x").await.unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotConnected);
    }

    #[tokio::test]
    async fn the_backoff_grows_and_is_capped() {
        assert_eq!(reconnect_delay(0), Duration::from_secs(1));
        assert_eq!(reconnect_delay(1), Duration::from_secs(2));
        assert_eq!(reconnect_delay(2), Duration::from_secs(4));
        assert_eq!(reconnect_delay(3), Duration::from_secs(5));
        assert_eq!(reconnect_delay(10), Duration::from_secs(5));
    }
}
