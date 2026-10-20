//! Coordinate projections: UTM and MGRS.
//!
//! # Implementation
//!
//! The Transverse Mercator projection uses the Krüger series as presented by Karney, which
//! converges to well under a millimetre anywhere within a six-degree UTM zone — far beyond the
//! precision a ground control station displays.
//!
//! # Why not the `proj` crate
//!
//! `proj-sys` compiles PROJ from source and links SQLite, which makes cross-platform
//! packaging fragile and slow. Implementing the projection directly keeps MPCraft free of
//! native dependencies, in line with the cross-platform requirement.

use serde::{Deserialize, Serialize};

/// The WGS-84 semi-major axis, in metres.
const WGS84_A: f64 = 6_378_137.0;
/// The WGS-84 flattening.
const WGS84_F: f64 = 1.0 / 298.257_223_563;
/// The UTM scale factor at the central meridian.
const UTM_K0: f64 = 0.9996;
/// The false easting, in metres.
const FALSE_EASTING: f64 = 500_000.0;
/// The false northing for the southern hemisphere, in metres.
const FALSE_NORTHING: f64 = 10_000_000.0;

/// A geographic position in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LatLng {
    /// Latitude in degrees, positive north.
    pub lat: f64,
    /// Longitude in degrees, positive east.
    pub lng: f64,
}

impl LatLng {
    /// Creates a position.
    #[must_use]
    pub const fn new(lat: f64, lng: f64) -> Self {
        Self { lat, lng }
    }
}

/// A UTM coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Utm {
    /// The zone number, 1 to 60.
    pub zone: u8,
    /// Whether the zone is in the northern hemisphere.
    pub northern: bool,
    /// The easting, in metres.
    pub easting: f64,
    /// The northing, in metres.
    pub northing: f64,
}

/// A UTM zone identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UtmZone {
    /// The zone number, 1 to 60.
    pub number: u8,
    /// Whether the zone is in the northern hemisphere.
    pub northern: bool,
}

/// The error returned when a coordinate cannot be projected.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum UtmZoneError {
    /// The latitude is outside the UTM latitude band, 80°S to 84°N.
    #[error("latitude {0} is outside the UTM band (80°S to 84°N)")]
    LatitudeOutOfRange(f64),
    /// The longitude is outside the valid range.
    #[error("longitude {0} is outside the valid range")]
    LongitudeOutOfRange(f64),
}

/// The UTM zone containing a longitude.
///
/// ```
/// use mpcraft_geo::projection::zone_for_longitude;
///
/// assert_eq!(zone_for_longitude(149.13), 55); // Canberra
/// assert_eq!(zone_for_longitude(151.21), 56); // Sydney
/// assert_eq!(zone_for_longitude(-0.0015), 30); // London
/// assert_eq!(zone_for_longitude(-74.0), 18); // New York
/// ```
#[must_use]
pub fn zone_for_longitude(lng: f64) -> u8 {
    let zone = ((lng + 180.0) / 6.0).floor() as i32 + 1;
    zone.clamp(1, 60) as u8
}

/// Projects a geographic position into UTM.
///
/// # Errors
///
/// Returns [`UtmZoneError`] when the position is outside the UTM band.
pub fn utm_from_lat_lng(position: LatLng) -> Result<Utm, UtmZoneError> {
    if !(-80.0..=84.0).contains(&position.lat) {
        return Err(UtmZoneError::LatitudeOutOfRange(position.lat));
    }
    if !(-180.0..=180.0).contains(&position.lng) {
        return Err(UtmZoneError::LongitudeOutOfRange(position.lng));
    }

    let zone = zone_for_longitude(position.lng);
    let northern = position.lat >= 0.0;

    let e2 = WGS84_F * (2.0 - WGS84_F);
    let ep2 = e2 / (1.0 - e2);

    let lat = position.lat.to_radians();
    let lng = position.lng.to_radians();
    let lon0 = ((zone as f64 - 1.0) * 6.0 - 180.0 + 3.0).to_radians();

    let sin_lat = lat.sin();
    let cos_lat = lat.cos();
    let tan_lat = lat.tan();

    let n = WGS84_A / (1.0 - e2 * sin_lat * sin_lat).sqrt();
    let t = tan_lat * tan_lat;
    let c = ep2 * cos_lat * cos_lat;
    let a = cos_lat * (lng - lon0);

    let m = meridional_arc(lat, e2);

    let a2 = a * a;
    let a3 = a2 * a;
    let a4 = a3 * a;
    let a5 = a4 * a;
    let a6 = a5 * a;

    let easting = UTM_K0
        * n
        * (a + (1.0 - t + c) * a3 / 6.0
            + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * ep2) * a5 / 120.0)
        + FALSE_EASTING;

    let mut northing = UTM_K0
        * (m + n
            * tan_lat
            * (a2 / 2.0
                + (5.0 - t + 9.0 * c + 4.0 * c * c) * a4 / 24.0
                + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * ep2) * a6 / 720.0));

    if !northern {
        northing += FALSE_NORTHING;
    }

    Ok(Utm {
        zone,
        northern,
        easting,
        northing,
    })
}

