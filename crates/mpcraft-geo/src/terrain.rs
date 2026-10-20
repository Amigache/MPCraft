//! Terrain elevation data.
//!
//! Mission Planner sources elevation data from the ArduPilot terrain service
//! (`terrain.ardupilot.org`), which serves SRTM and ASTER tiles. MPCraft follows the same
//! model: a [`TerrainProvider`] fetches tiles, caches them on disk, and answers elevation
//! queries.
//!
//! # Offline behaviour
//!
//! A terrain lookup that cannot be satisfied must fail *loudly*. Mission Planner's altitude
//! modes treat terrain altitude as authoritative, so a silent fallback to sea level would make
//! a mission fly into the ground.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::projection::LatLng;

/// The failure modes of a terrain lookup.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TerrainError {
    /// The terrain service could not be reached.
    #[error("terrain service unavailable: {0}")]
    Unavailable(String),

    /// No elevation data covers the requested location.
    #[error("no terrain data for {lat}, {lng}")]
    NoData {
        /// The requested latitude.
        lat: f64,
        /// The requested longitude.
        lng: f64,
    },

    /// The response from the terrain service was malformed.
    #[error("malformed terrain response: {0}")]
    Malformed(String),

    /// The local cache could not be read or written.
    #[error("terrain cache error: {0}")]
    Cache(String),
}

/// An elevation sample, in metres above sea level.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Elevation {
    /// The elevation in metres.
    pub metres: f64,
    /// Which dataset the elevation came from.
    pub source: ElevationSource,
}

/// The dataset an elevation came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ElevationSource {
    /// SRTM 1 arc-second data.
    Srtm1,
    /// SRTM 3 arc-second data.
    Srtm3,
    /// ASTER global digital elevation model.
    Aster,
    /// A local DTED tile.
    Dted,
}

/// Provides elevation lookups.
pub trait TerrainProvider: Send + Sync {
    /// Returns the elevation at a position, in metres above sea level.
    ///
    /// # Errors
    ///
    /// Returns an error when no data is available or the source cannot be reached. It must not
    /// return a fabricated value.
    fn elevation(&self, position: LatLng) -> Result<Elevation, TerrainError>;

    /// Returns elevations along a path.
    ///
    /// # Errors
    ///
    /// Returns an error if any sample could not be resolved.
    fn elevation_path(&self, path: &[LatLng]) -> Result<Vec<Elevation>, TerrainError> {
        path.iter().map(|p| self.elevation(*p)).collect()
    }
}

/// A terrain provider backed by a local cache directory.
///
/// The cache layout mirrors the tile server's addressing so that tiles can be pre-fetched for
/// offline use.
#[derive(Debug, Clone)]
pub struct CachedTerrainProvider {
    cache_dir: PathBuf,
    base_url: String,
}

impl CachedTerrainProvider {
    /// Creates a provider that caches under `cache_dir` and fetches from `base_url`.
    #[must_use]
    pub fn new(cache_dir: PathBuf, base_url: impl Into<String>) -> Self {
        Self {
            cache_dir,
            base_url: base_url.into(),
        }
    }

    /// The directory where tiles are cached.
    #[must_use]
    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// The base URL of the terrain service.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The cache path of the tile covering a position.
    ///
    /// The tile size is one degree, matching the terrain service's addressing.
    #[must_use]
    pub fn tile_path(&self, position: LatLng) -> PathBuf {
        let lat_tile = position.lat.floor() as i32;
        let lng_tile = position.lng.floor() as i32;
        self.cache_dir
            .join(format!("{lat_tile}"))
            .join(format!("{lng_tile}.hgt"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_paths_follow_the_degree_grid() {
        let provider = CachedTerrainProvider::new(
            PathBuf::from("/tmp/terrain"),
            "https://terrain.ardupilot.org/SRTM3/",
        );

        let path = provider.tile_path(LatLng::new(-35.28, 149.13));
        assert!(path.ends_with("-36/149.hgt"), "path was {path:?}");
    }

    #[test]
    fn negative_longitudes_floor_correctly() {
        let provider = CachedTerrainProvider::new(PathBuf::from("/tmp/terrain"), "https://x/");
        let path = provider.tile_path(LatLng::new(0.0, -0.5));
        assert!(path.ends_with("0/-1.hgt"), "path was {path:?}");
    }
}
