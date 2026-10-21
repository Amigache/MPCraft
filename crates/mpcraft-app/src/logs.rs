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
use mpcraft_mavlink::MavCmd;
use mpcraft_mavlink::Message;
use mpcraft_mission::{MissionItem, MissionItemId, MissionType, Plan};
use mpcraft_vehicle::graph::{self, History, Sample};
use mpcraft_vehicle::Telemetry;

use crate::error::{CommandError, CommandResult};
use crate::graphs::{SeriesData, SeriesPoints};

/// How often a sample of the log is taken, in microseconds.
///
/// A log of a long flight has a message every few milliseconds, and a sample for each
/// of them would be more than anybody can look at. A sample every fifth of a second is
/// enough to draw a graph and to play the flight back.
const SAMPLE_INTERVAL: u64 = 200_000;

/// How often the play-back of a log takes a sample, in microseconds.
const PLAYBACK_INTERVAL: u64 = 100_000;

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

/// The track of a log, as a KML document.
///
/// The document is the one Google Earth reads: the path of the flight as a line, and
/// every place of it as a mark.
///
/// # Errors
///
/// Returns an error when the file cannot be read, or when it does not hold a log.
pub fn track_kml(path: &Path) -> CommandResult<String> {
    let points = track(path)?;

    let items = points
        .iter()
        .map(|point| MissionItem {
            id: MissionItemId::next(),
            command: MavCmd::MAV_CMD_NAV_WAYPOINT,
            frame: mpcraft_mission::mavcmd::DEFAULT_FRAME,
            params: [0.0; 4],
            x: point.latitude,
            y: point.longitude,
            z: point.altitude,
            current: false,
            autocontinue: true,
            display_name: None,
        })
        .collect();

    let plan = Plan {
        mission_type: MissionType::Mission,
        items,
    };

    Ok(mpcraft_mission::kml::write_kml(&plan))
}

/// The samples of a log, as a CSV document.
///
/// The columns are the series of the graph that is chosen, and the rows are the
/// instants where the log has something to say.
///
/// # Errors
///
/// Returns an error when the file cannot be read, when it does not hold a log, or when
/// there is no graph with that name.
pub fn samples_csv(path: &Path, name: &str) -> CommandResult<String> {
    let compiled = graph::catalogue()
        .into_iter()
        .find(|candidate| candidate.preset.name.full() == name);

    let Some(compiled) = compiled else {
        return Err(CommandError::invalid_request(format!(
            "there is no graph called {name:?}"
        )));
    };

    let history = read_history(path)?;

    let mut writer = csv::Writer::from_writer(Vec::new());

    let header: Vec<String> = std::iter::once("seconds".to_owned())
        .chain(compiled.series.iter().map(|series| series.name.to_owned()))
        .collect();

    writer.write_record(&header).map_err(|error| {
        CommandError::internal(format!("the samples could not be written: {error}"))
    })?;

    // The samples are the ones the history holds, and the values of each series are
    // the ones it has at that instant: a series that has nothing there is left empty.
    for sample in history.samples() {
        let mut row: Vec<String> = vec![format!("{:.3}", sample.at.as_secs_f64())];

        for series in &compiled.series {
            let value = series
                .expression
                .evaluate(sample)
                .map(|value| format!("{value:.6}"))
                .unwrap_or_default();

            row.push(value);
        }

        writer.write_record(&row).map_err(|error| {
            CommandError::internal(format!("the samples could not be written: {error}"))
        })?;
    }

    let bytes = writer.into_inner().map_err(|error| {
        CommandError::internal(format!("the samples could not be written: {error}"))
    })?;

    String::from_utf8(bytes)
        .map_err(|error| CommandError::internal(format!("the samples are not text: {error}")))
}

/// A flight that is being played back.
///
/// The log is read once, and the telemetry of the flight is kept as it was: every
/// sample is the whole state of the vehicle at that instant, so the window can be
/// drawn as it was when the flight happened.
///
/// The samples are thinned, the same way the history of the graphs thins them: a
/// flight of an hour has a hundred thousand messages, and a tenth of a second is
/// enough to show it.
#[derive(Debug, Clone, Default)]
pub struct Playback {
    /// The telemetry of the flight, one snapshot per sample.
    samples: Vec<(Duration, Telemetry)>,
    /// Where the play-back is.
    position: Duration,
}