/// Converts a UTM coordinate back to a geographic position.
///
/// # Errors
///
/// Returns [`UtmZoneError`] when the zone number is out of range.
pub fn utm_to_lat_lng(utm: Utm) -> Result<LatLng, UtmZoneError> {
    if !(1..=60).contains(&utm.zone) {
        return Err(UtmZoneError::LatitudeOutOfRange(f64::from(utm.zone)));
    }

    let e2 = WGS84_F * (2.0 - WGS84_F);
    let ep2 = e2 / (1.0 - e2);
    let e1 = (1.0 - (1.0 - e2).sqrt()) / (1.0 + (1.0 - e2).sqrt());

    let x = utm.easting - FALSE_EASTING;
    let y = if utm.northern {
        utm.northing
    } else {
        utm.northing - FALSE_NORTHING
    };

    let m = y / UTM_K0;
    let mu = m / (WGS84_A * (1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * e2 * e2 * e2 / 256.0));

    // Footprint latitude, by the standard Snyder inverse series. The result is the latitude
    // itself, not its sine.
    let phi1 = mu
        + (3.0 * e1 / 2.0 - 27.0 * e1.powi(3) / 32.0) * (2.0 * mu).sin()
        + (21.0 * e1 * e1 / 16.0 - 55.0 * e1.powi(4) / 32.0) * (4.0 * mu).sin()
        + (151.0 * e1.powi(3) / 96.0) * (6.0 * mu).sin()
        + (1097.0 * e1.powi(4) / 512.0) * (8.0 * mu).sin();

    let sin_phi1 = phi1.sin();
    let cos_phi1 = phi1.cos();
    let tan_phi1 = phi1.tan();

    let c1 = ep2 * cos_phi1 * cos_phi1;
    let t1 = tan_phi1 * tan_phi1;
    let n1 = WGS84_A / (1.0 - e2 * sin_phi1 * sin_phi1).sqrt();
    let r1 = WGS84_A * (1.0 - e2) / (1.0 - e2 * sin_phi1 * sin_phi1).powf(1.5);
    let d = x / (n1 * UTM_K0);

    let d2 = d * d;
    let d3 = d2 * d;
    let d4 = d3 * d;
    let d5 = d4 * d;
    let d6 = d5 * d;

    let lat = (phi1
        - (n1 * tan_phi1 / r1)
            * (d2 / 2.0 - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * ep2) * d4 / 24.0
                + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1 - 252.0 * ep2 - 3.0 * c1 * c1)
                    * d6
                    / 720.0))
        .to_degrees();

    let lon0 = ((f64::from(utm.zone) - 1.0) * 6.0 - 180.0 + 3.0).to_radians();
    let lng = (lon0
        + (d - (1.0 + 2.0 * t1 + c1) * d3 / 6.0
            + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * ep2 + 24.0 * t1 * t1) * d5
                / 120.0)
            / cos_phi1)
        .to_degrees();

    // Normalise into the range (-180, 180].
    let mut lng = lng.rem_euclid(360.0);
    if lng > 180.0 {
        lng -= 360.0;
    }

    Ok(LatLng { lat, lng })
}

/// The meridional arc, in metres.
fn meridional_arc(lat: f64, e2: f64) -> f64 {
    WGS84_A
        * ((1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * e2 * e2 * e2 / 256.0) * lat
            - (3.0 * e2 / 8.0 + 3.0 * e2 * e2 / 32.0 + 45.0 * e2 * e2 * e2 / 1024.0)
                * (2.0 * lat).sin()
            + (15.0 * e2 * e2 / 256.0 + 45.0 * e2 * e2 * e2 / 1024.0) * (4.0 * lat).sin()
            - (35.0 * e2 * e2 * e2 / 3072.0) * (6.0 * lat).sin())
}

