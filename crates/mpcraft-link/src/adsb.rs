//! The traffic that the vehicle can see.
//!
//! A vehicle with a receiver for the traffic of the air sends what it hears as
//! `ADSB_VEHICLE` messages: where each aircraft is, how high it is, which way it is
//! going and how fast. The ground station draws them on the map, so that the operator
//! sees the traffic around the vehicle.
//!
//! Upstream has the same thing (`ExtLibs/Utilities/adsb.cs`), and it reads it either
//! from the vehicle or from a receiver of its own. The two are kept apart because they
//! are not the same: what the vehicle heard came over the link already, and what the
//! station read from a source of its own is what has to go *onto* the link, so that the
//! other stations hear it. That is the relay, and it cannot tell the two apart unless the
//! traffic keeps where it came from.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use mpcraft_mavlink::dialect::messages::ADSB_VEHICLE_DATA;
use mpcraft_mavlink::dialect::{AdsbAltitudeType, AdsbEmitterType, AdsbFlags, MavMessage};

use crate::error::LinkError;
use crate::session::Session;
use crate::transponder::char_array_of;

/// Where an aircraft of the traffic was heard from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdsbSource {
    /// The vehicle sent it, from the receiver it has.
    Vehicle,
    /// The ground station read it from a source of its own — an API or a receiver.
    Station,
}

/// One aircraft of the traffic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdsbVehicle {
    /// The address the aircraft has.
    pub icao_address: u32,
    /// The name it says, when it says one.
    pub callsign: Option<String>,
    /// The code it is squawking, when it says one.
    pub squawk: u16,
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
    /// Where it was heard from.
    pub source: AdsbSource,
}

impl AdsbVehicle {
    /// The message that carries an aircraft of the traffic.
    ///
    /// Everything the aircraft said is marked as valid, because the relay only sends what
    /// the source gave: an aircraft without a position is not in the list at all.
    #[must_use]
    pub fn message(&self) -> ADSB_VEHICLE_DATA {
        let mut flags = AdsbFlags::ADSB_FLAGS_VALID_COORDS
            | AdsbFlags::ADSB_FLAGS_VALID_ALTITUDE
            | AdsbFlags::ADSB_FLAGS_VALID_HEADING
            | AdsbFlags::ADSB_FLAGS_VALID_VELOCITY
            | AdsbFlags::ADSB_FLAGS_VALID_SQUAWK;

        if self.callsign.is_some() {
            flags |= AdsbFlags::ADSB_FLAGS_VALID_CALLSIGN;
        }

        ADSB_VEHICLE_DATA {
            ICAO_address: self.icao_address,
            lat: (self.latitude * 1e7) as i32,
            lon: (self.longitude * 1e7) as i32,
            // The height is in millimetres.
            altitude: (self.altitude * 1000.0) as i32,
            // The way is in hundredths of a degree.
            heading: (self.heading * 100.0) as u16,
            // The speed is in centimetres per second.
            hor_velocity: (self.speed * 100.0) as u16,
            ver_velocity: (self.vertical_speed * 100.0) as i16,
            flags,
            squawk: self.squawk,
            // The height is the one over the sea.
            altitude_type: AdsbAltitudeType::ADSB_ALTITUDE_TYPE_GEOMETRIC,
            callsign: char_array_of(self.callsign.as_deref().unwrap_or("")),
            emitter_type: AdsbEmitterType::ADSB_EMITTER_TYPE_NO_INFO,
            tslc: 0,
        }
    }
}

impl Session {
    /// Sends one aircraft of the traffic onto the link.
    ///
    /// The link carries the message to whoever is on the other end of it, which is what a
    /// relay is: the station heard the aircraft somewhere else, and the stations that are
    /// listening to this link hear it now.
    ///
    /// # Errors
    ///
    /// Returns an error when the link is closed or the write fails.
    pub async fn send_adsb(&self, aircraft: &AdsbVehicle) -> Result<(), LinkError> {
        self.write_message(&MavMessage::ADSB_VEHICLE(aircraft.message()))
            .await
    }
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

    /// The traffic that was heard from one source, and is not too old to be believed.
    ///
    /// The relay works on the aircraft it read from its own source: what came over the
    /// link is already on the link, and sending it back would say the same thing twice.
    #[must_use]
    pub fn heard_from(
        &self,
        source: AdsbSource,
        within: Duration,
        now: Instant,
    ) -> Vec<AdsbVehicle> {
        let mut found: Vec<AdsbVehicle> = self
            .aircraft
            .iter()
            .filter(|(_, (vehicle, heard))| {
                vehicle.source == source && within > now.saturating_duration_since(*heard)
            })
            .map(|(_, (vehicle, _))| vehicle.clone())
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
            squawk: 1200,
            latitude: -35.28,
            longitude: 149.13,
            altitude: 600.0,
            heading: 90.0,
            speed: 30.0,
            vertical_speed: 0.0,
            source: AdsbSource::Vehicle,
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
    fn the_relay_only_takes_what_the_station_read() {
        let mut registry = AdsbRegistry::new();
        let now = Instant::now();

        let mut from_vehicle = aircraft(0x1234);
        from_vehicle.source = AdsbSource::Vehicle;

        let mut from_station = aircraft(0x5678);
        from_station.source = AdsbSource::Station;

        registry.observe(from_vehicle, now);
        registry.observe(from_station, now);

        let heard = registry.heard_from(AdsbSource::Station, Duration::from_secs(30), now);

        assert_eq!(heard.len(), 1);
        assert_eq!(heard[0].icao_address, 0x5678);
    }

    #[test]
    fn an_aircraft_that_was_heard_too_long_ago_is_not_taken() {
        let mut registry = AdsbRegistry::new();
        let now = Instant::now();

        let mut stale = aircraft(0x5678);
        stale.source = AdsbSource::Station;

        let mut fresh = aircraft(0x1234);
        fresh.source = AdsbSource::Station;

        // Half a minute is what the relay believes: the stale one was heard a minute ago,
        // and the aircraft that is not heard any more is not around.
        registry.observe(stale, now);
        registry.observe(fresh, now + Duration::from_secs(60));

        let heard = registry.heard_from(
            AdsbSource::Station,
            Duration::from_secs(30),
            now + Duration::from_secs(60),
        );

        assert_eq!(heard.len(), 1);
        assert_eq!(heard[0].icao_address, 0x1234);
    }

    #[test]
    fn the_message_carries_the_aircraft_as_the_relay_sends_it() {
        let mut plane = aircraft(0x00AB_C123);
        plane.callsign = Some("BIGFOOT".to_owned());
        plane.squawk = 4501;

        let message = plane.message();

        assert_eq!(message.ICAO_address, 0x00AB_C123);
        assert_eq!(message.squawk, 4501);
        assert_eq!(message.lat, (-35.28 * 1e7) as i32);
        assert_eq!(message.altitude, 600_000);
        assert_eq!(message.heading, 9000);
        assert_eq!(message.hor_velocity, 3000);
        assert!(message.flags.contains(AdsbFlags::ADSB_FLAGS_VALID_COORDS));
        assert!(message.flags.contains(AdsbFlags::ADSB_FLAGS_VALID_CALLSIGN));
    }

    #[test]
    fn the_name_of_an_aircraft_is_read_from_its_bytes() {
        assert_eq!(callsign_of(b"ABC123\0\0"), Some("ABC123".to_owned()));
        assert_eq!(callsign_of(b"\0\0\0"), None);
        assert_eq!(callsign_of(b"  \0"), None);
    }
}
