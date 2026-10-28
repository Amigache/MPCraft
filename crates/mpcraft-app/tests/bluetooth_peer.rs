//! The Bluetooth channel as the session reads it, answered by a scripted peer.
//!
//! The rules themselves live in `crates/mpcraft-transport/src/bluetooth.rs`, and their tests carry the literals
//! those rules were written against — the pair Microsoft's own reader reads out of `DEVICEMAP`, the instance shape
//! `BthPS3` publishes, the full instance path BlueGauge addresses, the Serial Port profile zephyr names. What this
//! file asserts is the half only the application sees: that a channel built around those rules answers the contract
//! a session depends on, without a port being opened.
//!
//! Four things have to hold for a link built on a COM port to behave like every other link:
//!
//! * a clean end of the stream comes back as `Ok(0)` rather than as an error, which is how a reader loop learns
//!   that the peer went away;
//! * a peer that accepts part of a buffer does not truncate the frame, so the write goes through the provided
//!   method that loops;
//! * the reading that says which device the port belongs to survives a reconnect, because the row is what the
//!   operator was told about the port and a retry cannot make it a different fact;
//! * a refusal at the rules falls before anything travels, so a row that names nothing never puts bytes on a cable.
//!
//! What cannot be asserted here is said rather than assumed: there is no peripheral on this run, and nothing is
//! opened on COM40 — that port belongs to a module that may be doing something with it, so the live area
//! (`tests/bluetooth_live.rs`) reads only what the machine writes in its registry and its map. The codes these rules
//! carry cross the boundary as codes, which `crates/mpcraft-app/src/error.rs` asserts.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use mpcraft_transport::{
    BluetoothError, BluetoothTransport, EnumRow, MapRow, Transport, TransportKind,
};

/// A peer that answers once and then reports the stream closed, and takes three bytes at a time. The counters are
/// kept behind shared atoms because the peer is moved into the transport: what says a loop ran, or that nothing
/// travelled before a refusal, is read afterwards.
struct Peer {
    given: Vec<u8>,
    accepted: Arc<AtomicUsize>,
    travelled: Arc<AtomicBool>,
}

