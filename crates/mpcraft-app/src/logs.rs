//! The logs of the flights.
//!
//! # What this is
//!
//! A telemetry log is a stream of the frames that crossed the link, each of them with
//! the instant it was seen. It is what a flight is remembered by: a ground station that
//! cannot read it cannot say anything about a flight that has already happened.
//!
//! # What it is for
//!
//! The log is read twice: once to know what it holds, and once for what the operator
//! asks about it. Both passes stream, because a log of a long flight does not fit in
//! memory.
//!
//! The values of a graph over a log are the same values the live graph shows, read from
//! the log instead of from the link: the log is fed to the same aggregator the vehicle
//! feeds, and the samples are the same samples.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

use mpcraft_link::TelemetryAggregator;
use mpcraft_log::TlogReader;
use mpcraft_mavlink::dialect::message_name;
use mpcraft_mavlink::Message;
use mpcraft_vehicle::graph::{self, History, Sample};

use crate::error::{CommandError, CommandResult};
use crate::graphs::{SeriesData, SeriesPoints};

/// How often a sample of the log is taken, in microseconds.
///
/// A log of a long flight has a message every few milliseconds, and a sample for each
/// of them would be more than anybody can look at. A sample every fifth of a second is
/// enough to draw a graph and to play the flight back.
const SAMPLE_INTERVAL: u64 = 200_000;

/// The most samples the history of a log keeps.
const MAX_SAMPLES: usize = 20_000;

/// How long the history of a log keeps its samples.
///
/// A log of a flight does not last a day, but the history does not know that, and a
/// flight that lasted longer would lose its beginning.
const SPAN: Duration = Duration::from_secs(24 * 60 * 60);

/// What a log holds.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSummary {
    /// The messages the log holds, and how many of each there are.
    pub messages: Vec<MessageCount>,
    /// The instant the log starts at, in microseconds since the epoch.
    pub start: u64,
    /// The instant the log ends at, in microseconds since the epoch.
    pub end: u64,
    /// How many records the log holds.
    pub records: u64,
    /// How many records could not be read.
    pub broken: u64,
}

/// One of the messages a log holds.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageCount {
    /// The name of the message.
    pub name: String,
    /// How many of them the log holds.
    pub count: u64,
}

/// Reads a log and says what it holds.
///
/// # Errors
///
/// Returns an error when the file cannot be read, or when it does not hold a log.
pub fn summarise(path: &Path) -> CommandResult<LogSummary> {
    let file = File::open(path).map_err(|error| {
        CommandError::invalid_request(format!("the log could not be opened: {error}"))
    })?;

    let mut reader = TlogReader::new(file);
    let mut counts: BTreeMap<u32, u64> = BTreeMap::new();

    let mut records = 0_u64;
    let mut broken = 0_u64;
    let mut start = 0_u64;
    let mut end = 0_u64;

    while let Some(record) = reader
        .next_record()
        .map_err(|error| CommandError::invalid_request(error.to_string()))?
    {
        if records == 0 {
            start = record.timestamp;
        }

        end = record.timestamp;
        records += 1;

        let id = record.message.message_id();
        *counts.entry(id).or_insert(0) += 1;
    }

    // A log that cannot be read from the beginning is not a log, and saying so is better
    // than an empty list of messages.
    if records == 0 {
        broken += 1;
    }

    let messages = counts
        .into_iter()
        .map(|(id, count)| MessageCount {
            name: message_name(id).unwrap_or("UNKNOWN").to_owned(),
            count,
        })
        .collect();

    Ok(LogSummary {
        messages,
        start,
        end,
        records,
        broken,
    })
}

