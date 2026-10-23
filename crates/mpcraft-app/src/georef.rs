//! The pictures of a flight, and where they were taken.
//!
//! A camera writes where it was when it made a picture, but the clock of a camera is not the
//! clock of the ground station: the two are apart by whatever the camera's clock was showing.
//! Mission Planner works that difference out from the pictures and the log together
//! (`EstimateOffset`) and then gives every picture the place the log says the vehicle was at its
//! instant (`doworkGPSOFFSET`). MPCraft does the same, and writes the places back into copies of
//! the pictures.
//!
//! # What is kept apart
//!
//! The place a picture says is what the file itself carries; the place the log says is what the
//! flight says. They are shown next to each other rather than merged, because a picture whose
//! instant cannot be found in the log has no second place, and saying so is better than putting
//! the first one twice.

use std::path::Path;

use serde::{Deserialize, Serialize};

use mpcraft_geo::georef;
use mpcraft_geo::GeoPoint;
use mpcraft_log::tlog::TlogReader;

use crate::error::{CommandError, CommandResult};

/// The furthest a picture can be from a sample of the log and still be matched with it, in
/// seconds. Upstream uses two seconds, in `LookForLocation`.
pub const MATCH_WINDOW_SECONDS: f64 = 2.0;

/// The pictures at each end of a folder that are used to work the clock of the camera out:
/// upstream takes the first three and the last three, in `EstimateOffset`.
const ENDS: usize = 3;

/// The extensions of the files that are read as pictures, which are the ones upstream reads
/// (`PHOTO_FILES_FILTER`).
const PICTURE_EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "tif", "tiff"];

/// A picture, with the place it says it was taken and the place the log says the vehicle was
/// when it was made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PicturePlace {
    /// The name of the file, without the folder it is in.
    pub name: String,
    /// The width of the picture, in pixels.
    pub width: u32,
    /// The height of the picture, in pixels.
    pub height: u32,
    /// The way the picture is turned, as the camera wrote it.
    pub orientation: String,
    /// When the camera says the picture was made, in seconds since the epoch.
    pub taken_at: Option<i64>,
    /// The place the file itself says the picture was taken.
    pub place: Option<GeoPoint>,
    /// The instant of the sample of the log the picture was matched with, in seconds since the
    /// epoch.
    pub matched_at: Option<f64>,
    /// How far the picture's own instant was from that sample, in seconds.
    pub away_seconds: Option<f64>,
    /// The place the log says the vehicle was at that instant.
    pub matched_place: Option<GeoPoint>,
}

/// One of the places the log says the vehicle was, with the instant it was there.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogPlace {
    /// The instant, in seconds since the epoch.
    pub at: f64,
    /// The place.
    pub place: GeoPoint,
}

