//! The home position of a vehicle.
//!
//! The home is the place the vehicle returns to. It is a position on the ground — a
//! latitude, a longitude and an altitude over the sea — and the protocol carries it in
//! two messages:
//!
//! | Message | Who sends it | What it is |
//! | --- | --- | --- |
//! | `HOME_POSITION` (242) | the vehicle | the home the vehicle holds |
//! | `SET_HOME_POSITION` (243) | the ground station | the home the vehicle is to hold |
//!
//! # The units
//!
//! The coordinates travel in units of a hundred millionth of a degree and the altitude
//! in millimetres. The conversion happens once, at the edge of the protocol, and
//! everything above it works in degrees and metres.
//!
//! # Reading the home
//!
//! The vehicle reports its home whenever the home is moved, and on request. The
//! request is a command, and the answer is the message: the vehicle answers
//! `MAV_CMD_GET_HOME_POSITION` with a `HOME_POSITION`, and
//! `MAV_CMD_REQUEST_MESSAGE` with the id of the message is the newer way of asking for
//! the same thing.
//!
//! Upstream sends the first of the two (`MAVLinkInterface.cs:3357`) and waits seven
//! hundred milliseconds for the answer, three times over.
//!
//! # Writing the home
//!
//! The home is moved with `SET_HOME_POSITION`, and the vehicle confirms it by
//! reporting the home it now holds. The command `MAV_CMD_DO_SET_HOME` is the other
//! way of doing it: the position travels with the command and the vehicle answers
//! with an acknowledgement, which says whether the home was moved at all. Upstream
//! uses the command (`FlightData.cs:4986`).

use std::time::Duration;

// `SET_HOME_POSITION` is deprecated in the specification in favour of the command
// `MAV_CMD_DO_SET_HOME`, which the vehicle answers with an acknowledgement. The
// message is still what the protocol defines for moving the home, and the vehicles
// that answer it report the home they now hold.
#[allow(deprecated)]
use mpcraft_mavlink::dialect::messages::{COMMAND_LONG_DATA, SET_HOME_POSITION_DATA};
use mpcraft_mavlink::{MavCmd, MavMessage, MavResult};
use mpcraft_vehicle::GeoPoint;

use crate::error::LinkError;
use crate::session::{ReceivedMessage, Session};
use crate::vehicle::VehicleId;

/// The message id of `HOME_POSITION`.
pub const HOME_POSITION_ID: u32 = 242;

/// The message id of `SET_HOME_POSITION`.
pub const SET_HOME_POSITION_ID: u32 = 243;

/// How long to wait for the vehicle to report its home position.
///
/// Upstream waits seven hundred milliseconds for the answer (`MAVLinkInterface.cs:3368`).
pub const DEFAULT_HOME_TIMEOUT: Duration = Duration::from_millis(700);

/// How many times the request for the home position is repeated.
///
/// Upstream repeats it three times (`MAVLinkInterface.cs:3361`).
pub const DEFAULT_HOME_RETRIES: u32 = 3;

/// How far two positions may be apart and still be the same place.
///
/// The coordinates travel in units of a hundred millionth of a degree, which is about
/// a centimetre, and the altitude in millimetres. The vehicle keeps the home in
/// centimetres, so a millimetre of slack is not enough.
const POSITION_TOLERANCE: f64 = 1e-7;

/// How far two altitudes may be apart and still be the same height.
const ALTITUDE_TOLERANCE: f64 = 1e-2;

/// How the vehicle is asked for its home position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HomeRequest {
    /// The command `MAV_CMD_GET_HOME_POSITION`.
    ///
    /// This is what upstream Mission Planner sends (`MAVLinkInterface.cs:3357`).
    #[default]
    Command,
    /// The command `MAV_CMD_REQUEST_MESSAGE`, with the id of `HOME_POSITION` in the
    /// first parameter.
    ///
    /// ArduPilot is phasing `MAV_CMD_GET_HOME_POSITION` out — it is compiled out by
    /// default from ArduPilot 4.9 — and this is the replacement the documentation of
    /// the message names.
    MessageRequest,
}

