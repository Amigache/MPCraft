//! The camera search, read against an advertisement that is really on the network.
//!
//! The parsing is tested with values written down before it is called; what a scripted value cannot
//! show is that the query leaves the process and an answer comes back over the wire. The advertiser
//! here is `scripts/advertise-camera.py`, written with the `zeroconf` library rather than with the
//! crate this reads, so the agreement is between two implementations and not one crate with itself.
//!
//! With `MPCRAFT_MDNS_STREAM=1` the area goes one step further: it hands the address it discovered
//! to the relay and reads the picture back, so what is asserted is that the address a camera
//! advertised is an address that plays, not only that it was written down correctly.
//!
//! ```text
//! pip install zeroconf
//! python scripts/advertise-camera.py --seconds 400 --port 8554 --path /live --codecs "H264(640x480)"
//! MPCRAFT_MDNS_LIVE=1 MPCRAFT_MDNS_STREAM=1 \
//!   cargo test -p mpcraft-app --test cameras_live -- --nocapture
//! ```
//!
//! Discovery is link-local: it is answered on the network this machine is attached to, and nothing
//! forwards it. A machine with no network, or with multicast blocked by the access point, answers
//! nothing — which this area reports as the fact it is rather than as a failure of the parsing.

use std::time::Duration;

use mpcraft_app::cameras::discover;
use mpcraft_app::video::VideoRelayService;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

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
    let path =
        std::env::var("MPCRAFT_MDNS_PATH").unwrap_or_else(|_| "/axis-media/media.amp".to_owned());

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
    assert_eq!(camera.path, path);
    assert!(
        !camera.streams.is_empty(),
        "the camera advertised no stream at all"
    );
    assert!(
        camera
            .streams
            .iter()
            .all(|stream| stream.url.starts_with("rtsp://") && stream.url.contains(&path)),
        "every address carries the path the camera advertised"
    );

    // The address is what the relay is given, so the last thing to read is whether the picture
    // arrives from it. This is skipped unless the machine asks for it, because it needs a stream
    // behind the advertisement and not only an advertisement.
    if std::env::var("MPCRAFT_MDNS_STREAM").ok().as_deref() != Some("1") {
        println!("MPCRAFT_MDNS_STREAM is not 1, so the address was read and not drawn");

        return;
    }

    let url = camera.streams[0].url.clone();
    let relay = VideoRelayService::default();

    println!("drawing {url} through the relay");

    let serving = relay.start(&url).await.expect("the relay starts");

    let mut client = tokio::time::timeout(
        Duration::from_secs(10),
        TcpStream::connect(("127.0.0.1", serving.port)),
    )
    .await
    .expect("the relay connects")
    .expect("the relay listens");

    client
        .write_all(b"GET /stream.mjpeg HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .await
        .expect("the request is written");

    let mut received = Vec::new();

    tokio::time::timeout(Duration::from_secs(30), async {
        let mut buffer = vec![0_u8; 8192];

        while received.len() < 32 * 1024 {
            let count = client.read(&mut buffer).await.expect("the relay answers");

            if count == 0 {
                break;
            }

            received.extend_from_slice(&buffer[..count]);

            if received.windows(2).any(|pair| pair == [0xFF, 0xD8]) {
                break;
            }
        }
    })
    .await
    .expect("the picture arrives");

    let head = String::from_utf8_lossy(&received[..received.len().min(512)]);

    assert!(
        head.contains("multipart/x-mixed-replace"),
        "the loopback answers with the content type a picture element reads"
    );
    assert!(
        received.windows(2).any(|pair| pair == [0xFF, 0xD8]),
        "the body carries a JPEG"
    );

    println!("the picture arrived: {} bytes read", received.len());

    relay.stop(&url);
}
