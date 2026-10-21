//! Where the vehicle is, in the systems an operator reads.
//!
//! A position is a latitude and a longitude, but that is not what an operator reads: the
//! same position is given in the universal transverse Mercator grid, in the military
//! grid reference system, and in the decimal degrees that the protocol carries.
//!
//! The projections are the geodesy crate's business, and this module only says where
//! the vehicle is and joins the three answers.

use serde::{Deserialize, Serialize};

/// Where the vehicle is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coordinates {
    /// The latitude in decimal degrees, positive north.
    pub lat: f64,
    /// The longitude in decimal degrees, positive east.
    pub lng: f64,
    /// The altitude above mean sea level, in metres.
    pub alt: f64,
    /// The position in the universal transverse Mercator grid.
    ///
    /// It is absent when the vehicle is outside the band the grid covers, which is
    /// eighty degrees south to eighty-four north.
    pub utm: Option<Utm>,
    /// The position in the military grid reference system.
    pub mgrs: Option<String>,
}

/// A position in the universal transverse Mercator grid.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Utm {
    /// The zone number, 1 to 60.
    pub zone: u8,
    /// The latitude band letter.
    pub band: char,
    /// The easting, in metres.
    pub easting: f64,
    /// The northing, in metres.
    pub northing: f64,
}

/// The coordinates of a position.
///
/// The projections are only made when the position is one they cover: a position
/// outside the band of the grid has no grid coordinate, and inventing one would be
/// worse than saying that there is none.
///
/// ```
/// use mpcraft_geo::projection::{mgrs_from_lat_lng, utm_from_lat_lng, LatLng};
///
/// let canberra = LatLng::new(-35.2809, 149.1300);
///
/// let utm = utm_from_lat_lng(canberra).expect("Canberra is in the grid");
/// assert_eq!(utm.zone, 55);
///
/// let mgrs = mgrs_from_lat_lng(canberra, 5).expect("Canberra is in the grid");
/// assert!(mgrs.to_string().starts_with("55H"));
/// ```
#[must_use]
pub fn of(lat: f64, lng: f64, alt: f64) -> Coordinates {
    let position = mpcraft_geo::projection::LatLng::new(lat, lng);

    let utm = mpcraft_geo::projection::utm_from_lat_lng(position)
        .ok()
        .map(|utm| Utm {
            zone: utm.zone,
            band: mpcraft_geo::projection::latitude_band(lat),
            easting: utm.easting,
            northing: utm.northing,
        });

    let mgrs = mpcraft_geo::projection::mgrs_from_lat_lng(position, 5).map(|mgrs| mgrs.to_string());

    Coordinates {
        lat,
        lng,
        alt,
        utm,
        mgrs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_is_given_in_the_three_systems() {
        let coordinates = of(-35.2809, 149.1300, 584.0);

        assert!((coordinates.lat + 35.2809).abs() < 1e-9);
        assert!((coordinates.lng - 149.13).abs() < 1e-9);

        let utm = coordinates.utm.expect("Canberra is in the grid");
        assert_eq!(utm.zone, 55);
        assert_eq!(utm.band, 'H');
        // The easting and the northing of Canberra are about 694 km and about 6 094 km.
        assert!(
            (utm.easting - 693_714.0).abs() < 1_000.0,
            "was {}",
            utm.easting
        );
        assert!(
            (utm.northing - 6_093_725.0).abs() < 1_000.0,
            "was {}",
            utm.northing
        );

        let mgrs = coordinates.mgrs.expect("Canberra is in the grid");
        assert!(mgrs.starts_with("55H"), "was {mgrs}");
    }

    #[test]
    fn a_position_outside_the_band_has_no_grid_coordinate() {
        // The pole is outside the eighty-four degrees the grid covers.
        let coordinates = of(89.0, 0.0, 0.0);

        assert!(coordinates.utm.is_none());
        assert!(coordinates.mgrs.is_none());
        // The decimal degrees are still there.
        assert!((coordinates.lat - 89.0).abs() < 1e-9);
    }
}
