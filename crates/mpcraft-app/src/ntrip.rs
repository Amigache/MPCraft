//! The corrections of a base station.
//!
//! A base station watches the satellites it can see and works out how far each of them
//! is, and it sends those corrections to a **caster**. The caster passes them to whoever
//! asks, and the way of asking is NTRIP: an HTTP request that is answered with the
//! stream of the corrections.
//!
//! The position of the vehicle travels with the request, as a GGA sentence, so that the
//! caster knows which corrections are wanted. The corrections themselves are for the
//! receiver of the vehicle, and the way to give them to it is the serial port of its
//! GPS.
//!
//! Upstream has the same thing (`CommsNTRIP.cs`), and it presents itself as a serial
//! port that the operator points the GPS of the vehicle at.
//!
//! ```text
//! GCS  ──GET /mount HTTP/1.1──▶  caster
//! GCS  ◀────"200 OK"───────────  caster
//! GCS  ──$GPGGA,……*47──────────▶  caster      (where the vehicle is)
//! GCS  ◀────RTCM corrections───  caster      (over and over)
//! ```

use std::time::Duration;

use mpcraft_vehicle::GeoPoint;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// The port a caster listens on when the URL does not say.
const DEFAULT_PORT: u16 = 2101;

/// How long to wait for the caster to answer.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long to wait for the answer of the caster.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

/// The settings of a source of corrections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NtripSettings {
    /// The URL of the caster: `http://user:password@host:port/mount`.
    pub url: String,
    /// The port of the vehicle the corrections go to.
    pub gps_port: u8,
    /// Whether the caster is asked the old way, with the headers of NTRIP 1.0.
    pub version1: bool,
}

/// Why a source of corrections could not be used.
#[derive(Debug, thiserror::Error)]
pub enum NtripError {
    /// The URL of the caster is not one that can be used.
    #[error("the address of the caster is not one that can be used: {0}")]
    BadUrl(String),
    /// The caster could not be reached.
    #[error("the caster could not be reached: {0}")]
    Connect(String),
    /// The caster did not answer.
    #[error("the caster did not answer")]
    Timeout,
    /// The caster refused.
    #[error("the caster refused: {0}")]
    Refused(String),
    /// The connection to the caster broke.
    #[error("the connection to the caster broke: {0}")]
    Broken(String),
}

/// The parts of the address of a caster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CasterAddress {
    /// The host the caster is at.
    pub host: String,
    /// The port the caster listens on.
    pub port: u16,
    /// The path of the mount point.
    pub path: String,
    /// The name of the user, when the address carries one.
    pub user: Option<String>,
    /// The password of the user, when the address carries one.
    pub password: Option<String>,
}

impl CasterAddress {
    /// Reads the address of a caster out of its URL.
    ///
    /// The URL is the one upstream takes: `http://user:password@host:port/mount`, and
    /// the `ntrip` scheme is the same thing as `http`.
    ///
    /// # Errors
    ///
    /// Returns an error when the URL has no host, or when its port is not a number.
    pub fn parse(url: &str) -> Result<Self, NtripError> {
        let url = url.trim();

        let rest = url
            .strip_prefix("http://")
            .or_else(|| url.strip_prefix("ntrip://"))
            .or_else(|| url.strip_prefix("https://"))
            .ok_or_else(|| NtripError::BadUrl(url.to_owned()))?;

        // The credentials come first, and the last at-sign separates them from the host.
        let (credentials, rest) = match rest.rfind('@') {
            Some(at) => (Some(&rest[..at]), &rest[at + 1..]),
            None => (None, rest),
        };

        let (user, password) = match credentials {
            Some(text) => match text.split_once(':') {
                Some((user, password)) => (Some(user.to_owned()), Some(password.to_owned())),
                None => (Some(text.to_owned()), None),
            },
            None => (None, None),
        };

        let (authority, path) = match rest.find('/') {
            Some(slash) => (&rest[..slash], &rest[slash..]),
            None => (rest, "/"),
        };

        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) => {
                let port = port.parse::<u16>().map_err(|_| {
                    NtripError::BadUrl(format!("the port of the caster is not a number: {port}"))
                })?;

                (host, port)
            }
            None => (authority, DEFAULT_PORT),
        };

        if host.is_empty() {
            return Err(NtripError::BadUrl(url.to_owned()));
        }

        Ok(Self {
            host: host.to_owned(),
            port,
            path: path.to_owned(),
            user,
            password,
        })
    }

    /// The request the caster is asked with.
    ///
    /// The second version of the protocol says which version it is and names the host,
    /// which the first one does not.
    #[must_use]
    pub fn request(&self, version1: bool) -> Vec<u8> {
        let mut request = format!(
            "GET {} HTTP/1.1\r\nHost: {}:{}\r\n",
            self.path, self.host, self.port
        );

        if !version1 {
            request.push_str("Ntrip-Version: Ntrip/2.0\r\n");
        }

        request.push_str("User-Agent: NTRIP MPCraft/1.0\r\n");

        if let (Some(user), Some(password)) = (&self.user, &self.password) {
            let credentials = base64(format!("{user}:{password}").as_bytes());
            request.push_str(&format!("Authorization: Basic {credentials}\r\n"));
        }

        request.push_str("Connection: close\r\n\r\n");

        request.into_bytes()
    }
}

