//! The QGC `.waypoints` file format.
//!
//! # The format
//!
//! A `.waypoints` file is a text file: a header line and one line per waypoint.
//!
//! ```text
//! QGC WPL 110
//! 0	1	3	16	0	0	0	0	-35.28090000	149.13000000	584.000000	1
//! 1	0	3	22	0	0	0	0	-35.28100000	149.13100000	30.000000	1
//! ```
//!
//! The example is shown with the tabs the format uses, so the fields line up.
//!
//! The fields of a waypoint line are, in order: the sequence, the *current* flag, the
//! frame, the command, the four parameters, the latitude, the longitude, the altitude and
//! the autocontinue flag. They are separated by a tab. Mission Planner's reader also
//! accepts a space or a comma as a separator and ignores empty fields, so this reader
//! does too — a file written by another tool is not rejected over whitespace.
//!
//! # The frame
//!
//! The frame column holds a `MAV_FRAME` value, and **the same value** the model carries
//! in [`MissionItem::frame`]. There is no translation table: the number in the file *is*
//! the number in the plan, which is what makes a file round trip exactly.
//!
//! That is worth spelling out, because the frame decides what an altitude *means*:
//!
//! | Value | Frame | The altitude is |
//! | --- | --- | --- |
//! | 0 | `MAV_FRAME_GLOBAL` | above mean sea level |
//! | 3 | `MAV_FRAME_GLOBAL_RELATIVE_ALT` | above the home position |
//! | 10 | `MAV_FRAME_GLOBAL_TERRAIN_ALT` | above the terrain |
//!
//! The deprecated `_INT` synonyms — 5, 6 and 11 — are *different numbers* that mean the
//! same thing as 0, 3 and 10. A file that carries one of them keeps it, so the file
//! round trips; the folding onto the modern equivalent happens at the wire boundary, in
//! [`crate::wire`], which is the only place that talks to a vehicle.
//!
//! # The home
//!
//! The first item of a mission is the home position, and the file marks it with
//! `current = 1`. A file whose first waypoint is *not* the home — one that starts at
//! sequence 1, say — gets a blank home item inserted in front of it, so that the plan
//! always starts where the vehicle expects it to.
//!
//! # What is lost
//!
//! The format has no place for the display name of an item, so an imported item has none.
//!
//! Mission Planner's writer also drops the *current* flag of every item but the home.
//! This writer keeps it, so a plan that was downloaded from a vehicle — where one item
//! carries the flag — comes back from a file with that item still marked.

// The example above shows the format the way it is written, and the format is tab
// separated.
#![allow(clippy::tabs_in_doc_comments)]

use std::fmt::Write as _;

use mpcraft_mavlink::MavCmd;

use crate::mavcmd;
use crate::wire::MissionType;
use crate::{MissionItem, Plan};

/// The header a `.waypoints` file starts with.
const HEADER: &str = "QGC WPL 110";

/// The number of fields a waypoint line has to have to be read at all.
///
/// The last two fields — the altitude and the autocontinue flag — may be missing; they
/// then take their defaults.
const MINIMUM_FIELDS: usize = 10;

/// The number of decimals a coordinate is written with.
///
/// Eight decimals is a hundred nanometre, which is finer than the `1e7` scaling of the
/// integer mission encoding, so a coordinate survives the round trip untouched.
const COORDINATE_DECIMALS: usize = 8;

/// The number of decimals an altitude is written with.
const ALTITUDE_DECIMALS: usize = 6;

/// Why a `.waypoints` file could not be read.
#[derive(Debug, thiserror::Error)]
pub enum WaypointError {
    /// The file does not start with the `QGC WPL` header.
    #[error("the file does not start with the 'QGC WPL' header")]
    MissingHeader,

    /// A field of a waypoint line could not be read.
    #[error("line {line}: {reason}")]
    MalformedLine {
        /// The one-based number of the line in the file.
        line: usize,
        /// What is wrong with it.
        reason: String,
    },

    /// The command column does not name a `MAV_CMD`.
    #[error("line {line}: '{value}' is not a MAV_CMD")]
    UnknownCommand {
        /// The one-based number of the line in the file.
        line: usize,
        /// The text that was found in the command column.
        value: String,
    },
}

