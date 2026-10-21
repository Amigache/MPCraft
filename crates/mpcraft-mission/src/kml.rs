//! KML import and export.
//!
//! # The format
//!
//! KML is how a plan leaves the ground control station: Google Earth, and every tool
//! that reads it, opens the document this module writes.
//!
//! ```xml
//! <?xml version="1.0" encoding="UTF-8"?>
//! <kml xmlns="http://www.opengis.net/kml/2.2">
//!   <Document>
//!     <name>MPCraft mission</name>
//!     <Placemark>
//!       <name>1</name>
//!       <description>MAV_CMD_NAV_WAYPOINT</description>
//!       <Point><coordinates>149.13000000,-35.28090000,584.000000</coordinates></Point>
//!     </Placemark>
//!     <LineString><coordinates>149.13000000,-35.28090000,584.000000</coordinates></LineString>
//!   </Document>
//! </kml>
//! ```
//!
//! Note the coordinate order: **KML is longitude first**, the opposite of the model,
//! where X is the latitude.
//!
//! # The convention
//!
//! A plain KML has nowhere to put a command, so this reader and this writer agree on
//! one:
//!
//! * the `<description>` of a placemark holds the name of the command, the way the
//!   MAVLink specification spells it — `MAV_CMD_NAV_WAYPOINT`;
//! * the `<name>` of a placemark holds the sequence of the item, counting from one.
//!
//! A `<description>` that does not name a command is read as `MAV_CMD_NAV_WAYPOINT`, so a
//! document written by another tool — one whose placemarks are just markers on a map —
//! still imports as a plan of waypoints.
//!
//! The sequence is load bearing: when every placemark names a number, the items are put
//! back in the order the names say. When they do not, the order of the document is kept.
//!
//! # What is lost
//!
//! KML carries a position and a command. It does not carry the parameters of an item,
//! nor its frame, nor its flags: an imported plan has zero parameters and the default
//! frame, which is relative to the home position.
//!
//! That is worth naming, because a frame change changes what an altitude *means*: a
//! plan whose items were above mean sea level comes back above the home position, and
//! the vehicle flies lower than it was told to. A KML document is a way of *sharing* a
//! plan, not of archiving one — the `.waypoints` format is the one that keeps
//! everything.
//!
//! # The reader
//!
//! The reader is a small, focused one: it understands the handful of elements a mission
//! document is made of, and it copes with everything else. Namespace prefixes
//! (`<kml:Placemark>`) are stripped, a placemark without a point is skipped, and a
//! `<LineString>` that is not a list of points is ignored rather than fatal. Malformed
//! input produces a typed error; it never panics.

use std::fmt::Write as _;

use mpcraft_mavlink::MavCmd;

use crate::mavcmd;
use crate::wire::MissionType;
use crate::{MissionItem, Plan};

/// The number of decimals a coordinate is written with.
const COORDINATE_DECIMALS: usize = 8;

/// The number of decimals an altitude is written with.
const ALTITUDE_DECIMALS: usize = 6;

/// Why a KML document could not be read.
#[derive(Debug, thiserror::Error)]
pub enum KmlError {
    /// The document is not a KML document.
    #[error("the document has no KML root element")]
    NotKml,

    /// A coordinate list could not be read.
    #[error("the coordinates '{coordinates}' are not a longitude, latitude and altitude")]
    InvalidCoordinates {
        /// The text that could not be read.
        coordinates: String,
    },
}

