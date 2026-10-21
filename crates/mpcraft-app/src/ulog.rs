//! The log of PX4, read as the application reads every other log.
//!
//! A log of PX4 is a ULog file: a stream of messages, each of them a topic with its
//! samples. The reader in `mpcraft-log` knows the format; this module knows what the
//! topics of PX4 mean, and turns them into the telemetry model the rest of the
//! application speaks.
//!
//! Turning the log into the model is what makes the graphs work: the expressions of the
//! graphs are written over the model, and a log of PX4 is fed to the same model a
//! vehicle feeds, so the same graphs are drawn over it.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

use mpcraft_log::ulog::{ULogFormat, ULogReader, ULogSample};
use mpcraft_log::Value;
use mpcraft_vehicle::graph::{History, Sample};
use mpcraft_mavlink::MavGpsFixType;
use mpcraft_vehicle::{
    Attitude, Battery, FlightMode, GeoPoint, GpsStatus, Telemetry, Velocity,
};

use crate::error::{CommandError, CommandResult};
use crate::logs::{LogSummary, MessageCount, MessageSamples, TrackPoint};

/// How often a sample of the log is taken, in microseconds.
const SAMPLE_INTERVAL: u64 = 200_000;

/// The most samples the history of a log keeps.
const MAX_SAMPLES: usize = 20_000;

/// How long the history of a log keeps its samples.
const SPAN: Duration = Duration::from_secs(24 * 60 * 60);

/// Reads a log of PX4 and says what it holds.
///
/// # Errors
///
/// Returns an error when the file cannot be read, or when it does not hold a log.
pub fn summary(path: &Path) -> CommandResult<LogSummary> {
    let file = File::open(path).map_err(|error| {
        CommandError::invalid_request(format!("the log could not be opened: {error}"))
    })?;

    let mut reader = ULogReader::new(std::io::BufReader::new(file));
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut total = 0_u64;
    let mut start = 0_u64;
    let mut end = 0_u64;
    let mut first = true;

    while let Some(sample) = reader
        .next_sample()
        .map_err(|error| CommandError::invalid_request(error.to_string()))?
    {
        *counts.entry(sample.name.clone()).or_insert(0) += 1;
        total += 1;

        if first {
            start = sample.timestamp_us;
            first = false;
        }

        end = sample.timestamp_us;
    }

    let messages = counts
        .into_iter()
        .map(|(name, count)| MessageCount { name, count })
        .collect();

    Ok(LogSummary {
        messages,
        start,
        end,
        records: total,
        broken: reader.damage().len() as u64,
    })
}

/// The samples of one topic of a log of PX4.
///
/// # Errors
///
/// Returns an error when the file cannot be read, or when the log holds no topic of
/// that name.
pub fn samples(path: &Path, name: &str, limit: usize) -> CommandResult<MessageSamples> {
    let file = File::open(path).map_err(|error| {
        CommandError::invalid_request(format!("the log could not be opened: {error}"))
    })?;

    let mut reader = ULogReader::new(std::io::BufReader::new(file));

    let mut fields: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<serde_json::Value>> = Vec::new();
    let mut instants: Vec<f64> = Vec::new();
    let mut total = 0_u64;
    let mut start = 0_u64;
    let mut first = true;

    while let Some(sample) = reader
        .next_sample()
        .map_err(|error| CommandError::invalid_request(error.to_string()))?
    {
        if sample.name != name {
            continue;
        }

        total += 1;

        if first {
            start = sample.timestamp_us;
            first = false;
        }

        if rows.len() >= limit {
            continue;
        }

        if fields.is_empty() {
            if let Some(format) = reader.formats().get(name) {
                fields = format.fields.iter().map(|field| field.name.clone()).collect();
            }
        }

        let values: Vec<serde_json::Value> = sample
            .values
            .iter()
            .map(|value| match value {
                Value::Number(number) => serde_json::Value::from(*number),
                Value::Text(text) => serde_json::Value::String(text.clone()),
            })
            .collect();

        // The instants are said in seconds, like the ones of every other log.
        instants.push(sample.timestamp_us.saturating_sub(start) as f64 / 1_000_000.0);
        rows.push(values);
    }

    if total == 0 {
        return Err(CommandError::invalid_request(format!(
            "the log holds no topic called {name:?}"
        )));
    }

    Ok(MessageSamples {
        name: name.to_owned(),
        fields,
        rows,
        total,
        instants,
    })
}