/// The text of a GGA sentence, with its checksum.
///
/// The sentence says where the vehicle is, in the form the caster expects: the degrees
/// and the minutes, and the hemisphere apart.
#[must_use]
pub fn gga_sentence(
    time: &str,
    latitude: f64,
    longitude: f64,
    altitude: f64,
    quality: u8,
    satellites: u8,
    hdop: f32,
) -> String {
    let (lat_whole, lat_minutes) = to_degrees_minutes(latitude);
    let (lng_whole, lng_minutes) = to_degrees_minutes(longitude);

    let lat = format!("{lat_whole:02}{lat_minutes:05.2}");
    let lng = format!("{lng_whole:03}{lng_minutes:05.2}");

    let body = format!(
        "GPGGA,{time},{lat},{},{lng},{},{quality:01},{satellites:02},{hdop:.1},{altitude:.2},M,0.00,M,0.0,0000",
        if latitude < 0.0 { "S" } else { "N" },
        if longitude < 0.0 { "W" } else { "E" },
    );

    let checksum = checksum(&body);

    format!("${body}*{checksum:02X}")
}

/// The degrees and the minutes of a coordinate, in the form of a GGA sentence.
fn to_degrees_minutes(degrees: f64) -> (u32, f64) {
    let absolute = degrees.abs();
    let whole = absolute.trunc();
    let mut minutes = ((absolute - whole) * 60.0 * 100.0).round() / 100.0;

    if minutes >= 60.0 {
        minutes = 0.0;
    }

    (whole as u32, minutes)
}

/// The checksum of a sentence: the exclusive or of every character of it.
fn checksum(body: &str) -> u8 {
    body.bytes().fold(0_u8, |checksum, byte| checksum ^ byte)
}

/// The text of a byte string, in base sixty four.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::new();

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        let triple = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);

        out.push(ALPHABET[(triple >> 18) as usize & 0x3F] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 0x3F] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(triple >> 6) as usize & 0x3F] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[triple as usize & 0x3F] as char
        } else {
            '='
        });
    }

    out
}

/// A client of a caster of corrections.
#[derive(Debug)]
pub struct NtripClient {
    stream: TcpStream,
}

impl NtripClient {
    /// Connects to a caster and asks for the corrections.
    ///
    /// # Errors
    ///
    /// Returns an error when the address cannot be used, when the caster cannot be
    /// reached, or when it does not answer with the stream of the corrections.
    pub async fn connect(settings: &NtripSettings) -> Result<Self, NtripError> {
        let address = CasterAddress::parse(&settings.url)?;

        let stream = tokio::time::timeout(
            CONNECT_TIMEOUT,
            TcpStream::connect((address.host.as_str(), address.port)),
        )
        .await
        .map_err(|_| NtripError::Timeout)?
        .map_err(|error| NtripError::Connect(error.to_string()))?;

        let mut client = Self { stream };

        client
            .stream
            .write_all(&address.request(settings.version1))
            .await
            .map_err(|error| NtripError::Broken(error.to_string()))?;

        let answer = client.read_answer().await?;

        let first = answer.lines().next().unwrap_or_default().to_owned();

        if !first.contains("200") {
            return Err(NtripError::Refused(first));
        }

        if first.contains("SOURCETABLE") || answer.contains("SOURCETABLE") {
            return Err(NtripError::Refused(
                "the caster answered with the list of its mounts, not with the corrections"
                    .to_owned(),
            ));
        }

        Ok(client)
    }

    /// Reads the answer of the caster, up to the empty line that ends its headers.
    async fn read_answer(&mut self) -> Result<String, NtripError> {
        let mut answer = Vec::new();
        let mut byte = [0_u8; 1];

        loop {
            let read = tokio::time::timeout(ANSWER_TIMEOUT, self.stream.read(&mut byte))
                .await
                .map_err(|_| NtripError::Timeout)?
                .map_err(|error| NtripError::Broken(error.to_string()))?;

            if read == 0 {
                break;
            }

            answer.push(byte[0]);

            // The headers end with an empty line.
            if answer.ends_with(b"\r\n\r\n") || answer.ends_with(b"\n\n") {
                break;
            }

            if answer.len() > 8192 {
                break;
            }
        }

        Ok(String::from_utf8_lossy(&answer).to_string())
    }

