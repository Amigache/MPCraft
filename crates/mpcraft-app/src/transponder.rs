//! The transponder of the vehicle.
//!
//! A vehicle that is seen by the traffic of the air needs to say where it is: the
//! transponder is the thing that does it, and the ground station is what tells the
//! vehicle what to say. The vehicle relays the message, and the aircraft around it see
//! it.
//!
//! Upstream has the same thing (`FlightData.cs`, the transponder tab), and the same
//! message.

use std::sync::Arc;
use std::time::Duration;

use mpcraft_link::{Session, Transponder, VehicleId, TRANSPONDER_INTERVAL};
use mpcraft_vehicle::GeoPoint;
use serde::{Deserialize, Serialize};

/// What the transponder says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransponderSettings {
    /// The address of the aircraft.
    pub icao_address: u32,
    /// The name of the flight, the one the aircraft says.
    pub flight_id: String,
    /// The code the operator chose, in octal.
    pub squawk: u16,
}

/// What the transponder is doing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransponderStatus {
    /// Whether the transponder is being told what to say.
    pub running: bool,
    /// How many times the vehicle has been told.
    pub sent: u64,
    /// The last failure, when there was one.
    pub error: Option<String>,
    /// What the transponder says.
    pub settings: TransponderSettings,
}

/// The transponder of the vehicle.
#[derive(Clone, Default)]
pub struct TransponderService {
    inner: Arc<std::sync::Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    running: bool,
    sent: u64,
    error: Option<String>,
    settings: TransponderSettings,
    position: Option<GeoPoint>,
    heading: f64,
    speed: f64,
    vertical_speed: f64,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Default for TransponderSettings {
    fn default() -> Self {
        Self {
            icao_address: 0,
            flight_id: String::new(),
            squawk: 1200,
        }
    }
}

impl TransponderService {
    /// What the transponder is doing.
    #[must_use]
    pub fn status(&self) -> TransponderStatus {
        let Ok(inner) = self.inner.lock() else {
            return TransponderStatus {
                running: false,
                sent: 0,
                error: Some("the state of the transponder is locked".to_owned()),
                settings: TransponderSettings::default(),
            };
        };

        TransponderStatus {
            running: inner.running,
            sent: inner.sent,
            error: inner.error.clone(),
            settings: inner.settings.clone(),
        }
    }

    /// Says where the vehicle is, and how it is going.
    ///
    /// The position is what the transponder says, so it has to follow the vehicle.
    pub fn set_vehicle(&self, position: GeoPoint, heading: f64, speed: f64, vertical_speed: f64) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.position = Some(position);
            inner.heading = heading;
            inner.speed = speed;
            inner.vertical_speed = vertical_speed;
        }
    }

    /// Starts telling the vehicle what its transponder is to say.
    ///
    /// # Errors
    ///
    /// Returns an error when the transponder is already being told.
    pub fn start(
        &self,
        session: Arc<Session>,
        vehicle: VehicleId,
        settings: TransponderSettings,
    ) -> Result<(), String> {
        {
            let Ok(mut inner) = self.inner.lock() else {
                return Err("the state of the transponder is locked".to_owned());
            };

            if inner.running {
                return Err("the transponder is already being told".to_owned());
            }

            inner.error = None;
            inner.sent = 0;
            inner.settings = settings.clone();
        }

        let inner = Arc::clone(&self.inner);

        let task = tokio::spawn(async move {
            tell(session, vehicle, settings, inner).await;
        });

        let Ok(mut inner) = self.inner.lock() else {
            return Ok(());
        };

        inner.running = true;
        inner.task = Some(task);

        Ok(())
    }

    /// Stops telling the vehicle what its transponder is to say.
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

/// Tells the vehicle what its transponder is to say, once a second.
async fn tell(
    session: Arc<Session>,
    _vehicle: VehicleId,
    settings: TransponderSettings,
    inner: Arc<std::sync::Mutex<Inner>>,
) {
    loop {
        tokio::time::sleep(TRANSPONDER_INTERVAL).await;

        let position = inner.lock().ok().and_then(|inner| inner.position);

        let Some(position) = position else {
            // The vehicle has not said where it is: there is nothing to say.
            continue;
        };

        let (heading, speed, vertical_speed) = inner
            .lock()
            .ok()
            .map(|inner| (inner.heading, inner.speed, inner.vertical_speed))
            .unwrap_or((0.0, 0.0, 0.0));

        let transponder = Transponder {
            icao_address: settings.icao_address,
            flight_id: settings.flight_id.clone(),
            squawk: settings.squawk,
            latitude: position.lat,
            longitude: position.lng,
            altitude: position.alt,
            heading,
            speed,
            vertical_speed,
        };

        if let Err(error) = session.send_transponder(&transponder).await {
            if let Ok(mut inner) = inner.lock() {
                inner.running = false;
                inner.error = Some(error.to_string());
            }
            return;
        }

        if let Ok(mut inner) = inner.lock() {
            inner.sent += 1;
        }
    }
}

/// How long the operator waits between two tells.
pub const TELL_INTERVAL: Duration = TRANSPONDER_INTERVAL;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_transponder_says_nothing_when_nothing_is_chosen() {
        let settings = TransponderSettings::default();

        assert_eq!(settings.squawk, 1200);
        assert!(settings.flight_id.is_empty());
    }
}
