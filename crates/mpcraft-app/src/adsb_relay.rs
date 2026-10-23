//! The traffic of the air, sent back onto the link.
//!
//! A vehicle with a receiver tells the ground station what it hears, and the station draws
//! it on the map. But the station can hear the traffic somewhere else as well — from an API
//! that publishes what a network of receivers has seen — and then the aircraft it read are
//! not on the link, so the other stations listening to this link do not see them.
//!
//! The relay is what puts them on the link: it reads its source every ten seconds, which is
//! what the service asks of its readers, and once a second it takes the aircraft that came
//! from that source, keeps the ones close to the vehicle, and sends one of them, going through
//! the list in order. One per second rather than all at once, because the link carries
//! telemetry at twenty hertz and a list of forty aircraft would be noise.
//!
//! Upstream does the same (`MainV2.cs`, `ADSBRunner`): ten closest, one second apart, and
//! the ones that already came over the link are left out, because sending them back would
//! say the same thing twice.
//!
//! ```text
//! API  ──GET /v2/point/lat/lng/radius──▶  the station
//!                        │
//!                        ▼
//!              the ten closest to the vehicle
//!                        │  one each second, in order
//!                        ▼
//!                   the link, and every station on it
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use mpcraft_link::{AdsbRegistry, AdsbSource, AdsbVehicle, Session, VehicleId};
use mpcraft_vehicle::GeoPoint;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{CommandError, CommandResult};

/// How often the relay sends one aircraft.
pub const SEND_INTERVAL: Duration = Duration::from_secs(1);

/// How often the source is asked for the list of aircraft.
///
/// The service that publishes the traffic asks that its readers do not come more than once
/// every ten seconds; what was read is kept while the aircraft are around, so the relay can
/// still send one each second.
pub const FETCH_INTERVAL: Duration = Duration::from_secs(10);

/// What the client says about itself when it asks.
///
/// The source answers a client that says nothing about itself with a refusal: an address that
/// does not say who is asking is not a request a service can allow.
pub const USER_AGENT: &str = "MPCraft/0.1 (ground station reading the traffic of the air)";

/// How far from the vehicle an aircraft has to be to be worth sending, in metres.
pub const RELAY_RADIUS_M: f64 = 10_000.0;

/// How many aircraft the relay chooses between, the ten closest.
pub const RELAY_TOP: usize = 10;

/// An aircraft that has not been heard for half a minute is not around any more.
pub const RELAY_STALE: Duration = Duration::from_secs(30);

/// How far round the vehicle the API is asked, in nautical miles, when nothing is chosen.
pub const DEFAULT_RADIUS_NM: u32 = 100;

/// What the relay reads from and how far it looks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdsbRelaySettings {
    /// The address of the source, as an `http` or `https` URL.
    pub server: String,
    /// How far round the vehicle the list is asked for, in nautical miles.
    pub radius_nm: u32,
}

impl Default for AdsbRelaySettings {
    fn default() -> Self {
        Self {
            server: String::new(),
            radius_nm: DEFAULT_RADIUS_NM,
        }
    }
}

/// Why the relay could not be started or could not read its source.
#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    /// The address of the source is not one that can be used.
    #[error("the address of the source is not one that can be used: {0}")]
    BadServer(String),
    /// The distance to ask the source for is not one that can be used.
    #[error("the distance to ask the source for is not one that can be used: {0}")]
    BadRadius(u32),
    /// The relay is already sending, or its state cannot be reached.
    #[error("{0}")]
    Busy(String),
    /// The source could not be reached, or did not answer with what was expected.
    #[error("the source did not answer: {0}")]
    Fetch(String),
    /// The answer was not the document that was expected.
    #[error("the answer of the source is not the document that was expected: {0}")]
    Document(String),
}

impl RelayError {
    /// The stable code the frontend branches on.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadServer(_) | Self::BadRadius(_) | Self::Busy(_) => {
                CommandError::CODE_INVALID_REQUEST
            }
            Self::Fetch(_) => CommandError::CODE_INTERNAL,
            Self::Document(_) => CommandError::CODE_INTERNAL,
        }
    }
}