/// The track of a log of PX4: the places the vehicle went through.
///
/// # Errors
///
/// Returns an error when the file cannot be read, or when it does not hold a log.
pub fn track(path: &Path) -> CommandResult<Vec<TrackPoint>> {
    let file = File::open(path).map_err(|error| {
        CommandError::invalid_request(format!("the log could not be opened: {error}"))
    })?;

    let mut reader = ULogReader::new(std::io::BufReader::new(file));

    // The place of the vehicle is in the global position, and when the log does not
    // carry it, in the position of the GPS. Both are read, and the better one is
    // kept: the global position is the one the vehicle has worked out, and the GPS is
    // the one the receiver heard.
    let mut global: Vec<TrackPoint> = Vec::new();
    let mut gps: Vec<TrackPoint> = Vec::new();

    while let Some(sample) = reader
        .next_sample()
        .map_err(|error| CommandError::invalid_request(error.to_string()))?
    {
        let topic = match sample.name.as_str() {
            "vehicle_global_position" => "vehicle_global_position",
            "vehicle_gps_position" => "vehicle_gps_position",
            _ => continue,
        };

        let Some(format) = reader.formats().get(topic) else {
            continue;
        };

        let (Some(latitude), Some(longitude)) = (
            number(format, &sample, "lat"),
            number(format, &sample, "lon"),
        ) else {
            continue;
        };

        let points = if topic == "vehicle_global_position" {
            &mut global
        } else {
            &mut gps
        };

        let last = points.last();

        if last.is_some_and(|last: &TrackPoint| {
            (last.latitude - latitude).abs() < 1.0e-7 && (last.longitude - longitude).abs() < 1.0e-7
        }) {
            continue;
        }

        points.push(TrackPoint {
            latitude,
            longitude,
            altitude: number(format, &sample, "alt").unwrap_or(0.0),
            at: sample.timestamp_us,
        });
    }

    if global.is_empty() {
        Ok(gps)
    } else {
        Ok(global)
    }
}

/// Reads a log of PX4 and builds the history of its telemetry.
///
/// The history is the same one the telemetry of a vehicle builds, so the graphs of the
/// catalogue are drawn over a log of PX4 exactly as they are drawn over a flight.
///
/// # Errors
///
/// Returns an error when the file cannot be read, or when it does not hold a log.
pub fn history(path: &Path) -> CommandResult<History> {
    let file = File::open(path).map_err(|error| {
        CommandError::invalid_request(format!("the log could not be opened: {error}"))
    })?;

    let mut reader = ULogReader::new(std::io::BufReader::new(file));
    let mut telemetry = Telemetry::default();
    let mut history = History::new(MAX_SAMPLES, SPAN);

    let mut start = 0_u64;
    let mut last_sample = 0_u64;
    let mut first = true;
    let mut taken = false;

    while let Some(sample) = reader
        .next_sample()
        .map_err(|error| CommandError::invalid_request(error.to_string()))?
    {
        let Some(format) = reader.formats().get(&sample.name).cloned() else {
            continue;
        };

        apply(&mut telemetry, &format, &sample);

        if first {
            start = sample.timestamp_us;
            first = false;
        }

        let at = sample.timestamp_us.saturating_sub(start);

        if taken && at.saturating_sub(last_sample) < SAMPLE_INTERVAL {
            continue;
        }

        taken = true;
        last_sample = at;

        history.push(Sample::from_telemetry(
            &telemetry,
            Duration::from_micros(at),
        ));
    }

    Ok(history)
}

/// Applies one sample of PX4 to the telemetry of the vehicle.
fn apply(telemetry: &mut Telemetry, format: &ULogFormat, sample: &ULogSample) {
    match sample.name.as_str() {
        // The topic of the attitude was called `attitude_q` before the vehicle was
        // renamed, and `vehicle_attitude` after it.
        "attitude_q" | "vehicle_attitude" => {
            let Some(attitude) = attitude_of(format, sample) else {
                return;
            };

            telemetry.attitude = Some(attitude);
        }

        "vehicle_global_position" => {
            let (Some(latitude), Some(longitude)) = (
                number(format, sample, "lat"),
                number(format, sample, "lon"),
            ) else {
                return;
            };

            telemetry.position = Some(GeoPoint {
                lat: latitude,
                lng: longitude,
                alt: number(format, sample, "alt").unwrap_or(0.0),
            });
        }

        "vehicle_gps_position" => {
            let (Some(latitude), Some(longitude)) = (
                number(format, sample, "lat"),
                number(format, sample, "lon"),
            ) else {
                return;
            };

            telemetry.position = Some(GeoPoint {
                lat: latitude,
                lng: longitude,
                alt: number(format, sample, "alt").unwrap_or(0.0),
            });

            let satellites = number(format, sample, "num_sats").unwrap_or(0.0);
            telemetry.gps = Some(GpsStatus {
                fix_type: MavGpsFixType::GPS_FIX_TYPE_3D_FIX,
                satellites: satellites.clamp(0.0, 255.0) as u8,
                hdop: number(format, sample, "hdop").unwrap_or(0.0),
            });
        }

        "vehicle_local_position" => {
            let north = number(format, sample, "x").unwrap_or(0.0);
            let east = number(format, sample, "y").unwrap_or(0.0);
            let down = number(format, sample, "z").unwrap_or(0.0);

            let vx = number(format, sample, "vx").unwrap_or(0.0);
            let vy = number(format, sample, "vy").unwrap_or(0.0);
            let vz = number(format, sample, "vz").unwrap_or(0.0);

            telemetry.velocity = Some(Velocity {
                north: vx,
                east: vy,
                down: vz,
                ground_speed: (vx * vx + vy * vy).sqrt(),
            });

            // The local position is the place of the vehicle in the frame of the home
            // position, and the height is over the ground.
            let _ = (north, east, down);
        }

        "battery_status" => {
            telemetry.battery = Some(Battery {
                voltage: number(format, sample, "voltage_v").unwrap_or(0.0),
                current: number(format, sample, "current_a").unwrap_or(0.0),
                remaining: number(format, sample, "remaining").unwrap_or(0.0),
            });
        }

        "vehicle_status" => {
            let arming = number(format, sample, "arming_state").unwrap_or(0.0);
            telemetry.armed = Some(arming == 2.0);

            let navigation = number(format, sample, "nav_state").unwrap_or(0.0);
            telemetry.mode = Some(FlightMode {
                custom_mode: navigation as u32,
                name: mode_name(navigation),
            });
        }

        _ => {}
    }
}

