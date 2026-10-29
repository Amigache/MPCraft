//! The link copied to another program, over real sockets.
//!
//! A mirror is asserted against a socket rather than against a mock: the bytes the station reads
//! from the vehicle appear on the other program, and, when the row says write, what the other
//! program sends reaches the vehicle. The link is a UDP session to a scripted vehicle, which is
//! the same shape the protocol tests use.

use std::sync::Arc;
use std::time::Duration;

use mpcraft_app::mirrors::MirrorService;
use mpcraft_link::{Session, SessionOptions};
use mpcraft_settings::{MirrorDirection, MirrorProtocol, MirrorSpec};
use mpcraft_transport::ConnectionSpec;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

/// How long the test waits for something to cross a socket.
const WAIT: Duration = Duration::from_secs(5);

/// A row that connects to a TCP listener.
fn tcp_outbound(port: u16, write: bool) -> MirrorSpec {
    MirrorSpec {
        protocol: MirrorProtocol::Tcp,
        direction: MirrorDirection::Outbound,
        port: port.to_string(),
        extra: "127.0.0.1".to_owned(),
        write,
    }
}

/// The session and the scripted vehicle it talks to.
async fn link() -> (Arc<Session>, UdpSocket, std::net::SocketAddr) {
    let vehicle = UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("a socket for the vehicle");
    let address = vehicle.local_addr().expect("the address of the vehicle");
    let spec = ConnectionSpec::parse(&format!("udp:{address}")).expect("a target");

    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .expect("the link is open");

    (Arc::new(session), vehicle, address)
}

#[tokio::test]
async fn the_bytes_of_the_link_reach_the_other_program() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a listener for the mirror");
    let port = listener
        .local_addr()
        .expect("the port of the mirror")
        .port();

    let (session, vehicle, _address) = link().await;

    let mirrors = MirrorService::default();
    mirrors.set_specs(vec![tcp_outbound(port, false)]);

    let key = tcp_outbound(port, false).key();

    mirrors
        .start(Arc::clone(&session), &tcp_outbound(port, false))
        .await
        .expect("the mirror starts");

    let (mut program, _) = tokio::time::timeout(WAIT, listener.accept())
        .await
        .expect("the mirror connects")
        .expect("the program accepts");

    // The link announced itself to the vehicle; the address that datagram came from is the one
    // the vehicle answers to, the same way a board learns where the station is.
    let mut buffer = [0_u8; 2048];
    let (_, station) = tokio::time::timeout(WAIT, vehicle.recv_from(&mut buffer))
        .await
        .expect("the station announces itself")
        .expect("a datagram arrives");

    let payload = b"a frame of the flight";
    vehicle
        .send_to(payload, station)
        .await
        .expect("the vehicle speaks");

    let mut received = Vec::new();

    tokio::time::timeout(WAIT, async {
        loop {
            let mut chunk = [0_u8; 512];
            let count = program.read(&mut chunk).await.expect("the mirror reads");

            if count == 0 {
                break;
            }

            received.extend_from_slice(&chunk[..count]);

            if received
                .windows(payload.len())
                .any(|window| window == payload)
            {
                break;
            }
        }
    })
    .await
    .expect("the bytes cross the mirror");

    assert!(
        received
            .windows(payload.len())
            .any(|window| window == payload),
        "the bytes the vehicle sent are on the other program"
    );

    let status = mirrors.status();

    assert!(status[0].running);
    assert!(status[0].sent >= payload.len() as u64);

    mirrors.stop(&key);
}

#[tokio::test]
async fn a_row_that_writes_puts_the_other_program_on_the_link() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a listener for the mirror");
    let port = listener
        .local_addr()
        .expect("the port of the mirror")
        .port();

    let (session, vehicle, _address) = link().await;

    let mirrors = MirrorService::default();
    let spec = tcp_outbound(port, true);
    mirrors.set_specs(vec![spec.clone()]);

    mirrors
        .start(Arc::clone(&session), &spec)
        .await
        .expect("the mirror starts");

    let (mut program, _) = tokio::time::timeout(WAIT, listener.accept())
        .await
        .expect("the mirror connects")
        .expect("the program accepts");

    let payload = b"a frame from the other station";
    program
        .write_all(payload)
        .await
        .expect("the program writes");

    let mut received = Vec::new();

    tokio::time::timeout(WAIT, async {
        loop {
            let mut buffer = [0_u8; 2048];
            let (count, _) = vehicle
                .recv_from(&mut buffer)
                .await
                .expect("the vehicle reads");

            received.extend_from_slice(&buffer[..count]);

            if received
                .windows(payload.len())
                .any(|window| window == payload)
            {
                break;
            }
        }
    })
    .await
    .expect("the bytes reach the vehicle");

    assert!(
        received
            .windows(payload.len())
            .any(|window| window == payload),
        "what the other program wrote is written to the link"
    );
}

#[tokio::test]
async fn a_row_that_waits_for_the_other_program_copies_to_it() {
    // A free port is taken by binding and dropping a listener; the mirror binds it again.
    let temporary = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a place to find a free port");
    let port = temporary.local_addr().expect("the port").port();
    drop(temporary);

    let (session, vehicle, _address) = link().await;

    let spec = MirrorSpec {
        protocol: MirrorProtocol::Tcp,
        direction: MirrorDirection::Inbound,
        port: port.to_string(),
        extra: String::new(),
        write: false,
    };

    let mirrors = MirrorService::default();
    mirrors.set_specs(vec![spec.clone()]);

    mirrors
        .start(Arc::clone(&session), &spec)
        .await
        .expect("the mirror starts");

    let mut program = tokio::time::timeout(WAIT, TcpStream::connect(("127.0.0.1", port)))
        .await
        .expect("the port is open")
        .expect("the program connects");

    let mut buffer = [0_u8; 2048];
    let (_, station) = tokio::time::timeout(WAIT, vehicle.recv_from(&mut buffer))
        .await
        .expect("the station announces itself")
        .expect("a datagram arrives");

    let payload = b"a frame for the program that waits";
    vehicle
        .send_to(payload, station)
        .await
        .expect("the vehicle speaks");

    let mut received = Vec::new();

    tokio::time::timeout(WAIT, async {
        loop {
            let mut chunk = [0_u8; 512];
            let count = program.read(&mut chunk).await.expect("the program reads");

            if count == 0 {
                break;
            }

            received.extend_from_slice(&chunk[..count]);

            if received
                .windows(payload.len())
                .any(|window| window == payload)
            {
                break;
            }
        }
    })
    .await
    .expect("the bytes cross the mirror");

    assert!(
        received
            .windows(payload.len())
            .any(|window| window == payload),
        "the program that connected receives the stream"
    );
}

#[tokio::test]
async fn a_row_that_cannot_be_opened_is_refused_before_anything_runs() {
    let mirrors = MirrorService::default();
    let (session, _vehicle, _address) = link().await;

    let bad = MirrorSpec {
        protocol: MirrorProtocol::Tcp,
        direction: MirrorDirection::Outbound,
        port: "not a port".to_owned(),
        extra: "127.0.0.1".to_owned(),
        write: false,
    };

    let error = mirrors
        .start(Arc::clone(&session), &bad)
        .await
        .expect_err("a port that is not a number is refused");

    assert_eq!(error.code(), "invalid_request");
}