/// What the relay is doing.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdsbRelayStatus {
    /// Whether the relay is running.
    pub running: bool,
    /// How many aircraft have been sent onto the link.
    pub sent: u64,
    /// Which place of the list the next one will be taken from.
    pub index: usize,
    /// How many aircraft the station has read and is keeping.
    pub aircraft: usize,
    /// The last failure, when there was one.
    pub error: Option<String>,
    /// Where the vehicle is, because the list is asked around it.
    pub position: Option<GeoPoint>,
    /// What the relay reads from and how far it looks.
    pub settings: AdsbRelaySettings,
}

/// The state the task owns.
#[derive(Default)]
struct Inner {
    running: bool,
    sent: u64,
    index: usize,
    error: Option<String>,
    settings: AdsbRelaySettings,
    position: Option<GeoPoint>,
    registry: AdsbRegistry,
    task: Option<tokio::task::JoinHandle<()>>,
}

/// The traffic of the air, being sent back onto the link.
///
/// The service holds its own list of aircraft — the ones the station read — so that the
/// traffic the vehicle sent is not mixed with the traffic the station read: only the second
/// one is worth sending, and the two are kept apart by where they came from.
#[derive(Clone, Default)]
pub struct AdsbRelayService {
    inner: Arc<std::sync::Mutex<Inner>>,
}

impl AdsbRelayService {
    /// What the relay is doing.
    #[must_use]
    pub fn status(&self) -> AdsbRelayStatus {
        let Ok(inner) = self.inner.lock() else {
            return AdsbRelayStatus {
                running: false,
                sent: 0,
                index: 0,
                aircraft: 0,
                error: Some("the state of the relay is locked".to_owned()),
                position: None,
                settings: AdsbRelaySettings::default(),
            };
        };

        AdsbRelayStatus {
            running: inner.running,
            sent: inner.sent,
            index: inner.index,
            aircraft: inner.registry.len(),
            error: inner.error.clone(),
            position: inner.position,
            settings: inner.settings.clone(),
        }
    }

    /// The aircraft the station has read, so that the map can draw them with the ones the
    /// vehicle sent.
    #[must_use]
    pub fn aircraft(&self) -> Vec<AdsbVehicle> {
        self.inner
            .lock()
            .map(|inner| inner.registry.vehicles())
            .unwrap_or_default()
    }

    /// Says where the vehicle is, because the list of aircraft is asked around it.
    pub fn set_position(&self, position: GeoPoint) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.position = Some(position);
        }
    }

    /// Starts sending the aircraft the station reads onto the link.
    ///
    /// # Errors
    ///
    /// Returns an error when the relay is already sending, or when the settings are not ones
    /// that can be used.
    pub fn start(
        &self,
        session: Arc<Session>,
        vehicle: VehicleId,
        settings: AdsbRelaySettings,
    ) -> Result<(), RelayError> {
        check_settings(&settings)?;

        {
            let Ok(mut inner) = self.inner.lock() else {
                return Err(RelayError::Busy(
                    "the state of the relay is locked".to_owned(),
                ));
            };

            if inner.running {
                return Err(RelayError::Busy("the relay is already sending".to_owned()));
            }

            inner.error = None;
            inner.sent = 0;
            inner.index = 0;
            inner.settings = settings.clone();
        }

        let inner = Arc::clone(&self.inner);

        let task = tokio::spawn(async move {
            relay(session, vehicle, settings, inner).await;
        });

        let Ok(mut inner) = self.inner.lock() else {
            return Ok(());
        };

        inner.running = true;
        inner.task = Some(task);

        Ok(())
    }

    /// Stops sending the aircraft onto the link.
    pub fn stop(&self) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };

        inner.running = false;

        if let Some(task) = inner.task.take() {
            task.abort();
        }
    }
}

/// Says whether the settings are ones that can be used.
///
/// MPCraft reads the traffic from the HTTP API. Upstream also opens a plain TCP connection
/// to a receiver — that form is not ported, so an address that is not a URL is refused
/// rather than guessed at. A distance of nothing is refused for the same reason: the list is
/// asked around the vehicle, and a circle of nothing holds no aircraft.
pub fn check_settings(settings: &AdsbRelaySettings) -> Result<(), RelayError> {
    if !settings.server.starts_with("http://") && !settings.server.starts_with("https://") {
        return Err(RelayError::BadServer(settings.server.clone()));
    }

    if settings.radius_nm == 0 {
        return Err(RelayError::BadRadius(settings.radius_nm));
    }

    Ok(())
}