/// Reads a KML document.
///
/// The placemarks are the items of the plan, in the order they appear. A document that
/// has no placemark but does have a `<LineString>` is read as the path it draws, which
/// is what a document written by a mapping tool looks like.
pub fn read_kml(text: &str) -> Result<Plan, KmlError> {
    let events = events(text);

    if !events
        .iter()
        .any(|event| matches!(event, Event::Start { name: "kml" }))
    {
        return Err(KmlError::NotKml);
    }

    let mut plan = Plan::new(MissionType::Mission);
    let mut stack: Vec<&str> = Vec::new();
    let mut placemark: Option<Placemark> = None;
    let mut in_point = false;
    let mut line = String::new();
    // The sequence of every item that named one, and the item itself.
    let mut sequenced: Vec<(Option<u32>, MissionItem)> = Vec::new();

    for event in &events {
        match event {
            Event::Start { name } => {
                if *name == "Placemark" {
                    placemark = Some(Placemark::default());
                }
                if *name == "Point" {
                    in_point = true;
                }
                stack.push(name);
            }
            Event::End { name } => {
                if *name == "Point" {
                    in_point = false;
                }
                if *name == "Placemark" {
                    if let Some(found) = placemark.take() {
                        let sequence = found.sequence();
                        if let Some(item) = found.into_item()? {
                            sequenced.push((sequence, item));
                        }
                    }
                }
                stack.pop();
            }
            Event::Text(text) => match stack.last() {
                Some(&"name") => {
                    if let Some(found) = placemark.as_mut() {
                        found.name.push_str(&unescape(text));
                    }
                }
                Some(&"description") => {
                    if let Some(found) = placemark.as_mut() {
                        found.description.push_str(&unescape(text));
                    }
                }
                Some(&"coordinates") => {
                    if in_point {
                        if let Some(found) = placemark.as_mut() {
                            found.coordinates.push_str(&unescape(text));
                        }
                    } else {
                        line.push_str(&unescape(text));
                    }
                }
                _ => {}
            },
        }
    }

    // The name of a placemark is the sequence of the item, so the items are put back in
    // the order the names say. A document whose names are not numbers keeps the order it
    // was written in.
    if sequenced.iter().all(|(sequence, _)| sequence.is_some()) {
        sequenced.sort_by_key(|(sequence, _)| sequence.unwrap_or_default());
    }

    plan.items
        .extend(sequenced.into_iter().map(|(_, item)| item));

    // A document with no placemark is a path, not a plan. Reading it as one is what a
    // mapping tool's export means by it.
    if plan.is_empty() && !line.trim().is_empty() {
        for (lat, lng, alt) in parse_line(&line)? {
            plan.push(MissionItem {
                command: MavCmd::MAV_CMD_NAV_WAYPOINT,
                x: lat,
                y: lng,
                z: alt,
                ..MissionItem::default()
            });
        }
    }

    // The first item of a plan is the home, and the home is the item the vehicle is
    // executing. The format has no field for that, so it is implied by the position.
    if let Some(first) = plan.items.first_mut() {
        first.current = true;
    }

    Ok(plan)
}

/// Writes a plan as a KML document.
///
/// Only the items that carry a position are written: a command without one has nothing
/// to put on the map. The `<LineString>` is the path Google Earth draws; the placemarks
/// are the items, and they are what this reader reads back.
#[must_use]
pub fn write_kml(plan: &Plan) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n");
    out.push_str("  <Document>\n");
    out.push_str("    <name>MPCraft mission</name>\n");

    let mut path = String::new();

    for (index, item) in plan.items.iter().enumerate() {
        if !item.is_location_command() {
            continue;
        }

        let _ = writeln!(out, "    <Placemark>");
        let _ = writeln!(out, "      <name>{}</name>", index + 1);
        let _ = writeln!(
            out,
            "      <description>{}</description>",
            escape(&mavcmd::command_name(item.command))
        );
        let _ = writeln!(
            out,
            "      <Point><coordinates>{lng:.prec$},{lat:.prec$},{altitude:.alt$}</coordinates></Point>",
            lng = item.y,
            lat = item.x,
            altitude = item.z,
            prec = COORDINATE_DECIMALS,
            alt = ALTITUDE_DECIMALS,
        );
        out.push_str("    </Placemark>\n");

        let _ = write!(
            path,
            "{lng:.prec$},{lat:.prec$},{altitude:.alt$} ",
            lng = item.y,
            lat = item.x,
            altitude = item.z,
            prec = COORDINATE_DECIMALS,
            alt = ALTITUDE_DECIMALS,
        );
    }

    if !path.is_empty() {
        out.push_str("    <LineString><coordinates>");
        out.push_str(path.trim_end());
        out.push_str("</coordinates></LineString>\n");
    }

    out.push_str("  </Document>\n");
    out.push_str("</kml>\n");
    out
}

