//! The shapefile of ESRI.
//!
//! A shapefile is a file with the places of a drawing: the points, the lines and the
//! polygons. Mission Planner can bring a plan in from one of them (`LoadSHPFile`), and so
//! can MPCraft: the file is read, its points become waypoints, and the operator flies
//! them.
//!
//! The format is simple: a hundred byte header that says what the file holds, and then
//! the records, one after the other, each of them with the points it carries.
//!
//! The coordinates of a shapefile are in the projection the file says, which is written
//! in a file beside it with the same name and the extension `.prj`. A file that says
//! nothing is taken to be in the coordinates the ground station speaks: the longitude
//! and the latitude in degrees.

/// Why a shapefile could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShapeError {
    /// The file is not a shapefile.
    #[error("the file is not a shapefile")]
    NotAShapeFile,

    /// The file is cut short.
    #[error("the shapefile is cut short")]
    Truncated,

    /// The shape of the file is one that is not known.
    #[error("the shape {0} is not one that is known")]
    UnknownShape(u32),
}

/// A place of a shapefile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapePoint {
    /// The position along the first axis, in the units of the file.
    pub x: f64,
    /// The position along the second axis, in the units of the file.
    pub y: f64,
}

/// The points of a shapefile, in the order they are read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShapeFile {
    /// The points.
    pub points: Vec<ShapePoint>,
    /// The kind of shape the file holds: 1 a point, 3 a line, 5 a surface, 8 several
    /// points.
    pub shape_type: u32,
}

impl ShapeFile {
    /// Reads a shapefile from bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the bytes are not a shapefile, when the file is cut short, or
    /// when the file holds a shape that is not known.
    pub fn parse(bytes: &[u8]) -> Result<Self, ShapeError> {
        if bytes.len() < 100 {
            return Err(ShapeError::Truncated);
        }

        // The file code is 9994, in big endian, and the version is 1000.
        if u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) != 9994 {
            return Err(ShapeError::NotAShapeFile);
        }

        let shape_type = u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]);

        let mut points = Vec::new();
        let mut offset = 100_usize;

        while offset + 8 <= bytes.len() {
            // The number of the record, which is not used, and the length of what comes
            // after it, in words of two bytes.
            let length = u32::from_be_bytes([
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]) as usize;

            let start = offset + 8;
            let end = start + length * 2;

            if end > bytes.len() {
                return Err(ShapeError::Truncated);
            }

            read_record(&bytes[start..end], &mut points)?;

            offset = end;
        }

        Ok(Self { points, shape_type })
    }

    /// The points as longitudes and latitudes.
    ///
    /// A shapefile whose projection is not said is in the coordinates the ground station
    /// speaks: the longitude and the latitude in degrees.
    #[must_use]
    pub fn as_lat_lng(&self) -> Vec<(f64, f64)> {
        self.points.iter().map(|point| (point.y, point.x)).collect()
    }
}

/// Reads the points of one record.
fn read_record(record: &[u8], points: &mut Vec<ShapePoint>) -> Result<(), ShapeError> {
    let shape_type = read_u32(record, 0)?;

    match shape_type {
        // A point, with or without the measure that follows it: the kind of the shape and
        // its two coordinates.
        1 | 11 | 21 => {
            let (x, y) = read_point(record.get(4..20).ok_or(ShapeError::Truncated)?)?;
            points.push(ShapePoint { x, y });
        }

        // A line, a surface or a shape with holes: the box, the number of parts, the
        // number of points, the table of the parts and then the points.
        3 | 5 | 13 | 15 | 23 | 25 | 31 => {
            let parts = read_u32(record, 36)?;
            let count = read_u32(record, 40)?;

            let data_at = 44 + parts as usize * 4;
            read_points(record, data_at, count, points)?;
        }

        // Several points: the box, the number of them and then the points.
        8 | 18 | 28 => {
            let count = read_u32(record, 36)?;
            read_points(record, 40, count, points)?;
        }

        // A shape that holds no place.
        0 => {}

        other => return Err(ShapeError::UnknownShape(other)),
    }

    Ok(())
}

