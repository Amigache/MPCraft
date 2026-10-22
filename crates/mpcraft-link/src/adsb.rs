//! The traffic that the vehicle can see.
//!
//! A vehicle with a receiver for the traffic of the air sends what it hears as
//! `ADSB_VEHICLE` messages: where each aircraft is, how high it is, which way it is
//! going and how fast. The ground station draws them on the map, so that the operator
//! sees the traffic around the vehicle.
//!
//! Upstream has the same thing (`ExtLibs/Utilities/adsb.cs`), and it reads it either
//! from the vehicle or from a receiver of its own.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// One aircraft of the traffic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdsbVehicle {
    /// The address the aircraft has.
    pub icao_address: u32,
    /// The name it says, when it says one.
    pub callsign: Option<String>,
    /// Where it is.
    pub latitude: f64,
    /// Where it is.
    pub longitude: f64,
    /// How high it is, in metres.
    pub altitude: f64,
    /// Which way it is going, in degrees.
    pub heading: f64,
    /// How fast it is going, in metres per second.
    pub speed: f64,
    /// How fast it is going up or down, in metres per second.
    pub vertical_speed: f64,
}

/// The traffic that has been heard, by the address of the aircraft.
///
/// An aircraft that is not heard any more is forgotten: the list is of the traffic
/// that is around, and the traffic that has gone away is not around.
#[derive(Debug, Clone)]
pub struct AdsbRegistry {
    /// The aircraft, by the address they have.
    aircraft: HashMap<u32, (AdsbVehicle, Instant)>,
    /// How long an aircraft is kept after it was last heard.
    timeout: Duration,
    /// How many aircraft are kept at most.
    capacity: usize,
}

impl Default for AdsbRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AdsbRegistry {
    /// A registry that keeps the traffic of the last five minutes.
    #[must_use]
    pub fn new() -> Self {
        Self {
            aircraft: HashMap::new(),
            timeout: Duration::from_secs(300),
            capacity: 64,
        }
    }

    /// Records an aircraft that has been heard.
    pub fn observe(&mut self, vehicle: AdsbVehicle, now: Instant) {
        self.aircraft.insert(vehicle.icao_address, (vehicle, now));
        self.forget(now);
    }

    /// Forgets the aircraft that have not been heard for a while.
    fn forget(&mut self, now: Instant) {
        self.aircraft
            .retain(|_, (_, heard)| now.saturating_duration_since(*heard) <= self.timeout);

        // The list is bounded: a receiver that hears a great many aircraft must not
        // grow the state without limit.
        while self.aircraft.len() > self.capacity {
            let oldest = self
                .aircraft
                .iter()
                .min_by_key(|(_, (_, heard))| *heard)
                .map(|(address, _)| *address);

            let Some(oldest) = oldest else {
                break;
            };

            self.aircraft.remove(&oldest);
        }
    }

    /// The traffic that is around.
    #[must_use]
    pub fn vehicles(&self) -> Vec<AdsbVehicle> {
        let mut found: Vec<AdsbVehicle> = self
            .aircraft
            .values()
            .map(|(vehicle, _)| vehicle.clone())
            .collect();

        found.sort_by_key(|aircraft| aircraft.icao_address);

        found
    }

    /// How many aircraft are around.
    #[must_use]
    pub fn len(&self) -> usize {
        self.aircraft.len()
    }

    /// Whether there is no traffic around.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.aircraft.is_empty()
    }
}

/// The name of an aircraft, from the bytes the message carries.
#[must_use]
pub fn callsign_of(bytes: &[u8]) -> Option<String> {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());

    let name: String = bytes[..end]
        .iter()
        .map(|byte| {
            if byte.is_ascii_graphic() || *byte == b' ' {
                *byte as char
            } else {
                ' '
            }
        })
        .collect();

    let name = name.trim().to_owned();

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aircraft(icao_address: u32) -> AdsbVehicle {
        AdsbVehicle {
            icao_address,
            callsign: None,
            latitude: -35.28,
            longitude: 149.13,
            altitude: 600.0,
            heading: 90.0,
            speed: 30.0,
            vertical_speed: 0.0,
        }
    }

    #[test]
    fn an_aircraft_that_is_heard_is_around() {
        let mut registry = AdsbRegistry::new();
        registry.observe(aircraft(0x1234), Instant::now());

        assert_eq!(registry.len(), 1);
        assert_eq!(registry.vehicles()[0].icao_address, 0x1234);
    }

    #[test]
    fn an_aircraft_that_is_heard_again_is_the_same_one() {
        let mut registry = AdsbRegistry::new();
        registry.observe(aircraft(0x1234), Instant::now());

        let mut later = aircraft(0x1234);
        later.altitude = 900.0;
        registry.observe(later, Instant::now());

        assert_eq!(registry.len(), 1);
        assert_eq!(registry.vehicles()[0].altitude, 900.0);
    }

    #[test]
    fn an_aircraft_that_is_not_heard_any_more_is_forgotten() {
        let mut registry = AdsbRegistry::new();
        registry.observe(aircraft(0x1234), Instant::now());

        // Six minutes later the aircraft is not heard, and it is forgotten.
        registry.observe(aircraft(0x5678), Instant::now() + Duration::from_secs(360));

        assert_eq!(registry.len(), 1);
        assert_eq!(registry.vehicles()[0].icao_address, 0x5678);
    }

    #[test]
    fn the_name_of_an_aircraft_is_read_from_its_bytes() {
        assert_eq!(callsign_of(b"ABC123\0\0"), Some("ABC123".to_owned()));
        assert_eq!(callsign_of(b"\0\0\0"), None);
        assert_eq!(callsign_of(b"  \0"), None);
    }
}