/// The values of a graph over a log.
///
/// The graph is the same one the live view draws, and the values are read from the log
/// instead of from the link.
///
/// # Errors
///
/// Returns an error when the file cannot be read, when it does not hold a log, or when
/// there is no graph with that name.
pub fn series(path: &Path, name: &str) -> CommandResult<SeriesData> {
    let Some(preset) = graph::preset_named(name) else {
        return Err(CommandError::invalid_request(format!(
            "there is no graph called {name:?}"
        )));
    };

    let compiled = graph::catalogue()
        .into_iter()
        .find(|candidate| candidate.preset.name.full() == name);

    let Some(compiled) = compiled else {
        return Err(CommandError::invalid_request(format!(
            "the graph {name:?} could not be read"
        )));
    };

    let history = read_history(path)?;

    let series = compiled
        .series
        .iter()
        .map(|series| {
            let values = history.series(&series.expression);

            SeriesPoints {
                name: series.name.to_owned(),
                unit: series.unit.to_owned(),
                x: values.iter().map(|(at, _)| at.as_secs_f64()).collect(),
                y: values.iter().map(|(_, value)| *value).collect(),
            }
        })
        .collect();

    Ok(SeriesData {
        name: preset.name.full(),
        description: preset.description.to_owned(),
        series,
    })
}

/// The track of a log: the places the vehicle went through.
///
/// The track is what a flight looks like from above, and it is what the KML export
/// draws.
///
/// # Errors
///
/// Returns an error when the file cannot be read, or when it does not hold a log.
pub fn track(path: &Path) -> CommandResult<Vec<TrackPoint>> {
    let file = File::open(path).map_err(|error| {
        CommandError::invalid_request(format!("the log could not be opened: {error}"))
    })?;

    let mut reader = TlogReader::new(file);
    let mut aggregator = TelemetryAggregator::new();
    let mut points: Vec<TrackPoint> = Vec::new();

    while let Some(record) = reader
        .next_record()
        .map_err(|error| CommandError::invalid_request(error.to_string()))?
    {
        aggregator.apply(&record.message);

        let Some(position) = aggregator.telemetry().position else {
            continue;
        };

        let last = points.last();

        if last.is_some_and(|last: &TrackPoint| {
            (last.latitude - position.lat).abs() < 1.0e-7
                && (last.longitude - position.lng).abs() < 1.0e-7
        }) {
            continue;
        }

        points.push(TrackPoint {
            latitude: position.lat,
            longitude: position.lng,
            altitude: position.alt,
            at: record.timestamp,
        });
    }

    Ok(points)
}

/// One of the places of the track of a log.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackPoint {
    /// The latitude, in degrees.
    pub latitude: f64,
    /// The longitude, in degrees.
    pub longitude: f64,
    /// The height over the sea, in metres.
    pub altitude: f64,
    /// When the vehicle was there, in microseconds since the epoch.
    pub at: u64,
}

/// Reads a log and builds the history of its telemetry.
fn read_history(path: &Path) -> CommandResult<History> {
    let file = File::open(path).map_err(|error| {
        CommandError::invalid_request(format!("the log could not be opened: {error}"))
    })?;

    let mut reader = TlogReader::new(file);
    let mut aggregator = TelemetryAggregator::new();
    let mut history = History::new(MAX_SAMPLES, SPAN);

    let mut start = 0_u64;
    let mut last_sample = 0_u64;
    let mut first = true;

    while let Some(record) = reader
        .next_record()
        .map_err(|error| CommandError::invalid_request(error.to_string()))?
    {
        aggregator.apply(&record.message);

        if first {
            start = record.timestamp;
            first = false;
        }

        // The samples are thinned: the log has more messages than a graph needs, and
        // the history keeps a sample every fifth of a second.
        let at = record.timestamp.saturating_sub(start);

        if !first && at.saturating_sub(last_sample) < SAMPLE_INTERVAL {
            continue;
        }

        last_sample = at;

        history.push(Sample::from_telemetry(
            aggregator.telemetry(),
            Duration::from_micros(at),
        ));
    }

    Ok(history)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_log_that_is_not_there_is_an_error() {
        let error = summarise(Path::new("there-is-no-such-log.tlog")).unwrap_err();

        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);
    }

    #[test]
    fn a_graph_that_does_not_exist_is_an_error() {
        let error = series(Path::new("there-is-no-such-log.tlog"), "Nothing/Here").unwrap_err();

        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);
    }
}