    /// Sends the position of the vehicle to the caster.
    ///
    /// # Errors
    ///
    /// Returns an error when the connection to the caster breaks.
    pub async fn send_gga(&mut self, sentence: &str) -> Result<(), NtripError> {
        self.stream
            .write_all(format!("{sentence}\r\n").as_bytes())
            .await
            .map_err(|error| NtripError::Broken(error.to_string()))
    }

    /// Reads the corrections the caster sends.
    ///
    /// # Errors
    ///
    /// Returns an error when the connection to the caster breaks.
    pub async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, NtripError> {
        let read = self
            .stream
            .read(buffer)
            .await
            .map_err(|error| NtripError::Broken(error.to_string()))?;

        Ok(read)
    }
}

/// What the source of corrections is doing.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NtripStatus {
    /// Whether the corrections are being sent.
    pub running: bool,
    /// How many bytes of corrections have been sent to the vehicle.
    pub bytes: u64,
    /// The last failure, when there was one.
    pub error: Option<String>,
    /// The position that was last sent to the caster.
    pub position: Option<GeoPoint>,
}

/// The corrections of a base station, being sent to the vehicle.
///
/// The service holds the client and the task that reads the corrections, and the
/// position of the vehicle is fed to it as the vehicle flies.
#[derive(Clone, Default)]
pub struct NtripService {
    inner: std::sync::Arc<std::sync::Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    running: bool,
    bytes: u64,
    error: Option<String>,
    position: Option<GeoPoint>,
    stop: Option<tokio::task::JoinHandle<()>>,
}

impl NtripService {
    /// What the source of corrections is doing.
    #[must_use]
    pub fn status(&self) -> NtripStatus {
        let Ok(inner) = self.inner.lock() else {
            return NtripStatus {
                running: false,
                bytes: 0,
                error: Some("the state of the corrections is locked".to_owned()),
                position: None,
            };
        };

        NtripStatus {
            running: inner.running,
            bytes: inner.bytes,
            error: inner.error.clone(),
            position: inner.position,
        }
    }

    /// Says where the vehicle is.
    ///
    /// The position is sent to the caster, so that it knows which corrections are
    /// wanted.
    pub fn set_position(&self, position: GeoPoint) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.position = Some(position);
        }
    }

    /// Connects to a caster and starts sending the corrections to the vehicle.
    ///
    /// # Errors
    ///
    /// Returns an error when the caster cannot be reached, when it refuses, or when the
    /// corrections are already being sent.
    pub async fn start(
        &self,
        session: std::sync::Arc<mpcraft_link::Session>,
        vehicle: mpcraft_link::VehicleId,
        settings: NtripSettings,
    ) -> Result<(), NtripError> {
        {
            let Ok(mut inner) = self.inner.lock() else {
                return Err(NtripError::Broken("the state is locked".to_owned()));
            };

            if inner.running {
                return Err(NtripError::Broken(
                    "the corrections are already being sent".to_owned(),
                ));
            }

            inner.error = None;
            inner.bytes = 0;
        }

        let client = NtripClient::connect(&settings).await?;

        let inner = std::sync::Arc::clone(&self.inner);
        let gps_port = settings.gps_port;

        let task = tokio::spawn(async move {
            pump(client, session, vehicle, gps_port, inner).await;
        });

        let Ok(mut inner) = self.inner.lock() else {
            return Ok(());
        };

        inner.running = true;
        inner.stop = Some(task);

        Ok(())
    }

    /// Stops sending the corrections.
    pub fn stop(&self) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };

        inner.running = false;

        if let Some(task) = inner.stop.take() {
            task.abort();
        }
    }
}

/// Reads the corrections and gives them to the vehicle.
async fn pump(
    mut client: NtripClient,
    session: std::sync::Arc<mpcraft_link::Session>,
    vehicle: mpcraft_link::VehicleId,
    gps_port: u8,
    inner: std::sync::Arc<std::sync::Mutex<Inner>>,
) {
    let mut buffer = [0_u8; 512];

    // The position of the vehicle is sent to the caster every thirty seconds, and once
    // at the beginning: the caster needs it to know which corrections are wanted.
    let mut last_sent: Option<std::time::Instant> = None;

    loop {
        let read = tokio::select! {
            read = client.read(&mut buffer) => read,
            _ = tokio::time::sleep(Duration::from_millis(500)) => {
                // The time to send the position again.
                let due = last_sent
                    .map(|sent| sent.elapsed() >= Duration::from_secs(30))
                    .unwrap_or(true);

                if due {
                    if let Some(position) = position_of(&inner) {
                        let sentence = gga_sentence(
                            &time_of_day(),
                            position.lat,
                            position.lng,
                            position.alt,
                            1,
                            10,
                            1.0,
                        );

                        if let Err(error) = client.send_gga(&sentence).await {
                            fail(&inner, error.to_string());
                            return;
                        }

                        last_sent = Some(std::time::Instant::now());
                    }
                }

                continue;
            }
        };

        let read = match read {
            Ok(read) => read,
            Err(error) => {
                fail(&inner, error.to_string());
                return;
            }
        };

        if read == 0 {
            // The caster closed the connection.
            fail(&inner, "the caster closed the connection".to_owned());
            return;
        }

        if let Err(error) = session
            .serial_control(vehicle, gps_port, &buffer[..read], 0, 0)
            .await
        {
            fail(&inner, error.to_string());
            return;
        }

        if let Ok(mut inner) = inner.lock() {
            inner.bytes += read as u64;
        }
    }
}