impl Playback {
    /// Reads a log and prepares it to be played back.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read, or when it does not hold a log.
    pub fn open(path: &Path) -> CommandResult<Self> {
        let file = File::open(path).map_err(|error| {
            CommandError::invalid_request(format!("the log could not be opened: {error}"))
        })?;

        let mut reader = TlogReader::new(file);
        let mut aggregator = TelemetryAggregator::new();

        let mut samples: Vec<(Duration, Telemetry)> = Vec::new();

        let mut start = 0_u64;
        let mut last_sample = 0_u64;
        let mut first = true;
        let mut taken = false;

        while let Some(record) = reader
            .next_record()
            .map_err(|error| CommandError::invalid_request(error.to_string()))?
        {
            aggregator.apply(&record.message);

            if first {
                start = record.timestamp;
                first = false;
            }

            let at = record.timestamp.saturating_sub(start);

            if taken && at.saturating_sub(last_sample) < PLAYBACK_INTERVAL {
                continue;
            }

            taken = true;
            last_sample = at;

            samples.push((Duration::from_micros(at), aggregator.telemetry().clone()));
        }

        Ok(Self {
            samples,
            position: Duration::ZERO,
        })
    }

    /// How long the flight lasted, in seconds.
    #[must_use]
    pub fn duration(&self) -> f64 {
        self.samples.last().map_or(0.0, |(at, _)| at.as_secs_f64())
    }

    /// How many samples the flight holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Whether the flight holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Moves the play-back to the given second of the flight.
    ///
    /// The telemetry of that instant is returned: the sample that is at or before
    /// the position, because a telemetry snapshot is the state of the vehicle and not
    /// a moment.
    pub fn seek(&mut self, seconds: f64) -> Telemetry {
        let position = Duration::from_secs_f64(seconds.max(0.0));

        self.position = position;

        self.samples
            .iter()
            .rev()
            .find(|(at, _)| *at <= position)
            .or_else(|| self.samples.first())
            .map_or_else(Telemetry::default, |(_, telemetry)| telemetry.clone())
    }

    /// Where the play-back is, in seconds.
    #[must_use]
    pub fn position(&self) -> f64 {
        self.position.as_secs_f64()
    }
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
    let mut taken = false;

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
        // the history keeps a sample every fifth of a second. The first one is always
        // taken, or the beginning of the flight would be missing.
        let at = record.timestamp.saturating_sub(start);

        if taken && at.saturating_sub(last_sample) < SAMPLE_INTERVAL {
            continue;
        }

        taken = true;
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
    use mpcraft_mavlink::dialect::messages::ATTITUDE_DATA;
    use mpcraft_mavlink::frame::MavHeader;
    use mpcraft_mavlink::MavMessage;

    /// A frame of an attitude, as it goes over the wire.
    fn attitude_frame(roll: f32, pitch: f32) -> Vec<u8> {
        let message = MavMessage::ATTITUDE(ATTITUDE_DATA {
            time_boot_ms: 0,
            roll,
            pitch,
            yaw: 0.0,
            rollspeed: 0.0,
            pitchspeed: 0.0,
            yawspeed: 0.0,
        });

        mpcraft_mavlink::frame::encode_v2(
            &message,
            MavHeader {
                system_id: 1,
                component_id: 1,
                sequence: 0,
            },
        )
    }

    /// A log with the given frames, each of them with its timestamp.
    fn write_tlog(path: &Path, records: &[(u64, Vec<u8>)]) {
        let mut bytes = Vec::new();

        for (timestamp, frame) in records {
            bytes.extend_from_slice(&timestamp.to_be_bytes());
            bytes.extend_from_slice(frame);
        }

        std::fs::write(path, bytes).expect("the log is written");
    }

    /// A log of a short flight, in a file of its own.
    fn log_file(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(name);

        let records: Vec<(u64, Vec<u8>)> = (0..20)
            .map(|step| {
                (
                    1_000_000 + step * 300_000,
                    attitude_frame(step as f32 * 0.1, 0.0),
                )
            })
            .collect();

        write_tlog(&path, &records);

        path
    }

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

    #[test]
    fn the_summary_says_what_the_log_holds() {
        let path = log_file("mpcraft-test-summary.tlog");

        let summary = summarise(&path).expect("the log is read");

        assert_eq!(summary.records, 20);
        assert_eq!(summary.messages.len(), 1);
        assert_eq!(summary.messages[0].name, "ATTITUDE");
        assert_eq!(summary.messages[0].count, 20);
        assert!(summary.end > summary.start);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_graph_over_a_log_has_the_values_of_the_log() {
        let path = log_file("mpcraft-test-series.tlog");

        let data = series(&path, "Attitude/Roll and Pitch").expect("the graph is read");

        assert!(!data.series.is_empty());
        assert_eq!(data.series[0].x.len(), 20);
        assert_eq!(data.series[0].x.len(), data.series[0].y.len());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn the_samples_of_a_log_are_written_as_csv() {
        let path = log_file("mpcraft-test-csv.tlog");

        let text = samples_csv(&path, "Attitude/Roll and Pitch").expect("the samples are read");

        assert!(text.starts_with("seconds,"));
        assert_eq!(text.lines().count(), 21);

        let _ = std::fs::remove_file(path);
    }
}
