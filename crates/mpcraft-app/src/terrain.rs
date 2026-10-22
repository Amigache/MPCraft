//! The terrain of the ground the vehicle flies over.
//!
//! There are two sources of terrain in MPCraft. The one that is always there is the
//! service of heights that is fetched and kept on the disk; the other is the folder of
//! DTED cells that the operator brings: the military format of the heights of the
//! ground, which is what a survey of a field uses.
//!
//! The folder is opened once and kept, and the path that follows the ground is worked
//! out from it. The path is the zigzag of a survey whose strips run at a constant
//! height above the ground, which is what a camera that needs the same distance to the
//! ground everywhere needs.

use std::path::Path;

use mpcraft_geo::dted::DtedLibrary;
use mpcraft_geo::projection::LatLng;
use mpcraft_geo::terrain::TerrainProvider;
use mpcraft_geo::terrain_path::{terrain_following_path, TerrainPathOptions, TerrainPoint};

use crate::error::{CommandError, CommandResult};

/// One cell of the folder of DTED files.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DtedCellInfo {
    /// The name of the file.
    pub name: String,
    /// The level of the cell: `DTED0`, `DTED1` or `DTED2`.
    pub level: String,
    /// The latitude of the south-west corner, in degrees.
    pub origin_lat: f64,
    /// The longitude of the south-west corner, in degrees.
    pub origin_lng: f64,
    /// The distance between two posts, in degrees.
    pub spacing_lat: f64,
    /// The distance between two columns, in degrees.
    pub spacing_lng: f64,
    /// How many columns the cell has.
    pub columns: usize,
    /// How many rows the cell has.
    pub rows: usize,
    /// How many checksums of the file did not add up.
    pub checksum_failures: usize,
}

/// What a folder of DTED files holds.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DtedLibraryInfo {
    /// The cells that were read.
    pub cells: Vec<DtedCellInfo>,
    /// How many files of the folder could not be read.
    pub failures: usize,
}

/// One point of a path that follows the ground.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerrainPathPoint {
    /// The latitude, in degrees.
    pub lat: f64,
    /// The longitude, in degrees.
    pub lng: f64,
    /// The height of the ground under the point, in metres.
    pub ground_m: f64,
    /// The height the vehicle flies at, in metres.
    pub alt_m: f64,
}

/// Reads a folder of DTED cells.
///
/// # Errors
///
/// Returns an error when the folder cannot be read, or when it holds no cell at all.
pub fn open_dted(directory: &Path) -> CommandResult<(DtedLibraryInfo, DtedLibrary)> {
    let library = DtedLibrary::from_directory(directory).map_err(|error| {
        CommandError::invalid_request(format!(
            "the folder of DTED cells could not be read: {error}"
        ))
    })?;

    if library.is_empty() {
        return Err(CommandError::invalid_request(format!(
            "the folder {} holds no DTED cell",
            directory.display()
        )));
    }

    let cells = library
        .cells()
        .iter()
        .map(|cell| {
            let header = cell.header();

            DtedCellInfo {
                name: cell
                    .path()
                    .and_then(|path| path.file_name())
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default(),
                level: format!("{:?}", cell.level()),
                origin_lat: header.origin_lat,
                origin_lng: header.origin_lng,
                spacing_lat: header.spacing_lat,
                spacing_lng: header.spacing_lng,
                columns: header.columns,
                rows: header.rows,
                checksum_failures: cell.checksum_failures(),
            }
        })
        .collect();

    let info = DtedLibraryInfo {
        cells,
        failures: library.failures(),
    };

    Ok((info, library))
}

/// The height of the ground at a place, in metres above the sea.
///
/// # Errors
///
/// Returns an error when no cell of the folder covers the place.
pub fn dted_elevation(library: &DtedLibrary, lat: f64, lng: f64) -> CommandResult<f64> {
    let elevation = library.elevation(LatLng::new(lat, lng)).map_err(|error| {
        CommandError::invalid_request(format!("the height of that place is not known: {error}"))
    })?;

    Ok(elevation.metres)
}

/// The zigzag of a survey that follows the ground.
///
/// The strips run across the line that joins the two places, one after the other, and
/// each of them is walked in the opposite direction to the one before. The vehicle
/// flies at a constant height above the ground.
///
/// # Errors
///
/// Returns an error when the ground under the path is not known.
pub fn terrain_path(
    library: &DtedLibrary,
    from: (f64, f64),
    to: (f64, f64),
    options: TerrainPathOptions,
) -> CommandResult<Vec<TerrainPathPoint>> {
    let points = terrain_following_path(
        library,
        LatLng::new(from.0, from.1),
        LatLng::new(to.0, to.1),
        options,
    )
    .map_err(|error| {
        CommandError::invalid_request(format!(
            "the path that follows the ground could not be worked out: {error}"
        ))
    })?;

    Ok(points.iter().map(point_of).collect())
}

/// One point of the path, as the interface sees it.
fn point_of(point: &TerrainPoint) -> TerrainPathPoint {
    TerrainPathPoint {
        lat: point.position.lat,
        lng: point.position.lng,
        ground_m: point.ground_m,
        alt_m: point.alt_m,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_that_does_not_exist_is_an_error_and_not_a_panic() {
        let error =
            open_dted(Path::new("there/is/no/such/folder")).expect_err("there is no folder");
        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);
    }

    #[test]
    fn a_folder_without_cells_is_an_error() {
        let directory = std::env::temp_dir().join("mpcraft-dted-empty");
        std::fs::create_dir_all(&directory).expect("the directory is created");

        let error = open_dted(&directory).expect_err("the folder holds no cell");
        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);

        std::fs::remove_dir(&directory).ok();
    }
}
