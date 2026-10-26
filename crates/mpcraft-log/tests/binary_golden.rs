//! The reader of the binary logs of DataFlash, over bytes that came from a real card.
//!
//! The fixture is the head of a log this board wrote — 8 280 bytes brought over the transfer protocol, which is
//! what a parser needs, because the declarations of DataFlash are at the top of the file. The numbers here are
//! worked out from the published table the logger uses (`pymavlink/DFReader.py`, which follows ArduPilot's
//! `FormatEnum`) and not from the code being tested: the same bytes were read by pymavlink's own reader and by a
//! walker written from the rules, and the two agreed on every field. The head is cut in the middle of its
//! sixty-fourth declaration, which is what a partial fetch gives, and the test says where it stops rather than
//! pretending the file is whole.
//!
//! The values that prove the characters the map did not have are the ones of type `c` and `C` — a half word and an
//! unsigned half word said in hundredths — which this log carries in its extended Kalman fields. Before the map
//! covered them those characters were skipped, so the values of a message that carries them were read from the
//! wrong bytes: `XKF1` came back with its roll in the place of its instance.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;

use mpcraft_log::binary::BinaryReader;
use mpcraft_log::{FieldKind, Value};

/// The log the tests are read from.
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/card_log_head.bin")
}

/// A reader of the log the tests are read from.
fn reader() -> BinaryReader<Cursor<Vec<u8>>> {
    BinaryReader::new(Cursor::new(
        std::fs::read(fixture()).expect("the fixture is there"),
    ))
}

/// The first sample of each message, in the order the log holds them.
fn first_samples(reader: &mut BinaryReader<Cursor<Vec<u8>>>) -> HashMap<String, Vec<Value>> {
    let mut seen: HashMap<String, Vec<Value>> = HashMap::new();

    while let Some(sample) = reader.next_sample().expect("the log is read") {
        seen.entry(sample.name.clone())
            .or_insert(sample.values.clone());
    }

    seen
}

/// The number of a value, or nothing when it is not one.
fn number(values: &[Value], field: usize) -> Option<f64> {
    values.get(field)?.number()
}

#[test]
fn the_declarations_of_a_real_card_are_read() {
    let mut reader = reader();

    let samples = first_samples(&mut reader);

    let formats = reader.formats();

    // Worked out from the same bytes by pymavlink's own reader: the head carries sixty-three
    // declarations, and every one of them names its fields with a character the table gives.
    assert_eq!(formats.len(), 63);

    let fmt = formats
        .values()
        .find(|format| format.name == "FMT")
        .expect("the log declares FMT");

    // The declaration of the declarations, as the published table gives it: two bytes, an array of four
    // characters, an array of sixteen, an array of sixty-four — and the length counts the marker.
    assert_eq!(fmt.length, 89);
    assert_eq!(
        fmt.fields,
        vec!["Type", "Length", "Name", "Format", "Columns"]
    );
    assert_eq!(
        fmt.kinds,
        vec![
            FieldKind::U8,
            FieldKind::U8,
            FieldKind::Text(4),
            FieldKind::Text(16),
            FieldKind::Text(64),
        ]
    );

    assert!(samples.contains_key("MSG"), "the head holds a sample");
}

#[test]
fn the_half_words_in_hundredths_are_read_from_a_real_card() {
    let mut reader = reader();

    let samples = first_samples(&mut reader);

    let xkf1 = samples.get("XKF1").expect("the log holds XKF1");

    // TimeUS, C, Roll, Pitch, Yaw, VN, VE, VD, dPD, PN, PE, PD, GX, GY, GZ, OH
    assert_eq!(number(xkf1, 0), Some(165_966_547.0));
    assert_eq!(number(xkf1, 1), Some(0.0));

    // `c` is a half word said in hundredths and `C` an unsigned half word said in hundredths.
    assert!((number(xkf1, 2).unwrap() - -0.39).abs() < 1e-9);
    assert!((number(xkf1, 3).unwrap() - -0.30).abs() < 1e-9);
    assert!((number(xkf1, 4).unwrap() - 131.65).abs() < 1e-9);

    // `e` is a word said in hundredths, which is the type the map had — but read as four bytes, not two.
    assert!((number(xkf1, 15).unwrap() - 305.78).abs() < 1e-6);

    let kinds = reader
        .formats()
        .values()
        .find(|format| format.name == "XKF1")
        .expect("the log declares XKF1")
        .kinds
        .clone();

    assert_eq!(kinds[2], FieldKind::CentsI16);
    assert_eq!(kinds[4], FieldKind::CentsU16);
    assert_eq!(kinds[15], FieldKind::CentsI32);
}

#[test]
fn the_values_of_a_real_card_are_said_in_the_units_they_are_in() {
    let mut reader = reader();

    let samples = first_samples(&mut reader);

    let position = samples.get("POS").expect("the log holds POS");

    // A position in millionths of a degree is said in degrees.
    assert!((number(position, 1).unwrap() - 37.677_508_6).abs() < 1e-9);
    assert!((number(position, 2).unwrap() + 6.568_536_9).abs() < 1e-9);
    assert!((number(position, 3).unwrap() - 305.75).abs() < 1e-6);

    let imu = samples.get("IMU").expect("the log holds IMU");

    assert_eq!(number(imu, 0), Some(165_966_972.0));
    assert!((number(imu, 7).unwrap() + 9.782_096_862_792_969).abs() < 1e-9);
    assert_eq!(number(imu, 13), Some(1002.0));

    let message = samples.get("MSG").expect("the log holds MSG");

    assert_eq!(message[1], Value::Text("Throttle armed".to_owned()));

    // The field that carries no type the table does not know: on this card every declaration is read.
    for format in reader.formats().values() {
        assert!(
            format.untyped().is_empty(),
            "{} has a field the table does not give",
            format.name
        );
    }
}

#[test]
fn a_real_card_counts_its_own_samples() {
    let mut reader = reader();

    let mut counts: HashMap<String, usize> = HashMap::new();

    while let Some(sample) = reader.next_sample().expect("the log is read") {
        *counts.entry(sample.name.clone()).or_default() += 1;
    }

    // Worked out from the same bytes by pymavlink's reader and by a walker written from the rules: the head holds
    // sixty-three complete declarations and sixty-three messages that are not declarations, and none of them is
    // read before its declaration arrives.
    assert_eq!(counts.values().sum::<usize>(), 63);

    assert_eq!(counts.get("FMTU"), Some(&30));
    assert_eq!(counts.get("RAD"), Some(&5));
    assert_eq!(counts.get("IMU"), Some(&2));
    assert_eq!(counts.get("MSG"), Some(&1));
    assert_eq!(counts.get("DSF"), Some(&1));

    // The head is cut in the middle of its sixty-fourth declaration — seventy-one of its eighty-six bytes are
    // there — so what came before is kept and the place where it stopped is said. Worked out from the same bytes:
    // the declaration begins at 8 206, its three-byte marker is there, so the bytes that could not be read begin
    // at 8 209.
    let damage = reader.damage();

    assert_eq!(damage.len(), 1);
    assert_eq!(damage[0].offset, 8209);
    assert_eq!(
        damage[0].reason,
        "the log ends in the middle of a record of 86 bytes"
    );
}