/// The position the vehicle is at, when it is known.
fn position_of(inner: &std::sync::Arc<std::sync::Mutex<Inner>>) -> Option<GeoPoint> {
    inner.lock().ok().and_then(|inner| inner.position)
}

/// Says that the corrections have stopped, and why.
fn fail(inner: &std::sync::Arc<std::sync::Mutex<Inner>>, error: String) {
    if let Ok(mut inner) = inner.lock() {
        inner.running = false;
        inner.error = Some(error);
    }
}

/// The time of the day, as a GGA sentence wants it.
fn time_of_day() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    let seconds = now.as_secs() % 86_400;

    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let rest = seconds % 60;

    format!("{hours:02}{minutes:02}{rest:02}.00")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_is_read_out_of_its_url() {
        let address = CasterAddress::parse("http://user:secret@caster.example:2101/mount").unwrap();

        assert_eq!(address.host, "caster.example");
        assert_eq!(address.port, 2101);
        assert_eq!(address.path, "/mount");
        assert_eq!(address.user.as_deref(), Some("user"));
        assert_eq!(address.password.as_deref(), Some("secret"));
    }

    #[test]
    fn an_address_without_a_port_uses_the_one_of_the_protocol() {
        let address = CasterAddress::parse("http://caster.example/mount").unwrap();

        assert_eq!(address.port, DEFAULT_PORT);
        assert_eq!(address.user, None);
    }

    #[test]
    fn the_scheme_of_the_protocol_is_the_one_of_the_web() {
        let address = CasterAddress::parse("ntrip://caster.example:2101/mount").unwrap();

        assert_eq!(address.host, "caster.example");
    }

    #[test]
    fn an_address_without_a_host_is_refused() {
        assert!(CasterAddress::parse("http://").is_err());
    }

    #[test]
    fn the_request_says_which_version_of_the_protocol_it_is() {
        let address = CasterAddress::parse("http://caster.example:2101/mount").unwrap();

        let request = String::from_utf8(address.request(false)).unwrap();
        assert!(request.starts_with("GET /mount HTTP/1.1"));
        assert!(request.contains("Ntrip-Version: Ntrip/2.0"));
        assert!(request.contains("Host: caster.example:2101"));

        let request = String::from_utf8(address.request(true)).unwrap();
        assert!(!request.contains("Ntrip-Version"));
    }

    #[test]
    fn the_credentials_are_sent_as_the_protocol_says() {
        let address = CasterAddress::parse("http://user:secret@caster.example/mount").unwrap();

        let request = String::from_utf8(address.request(false)).unwrap();
        assert!(request.contains("Authorization: Basic dXNlcjpzZWNyZXQ="));
    }

    #[test]
    fn a_sentence_carries_the_position_and_its_checksum() {
        let sentence = gga_sentence("123519.00", 48.1173, 11.5167, 545.4, 1, 8, 0.9);

        assert!(sentence.starts_with("$GPGGA,123519.00,4807.04,N,01131.00,E,1,08,0.9,545.40,M,"));

        // The checksum is the exclusive or of everything between the two marks.
        let body = &sentence[1..sentence.len() - 3];
        assert_eq!(sentence, format!("${body}*{:02X}", checksum(body)));
    }

    #[test]
    fn a_sentence_says_the_hemisphere_apart() {
        let north = gga_sentence("123519.00", 48.1173, 11.5167, 545.4, 1, 8, 0.9);
        let south = gga_sentence("123519.00", -48.1173, -11.5167, 545.4, 1, 8, 0.9);

        assert!(north.contains(",N,"));
        assert!(north.contains(",E,"));
        assert!(south.contains(",S,"));
        assert!(south.contains(",W,"));
    }

    #[test]
    fn the_checksum_is_the_exclusive_or_of_the_sentence() {
        let body = "GPGGA,123519.00,4807.04,N,01131.00,E,1,08,0.9,545.40,M,0.00,M,0.0,0000";
        let expected = body.bytes().fold(0_u8, |sum, byte| sum ^ byte);

        assert_eq!(checksum(body), expected);
    }
}
