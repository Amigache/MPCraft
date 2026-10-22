//! The tiles of a map, fetched and kept on the disk.
//!
//! # What this is
//!
//! A map is drawn with tiles: little pictures of the ground, one for each square of a
//! grid, for each level of zoom. The ground station asks the server for the tiles it
//! needs and the server answers with the pictures.
//!
//! When the link is not there — over the sea, in a valley, in a country where the
//! service is not — the tiles are fetched beforehand and kept on the disk, and the map
//! is drawn with the ones that are there.
//!
//! # What it is for
//!
//! A survey is flown where there is no signal. The tiles of the field are fetched
//! before the flight, and the map of the field is drawn from the disk.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The name of the file a tile is kept in.
const TILE_EXTENSION: &str = "png";

/// What can go wrong while a tile is fetched.
#[derive(Debug, thiserror::Error)]
pub enum TileError {
    /// The tile could not be fetched.
    #[error("the tile could not be fetched: {0}")]
    Fetch(String),
    /// The tile could not be kept on the disk.
    #[error("the tile could not be written: {0}")]
    Io(#[from] std::io::Error),
    /// The service answered with something that is not a picture.
    #[error("the service answered with {0} bytes, which is not a picture")]
    NotAPicture(usize),
}

/// The place a region of the ground covers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bounds {
    /// The latitude of the northern side, in degrees.
    pub north: f64,
    /// The latitude of the southern side, in degrees.
    pub south: f64,
    /// The longitude of the eastern side, in degrees.
    pub east: f64,
    /// The longitude of the western side, in degrees.
    pub west: f64,
}

/// The service that fetches the tiles and keeps them.
#[derive(Debug, Clone)]
pub struct TileService {
    /// Where the tiles are kept.
    cache: PathBuf,
    /// The folder of tiles the operator has brought, when there is one.
    injection: std::sync::Arc<std::sync::Mutex<Option<PathBuf>>>,
    /// The client that asks for them.
    client: reqwest::Client,
}

/// One source of tiles.
///
/// The sources are the ones that are free and that need neither an account nor a key,
/// because a ground station that asks for a key before it draws a map is a ground
/// station that does not draw a map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TileProvider {
    /// The identifier the choice is kept under.
    pub id: String,
    /// The name the operator reads it with.
    pub name: String,
    /// The address of the style, for a map of vectors.
    pub style_url: Option<String>,
    /// The address of the tiles, for a map of pictures.
    pub tiles: Option<String>,
    /// The copyright the source asks to be shown.
    pub attribution: String,
}

/// The sources of tiles, in the order they are offered.
#[must_use]
pub fn providers() -> Vec<TileProvider> {
    vec![
        TileProvider {
            id: "openfreemap-liberty".to_owned(),
            name: "OpenFreeMap Liberty".to_owned(),
            style_url: Some("https://tiles.openfreemap.org/styles/liberty".to_owned()),
            tiles: None,
            attribution: "© OpenFreeMap contributors".to_owned(),
        },
        TileProvider {
            id: "openfreemap-bright".to_owned(),
            name: "OpenFreeMap Bright".to_owned(),
            style_url: Some("https://tiles.openfreemap.org/styles/bright".to_owned()),
            tiles: None,
            attribution: "© OpenFreeMap contributors".to_owned(),
        },
        TileProvider {
            id: "openfreemap-positron".to_owned(),
            name: "OpenFreeMap Positron".to_owned(),
            style_url: Some("https://tiles.openfreemap.org/styles/positron".to_owned()),
            tiles: None,
            attribution: "© OpenFreeMap contributors".to_owned(),
        },
        TileProvider {
            id: "openstreetmap".to_owned(),
            name: "OpenStreetMap".to_owned(),
            style_url: None,
            tiles: Some("https://tile.openstreetmap.org/{z}/{x}/{y}.png".to_owned()),
            attribution: "© OpenStreetMap contributors".to_owned(),
        },
        TileProvider {
            id: "opentopomap".to_owned(),
            name: "OpenTopoMap".to_owned(),
            style_url: None,
            tiles: Some("https://a.tile.opentopomap.org/{z}/{x}/{y}.png".to_owned()),
            attribution: "© OpenTopoMap contributors, © OpenStreetMap contributors".to_owned(),
        },
        TileProvider {
            id: "esri-imagery".to_owned(),
            name: "Esri World Imagery".to_owned(),
            style_url: None,
            tiles: Some(
                "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}"
                    .to_owned(),
            ),
            attribution: "© Esri, Maxar, Earthstar Geographics".to_owned(),
        },
        TileProvider {
            id: "cartodb-light".to_owned(),
            name: "CartoDB Light".to_owned(),
            style_url: None,
            tiles: Some("https://basemaps.cartocdn.com/light_all/{z}/{x}/{y}.png".to_owned()),
            attribution: "© OpenStreetMap contributors, © CARTO".to_owned(),
        },
    ]
}

