//! A camera stream turned into a picture, over a real RTSP source.
//!
//! The source is named by `MPCRAFT_VIDEO_SOURCE`, and the test is skipped when it is not. It is a
//! server that really speaks RTSP (a container of `mediamtx` with a publisher of `ffmpeg` in front),
//! so what is asserted is the relay: the loopback server answers with the content type a picture
//! element reads, and the body begins with a JPEG.
//!
//! ```text
//! MPCRAFT_VIDEO_LIVE=1 MPCRAFT_VIDEO_SOURCE=rtsp://127.0.0.1:8554/live \
//! cargo test -p mpcraft-app --test video_relay -- --nocapture
//! ```

use std::time::Duration;

use mpcraft_app::video::VideoRelayService;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::test]
async fn a_camera_stream_is_drawn_as_a_picture() {
    if std::env::var("MPCRAFT_VIDEO_LIVE").ok().as_deref() != Some("1") {
        println!(
            "MPCRAFT_VIDEO_LIVE is not 1, so no camera is reached; the address and the refusal are in the unit tests"
        );
        return;
    }

    let Some(source) = std::env::var("MPCRAFT_VIDEO_SOURCE").ok() else {
        println!("MPCRAFT_VIDEO_SOURCE does not name a camera, so nothing is drawn");
        return;
    };

    let service = VideoRelayService::default();
    let relay = service.start(&source).await.expect("the relay starts");

    let mut client = tokio::time::timeout(
        Duration::from_secs(5),
        TcpStream::connect(("127.0.0.1", relay.port)),
    )
    .await
    .expect("the relay connects")
    .expect("the relay listens");

    client
        .write_all(b"GET /stream.mjpeg HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .await
        .expect("the request is written");

    let mut received = Vec::new();

    tokio::time::timeout(Duration::from_secs(20), async {
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

    let head = &received[..received.len().min(512)];
    let text = String::from_utf8_lossy(head);

    assert!(
        text.contains("multipart/x-mixed-replace"),
        "the content type is the one a picture element reads"
    );
    assert!(
        received.windows(2).any(|pair| pair == [0xFF, 0xD8]),
        "the body carries a JPEG"
    );

    service.stop(&source);

    assert!(service.status().is_empty(), "the relay is stopped");
}