/// The ten closest aircraft to the vehicle, in the order the relay walks through them.
///
/// An aircraft that is further away than the radius is not around the vehicle, and an
/// aircraft that is not in the ten closest is not what a station with a hundred aircraft
/// should be asked to say every second.
#[must_use]
pub fn closest(aircraft: &[AdsbVehicle], from: GeoPoint) -> Vec<AdsbVehicle> {
    let mut near: Vec<(f64, AdsbVehicle)> = aircraft
        .iter()
        .map(|plane| {
            (
                mpcraft_geo::distance_m(
                    mpcraft_geo::GeoPoint::new(from.lat, from.lng),
                    mpcraft_geo::GeoPoint::new(plane.latitude, plane.longitude),
                ),
                plane.clone(),
            )
        })
        .filter(|(distance, _)| *distance <= RELAY_RADIUS_M)
        .collect();

    // The order is by distance, and two aircraft at the same distance are put in the order
    // of their addresses so that the list does not change between two runs.
    near.sort_by(|(left_distance, left), (right_distance, right)| {
        left_distance
            .partial_cmp(right_distance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(left.icao_address.cmp(&right.icao_address))
    });

    near.truncate(RELAY_TOP);

    near.into_iter().map(|(_, plane)| plane).collect()
}

/// Reads the list of aircraft the API answers with.
///
/// The document is the one `api.adsb.lol` publishes: an array called `ac`, one object per
/// aircraft, with the address in hexadecimal, the position, the height in feet, the way and
/// the speed, and the code the aircraft is squawking. A field that is not there is not an
/// error — a receiver that heard only part of what an aircraft says is still worth drawing.
///
/// # Errors
///
/// Returns an error when the body is not a JSON document or when it does not hold the list.
pub fn parse_aircraft_list(body: &str) -> CommandResult<Vec<AdsbVehicle>> {
    let document: Value = serde_json::from_str(body).map_err(|error| {
        CommandError::internal(format!("the answer is not a JSON document: {error}"))
    })?;

    let entries = document
        .get("ac")
        .and_then(Value::as_array)
        .ok_or_else(|| CommandError::internal("the answer does not hold the list of aircraft"))?;

    let mut found: Vec<AdsbVehicle> = Vec::with_capacity(entries.len());

    for entry in entries {
        let Some(latitude) = number(entry, "lat") else {
            continue;
        };

        let Some(longitude) = number(entry, "lon") else {
            continue;
        };

        // An aircraft with no position is not traffic: it is a receiver that heard a message
        // it could not read.
        if latitude == 0.0 || longitude == 0.0 {
            continue;
        }

        found.push(AdsbVehicle {
            icao_address: hex(entry, "hex").unwrap_or(0),
            callsign: text(entry, "flight").or_else(|| text(entry, "callSign")),
            squawk: hex(entry, "squawk").unwrap_or(0) as u16,
            latitude,
            longitude,
            // The height arrives in feet and is said in metres.
            altitude: number(entry, "alt_baro")
                .map(|feet| feet * FEET_TO_METRES)
                .unwrap_or(0.0),
            heading: number(entry, "track").unwrap_or(0.0),
            // The speed arrives in knots and is said in metres per second.
            speed: number(entry, "gs")
                .map(|knots| knots * KNOTS_TO_METRES_PER_SECOND)
                .unwrap_or(0.0),
            // The climb arrives in feet per minute and is said in metres per second.
            vertical_speed: number(entry, "baro_rate")
                .map(|rate| rate * FEET_PER_MINUTE_TO_METRES_PER_SECOND)
                .unwrap_or(0.0),
            source: AdsbSource::Station,
        });
    }

    found.sort_by_key(|plane| plane.icao_address);

    Ok(found)
}

/// Feet to metres, the way upstream converts them.
const FEET_TO_METRES: f64 = 1.0 / 3.28;

/// Knots to metres per second.
const KNOTS_TO_METRES_PER_SECOND: f64 = 0.514_444;

/// Feet per minute to metres per second.
const FEET_PER_MINUTE_TO_METRES_PER_SECOND: f64 = 1.0 / 196.8;

/// One field of an aircraft, read as a number when it is one.
///
/// A height that is written as `GND` — an aircraft on the ground — is not a number, and the
/// reader does not make one up for it.
fn number(entry: &Value, key: &str) -> Option<f64> {
    let value = entry.get(key)?;

    if let Some(number) = value.as_f64() {
        return Some(number);
    }

    value.as_str().and_then(|text| text.parse::<f64>().ok())
}

/// One field of an aircraft, read as text.
fn text(entry: &Value, key: &str) -> Option<String> {
    let value = entry.get(key)?.as_str()?;

    let name = value.trim().to_owned();

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// One field of an aircraft, read as a hexadecimal number.
fn hex(entry: &Value, key: &str) -> Option<u32> {
    let value = entry.get(key)?;

    match value {
        Value::Number(number) => number.as_u64().map(|number| number as u32),
        Value::String(text) => u32::from_str_radix(text.trim(), 16).ok(),
        _ => None,
    }
}

/// Reads the source and sends one aircraft each second, in the order of the ones that are close.
///
/// The vehicle the message is attributed to is the one the station is talking to: the relay
/// says what it read, not what the vehicle heard.
async fn relay(
    session: Arc<Session>,
    _vehicle: VehicleId,
    settings: AdsbRelaySettings,
    inner: Arc<std::sync::Mutex<Inner>>,
) {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let mut last_read: Option<Instant> = None;

    loop {
        tokio::time::sleep(SEND_INTERVAL).await;

        // The position is read out of the state before anything is waited for: the lock is
        // not held across an await. The settings do not change while the task runs.
        let position = match inner.lock() {
            Ok(inner) if inner.running => inner.position,
            _ => return,
        };

        let Some(position) = position else {
            // The vehicle has not said where it is, and the list is asked around it.
            continue;
        };

        let now = Instant::now();

        let due = match last_read {
            Some(when) => now.saturating_duration_since(when) >= FETCH_INTERVAL,
            None => true,
        };

        if due {
            last_read = Some(now);
            read_source(&client, &settings, position, &inner).await;
        }

        let near = inner
            .lock()
            .map(|inner| {
                closest(
                    &inner
                        .registry
                        .heard_from(AdsbSource::Station, RELAY_STALE, Instant::now()),
                    position,
                )
            })
            .unwrap_or_default();

        if near.is_empty() {
            continue;
        }

        // The list is walked through one place at a time, so that every aircraft close to
        // the vehicle gets sent in turn rather than the same one being sent again.
        // The round goes through at most the ten closest, which is what upstream takes.
        let round = near.len().clamp(1, RELAY_TOP);

        let index = (inner.lock().map(|inner| inner.index).unwrap_or(0) + 1) % round;

        let Some(plane) = near.get(index) else {
            continue;
        };

        if let Err(error) = session.send_adsb(plane).await {
            record(&inner, error.to_string());
            return;
        }

        if let Ok(mut inner) = inner.lock() {
            inner.index = index;
            inner.sent += 1;
        }
    }
}

/// Asks the source for the aircraft around the place the vehicle is and keeps them.
///
/// A source that does not answer is kept as the failure the interface shows: the relay is a
/// service, and a service does not stop when one of its reads fails.
async fn read_source(
    client: &reqwest::Client,
    settings: &AdsbRelaySettings,
    position: GeoPoint,
    inner: &Arc<std::sync::Mutex<Inner>>,
) {
    let url = format!(
        "{}/v2/point/{:.5}/{:.5}/{radius}",
        settings.server.trim_end_matches('/'),
        position.lat,
        position.lng,
        radius = settings.radius_nm
    );

    match client.get(url).send().await {
        Ok(response) if response.status().is_success() => match response.text().await {
            Ok(body) => match parse_aircraft_list(&body) {
                Ok(aircraft) => {
                    let now = Instant::now();

                    if let Ok(mut inner) = inner.lock() {
                        for plane in &aircraft {
                            inner.registry.observe(plane.clone(), now);
                        }
                    }
                }
                Err(error) => record(inner, error.message),
            },
            Err(error) => record(inner, error.to_string()),
        },
        Ok(response) => record(
            inner,
            format!(
                "the source answered {code}",
                code = response.status().as_u16()
            ),
        ),
        Err(error) => record(inner, error.to_string()),
    }
}

/// Keeps a failure so that the interface can say it.
fn record(inner: &Arc<std::sync::Mutex<Inner>>, message: String) {
    tracing::warn!("the relay of the traffic: {message}");

    if let Ok(mut inner) = inner.lock() {
        inner.error = Some(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane(icao_address: u32, latitude: f64, longitude: f64) -> AdsbVehicle {
        AdsbVehicle {
            icao_address,
            callsign: None,
            squawk: 0,
            latitude,
            longitude,
            altitude: 0.0,
            heading: 0.0,
            speed: 0.0,
            vertical_speed: 0.0,
            source: AdsbSource::Station,
        }
    }

    #[test]
    fn the_list_of_aircraft_is_read_with_the_units_it_arrives_in() {
        let body = r#"{
            "now": 1700000000,
            "ac": [
                {
                    "hex": "A1B2C3",
                    "flight": "QFA456",
                    "lat": -33.433638,
                    "lon": 149.130000,
                    "alt_baro": 35000,
                    "track": 123.4,
                    "gs": 450,
                    "baro_rate": 1200,
                    "squawk": "4501"
                }
            ]
        }"#;

        let aircraft = parse_aircraft_list(body).expect("the document should be read");

        assert_eq!(aircraft.len(), 1);
        assert_eq!(aircraft[0].icao_address, 0x00A1_B2C3);
        assert_eq!(aircraft[0].callsign.as_deref(), Some("QFA456"));
        assert_eq!(aircraft[0].squawk, 0x4501);
        assert_eq!(aircraft[0].latitude, -33.433638);
        // Thirty-five thousand feet is a little under eleven thousand metres.
        assert!((aircraft[0].altitude - 10_670.7) < 0.5);
        // Four hundred and fifty knots is a little under two hundred and thirty-two metres
        // per second.
        assert!((aircraft[0].speed - 231.5) < 0.5);
        assert_eq!(aircraft[0].source, AdsbSource::Station);
    }

    #[test]
    fn an_aircraft_with_no_position_is_not_traffic() {
        let body =
            r#"{"ac":[{"hex":"A1B2C3","lat":0,"lon":0},{"hex":"A1B2C4","lat":-33.4,"lon":149.1}]}"#;

        let aircraft = parse_aircraft_list(body).expect("the document should be read");

        assert_eq!(aircraft.len(), 1);
        assert_eq!(aircraft[0].icao_address, 0x00A1_B2C4);
    }

    #[test]
    fn an_answer_that_is_not_the_document_expected_is_an_error() {
        let error = parse_aircraft_list("<html>rate limited</html>")
            .expect_err("the answer is not a document");

        assert_eq!(error.code, CommandError::CODE_INTERNAL);
    }

    #[test]
    fn the_relay_keeps_only_the_aircraft_that_are_close() {
        let here = GeoPoint {
            lat: -33.8568,
            lng: 151.2153,
            alt: 0.0,
        };

        let near = plane(0x00A1_B2C3, -33.86, 151.22);
        let far = plane(0x00A1_B2C4, -33.95, 151.22);

        let chosen = closest(&[far, near], here);

        assert_eq!(chosen.len(), 1);
        assert_eq!(chosen[0].icao_address, 0x00A1_B2C3);
    }

    #[test]
    fn the_relay_takes_the_ten_closest_in_the_order_of_distance() {
        let here = GeoPoint {
            lat: -33.8568,
            lng: 151.2153,
            alt: 0.0,
        };

        // Twelve aircraft, all close, each one a little further away than the one before.
        let around: Vec<AdsbVehicle> = (0..12)
            .map(|index| {
                plane(
                    0x1000 + index,
                    -33.8568 + f64::from(index) * 0.002,
                    151.2153,
                )
            })
            .collect();

        let chosen = closest(&around, here);

        assert_eq!(chosen.len(), RELAY_TOP);
        assert_eq!(chosen[0].icao_address, 0x1000);
        assert_eq!(chosen[RELAY_TOP - 1].icao_address, 0x1009);
    }

    #[test]
    fn a_source_that_is_not_an_url_cannot_be_used() {
        let settings = AdsbRelaySettings {
            server: "127.0.0.1:30003".to_owned(),
            radius_nm: DEFAULT_RADIUS_NM,
        };

        let error = check_settings(&settings).expect_err("the address is not one that can be used");

        assert_eq!(error.code(), CommandError::CODE_INVALID_REQUEST);
    }

    #[test]
    fn a_distance_of_nothing_cannot_be_used() {
        let settings = AdsbRelaySettings {
            server: "https://api.adsb.lol".to_owned(),
            radius_nm: 0,
        };

        let error =
            check_settings(&settings).expect_err("the distance is not one that can be used");

        assert_eq!(error.code(), CommandError::CODE_INVALID_REQUEST);
    }
}
