//! The elevation grid of a tile.
//!
//! A tile of the terrain is a square of samples, one degree across, with the elevation
//! of each of them in metres above the sea. The samples are sixteen bit integers, big
//! endian, and the value -32768 means "there is nothing here": it is the sea, or the
//! data was never gathered.
//!
//! The grid is the domain's own format, so it lives here and not in the interface.

use crate::terrain::{Elevation, ElevationSource, TerrainError};

/// The value that means "there is no elevation here".
const VOID: i16 = -32768;

/// A grid of elevations, one degree across.
#[derive(Debug, Clone, PartialEq)]
pub struct HeightGrid {
    /// The number of samples on a side.
    width: usize,
    /// The samples, from the south-west corner to the north-east.
    samples: Vec<i16>,
    /// The latitude of the south-west corner.
    origin_lat: f64,
    /// The longitude of the south-west corner.
    origin_lng: f64,
    /// How many degrees the grid covers.
    span: f64,
    /// Which dataset the grid came from.
    source: ElevationSource,
}

impl HeightGrid {
    /// Reads a grid from the bytes of a tile.
    ///
    /// The tile is a square of samples of sixteen bits, big endian. The number of samples
    /// on a side is the square root of the number of bytes, and it has to be a whole
    /// number: a tile that is not square is not a tile.
    ///
    /// # Errors
    ///
    /// Returns an error when the bytes are not a square of sixteen bit samples.
    pub fn from_bytes(
        bytes: &[u8],
        origin_lat: f64,
        origin_lng: f64,
        source: ElevationSource,
    ) -> Result<Self, TerrainError> {
        if !bytes.len().is_multiple_of(2) {
            return Err(TerrainError::Malformed(
                "the tile holds an odd number of bytes".to_owned(),
            ));
        }

        let count = bytes.len() / 2;
        let width = (count as f64).sqrt() as usize;

        if width * width != count {
            return Err(TerrainError::Malformed(format!(
                "the tile holds {count} samples, which is not a square"
            )));
        }

        let (pairs, _) = bytes.as_chunks::<2>();
        let samples: Vec<i16> = pairs
            .iter()
            .map(|pair| i16::from_be_bytes([pair[0], pair[1]]))
            .collect();

        Ok(Self {
            width,
            samples,
            origin_lat,
            origin_lng,
            span: 1.0,
            source,
        })
    }

    /// The number of samples on a side.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Which dataset the grid came from.
    #[must_use]
    pub fn source(&self) -> ElevationSource {
        self.source
    }

    /// The elevation at a position, in metres above the sea.
    ///
    /// A position outside the grid has no elevation: the grid covers one degree, and a
    /// position outside it is in another tile.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no data at the position: the sea, or a place the
    /// survey never reached.
    pub fn elevation_at(&self, lat: f64, lng: f64) -> Result<Elevation, TerrainError> {
        let Some((x, y)) = self.sample_of(lat, lng) else {
            return Err(TerrainError::NoData { lat, lng });
        };

        let sample = self.samples[y * self.width + x];

        if sample == VOID {
            return Err(TerrainError::NoData { lat, lng });
        }

        Ok(Elevation {
            metres: f64::from(sample),
            source: self.source,
        })
    }

    /// The sample of the grid that covers a position.
    ///
    /// The rows of a tile go from the north to the south, so the first sample of the
    /// file is the north-west corner and the last one is the south-east.
    fn sample_of(&self, lat: f64, lng: f64) -> Option<(usize, usize)> {
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lng) {
            return None;
        }

        // The position within the grid, as a fraction of its side.
        let along = (lng - self.origin_lng) / self.span;
        let up = (lat - self.origin_lat) / self.span;

        if !(0.0..=1.0).contains(&along) || !(0.0..=1.0).contains(&up) {
            return None;
        }

        let x = (along * (self.width - 1) as f64).round() as usize;
        let y = ((1.0 - up) * (self.width - 1) as f64).round() as usize;

        if x >= self.width || y >= self.width {
            return None;
        }

        Some((x, y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grid of three samples a side, with the elevation written in it.
    fn grid() -> HeightGrid {
        // The south-west corner is 0, the rows go from the south to the north.
        let mut samples = Vec::new();

        for row in 0..3 {
            for column in 0..3 {
                let value = (row * 3 + column) as i16 * 100;
                samples.extend_from_slice(&value.to_be_bytes());
            }
        }

        HeightGrid::from_bytes(&samples, -1.0, 149.0, ElevationSource::Srtm3).expect("la tesela")
    }

    #[test]
    fn a_grid_is_read() {
        let grid = grid();

        assert_eq!(grid.width(), 3);
    }

    #[test]
    fn the_elevation_is_read_at_a_position() {
        let grid = grid();

        // The first sample of the file is the north-west corner.
        let elevation = grid.elevation_at(0.0, 149.0).expect("hay elevacion");
        assert!((elevation.metres - 0.0).abs() < 1e-9);

        // The last one is the south-east corner.
        let elevation = grid.elevation_at(-1.0, 150.0).expect("hay elevacion");
        assert!((elevation.metres - 800.0).abs() < 1e-9);
    }

    #[test]
    fn a_position_outside_the_grid_has_no_elevation() {
        let grid = grid();

        let error = grid
            .elevation_at(10.0, 149.0)
            .expect_err("fuera de la tesela");

        assert!(matches!(error, TerrainError::NoData { .. }));
    }

    #[test]
    fn the_sea_has_no_elevation() {
        let mut samples = vec![0_u8; 8];
        samples[0..2].copy_from_slice(&VOID.to_be_bytes());

        let grid =
            HeightGrid::from_bytes(&samples, -1.0, 149.0, ElevationSource::Srtm3).expect("tesela");

        let error = grid
            .elevation_at(0.0, 149.0)
            .expect_err("el mar no tiene elevacion");

        assert!(matches!(error, TerrainError::NoData { .. }));
    }

    #[test]
    fn a_tile_that_is_not_a_square_is_refused() {
        let error = HeightGrid::from_bytes(&[0, 1, 2], 0.0, 0.0, ElevationSource::Srtm3)
            .expect_err("no es una tesela");

        assert!(matches!(error, TerrainError::Malformed(_)));
    }
}