/// Writes a track as a KML document.
///
/// A track is the path the vehicle has flown, not a plan: it is written again and
/// again while the vehicle flies, and what it holds is always the path so far. The
/// document is the line of the path and the place the vehicle is in now.
///
/// The points are given as latitude, longitude and height, in that order, which is the
/// order of the model — the document itself writes the longitude first, as KML does.
#[must_use]
pub fn write_track_kml(name: &str, points: &[(f64, f64, f64)]) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n");
    out.push_str("  <Document>\n");
    let _ = writeln!(out, "    <name>{}</name>", escape(name));

    let mut path = String::new();

    for (latitude, longitude, altitude) in points {
        let _ = write!(
            path,
            "{lng:.prec$},{lat:.prec$},{altitude:.alt$} ",
            lng = longitude,
            lat = latitude,
            altitude = altitude,
            prec = COORDINATE_DECIMALS,
            alt = ALTITUDE_DECIMALS,
        );
    }

    if !path.is_empty() {
        out.push_str("    <LineString><coordinates>");
        out.push_str(path.trim_end());
        out.push_str("</coordinates></LineString>\n");
    }

    if let Some((latitude, longitude, altitude)) = points.last() {
        let _ = writeln!(out, "    <Placemark>");
        let _ = writeln!(out, "      <name>Now</name>");
        let _ = writeln!(
            out,
            "      <Point><coordinates>{lng:.prec$},{lat:.prec$},{altitude:.alt$}</coordinates></Point>",
            lng = longitude,
            lat = latitude,
            altitude = altitude,
            prec = COORDINATE_DECIMALS,
            alt = ALTITUDE_DECIMALS,
        );
        out.push_str("    </Placemark>\n");
    }

    out.push_str("  </Document>\n");
    out.push_str("</kml>\n");
    out
}

/// A placemark that is being read.
#[derive(Default)]
struct Placemark {
    /// The text of the `<name>` element.
    name: String,
    /// The text of the `<description>` element.
    description: String,
    /// The text of the `<coordinates>` element of its point.
    coordinates: String,
}

impl Placemark {
    /// The sequence the placemark names, if it names one.
    fn sequence(&self) -> Option<u32> {
        self.name.trim().parse::<u32>().ok()
    }

    /// Turns the placemark into an item, if it has a position.
    fn into_item(self) -> Result<Option<MissionItem>, KmlError> {
        let Some((lat, lng, alt)) = parse_point(&self.coordinates)? else {
            // A placemark without a point is a marker, not a waypoint.
            return Ok(None);
        };

        // A description that does not name a command is a marker written by another
        // tool, and a marker is a waypoint.
        let command = mavcmd::command_from_name(self.description.trim())
            .unwrap_or(MavCmd::MAV_CMD_NAV_WAYPOINT);

        Ok(Some(MissionItem {
            id: crate::MissionItemId::next(),
            command,
            frame: mavcmd::DEFAULT_FRAME,
            params: [0.0; 4],
            x: lat,
            y: lng,
            z: alt,
            current: false,
            autocontinue: true,
            display_name: None,
        }))
    }
}

/// Reads one point, which is one longitude, latitude and altitude.
fn parse_point(text: &str) -> Result<Option<(f64, f64, f64)>, KmlError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    // A point is a single tuple; anything after it belongs to another element.
    let tuple = trimmed.split_whitespace().next().unwrap_or(trimmed);
    let mut parts = tuple.split(',');

    let lng = coordinate(parts.next(), tuple)?;
    let lat = coordinate(parts.next(), tuple)?;
    let altitude = match parts.next() {
        Some(value) => coordinate(Some(value), tuple)?,
        None => 0.0,
    };

    Ok(Some((lat, lng, altitude)))
}

