//! The terrain of the ground, asked for by the vehicle.
//!
//! A vehicle that follows the ground needs to know how high the ground is. It asks
//! for it: the request says where the vehicle is, how far apart the points of the grid
//! are, and which squares of the grid are wanted. The ground station answers with the
//! heights of the squares that are asked for.
//!
//! Upstream has the same thing (`ExtLibs/Utilities/TerrainFollow.cs`), and it answers
//! with the heights it reads from the files of the terrain.
//!
//! ```text
//! GCS  ◀──TERRAIN_REQUEST──────  vehicle      (where, how far apart, which squares)
//! GCS  ──TERRAIN_DATA─────────▶  vehicle      (the heights of a square, four by four)
//! ```

use mpcraft_mavlink::dialect::messages::TERRAIN_DATA_DATA;
use mpcraft_mavlink::dialect::MavMessage;

/// The id of the `TERRAIN_REQUEST` message.
pub const TERRAIN_REQUEST_ID: u32 = 133;
/// The id of the `TERRAIN_DATA` message.
pub const TERRAIN_DATA_ID: u32 = 134;

/// How many points a square of the grid has on each side.
pub const GRID_SIDE: usize = 4;

/// How many points a square of the grid has.
pub const GRID_POINTS: usize = GRID_SIDE * GRID_SIDE;

/// A request of the terrain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainRequest {
    /// Where the vehicle is, in degrees.
    pub latitude: f64,
    /// Where the vehicle is, in degrees.
    pub longitude: f64,
    /// How far apart the points of the grid are, in metres.
    pub grid_spacing: u16,
    /// The squares of the grid that are wanted.
    pub mask: u64,
}

impl TerrainRequest {
    /// The square of the grid that a bit of the mask names.
    ///
    /// The grid is eight squares across and seven down, and the bit of the mask says
    /// which one it is.
    #[must_use]
    pub fn square_of(&self, bit: u32) -> (f64, f64) {
        // The squares are a grid that grows from the place the vehicle is at: eight
        // across and seven down, and the distance between the squares is four times
        // the spacing of the grid.
        let across = f64::from(bit % 8);
        let down = f64::from(bit / 8);

        // A degree of longitude is shorter than a degree of latitude the further north
        // it is, and the cosine of the latitude says how much shorter.
        let scale = 111_320.0 * self.latitude.to_radians().cos().abs().max(0.000_001);

        let north = self.latitude + (across * f64::from(self.grid_spacing) * 4.0) / 111_320.0;
        let east = self.longitude + (down * f64::from(self.grid_spacing) * 4.0) / scale;

        (north, east)
    }

    /// The place of a point of a square of the grid.
    ///
    /// The points of a square are four by four, and they are as far apart as the grid
    /// says.
    #[must_use]
    pub fn point_of(&self, origin: (f64, f64), x: usize, y: usize) -> (f64, f64) {
        let spacing = f64::from(self.grid_spacing);

        // A degree of longitude is shorter than a degree of latitude the further north
        // it is, and the cosine of the latitude says how much shorter.
        let scale = 111_320.0 * origin.0.to_radians().cos().abs().max(0.000_001);

        let north = origin.0 + (f64::from(y as u32) * spacing) / 111_320.0;
        let east = origin.1 + (f64::from(x as u32) * spacing) / scale;

        (north, east)
    }

    /// The squares of the grid that are wanted, with their bit.
    #[must_use]
    pub fn squares(&self) -> Vec<(u32, u64)> {
        (0..56)
            .filter(|bit| self.mask & (1_u64 << bit) != 0)
            .map(|bit| (bit, 1_u64 << bit))
            .collect()
    }
}

/// The answer of the terrain: the heights of a square of the grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainSquare {
    /// Where the vehicle is, in degrees.
    pub latitude: f64,
    /// Where the vehicle is, in degrees.
    pub longitude: f64,
    /// How far apart the points of the grid are, in metres.
    pub grid_spacing: u16,
    /// Which square of the grid this is.
    pub bit: u8,
    /// The heights of the points of the square, in metres.
    pub heights: [i16; GRID_POINTS],
}

impl TerrainSquare {
    /// The message the vehicle is answered with.
    #[must_use]
    pub fn message(&self) -> TERRAIN_DATA_DATA {
        TERRAIN_DATA_DATA {
            lat: (self.latitude * 1e7) as i32,
            lon: (self.longitude * 1e7) as i32,
            grid_spacing: self.grid_spacing,
            gridbit: self.bit,
            data: self.heights,
        }
    }
}

impl From<TerrainSquare> for MavMessage {
    fn from(square: TerrainSquare) -> Self {
        Self::TERRAIN_DATA(square.message())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_squares_that_are_wanted_are_the_ones_the_mask_names() {
        let request = TerrainRequest {
            latitude: -35.28,
            longitude: 149.13,
            grid_spacing: 100,
            mask: (1 << 0) | (1 << 9),
        };

        let squares = request.squares();

        assert_eq!(squares.len(), 2);
        assert_eq!(squares[0].0, 0);
        assert_eq!(squares[1].0, 9);
    }

    #[test]
    fn a_square_that_is_wanted_is_near_the_vehicle() {
        let request = TerrainRequest {
            latitude: -35.28,
            longitude: 149.13,
            grid_spacing: 100,
            mask: 1,
        };

        let (north, east) = request.square_of(0);

        assert!((north - request.latitude).abs() < 0.01);
        assert!((east - request.longitude).abs() < 0.01);
    }

    #[test]
    fn a_point_of_a_square_is_near_the_square() {
        let request = TerrainRequest {
            latitude: -35.28,
            longitude: 149.13,
            grid_spacing: 100,
            mask: 1,
        };

        let origin = request.square_of(0);
        let (latitude, longitude) = request.point_of(origin, 3, 3);

        // Three hundred metres to the north and to the east, which is a small fraction
        // of a degree.
        assert!((latitude - origin.0 - 0.0027).abs() < 0.0002);
        assert!((longitude - origin.1 - 0.0033).abs() < 0.0002);
    }

    #[test]
    fn the_heights_of_a_square_travel_in_the_message() {
        let square = TerrainSquare {
            latitude: -35.28,
            longitude: 149.13,
            grid_spacing: 100,
            bit: 3,
            heights: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        };

        let message = square.message();

        assert_eq!(message.gridbit, 3);
        assert_eq!(message.data[0], 1);
        assert_eq!(message.data[15], 16);
        assert_eq!(message.lat, (-35.28 * 1e7) as i32);
    }
}
