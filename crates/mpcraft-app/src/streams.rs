//! What the flight-data window asks the vehicle to stream.
//!
//! A vehicle sends its telemetry by the rates configured on the port it is talking on, and those
//! rates can all be zero. Against such a vehicle the screens of the flight-data window stay empty:
//! the aggregator is filled by what arrives, and nothing arrives. So the station asks for what it
//! reads, one message at a time, the way upstream does through its data-stream request or through
//! this command.
//!
//! The table is the rule the ask follows: it holds the messages the flight-data window reads, and
//! the rate of each one is the rate the category it belongs to has upstream. It lives here rather
//! than in the link because the link carries the ask and this says what the station reads — which
//! screen reads what is a policy, not a protocol.
//!
//! The names are the ones the dialect gives the messages, and the numbers come from the dialect
//! rather than being written down: a number copied into a table is a number that can fall out of
//! step with the dialect it came from. One of them — `EKF_STATUS_REPORT` — is not in the compiled
//! dialect but is one a vehicle speaks, so its number comes from the table of those messages.

use mpcraft_mavlink::dialect::message_id;

/// The rate upstream asks for the attitude — its EXTRA1.
pub const ATTITUDE_RATE: u32 = 10;

/// The rate upstream asks for the position — its POSITION.
pub const POSITION_RATE: u32 = 4;

/// The rate upstream asks for the sensors — its RAW_SENSORS, EXTRA3 and RAW_CTRL.
pub const SENSORS_RATE: u32 = 2;

/// The rate upstream asks for the state of the vehicle — its EXT_STAT, RC_CHAN and PARAMS.
pub const STATUS_RATE: u32 = 1;

/// One message the flight-data window reads, at the rate its category has.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stream {
    /// The name the dialect gives the message.
    pub name: &'static str,
    /// The number the dialect gives the message.
    pub id: u32,
    /// How many times a second the vehicle is asked to send it.
    pub rate_hz: u32,
}

impl Stream {
    /// The interval the ask carries, in microseconds.
    ///
    /// The wire speaks an interval rather than a rate: the vehicle is told how long to wait between
    /// two sends, so the rate has to be turned into it. This is the only place the two are
    /// connected, and zero says stop — which is what upstream sends when it wants a stream back on
    /// the rates the port has.
    #[must_use]
    pub fn interval_us(&self) -> f32 {
        if self.rate_hz == 0 {
            return 0.0;
        }

        (1_000_000.0 / f64::from(self.rate_hz)) as f32
    }
}

/// The messages the flight-data window reads: the quick view, the horizon, the dials, the graphs,
/// the state of the vehicle, its outputs and its auxiliary functions.
///
/// Two things the window shows are not in the table because they do not travel as a stream: the
/// messages the board prints arrive as they are said, and the relays are asked for by the tab that
/// draws them. The home is in it because the map draws it and the distance bar reads it, and a live
/// vehicle sends it only once the station asks — which is what the run against the simulator found.
#[must_use]
pub fn table() -> Vec<Stream> {
    [
        ("ATTITUDE", ATTITUDE_RATE),
        ("GLOBAL_POSITION_INT", POSITION_RATE),
        ("GPS_RAW_INT", SENSORS_RATE),
        ("GPS2_RAW", SENSORS_RATE),
        ("SYS_STATUS", STATUS_RATE),
        ("VFR_HUD", SENSORS_RATE),
        ("RAW_IMU", SENSORS_RATE),
        ("SCALED_IMU", SENSORS_RATE),
        ("SCALED_PRESSURE", SENSORS_RATE),
        ("EKF_STATUS_REPORT", SENSORS_RATE),
        ("VIBRATION", SENSORS_RATE),
        ("SERVO_OUTPUT_RAW", SENSORS_RATE),
        ("RC_CHANNELS", SENSORS_RATE),
        ("BATTERY_STATUS", STATUS_RATE),
        ("HOME_POSITION", STATUS_RATE),
    ]
    .into_iter()
    .map(|(name, rate_hz)| Stream {
        name,
        id: message_id(name).unwrap_or(0),
        rate_hz,
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_of_the_table_is_one_the_dialect_names() {
        // A table entry whose name the dialect does not know carries the number 0, and the ask would
        // send a request for nothing. The rule is checked rather than assumed.
        for stream in table() {
            assert!(
                message_id(stream.name).is_some(),
                "{name} is not named",
                name = stream.name
            );
            assert!(stream.id > 0, "{name} has no number", name = stream.name);
        }
    }

    #[test]
    fn the_table_agrees_with_the_published_dictionary() {
        // The numbers below are read from the MAVLink dictionary, not from this project's dialect, so
        // the test is a second source rather than a mirror of the code it checks. A number copied into
        // a table is a number that can fall out of step with the source it was copied from, which is
        // why the table reads its numbers from the dialect instead of writing them down.
        const DICTIONARY: [(&str, u32); 15] = [
            ("SYS_STATUS", 1),
            ("GPS_RAW_INT", 24),
            ("SCALED_IMU", 26),
            ("RAW_IMU", 27),
            ("SCALED_PRESSURE", 29),
            ("ATTITUDE", 30),
            ("GLOBAL_POSITION_INT", 33),
            ("SERVO_OUTPUT_RAW", 36),
            ("RC_CHANNELS", 65),
            ("VFR_HUD", 74),
            ("GPS2_RAW", 124),
            ("BATTERY_STATUS", 147),
            ("EKF_STATUS_REPORT", 193),
            ("VIBRATION", 241),
            ("HOME_POSITION", 242),
        ];

        for (name, expected) in DICTIONARY {
            assert_eq!(message_id(name), Some(expected), "{name}");
        }
    }

    #[test]
    fn the_rate_is_turned_into_the_interval_the_wire_carries() {
        // The wire speaks an interval, so the rate of the table has to be turned into it once, here.
        let attitude = Stream {
            name: "ATTITUDE",
            id: 30,
            rate_hz: ATTITUDE_RATE,
        };
        assert_eq!(attitude.interval_us(), 100_000.0);

        let position = Stream {
            name: "GLOBAL_POSITION_INT",
            id: 74,
            rate_hz: POSITION_RATE,
        };
        assert_eq!(position.interval_us(), 250_000.0);

        // Zero is not a rate: it says stop, which is what upstream sends when it wants the rates of
        // the port back.
        let stopped = Stream {
            name: "VFR_HUD",
            id: 90,
            rate_hz: 0,
        };
        assert_eq!(stopped.interval_us(), 0.0);
    }

    #[test]
    fn the_table_carries_the_categories_it_says_it_reads() {
        // Every entry belongs to one of the four categories upstream asks for, and its rate is the
        // rate of that category. A table that grew a rate of its own would be a rule nobody wrote
        // down.
        let known = [ATTITUDE_RATE, POSITION_RATE, SENSORS_RATE, STATUS_RATE];

        for stream in table() {
            assert!(
                known.contains(&stream.rate_hz),
                "{name} has a rate no category has",
                name = stream.name
            );
        }

        assert_eq!(table().len(), 15, "the window reads fifteen messages");
    }
}