/// The attitude of the vehicle, from the quaternion of PX4.
fn attitude_of(format: &ULogFormat, sample: &ULogSample) -> Option<Attitude> {
    let w = number(format, sample, "q[0]")?;
    let x = number(format, sample, "q[1]")?;
    let y = number(format, sample, "q[2]")?;
    let z = number(format, sample, "q[3]")?;

    let roll = (2.0 * (w * x + y * z)).atan2(1.0 - 2.0 * (x * x + y * y));
    let pitch = (2.0 * (w * y - z * x)).clamp(-1.0, 1.0).asin();
    let yaw = (2.0 * (w * z + x * y)).atan2(1.0 - 2.0 * (y * y + z * z));

    Some(Attitude {
        roll: roll.to_degrees(),
        pitch: pitch.to_degrees(),
        yaw: yaw.to_degrees(),
    })
}

/// The name of the mode of PX4, given its number.
fn mode_name(navigation: f64) -> Option<String> {
    let name = match navigation as i64 {
        0 => "STABILIZED",
        1 => "STABILIZED",
        2 => "MANUAL",
        3 => "ALTCTL",
        4 => "POSCTL",
        5 => "AUTO",
        6 => "ACRO",
        7 => "OFFBOARD",
        8 => "STABILIZED",
        12 => "LAND",
        13 => "RETURN",
        14 => "FLIGHT_TERMINATION",
        15 => "FOLLOW",
        17 => "PRECISION_LAND",
        18 => "ORBIT",
        _ => return None,
    };

    Some(name.to_owned())
}

/// One value of a sample, as a number.
fn number(format: &ULogFormat, sample: &ULogSample, name: &str) -> Option<f64> {
    match format.value(sample, name)? {
        Value::Number(number) => Some(*number),
        Value::Text(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The log that comes with the crate, cut from a real flight of PX4.
    fn sample_log() -> Option<std::path::PathBuf> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../mpcraft-log/tests/fixtures/sample.ulg");

        path.exists().then_some(path)
    }

    #[test]
    fn the_summary_says_what_the_log_holds() {
        let Some(path) = sample_log() else {
            return;
        };

        let summary = summary(&path).expect("the log is readable");

        assert!(summary.records > 0, "the log holds records");
        assert!(
            summary
                .messages
                .iter()
                .any(|message| message.name == "vehicle_gps_position"),
            "the log holds the position: {:?}",
            summary.messages
        );
    }

    #[test]
    fn the_samples_of_a_topic_are_read() {
        let Some(path) = sample_log() else {
            return;
        };

        let samples = samples(&path, "vehicle_gps_position", 10).expect("the topic is there");

        assert!(!samples.fields.is_empty(), "the topic declares its fields");
        assert_eq!(samples.rows.len(), 10);
        assert!(samples.total > 10, "the topic holds more samples than asked for");
    }

    #[test]
    fn a_topic_that_is_not_in_the_log_is_an_error() {
        let Some(path) = sample_log() else {
            return;
        };

        let error = samples(&path, "there_is_no_such_topic", 10)
            .expect_err("the topic is not in the log");

        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);
    }

    #[test]
    fn the_history_is_built_from_the_topics_of_the_log() {
        let Some(path) = sample_log() else {
            return;
        };

        let history = history(&path).expect("the log is readable");

        assert!(!history.is_empty(), "the history holds samples");
    }
}
