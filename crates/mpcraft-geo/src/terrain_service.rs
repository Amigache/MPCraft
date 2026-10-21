//! The service the terrain comes from.
//!
//! The service of ArduPilot serves the terrain as tiles of one degree, compressed. The
//! tiles are named after the place they cover, and they live in a folder named after
//! the part of the world they are in — the service has one folder per continent, and the
//! continent of a tile is not written down anywhere.
//!
//! The tiles that are fetched are kept on the disk, so that a plan is looked at once
//! and not once every time the operator moves a waypoint.

use std::path::{Path, PathBuf};

use crate::height::HeightGrid;
use crate::projection::LatLng;
use crate::terrain::{Elevation, ElevationSource, TerrainError};

/// The address of the service.
const BASE_URL: &str = "https://terrain.ardupilot.org/SRTM3";

/// The parts of the world the service keeps its tiles in.
///
/// The order is the one the tiles are most likely to be found in, because a tile that is
/// not where it is looked for costs a request.
const CONTINENTS: &[&str] = &[
    "Eurasia",
    "Africa",
    "North_America",
    "South_America",
    "Australia",
    "Islands",
    "Antarctic",
    "Arctic",
    "Misc",
];

/// The terrain, fetched from the service and kept on the disk.
#[derive(Debug, Clone)]
pub struct TerrainService {
    /// The directory the tiles are kept in.
    cache: PathBuf,
    /// The client the tiles are fetched with.
    client: reqwest::Client,
}

impl TerrainService {
    /// Creates a service that keeps its tiles under the given directory.
    #[must_use]
    pub fn new(cache: PathBuf) -> Self {
        Self {
            cache,
            client: reqwest::Client::new(),
        }
    }

    /// The directory the tiles are kept in.
    #[must_use]
    pub fn cache_dir(&self) -> &Path {
        &self.cache
    }

    /// The name of the tile that covers a position.
    ///
    /// The tiles are named after the south-west corner of the square they cover, and the
    /// hemisphere is the first letter: `S36E149` is the tile of the thirty-six degrees
    /// south and the hundred and forty-nine degrees east.
    #[must_use]
    pub fn tile_name(position: LatLng) -> String {
        let lat = position.lat.floor() as i32;
        let lng = position.lng.floor() as i32;

        let hemisphere = if lat >= 0 { 'N' } else { 'S' };
        let other = if lng >= 0 { 'E' } else { 'W' };

        format!("{hemisphere}{:02}{other}{:03}", lat.abs(), lng.abs())
    }

    /// The elevation at a position, in metres above the sea.
    ///
    /// # Errors
    ///
    /// Returns an error when the service cannot be reached, when the tile holds no data
    /// at the position, or when the tile cannot be read.
    pub async fn elevation(&self, position: LatLng) -> Result<Elevation, TerrainError> {
        let name = Self::tile_name(position);

        let grid = self.grid(&name, position).await?;

        grid.elevation_at(position.lat, position.lng)
    }

    /// The elevations along a path.
    ///
    /// # Errors
    ///
    /// Returns an error if any sample could not be resolved.
    pub async fn elevation_path(&self, path: &[LatLng]) -> Result<Vec<Elevation>, TerrainError> {
        let mut found = Vec::with_capacity(path.len());

        for position in path {
            found.push(self.elevation(*position).await?);
        }

        Ok(found)
    }

    /// The grid of the tile, from the cache or from the service.
    async fn grid(&self, name: &str, position: LatLng) -> Result<HeightGrid, TerrainError> {
        let cached = self.cache.join(format!("{name}.hgt"));

        if let Ok(bytes) = tokio::fs::read(&cached).await {
            if let Ok(grid) = HeightGrid::from_bytes(
                &bytes,
                position.lat.floor(),
                position.lng.floor(),
                ElevationSource::Srtm3,
            ) {
                return Ok(grid);
            }
        }

        let bytes = self.fetch_tile(name).await?;

        // The tile is kept, so that the next look at the same place is a look at the
        // disk and not at the service.
        if let Some(parent) = cached.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        let _ = tokio::fs::write(&cached, &bytes).await;

        HeightGrid::from_bytes(
            &bytes,
            position.lat.floor(),
            position.lng.floor(),
            ElevationSource::Srtm3,
        )
    }

    /// Fetches a tile from the service.
    async fn fetch_tile(&self, name: &str) -> Result<Vec<u8>, TerrainError> {
        let file = format!("{name}.hgt.zip");

        for continent in CONTINENTS {
            let url = format!("{BASE_URL}/{continent}/{file}");

            let Ok(response) = self.client.head(&url).send().await else {
                continue;
            };

            if !response.status().is_success() {
                continue;
            }

            let response = self
                .client
                .get(&url)
                .send()
                .await
                .map_err(|error| TerrainError::Unavailable(error.to_string()))?;

            if !response.status().is_success() {
                continue;
            }

            let bytes = response
                .bytes()
                .await
                .map_err(|error| TerrainError::Unavailable(error.to_string()))?;

            return crate::zip::read_tile(&bytes);
        }

        Err(TerrainError::Unavailable(format!(
            "the service has no tile called {name}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The elevation of a place, from the service.
    ///
    /// The test reaches the network, so it is left out of the usual run: the service may
    /// be down, and a ground station that cannot draw a profile is not broken.
    #[tokio::test]
    #[ignore = "it reaches the network"]
    async fn the_elevation_of_a_place_is_read_from_the_service() {
        let cache = std::env::temp_dir().join("mpcraft-terrain-test");
        let service = TerrainService::new(cache);

        // Canberra.
        let elevation = service
            .elevation(LatLng::new(-35.2809, 149.13))
            .await
            .expect("hay elevacion en Canberra");

        // The city is about 580 metres above the sea.
        assert!(
            (elevation.metres - 580.0).abs() < 120.0,
            "the elevation was {}",
            elevation.metres
        );
    }

    #[test]
    fn a_tile_is_named_after_its_south_west_corner() {
        let name = TerrainService::tile_name(LatLng::new(-35.28, 149.13));
        assert_eq!(name, "S36E149");

        let name = TerrainService::tile_name(LatLng::new(35.28, -149.13));
        assert_eq!(name, "N35W150");
    }
}
