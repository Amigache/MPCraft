//! The route that follows the ground.
//!
//! A survey is flown over ground that is not flat: the vehicle has to stay a certain height
//! above what is under it, and the plan has to say how high that is at every point. Mission
//! Planner draws such a route with the terrain of the place — the SRTM tiles it downloads, or
//! the DTED files of the operator — and it cuts the route where the terrain data ends.
//!
//! Two things are here:
//!
//! * [`terrain_following_path`] draws the zigzag of a survey: the strips the vehicle flies
//!   one after the other, apart by the distance the sensor sweeps, each of them at a fixed
//!   height above the ground.
//! * [`intersection_with_terrain`] answers the question of where a leg that was planned at a
//!   fixed height meets the ground, which is where the leg has to be cut.
//!
//! The ground comes from a [`TerrainProvider`], so the same route can be drawn with the
//! terrain of the service or with a DTED file of the operator.

use crate::projection::LatLng;
use crate::terrain::{TerrainError, TerrainProvider};
use crate::{bearing_deg, distance_m, offset, GeoPoint};

/// The distance between two samples of a strip when the caller does not say.
const DEFAULT_STEP_M: f64 = 50.0;

/// A point of a route that follows the ground.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainPoint {
    /// Where the point is.
    pub position: LatLng,
    /// The height of the ground under the point, in metres above the sea.
    pub ground_m: f64,
    /// The height the vehicle flies at, in metres above the sea.
    pub alt_m: f64,
}

impl TerrainPoint {
    /// Creates a point of a route.
    #[must_use]
    pub const fn new(position: LatLng, ground_m: f64, alt_m: f64) -> Self {
        Self {
            position,
            ground_m,
            alt_m,
        }
    }
}

/// How a survey that follows the ground is laid out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainPathOptions {
    /// The length of one strip, in metres.
    pub strip_length_m: f64,
    /// The distance between the middle of one strip and the middle of the next, in metres.
    ///
    /// This is the distance the sensor sweeps: the strips have to be apart by as much, or the
    /// ground is surveyed twice.
    pub strip_spacing_m: f64,
    /// The height above the ground the vehicle flies at, in metres.
    pub clearance_m: f64,
    /// The distance between two points of a strip, in metres.
    pub sample_step_m: f64,
}

impl Default for TerrainPathOptions {
    fn default() -> Self {
        Self {
            strip_length_m: 1000.0,
            strip_spacing_m: 200.0,
            clearance_m: 30.0,
            sample_step_m: DEFAULT_STEP_M,
        }
    }
}

/// The zigzag of a survey that follows the ground.
///
/// The strips run across the line that joins the two places given, one after the other, and
/// each of them is walked in the opposite direction to the one before — the zigzag of a
/// mower. The height of every point is the height of the ground under it plus the clearance
/// asked for.
///
/// A strip is cut where the terrain data ends: the points up to the edge of the data are
/// kept, and the survey stops there, because beyond it the ground is not known and the
/// vehicle could not be told how high to fly.
///
/// # Errors
///
/// Returns an error when the sizes asked for make no sense, or when the terrain cannot be
/// read.
///
/// # Example
///
/// ```
/// use mpcraft_geo::terrain::{Elevation, ElevationSource, TerrainError, TerrainProvider};
/// use mpcraft_geo::terrain_path::{TerrainPathOptions, terrain_following_path};
/// use mpcraft_geo::projection::LatLng;
///
/// /// A ground that is flat at fifty metres.
/// struct Flat;
///
/// impl TerrainProvider for Flat {
///     fn elevation(
///         &self,
///         _position: LatLng,
///     ) -> Result<Elevation, TerrainError> {
///         Ok(Elevation { metres: 50.0, source: ElevationSource::Srtm3 })
///     }
/// }
///
/// let options = TerrainPathOptions {
///     strip_length_m: 400.0,
///     strip_spacing_m: 200.0,
///     clearance_m: 30.0,
///     sample_step_m: 100.0,
/// };
///
/// let route = terrain_following_path(
///     &Flat,
///     LatLng::new(-35.28, 149.13),
///     LatLng::new(-35.30, 149.13),
///     options,
/// )
/// .expect("the route is drawn");
///
/// assert!(!route.is_empty());
/// assert!(route.iter().all(|point| point.alt_m == 80.0));
/// ```
pub fn terrain_following_path(
    provider: &dyn TerrainProvider,
    from: LatLng,
    to: LatLng,
    options: TerrainPathOptions,
) -> Result<Vec<TerrainPoint>, TerrainError> {
    if options.strip_length_m <= 0.0 {
        return Err(TerrainError::InvalidRequest(
            "the length of a strip must be greater than zero".to_owned(),
        ));
    }

    if options.strip_spacing_m <= 0.0 {
        return Err(TerrainError::InvalidRequest(
            "the distance between the strips must be greater than zero".to_owned(),
        ));
    }

    if options.sample_step_m <= 0.0 {
        return Err(TerrainError::InvalidRequest(
            "the distance between the points of a strip must be greater than zero".to_owned(),
        ));
    }

    let spine_bearing = bearing_deg(point(from), point(to));
    let spine_length = distance_m(point(from), point(to));
    let across = (spine_bearing + 90.0).rem_euclid(360.0);

    let strips = (spine_length / options.strip_spacing_m).floor() as usize + 1;
    let half = options.strip_length_m / 2.0;

    let mut route = Vec::new();

    for index in 0..strips {
        let anchor = offset(
            point(from),
            spine_bearing,
            index as f64 * options.strip_spacing_m,
        );

        // The strips are walked one way and then the other.
        let direction = if index % 2 == 0 {
            across
        } else {
            (across + 180.0).rem_euclid(360.0)
        };

        let start = offset(anchor, (direction + 180.0).rem_euclid(360.0), half);
        let end = offset(anchor, direction, half);

        let strip = follow_strip(provider, start, end, &options)?;

        // The ground has ended: there is nothing more to survey.
        if strip.is_empty() {
            break;
        }

        let cut = strip.len() < points_of_strip(&options);

        route.extend(strip);

        if cut {
            break;
        }
    }

    Ok(route)
}