impl Peer {
    fn new() -> Self {
        Self {
            given: vec![0xFD, 51, 0, 1, 2, 3],
            accepted: Arc::new(AtomicUsize::new(0)),
            travelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl Transport for Peer {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.travelled.store(true, Ordering::Relaxed);

        if self.given.is_empty() {
            return Ok(0);
        }

        let taken = buf.len().min(self.given.len());
        buf[..taken].copy_from_slice(&self.given[..taken]);
        self.given.drain(..taken);
        Ok(taken)
    }

    async fn write_some(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.travelled.store(true, Ordering::Relaxed);
        let taken = buf.len().min(3);
        self.accepted.fetch_add(taken, Ordering::Relaxed);
        Ok(taken)
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
        "a peer that answers once and then reports the stream closed".to_owned()
    }
}

#[tokio::test]
async fn a_channel_answers_the_contract_a_session_reads() {
    // The row this machine writes for its RFCOMM device, read as the map writes it, and the device its own instance
    // part names. Both are literals written down before anything was called, so what follows is the reading rather
    // than a value the test chose.
    let row =
        MapRow::read("\\Device\\BthModem0", "COM40").expect("the pair is the shape the map writes");
    let named = EnumRow::read(concat!(
        "BTHENUM\\{00001101-0000-1000-8000-00805f9b34fb}_LOCALMFG&08E7",
        "\\B&1DB17D3F&0&0424051406B2_C00000000",
    ))
    .expect("the row is the shape a published reader addresses");

    assert!(
        row.belongs_to_bluetooth(),
        "the prefix the service writes is what makes this row Bluetooth"
    );
    assert!(
        named.is_serial_row(),
        "and the profile says it carries a stream"
    );

    let peer = Peer::new();
    let accepted = Arc::clone(&peer.accepted);
    let travelled = Arc::clone(&peer.travelled);
    let mut channel = BluetoothTransport::wrap(
        Box::new(peer),
        Some(row.clone()),
        named.device.clone(),
        9600,
    );

    assert_eq!(channel.kind(), TransportKind::Bluetooth);
    assert!(
        !travelled.load(Ordering::Relaxed),
        "nothing travels before anything is asked"
    );

    // The first read answers with the frame and the next one says the peer closed — a zero, not an error, which is
    // the rule every transport carries.
    let mut buf = [0_u8; 16];
    let got = channel.read(&mut buf).await.expect("the peer answers");
    assert_eq!(got, 6);
    assert_eq!(&buf[..got], &[0xFD, 51, 0, 1, 2, 3]);
    assert_eq!(
        channel
            .read(&mut buf)
            .await
            .expect("the end is not an error"),
        0,
        "a clean end of the stream is how a session learns the link went away"
    );

    // A peer that takes three bytes at a time cannot truncate a frame: the write loops over the partial accepts, so
    // the whole buffer arrives.
    channel
        .write(&[1, 2, 3, 4, 5, 6, 7])
        .await
        .expect("the write loops");
    assert_eq!(
        accepted.load(Ordering::Relaxed),
        7,
        "three at a time, so the loop ran three times"
    );

    // A reconnect does not change what was read about the port. The row and the device it names are what the
    // operator was told, and the rate stays said as the number the driver was given rather than as something the
    // vehicle talks at.
    channel.reconnect().await.expect("the peer restores");
    assert_eq!(channel.row(), Some(&row));
    assert_eq!(channel.device(), Some("0424051406B2"));
    assert!(channel
        .describe()
        .contains("the rate 9600 is what the driver was given"));
}

#[tokio::test]
async fn a_refusal_falls_before_anything_travels_on_the_cable() {
    // The row this machine answers with COM41 carries `LOCALMFG&0000` and a group of twelve zeros, so Windows
    // presents a row and nothing behind it was decided. That is read as its own code, beside the code for a row a
    // reader cannot read at all, and neither reading reaches the cable.
    let peer = Peer::new();
    let travelled = Arc::clone(&peer.travelled);
    let mut channel = BluetoothTransport::wrap(Box::new(peer), None, None, 9600);

    let unnamed = EnumRow::read(concat!(
        "BTHENUM\\{00001101-0000-1000-8000-00805f9b34fb}_LOCALMFG&0000",
        "\\B&1DB17D3F&0&000000000000_00000003",
    ))
    .expect_err("the group that would name a device says nothing");
    assert_eq!(unnamed.code(), "bluetooth_no_device_named");

    // Nothing was said about a device, so the channel says that as the fact it is rather than naming one.
    assert!(channel.describe().contains("no row read"));

    let shape =
        MapRow::read("\\Device\\LSIModem5", "MODEM5").expect_err("the value is not a COM number");
    assert_eq!(shape.code(), "bluetooth_row_not_the_shape");

    // A virtual pair named with the word is not an RFCOMM channel — SuperCollider's looser comparison matches any
    // port name, and that is what this rule refuses.
    let virtual_pair = MapRow::read("\\Device\\com0comBluetooth0", "COM18").unwrap();
    assert!(!virtual_pair.belongs_to_bluetooth());

    assert!(
        !travelled.load(Ordering::Relaxed),
        "both refusals fell before anything was asked of the cable"
    );

    // Once something is asked, it travels, which is what makes the line above a fact about the order rather than
    // about a peer that could not answer.
    let mut buf = [0_u8; 8];
    assert_eq!(channel.read(&mut buf).await.expect("the peer answers"), 6);
    assert!(travelled.load(Ordering::Relaxed));
}

/// The two codes are kept apart because they say two different things to the operator: put a device there, or the
/// row was not written as the machine writes rows. `crates/mpcraft-app/src/error.rs` asserts that they cross the
/// boundary as codes; what is asserted here is that the two readings stay two facts.
#[test]
fn the_two_readings_stay_two_facts() {
    let unnamed = BluetoothError::NoDeviceNamed {
        row: "BTHENUM\\{00001101-0000-1000-8000-00805f9b34fb}_LOCALMFG&0000".to_owned(),
        reason: "the group that would name a device says 000000000000".to_owned(),
    };
    let unreadable = BluetoothError::NotTheShape {
        row: "\\Device\\LSIModem5 = MODEM5".to_owned(),
        reason: "the value is not a COM number".to_owned(),
    };

    assert_ne!(unnamed.code(), unreadable.code());
}
