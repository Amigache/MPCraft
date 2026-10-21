//! The reader of the logs of PX4, over a log that a real vehicle wrote.
//!
//! The fixture is a log of PX4 v1.11 that was cut down to its declarations and to
//! the first samples of a few of its topics, so that the test is fast and the file
//! is small enough to live in the repository.

use std::io::Cursor;
use std::path::PathBuf;

use mpcraft_log::ulog::{ULog, ULogReader, ULogType};
use mpcraft_log::Value;

/// The log the tests are read from.
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.ulg")
}

/// The bytes of the log the tests are read from.
fn bytes() -> Vec<u8> {
    std::fs::read(fixture()).expect("the fixture is there")
}

/// A reader of the log the tests are read from.
fn reader() -> ULogReader<Cursor<Vec<u8>>> {
    ULogReader::new(Cursor::new(bytes()))
}

/// Where the first sample of the log is, in bytes.
///
/// The declarations of a log come first and take most of it, so a log that is cut in
/// the middle of its bytes is often cut in the middle of its declarations.
fn first_sample_offset(bytes: &[u8]) -> usize {
    let mut position = 16_usize;

    while position + 3 <= bytes.len() {
        let size = u16::from_le_bytes([bytes[position], bytes[position + 1]]) as usize;

        if bytes[position + 2] == b'D' {
            return position;
        }

        position += 3 + size;
    }

    bytes.len()
}

#[test]
fn the_header_of_a_real_log_is_read() {
    let mut reader = reader();

    while reader.next_sample().expect("the log is read").is_some() {}

    let header = reader.header().expect("the log has a header");

    assert_eq!(header.version, 1);
    assert_eq!(header.start_timestamp_us, 20_309_082);
}

#[test]
fn the_declarations_of_a_real_log_are_read() {
    let mut reader = reader();

    while reader.next_sample().expect("the log is read").is_some() {}

    let formats = reader.formats();

    assert_eq!(formats.len(), 82);

    let gps = formats
        .get("vehicle_gps_position")
        .expect("the log declares the topic");

    assert_eq!(gps.fields[0].name, "timestamp");
    assert_eq!(gps.fields[0].kind, ULogType::UInt64);
    assert_eq!(gps.fields[2].name, "lat");
    assert_eq!(gps.fields[2].kind, ULogType::Int32);
    assert_eq!(gps.field("lat"), Some(2));

    // A list of values is one column per value.
    let attitude = formats
        .get("vehicle_attitude")
        .expect("the log declares the topic");

    let names: Vec<&str> = attitude
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();

    assert_eq!(
        names,
        vec![
            "timestamp",
            "q[0]",
            "q[1]",
            "q[2]",
            "q[3]",
            "delta_q_reset[0]",
            "delta_q_reset[1]",
            "delta_q_reset[2]",
            "delta_q_reset[3]",
            "quat_reset_counter",
        ]
    );
}

#[test]
fn a_topic_that_uses_another_one_is_spread_over_its_columns() {
    let mut reader = reader();

    while reader.next_sample().expect("the log is read").is_some() {}

    // The topic of the telemetry holds four heartbeats, and a heartbeat is a topic
    // of its own.
    let status = reader
        .formats()
        .get("telemetry_status")
        .expect("the log declares the topic");

    let names: Vec<&str> = status
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();

    assert_eq!(names[13], "heartbeats[0].timestamp");
    assert_eq!(names[17], "heartbeats[0].state");
    assert_eq!(names[18], "heartbeats[1].timestamp");
    assert_eq!(names[32], "heartbeats[3].state");
    assert_eq!(names.len(), 33);
}

#[test]
fn the_units_of_a_real_log_are_known() {
    let mut reader = reader();

    while reader.next_sample().expect("the log is read").is_some() {}

    let gps = reader
        .formats()
        .get("vehicle_gps_position")
        .expect("the log declares the topic");

    // The position of this vehicle is written as a whole number of hundred-millionths
    // of a degree and as a whole number of millimetres.
    assert_eq!(gps.fields[2].unit.as_deref(), Some("deg"));
    assert_eq!(gps.fields[2].scale, 1e-7);
    assert_eq!(gps.fields[3].unit.as_deref(), Some("deg"));
    assert_eq!(gps.fields[4].unit.as_deref(), Some("m"));
    assert_eq!(gps.fields[4].scale, 1e-3);

    // A topic that came later writes its position as a double of degrees, and a
    // double of degrees is not to be scaled.
    let global = reader
        .formats()
        .get("vehicle_global_position")
        .expect("the log declares the topic");

    assert_eq!(global.fields[1].kind, ULogType::Double);
    assert_eq!(global.fields[1].unit.as_deref(), Some("deg"));
    assert_eq!(global.fields[1].scale, 1.0);
}

#[test]
fn the_information_of_a_real_log_is_read() {
    let mut reader = reader();

    while reader.next_sample().expect("the log is read").is_some() {}

    let info = reader.info();

    assert_eq!(
        info.get("sys_name").expect("the log holds the name").value,
        Value::Text("PX4".to_owned())
    );
    assert_eq!(
        info.get("ver_hw").expect("the log holds the board").value,
        Value::Text("CUBEPILOT_CUBEORANGE".to_owned())
    );
    assert_eq!(
        info.get("ver_sw_release")
            .expect("the log holds the release")
            .value
            .number(),
        Some(0x010B_0200_u32 as f64)
    );

    // The vehicle says how far the clock of the board was from the clock of the
    // world.
    assert_eq!(
        info.get("time_ref_utc")
            .expect("the log holds the offset")
            .value
            .number(),
        Some(0.0)
    );
}