/// The number of points a strip of the given size holds.
fn points_of_strip(options: &TerrainPathOptions) -> usize {
    let steps = (options.strip_length_m / options.sample_step_m).ceil() as usize;

    steps + 1
}

/// Walks one strip, from one end to the other, at the height of the ground.
fn follow_strip(
    provider: &dyn TerrainProvider,
    start: GeoPoint,
    end: GeoPoint,
    options: &TerrainPathOptions,
) -> Result<Vec<TerrainPoint>, TerrainError> {
    let steps = points_of_strip(options);
    let bearing = bearing_deg(start, end);

    let mut points = Vec::with_capacity(steps);

    for step in 0..steps {
        let distance = step as f64 * options.sample_step_m;
        let here = offset(start, bearing, distance);

        let position = LatLng::new(here.lat, here.lng);

        // The edge of the terrain: the strip stops here.
        let Ok(ground) = provider.elevation(position) else {
            break;
        };

        points.push(TerrainPoint::new(
            position,
            ground.metres,
            ground.metres + options.clearance_m,
        ));
    }

    Ok(points)
}

/// A leg that follows the ground, from one place to another.
///
/// The leg is sampled every `step_m` metres and every point is at the height of the ground
/// under it plus the clearance asked for. The leg stops where the terrain data ends.
///
/// # Errors
///
/// Returns an error when the terrain cannot be read.
pub fn terrain_following_leg(
    provider: &dyn TerrainProvider,
    from: LatLng,
    to: LatLng,
    clearance_m: f64,
    step_m: f64,
) -> Result<Vec<TerrainPoint>, TerrainError> {
    if step_m <= 0.0 {
        return Err(TerrainError::InvalidRequest(
            "the distance between the points of the leg must be greater than zero".to_owned(),
        ));
    }

    let options = TerrainPathOptions {
        strip_length_m: distance_m(point(from), point(to)),
        strip_spacing_m: 1.0,
        clearance_m,
        sample_step_m: step_m,
    };

    follow_strip(provider, point(from), point(to), &options)
}

/// Where a leg that was planned at a fixed height meets the ground.
///
/// The leg is walked in steps of `step_m` metres and the height of the ground is looked up
/// at both ends of every step. The first step where the straight line of the leg — from
/// `start_alt_m` to `end_alt_m` — crosses the ground is the place where the leg has to be
/// cut. When the leg never meets the ground, the whole leg is returned.
///
/// Where the terrain data ends, the ground is taken to be at the level of the sea, as
/// Mission Planner does: beyond the edge of the data the ground is not known to be clear.
///
/// # Errors
///
/// Returns an error when the sizes asked for make no sense.
pub fn intersection_with_terrain(
    provider: &dyn TerrainProvider,
    start: LatLng,
    end: LatLng,
    start_alt_m: f64,
    end_alt_m: f64,
    step_m: f64,
) -> Result<TerrainPoint, TerrainError> {
    if step_m <= 0.0 {
        return Err(TerrainError::InvalidRequest(
            "the distance between the samples of the leg must be greater than zero".to_owned(),
        ));
    }

    let from = point(start);
    let to = point(end);

    let length = distance_m(from, to);
    let bearing = bearing_deg(from, to);

    let mut walked = 0.0;

    while walked < length {
        let first = offset(from, bearing, walked);
        let second = offset(from, bearing, walked + step_m);

        let ground_first = ground_or_sea(provider, LatLng::new(first.lat, first.lng));
        let ground_second = ground_or_sea(provider, LatLng::new(second.lat, second.lng));

        let flight = (0.0, start_alt_m, length, end_alt_m);
        let ground = (walked, ground_first, walked + step_m, ground_second);

        if let Some((distance, altitude)) = crossing(flight, ground) {
            let here = offset(from, bearing, distance);

            return Ok(TerrainPoint::new(
                LatLng::new(here.lat, here.lng),
                altitude,
                altitude,
            ));
        }

        walked += step_m;
    }

    Ok(TerrainPoint::new(
        end,
        ground_or_sea(provider, end),
        end_alt_m,
    ))
}

