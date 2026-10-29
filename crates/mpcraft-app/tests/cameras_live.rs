//! The camera search, read against an advertisement that is really on the network.
//!
//! The parsing is tested with values written down before it is called; what a scripted value cannot
//! show is that the query leaves the process and an answer comes back over the wire. The advertiser
//! here is `scripts/advertise-camera.py`, written with the `zeroconf` library rather than with the
//! crate this reads, so the agreement is between two implementations and not one crate with itself.
//!
//! ```text
//! pip install zeroconf
//! python scripts/advertise-camera.py --seconds 60
//! MPCRAFT_MDNS_LIVE=1 cargo test -p mpcraft-app --test cameras_live -- --nocapture
//! ```
//!
//! Discovery is link-local: it is answered on the network this machine is attached to, and nothing
//! forwards it. A machine with no network, or with multicast blocked by the access point, answers
//! nothing — which this area reports as the fact it is rather than as a failure of the parsing.

use std::time::Duration;

use mpcraft_app::cameras::discover;

#[tokio::test]
async fn a_camera_advertised_on_the_network_is_found() {
    if std::env::var("MPCRAFT_MDNS_LIVE").ok().as_deref() != Some("1") {
        println!(
            "MPCRAFT_MDNS_LIVE is not 1, so nothing is advertised; the reading of an advertisement is in the unit tests"
        );

        return;
    }

    let name =
        std::env::var("MPCRAFT_MDNS_NAME").unwrap_or_else(|_| "MPCraft Test Camera".to_owned());
    let port: u16 = std::env::var("MPCRAFT_MDNS_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(8554);

    let cameras = discover(Duration::from_secs(8))
        .await
        .expect("the local network is asked");

    println!("{} cameras answered", cameras.len());

    for camera in &cameras {
        println!(
            "  {} at {}:{} ({})",
            camera.name, camera.address, camera.port, camera.id
        );

        for stream in &camera.streams {
            println!("    {stream:?}");
        }
    }

    let found = cameras.iter().find(|camera| camera.name == name);

    assert!(
        found.is_some(),
        "the advertiser named {name} was not among the {0} cameras that answered",
        cameras.len()
    );

    let camera = found.expect("the camera is there");

    assert_eq!(camera.port, port);
    assert_eq!(camera.path, "/axis-media/media.amp");
    assert_eq!(
        camera.streams.len(),
        2,
        "the TXT record advertises two sizes"
    );
    assert!(camera
        .streams
        .iter()
        .all(|stream| stream.url.starts_with("rtsp://")));
    assert!(
        camera
            .streams
            .iter()
            .any(|stream| stream.width == Some(1920) && stream.height == Some(1080)),
        "the sizes are the ones the advertisement named"
    );
}