/// The pictures of a folder, read one by one.
///
/// # Errors
///
/// Returns an error when the folder cannot be read, or when a picture inside it is not one that
/// can be read.
pub fn read_pictures(folder: &Path) -> CommandResult<Vec<PicturePlace>> {
    let entries = std::fs::read_dir(folder).map_err(|error| {
        CommandError::invalid_request(format!("the folder of pictures could not be read: {error}"))
    })?;

    let mut pictures = Vec::new();

    for entry in entries {
        let path = entry
            .map_err(|error| {
                CommandError::invalid_request(format!(
                    "a file of the folder could not be read: {error}"
                ))
            })?
            .path();

        if !path.is_file() {
            continue;
        }

        let extension = match path.extension().and_then(|extension| extension.to_str()) {
            Some(extension) => extension.to_lowercase(),
            None => continue,
        };

        if !PICTURE_EXTENSIONS.contains(&extension.as_str()) {
            continue;
        }

        let bytes = std::fs::read(&path).map_err(|error| {
            CommandError::invalid_request(format!(
                "the picture {} could not be read: {error}",
                path.display()
            ))
        })?;

        let picture = georef::GeoreferencedImage::parse(&bytes).map_err(|error| {
            CommandError::invalid_request(format!(
                "the picture {} could not be read: {error}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ))
        })?;

        pictures.push(PicturePlace {
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            width: picture.width,
            height: picture.height,
            orientation: picture.orientation.name().to_string(),
            taken_at: picture.taken_at,
            place: picture.position,
            matched_at: None,
            away_seconds: None,
            matched_place: None,
        });
    }

    // The pictures are read in the order the folder gives them, which is not the order they
    // were made in. A ground station shows a flight in the order it happened.
    pictures.sort_by(|first, second| {
        first
            .taken_at
            .cmp(&second.taken_at)
            .then_with(|| first.name.cmp(&second.name))
    });

    Ok(pictures)
}

/// The places the log says the vehicle was, with the instants.
///
/// # Errors
///
/// Returns an error when the log cannot be read, or when it does not hold one.
pub fn log_places(path: &Path) -> CommandResult<Vec<LogPlace>> {
    let file = std::fs::File::open(path).map_err(|error| {
        CommandError::invalid_request(format!("the log could not be opened: {error}"))
    })?;

    let mut reader = TlogReader::new(file);
    let mut places = Vec::new();

    while let Some(record) = reader
        .next_record()
        .map_err(|error| CommandError::invalid_request(error.to_string()))?
    {
        let place = match &record.message {
            mpcraft_mavlink::MavMessage::GLOBAL_POSITION_INT(position) => Some(GeoPoint::with_alt(
                f64::from(position.lat) / 1e7,
                f64::from(position.lon) / 1e7,
                f64::from(position.alt) / 1e3,
            )),
            mpcraft_mavlink::MavMessage::GPS_RAW_INT(raw) => Some(GeoPoint::with_alt(
                f64::from(raw.lat) / 1e7,
                f64::from(raw.lon) / 1e7,
                f64::from(raw.alt) / 1e3,
            )),
            _ => None,
        };

        if let Some(place) = place {
            places.push(LogPlace {
                at: record.timestamp as f64 / 1e6,
                place,
            });
        }
    }

    places.sort_by(|first, second| {
        first
            .at
            .partial_cmp(&second.at)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(places)
}

/// The difference between the clock of the camera and the clock of the log, in seconds.
///
/// The pictures at both ends of a folder are compared with the samples of the log at the same
/// positions, and each comparison is given half of what was worked out before it. A folder whose
/// instants cannot be lined up with a log has nothing to say, and `None` is said rather than a
/// number that is guessed.
pub fn estimate_offset(pictures: &[PicturePlace], places: &[LogPlace]) -> Option<f64> {
    let instants: Vec<i64> = pictures
        .iter()
        .filter_map(|picture| picture.taken_at)
        .collect();

    if instants.is_empty() || places.is_empty() {
        return None;
    }

    let ends = ENDS.min(instants.len());
    let mut offset: Option<f64> = None;

    for index in (0..ends).chain((instants.len() - ends)..instants.len()) {
        let sample = match places.get(index) {
            Some(sample) => sample,
            None => continue,
        };

        let difference = instants[index] as f64 - sample.at;

        offset = Some(match offset {
            None => difference,
            Some(before) => before * 0.5 + difference * 0.5,
        });
    }

    offset
}

/// The pictures, with the place the log says the vehicle was when each of them was made.
///
/// `lag` is what the camera took between being told to take a picture and taking it, which
/// upstream calls the shutter lag. A picture whose instant is more than [`MATCH_WINDOW_SECONDS`]
/// from any sample of the log is left without a second place: the log does not say where the
/// vehicle was, and MPCraft does not say that it does.
pub fn match_pictures(
    pictures: &[PicturePlace],
    places: &[LogPlace],
    offset: f64,
    lag: f64,
) -> Vec<PicturePlace> {
    pictures
        .iter()
        .map(|picture| {
            let mut matched = picture.clone();

            // What was worked out says how far ahead the clock of the camera is, so it is taken
            // off the instant the picture carries, which puts the picture on the clock of the
            // log — the same thing upstream does in `doworkGPSOFFSET`. The shutter lag is added
            // to it, because the picture was made after the camera was told to make it.
            let taken_at = match picture.taken_at {
                Some(taken_at) => taken_at as f64 - offset + lag,
                None => return matched,
            };

            let nearest = places.iter().min_by(|first, second| {
                (first.at - taken_at)
                    .abs()
                    .total_cmp(&(second.at - taken_at).abs())
            });

            match nearest {
                Some(sample) if (sample.at - taken_at).abs() <= MATCH_WINDOW_SECONDS => {
                    matched.matched_at = Some(sample.at);
                    matched.away_seconds = Some(taken_at - sample.at);
                    matched.matched_place = Some(sample.place);
                }
                _ => {}
            }

            matched
        })
        .collect()
}

/// Copies of the pictures, with their places written into them, in the output folder.
///
/// A picture is written with the place the log says the vehicle was when it was made, and with
/// the place the file itself carries when the flight says nothing about that instant. A picture
/// with neither is left where it is: writing a place that is not known would put something in the
/// file that is not true.
///
/// # Errors
///
/// Returns an error when the folder or the output cannot be read or written.
pub fn write_geotagged(
    folder: &Path,
    output: &Path,
    pictures: &[PicturePlace],
) -> CommandResult<usize> {
    std::fs::create_dir_all(output).map_err(|error| {
        CommandError::internal(format!(
            "the folder for the copies could not be made: {error}"
        ))
    })?;

    let mut written = 0_usize;

    for picture in pictures {
        let place = match picture.matched_place.or(picture.place) {
            Some(place) => place,
            None => continue,
        };

        let bytes = std::fs::read(folder.join(&picture.name)).map_err(|error| {
            CommandError::internal(format!(
                "the picture {} could not be read: {error}",
                picture.name
            ))
        })?;

        let copy = georef::geotagged(&bytes, picture.taken_at, place).map_err(|error| {
            CommandError::invalid_request(format!(
                "the picture {} could not be written: {error}",
                picture.name
            ))
        })?;

        std::fs::write(output.join(&picture.name), copy).map_err(|error| {
            CommandError::internal(format!(
                "the copy of {} could not be written: {error}",
                picture.name
            ))
        })?;

        written += 1;
    }

    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(name: &str, taken_at: i64) -> PicturePlace {
        PicturePlace {
            name: name.to_string(),
            width: 640,
            height: 480,
            orientation: String::new(),
            taken_at: Some(taken_at),
            place: None,
            matched_at: None,
            away_seconds: None,
            matched_place: None,
        }
    }

    fn place(at: f64, lat: f64, lng: f64) -> LogPlace {
        LogPlace {
            at,
            place: GeoPoint::with_alt(lat, lng, 0.0),
        }
    }

    #[test]
    fn the_clock_of_the_camera_is_worked_out_from_both_ends_of_a_folder() {
        let pictures: Vec<PicturePlace> = (0..8)
            .map(|index| picture(&format!("p{index}.jpg"), 1_000 + index as i64 * 10))
            .collect();

        // Every sample of the log is at the same instant, so the difference at each end is what
        // the picture says, and only the first three and the last three are used.
        let places = vec![place(990.0, 0.0, 0.0); 8];

        assert_eq!(estimate_offset(&pictures, &places), Some(67.812_5));
    }

    #[test]
    fn a_folder_that_cannot_be_lined_up_with_a_log_says_nothing() {
        assert_eq!(estimate_offset(&[], &[place(1.0, 0.0, 0.0)]), None);
    }

    #[test]
    fn a_picture_is_matched_with_the_sample_of_the_log_it_is_closest_to() {
        let pictures = [picture("p1.jpg", 1_000)].to_vec();
        let places = [place(1_000.4, 35.28, 149.13), place(1_001.9, 35.29, 149.14)].to_vec();

        let matched = match_pictures(&pictures, &places, 0.0, 0.0);
        let first = &matched[0];

        assert_eq!(first.matched_at, Some(1_000.4));
        assert!(
            first
                .away_seconds
                .is_some_and(|away| (away + 0.4).abs() < 1e-9),
            "away {:?}",
            first.away_seconds
        );
        assert_eq!(first.matched_place.map(|point| point.lat), Some(35.28));
    }

    #[test]
    fn a_picture_that_is_more_than_the_window_from_the_log_is_not_matched() {
        let pictures = [picture("p1.jpg", 1_000)].to_vec();
        let places = [place(1_002.1, 35.28, 149.13)].to_vec();

        let matched = match_pictures(&pictures, &places, 0.0, 0.0);

        assert_eq!(matched[0].matched_at, None);
        assert_eq!(matched[0].matched_place, None);
    }

    #[test]
    fn the_shutter_lag_moves_the_picture_to_the_instant_it_was_taken() {
        let pictures = [picture("p1.jpg", 1_000)].to_vec();
        let places = [place(1_001.5, 35.28, 149.13)].to_vec();

        let matched = match_pictures(&pictures, &places, 0.0, 1.5);

        assert_eq!(matched[0].matched_at, Some(1_001.5));
        assert_eq!(matched[0].away_seconds, Some(0.0));
    }

    #[test]
    fn a_camera_ahead_of_the_log_is_matched_by_taking_the_difference_off_its_clock() {
        // The picture says it was made sixty seconds after the sample of the log, which is what
        // `estimate_offset` would have answered for it. The difference is taken off the instant
        // the picture carries, so the picture lands on the sample and not two minutes away from
        // it — the sign upstream's `doworkGPSOFFSET` uses.
        let pictures = [picture("p1.jpg", 1_060)].to_vec();
        let places = [place(1_000.0, 35.28, 149.13)].to_vec();

        let matched = match_pictures(&pictures, &places, 60.0, 0.0);

        assert_eq!(matched[0].matched_at, Some(1_000.0));
        assert_eq!(matched[0].away_seconds, Some(0.0));
        assert_eq!(matched[0].matched_place.map(|point| point.lat), Some(35.28));
    }
}
