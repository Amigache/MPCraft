//! The live picture of the camera, drawn through `ffmpeg`.
//!
//! A camera describes its streams with a URI, and the stream travels outside MAVLink. A picture
//! element can draw an MJPEG stream or a still over `http` or `https`, but not the RTSP stream most
//! cameras publish. Mission Planner draws those through GStreamer; MPCraft asks `ffmpeg` to turn
//! the stream into MJPEG over a socket of this machine, and the picture element draws that.
//!
//! The relay is one process and one small HTTP server per stream. The process reads the camera and
//! writes the multipart body to its standard output; the server hands the same body to every
//! client that asks for `/stream.mjpeg`, with the content type a picture element expects. A client
//! that falls behind loses frames rather than growing without a bound, which is what a live picture
//! wants.
//!
//! Nothing is started until the operator says so, and only the loopback interface is opened: the
//! application makes no network call that nobody chose, and the relay is not a server for the
//! network.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::error::CommandError;

/// How often a client that has fallen behind is allowed to skip to the newest frame.
const CLIENT_BUFFER: usize = 32;

/// How much of the picture is read from `ffmpeg` at once.
const CHUNK: usize = 64 * 1024;

/// How long a client is given to send its request line.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// The path the relay answers.
const PATH: &str = "/stream.mjpeg";

/// The environment variable that names `ffmpeg`, for a machine that keeps it somewhere else.
pub const FFMPEG_PATH: &str = "MPCRAFT_FFMPEG";

/// What a relay is, as the interface sees it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoRelayInfo {
    /// The address of the camera the relay reads.
    pub uri: String,
    /// The address the picture element draws.
    pub url: String,
    /// The loopback port the relay listens on.
    pub port: u16,
}

/// Why a relay could not be started or stopped.
#[derive(Debug, thiserror::Error)]
pub enum VideoError {
    /// The address is not one a relay can read.
    #[error("the address is not one a relay can read: {0}")]
    BadAddress(String),
    /// `ffmpeg` is not on this machine.
    #[error("ffmpeg was not found: {0}")]
    NoFfmpeg(String),
    /// The process or the loopback port could not be opened.
    #[error("the relay could not be started: {0}")]
    Start(String),
    /// The state of the service cannot be reached.
    #[error("the state of the relays cannot be reached")]
    Locked,
}

impl VideoError {
    /// The stable code the frontend branches on.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadAddress(_) => CommandError::CODE_INVALID_REQUEST,
            Self::NoFfmpeg(_) | Self::Start(_) | Self::Locked => CommandError::CODE_INTERNAL,
        }
    }
}

/// One running relay.
struct Relay {
    /// Where the picture element draws it.
    url: String,
    /// The loopback port.
    port: u16,
    /// The process that reads the camera.
    child: tokio::process::Child,
    /// The server that answers the picture element.
    server: JoinHandle<()>,
    /// Set when the relay is being stopped, so the reader ends without reporting a failure.
    stopping: Arc<AtomicBool>,
}

/// The relays this station is running.
#[derive(Clone, Default)]
pub struct VideoRelayService {
    inner: Arc<Mutex<HashMap<String, Relay>>>,
}

impl VideoRelayService {
    /// Starts a relay for one camera address.
    ///
    /// The same address started twice replaces the first relay rather than opening a second
    /// process for it.
    ///
    /// # Errors
    ///
    /// Returns an error when the address is not one a relay can read, when `ffmpeg` is not on this
    /// machine, or when the loopback port or the process cannot be opened.
    pub async fn start(&self, uri: &str) -> Result<VideoRelayInfo, VideoError> {
        let uri = uri.trim();

        check_address(uri)?;

        self.stop(uri);

        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|error| VideoError::Start(error.to_string()))?;
        let port = listener
            .local_addr()
            .map_err(|error| VideoError::Start(error.to_string()))?
            .port();