/// Reads a `.waypoints` file.
///
/// The plan that comes back is a mission plan: the home is the first item, whether the
/// file said so or the reader had to insert a blank one.
pub fn read_waypoints(text: &str) -> Result<Plan, WaypointError> {
    let mut lines = text.lines();

    let header = lines.next().ok_or(WaypointError::MissingHeader)?;
    if !header.contains("QGC WPL") {
        return Err(WaypointError::MissingHeader);
    }

    let mut plan = Plan::new(MissionType::Mission);
    let mut seen_item = false;

    for (offset, line) in lines.enumerate() {
        // The header is line one, so the first waypoint is line two.
        let number = offset + 2;
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let fields: Vec<&str> = trimmed
            .split(['\t', ' ', ','])
            .filter(|field| !field.is_empty())
            .collect();

        // A line that is too short is not a waypoint. Mission Planner skips it rather
        // than failing the whole import, and so does this reader.
        if fields.len() < MINIMUM_FIELDS {
            continue;
        }

        let sequence = parse_number::<u32>(fields[0], number)?;
        let item = parse_item(&fields, number)?;

        if !seen_item {
            seen_item = true;
            // The list has to start at the home position. A file that starts anywhere
            // else gets a blank home in front of it.
            if sequence != 0 {
                plan.push(blank_home());
            }
        }

        plan.push(item);
    }

    Ok(plan)
}

/// Writes a plan as a `.waypoints` file.
///
/// The first item of the plan is the home, and it is written with `current = 1`; the
/// rest keep the flags they had.
#[must_use]
pub fn write_waypoints(plan: &Plan) -> String {
    let mut out = String::with_capacity(96 * (plan.len() + 1));
    out.push_str(HEADER);
    out.push('\n');

    for (index, item) in plan.items.iter().enumerate() {
        let current = index == 0 || item.current;
        let _ = writeln!(
            out,
            "{seq}\t{current}\t{frame}\t{command}\t{p1:.prec$}\t{p2:.prec$}\t{p3:.prec$}\
             \t{p4:.prec$}\t{lat:.prec$}\t{lng:.prec$}\t{altitude:.alt$}\t{autocontinue}",
            seq = index,
            current = u8::from(current),
            frame = item.frame,
            command = item.command as u32,
            p1 = item.params[0],
            p2 = item.params[1],
            p3 = item.params[2],
            p4 = item.params[3],
            lat = item.x,
            lng = item.y,
            altitude = item.z,
            autocontinue = u8::from(item.autocontinue),
            prec = COORDINATE_DECIMALS,
            alt = ALTITUDE_DECIMALS,
        );
    }

    out
}

/// Reads one waypoint line.
fn parse_item(fields: &[&str], line: usize) -> Result<MissionItem, WaypointError> {
    let current = parse_number::<u8>(fields[1], line)? != 0;
    let frame = parse_number::<u32>(fields[2], line)?;
    let command = parse_command(fields[3], line)?;

    let params = [
        parse_number::<f32>(fields[4], line)?,
        parse_number::<f32>(fields[5], line)?,
        parse_number::<f32>(fields[6], line)?,
        parse_number::<f32>(fields[7], line)?,
    ];

    let x = parse_number::<f64>(fields[8], line)?;
    let y = parse_number::<f64>(fields[9], line)?;

    // The altitude and the autocontinue flag are the two fields a file may leave out.
    let z = match fields.get(10) {
        Some(field) => parse_number::<f64>(field, line)?,
        None => 0.0,
    };
    let autocontinue = match fields.get(11) {
        Some(field) => parse_number::<u8>(field, line)? != 0,
        None => true,
    };

    Ok(MissionItem {
        id: crate::MissionItemId::next(),
        command,
        frame,
        params,
        x,
        y,
        z,
        current,
        autocontinue,
        display_name: None,
    })
}