#[test]
fn the_parameters_of_a_real_log_are_read() {
    let mut reader = reader();

    while reader.next_sample().expect("the log is read").is_some() {}

    let parameters = reader.parameters();

    assert_eq!(parameters.len(), 980);

    let gate = parameters
        .get("ASPD_BETA_GATE")
        .expect("the log holds the parameter");

    assert_eq!(gate.kind, ULogType::Int32);
    assert_eq!(gate.value.number(), Some(1.0));
}

#[test]
fn the_samples_of_a_real_log_are_read() {
    let mut reader = reader();

    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut first: Option<mpcraft_log::ULogSample> = None;

    while let Some(sample) = reader.next_sample().expect("the log is read") {
        *counts.entry(sample.name.clone()).or_insert(0) += 1;

        if first.is_none() {
            first = Some(sample);
        }
    }

    assert_eq!(counts.get("vehicle_gps_position"), Some(&25));
    assert_eq!(counts.get("vehicle_attitude"), Some(&25));
    assert_eq!(counts.get("vehicle_status"), Some(&14));

    // The log is whole: nothing of it was lost.
    assert!(reader.damage().is_empty());

    let sample = first.expect("the log holds a sample");
    assert_eq!(sample.name, "sensor_combined");
    assert_eq!(sample.multi_id, 0);
    assert!(sample.timestamp_us > 0);
}

#[test]
fn a_position_of_a_real_log_is_given_in_degrees_and_metres() {
    let mut reader = reader();

    let mut gps = None;

    while let Some(sample) = reader.next_sample().expect("the log is read") {
        if sample.name == "vehicle_gps_position" {
            gps = Some(sample);
            break;
        }
    }

    let gps = gps.expect("the log holds the position");
    let format = reader
        .formats()
        .get("vehicle_gps_position")
        .expect("the log declares the topic");

    let latitude = format
        .value(&gps, "lat")
        .and_then(Value::number)
        .expect("the latitude is a number");
    assert!(
        (latitude - 63.4170622).abs() < 1e-9,
        "latitude was {latitude}"
    );

    let longitude = format
        .value(&gps, "lon")
        .and_then(Value::number)
        .expect("the longitude is a number");
    assert!(
        (longitude - 10.4082151).abs() < 1e-9,
        "longitude was {longitude}"
    );

    let altitude = format
        .value(&gps, "alt")
        .and_then(Value::number)
        .expect("the height is a number");
    assert!((altitude - 66.814).abs() < 1e-6, "height was {altitude}");

    // The fix of the vehicle was a good one.
    assert_eq!(
        format.value(&gps, "fix_type").and_then(Value::number),
        Some(3.0)
    );
    assert_eq!(
        format
            .value(&gps, "satellites_used")
            .and_then(Value::number),
        Some(15.0)
    );
}

#[test]
fn a_log_that_is_cut_in_the_middle_is_read_up_to_there() {
    let whole = bytes();

    // The log is cut in the middle of the declarations: what came before is kept and
    // the place where the log is broken is said.
    let cut = whole.len() / 3;

    let mut reader = ULogReader::new(Cursor::new(whole[..cut].to_vec()));

    while reader
        .next_sample()
        .expect("the log is read up to the end")
        .is_some()
    {}

    assert!(!reader.damage().is_empty());
    assert!(reader
        .damage()
        .iter()
        .any(|damage| damage.reason.contains("ends in the middle")));
}

#[test]
fn a_log_that_is_cut_in_the_middle_holds_what_came_before() {
    let whole = bytes();

    // The log is cut in the middle of the samples: the declarations and the samples
    // that came before are still there.
    let cut = first_sample_offset(&whole) + 2_000;

    let mut reader = ULogReader::new(Cursor::new(whole[..cut].to_vec()));

    let mut samples = 0_usize;

    while reader
        .next_sample()
        .expect("the log is read up to the end")
        .is_some()
    {
        samples += 1;
    }

    assert!(samples > 0, "the samples that came before are kept");
    assert!(!reader.formats().is_empty());
}

#[test]
fn a_whole_log_holds_its_topics() {
    let log = ULog::read(Cursor::new(bytes())).expect("the log is read");

    // A topic that is logged more than once — two batteries, for instance — is one
    // topic per instance.
    assert_eq!(log.topics().len(), 10);

    let gps = log
        .topic("vehicle_gps_position")
        .expect("the log holds the topic");
    assert_eq!(gps.samples.len(), 25);
    assert_eq!(gps.format.fields.len(), 25);

    // The log says when it started, and the vehicle says how far its clock was from
    // the clock of the world.
    assert_eq!(log.header().start_timestamp_us, 20_309_082);
    assert_eq!(log.epoch_us(0), Some(20_309_082));

    // The first and the last sample of the log bound the flight.
    assert!(log.start_us().expect("the log holds a sample") > 0);
    assert!(log.end_us().expect("the log holds a sample") > log.start_us().unwrap_or(0));
}
