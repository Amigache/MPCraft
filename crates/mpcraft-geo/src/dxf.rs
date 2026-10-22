//! The drawing of a CAD program, in its DXF flavour.
//!
//! A DXF file is a text file of pairs: a number, which says what the value is, and the
//! value itself, one after the other. The pairs that matter here are the ones that begin
//! an entity — `0` and the name of the entity — and the coordinates of it, `10` and `20`.
//!
//! ```text
//!   0
//! SECTION
//!   2
//! ENTITIES
//!   0
//! POINT
//!  10
//! 149.13
//!  20
//! -35.28
//!   0
//! ENDSEC
//! ```
//!
//! Mission Planner brings a plan in from a drawing, and this is the reader for the half
//! of that which is text: the points of the drawing are the waypoints, in the order they
//! were drawn. The units of a drawing are the units of the drawing — a plan needs
//! degrees — so a file that is not georeferenced is imported as if its coordinates were
//! longitude and latitude, which is what a plan exported from a mapping tool looks like.

use crate::shapefile::ShapePoint;

/// Why a drawing could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DxfError {
    /// The file is not a DXF drawing.
    #[error("the file is not a DXF drawing")]
    NotDxf,
}

/// The points of a drawing, in the order they are read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DxfFile {
    /// The points.
    pub points: Vec<ShapePoint>,
}

impl DxfFile {
    /// Reads a DXF drawing from its text.
    ///
    /// # Errors
    ///
    /// Returns an error when the text holds no section of entities, which is what makes a
    /// DXF a drawing rather than any other text file.
    pub fn parse(text: &str) -> Result<Self, DxfError> {
        let pairs = pairs(text);
        let mut points = Vec::new();

        let mut in_entities = false;
        let mut seen_entities = false;
        // The first coordinate of a point whose second coordinate has not been read yet.
        let mut pending: Option<f64> = None;
        // Whether the last pair asked for the name of a section.
        let mut wants_section_name = false;

        for (code, value) in pairs {
            match code {
                0 => {
                    if value.eq_ignore_ascii_case("SECTION") {
                        wants_section_name = true;
                    } else if value.eq_ignore_ascii_case("ENDSEC") {
                        in_entities = false;
                    }
                }
                2 if wants_section_name => {
                    in_entities = value.eq_ignore_ascii_case("ENTITIES");
                    seen_entities |= in_entities;
                    wants_section_name = false;
                }
                // The first coordinate of the point, of the start of a line, or of the
                // next vertex of a polyline.
                10 | 11 if in_entities => {
                    pending = value.trim().parse::<f64>().ok();
                }
                // And the second one, which completes it.
                20 | 21 if in_entities => {
                    if let (Some(x), Ok(y)) = (pending, value.trim().parse::<f64>()) {
                        points.push(ShapePoint { x, y });
                    }
                    pending = None;
                }
                _ => {}
            }
        }

        if !seen_entities {
            return Err(DxfError::NotDxf);
        }

        Ok(Self { points })
    }

    /// The points as longitudes and latitudes.
    ///
    /// A drawing that says nothing of its projection is taken to be in the coordinates
    /// the ground station speaks: the longitude and the latitude in degrees.
    #[must_use]
    pub fn as_lat_lng(&self) -> Vec<(f64, f64)> {
        self.points.iter().map(|point| (point.y, point.x)).collect()
    }
}

/// The pairs of the file: a number and the value that follows it.
fn pairs(text: &str) -> Vec<(i32, &str)> {
    let mut lines = text.lines();
    let mut pairs = Vec::new();

    while let (Some(code), Some(value)) = (lines.next(), lines.next()) {
        let Ok(code) = code.trim().parse::<i32>() else {
            continue;
        };

        pairs.push((code, value.trim()));
    }

    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A drawing with a point, a line and a polyline of two vertices.
    fn sample() -> &'static str {
        "  0\nSECTION\n  2\nENTITIES\n\
           0\nPOINT\n 10\n149.13\n 20\n-35.28\n\
           0\nLINE\n 10\n1.0\n 20\n2.0\n 11\n3.0\n 21\n4.0\n\
           0\nLWPOLYLINE\n 10\n10.0\n 20\n20.0\n 10\n30.0\n 20\n40.0\n\
           0\nENDSEC\n  0\nEOF\n"
    }

    #[test]
    fn the_points_of_the_entities_are_read() {
        let file = DxfFile::parse(sample()).expect("the drawing is read");

        assert_eq!(file.points.len(), 5);
        assert!((file.points[0].x - 149.13).abs() < 1e-9);
        assert!((file.points[0].y + 35.28).abs() < 1e-9);
        // The line gives its two ends.
        assert!((file.points[1].x - 1.0).abs() < 1e-9);
        assert!((file.points[2].x - 3.0).abs() < 1e-9);
        // The polyline gives one point per vertex.
        assert!((file.points[3].y - 20.0).abs() < 1e-9);
        assert!((file.points[4].y - 40.0).abs() < 1e-9);
    }

    #[test]
    fn the_points_are_longitude_and_latitude() {
        let file = DxfFile::parse(sample()).expect("the drawing is read");
        let places = file.as_lat_lng();

        assert!(
            (places[0].0 + 35.28).abs() < 1e-9,
            "the latitude comes first"
        );
        assert!(
            (places[0].1 - 149.13).abs() < 1e-9,
            "and then the longitude"
        );
    }

    #[test]
    fn a_text_that_is_not_a_drawing_is_an_error() {
        let error = DxfFile::parse("a note\nwith two lines\n").expect_err("it is not a drawing");
        assert_eq!(error, DxfError::NotDxf);
    }

    #[test]
    fn the_coordinates_outside_the_entities_are_not_points() {
        // The tables of the drawing repeat coordinates in the header, and a reader that
        // took them would fly the plan to the first corner of the page.
        let text = "  0\nSECTION\n  2\nHEADER\n  9\n$EXTMIN\n 10\n0.0\n 20\n0.0\n  0\nENDSEC\n\
                    0\nSECTION\n  2\nENTITIES\n  0\nPOINT\n 10\n5.0\n 20\n6.0\n  0\nENDSEC\n";
        let file = DxfFile::parse(text).expect("the drawing is read");

        assert_eq!(file.points.len(), 1);
        assert!((file.points[0].x - 5.0).abs() < 1e-9);
    }
}
