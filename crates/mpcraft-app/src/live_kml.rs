//! The track of a flight that is being streamed to a file.
//!
//! Mission Planner can write the path of the vehicle to a KML file while it flies, so
//! that another program — Google Earth, most of the time — follows the flight as it
//! happens. The file is written again and again, and what it holds is always the path
//! so far.
//!
//! The track lives in the application, not in the shell: the shell only asks for it to
//! be started and stopped, and the link feeds it with the places the vehicle has been.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::error::{CommandError, CommandResult};

/// How far the vehicle must go for a place to be worth writing.
///
/// A vehicle that hovers writes the same place twenty times a second, and a file of
/// twenty places a second is a file nobody can open.
const MINIMUM_STEP_METRES: f64 = 2.0;

/// How often the file is written.
const WRITE_INTERVAL: Duration = Duration::from_secs(1);

/// How many metres a degree of latitude is worth.
const METRES_PER_DEGREE: f64 = 111_320.0;

/// The track of a flight, written to a file as the vehicle flies.
pub struct LiveTrack {
    /// The file the track is written to.
    path: PathBuf,
    /// The places the vehicle has been: latitude, longitude and height.
    points: Vec<(f64, f64, f64)>,
    /// When the file was last written.
    last_write: Option<Instant>,
}

impl LiveTrack {
    /// Starts a track that is written to the given file.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            points: Vec::new(),
            last_write: None,
        }
    }

    /// The file the track is written to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many places the track holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Whether the track holds no places yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Adds a place of the track.
    ///
    /// The place is kept when the vehicle has moved far enough from the last one. The
    /// answer says whether the track grew: a place that is not kept is not a place of
    /// the flight.
    pub fn push(&mut self, latitude: f64, longitude: f64, altitude: f64) -> bool {
        if let Some((last_lat, last_lng, _)) = self.points.last() {
            if distance(*last_lat, *last_lng, latitude, longitude) < MINIMUM_STEP_METRES {
                return false;
            }
        }

        self.points.push((latitude, longitude, altitude));
        true
    }

    /// Writes the file, if it is time to.
    ///
    /// The answer says whether the file was written.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be written.
    pub fn write(&mut self) -> CommandResult<bool> {
        let now = Instant::now();

        if let Some(last) = self.last_write {
            if now.duration_since(last) < WRITE_INTERVAL {
                return Ok(false);
            }
        }

        self.flush()?;
        self.last_write = Some(now);
        Ok(true)
    }

    /// Writes the file now.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be written.
    pub fn flush(&mut self) -> CommandResult<()> {
        let document = self.document();

        std::fs::write(&self.path, document).map_err(|error| {
            CommandError::internal(format!(
                "cannot write the track to {}: {error}",
                self.path.display()
            ))
        })
    }

    /// The document that is written to the file.
    #[must_use]
    pub fn document(&self) -> String {
        mpcraft_mission::kml::write_track_kml("MPCraft track", &self.points)
    }
}

/// How far apart two places are, in metres.
///
/// The distance is good enough for the purpose: it decides whether a place of the
/// track is worth writing, and not where the vehicle is.
fn distance(lat_a: f64, lng_a: f64, lat_b: f64, lng_b: f64) -> f64 {
    let north = (lat_b - lat_a) * METRES_PER_DEGREE;
    let middle = (lat_a + lat_b) / 2.0;
    let east = (lng_b - lng_a) * METRES_PER_DEGREE * middle.to_radians().cos();

    (north * north + east * east).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_place_is_always_kept() {
        let mut track = LiveTrack::new(PathBuf::from("track.kml"));
        assert!(track.push(-35.28, 149.13, 584.0));
        assert_eq!(track.len(), 1);
    }

    #[test]
    fn a_place_the_vehicle_has_not_moved_to_is_not_kept() {
        let mut track = LiveTrack::new(PathBuf::from("track.kml"));
        track.push(-35.28, 149.13, 584.0);

        // A hundredth of a degree of latitude is about a kilometre, but a place that is
        // the same place twice is not a place of the flight.
        assert!(!track.push(-35.2800001, 149.13, 584.0));
        assert_eq!(track.len(), 1);
    }

    #[test]
    fn a_place_the_vehicle_has_moved_to_is_kept() {
        let mut track = LiveTrack::new(PathBuf::from("track.kml"));
        track.push(-35.28, 149.13, 584.0);

        // A thousandth of a degree is about a hundred metres.
        assert!(track.push(-35.281, 149.13, 584.0));
        assert_eq!(track.len(), 2);
    }

    #[test]
    fn the_document_is_a_kml_document_with_the_path() {
        let mut track = LiveTrack::new(PathBuf::from("track.kml"));
        track.push(-35.28, 149.13, 584.0);
        track.push(-35.281, 149.131, 600.0);

        let document = track.document();

        assert!(document.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(document.contains("<LineString><coordinates>"));
        assert!(document.contains("149.13000000,-35.28000000,584.000000"));
        assert!(document.contains("149.13100000,-35.28100000,600.000000"));
    }

    #[test]
    fn the_file_is_written_and_read_back() {
        let directory = std::env::temp_dir().join("mpcraft-live-track-test");
        std::fs::create_dir_all(&directory).expect("the directory is created");
        let path = directory.join("track.kml");

        let mut track = LiveTrack::new(path.clone());
        track.push(-35.28, 149.13, 584.0);
        track.flush().expect("the file is written");

        let written = std::fs::read_to_string(&path).expect("the file is there");
        assert!(written.contains("<LineString><coordinates>"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_file_that_cannot_be_written_is_an_error_and_not_a_panic() {
        let mut track = LiveTrack::new(PathBuf::from("this/does/not/exist/track.kml"));
        track.push(-35.28, 149.13, 584.0);

        let error = track.flush().expect_err("the file cannot be written");
        assert_eq!(error.code, CommandError::CODE_INTERNAL);
    }
}