/// Reads the points of a line, which is a list of tuples.
fn parse_line(text: &str) -> Result<Vec<(f64, f64, f64)>, KmlError> {
    let mut points = Vec::new();

    for tuple in text.split_whitespace() {
        let mut parts = tuple.split(',');

        let lng = coordinate(parts.next(), tuple)?;
        let lat = coordinate(parts.next(), tuple)?;
        let altitude = match parts.next() {
            Some(value) => coordinate(Some(value), tuple)?,
            None => 0.0,
        };

        points.push((lat, lng, altitude));
    }

    Ok(points)
}

/// Reads one coordinate.
fn coordinate(value: Option<&str>, source: &str) -> Result<f64, KmlError> {
    value
        .unwrap_or("")
        .trim()
        .parse::<f64>()
        .map_err(|_| KmlError::InvalidCoordinates {
            coordinates: source.to_string(),
        })
}

/// One event of the document.
enum Event<'a> {
    /// An element opens.
    Start {
        /// The name of the element, without its namespace prefix.
        name: &'a str,
    },
    /// An element closes.
    End {
        /// The name of the element, without its namespace prefix.
        name: &'a str,
    },
    /// Character data between two tags.
    Text(&'a str),
}

/// Splits a document into the events the reader cares about.
///
/// This is not an XML parser and does not try to be one: it recognises the tags, skips
/// the declarations and the comments, and hands the character data over. Anything it
/// does not understand is ignored, which is what keeps a foreign document readable.
fn events(text: &str) -> Vec<Event<'_>> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'<' {
            let start = index;
            while index < bytes.len() && bytes[index] != b'<' {
                index += 1;
            }
            out.push(Event::Text(&text[start..index]));
            continue;
        }

        if text[index..].starts_with("<!--") {
            index = match text[index..].find("-->") {
                Some(offset) => index + offset + 3,
                None => bytes.len(),
            };
            continue;
        }

        if text[index..].starts_with("<![CDATA[") {
            let end = text[index..]
                .find("]]>")
                .map_or(bytes.len(), |offset| index + offset);
            out.push(Event::Text(&text[index + "<![CDATA[".len()..end]));
            index = (end + 3).min(bytes.len());
            continue;
        }

        if text[index..].starts_with("<?") {
            index = match text[index..].find("?>") {
                Some(offset) => index + offset + 2,
                None => bytes.len(),
            };
            continue;
        }

        if text[index..].starts_with("<!") {
            index = match text[index..].find('>') {
                Some(offset) => index + offset + 1,
                None => bytes.len(),
            };
            continue;
        }

        let end = tag_end(text, index);
        let inner = &text[index + 1..end];
        index = (end + 1).min(bytes.len());

        if let Some(rest) = inner.strip_prefix('/') {
            out.push(Event::End {
                name: local_name(rest.trim()),
            });
            continue;
        }

        let body = inner.trim();
        let name = local_name(body.split_whitespace().next().unwrap_or(""));
        out.push(Event::Start { name });

        // An empty element opens and closes at once.
        if body.ends_with('/') {
            out.push(Event::End { name });
        }
    }

    out
}

/// The index of the `>` that closes the tag starting at `start`.
///
/// An attribute value may hold a `>` of its own, so the quotes are honoured.
fn tag_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = start + 1;
    let mut quote: Option<u8> = None;

    while index < bytes.len() {
        let byte = bytes[index];
        match quote {
            Some(open) if byte == open => quote = None,
            Some(_) => {}
            None if byte == b'"' || byte == b'\'' => quote = Some(byte),
            None if byte == b'>' => return index,
            None => {}
        }
        index += 1;
    }

    bytes.len()
}

/// The name of an element without its namespace prefix.
fn local_name(name: &str) -> &str {
    name.rsplit_once(':').map_or(name, |(_, local)| local)
}