        let boundary = format!("mpcraft{port}");
        let mut child = spawn(uri, &boundary).await?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| VideoError::Start("ffmpeg gave no output".to_owned()))?;
        let stderr = child.stderr.take();

        let (frames, _) = broadcast::channel::<Vec<u8>>(CLIENT_BUFFER);
        let stop = Arc::new(AtomicBool::new(false));

        tokio::spawn(read_frames(stdout, frames.clone(), Arc::clone(&stop)));

        if let Some(stderr) = stderr {
            tokio::spawn(read_errors(stderr));
        }

        let server = tokio::spawn(serve(listener, frames, boundary.clone()));

        let url = format!("http://127.0.0.1:{port}{PATH}");
        let relay = Relay {
            url: url.clone(),
            port,
            child,
            server,
            stopping: stop,
        };

        let mut inner = self.inner.lock().map_err(|_| VideoError::Locked)?;
        inner.insert(uri.to_owned(), relay);

        Ok(VideoRelayInfo {
            uri: uri.to_owned(),
            url,
            port,
        })
    }

    /// Stops the relay for one address, when it is running.
    pub fn stop(&self, uri: &str) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };

        if let Some(mut relay) = inner.remove(uri.trim()) {
            relay.stopping.store(true, Ordering::Relaxed);
            relay.server.abort();
            let _ = relay.child.start_kill();
        }
    }

    /// Stops every relay.
    pub fn stop_all(&self) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };

        for (_, mut relay) in inner.drain() {
            relay.stopping.store(true, Ordering::Relaxed);
            relay.server.abort();
            let _ = relay.child.start_kill();
        }
    }

    /// The relays that are running, as the interface sees them.
    #[must_use]
    pub fn status(&self) -> Vec<VideoRelayInfo> {
        let Ok(inner) = self.inner.lock() else {
            return Vec::new();
        };

        inner
            .iter()
            .map(|(uri, relay)| VideoRelayInfo {
                uri: uri.clone(),
                url: relay.url.clone(),
                port: relay.port,
            })
            .collect()
    }
}

/// Whether the address is one a relay can read.
///
/// # Errors
///
/// Returns the address when it does not begin with a scheme `ffmpeg` reads.
pub fn check_address(address: &str) -> Result<(), VideoError> {
    const SCHEMES: [&str; 9] = [
        "rtsp://", "rtsps://", "rtmp://", "rtmps://", "udp://", "srt://", "tcp://", "http://",
        "https://",
    ];

    if address.is_empty() {
        return Err(VideoError::BadAddress("the address is empty".to_owned()));
    }

    let lower = address.to_ascii_lowercase();

    let Some(scheme) = SCHEMES.iter().find(|scheme| lower.starts_with(*scheme)) else {
        return Err(VideoError::BadAddress(address.to_owned()));
    };

    if address[scheme.len()..].trim_matches('/').is_empty() {
        return Err(VideoError::BadAddress(address.to_owned()));
    }

    Ok(())
}

/// The `ffmpeg` this station runs.
///
/// Three places are tried, in this order: the variable the environment names, which is how a portable
/// installation and a test point at a build; a file beside the executable, which is where the
/// installer puts a sidecar; and the `PATH`. The sidecar is preferred over the `PATH` so that a
/// release carries the build it was tested with rather than whatever the machine happens to have.
fn program() -> String {
    let named = std::env::var(FFMPEG_PATH).ok();
    let executable = std::env::current_exe().ok();

    program_from(named.as_deref(), executable.as_deref())
}

/// The build named by one of those three places, chosen without touching the machine.
///
/// The choice is a function of two values so that it can be read rather than guessed at: a test hands
/// it the names and gets back what the relay would run.
#[must_use]
pub fn program_from(named: Option<&str>, executable: Option<&std::path::Path>) -> String {
    if let Some(program) = named {
        return program.to_owned();
    }

    if let Some(folder) = executable.and_then(std::path::Path::parent) {
        for name in ["ffmpeg.exe", "ffmpeg"] {
            let candidate = folder.join(name);

            if candidate.is_file() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }

    "ffmpeg".to_owned()
}

/// Starts `ffmpeg` for one address.
async fn spawn(address: &str, boundary: &str) -> Result<tokio::process::Child, VideoError> {
    let program = program();

    let mut command = Command::new(&program);

    command
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-fflags",
            "nobuffer",
            "-flags",
            "low_delay",
        ]);

    // RTSP is the protocol most cameras publish, and asking for TCP keeps a stream from being
    // broken by a network that drops UDP; the other protocols are left to the demuxer.
    if address.to_ascii_lowercase().starts_with("rtsp") {
        command.args(["-rtsp_transport", "tcp"]);
    }

    command.args(["-i", address]).args([
        "-an",
        "-f",
        "mpjpeg",
        "-q:v",
        "5",
        "-boundary_tag",
        boundary,
        "pipe:1",
    ]);

    command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            VideoError::NoFfmpeg(program)
        } else {
            VideoError::Start(error.to_string())
        }
    })
}

