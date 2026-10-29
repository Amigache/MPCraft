//! The vehicle search, read against an advertisement that is really on the network.
//!
//! The reading of an advertisement is tested with values written down before it is called; what a
//! scripted value cannot show is that the query leaves the process and an answer comes back over the
//! wire. The advertiser is `scripts/advertise-vehicle.py`, written with Python's `zeroconf` rather
//! than with the crate this reads, so the agreement is between two implementations.
//!
//! ```text
//! pip install zeroconf
//! python scripts/advertise-vehicle.py --seconds 120 --port 14550 --name SITL
//! MPCRAFT_MDNS_LIVE=1 cargo test -p mpcraft-app --test vehicles_live -- --nocapture
//! ```
//!
//! Discovery is link-local: it is answered on the network this machine is attached to, and nothing
//! forwards it.

use std::time::Duration;

use mpcraft_app::vehicles::discover;
use mpcraft_link::{Session, SessionOptions};
use mpcraft_transport::ConnectionSpec;

#[tokio::test]
async fn a_vehicle_advertised_on_the_network_is_found() {
    if std::env::var("MPCRAFT_MDNS_LIVE").ok().as_deref() != Some("1") {
        println!(
            "MPCRAFT_MDNS_LIVE is not 1, so nothing is advertised; the reading of an advertisement is in the unit tests"
        );

        return;
    }

    let name = std::env::var("MPCRAFT_MDNS_VEHICLE").unwrap_or_else(|_| "SITL".to_owned());
    let port: u16 = std::env::var("MPCRAFT_MDNS_VEHICLE_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(14550);

    let vehicles = discover(Duration::from_secs(8))
        .await
        .expect("the local network is asked");

    println!("{} vehicles answered", vehicles.len());

    for vehicle in &vehicles {
        println!(
            "  {} at {}:{} -> {}",
            vehicle.name, vehicle.address, vehicle.port, vehicle.target
        );
    }

    let found = vehicles.iter().find(|vehicle| vehicle.name == name);

    assert!(
        found.is_some(),
        "the advertiser named {name} was not among the {0} vehicles that answered",
        vehicles.len()
    );

    let vehicle = found.expect("the vehicle is there");

    assert_eq!(vehicle.port, port);
    assert!(
        vehicle.target.starts_with("udp:"),
        "the target opens the link the way the station already knows"
    );
    assert!(
        vehicle.target.ends_with(&format!(":{port}")),
        "the target carries the port that answered"
    );
    assert_eq!(vehicle.target, format!("udp:{}:{}", vehicle.address, port));

    // The target is what the station would open, so the last thing to read is whether a link on it
    // answers. This is skipped unless the machine asks for it, because it needs a vehicle behind the
    // advertisement and not only an advertisement.
    if std::env::var("MPCRAFT_MDNS_LINK").ok().as_deref() != Some("1") {
        println!("MPCRAFT_MDNS_LINK is not 1, so the target was read and not opened");

        return;
    }

    let spec = ConnectionSpec::parse(&vehicle.target).expect("the target is one the station reads");
    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .expect("the link opens on the discovered target");

    let heard = session
        .wait_for_any_vehicle(Duration::from_secs(15))
        .await
        .expect("the vehicle answers on the address it advertised");

    println!("the vehicle answered as {heard:?}");
}