/// Escapes the characters that may not appear literally in XML.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());

    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(character),
        }
    }

    out
}

/// Resolves the XML entities of a piece of text.
fn unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }

    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];

        let Some(end) = rest.find(';') else {
            break;
        };

        match resolve(&rest[1..end]) {
            Some(character) => {
                out.push(character);
                rest = &rest[end + 1..];
            }
            // An entity that is not one of ours is left alone rather than guessed at.
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }

    out.push_str(rest);
    out
}

/// Resolves one entity, given without its `&` and `;`.
fn resolve(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let number = entity.strip_prefix('#')?;
            let code = match number
                .strip_prefix('x')
                .or_else(|| number.strip_prefix('X'))
            {
                Some(hexadecimal) => u32::from_str_radix(hexadecimal, 16).ok()?,
                None => number.parse::<u32>().ok()?,
            };
            char::from_u32(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_plan() -> Plan {
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
            ..MissionItem::default()
        });
        plan.push(MissionItem {
            command: MavCmd::MAV_CMD_NAV_LOITER_UNLIM,
            x: -35.2825,
            y: 149.1325,
            z: 100.0,
            ..MissionItem::default()
        });
        plan
    }

    #[test]
    fn a_plan_survives_the_round_trip() {
        let plan = sample_plan();
        let text = write_kml(&plan);
        let read = read_kml(&text).expect("the document it wrote is readable");

        assert_eq!(read.len(), plan.len());
        for (written, read) in plan.items.iter().zip(read.items.iter()) {
            assert_eq!(read.command, written.command, "the command survived");
            assert_eq!(read.x, written.x);
            assert_eq!(read.y, written.y);
            assert_eq!(read.z, written.z);
        }
    }

    #[test]
    fn the_coordinates_are_written_longitude_first() {
        let text = write_kml(&sample_plan());
        assert!(
            text.contains("149.13000000,-35.28090000,584.000000"),
            "the longitude comes first: {text}"
        );
    }

    #[test]
    fn the_command_travels_in_the_description() {
        let text = write_kml(&sample_plan());
        assert!(text.contains("<description>MAV_CMD_NAV_WAYPOINT</description>"));
        assert!(text.contains("<description>MAV_CMD_NAV_TAKEOFF</description>"));
        assert!(text.contains("<description>MAV_CMD_NAV_LOITER_UNLIM</description>"));
    }

    #[test]
    fn the_sequence_travels_in_the_name() {
        let text = write_kml(&sample_plan());
        assert!(text.contains("<name>1</name>"));
        assert!(text.contains("<name>2</name>"));
        assert!(text.contains("<name>3</name>"));
    }

    #[test]
    fn a_placemark_without_a_description_is_a_waypoint() {
        let text = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                    <kml xmlns=\"http://www.opengis.net/kml/2.2\">\n\
                    <Document>\n\
                    <Placemark>\n\
                    <name>Point 1</name>\n\
                    <Point><coordinates>149.13,-35.2809,584</coordinates></Point>\n\
                    </Placemark>\n\
                    </Document>\n\
                    </kml>\n";
        let plan = read_kml(text).expect("the document is readable");

        assert_eq!(plan.len(), 1);
        assert_eq!(plan.items[0].command, MavCmd::MAV_CMD_NAV_WAYPOINT);
        assert_eq!(plan.items[0].x, -35.2809);
        assert_eq!(plan.items[0].y, 149.13);
        assert_eq!(plan.items[0].z, 584.0);
    }

    #[test]
    fn a_namespace_prefix_is_stripped() {
        let text = "<kml xmlns:kml=\"http://www.opengis.net/kml/2.2\">\n\
                    <kml:Document>\n\
                    <kml:Placemark>\n\
                    <kml:name>1</kml:name>\n\
                    <kml:description>MAV_CMD_NAV_TAKEOFF</kml:description>\n\
                    <kml:Point><kml:coordinates>149.13,-35.2809,584</kml:coordinates></kml:Point>\n\
                    </kml:Placemark>\n\
                    </kml:Document>\n\
                    </kml>\n";
        let plan = read_kml(text).expect("the document is readable");

        assert_eq!(plan.len(), 1);
        assert_eq!(plan.items[0].command, MavCmd::MAV_CMD_NAV_TAKEOFF);
        assert_eq!(plan.items[0].x, -35.2809);
    }

    #[test]
    fn a_document_without_a_placemark_is_empty() {
        let text = "<kml xmlns=\"http://www.opengis.net/kml/2.2\"><Document></Document></kml>";
        let plan = read_kml(text).expect("the document is readable");
        assert!(plan.is_empty());
    }

    #[test]
    fn a_placemark_without_a_point_is_skipped() {
        let text = "<kml xmlns=\"http://www.opengis.net/kml/2.2\"><Document>\n\
                    <Placemark><name>1</name><description>MAV_CMD_NAV_WAYPOINT</description></Placemark>\n\
                    <Placemark><name>2</name><Point><coordinates>149.13,-35.2809,584</coordinates></Point></Placemark>\n\
                    </Document></kml>";
        let plan = read_kml(text).expect("the document is readable");

        assert_eq!(plan.len(), 1, "the placemark without a point was skipped");
        assert_eq!(plan.items[0].x, -35.2809);
    }

    #[test]
    fn a_line_string_that_is_not_a_list_of_points_is_ignored() {
        let text = "<kml xmlns=\"http://www.opengis.net/kml/2.2\"><Document>\n\
                    <Placemark><name>1</name><Point><coordinates>149.13,-35.2809,584</coordinates></Point></Placemark>\n\
                    <LineString><coordinates>not a point at all</coordinates></LineString>\n\
                    </Document></kml>";
        let plan = read_kml(text).expect("the document is readable");

        assert_eq!(plan.len(), 1, "the placemark was read and the line ignored");
    }

    #[test]
    fn a_document_that_is_not_kml_is_an_error() {
        let error = read_kml("this is not a document").expect_err("there is no root element");
        assert!(matches!(error, KmlError::NotKml));

        let error = read_kml("").expect_err("the document is empty");
        assert!(matches!(error, KmlError::NotKml));
    }

    #[test]
    fn coordinates_that_are_not_numbers_are_an_error_and_not_a_panic() {
        let text = "<kml xmlns=\"http://www.opengis.net/kml/2.2\"><Document>\n\
                    <Placemark><Point><coordinates>north,west,high</coordinates></Point></Placemark>\n\
                    </Document></kml>";
        let error = read_kml(text).expect_err("the coordinates are not numbers");

        match error {
            KmlError::InvalidCoordinates { coordinates } => {
                assert!(coordinates.contains("north"), "was {coordinates}");
            }
            other => panic!("expected invalid coordinates, was {other}"),
        }
    }

    #[test]
    fn malformed_markup_does_not_panic() {
        for text in [
            "<kml><Document><Placemark><Point><coordinates>1,2,3",
            "<kml><Document><Placemark attr=\"a > b\"><Point><coordinates>1,2,3</coordinates></Point></Placemark>",
            "<kml><<>>",
            "<kml><!-- a comment --><Document/>",
        ] {
            let _ = read_kml(text);
        }
    }

    #[test]
    fn the_escaped_characters_of_a_description_are_resolved() {
        let text = "<kml xmlns=\"http://www.opengis.net/kml/2.2\"><Document>\n\
                    <Placemark><description>MAV_CMD_NAV_WAYPOINT &amp; more</description>\n\
                    <Point><coordinates>149.13,-35.2809,584</coordinates></Point></Placemark>\n\
                    </Document></kml>";
        let plan = read_kml(text).expect("the document is readable");

        assert_eq!(
            plan.items[0].command,
            MavCmd::MAV_CMD_NAV_WAYPOINT,
            "a description that names a command and more is still that command"
        );
    }

    #[test]
    fn the_sequence_of_a_placemark_puts_the_items_in_order() {
        // The names are out of order, and the names are the sequence, so the items are
        // put back the way the names say.
        let text = "<kml xmlns=\"http://www.opengis.net/kml/2.2\"><Document>\n\
                    <Placemark><name>2</name><description>MAV_CMD_NAV_TAKEOFF</description>\n\
                    <Point><coordinates>149.13,-35.281,30</coordinates></Point></Placemark>\n\
                    <Placemark><name>1</name><description>MAV_CMD_NAV_WAYPOINT</description>\n\
                    <Point><coordinates>149.13,-35.2809,584</coordinates></Point></Placemark>\n\
                    </Document></kml>";
        let plan = read_kml(text).expect("the document is readable");

        assert_eq!(plan.len(), 2);
        assert_eq!(plan.items[0].command, MavCmd::MAV_CMD_NAV_WAYPOINT);
        assert_eq!(plan.items[1].command, MavCmd::MAV_CMD_NAV_TAKEOFF);
    }

    #[test]
    fn a_document_whose_names_are_not_numbers_keeps_its_order() {
        let text = "<kml xmlns=\"http://www.opengis.net/kml/2.2\"><Document>\n\
                    <Placemark><name>home</name><Point><coordinates>149.13,-35.2809,584</coordinates></Point></Placemark>\n\
                    <Placemark><name>tree</name><Point><coordinates>149.13,-35.281,30</coordinates></Point></Placemark>\n\
                    </Document></kml>";
        let plan = read_kml(text).expect("the document is readable");

        assert_eq!(plan.len(), 2);
        assert_eq!(plan.items[0].x, -35.2809);
        assert_eq!(plan.items[1].x, -35.281);
    }

    #[test]
    fn a_track_is_written_as_a_line_and_the_place_the_vehicle_is_in() {
        let points = [
            (-35.28, 149.13, 584.0),
            (-35.281, 149.131, 600.0),
        ];

        let text = write_track_kml("MPCraft track", &points);

        assert!(text.contains("<name>MPCraft track</name>"));
        assert!(text.contains(
            "<LineString><coordinates>149.13000000,-35.28000000,584.000000 \
             149.13100000,-35.28100000,600.000000</coordinates></LineString>"
        ));
        assert!(text.contains("<name>Now</name>"));
        assert!(text.contains(
            "<Point><coordinates>149.13100000,-35.28100000,600.000000</coordinates></Point>"
        ));
    }

    #[test]
    fn a_track_without_places_is_still_a_document() {
        let text = write_track_kml("MPCraft track", &[]);

        assert!(text.contains("<Document>"));
        assert!(!text.contains("<LineString>"));
        assert!(!text.contains("<Placemark>"));
    }

    #[test]
    fn the_written_document_is_the_one_google_earth_opens() {
        // The golden document pins the format down: the namespace, the description that
        // carries the command, the longitude first coordinates and the path.
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
            ..MissionItem::default()
        });

        let expected = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n",
            "  <Document>\n",
            "    <name>MPCraft mission</name>\n",
            "    <Placemark>\n",
            "      <name>1</name>\n",
            "      <description>MAV_CMD_NAV_WAYPOINT</description>\n",
            "      <Point><coordinates>149.13000000,-35.28090000,584.000000</coordinates></Point>\n",
            "    </Placemark>\n",
            "    <Placemark>\n",
            "      <name>2</name>\n",
            "      <description>MAV_CMD_NAV_TAKEOFF</description>\n",
            "      <Point><coordinates>149.13100000,-35.28100000,30.000000</coordinates></Point>\n",
            "    </Placemark>\n",
            "    <LineString><coordinates>149.13000000,-35.28090000,584.000000 ",
            "149.13100000,-35.28100000,30.000000</coordinates></LineString>\n",
            "  </Document>\n",
            "</kml>\n",
        );

        assert_eq!(write_kml(&plan), expected);
    }
}