/// Reads the multipart body from `ffmpeg` and hands it to the clients.
async fn read_frames(
    mut stdout: tokio::process::ChildStdout,
    frames: broadcast::Sender<Vec<u8>>,
    stopping: Arc<AtomicBool>,
) {
    let mut buffer = vec![0_u8; CHUNK];

    loop {
        match stdout.read(&mut buffer).await {
            Ok(0) => break,
            Ok(count) => {
                let _ = frames.send(buffer[..count].to_vec());
            }
            Err(error) => {
                if !stopping.load(Ordering::Relaxed) {
                    tracing::warn!("the video relay stopped reading: {error}");
                }

                break;
            }
        }
    }
}

/// Keeps what `ffmpeg` says about a stream it cannot read, so a failure is not silent.
async fn read_errors(mut stderr: tokio::process::ChildStderr) {
    let mut buffer = vec![0_u8; 4096];

    while let Ok(count) = stderr.read(&mut buffer).await {
        if count == 0 {
            break;
        }

        let text = String::from_utf8_lossy(&buffer[..count]);
        let text = text.trim();

        if !text.is_empty() {
            tracing::warn!("the video relay: {text}");
        }
    }
}

/// Answers the picture element with the multipart body.
async fn serve(listener: TcpListener, frames: broadcast::Sender<Vec<u8>>, boundary: String) {
    loop {
        let Ok((mut stream, _)) = listener.accept().await else {
            continue;
        };

        let frames = frames.clone();
        let boundary = boundary.clone();

        tokio::spawn(async move {
            let _ = answer(&mut stream, frames, &boundary).await;
        });
    }
}

/// Answers one request.
async fn answer(
    stream: &mut tokio::net::TcpStream,
    frames: broadcast::Sender<Vec<u8>>,
    boundary: &str,
) -> std::io::Result<()> {
    let mut request = vec![0_u8; 2048];

    let count = tokio::time::timeout(REQUEST_TIMEOUT, stream.read(&mut request))
        .await
        .unwrap_or(Ok(0))?;

    let text = String::from_utf8_lossy(&request[..count]);
    let path = text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    if path != PATH {
        stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await?;

        return Ok(());
    }

    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: multipart/x-mixed-replace; boundary={boundary}\r\nCache-Control: no-cache, no-store, must-revalidate\r\nPragma: no-cache\r\nConnection: close\r\n\r\n"
    );

    stream.write_all(header.as_bytes()).await?;

    // The subscription is taken after the headers, so a client that connected before the first
    // frame gets the frames that follow rather than the ones that were already sent.
    let mut frames = frames.subscribe();

    loop {
        match frames.recv().await {
            Ok(frame) => {
                if stream.write_all(&frame).await.is_err() {
                    return Ok(());
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_addresses_a_relay_reads_are_the_schemes_it_knows() {
        for address in [
            "rtsp://camera/live",
            "rtsps://camera/live",
            "rtmp://camera/live",
            "udp://127.0.0.1:5600",
            "srt://127.0.0.1:9000",
            "http://camera/stream.mjpeg",
        ] {
            assert!(check_address(address).is_ok(), "{address} is read");
        }
    }

    #[test]
    fn an_address_that_is_not_a_stream_is_refused() {
        for address in ["", "file:///tmp/a.mp4", "camera", "rtsp://", "rtsp:///"] {
            assert!(check_address(address).is_err(), "{address} is refused");
        }
    }

    #[test]
    fn the_build_beside_the_executable_is_the_one_that_runs() {
        let folder = std::env::temp_dir().join(format!(
            "mpcraft-ffmpeg-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));

        std::fs::create_dir_all(&folder).expect("a folder for the test");
        std::fs::write(folder.join("ffmpeg.exe"), b"not a build").expect("the file is written");

        let executable = folder.join("mpcraft");
        let chosen = program_from(None, Some(&executable));

        assert_eq!(
            chosen,
            folder.join("ffmpeg.exe").to_string_lossy(),
            "the sidecar the installer carries is what runs"
        );

        std::fs::remove_dir_all(&folder).ok();
    }

    #[test]
    fn a_build_the_environment_names_is_used_whatever_sits_beside_the_executable() {
        let chosen = program_from(
            Some("/opt/ffmpeg/bin/ffmpeg"),
            Some(std::path::Path::new("/opt/mpcraft/mpcraft")),
        );

        assert_eq!(chosen, "/opt/ffmpeg/bin/ffmpeg");
    }

    #[test]
    fn nothing_is_guessed_when_no_build_is_named_or_beside_it() {
        assert_eq!(program_from(None, None), "ffmpeg");
        assert_eq!(
            program_from(None, Some(std::path::Path::new("/opt/mpcraft/mpcraft"))),
            "ffmpeg"
        );
    }
}