/// Reads `count` points that begin at `at`.
fn read_points(
    record: &[u8],
    at: usize,
    count: u32,
    points: &mut Vec<ShapePoint>,
) -> Result<(), ShapeError> {
    for index in 0..count as usize {
        let start = at + index * 16;
        let (x, y) = read_point(record.get(start..start + 16).ok_or(ShapeError::Truncated)?)?;
        points.push(ShapePoint { x, y });
    }

    Ok(())
}

/// Reads one point: two numbers of eight bytes, in little endian.
fn read_point(slice: &[u8]) -> Result<(f64, f64), ShapeError> {
    if slice.len() < 16 {
        return Err(ShapeError::Truncated);
    }

    let x = f64::from_le_bytes(slice[0..8].try_into().map_err(|_| ShapeError::Truncated)?);
    let y = f64::from_le_bytes(slice[8..16].try_into().map_err(|_| ShapeError::Truncated)?);

    Ok((x, y))
}

/// Reads a number of four bytes, in little endian.
fn read_u32(record: &[u8], at: usize) -> Result<u32, ShapeError> {
    let slice = record.get(at..at + 4).ok_or(ShapeError::Truncated)?;

    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shapefile with one line of two points.
    fn sample() -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();

        // The header: the code of the file, the length, the version and the kind of the
        // shape, and then the box of the file.
        bytes.extend_from_slice(&9994_u32.to_be_bytes());
        bytes.extend_from_slice(&[0_u8; 20]);
        bytes.extend_from_slice(&((80_u32) + 100).to_be_bytes());
        bytes.extend_from_slice(&1000_u32.to_le_bytes());
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&[0_u8; 64]);

        // One record of one line with two points.
        let mut record: Vec<u8> = Vec::new();
        record.extend_from_slice(&3_u32.to_le_bytes());
        record.extend_from_slice(&[0_u8; 32]);
        record.extend_from_slice(&1_u32.to_le_bytes());
        record.extend_from_slice(&2_u32.to_le_bytes());
        record.extend_from_slice(&0_u32.to_le_bytes());
        record.extend_from_slice(&149.13_f64.to_le_bytes());
        record.extend_from_slice(&(-35.28_f64).to_le_bytes());
        record.extend_from_slice(&149.14_f64.to_le_bytes());
        record.extend_from_slice(&(-35.29_f64).to_le_bytes());

        bytes.extend_from_slice(&1_u32.to_be_bytes());
        bytes.extend_from_slice(&((record.len() / 2) as u32).to_be_bytes());
        bytes.extend_from_slice(&record);

        bytes
    }

    #[test]
    fn the_points_of_a_line_are_read() {
        let file = ShapeFile::parse(&sample()).expect("the file is read");

        assert_eq!(file.shape_type, 3);
        assert_eq!(file.points.len(), 2);
        assert!((file.points[0].x - 149.13).abs() < 1e-9);
        assert!((file.points[0].y + 35.28).abs() < 1e-9);
    }

    #[test]
    fn the_points_are_longitude_and_latitude() {
        let file = ShapeFile::parse(&sample()).expect("the file is read");
        let places = file.as_lat_lng();

        assert!(
            (places[0].0 + 35.28).abs() < 1e-9,
            "the latitude comes first"
        );
        assert!(
            (places[0].1 - 149.13).abs() < 1e-9,
            "and then the longitude"
        );
    }

    #[test]
    fn a_file_that_is_not_a_shapefile_is_an_error() {
        let error = ShapeFile::parse(&[7_u8; 128]).expect_err("a file of another kind is not one");
        assert_eq!(error, ShapeError::NotAShapeFile);
    }

    #[test]
    fn a_file_that_is_cut_short_is_an_error() {
        let error = ShapeFile::parse(&[0_u8; 10]).expect_err("it is cut short");
        assert_eq!(error, ShapeError::Truncated);
    }
}
