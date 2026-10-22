//! The web server of the application.
//!
//! Mission Planner can serve what it knows to other programs: the picture of the
//! camera, the track of the flight as KML, the state of the vehicle. MPCraft serves the
//! two that are text — the track and the state — over a small server of its own, so
//! that another machine can follow the flight without a link of its own.
//!
//! The server is deliberately small: it answers three things and nothing else, it
//! listens on the loopback and on the network of the machine, and it never talks to
//! anything that is not asking for one of those three things.

use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::error::{CommandError, CommandResult};

/// The port the server listens on when none is said.
pub const DEFAULT_PORT: u16 = 8080;

/// How long a request is given to arrive.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// The server of the application.
pub struct WebServer {
    /// The port it listens on.
    port: u16,
    /// The task that serves, which is stopped when the server is dropped.
    task: tokio::task::JoinHandle<()>,
}

impl WebServer {
    /// Starts the server.
    ///
    /// # Errors
    ///
    /// Returns an error when the port cannot be opened.
    pub async fn start(
        port: u16,
        state: Arc<crate::AppState>,
    ) -> CommandResult<Self> {
        let listener = TcpListener::bind(("0.0.0.0", port))
            .await
            .map_err(|error| {
                CommandError::internal(format!("the port {port} could not be opened: {error}"))
            })?;

        let task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    continue;
                };

                let state = Arc::clone(&state);
                tokio::spawn(async move {
                    let _ = serve(&mut stream, state).await;
                });
            }
        });

        Ok(Self { port, task })
    }

    /// The port the server listens on.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Stops the server.
    pub fn stop(self) {
        self.task.abort();
    }
}

/// Answers one request.
async fn serve(
    stream: &mut tokio::net::TcpStream,
    state: Arc<crate::AppState>,
) -> std::io::Result<()> {
    // The request is a line and a few headers; only the line is read.
    let mut buffer = vec![0_u8; 2048];
    let read = tokio::time::timeout(REQUEST_TIMEOUT, stream.read(&mut buffer))
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "no request"))?
        .unwrap_or(0);

    if read == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..read]);
    let path = request
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/")
        .to_owned();

    let (content_type, body) = answer(&path, &state);

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );

    stream.write_all(response.as_bytes()).await?;
    stream.flush().await
}

/// What is said for a path.
fn answer(path: &str, state: &crate::AppState) -> (&'static str, String) {
    match path {
        "/" | "/index.html" => ("text/html; charset=utf-8", page()),

        "/telemetry.json" => (
            "application/json",
            serde_json::to_string_pretty(&telemetry_of(state)).unwrap_or_else(|_| "{}".to_owned()),
        ),

        "/kml" => ("application/vnd.google-earth.kml+xml", kml_of(state)),

        _ => ("text/plain; charset=utf-8", "not found".to_owned()),
    }
}

/// The state of the vehicle, as it is now.
fn telemetry_of(state: &crate::AppState) -> serde_json::Value {
    match state.supervisor.state() {
        Some(session) => serde_json::to_value(&session.telemetry).unwrap_or_else(|_| serde_json::json!({})),
        None => serde_json::json!({}),
    }
}

/// The track of the flight, as a KML document.
fn kml_of(state: &crate::AppState) -> String {
    let track = state
        .live_track
        .lock()
        .ok()
        .and_then(|cell| cell.as_ref().map(|track| track.document()));

    track.unwrap_or_else(|| {
        mpcraft_mission::kml::write_track_kml("MPCraft track", &[])
    })
}

/// The page that shows the flight.
fn page() -> String {
    // The page is a small one: it asks the server for the state of the vehicle and
    // draws it, once a second, the way the window does.
    r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>MPCraft</title>
<style>
  body { background: #10131a; color: #d8dee9; font-family: ui-monospace, monospace; margin: 2rem; }
  h1 { font-size: 1rem; letter-spacing: .1em; text-transform: uppercase; color: #8fbcbb; }
  table { border-collapse: collapse; }
  td { padding: .2rem 1.5rem .2rem 0; }
  .k { color: #7b8894; }
</style>
</head>
<body>
<h1>MPCraft</h1>
<table id="t"></table>
<p><a href="/kml" style="color:#8fbcbb">The track (KML)</a> · <a href="/telemetry.json" style="color:#8fbcbb">The state (JSON)</a></p>
<script>
async function draw() {
  const answer = await fetch("/telemetry.json");
  const state = await answer.json();
  const rows = [];
  const put = (name, value) => rows.push(`<tr><td class="k">${name}</td><td>${value}</td></tr>`);
  if (state.position) {
    put("Latitude", state.position.lat.toFixed(7));
    put("Longitude", state.position.lng.toFixed(7));
    put("Height", Math.round(state.position.alt) + " m");
  }
  if (state.attitude) {
    put("Roll", state.attitude.roll.toFixed(1) + "°");
    put("Pitch", state.attitude.pitch.toFixed(1) + "°");
    put("Yaw", state.attitude.yaw.toFixed(1) + "°");
  }
  if (state.velocity) put("Ground speed", state.velocity.groundSpeed.toFixed(1) + " m/s");
  if (state.battery) put("Battery", state.battery.voltage.toFixed(2) + " V");
  if (state.mode) put("Mode", state.mode.name ?? state.mode.customMode);
  put("Armed", state.armed === null ? "—" : (state.armed ? "yes" : "no"));
  document.getElementById("t").innerHTML = rows.join("");
}
draw();
setInterval(draw, 1000);
</script>
</body>
</html>
"#
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppState;

    #[test]
    fn the_paths_are_answered_with_what_they_are_asked_for() {
        let state = Arc::new(AppState::default());

        let (kind, body) = answer("/telemetry.json", &state);
        assert_eq!(kind, "application/json");
        assert!(body.starts_with('{'), "the state is a document: {body}");

        let (kind, body) = answer("/kml", &state);
        assert!(kind.contains("kml"));
        assert!(body.contains("<kml"), "the track is a document: {body}");

        let (kind, body) = answer("/", &state);
        assert!(kind.contains("text/html"));
        assert!(body.contains("MPCraft"));
    }

    #[test]
    fn a_path_that_is_not_asked_for_is_not_found() {
        let state = Arc::new(AppState::default());

        let (kind, body) = answer("/nothing-here", &state);
        assert_eq!(kind, "text/plain; charset=utf-8");
        assert_eq!(body, "not found");
    }
}