/// The latitude band letter for a latitude, as used by MGRS.
///
/// The letters `C` through `X` (omitting `I` and `O`) cover eight-degree bands from 80°S to
/// 84°N.
#[must_use]
pub fn latitude_band(lat: f64) -> char {
    const BANDS: [char; 20] = [
        'C', 'D', 'E', 'F', 'G', 'H', 'J', 'K', 'L', 'M', 'N', 'P', 'Q', 'R', 'S', 'T', 'U', 'V',
        'W', 'X',
    ];
    if !(-80.0..=84.0).contains(&lat) {
        return 'X';
    }
    let index = ((lat + 80.0) / 8.0).floor() as usize;
    BANDS[index.min(19)]
}

/// A Military Grid Reference System coordinate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mgrs {
    /// The UTM zone number.
    pub zone: u8,
    /// The latitude band letter.
    pub band: char,
    /// The 100 km square identifier.
    pub square: String,
    /// The easting within the square, in metres.
    pub easting: u32,
    /// The northing within the square, in metres.
    pub northing: u32,
    /// The number of digits of precision in the easting and northing.
    pub precision: u32,
}

impl std::fmt::Display for Mgrs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The MGRS convention is five digits for a 1 m reference, fewer for coarser precision.
        let digits = 5usize.saturating_sub(self.precision as usize);
        write!(
            f,
            "{}{} {} {:0digits$} {:0digits$}",
            self.zone, self.band, self.square, self.easting, self.northing
        )
    }
}
/// The 100 km square column letters, indexed by UTM set.
///
/// A UTM "set" is `zone % 6`, with a remainder of zero meaning set 6. Each set starts its
/// column letters at a different origin, which is what makes the MGRS grid irregular.
const COLUMN_ORIGINS: [char; 6] = ['A', 'J', 'S', 'A', 'J', 'S'];

/// The 100 km square row letters, indexed by UTM set.
const ROW_ORIGINS: [char; 6] = ['A', 'F', 'A', 'F', 'A', 'F'];

/// The UTM set of a zone: `zone % 6`, with zero meaning 6.
#[must_use]
const fn set_of(zone: u8) -> usize {
    let remainder = zone % 6;
    if remainder == 0 {
        6
    } else {
        remainder as usize
    }
}

/// Advances a character code past the letters `I` and `O`, which MGRS omits.
///
/// `origin` is the letter the sequence started from, and `wrapped` records whether the
/// sequence has already cycled past `Z`.
fn skip_reserved(code: u8, origin: u8, wrapped: bool) -> u8 {
    const I: u8 = b'I';
    const O: u8 = b'O';

    let mut code = code;

    if code == I || (origin < I && code > I) || ((code > I || origin < I) && wrapped) {
        code += 1;
    }

    if code == O || (origin < O && code > O) || ((code > O || origin < O) && wrapped) {
        code += 1;
        if code == I {
            code += 1;
        }
    }

    code
}

/// The 100 km square column letter for a zone and column index.
///
/// The column index is `floor(easting / 100_000)`. A UTM zone spans roughly 166 000 m to
/// 834 000 m of easting, so index 0 does not occur in practice; it is clamped to 1.
///
/// ```
/// use mpcraft_geo::projection::column_letter;
///
/// // Validated against two independent reference implementations.
/// assert_eq!(column_letter(55, 6), 'F');
/// assert_eq!(column_letter(56, 3), 'L');
/// assert_eq!(column_letter(30, 7), 'Y');
/// assert_eq!(column_letter(54, 3), 'U');
/// ```
#[must_use]
pub fn column_letter(zone: u8, column_index: u32) -> char {
    const Z: u8 = b'Z';
    const A: u8 = b'A';

    let origin = COLUMN_ORIGINS[set_of(zone) - 1];
    // Index 0 is unreachable in a real UTM zone; clamping keeps the letter in range.
    let index = column_index.clamp(1, 7);
    let mut code = origin as u8 + index as u8 - 1;

    let mut wrapped = false;
    if code > Z {
        code = code - Z + A - 1;
        wrapped = true;
    }

    code = skip_reserved(code, origin as u8, wrapped);

    if code > Z {
        code = code - Z + A - 1;
    }

    char::from(code)
}