/// The reasons a home position operation can fail.
#[derive(Debug, Clone, PartialEq)]
pub enum HomeError {
    /// The vehicle reported a home position that is not the one that was asked for.
    ///
    /// The vehicle refused the move, and reported the home it kept.
    UnexpectedPosition {
        /// The position that was asked for.
        requested: GeoPoint,
        /// The position the vehicle reported.
        reported: GeoPoint,
    },
}

impl std::fmt::Display for HomeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedPosition {
                requested,
                reported,
            } => write!(
                f,
                "the vehicle kept the home at {:.7}, {:.7} instead of {:.7}, {:.7}",
                reported.lat, reported.lng, requested.lat, requested.lng
            ),
        }
    }
}

impl std::error::Error for HomeError {}

/// The home position operations of a session.
impl Session {
    /// Asks the vehicle for its home position.
    ///
    /// The request is a command; the answer is the `HOME_POSITION` message the vehicle
    /// sends when the home is moved or when it is asked for. The acknowledgement of the
    /// command is of no use here, and it is ignored.
    ///
    /// The request is repeated when the vehicle does not answer: a lost command is the
    /// common failure, and the vehicle does not repeat it.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle does not answer in time.
    pub async fn request_home_position(
        &self,
        vehicle: VehicleId,
        how: HomeRequest,
        timeout: Duration,
    ) -> Result<GeoPoint, LinkError> {
        // The matcher is registered before the request is sent: a vehicle on a fast
        // link can answer before the request has even left the session.
        let mut request = self.begin_request(home_matcher(vehicle)).await?;

        self.write_message(&home_request_message(vehicle, how))
            .await?;

        let mut attempts = 0u32;

        loop {
            match request.wait_for(timeout).await {
                Ok(message) => {
                    return home_position_of(&message).ok_or(LinkError::Protocol(
                        mpcraft_mavlink::ProtocolError::Malformed(
                            "the home position carried no position",
                        ),
                    ));
                }
                Err(LinkError::Timeout(_)) if attempts < DEFAULT_HOME_RETRIES => {
                    attempts += 1;
                    self.write_message(&home_request_message(vehicle, how))
                        .await?;
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Sets the home position of the vehicle.
    ///
    /// The position travels in the `SET_HOME_POSITION` message, and the vehicle
    /// confirms it by reporting the home it now holds — ArduPilot sends a
    /// `HOME_POSITION` whenever the home is moved.
    ///
    /// A vehicle that refuses the move reports the home it kept, and the position that
    /// was asked for is not what the vehicle has: the error says so.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle does not report the new home in time, or when
    /// it reports a different one.
    pub async fn set_home_position(
        &self,
        vehicle: VehicleId,
        home: GeoPoint,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        // The matcher is registered before the message is sent: the vehicle may answer
        // before the message has even left the session.
        let mut request = self.begin_request(home_matcher(vehicle)).await?;

        #[allow(deprecated)]
        let message = MavMessage::SET_HOME_POSITION(set_home_message(vehicle, home));

        self.write_message(&message).await?;

        let mut attempts = 0u32;

        loop {
            match request.wait_for(timeout).await {
                Ok(message) => {
                    let reported = home_position_of(&message).ok_or(LinkError::Protocol(
                        mpcraft_mavlink::ProtocolError::Malformed(
                            "the home position carried no position",
                        ),
                    ))?;

                    if !is_same_position(reported, home) {
                        return Err(LinkError::Rejected(
                            HomeError::UnexpectedPosition {
                                requested: home,
                                reported,
                            }
                            .to_string(),
                        ));
                    }

                    return Ok(());
                }
                Err(LinkError::Timeout(_)) if attempts < DEFAULT_HOME_RETRIES => {
                    attempts += 1;
                    #[allow(deprecated)]
                    let message = MavMessage::SET_HOME_POSITION(set_home_message(vehicle, home));
                    self.write_message(&message).await?;
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Sets the home position of the vehicle with the command `MAV_CMD_DO_SET_HOME`.
    ///
    /// The position travels with the command, and the vehicle answers with an
    /// acknowledgement that says whether the home was moved. This is the way upstream
    /// does it (`FlightData.cs:4986`).
    ///
    /// The altitude is in metres and is rounded to the metre: the command carries it as
    /// a single precision number.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command or does not answer in
    /// time.
    pub async fn set_home_position_by_command(
        &self,
        vehicle: VehicleId,
        home: GeoPoint,
        timeout: Duration,
    ) -> Result<MavResult, LinkError> {
        self.command_int(
            vehicle,
            MavCmd::MAV_CMD_DO_SET_HOME,
            [
                // The first parameter says whether the current position is to be used
                // instead of the one that is given.
                0,
                0,
                0,
                0,
                scaled_coordinate(home.lat),
                scaled_coordinate(home.lng),
                home.alt.round() as i32,
            ],
            timeout,
        )
        .await
    }
}

/// The matcher of the home position a vehicle reports.
fn home_matcher(vehicle: VehicleId) -> impl Fn(&ReceivedMessage) -> bool + Send + 'static {
    move |message| {
        message.message_id == HOME_POSITION_ID
            && message.system_id == vehicle.system_id
            && message.component_id == vehicle.component_id
    }
}

/// The command that asks the vehicle for its home position.
///
/// `MAV_CMD_GET_HOME_POSITION` is deprecated in the specification in favour of
/// `MAV_CMD_REQUEST_MESSAGE`, but it is the command upstream Mission Planner sends and
/// the one every vehicle answers.
#[allow(deprecated)]
fn home_request_message(vehicle: VehicleId, how: HomeRequest) -> MavMessage {
    let (command, params) = match how {
        HomeRequest::Command => (MavCmd::MAV_CMD_GET_HOME_POSITION, [0.0; 7]),
        HomeRequest::MessageRequest => (
            MavCmd::MAV_CMD_REQUEST_MESSAGE,
            [
                // The id of the message that is asked for.
                HOME_POSITION_ID as f32,
                // The instance of it.
                0.0,
                0.0,
                0.0,
                // The system and the component that are asked.
                f32::from(vehicle.system_id),
                f32::from(vehicle.component_id),
                0.0,
            ],
        ),
    };

    MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
        param1: params[0],
        param2: params[1],
        param3: params[2],
        param4: params[3],
        param5: params[4],
        param6: params[5],
        param7: params[6],
        command,
        target_system: vehicle.system_id,
        target_component: vehicle.component_id,
        confirmation: 0,
    })
}

/// The message that moves the home of a vehicle.
///
/// The message is deprecated in the MAVLink specification — the command
/// `MAV_CMD_DO_SET_HOME` replaced it, because the command lets the vehicle say whether
/// the home was moved — but it is the message the protocol defines for it, and the
/// vehicles that answer it are the ones that report the new home.
#[allow(deprecated)]
fn set_home_message(vehicle: VehicleId, home: GeoPoint) -> SET_HOME_POSITION_DATA {
    SET_HOME_POSITION_DATA {
        latitude: scaled_coordinate(home.lat),
        longitude: scaled_coordinate(home.lng),
        altitude: scaled_altitude(home.alt),
        // The local frame, the orientation of the ground and the approach vector are
        // the vehicle's own: the ground station does not know them.
        x: 0.0,
        y: 0.0,
        z: 0.0,
        q: [0.0; 4],
        approach_x: 0.0,
        approach_y: 0.0,
        approach_z: 0.0,
        time_usec: 0,
        target_system: vehicle.system_id,
    }
}

/// The position a `HOME_POSITION` message carries.
fn home_position_of(message: &ReceivedMessage) -> Option<GeoPoint> {
    match &message.message {
        MavMessage::HOME_POSITION(home) => Some(GeoPoint {
            lat: f64::from(home.latitude) / 1e7,
            lng: f64::from(home.longitude) / 1e7,
            alt: f64::from(home.altitude) / 1e3,
        }),
        _ => None,
    }
}

/// A coordinate of a position, as the protocol carries it.
///
/// The protocol scales the coordinates by ten million, and the unit of the result is a
/// hundred millionth of a degree.
fn scaled_coordinate(value: f64) -> i32 {
    (value * 1e7).round() as i32
}

/// The altitude of a position, as the protocol carries it: millimetres.
fn scaled_altitude(value: f64) -> i32 {
    (value * 1e3).round() as i32
}

/// Whether two positions are the same place.
fn is_same_position(left: GeoPoint, right: GeoPoint) -> bool {
    (left.lat - right.lat).abs() <= POSITION_TOLERANCE
        && (left.lng - right.lng).abs() <= POSITION_TOLERANCE
        && (left.alt - right.alt).abs() <= ALTITUDE_TOLERANCE
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    use mpcraft_mavlink::dialect::messages::HOME_POSITION_DATA;

    /// Builds a received home position.
    fn received(lat: f64, lng: f64, alt: f64) -> ReceivedMessage {
        ReceivedMessage {
            system_id: 1,
            component_id: 1,
            message_id: HOME_POSITION_ID,
            message_name: "HOME_POSITION".to_owned(),
            message: MavMessage::HOME_POSITION(HOME_POSITION_DATA {
                latitude: scaled_coordinate(lat),
                longitude: scaled_coordinate(lng),
                altitude: scaled_altitude(alt),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn the_position_is_decoded_into_degrees_and_metres() {
        let message = received(-35.2809, 149.13, 584.0);
        let home = home_position_of(&message).expect("a home position");

        assert!((home.lat - -35.2809).abs() < 1e-7);
        assert!((home.lng - 149.13).abs() < 1e-7);
        assert!((home.alt - 584.0).abs() < 1e-3);
    }

    #[test]
    fn the_altitude_travels_in_millimetres() {
        assert_eq!(scaled_altitude(584.25), 584_250);
    }

    #[test]
    fn a_position_that_survived_the_wire_is_the_same_position() {
        let asked = GeoPoint {
            lat: -35.2809,
            lng: 149.13,
            alt: 584.0,
        };
        let reported = home_position_of(&received(-35.2809, 149.13, 584.0)).expect("a home");

        assert!(is_same_position(reported, asked));
    }

    #[test]
    fn a_position_the_vehicle_kept_is_not_the_one_that_was_asked_for() {
        let asked = GeoPoint {
            lat: -35.2809,
            lng: 149.13,
            alt: 584.0,
        };
        let reported = home_position_of(&received(-35.2810, 149.13, 584.0)).expect("a home");

        assert!(!is_same_position(reported, asked));
    }

    #[test]
    fn the_request_names_the_message_that_is_asked_for() {
        let message = home_request_message(VehicleId::new(1, 1), HomeRequest::MessageRequest);

        match message {
            MavMessage::COMMAND_LONG(command) => {
                assert_eq!(command.command, MavCmd::MAV_CMD_REQUEST_MESSAGE);
                assert_eq!(command.param1, HOME_POSITION_ID as f32);
                assert_eq!(command.param5, 1.0);
            }
            other => panic!("expected a command, was {other:?}"),
        }
    }

    #[test]
    fn the_other_request_is_the_command_upstream_sends() {
        let message = home_request_message(VehicleId::new(1, 1), HomeRequest::Command);

        match message {
            MavMessage::COMMAND_LONG(command) => {
                assert_eq!(command.command, MavCmd::MAV_CMD_GET_HOME_POSITION);
            }
            other => panic!("expected a command, was {other:?}"),
        }
    }

    #[test]
    fn the_message_that_moves_the_home_is_addressed_to_the_vehicle() {
        let home = GeoPoint {
            lat: -35.2809,
            lng: 149.13,
            alt: 584.0,
        };
        let message = set_home_message(VehicleId::new(7, 1), home);

        assert_eq!(message.target_system, 7);
        assert_eq!(message.latitude, scaled_coordinate(-35.2809));
        assert_eq!(message.longitude, scaled_coordinate(149.13));
        assert_eq!(message.altitude, 584_000);
    }
}
