//! What a link does when the board stops answering.
//!
//! A port stays open when the vehicle goes away: the operating system gave it to the board, and
//! a board that was restarted — or one that left MAVLink for its bootloader — leaves the port
//! exactly where it was. Without a rule for silence the reader waits for a vehicle that is no
//! longer speaking, and the station goes on saying it is connected.

use std::time::Duration;

use async_trait::async_trait;
use mpcraft_link::{Session, SessionOptions, SessionState};
use mpcraft_mavlink::dialect::messages::HEARTBEAT_DATA;
use mpcraft_mavlink::frame::{encode_v2, MavHeader};
use mpcraft_mavlink::{MavAutopilot, MavMessage, MavType};
use mpcraft_transport::{Transport, TransportKind};

/// A transport that delivers one frame and then says nothing, with the port still open.
struct Quiet {
    frame: Option<Vec<u8>>,
}

impl Quiet {
    fn with(frame: Vec<u8>) -> Self {
        Self { frame: Some(frame) }
    }

    fn silent() -> Self {
        Self { frame: None }
    }
}

#[async_trait]
impl Transport for Quiet {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Some(frame) = self.frame.take() {
            buf[..frame.len()].copy_from_slice(&frame);
            return Ok(frame.len());
        }

        // The board is still there — the port is open — and it has nothing to say. A read that
        // never answers is what a serial port does when the vehicle behind it is gone; the
        // session bounds it with its own poll interval, which is the wait this exercises.
        std::future::pending::<()>().await;
        unreachable!("the session bounds every read by its poll interval")
    }

    async fn write_some(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Serial
    }

    fn is_open(&self) -> bool {
        true
    }

    async fn reconnect(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn describe(&self) -> String {
        "a quiet board".to_owned()
    }
}

fn heartbeat() -> Vec<u8> {
    let message = MavMessage::HEARTBEAT(HEARTBEAT_DATA {
        mavtype: MavType::MAV_TYPE_QUADROTOR,
        autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
        ..Default::default()
    });

    encode_v2(
        &message,
        MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: 0,
        },
    )
}

fn options() -> SessionOptions {
    SessionOptions {
        poll_interval: Duration::from_millis(10),
        silence_timeout: Duration::from_millis(150),
        ..SessionOptions::default()
    }
}

async fn wait_for<F>(session: &Session, timeout: Duration, condition: F) -> bool
where
    F: Fn(&SessionState) -> bool,
{
    let deadline = std::time::Instant::now() + timeout;

    while std::time::Instant::now() < deadline {
        if condition(&session.state()) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    condition(&session.state())
}

#[tokio::test]
async fn a_link_that_hears_the_vehicle_and_then_goes_quiet_is_closed() {
    let session = Session::with_transport(Box::new(Quiet::with(heartbeat())), options());

    assert!(
        wait_for(&session, Duration::from_secs(1), |state| !state
            .vehicles
            .is_empty())
        .await,
        "the heartbeat was never read"
    );

    assert!(
        wait_for(&session, Duration::from_secs(2), |state| !state.connected).await,
        "the link stayed open after the vehicle went quiet"
    );
}

#[tokio::test]
async fn a_station_that_has_heard_nothing_is_not_closed_for_silence() {
    // A port opened to listen is quiet because the vehicle has not arrived, not because it left.
    // Closing it would make a station that waits for a board impossible.
    let session = Session::with_transport(Box::new(Quiet::silent()), options());

    tokio::time::sleep(Duration::from_millis(600)).await;

    assert!(
        session.state().connected,
        "a station waiting for a vehicle was closed by the silence rule"
    );
}