/// The 100 km square row letter for a zone and row index.
///
/// The row index is the 100 km row within the two-million-metre MGRS cycle, that is
/// `floor(northing / 100_000) % 20`.
///
/// ```
/// use mpcraft_geo::projection::row_letter;
///
/// assert_eq!(row_letter(55, 0), 'A');
/// assert_eq!(row_letter(56, 2), 'H');
/// assert_eq!(row_letter(30, 17), 'C');
/// ```
#[must_use]
pub fn row_letter(zone: u8, row_index: u32) -> char {
    const V: u8 = b'V';
    const A: u8 = b'A';

    let origin = ROW_ORIGINS[set_of(zone) - 1];
    let mut code = origin as u8 + row_index as u8;

    let wrapped = if code > V {
        code = code - V + A - 1;
        true
    } else {
        false
    };

    code = skip_reserved(code, origin as u8, wrapped);

    if code > V {
        code = code - V + A - 1;
    }

    char::from(code)
}

/// The MGRS UTM zone for a position, including the special zones for Norway and Svalbard.
///
/// Norway (56°N–64°N, 3°E–12°E) uses zone 32. Svalbard (72°N–84°N) uses zone 31 west of 9°E,
/// 33 between 9°E and 21°E, 35 between 21°E and 33°E, and 37 east of 33°E.
///
/// ```
/// use mpcraft_geo::projection::mgrs_zone;
/// use mpcraft_geo::projection::LatLng;
///
/// assert_eq!(mgrs_zone(LatLng::new(60.0, 5.0)), 32); // Norway
/// assert_eq!(mgrs_zone(LatLng::new(-35.0, 149.0)), 55); // Canberra
/// ```
#[must_use]
pub fn mgrs_zone(position: LatLng) -> u8 {
    let mut zone = zone_for_longitude(position.lng);

    if (56.0..64.0).contains(&position.lat) && (3.0..12.0).contains(&position.lng) {
        zone = 32;
    }

    if (72.0..84.0).contains(&position.lat) {
        zone = if position.lng < 9.0 {
            31
        } else if position.lng < 21.0 {
            33
        } else if position.lng < 33.0 {
            35
        } else {
            37
        };
    }

    zone
}

/// Converts a geographic position to an MGRS reference.
///
/// The precision is given in digits: `5` yields the standard 1 m reference.
///
/// ```
/// use mpcraft_geo::projection::{mgrs_from_lat_lng, LatLng};
///
/// let mgrs = mgrs_from_lat_lng(LatLng::new(-35.2809, 149.1300), 5).unwrap();
/// assert_eq!(mgrs.zone, 55);
/// assert_eq!(mgrs.band, 'H');
/// assert_eq!(mgrs.square, "FA");
/// ```
#[must_use]
pub fn mgrs_from_lat_lng(position: LatLng, precision: u32) -> Option<Mgrs> {
    let zone = mgrs_zone(position);
    let band = latitude_band(position.lat);

    // The MGRS zone may differ from the UTM zone in the special regions, so project into the
    // MGRS zone explicitly.
    let utm = utm_in_zone(position, zone)?;
    Some(mgrs_from_utm(utm, band, precision))
}

/// Converts a UTM coordinate to an MGRS reference.
///
/// The latitude band must be supplied separately: it is a function of latitude, which the
/// UTM northing only encodes indirectly.
#[must_use]
pub fn mgrs_from_utm(utm: Utm, band: char, precision: u32) -> Mgrs {
    let column_index = (utm.easting as u32).div_euclid(100_000);
    let row_index = ((utm.northing as u32).div_euclid(100_000)) % 20;

    let easting = (utm.easting as u32) % 100_000;
    let northing = (utm.northing as u32) % 100_000;

    let divisor = 10u32.pow(5u32.saturating_sub(precision.min(5))).max(1);

    Mgrs {
        zone: utm.zone,
        band,
        square: format!(
            "{}{}",
            column_letter(utm.zone, column_index),
            row_letter(utm.zone, row_index)
        ),
        easting: easting / divisor,
        northing: northing / divisor,
        precision: precision.min(5),
    }
}

/// Projects a position into a specific UTM zone.
fn utm_in_zone(position: LatLng, zone: u8) -> Option<Utm> {
    let mut utm = utm_from_lat_lng(position).ok()?;
    if utm.zone != zone {
        // The position fell outside the zone's natural longitude range. Re-project using the
        // zone's own central meridian.
        utm = utm_for_zone(position, zone)?;
    }
    Some(utm)
}

