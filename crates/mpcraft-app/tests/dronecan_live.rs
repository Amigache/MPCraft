//! The DroneCAN node, read and written over a link that forwards the bus.
//!
//! The node is `scripts/dronecan-node.py`: a MAVLink vehicle on a UDP port with one DroneCAN node
//! behind a forwarded bus. It is written from the DSDL and the transport specification rather than
//! from this crate, so an agreement here is between two implementations.
//!
//! ```text
//! pip install pymavlink
//! python scripts/dronecan-node.py --port 14650 --node 42
//! MPCRAFT_DRONECAN_LIVE=1 cargo test -p mpcraft-app --test dronecan_live -- --nocapture
//! ```
//!
//! With `MPCRAFT_DRONECAN_WRITE=1` the area also writes a parameter, saves and restarts, which are
//! the calls that change a node rather than read it.

use std::time::Duration;

use mpcraft_app::dronecan::{value_of, DronecanService};
use mpcraft_dronecan::ParameterValue;
use mpcraft_link::{Session, SessionOptions};
use mpcraft_transport::ConnectionSpec;

const BUS: u8 = 1;
const NODE: u8 = 42;

#[tokio::test]
async fn a_node_behind_a_forwarded_bus_answers() {
    if std::env::var("MPCRAFT_DRONECAN_LIVE").ok().as_deref() != Some("1") {
        println!(
            "MPCRAFT_DRONECAN_LIVE is not 1, so no node is reached; the framing and the messages are in the unit tests"
        );

        return;
    }

    let target = std::env::var("MPCRAFT_DRONECAN_TARGET")
        .unwrap_or_else(|_| "udp:127.0.0.1:14650".to_owned());

    let spec = ConnectionSpec::parse(&target).expect("the target is one the station reads");
    let session = std::sync::Arc::new(
        Session::connect(&spec, SessionOptions::default())
            .await
            .expect("the link opens"),
    );

    let vehicle = session
        .wait_for_any_vehicle(Duration::from_secs(15))
        .await
        .expect("the vehicle answers");

    println!("the vehicle answered as {vehicle:?}");

    let service = DronecanService::new();

    let info = service
        .node_info(&session, vehicle, BUS, NODE)
        .await
        .expect("the node says what it is");

    println!(
        "the node {} says {:?}, software {}:{}, hardware {}:{}",
        info.name, info.status, info.software_major, info.software_minor,
        info.hardware_major, info.hardware_minor
    );

    assert_eq!(info.name, "com.example.bench");
    assert_eq!(info.status.health, 0);
    assert_eq!(info.software_major, 1);
    assert_eq!(info.software_minor, 2);
    assert_eq!(info.hardware_major, 4);
    assert_eq!(info.hardware_minor, 5);

    let parameters = service
        .parameters(&session, vehicle, BUS, NODE)
        .await
        .expect("the node lists what it holds");

    println!("{} parameters:", parameters.len());

    for parameter in &parameters {
        println!(
            "  {} = {} (default {}, {}..{})",
            parameter.name, parameter.value, parameter.default_value, parameter.min_value,
            parameter.max_value
        );
    }

    assert_eq!(parameters.len(), 3);

    let gain = parameters
        .iter()
        .find(|parameter| parameter.name == "BENCH_GAIN")
        .expect("the gain is there");

    assert_eq!(gain.value, "15");

    if std::env::var("MPCRAFT_DRONECAN_WRITE").ok().as_deref() != Some("1") {
        println!("MPCRAFT_DRONECAN_WRITE is not 1, so the node was read and not changed");

        return;
    }

    let written = service
        .write_parameter(
            &session,
            vehicle,
            BUS,
            NODE,
            "BENCH_GAIN",
            value_of("21"),
        )
        .await
        .expect("the node takes the value");

    println!("the node stored {} = {}", written.name, written.value);

    // What the node stores is what the sheet shows, which for a node that clamps is not always what
    // was written.
    assert_eq!(written.name, "BENCH_GAIN");
    assert_eq!(written.value, "21");

    service
        .save_parameters(&session, vehicle, BUS, NODE)
        .await
        .expect("the node keeps what it holds");

    service
        .restart_node(&session, vehicle, BUS, NODE)
        .await
        .expect("the node answers the restart");

    // A name the node does not hold is refused rather than stored.
    let refused = service
        .write_parameter(
            &session,
            vehicle,
            BUS,
            NODE,
            "NOT_A_PARAMETER",
            ParameterValue::Integer(1),
        )
        .await;

    assert!(refused.is_err(), "a name the node has not is not written");

    println!("the node answered every call");
}