/// The height of the ground at a place, or the level of the sea when it is not known.
fn ground_or_sea(provider: &dyn TerrainProvider, position: LatLng) -> f64 {
    provider
        .elevation(position)
        .map(|elevation| elevation.metres)
        .unwrap_or(0.0)
}

/// Where two straight lines cross, when the crossing is on both of them.
///
/// The two lines are given as `(x1, y1, x2, y2)`.
fn crossing(first: (f64, f64, f64, f64), second: (f64, f64, f64, f64)) -> Option<(f64, f64)> {
    let (x1, y1, x2, y2) = first;
    let (x3, y3, x4, y4) = second;

    let denominator = (x2 - x1) * (y4 - y3) - (y2 - y1) * (x4 - x3);

    // The two lines are parallel, or the same one.
    if denominator == 0.0 {
        return None;
    }

    let along_first = ((x3 - x1) * (y4 - y3) - (y3 - y1) * (x4 - x3)) / denominator;
    let along_second = ((x3 - x1) * (y2 - y1) - (y3 - y1) * (x2 - x1)) / denominator;

    if !(0.0..=1.0).contains(&along_first) || !(0.0..=1.0).contains(&along_second) {
        return None;
    }

    Some((x1 + along_first * (x2 - x1), y1 + along_first * (y2 - y1)))
}