/// The source with the given identifier, when there is one.
#[must_use]
pub fn provider_of(id: &str) -> Option<TileProvider> {
    providers().into_iter().find(|provider| provider.id == id)
}

impl TileService {
    /// A service that keeps the tiles in the given folder.
    #[must_use]
    pub fn new(cache: PathBuf) -> Self {
        Self {
            cache,
            injection: std::sync::Arc::new(std::sync::Mutex::new(None)),
            client: reqwest::Client::new(),
        }
    }

    /// Says which folder of tiles the operator has brought.
    ///
    /// The folder is looked at before the cache: the tiles the operator brought are
    /// the ones they want to be seen.
    pub fn set_injection(&self, folder: Option<PathBuf>) {
        if let Ok(mut injection) = self.injection.lock() {
            *injection = folder;
        }
    }

    /// The folder of tiles the operator has brought, when there is one.
    #[must_use]
    pub fn injection_dir(&self) -> Option<PathBuf> {
        self.injection
            .lock()
            .ok()
            .and_then(|injection| injection.clone())
    }

    /// Where the tiles are kept.
    #[must_use]
    pub fn cache_dir(&self) -> &Path {
        &self.cache
    }

    /// The name of the file a tile is kept in.
    ///
    /// The name says which provider it is, which level of zoom it is and where it is
    /// on the grid, so that the tiles of two providers do not mix.
    #[must_use]
    pub fn tile_name(provider: &str, zoom: u8, x: u32, y: u32) -> String {
        let provider: String = provider
            .chars()
            .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
            .collect();

        format!("{provider}/{zoom}/{x}/{y}.{TILE_EXTENSION}")
    }

    /// The place a tile is kept in.
    #[must_use]
    pub fn tile_path(&self, provider: &str, zoom: u8, x: u32, y: u32) -> PathBuf {
        self.cache.join(Self::tile_name(provider, zoom, x, y))
    }

    /// Whether the tile is on the disk.
    #[must_use]
    pub fn has(&self, provider: &str, zoom: u8, x: u32, y: u32) -> bool {
        self.tile_path(provider, zoom, x, y).is_file()
    }

    /// Fetches a tile and keeps it.
    ///
    /// A tile that is already on the disk is not fetched again.
    ///
    /// # Errors
    ///
    /// Returns an error when the tile cannot be fetched, or when it cannot be kept.
    pub async fn fetch(
        &self,
        template: &str,
        provider: &str,
        zoom: u8,
        x: u32,
        y: u32,
    ) -> Result<bool, TileError> {
        let path = self.tile_path(provider, zoom, x, y);

        if path.is_file() {
            return Ok(false);
        }

        let url = template
            .replace("{z}", &zoom.to_string())
            .replace("{x}", &x.to_string())
            .replace("{y}", &y.to_string());

        let answer = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|error| TileError::Fetch(error.to_string()))?;

        if !answer.status().is_success() {
            return Err(TileError::Fetch(format!(
                "the service answered with {}",
                answer.status()
            )));
        }

        let bytes = answer
            .bytes()
            .await
            .map_err(|error| TileError::Fetch(error.to_string()))?;

        if bytes.is_empty() {
            return Err(TileError::NotAPicture(0));
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&path, &bytes)?;