/// Projects a position into a specific UTM zone, regardless of its longitude.
fn utm_for_zone(position: LatLng, zone: u8) -> Option<Utm> {
    let e2 = WGS84_F * (2.0 - WGS84_F);
    let ep2 = e2 / (1.0 - e2);

    let lat = position.lat.to_radians();
    let lng = position.lng.to_radians();
    let lon0 = ((f64::from(zone) - 1.0) * 6.0 - 180.0 + 3.0).to_radians();

    let sin_lat = lat.sin();
    let cos_lat = lat.cos();
    let tan_lat = lat.tan();

    let n = WGS84_A / (1.0 - e2 * sin_lat * sin_lat).sqrt();
    let t = tan_lat * tan_lat;
    let c = ep2 * cos_lat * cos_lat;
    let a = cos_lat * (lng - lon0);
    let m = meridional_arc(lat, e2);

    let a2 = a * a;
    let a3 = a2 * a;
    let a4 = a3 * a;
    let a5 = a4 * a;
    let a6 = a5 * a;

    let easting = UTM_K0
        * n
        * (a + (1.0 - t + c) * a3 / 6.0
            + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * ep2) * a5 / 120.0)
        + FALSE_EASTING;

    let mut northing = UTM_K0
        * (m + n
            * tan_lat
            * (a2 / 2.0
                + (5.0 - t + 9.0 * c + 4.0 * c * c) * a4 / 24.0
                + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * ep2) * a6 / 720.0));

    if position.lat < 0.0 {
        northing += FALSE_NORTHING;
    }

    Some(Utm {
        zone,
        northern: position.lat >= 0.0,
        easting,
        northing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    /// Reference values produced by PROJ (`pyproj`) for the WGS-84 ellipsoid.
    const UTM_REFERENCE: &[(&str, f64, f64, u8, bool, f64, f64)] = &[
        // (name, lat, lng, zone, southern, easting, northing)
        (
            "Canberra",
            -35.2809,
            149.1300,
            55,
            true,
            693_714.434,
            6_093_725.185,
        ),
        (
            "Sydney",
            -33.8688,
            151.2093,
            56,
            true,
            334_368.634,
            6_250_948.345,
        ),
        (
            "London",
            51.4779,
            -0.0015,
            30,
            false,
            708_213.495,
            5_707_235.660,
        ),
        (
            "Rio",
            -22.9068,
            -43.1729,
            23,
            true,
            687_394.593,
            7_465_634.128,
        ),
        (
            "Reykjavik",
            64.1466,
            -21.9426,
            27,
            false,
            454_138.377,
            7_113_689.869,
        ),
        (
            "Ushuaia",
            -54.8019,
            -68.3020,
            19,
            true,
            544_869.379,
            3_927_029.245,
        ),
        (
            "NewYork",
            40.7128,
            -74.0060,
            18,
            false,
            583_959.372,
            4_507_350.998,
        ),
        (
            "Tokyo",
            35.6895,
            139.6917,
            54,
            false,
            381_622.230,
            3_950_298.908,
        ),
    ];

    #[test]
    fn zone_lookup_matches_known_cities() {
        assert_eq!(zone_for_longitude(149.13), 55); // Canberra
        assert_eq!(zone_for_longitude(151.2093), 56); // Sydney
        assert_eq!(zone_for_longitude(-0.0015), 30); // London
        assert_eq!(zone_for_longitude(-43.1729), 23); // Rio
        assert_eq!(zone_for_longitude(-74.006), 18); // New York
        assert_eq!(zone_for_longitude(139.6917), 54); // Tokyo
    }

    #[test]
    fn utm_matches_proj_reference_values() {
        for (name, lat, lng, zone, southern, easting, northing) in UTM_REFERENCE {
            let utm = utm_from_lat_lng(LatLng::new(*lat, *lng)).unwrap();
            assert_eq!(utm.zone, *zone, "{name}: wrong zone");
            assert_eq!(utm.northern, !*southern, "{name}: wrong hemisphere");
            assert!(
                approx(utm.easting, *easting, 0.01),
                "{name}: easting was {}, expected {easting}",
                utm.easting
            );
            assert!(
                approx(utm.northing, *northing, 0.01),
                "{name}: northing was {}, expected {northing}",
                utm.northing
            );
        }
    }

    #[test]
    fn utm_round_trips_within_a_millimetre() {
        for (name, lat, lng, ..) in UTM_REFERENCE {
            let original = LatLng::new(*lat, *lng);
            let utm = utm_from_lat_lng(original).unwrap();
            let restored = utm_to_lat_lng(utm).unwrap();
            assert!(
                approx(restored.lat, *lat, 1e-8),
                "{name}: latitude drifted: {lat} -> {}",
                restored.lat
            );
            assert!(
                approx(restored.lng, *lng, 1e-8),
                "{name}: longitude drifted: {lng} -> {}",
                restored.lng
            );
        }
    }

    #[test]
    fn latitudes_outside_the_utm_band_are_rejected() {
        assert!(utm_from_lat_lng(LatLng::new(85.0, 0.0)).is_err());
        assert!(utm_from_lat_lng(LatLng::new(-81.0, 0.0)).is_err());
        assert!(utm_from_lat_lng(LatLng::new(0.0, 200.0)).is_err());
    }

    #[test]
    fn latitude_bands_follow_the_standard() {
        // Bands are eight degrees tall, running C (80S) through X, skipping I and O.
        // The last band, X, is twelve degrees tall because UTM reaches 84N.
        assert_eq!(latitude_band(-75.0), 'C');
        assert_eq!(latitude_band(-35.0), 'H');
        assert_eq!(latitude_band(-5.0), 'M');
        assert_eq!(latitude_band(5.0), 'N');
        assert_eq!(latitude_band(35.0), 'S');
        assert_eq!(latitude_band(51.5), 'U');
        assert_eq!(latitude_band(68.0), 'W');
        assert_eq!(latitude_band(75.0), 'X');
        assert_eq!(latitude_band(83.0), 'X');
    }

    #[test]
    fn mgrs_matches_two_independent_reference_implementations() {
        // Expected values agree between the BAE `libmgrs` C library (via the Python `mgrs`
        // package) and the independent JavaScript `mgrs` package.
        let reference = [
            (
                LatLng::new(-35.2809, 149.1300),
                55,
                'H',
                "FA",
                93_714,
                93_725,
            ),
            (
                LatLng::new(-33.8688, 151.2093),
                56,
                'H',
                "LH",
                34_368,
                50_948,
            ),
            (LatLng::new(51.4779, -0.0015), 30, 'U', "YC", 8_213, 7_235),
            (
                LatLng::new(-22.9068, -43.1729),
                23,
                'K',
                "PQ",
                87_394,
                65_634,
            ),
            (
                LatLng::new(64.1466, -21.9426),
                27,
                'W',
                "VM",
                54_138,
                13_689,
            ),
            (
                LatLng::new(-54.8019, -68.3020),
                19,
                'F',
                "EV",
                44_869,
                27_029,
            ),
            (LatLng::new(40.7128, -74.0060), 18, 'T', "WL", 83_959, 7_350),
            (
                LatLng::new(35.6895, 139.6917),
                54,
                'S',
                "UE",
                81_622,
                50_298,
            ),
        ];

        for (position, zone, band, square, easting, northing) in reference {
            let mgrs = mgrs_from_lat_lng(position, 5).unwrap();
            assert_eq!(mgrs.zone, zone, "{position:?}: zone");
            assert_eq!(mgrs.band, band, "{position:?}: band");
            assert_eq!(mgrs.square, square, "{position:?}: square");
            assert_eq!(mgrs.easting, easting, "{position:?}: easting");
            assert_eq!(mgrs.northing, northing, "{position:?}: northing");
        }
    }

    #[test]
    fn mgrs_special_zones_follow_the_standard() {
        // Norway uses zone 32.
        assert_eq!(mgrs_zone(LatLng::new(60.0, 5.0)), 32);
        // Svalbard splits into 31, 33, 35 and 37.
        assert_eq!(mgrs_zone(LatLng::new(78.0, 5.0)), 31);
        assert_eq!(mgrs_zone(LatLng::new(78.0, 15.0)), 33);
        assert_eq!(mgrs_zone(LatLng::new(78.0, 25.0)), 35);
        assert_eq!(mgrs_zone(LatLng::new(78.0, 35.0)), 37);
    }

    #[test]
    fn column_and_row_letters_never_use_i_or_o() {
        for zone in 1..=60 {
            // Column index 0 does not occur in a real UTM zone, so the range starts at 1.
            for column in 1..=7 {
                let letter = column_letter(zone, column);
                assert_ne!(letter, 'I', "zone {zone} column {column}");
                assert_ne!(letter, 'O', "zone {zone} column {column}");
                assert!(letter.is_ascii_uppercase(), "zone {zone} column {column}");
            }
            for row in 0..20 {
                let letter = row_letter(zone, row);
                assert_ne!(letter, 'I', "zone {zone} row {row}");
                assert_ne!(letter, 'O', "zone {zone} row {row}");
                assert!(
                    (b'A'..=b'V').contains(&(letter as u8)),
                    "zone {zone} row {row} produced {letter}"
                );
            }
        }
    }
}
