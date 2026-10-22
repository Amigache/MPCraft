//! The places of a drawing, in the coordinates of the ground station.
//!
//! A plan can be brought in from a drawing — the shapefile of a GIS or the DXF of a CAD
//! program — and a drawing carries its places in whatever projection it was made in. The
//! ground station speaks one: the latitude and the longitude in degrees.
//!
//! Two cases are handled, which are the ones a plan is drawn in:
//!
//! * a drawing that says nothing of its projection, or says it is in degrees, is read as
//!   it is: its first coordinate is the longitude and its second one the latitude;
//! * a drawing whose `.prj` names a UTM zone is reprojected, because its coordinates are
//!   metres from the central meridian and metres from the equator, and a plan made of
//!   metres would fly the vehicle to a place that does not exist.

use crate::dxf::{DxfError, DxfFile};
use crate::projection::{utm_to_lat_lng, LatLng, Utm, UtmZoneError};
use crate::shapefile::{ShapeError, ShapeFile};

/// Why a drawing could not be brought in.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DrawingError {
    /// The shapefile could not be read.
    #[error("{0}")]
    Shape(#[from] ShapeError),

    /// The DXF could not be read.
    #[error("{0}")]
    Dxf(#[from] DxfError),

    /// The file names a projection that cannot be honoured.
    #[error("the projection of the drawing cannot be read: {0}")]
    Projection(String),

    /// A place of the drawing is not a place of the Earth.
    #[error("{0}")]
    Coordinates(#[from] UtmZoneError),

    /// The drawing holds no place.
    #[error("the drawing holds no point")]
    Empty,
}

/// The places of a drawing, in degrees.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Drawing {
    /// The places, as latitude and longitude, in the order they were drawn.
    pub places: Vec<(f64, f64)>,
}

impl Drawing {
    /// Reads a shapefile, and the projection beside it.
    ///
    /// The projection is the text of the `.prj` file of the same name, when there is one.
    ///
    /// # Errors
    ///
    /// Returns an error when the bytes are not a shapefile, when the projection names
    /// something that cannot be honoured, or when the drawing holds no place.
    pub fn read_shapefile(bytes: &[u8], prj: Option<&str>) -> Result<Self, DrawingError> {
        let file = ShapeFile::parse(bytes)?;
        let places = project(&file.as_lat_lng(), prj)?;

        Self::checked(places)
    }

    /// Reads a DXF drawing.
    ///
    /// # Errors
    ///
    /// Returns an error when the text is not a drawing, or when it holds no place.
    pub fn read_dxf(text: &str) -> Result<Self, DrawingError> {
        let file = DxfFile::parse(text)?;

        Self::checked(file.as_lat_lng())
    }

    /// A drawing, once it is known to hold a place.
    fn checked(places: Vec<(f64, f64)>) -> Result<Self, DrawingError> {
        if places.is_empty() {
            return Err(DrawingError::Empty);
        }

        Ok(Self { places })
    }
}

/// The places of a drawing, in degrees, whatever projection the drawing was made in.
fn project(places: &[(f64, f64)], prj: Option<&str>) -> Result<Vec<(f64, f64)>, DrawingError> {
    let Some(prj) = prj else {
        return Ok(places.to_vec());
    };

    let Some((zone, northern)) = utm_zone(prj) else {
        // A projection that is not UTM is not one this reader knows. The coordinates are
        // taken as they are: a drawing of the ground station's own degrees says nothing
        // in its `.prj` that contradicts it.
        if prj.to_ascii_uppercase().contains("UTM") {
            return Err(DrawingError::Projection(prj.to_owned()));
        }

        return Ok(places.to_vec());
    };

    places
        .iter()
        .map(|(lat, lng)| {
            // The shapefile writes the easting first and the northing second, which is
            // the opposite order of a latitude and a longitude.
            let utm = Utm {
                zone,
                northern,
                easting: *lng,
                northing: *lat,
            };

            let position: LatLng = utm_to_lat_lng(utm)?;

            Ok((position.lat, position.lng))
        })
        .collect()
}

/// The zone a `.prj` names, when it names a UTM one.
///
/// The text of a projection is written by more than one program, so the number and the
/// hemisphere are looked for where they are written and not where one program puts them:
/// `WGS_1984_UTM_Zone_33N` and `WGS 84 / UTM zone 33N` are the same zone.
#[must_use]
pub fn utm_zone(prj: &str) -> Option<(u8, bool)> {
    let upper = prj.to_ascii_uppercase();
    let at = upper.find("UTM")?;
    let after = &upper[at + 3..];

    // The first number after the word is the zone.
    let start = after.find(|character: char| character.is_ascii_digit())?;
    let digits: String = after[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let zone: u8 = digits.parse().ok()?;

    if !(1..=60).contains(&zone) {
        return None;
    }

    // The hemisphere is the first letter after the number, when it is one of the two.
    let hemisphere = after[start + digits.len()..]
        .chars()
        .find(|character| character.is_ascii_alphabetic());

    Some((zone, hemisphere != Some('S')))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection::utm_from_lat_lng;

    /// A point of the southern hemisphere, in degrees.
    fn sydney() -> (f64, f64) {
        (-35.28, 149.13)
    }

    #[test]
    fn a_drawing_of_degrees_is_read_as_it_is() {
        let places = project(&[sydney()], None).expect("the places are kept");

        assert_eq!(places, vec![sydney()]);
    }

    #[test]
    fn a_projection_of_utm_is_read() {
        assert_eq!(
            utm_zone("PROJCS[\"WGS 84 / UTM zone 55S\"]"),
            Some((55, false))
        );
        assert_eq!(
            utm_zone("GEOGCS[\"GCS_WGS_1984\"],PROJCS[\"WGS_1984_UTM_Zone_33N\"]"),
            Some((33, true))
        );
        assert_eq!(utm_zone("GEOGCS[\"GCS_WGS_1984\"]"), None);
    }

    #[test]
    fn the_metres_of_a_utm_drawing_are_turned_back_into_degrees() {
        let (lat, lng) = sydney();
        let utm = utm_from_lat_lng(LatLng { lat, lng }).expect("the point is in a zone");

        // The shapefile writes the easting and the northing, so the drawing holds the
        // metres where the degrees were.
        let metre_places = vec![(utm.northing, utm.easting)];
        let prj = format!("PROJCS[\"WGS 84 / UTM zone {}S\"]", utm.zone);
        let places = project(&metre_places, Some(&prj)).expect("the drawing is projected");

        assert!((places[0].0 - lat).abs() < 1e-6, "the latitude comes back");
        assert!(
            (places[0].1 - lng).abs() < 1e-6,
            "and so does the longitude"
        );
    }

    #[test]
    fn a_projection_that_says_utm_and_no_zone_is_refused() {
        let error = project(&[sydney()], Some("PROJCS[\"UTM\"]")).expect_err("no zone");
        assert!(matches!(error, DrawingError::Projection(_)));
    }

    #[test]
    fn a_drawing_without_places_is_an_error() {
        let error = Drawing::checked(Vec::new()).expect_err("nothing to fly to");
        assert_eq!(error, DrawingError::Empty);
    }

    #[test]
    fn the_points_of_dxf_are_kept() {
        let text =
            "  0\nSECTION\n  2\nENTITIES\n  0\nPOINT\n 10\n149.13\n 20\n-35.28\n  0\nENDSEC\n";
        let drawing = Drawing::read_dxf(text).expect("the drawing is read");

        assert_eq!(drawing.places.len(), 1);
        assert!((drawing.places[0].0 + 35.28).abs() < 1e-9);
        assert!((drawing.places[0].1 - 149.13).abs() < 1e-9);
    }
}