        Ok(true)
    }

    /// The tile of a place, from the disk when it is there and from the service when it
    /// is not.
    ///
    /// The folder the operator has brought is looked at first, then the cache, and the
    /// service is asked only when the tile is in neither. That is the order that makes
    /// a map work where there is no signal: the tiles that were fetched beforehand are
    /// the ones that are drawn.
    ///
    /// # Errors
    ///
    /// Returns an error when the tile cannot be fetched, or when it cannot be kept.
    pub async fn tile(
        &self,
        provider: &str,
        zoom: u8,
        x: u32,
        y: u32,
    ) -> Result<Vec<u8>, TileError> {
        if let Some(bytes) = self.injected(provider, zoom, x, y) {
            return Ok(bytes);
        }

        let path = self.tile_path(provider, zoom, x, y);

        if let Ok(bytes) = std::fs::read(&path) {
            return Ok(bytes);
        }

        let Some(source) = provider_of(provider) else {
            return Err(TileError::Fetch(format!(
                "there is no source of tiles called {provider:?}"
            )));
        };

        let Some(template) = source.tiles else {
            return Err(TileError::Fetch(format!(
                "the source {provider:?} is a map of vectors, and it has no tiles to fetch"
            )));
        };

        self.fetch(&template, provider, zoom, x, y).await?;

        let bytes = std::fs::read(&path)?;

        Ok(bytes)
    }

    /// The tile of a place in the folder the operator has brought.
    ///
    /// The folder is looked at in the shape the tiles are kept in — the source, the
    /// level of zoom and the place on the grid — and in the plain shape of a folder of
    /// pictures: the level of zoom and the place.
    fn injected(&self, provider: &str, zoom: u8, x: u32, y: u32) -> Option<Vec<u8>> {
        let folder = self.injection_dir()?;

        let candidates = [
            folder.join(Self::tile_name(provider, zoom, x, y)),
            folder.join(format!("{zoom}/{x}/{y}.{TILE_EXTENSION}")),
            folder.join(format!("{zoom}_{x}_{y}.{TILE_EXTENSION}")),
        ];

        candidates
            .into_iter()
            .find(|path| path.is_file())
            .and_then(|path| std::fs::read(path).ok())
    }

    /// Fetches every tile of a region, for the levels of zoom that are asked for.    ///
    /// The number of tiles that were fetched is returned: the tiles that were already
    /// on the disk are not counted, because they were not fetched.
    ///
    /// # Errors
    ///
    /// Returns an error when a tile cannot be fetched, or when it cannot be kept.
    pub async fn prefetch(
        &self,
        template: &str,
        provider: &str,
        bounds: Bounds,
        zooms: &[u8],
    ) -> Result<usize, TileError> {
        let mut fetched = 0_usize;

        for zoom in zooms {
            let (west, north, east, south) = tiles_of(bounds, *zoom);

            for x in west..=east {
                for y in north..=south {
                    if self.fetch(template, provider, *zoom, x, y).await? {
                        fetched += 1;
                    }
                }
            }
        }

        Ok(fetched)
    }
}

/// The tiles of a region, at a level of zoom.
///
/// The grid is the one of the web: the world is a square of `2^zoom` tiles, and the
/// places are projected onto it with the Mercator projection.
#[must_use]
pub fn tiles_of(bounds: Bounds, zoom: u8) -> (u32, u32, u32, u32) {
    let scale = 2_f64.powi(i32::from(zoom));

    let west = longitude_to_tile(bounds.west, scale);
    let east = longitude_to_tile(bounds.east, scale);
    let north = latitude_to_tile(bounds.north, scale);
    let south = latitude_to_tile(bounds.south, scale);

    (
        west.min(east),
        north.min(south),
        west.max(east),
        north.max(south),
    )
}

/// The column of the tile a longitude is in.
fn longitude_to_tile(longitude: f64, scale: f64) -> u32 {
    let x = ((longitude + 180.0) / 360.0 * scale).floor();

    x.clamp(0.0, scale - 1.0) as u32
}

/// The row of the tile a latitude is in.
fn latitude_to_tile(latitude: f64, scale: f64) -> u32 {
    let radians = latitude.to_radians();
    let y = ((1.0 - radians.tan().asinh() / std::f64::consts::PI) / 2.0 * scale).floor();

    y.clamp(0.0, scale - 1.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tiles_of_canberra_are_the_ones_of_the_grid() {
        let bounds = Bounds {
            north: -35.25,
            south: -35.30,
            east: 149.16,
            west: 149.10,
        };

        let (west, north, east, south) = tiles_of(bounds, 12);

        // Canberra is in the south-east of the world, and the tiles are numbered from
        // the north-west.
        assert!(west > 2_000);
        assert!(east >= west);
        assert!(north > 1_400);
        assert!(south >= north);
    }

    #[test]
    fn the_zoom_gives_more_tiles() {
        let bounds = Bounds {
            north: -35.25,
            south: -35.30,
            east: 149.16,
            west: 149.10,
        };

        let (west, north, east, south) = tiles_of(bounds, 12);
        let (west2, north2, east2, south2) = tiles_of(bounds, 14);

        assert!(west2 >= west);
        assert!(east2 >= east);
        assert!(north2 >= north);
        assert!(south2 >= south);
    }

    #[test]
    fn the_name_of_a_tile_says_where_it_is() {
        let name = TileService::tile_name("openfreemap", 12, 3_700, 3_100);

        assert_eq!(name, "openfreemap/12/3700/3100.png");
    }

    #[test]
    fn the_name_of_a_tile_has_no_strange_characters() {
        let name = TileService::tile_name("open free map/../..", 1, 2, 3);

        assert!(!name.contains(' '));
        assert!(!name.contains(".."));
    }
}