/// Turns a position into the point of the geodesy of the crate.
fn point(position: LatLng) -> GeoPoint {
    GeoPoint::new(position.lat, position.lng)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{Elevation, ElevationSource};

    /// A ground that is flat at a given height.
    struct Flat {
        height: f64,
    }

    impl TerrainProvider for Flat {
        fn elevation(&self, _position: LatLng) -> Result<Elevation, TerrainError> {
            Ok(Elevation {
                metres: self.height,
                source: ElevationSource::Srtm3,
            })
        }
    }

    /// A ground that rises towards the north, and that ends at a given latitude.
    struct Slope {
        /// The height at the southern edge.
        base: f64,
        /// The height for every degree of latitude.
        per_degree: f64,
        /// The northern edge of the ground.
        limit: f64,
    }

    impl TerrainProvider for Slope {
        fn elevation(&self, position: LatLng) -> Result<Elevation, TerrainError> {
            if position.lat > self.limit {
                return Err(TerrainError::NoData {
                    lat: position.lat,
                    lng: position.lng,
                });
            }

            Ok(Elevation {
                metres: self.base + (position.lat + 35.0) * self.per_degree,
                source: ElevationSource::Srtm3,
            })
        }
    }

    #[test]
    fn the_route_of_a_flat_ground_is_at_the_clearance() {
        let options = TerrainPathOptions {
            strip_length_m: 400.0,
            strip_spacing_m: 200.0,
            clearance_m: 30.0,
            sample_step_m: 100.0,
        };

        let route = terrain_following_path(
            &Flat { height: 50.0 },
            LatLng::new(-35.30, 149.10),
            LatLng::new(-35.30, 149.12),
            options,
        )
        .expect("the route is drawn");

        // The spine is about 1 800 metres long, so there are ten strips of five points.
        assert_eq!(route.len(), 50);

        for point in &route {
            assert!((point.ground_m - 50.0).abs() < 1e-9);
            assert!((point.alt_m - 80.0).abs() < 1e-9);
        }
    }

    #[test]
    fn the_strips_of_the_route_are_apart_and_alternate() {
        let options = TerrainPathOptions {
            strip_length_m: 400.0,
            strip_spacing_m: 200.0,
            clearance_m: 30.0,
            sample_step_m: 100.0,
        };

        let route = terrain_following_path(
            &Flat { height: 50.0 },
            LatLng::new(-35.30, 149.10),
            LatLng::new(-35.30, 149.12),
            options,
        )
        .expect("the route is drawn");

        let first_strip = &route[0..5];
        let second_strip = &route[5..10];

        // The strips lie at the distance asked for from one another.
        let apart = distance_m(
            point(first_strip[2].position),
            point(second_strip[2].position),
        );
        assert!((apart - 200.0).abs() < 1.0, "the strips were {apart} apart");

        // The second strip is walked the other way.
        let first_direction = first_strip[4].position.lat - first_strip[0].position.lat;
        let second_direction = second_strip[4].position.lat - second_strip[0].position.lat;

        assert!(first_direction.abs() > 1e-6, "the strip does not move");
        assert!(
            first_direction * second_direction < 0.0,
            "the two strips are walked the same way"
        );
    }

    #[test]
    fn the_route_of_a_sloping_ground_follows_the_ground() {
        let options = TerrainPathOptions {
            strip_length_m: 400.0,
            strip_spacing_m: 500.0,
            clearance_m: 20.0,
            sample_step_m: 100.0,
        };

        let route = terrain_following_path(
            &Slope {
                base: 100.0,
                per_degree: 1000.0,
                limit: 90.0,
            },
            LatLng::new(-35.30, 149.10),
            LatLng::new(-35.29, 149.10),
            options,
        )
        .expect("the route is drawn");

        assert!(!route.is_empty());

        for point in &route {
            let ground = 100.0 + (point.position.lat + 35.0) * 1000.0;

            assert!(
                (point.ground_m - ground).abs() < 1e-6,
                "the ground was {} and should be {ground}",
                point.ground_m
            );
            assert!((point.alt_m - point.ground_m - 20.0).abs() < 1e-6);
        }
    }

    #[test]
    fn the_route_stops_at_the_edge_of_the_terrain() {
        let options = TerrainPathOptions {
            strip_length_m: 400.0,
            strip_spacing_m: 200.0,
            clearance_m: 20.0,
            sample_step_m: 100.0,
        };

        // The ground ends at the latitude of -35.295, which cuts the strips in two.
        let route = terrain_following_path(
            &Slope {
                base: 100.0,
                per_degree: 1000.0,
                limit: -35.295,
            },
            LatLng::new(-35.30, 149.10),
            LatLng::new(-35.29, 149.10),
            options,
        )
        .expect("the route is drawn");

        assert!(!route.is_empty());

        for point in &route {
            assert!(
                point.position.lat <= -35.295,
                "the route went to {}",
                point.position.lat
            );
        }
    }

    #[test]
    fn a_leg_that_meets_the_ground_is_cut_at_the_crossing() {
        // The ground is flat at fifty metres and the leg is planned at ten.
        let crossing_point = intersection_with_terrain(
            &Flat { height: 50.0 },
            LatLng::new(-35.28, 149.13),
            LatLng::new(-35.28, 149.15),
            10.0,
            60.0,
            50.0,
        )
        .expect("the leg is walked");

        // The leg rises from ten to sixty metres over about 1 800 metres, and it meets the
        // ground fifty metres above the sea some 1 450 metres along.
        let walked = distance_m(
            point(LatLng::new(-35.28, 149.13)),
            point(crossing_point.position),
        );

        assert!(
            (walked - 1_450.0).abs() < 50.0,
            "the crossing was at {walked} metres"
        );
        assert!((crossing_point.alt_m - 50.0).abs() < 2.0);
    }

    #[test]
    fn a_leg_that_never_meets_the_ground_is_whole() {
        let end = LatLng::new(-35.28, 149.15);

        let crossing_point = intersection_with_terrain(
            &Flat { height: 50.0 },
            LatLng::new(-35.28, 149.13),
            end,
            100.0,
            120.0,
            50.0,
        )
        .expect("the leg is walked");

        assert_eq!(crossing_point.position, end);
        assert!((crossing_point.alt_m - 120.0).abs() < 1e-9);
    }

    #[test]
    fn a_leg_that_does_not_make_sense_is_refused() {
        let error = intersection_with_terrain(
            &Flat { height: 50.0 },
            LatLng::new(-35.28, 149.13),
            LatLng::new(-35.28, 149.15),
            100.0,
            120.0,
            0.0,
        )
        .expect_err("the step is not a step");

        assert!(matches!(error, TerrainError::InvalidRequest(_)));
    }

    #[test]
    fn a_route_that_does_not_make_sense_is_refused() {
        let error = terrain_following_path(
            &Flat { height: 50.0 },
            LatLng::new(-35.28, 149.13),
            LatLng::new(-35.28, 149.15),
            TerrainPathOptions {
                strip_spacing_m: 0.0,
                ..TerrainPathOptions::default()
            },
        )
        .expect_err("the strips would be on top of each other");

        assert!(matches!(error, TerrainError::InvalidRequest(_)));
    }
}