/// Reads the command column, which holds either the wire value or the command name.
fn parse_command(field: &str, line: usize) -> Result<MavCmd, WaypointError> {
    let command = match field.parse::<u32>() {
        Ok(value) => mavcmd::command_from_value(value),
        Err(_) => mavcmd::command_from_name(field),
    };

    command.ok_or_else(|| WaypointError::UnknownCommand {
        line,
        value: field.to_string(),
    })
}

/// The blank home item that is inserted in front of a file that does not start at one.
fn blank_home() -> MissionItem {
    MissionItem {
        current: true,
        ..MissionItem::default()
    }
}

/// Reads one number, with a message that says which line it came from.
fn parse_number<T: std::str::FromStr>(text: &str, line: usize) -> Result<T, WaypointError> {
    text.parse::<T>().map_err(|_| WaypointError::MalformedLine {
        line,
        reason: format!("'{text}' is not a number"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waypoint(lat: f64, lng: f64, alt: f64) -> MissionItem {
        MissionItem {
            command: MavCmd::MAV_CMD_NAV_WAYPOINT,
            x: lat,
            y: lng,
            z: alt,
            current: true,
            ..MissionItem::default()
        }
    }

    fn sample_plan() -> Plan {
        let mut plan = Plan::new(MissionType::Mission);
        plan.push(waypoint(-35.2809, 149.13, 584.0));
        plan.push(MissionItem {
            command: MavCmd::MAV_CMD_NAV_TAKEOFF,
            x: -35.281,
            y: 149.131,
            z: 30.0,
            ..MissionItem::default()
        });
        plan.push(MissionItem {
            command: MavCmd::MAV_CMD_NAV_LOITER_UNLIM,
            x: -35.2825,
            y: 149.1325,
            z: 100.0,
            params: [0.0, 0.0, 25.0, 0.0],
            ..MissionItem::default()
        });
        plan
    }

    #[test]
    fn a_plan_survives_the_round_trip() {
        let plan = sample_plan();
        let text = write_waypoints(&plan);
        let read = read_waypoints(&text).expect("the file it wrote is readable");

        assert_eq!(read.len(), plan.len());
        for (written, read) in plan.items.iter().zip(read.items.iter()) {
            assert_eq!(read.command, written.command);
            assert_eq!(read.frame, written.frame);
            assert_eq!(read.params, written.params);
            assert_eq!(read.x, written.x);
            assert_eq!(read.y, written.y);
            assert_eq!(read.z, written.z);
            assert_eq!(read.current, written.current);
            assert_eq!(read.autocontinue, written.autocontinue);
        }
    }

    #[test]
    fn the_file_starts_with_the_qgc_header() {
        let text = write_waypoints(&sample_plan());
        assert!(text.starts_with("QGC WPL 110\n"), "was {text}");
    }

    #[test]
    fn the_home_is_the_first_item_and_is_marked_current() {
        let text = write_waypoints(&sample_plan());
        let first = text.lines().nth(1).expect("the file has a first waypoint");
        let fields: Vec<&str> = first.split('\t').collect();

        assert_eq!(fields[0], "0", "the home is the first item");
        assert_eq!(fields[1], "1", "the home is the current item");
        assert_eq!(fields[3], "16", "the home is a waypoint");
    }

    #[test]
    fn the_numbers_are_written_with_a_dot_and_a_fixed_precision() {
        let text = write_waypoints(&sample_plan());
        let first = text.lines().nth(1).expect("the file has a first waypoint");
        let fields: Vec<&str> = first.split('\t').collect();

        assert_eq!(fields[8], "-35.28090000");
        assert_eq!(fields[9], "149.13000000");
        assert_eq!(fields[10], "584.000000");
        assert!(!text.contains(','), "the decimal separator is a dot");
    }

    #[test]
    fn a_file_that_does_not_start_at_the_home_gets_a_blank_one() {
        let text =
            "QGC WPL 110\n1\t0\t3\t16\t0\t0\t0\t0\t-35.28090000\t149.13000000\t584.000000\t1\n";
        let plan = read_waypoints(text).expect("the file is readable");

        assert_eq!(plan.len(), 2, "the blank home was inserted");
        assert_eq!(plan.items[0].command, MavCmd::MAV_CMD_NAV_WAYPOINT);
        assert_eq!(plan.items[0].x, 0.0);
        assert_eq!(plan.items[0].y, 0.0);
        assert_eq!(plan.items[0].z, 0.0);
        assert!(plan.items[0].current, "the blank home is the current item");
        assert_eq!(plan.items[1].x, -35.2809);
    }

    #[test]
    fn a_file_that_starts_at_the_home_gets_no_extra_item() {
        let text =
            "QGC WPL 110\n0\t1\t3\t16\t0\t0\t0\t0\t-35.28090000\t149.13000000\t584.000000\t1\n";
        let plan = read_waypoints(text).expect("the file is readable");

        assert_eq!(plan.len(), 1);
        assert_eq!(plan.items[0].x, -35.2809);
    }

    #[test]
    fn comments_and_short_lines_are_skipped() {
        let text = "QGC WPL 110\n# a comment\n\n0\t1\t3\t16\t0\t0\t0\t0\t-35.28090000\t149.13000000\t584.000000\t1\n1\t0\t3\t22\n";
        let plan = read_waypoints(text).expect("the file is readable");

        assert_eq!(plan.len(), 1, "only the complete waypoint was read");
        assert_eq!(plan.items[0].command, MavCmd::MAV_CMD_NAV_WAYPOINT);
    }

    #[test]
    fn a_missing_header_is_an_error() {
        let error = read_waypoints("0\t1\t3\t16\t0\t0\t0\t0\t0\t0\t0\t1\n")
            .expect_err("the file has no header");
        assert!(matches!(error, WaypointError::MissingHeader));

        let error = read_waypoints("").expect_err("the file is empty");
        assert!(matches!(error, WaypointError::MissingHeader));
    }

    #[test]
    fn a_malformed_line_is_an_error_and_not_a_panic() {
        let text = "QGC WPL 110\n0\t1\t3\t16\t0\t0\t0\t0\tnorth\t149.13000000\t584.000000\t1\n";
        let error = read_waypoints(text).expect_err("the latitude is not a number");

        match error {
            WaypointError::MalformedLine { line, reason } => {
                assert_eq!(line, 2);
                assert!(reason.contains("north"), "was {reason}");
            }
            other => panic!("expected a malformed line, was {other}"),
        }
    }

    #[test]
    fn a_command_that_is_not_a_mav_cmd_is_an_error() {
        let text =
            "QGC WPL 110\n0\t1\t3\t4242\t0\t0\t0\t0\t-35.28090000\t149.13000000\t584.000000\t1\n";
        let error = read_waypoints(text).expect_err("4242 is not a command");

        match error {
            WaypointError::UnknownCommand { line, value } => {
                assert_eq!(line, 2);
                assert_eq!(value, "4242");
            }
            other => panic!("expected an unknown command, was {other}"),
        }
    }

    #[test]
    fn a_command_may_be_named_as_well_as_numbered() {
        let text = "QGC WPL 110\n0\t1\t3\tMAV_CMD_NAV_WAYPOINT\t0\t0\t0\t0\t-35.28090000\t149.13000000\t584.000000\t1\n";
        let plan = read_waypoints(text).expect("the file is readable");

        assert_eq!(plan.items[0].command, MavCmd::MAV_CMD_NAV_WAYPOINT);
    }

    #[test]
    fn the_fields_may_be_separated_by_spaces_or_commas() {
        let text = "QGC WPL 110\n0 1 3 16 0 0 0 0 -35.28090000 149.13000000 584.000000 1\n";
        let plan = read_waypoints(text).expect("the file is readable");
        assert_eq!(plan.items[0].x, -35.2809);

        let text = "QGC WPL 110\n0,1,3,16,0,0,0,0,-35.28090000,149.13000000,584.000000,1\n";
        let plan = read_waypoints(text).expect("the file is readable");
        assert_eq!(plan.items[0].y, 149.13);
    }

    #[test]
    fn the_frame_is_the_frame_the_model_carries() {
        // The file stores the MAV_FRAME value, and the model stores the same value, so
        // nothing is translated in either direction.
        let mut plan = Plan::new(MissionType::Mission);
        plan.push(MissionItem {
            frame: 10,
            ..waypoint(-35.2809, 149.13, 584.0)
        });

        let text = write_waypoints(&plan);
        let read = read_waypoints(&text).expect("the file is readable");
        assert_eq!(read.items[0].frame, 10, "the terrain frame survived");

        let fields: Vec<&str> = text.lines().nth(1).unwrap().split('\t').collect();
        assert_eq!(fields[2], "10");
    }

    #[test]
    fn a_line_without_an_altitude_is_read_with_a_zero_one() {
        let text = "QGC WPL 110\n0\t1\t3\t16\t0\t0\t0\t0\t-35.28090000\t149.13000000\n";
        let plan = read_waypoints(text).expect("the file is readable");

        assert_eq!(plan.items[0].z, 0.0);
        assert!(plan.items[0].autocontinue);
    }

    #[test]
    fn a_real_mission_file_is_read_the_way_it_is_written() {
        // The shape of a file QGroundControl writes: the home, a takeoff, a few
        // waypoints, a loiter and a return to launch.
        let text = "QGC WPL 110\n\
                    0\t1\t3\t16\t0\t0\t0\t0\t-35.363261\t149.165230\t584\t1\n\
                    1\t0\t3\t22\t15\t0\t0\t0\t-35.363261\t149.165230\t50\t1\n\
                    2\t0\t3\t16\t0\t0\t0\t0\t-35.363261\t149.165230\t100\t1\n\
                    3\t0\t3\t16\t10\t20\t0\t0\t-35.363261\t149.165230\t100\t1\n\
                    4\t0\t3\t17\t0\t0\t20\t0\t-35.363261\t149.165230\t100\t1\n\
                    5\t0\t3\t20\t0\t0\t0\t0\t-35.363261\t149.165230\t0\t1\n";
        let plan = read_waypoints(text).expect("the file is readable");

        assert_eq!(plan.len(), 6, "every waypoint was read");
        assert_eq!(plan.items[0].command, MavCmd::MAV_CMD_NAV_WAYPOINT);
        assert!(plan.items[0].current, "the home is the current item");
        assert_eq!(plan.items[0].z, 584.0);
        assert_eq!(plan.items[1].command, MavCmd::MAV_CMD_NAV_TAKEOFF);
        assert_eq!(plan.items[1].params[0], 15.0);
        assert_eq!(plan.items[3].params, [10.0, 20.0, 0.0, 0.0]);
        assert_eq!(plan.items[4].command, MavCmd::MAV_CMD_NAV_LOITER_UNLIM);
        assert_eq!(plan.items[4].params[2], 20.0);
        assert_eq!(plan.items[5].command, MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH);

        for item in &plan.items {
            assert_eq!(item.frame, 3, "the frame is relative to the home");
            assert!(item.autocontinue);
        }
    }

    #[test]
    fn the_written_file_is_the_one_mission_planner_writes() {
        // The golden file pins the format down: the header, the tab separated fields,
        // the eight decimals of a coordinate, the six of an altitude and the home on
        // the first line.
        let mut plan = Plan::new(MissionType::Mission);
        plan.push(MissionItem {
            command: MavCmd::MAV_CMD_NAV_WAYPOINT,
            x: -35.2809,
            y: 149.13,
            z: 584.0,
            current: true,
            ..MissionItem::default()
        });
        plan.push(MissionItem {
            command: MavCmd::MAV_CMD_NAV_TAKEOFF,
            x: -35.281,
            y: 149.131,
            z: 30.0,
            params: [15.0, 0.0, 0.0, 0.0],
            ..MissionItem::default()
        });

        let expected = "QGC WPL 110\n\
                        0\t1\t3\t16\t0.00000000\t0.00000000\t0.00000000\t0.00000000\t-35.28090000\t149.13000000\t584.000000\t1\n\
                        1\t0\t3\t22\t15.00000000\t0.00000000\t0.00000000\t0.00000000\t-35.28100000\t149.13100000\t30.000000\t1\n";

        assert_eq!(write_waypoints(&plan), expected);
    }
}
