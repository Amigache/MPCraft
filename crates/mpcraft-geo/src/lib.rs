//! Geodesy and terrain for MPCraft.
//!
//! Coordinates are decimal degrees in `f64`. Rounding to the wire format happens at exactly
//! one boundary — the encoder — so no value is silently degraded in the model.
//!
//! # Projections
//!
//! UTM and MGRS are implemented in pure Rust rather than through the `proj` crate.
//! `proj-sys` builds PROJ from source and needs SQLite and a working C toolchain, which makes
//! cross-platform packaging fragile — the opposite of what [ADR-006] requires. The
//! Transverse Mercator series used here is the standard Karney/Krüger formulation, accurate
//! to well under a millimetre within a UTM zone, which is far beyond what a GCS needs.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod height;
pub mod projection;
pub mod terrain;
pub mod terrain_service;
pub mod tiles;
pub mod zip;

pub use projection::{utm_from_lat_lng, utm_to_lat_lng, LatLng, Mgrs, Utm, UtmZone, UtmZoneError};
pub use terrain::{TerrainError, TerrainProvider};

use serde::{Deserialize, Serialize};

/// A geographic position in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    /// Latitude in degrees, positive north.
    pub lat: f64,
    /// Longitude in degrees, positive east.
    pub lng: f64,
    /// Altitude in metres above mean sea level.
    pub alt: f64,
}

impl GeoPoint {
    /// Creates a point with a zero altitude.
    #[must_use]
    pub const fn new(lat: f64, lng: f64) -> Self {
        Self { lat, lng, alt: 0.0 }
    }

    /// Creates a point with an explicit altitude.
    #[must_use]
    pub const fn with_alt(lat: f64, lng: f64, alt: f64) -> Self {
        Self { lat, lng, alt }
    }
}

/// The great-circle distance and initial bearing between two points.
///
/// Returns the distance in metres and the bearing in degrees clockwise from north.
///
/// ```
/// use mpcraft_geo::{great_circle, GeoPoint};
///
/// let canberra = GeoPoint::new(-35.2809, 149.1300);
/// let sydney = GeoPoint::new(-33.8688, 151.2093);
///
/// let (distance, bearing) = great_circle(canberra, sydney);
/// assert!((distance - 246_849.179).abs() < 400.0, "distance was {distance}");
/// assert!((bearing - 51.2064).abs() < 1.0, "bearing was {bearing}");
/// ```
#[must_use]
pub fn great_circle(from: GeoPoint, to: GeoPoint) -> (f64, f64) {
    const EARTH_RADIUS_M: f64 = 6_371_008.8;

    let lat1 = from.lat.to_radians();
    let lat2 = to.lat.to_radians();
    let delta_lat = (to.lat - from.lat).to_radians();
    let delta_lng = (to.lng - from.lng).to_radians();

    let a =
        (delta_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (delta_lng / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    let distance = EARTH_RADIUS_M * c;

    let y = delta_lng.sin() * lat2.cos();
    let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * delta_lng.cos();
    let bearing = y.atan2(x).to_degrees().rem_euclid(360.0);

    (distance, bearing)
}

/// The great-circle distance between two points, in metres.
///
/// Uses the haversine formula on a spherical earth. The error against the WGS-84 ellipsoid is
/// below 0.5 %, which is far below the precision a GCS displays.
#[must_use]
pub fn distance_m(a: GeoPoint, b: GeoPoint) -> f64 {
    great_circle(a, b).0
}

/// The initial bearing from one point to another, in degrees clockwise from north.
#[must_use]
pub fn bearing_deg(a: GeoPoint, b: GeoPoint) -> f64 {
    great_circle(a, b).1
}

/// Moves a point by a bearing and a distance, in metres.
///
/// Used to place relative-position mission items.
#[must_use]
pub fn offset(from: GeoPoint, bearing_deg: f64, distance_m: f64) -> GeoPoint {
    const EARTH_RADIUS_M: f64 = 6_371_008.8;

    let angular = distance_m / EARTH_RADIUS_M;
    let bearing = bearing_deg.to_radians();
    let lat1 = from.lat.to_radians();
    let lng1 = from.lng.to_radians();

    let lat2 = (lat1.sin() * angular.cos() + lat1.cos() * angular.sin() * bearing.cos()).asin();

    // The numerator carries a `cos(lat1)` factor: without it the eastward displacement is
    // overstated by a factor of `1 / cos(lat1)`, which at 35 degrees of latitude is a 22 %
    // error in the resulting position.
    let lng2 = lng1
        + (bearing.sin() * angular.sin() * lat1.cos())
            .atan2(angular.cos() - lat1.sin() * lat2.sin());

    GeoPoint {
        lat: lat2.to_degrees(),
        lng: lng2.to_degrees(),
        alt: from.alt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn distance_between_cities_matches_the_geodesic_reference() {
        // PROJ (`pyproj.Geod`) reports 246 849.179 m for this pair. The spherical haversine
        // model used here is within 0.1 % of the ellipsoidal value.
        let canberra = GeoPoint::new(-35.2809, 149.1300);
        let sydney = GeoPoint::new(-33.8688, 151.2093);
        let d = distance_m(canberra, sydney);
        assert!(approx(d, 246_849.179, 400.0), "distance was {d}");
    }

    #[test]
    fn distance_is_zero_for_identical_points() {
        let p = GeoPoint::new(12.34, 56.78);
        assert!(distance_m(p, p) < 0.001);
    }

    #[test]
    fn bearing_matches_the_geodesic_reference() {
        // PROJ reports an initial azimuth of 51.2064 degrees. The spherical great-circle
        // model used here agrees to within a degree.
        let canberra = GeoPoint::new(-35.2809, 149.1300);
        let sydney = GeoPoint::new(-33.8688, 151.2093);
        let b = bearing_deg(canberra, sydney);
        assert!(approx(b, 51.2064, 1.0), "bearing was {b}");
    }

    #[test]
    fn offset_and_distance_are_inverse() {
        let origin = GeoPoint::new(-35.0, 149.0);
        let moved = offset(origin, 45.0, 1000.0);
        assert!(approx(distance_m(origin, moved), 1000.0, 1.0));
        assert!(approx(bearing_deg(origin, moved), 45.0, 0.5));
    }

    #[test]
    fn offset_cardinal_directions_are_correct() {
        let origin = GeoPoint::new(0.0, 0.0);

        let north = offset(origin, 0.0, 1000.0);
        assert!(north.lat > 0.0, "north offset did not increase latitude");
        assert!(
            approx(north.lat, 1000.0 / 111_194.9, 1e-5),
            "north latitude was {}",
            north.lat
        );

        let east = offset(origin, 90.0, 1000.0);
        assert!(east.lng > 0.0, "east offset did not increase longitude");
        assert!(
            approx(east.lng, 1000.0 / 111_194.9, 1e-5),
            "east longitude was {}",
            east.lng
        );
    }
}
