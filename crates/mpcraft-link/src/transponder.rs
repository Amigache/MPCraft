//! The transponder of the vehicle.
//!
//! A vehicle that is seen by the traffic of the air needs to say where it is: the
//! transponder is the thing that does it, and the ground station is what tells the
//! vehicle what to say. The vehicle relays the message, and the aircraft around it see
//! it.
//!
//! Upstream has the same thing (`FlightData.cs`, the transponder tab), and it sends the
//! same message: the position of the vehicle, the squawk the operator chose, and the
//! name of the flight.

use std::time::Duration;

use mpcraft_mavlink::dialect::messages::ADSB_VEHICLE_DATA;
use mpcraft_mavlink::dialect::{AdsbAltitudeType, AdsbEmitterType, AdsbFlags, MavMessage};
use mpcraft_mavlink::CharArray;

use crate::error::LinkError;
use crate::session::Session;

/// How often the transponder is told where the vehicle is.
pub const TRANSPONDER_INTERVAL: Duration = Duration::from_secs(1);

/// What the transponder says.
#[derive(Debug, Clone, PartialEq)]
pub struct Transponder {
    /// The address of the aircraft.
    pub icao_address: u32,
    /// The name of the flight, the one the aircraft says.
    pub flight_id: String,
    /// The code the operator chose, in octal.
    pub squawk: u16,
    /// Where the vehicle is.
    pub latitude: f64,
    /// Where the vehicle is.
    pub longitude: f64,
    /// How high the vehicle is, in metres.
    pub altitude: f64,
    /// Which way the vehicle is going, in degrees.
    pub heading: f64,
    /// How fast the vehicle is going, in metres per second.
    pub speed: f64,
    /// How fast the vehicle is going up or down, in metres per second.
    pub vertical_speed: f64,
}

impl Transponder {
    /// The message the vehicle is told to send.
    #[must_use]
    pub fn message(&self) -> ADSB_VEHICLE_DATA {
        let mut flags = AdsbFlags::ADSB_FLAGS_VALID_COORDS
            | AdsbFlags::ADSB_FLAGS_VALID_ALTITUDE
            | AdsbFlags::ADSB_FLAGS_VALID_HEADING
            | AdsbFlags::ADSB_FLAGS_VALID_VELOCITY
            | AdsbFlags::ADSB_FLAGS_VALID_SQUAWK;

        if !self.flight_id.is_empty() {
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
            callsign: char_array_of(&self.flight_id),
            emitter_type: AdsbEmitterType::ADSB_EMITTER_TYPE_NO_INFO,
            tslc: 0,
        }
    }
}

/// The name of the flight, as the message carries it.
fn char_array_of(name: &str) -> CharArray<9> {
    let mut array = CharArray::from([0_u8; 9]);

    for (index, byte) in name.as_bytes().iter().take(9).enumerate() {
        array[index] = *byte;
    }

    array
}

impl Session {
    /// Tells the vehicle what its transponder is to say.
    ///
    /// The vehicle relays the message, and the aircraft around it see it.
    ///
    /// # Errors
    ///
    /// Returns an error when the link is closed or the write fails.
    pub async fn send_transponder(&self, transponder: &Transponder) -> Result<(), LinkError> {
        self.write_message(&MavMessage::ADSB_VEHICLE(transponder.message()))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_carries_what_the_operator_chose() {
        let transponder = Transponder {
            icao_address: 0x00AB_C123,
            flight_id: "MPCRAFT".to_owned(),
            squawk: 1200,
            latitude: -35.2809,
            longitude: 149.1300,
            altitude: 600.0,
            heading: 45.0,
            speed: 12.0,
            vertical_speed: 0.5,
        };

        let message = transponder.message();

        assert_eq!(message.ICAO_address, 0x00AB_C123);
        assert_eq!(message.squawk, 1200);
        assert_eq!(message.lat, (-35.2809 * 1e7) as i32);
        assert_eq!(message.altitude, 600_000);
        assert_eq!(message.heading, 4500);
        assert_eq!(message.hor_velocity, 1200);
        assert!(message.flags.contains(AdsbFlags::ADSB_FLAGS_VALID_COORDS));
        assert!(message.flags.contains(AdsbFlags::ADSB_FLAGS_VALID_CALLSIGN));
    }

    #[test]
    fn the_name_of_the_flight_is_cut_when_it_is_too_long() {
        let array = char_array_of("A_NAME_THAT_IS_TOO_LONG");

        let name: String = array
            .iter()
            .copied()
            .take_while(|byte| *byte != 0)
            .map(char::from)
            .collect();

        assert_eq!(name, "A_NAME_TH");
    }
}
