//! Digital terrain elevation data — DTED.
//!
//! DTED is the elevation format of the United States military: a square of the ground, one
//! degree across, with the height of the ground in metres at every post of a grid. Mission
//! Planner reads the files of the National Imagery and Mapping Agency — the `.dt0`, `.dt1` and
//! `.dt2` files — and uses them when the operator asks for the height of the ground at a
//! place.
//!
//! # The shape of a file
//!
//! A file is made of three ASCII records and then the posts of the grid:
//!
//! | Record | Bytes | What it holds |
//! | --- | --- | --- |
//! | User header label (UHL) | 80 | The level of the file, the south-west corner, the spacing of the posts |
//! | Data set identification (DSI) | 648 | Who made the file, and when |
//! | Accuracy description (ACC) | 2700 | How good the heights are |
//! | Data records | the rest | The heights themselves |
//!
//! The user header label is a line of eighty characters. Its first three are `UHL`, the fourth
//! is the level of the file (`0`, `1` or `2`), the next eight hold the longitude of the
//! south-west corner as `DDDMMSSH` and the next eight the latitude of the same corner in the
//! same way. The two fields that follow hold the distance between the posts — in tenths of an
//! arc-second — and the last two numbers of the header say how many posts there are from the
//! west to the east and from the south to the north.
//!
//! A data record is one column of the grid: eight bytes of header, one height per post and a
//! checksum of four bytes at the end. The heights are written from the south to the north, so
//! the first one is the height of the south-west corner of the cell and the last one is the
//! height of the north-east corner.
//!
//! # The heights
//!
//! The standard writes a height as a magnitude of fifteen bits with the sign in the top bit,
//! and `-32767` means "there is no height here". A few producers wrote the negative heights
//! in two's complement by mistake; those are recognised and corrected, as the reference reader
//! of the format does.
//!
//! Files whose posts are padded to six bytes — the height in two bytes and four bytes of
//! nothing — are read as well: the shape of the file tells the two apart.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::projection::LatLng;
use crate::terrain::{Elevation, ElevationSource, TerrainError, TerrainProvider};

/// The size of the user header label.
const UHL_SIZE: usize = 80;

/// The size of the data set identification record.
const DSI_SIZE: usize = 648;

/// The size of the accuracy description record.
const ACC_SIZE: usize = 2700;

/// The place the data of a file begins at, when the three records are there.
const DATA_OFFSET: usize = UHL_SIZE + DSI_SIZE + ACC_SIZE;

/// The size of the header of a data record.
const RECORD_HEADER_SIZE: usize = 8;

/// The size of the checksum at the end of a data record.
const CHECKSUM_SIZE: usize = 4;

/// The height that means "there is nothing here".
const NODATA: i16 = -32767;

/// The number of degrees of a tenth of an arc-second.
const TENTH_OF_ARC_SECOND: f64 = 1.0 / 36_000.0;

/// How close to the edge of the cell a point may be and still be inside it.
const EDGE_TOLERANCE: f64 = 1e-9;

/// The level of a DTED file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DtedLevel {
    /// Level 0: the ground every thirty seconds.
    Level0,
    /// Level 1: the ground every three seconds.
    Level1,
    /// Level 2: the ground every second.
    Level2,
}

impl DtedLevel {
    /// The distance between two posts, in arc-seconds, in the first zone of latitude.
    ///
    /// The zones of latitude make the distance between the posts of the east and the west
    /// grow, but never the one of the north and the south.
    #[must_use]
    pub fn post_spacing_arcsec(self) -> f64 {
        match self {
            DtedLevel::Level0 => 30.0,
            DtedLevel::Level1 => 3.0,
            DtedLevel::Level2 => 1.0,
        }
    }

    /// The number of posts from the south to the north of the cell.
    #[must_use]
    pub fn rows(self) -> usize {
        (3600.0 / self.post_spacing_arcsec()) as usize + 1
    }

    /// The number of posts from the west to the east of the cell, at the given latitude.
    ///
    /// Above the fiftieth degree the posts of the east and the west are twice as far apart,
    /// and further north they are six times as far apart.
    #[must_use]
    pub fn columns(self, latitude: f64) -> usize {
        let zone = latitude.abs();
        let factor = if zone < 50.0 {
            1.0
        } else if zone < 70.0 {
            2.0
        } else if zone < 75.0 {
            3.0
        } else if zone < 80.0 {
            4.0
        } else {
            6.0
        };

        (3600.0 / (self.post_spacing_arcsec() * factor)) as usize + 1
    }
}

/// What the header of a DTED file says.
#[derive(Debug, Clone, PartialEq)]
pub struct DtedHeader {
    /// The level of the file.
    pub level: DtedLevel,
    /// The latitude of the south-west corner of the cell, in degrees.
    pub origin_lat: f64,
    /// The longitude of the south-west corner of the cell, in degrees.
    pub origin_lng: f64,
    /// The distance between two posts from the south to the north, in degrees.
    pub spacing_lat: f64,
    /// The distance between two posts from the west to the east, in degrees.
    pub spacing_lng: f64,
    /// The number of posts from the west to the east.
    pub columns: usize,
    /// The number of posts from the south to the north.
    pub rows: usize,
    /// The vertical datum of the heights, when the file declares one.
    ///
    /// The files of the agency declare `VERTCG`, the vertical datum of the world geodetic
    /// system of 1984.
    pub vertical_datum: Option<String>,
}

/// The shape of the data of a DTED file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DtedLayout {
    /// The records of the standard: eight bytes of header, two bytes per post, a checksum.
    Standard,
    /// The records of the standard with the posts padded to six bytes.
    Padded,
    /// A single block of two bytes per post, with no record header.
    Flat,
    /// A single block of six bytes per post, with no record header.
    FlatPadded,
}

impl DtedLayout {
    /// The number of bytes a post takes.
    #[must_use]
    pub fn post_size(self) -> usize {
        match self {
            DtedLayout::Standard | DtedLayout::Flat => 2,
            DtedLayout::Padded | DtedLayout::FlatPadded => 6,
        }
    }
}

/// A rectangle of the ground, in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DtedBounds {
    /// The latitude of the southern side.
    pub south: f64,
    /// The longitude of the western side.
    pub west: f64,
    /// The latitude of the northern side.
    pub north: f64,
    /// The longitude of the eastern side.
    pub east: f64,
}

impl DtedBounds {
    /// Creates a rectangle from its southern and northern sides.
    #[must_use]
    pub const fn new(south: f64, west: f64, north: f64, east: f64) -> Self {
        Self {
            south,
            west,
            north,
            east,
        }
    }
}

/// The heights of a piece of ground.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DtedElevation {
    /// The lowest height of the piece, in metres above the sea.
    pub min_m: f64,
    /// The highest height of the piece, in metres above the sea.
    pub max_m: f64,
    /// The mean height of the piece, in metres above the sea.
    pub mean_m: f64,
    /// The number of posts the figures were taken from.
    pub samples: usize,
}

/// What can go wrong while a DTED file is read.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DtedError {
    /// The file could not be read from the disk.
    #[error("the DTED file could not be read: {0}")]
    Io(#[from] std::io::Error),

    /// The file is not a DTED file.
    #[error("{0} is not a DTED file")]
    NotDted(String),

    /// The header of the file does not say what it should.
    #[error("the DTED header is malformed: {0}")]
    MalformedHeader(String),

    /// The heights of the file do not fit the shape the header describes.
    #[error("the DTED data is malformed: {0}")]
    MalformedData(String),

    /// No height covers the place that was asked for.
    #[error("no DTED data covers {lat}, {lng}")]
    NoData {
        /// The latitude that was asked for.
        lat: f64,
        /// The longitude that was asked for.
        lng: f64,
    },
}

impl From<DtedError> for TerrainError {
    fn from(error: DtedError) -> Self {
        match error {
            DtedError::NoData { lat, lng } => TerrainError::NoData { lat, lng },
            DtedError::Io(error) => TerrainError::Cache(error.to_string()),
            other => TerrainError::Malformed(other.to_string()),
        }
    }
}

/// A DTED file: the heights of a square of the ground.
#[derive(Debug, Clone)]
pub struct DtedFile {
    header: DtedHeader,
    layout: DtedLayout,
    /// The file the cell was read from, when it was read from the disk.
    path: Option<std::path::PathBuf>,
    /// The posts, from the west to the east and from the south to the north within a column.
    posts: Vec<i16>,
    /// The number of records whose checksum did not add up.
    checksum_failures: usize,
}

impl DtedFile {
    /// Reads a DTED file from the disk.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read or when it is not a DTED file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DtedError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)?;
        let mut cell = Self::parse(&bytes)?;
        cell.path = Some(path.to_path_buf());
        Ok(cell)
    }

    /// The file the cell was read from, when it was read from the disk.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Reads a DTED file from the bytes of one.
    ///
    /// # Errors
    ///
    /// Returns an error when the bytes are not a DTED file, or when the header does not agree
    /// with the data.
    pub fn parse(bytes: &[u8]) -> Result<Self, DtedError> {
        let start = uhl_offset(bytes)?;

        let uhl = bytes
            .get(start..start + UHL_SIZE)
            .ok_or_else(|| DtedError::MalformedHeader("the header is cut".to_owned()))?;

        let mut header = parse_header(uhl)?;

        let data_start = data_offset(bytes, start);

        // A file that does not declare its own shape is given the shape the level and the
        // latitude of the corner ask for.
        if header.columns == 0 || header.rows == 0 {
            let rows = header.level.rows();
            let columns = header.level.columns(header.origin_lat);
            header.columns = columns;
            header.rows = rows;
        }

        let (layout, posts, checksum_failures) = decode(bytes, data_start, &header)?;

        Ok(Self {
            header,
            layout,
            path: None,
            posts,
            checksum_failures,
        })
    }

    /// What the header of the file says.
    #[must_use]
    pub fn header(&self) -> &DtedHeader {
        &self.header
    }

    /// The level of the file.
    #[must_use]
    pub fn level(&self) -> DtedLevel {
        self.header.level
    }

    /// The shape of the data of the file.
    #[must_use]
    pub fn layout(&self) -> DtedLayout {
        self.layout
    }

    /// The number of records whose checksum did not add up.
    ///
    /// A file whose checksums are wrong is still read, but the operator is told about it.
    #[must_use]
    pub fn checksum_failures(&self) -> usize {
        self.checksum_failures
    }

    /// The height of the ground at a place, in metres above the sea.
    ///
    /// The height between two posts is interpolated, so the ground is a smooth surface and not
    /// a staircase.
    ///
    /// # Errors
    ///
    /// Returns an error when the place is outside the cell, or when one of the posts around it
    /// has no height.
    pub fn elevation_at(&self, lat: f64, lng: f64) -> Result<f64, DtedError> {
        let columns = self.header.columns;
        let rows = self.header.rows;

        if columns == 0 || rows == 0 {
            return Err(DtedError::NoData { lat, lng });
        }

        let x = (lng - self.header.origin_lng) / self.header.spacing_lng;
        let y = (lat - self.header.origin_lat) / self.header.spacing_lat;

        if !x.is_finite() || !y.is_finite() {
            return Err(DtedError::NoData { lat, lng });
        }

        let last_x = (columns - 1) as f64;
        let last_y = (rows - 1) as f64;

        if x < -EDGE_TOLERANCE
            || y < -EDGE_TOLERANCE
            || x > last_x + EDGE_TOLERANCE
            || y > last_y + EDGE_TOLERANCE
        {
            return Err(DtedError::NoData { lat, lng });
        }

        let x = x.clamp(0.0, last_x);
        let y = y.clamp(0.0, last_y);

        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(columns - 1);
        let y1 = (y0 + 1).min(rows - 1);

        let fx = x - x0 as f64;
        let fy = y - y0 as f64;

        let v00 = self.height(x0, y0, lat, lng)?;
        let v10 = self.height(x1, y0, lat, lng)?;
        let v01 = self.height(x0, y1, lat, lng)?;
        let v11 = self.height(x1, y1, lat, lng)?;

        let lower = v00 + (v10 - v00) * fx;
        let upper = v01 + (v11 - v01) * fx;

        Ok(lower + (upper - lower) * fy)
    }

    /// The heights of a piece of ground.
    ///
    /// The figures are taken from the posts that lie inside the rectangle, and not from an
    /// interpolation between them.
    ///
    /// # Errors
    ///
    /// Returns an error when the rectangle holds no post with a height.
    pub fn elevation_in(&self, bounds: DtedBounds) -> Result<DtedElevation, DtedError> {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        let mut sum = 0.0;
        let mut count = 0_usize;

        let first_column = self.column_of(bounds.west);
        let last_column = self.column_of(bounds.east);
        let first_row = self.row_of(bounds.south);
        let last_row = self.row_of(bounds.north);

        for column in first_column..=last_column.min(self.header.columns.saturating_sub(1)) {
            for row in first_row..=last_row.min(self.header.rows.saturating_sub(1)) {
                let Some(height) = self.post(column, row) else {
                    continue;
                };

                if height == NODATA {
                    continue;
                }

                let height = f64::from(height);
                min = min.min(height);
                max = max.max(height);
                sum += height;
                count += 1;
            }
        }

        if count == 0 {
            return Err(DtedError::NoData {
                lat: bounds.south,
                lng: bounds.west,
            });
        }

        Ok(DtedElevation {
            min_m: min,
            max_m: max,
            mean_m: sum / count as f64,
            samples: count,
        })
    }

    /// Whether the cell covers a place.
    #[must_use]
    pub fn contains(&self, lat: f64, lng: f64) -> bool {
        let last_x = (self.header.columns.saturating_sub(1)) as f64;
        let last_y = (self.header.rows.saturating_sub(1)) as f64;

        let x = (lng - self.header.origin_lng) / self.header.spacing_lng;
        let y = (lat - self.header.origin_lat) / self.header.spacing_lat;

        x.is_finite()
            && y.is_finite()
            && x >= -EDGE_TOLERANCE
            && y >= -EDGE_TOLERANCE
            && x <= last_x + EDGE_TOLERANCE
            && y <= last_y + EDGE_TOLERANCE
    }

    /// The height of one post of the grid, in metres above the sea.
    ///
    /// The rows are counted from the south. A post with no height is `None`.
    #[must_use]
    pub fn post(&self, column: usize, row: usize) -> Option<i16> {
        if column >= self.header.columns || row >= self.header.rows {
            return None;
        }

        let height = self.posts[column * self.header.rows + row];

        if height == NODATA {
            None
        } else {
            Some(height)
        }
    }

    /// The height of one post of the grid, in metres above the sea.
    fn height(&self, column: usize, row: usize, lat: f64, lng: f64) -> Result<f64, DtedError> {
        self.post(column, row)
            .map(f64::from)
            .ok_or(DtedError::NoData { lat, lng })
    }

    /// The column of the grid a longitude falls in.
    fn column_of(&self, lng: f64) -> usize {
        let column = (lng - self.header.origin_lng) / self.header.spacing_lng;

        if column.is_finite() {
            // The rounding of the arithmetic must not throw away a post that is there.
            (column + 1e-9).floor().max(0.0) as usize
        } else {
            0
        }
    }

    /// The row of the grid a latitude falls in, counted from the south.
    fn row_of(&self, lat: f64) -> usize {
        let row = (lat - self.header.origin_lat) / self.header.spacing_lat;

        if row.is_finite() {
            (row + 1e-9).floor().max(0.0) as usize
        } else {
            0
        }
    }
}

impl TerrainProvider for DtedFile {
    fn elevation(&self, position: LatLng) -> Result<Elevation, TerrainError> {
        self.elevation_at(position.lat, position.lng)
            .map(|metres| Elevation {
                metres,
                source: ElevationSource::Dted,
            })
            .map_err(TerrainError::from)
    }
}

/// A set of DTED files: the cells of a piece of the world.
///
/// The files of the agency come in a folder, one per degree of the ground, and the operator
/// points at the folder. The library reads them all and answers the questions about the
/// height of the ground anywhere inside them.
///
/// ```
/// use mpcraft_geo::dted::DtedLibrary;
///
/// let library = DtedLibrary::new();
///
/// assert_eq!(library.len(), 0);
/// ```
#[derive(Debug, Clone, Default)]
pub struct DtedLibrary {
    cells: Vec<DtedFile>,
    /// The files that could not be read.
    failures: usize,
}

impl DtedLibrary {
    /// Creates an empty library.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads every DTED file of a folder.
    ///
    /// The files that end in `.dt0`, `.dt1` and `.dt2` are read. A file that cannot be read
    /// is left out and counted, so that the operator can be told about it.
    ///
    /// # Errors
    ///
    /// Returns an error when the folder cannot be read.
    pub fn from_directory(directory: impl AsRef<Path>) -> Result<Self, DtedError> {
        let mut library = Self::new();

        let entries = std::fs::read_dir(directory)?;

        for entry in entries {
            let Ok(entry) = entry else {
                library.failures += 1;
                continue;
            };

            let path = entry.path();

            if !is_dted_file(&path) {
                continue;
            }

            match DtedFile::open(&path) {
                Ok(cell) => library.cells.push(cell),
                Err(_) => library.failures += 1,
            }
        }

        Ok(library)
    }

    /// Adds a cell to the library.
    pub fn add(&mut self, cell: DtedFile) {
        self.cells.push(cell);
    }

    /// The cells of the library.
    #[must_use]
    pub fn cells(&self) -> &[DtedFile] {
        &self.cells
    }

    /// The number of cells of the library.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether the library is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// The number of files that could not be read.
    #[must_use]
    pub fn failures(&self) -> usize {
        self.failures
    }

    /// The cell that covers a place, when there is one.
    #[must_use]
    pub fn cell_at(&self, lat: f64, lng: f64) -> Option<&DtedFile> {
        self.cells.iter().find(|cell| cell.contains(lat, lng))
    }
}

impl TerrainProvider for DtedLibrary {
    fn elevation(&self, position: LatLng) -> Result<Elevation, TerrainError> {
        let cell = self
            .cell_at(position.lat, position.lng)
            .ok_or(TerrainError::NoData {
                lat: position.lat,
                lng: position.lng,
            })?;

        cell.elevation(position)
    }
}

/// Whether a file is one of the files of the agency.
fn is_dted_file(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };

    matches!(
        extension.to_ascii_lowercase().as_str(),
        "dt0" | "dt1" | "dt2"
    )
}

/// The place the user header label starts at.
///
/// The files that come from a tape carry a volume and a header label before it.
fn uhl_offset(bytes: &[u8]) -> Result<usize, DtedError> {
    let mut start = 0_usize;

    while start + UHL_SIZE <= bytes.len() {
        let record = &bytes[start..start + UHL_SIZE];

        if starts_with_ignore_case(record, b"VOL") || starts_with_ignore_case(record, b"HDR") {
            start += UHL_SIZE;
            continue;
        }

        if starts_with_ignore_case(record, b"UHL") {
            return Ok(start);
        }

        break;
    }

    Err(DtedError::NotDted(
        "the file does not begin with a user header label".to_owned(),
    ))
}

/// The place the data of the file starts at.
fn data_offset(bytes: &[u8], uhl_start: usize) -> usize {
    let dsi = uhl_start + UHL_SIZE;
    let acc = dsi + DSI_SIZE;

    if acc + ACC_SIZE <= bytes.len() && starts_with_ignore_case(&bytes[acc..], b"ACC") {
        return acc + ACC_SIZE;
    }

    // A file that leaves one of the records out still says where the accuracy record is.
    if dsi < bytes.len() {
        if let Some(found) = find_ignore_case(&bytes[dsi..], b"ACC") {
            let end = dsi + found + ACC_SIZE;

            if end <= bytes.len() {
                return end;
            }
        }
    }

    (uhl_start + DATA_OFFSET).min(bytes.len())
}

/// Reads the header of a file.
///
/// Two shapes of the user header label are found in the world. The files of the agency carry
/// the level as a single character and the place of the cell right after it; the first
/// edition of the standard carries the name of the file — `DTED1` — where the level is, and
/// the fields of the cell after it. Both are read.
fn parse_header(uhl: &[u8]) -> Result<DtedHeader, DtedError> {
    let level = detect_level(uhl).ok_or_else(|| {
        DtedError::MalformedHeader("the header does not say which level the file is".to_owned())
    })?;

    let shapes = [
        // The level is one character, and the cell follows it.
        HeaderShape {
            longitude: 4,
            latitude: 12,
            spacing_lng: 20,
            spacing_lat: 24,
            columns: Some(47),
            rows: Some(51),
        },
        // The name of the file is where the level is, and the cell follows it.
        HeaderShape {
            longitude: 9,
            latitude: 17,
            spacing_lng: 25,
            spacing_lat: 29,
            columns: None,
            rows: None,
        },
    ];

    for shape in shapes {
        if let Some(header) = read_header_shape(uhl, level, shape) {
            return Ok(header);
        }
    }

    Err(DtedError::MalformedHeader(
        "the header does not say where the cell is".to_owned(),
    ))
}

/// Where the fields of the user header label are.
struct HeaderShape {
    /// The place the longitude of the south-west corner is.
    longitude: usize,
    /// The place the latitude of the south-west corner is.
    latitude: usize,
    /// The place the distance between the posts of the east and the west is.
    spacing_lng: usize,
    /// The place the distance between the posts of the north and the south is.
    spacing_lat: usize,
    /// The place the number of posts of the east and the west is.
    columns: Option<usize>,
    /// The place the number of posts of the north and the south is.
    rows: Option<usize>,
}

/// Reads the header of a file with the given shape.
fn read_header_shape(uhl: &[u8], level: DtedLevel, shape: HeaderShape) -> Option<DtedHeader> {
    let longitude_field = uhl.get(shape.longitude..shape.longitude + 8)?;
    let latitude_field = uhl.get(shape.latitude..shape.latitude + 8)?;

    // The longitude comes first and the latitude after it, but the first edition of the
    // standard had them the other way round: the hemisphere tells which is which.
    let mut longitude = parse_dms(longitude_field)?;
    let mut latitude = parse_dms(latitude_field)?;

    let longitude_hemisphere = longitude_field[7].to_ascii_uppercase();
    let latitude_hemisphere = latitude_field[7].to_ascii_uppercase();

    let swapped =
        matches!(longitude_hemisphere, b'N' | b'S') && matches!(latitude_hemisphere, b'E' | b'W');

    if swapped {
        std::mem::swap(&mut longitude, &mut latitude);
    }

    let mut spacing_lng = tenths_of_arc_second(uhl.get(shape.spacing_lng..shape.spacing_lng + 4)?);
    let mut spacing_lat = tenths_of_arc_second(uhl.get(shape.spacing_lat..shape.spacing_lat + 4)?);

    if spacing_lng <= 0.0 || spacing_lat <= 0.0 {
        let spacing = level.post_spacing_arcsec() / 10.0;
        spacing_lng = spacing;
        spacing_lat = spacing;
    }

    let columns = shape
        .columns
        .and_then(|start| uhl.get(start..start + 4))
        .map_or(0, number);
    let rows = shape
        .rows
        .and_then(|start| uhl.get(start..start + 4))
        .map_or(0, number);

    Some(DtedHeader {
        level,
        origin_lat: latitude,
        origin_lng: longitude,
        spacing_lat,
        spacing_lng,
        columns,
        rows,
        vertical_datum: vertical_datum(uhl),
    })
}

/// The level a file says it is.
fn detect_level(uhl: &[u8]) -> Option<DtedLevel> {
    for (token, level) in [
        (&b"DTED0"[..], DtedLevel::Level0),
        (&b"DTED1"[..], DtedLevel::Level1),
        (&b"DTED2"[..], DtedLevel::Level2),
    ] {
        if find_ignore_case(uhl, token).is_some() {
            return Some(level);
        }
    }

    match uhl.get(3) {
        Some(b'0') => Some(DtedLevel::Level0),
        Some(b'1') => Some(DtedLevel::Level1),
        Some(b'2') => Some(DtedLevel::Level2),
        _ => None,
    }
}

/// The vertical datum a file declares, when it declares one.
fn vertical_datum(uhl: &[u8]) -> Option<String> {
    let position = find_ignore_case(uhl, b"VERTCG")?;

    let mut start = position;
    while start > 0 && !uhl[start - 1].is_ascii_whitespace() {
        start -= 1;
    }

    let mut end = position + 6;
    while end < uhl.len() && !uhl[end].is_ascii_whitespace() {
        end += 1;
    }

    Some(String::from_utf8_lossy(&uhl[start..end]).trim().to_owned())
}

/// The heights of a file, read with the shape the file has.
fn decode(
    bytes: &[u8],
    data_start: usize,
    header: &DtedHeader,
) -> Result<(DtedLayout, Vec<i16>, usize), DtedError> {
    let data_len = bytes.len().saturating_sub(data_start);
    let columns = header.columns;
    let rows = header.rows;

    if columns == 0 || rows == 0 {
        return Err(DtedError::MalformedData(
            "the header does not say how many posts the cell has".to_owned(),
        ));
    }

    let candidates = [
        (
            DtedLayout::Standard,
            columns * (RECORD_HEADER_SIZE + 2 * rows + CHECKSUM_SIZE),
        ),
        (
            DtedLayout::Padded,
            columns * (RECORD_HEADER_SIZE + 6 * rows + CHECKSUM_SIZE),
        ),
        (DtedLayout::Flat, columns * rows * 2),
        (DtedLayout::FlatPadded, columns * rows * 6),
    ];

    if let Some((layout, _)) = candidates
        .iter()
        .find(|(_, size)| *size == data_len)
        .copied()
    {
        return read_data(bytes, data_start, header, layout, columns);
    }

    // A cell that is only partly there: the records that are there are read, and the rest of
    // the cell is left empty.
    for (layout, post_size) in [
        (DtedLayout::Standard, 2_usize),
        (DtedLayout::Padded, 6_usize),
    ] {
        let record_size = RECORD_HEADER_SIZE + post_size * rows + CHECKSUM_SIZE;

        if record_size > 0
            && data_len.is_multiple_of(record_size)
            && data_len / record_size < columns
        {
            return read_data(bytes, data_start, header, layout, columns);
        }
    }

    Err(DtedError::MalformedData(format!(
        "the file holds {data_len} bytes of heights, which is not the shape of a cell of {} posts",
        columns * rows
    )))
}

/// Reads the heights of a file into the grid.
fn read_data(
    bytes: &[u8],
    data_start: usize,
    header: &DtedHeader,
    layout: DtedLayout,
    columns: usize,
) -> Result<(DtedLayout, Vec<i16>, usize), DtedError> {
    let rows = header.rows;
    let mut posts = vec![NODATA; columns * rows];
    let mut checksum_failures = 0_usize;

    match layout {
        DtedLayout::Standard | DtedLayout::Padded => {
            let post_size = layout.post_size();
            let record_size = RECORD_HEADER_SIZE + post_size * rows + CHECKSUM_SIZE;
            let available = bytes.len().saturating_sub(data_start);
            let count = available / record_size;

            for record in 0..count {
                let start = data_start + record * record_size;
                let Some(record_bytes) = bytes.get(start..start + record_size) else {
                    break;
                };

                // The header of the record says which column it holds, which is how a cell
                // that is only partly there is put back together.
                let column = longitude_count(record_bytes).unwrap_or(record);

                if column >= columns {
                    continue;
                }

                for row in 0..rows {
                    let offset = RECORD_HEADER_SIZE + row * post_size;

                    if let Some(post) = record_bytes.get(offset..offset + 2) {
                        posts[column * rows + row] = decode_post(post, layout);
                    }
                }

                if layout == DtedLayout::Standard
                    && checksum_of(record_bytes) != record_checksum(record_bytes)
                {
                    checksum_failures += 1;
                }
            }
        }
        DtedLayout::Flat | DtedLayout::FlatPadded => {
            let post_size = layout.post_size();

            'posts: for column in 0..columns {
                for row in 0..rows {
                    let offset = column * rows * post_size + row * post_size;

                    let Some(post) = bytes.get(data_start + offset..data_start + offset + 2) else {
                        break 'posts;
                    };

                    posts[column * rows + row] = decode_post(post, layout);
                }
            }
        }
    }

    Ok((layout, posts, checksum_failures))
}

/// Reads one height.
fn decode_post(bytes: &[u8], layout: DtedLayout) -> i16 {
    let Some(raw) = bytes.get(0..2) else {
        return NODATA;
    };

    let raw = u16::from_be_bytes([raw[0], raw[1]]);

    match layout {
        DtedLayout::Standard => {
            // The standard writes the height as a magnitude with the sign in the top bit.
            let mut value = i32::from(raw & 0x7fff);

            if raw & 0x8000 != 0 {
                value = -value;

                // Some producers wrote the negative heights in two's complement by mistake.
                if value < -16_000 && value != i32::from(NODATA) {
                    value = i32::from(raw as i16);
                }
            }

            value as i16
        }
        DtedLayout::Padded | DtedLayout::Flat | DtedLayout::FlatPadded => raw as i16,
    }
}

/// The column a record says it holds.
fn longitude_count(record: &[u8]) -> Option<usize> {
    if !record.first().is_some_and(|byte| *byte == 0xaa) {
        return None;
    }

    let count = u16::from_be_bytes([record[4], record[5]]);

    Some(count as usize)
}

/// The checksum a record carries.
fn record_checksum(record: &[u8]) -> u32 {
    let Some(tail) = record.get(record.len() - 4..) else {
        return 0;
    };

    u32::from_be_bytes([tail[0], tail[1], tail[2], tail[3]])
}

/// The checksum the bytes of a record add up to.
fn checksum_of(record: &[u8]) -> u32 {
    record
        .iter()
        .take(record.len().saturating_sub(4))
        .map(|byte| u32::from(*byte))
        .sum()
}

/// Reads a number out of a field of the header.
fn number(field: &[u8]) -> usize {
    let mut value = 0_usize;

    for byte in field {
        if !byte.is_ascii_digit() {
            return 0;
        }

        value = value * 10 + usize::from(byte - b'0');
    }

    value
}

/// Reads a distance in tenths of an arc-second out of a field of the header.
fn tenths_of_arc_second(field: &[u8]) -> f64 {
    number(field) as f64 * TENTH_OF_ARC_SECOND
}

/// Reads a position out of a field of the header.
fn parse_dms(field: &[u8]) -> Option<f64> {
    if field.len() < 8 {
        return None;
    }

    let degrees = number(&field[0..3]);
    let minutes = number(&field[3..5]);
    let seconds = number(&field[5..7]);

    let hemisphere = field[7].to_ascii_uppercase();

    let mut value = degrees as f64 + minutes as f64 / 60.0 + seconds as f64 / 3600.0;

    match hemisphere {
        b'N' | b'E' => {}
        b'S' | b'W' => value = -value,
        _ => return None,
    }

    Some(value)
}

/// Whether a record begins with the given characters.
fn starts_with_ignore_case(record: &[u8], prefix: &[u8]) -> bool {
    record.len() >= prefix.len() && record[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// Looks for a sequence of characters in a record.
fn find_ignore_case(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }

    haystack
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cell of the ground with the heights written in it.
    struct Cell {
        bytes: Vec<u8>,
        columns: usize,
        rows: usize,
    }

    impl Cell {
        /// A level 1 cell whose south-west corner is at the given place.
        fn new(origin_lat: f64, origin_lng: f64, columns: usize, rows: usize) -> Self {
            let mut bytes =
                vec![0_u8; DATA_OFFSET + columns * (RECORD_HEADER_SIZE + 2 * rows + CHECKSUM_SIZE)];

            // The user header label.
            bytes[0..3].copy_from_slice(b"UHL");
            bytes[3] = b'1';
            write_dms(&mut bytes[4..12], origin_lng, 'E');
            write_dms(&mut bytes[12..20], origin_lat, 'S');
            write_number(&mut bytes[20..24], 30);
            write_number(&mut bytes[24..28], 30);
            write_number(&mut bytes[47..51], columns);
            write_number(&mut bytes[51..55], rows);

            // The data set identification and the accuracy record.
            bytes[UHL_SIZE..UHL_SIZE + 3].copy_from_slice(b"DSI");
            bytes[UHL_SIZE + DSI_SIZE..UHL_SIZE + DSI_SIZE + 3].copy_from_slice(b"ACC");

            Self {
                bytes,
                columns,
                rows,
            }
        }

        /// Writes the height of a post.
        fn set(&mut self, column: usize, row: usize, height: i16) {
            let record =
                DATA_OFFSET + column * (RECORD_HEADER_SIZE + 2 * self.rows + CHECKSUM_SIZE);
            let offset = record + RECORD_HEADER_SIZE + row * 2;

            let magnitude = height.unsigned_abs();
            self.bytes[offset] = (magnitude >> 8) as u8;
            self.bytes[offset + 1] = magnitude as u8;

            if height < 0 {
                self.bytes[offset] |= 0x80;
            }

            self.stamp(column);
        }

        /// Writes the header and the checksum of the record of a column.
        fn stamp(&mut self, column: usize) {
            let record =
                DATA_OFFSET + column * (RECORD_HEADER_SIZE + 2 * self.rows + CHECKSUM_SIZE);
            let size = RECORD_HEADER_SIZE + 2 * self.rows + CHECKSUM_SIZE;

            self.bytes[record] = 0xaa;
            self.bytes[record + 1] = 0;
            self.bytes[record + 2] = (column >> 8) as u8;
            self.bytes[record + 3] = column as u8;
            self.bytes[record + 4] = (column >> 8) as u8;
            self.bytes[record + 5] = column as u8;
            self.bytes[record + 6] = 0;
            self.bytes[record + 7] = 0;

            let checksum = checksum_of(&self.bytes[record..record + size]);
            self.bytes[record + size - 4] = (checksum >> 24) as u8;
            self.bytes[record + size - 3] = (checksum >> 16) as u8;
            self.bytes[record + size - 2] = (checksum >> 8) as u8;
            self.bytes[record + size - 1] = checksum as u8;
        }

        /// The finished file.
        fn finish(mut self) -> Vec<u8> {
            for column in 0..self.columns {
                self.stamp(column);
            }

            self.bytes
        }
    }

    /// Writes a position into a field of the header.
    fn write_dms(field: &mut [u8], value: f64, hemisphere: char) {
        let sign = value.signum();
        let absolute = value.abs();
        let degrees = absolute.floor() as usize;
        let minutes_exact = (absolute - degrees as f64) * 60.0;
        let minutes = minutes_exact.floor() as usize;
        let seconds = ((minutes_exact - minutes as f64) * 60.0).round() as usize;

        write_number(&mut field[0..3], degrees);
        write_number(&mut field[3..5], minutes);
        write_number(&mut field[5..7], seconds);
        field[7] = if sign < 0.0 {
            match hemisphere {
                'N' => b'S',
                'E' => b'W',
                other => other as u8,
            }
        } else {
            hemisphere as u8
        };
    }

    /// Writes a number into a field of the header.
    fn write_number(field: &mut [u8], value: usize) {
        let text = format!("{value:0width$}", width = field.len());

        field.copy_from_slice(text.as_bytes());
    }

    #[test]
    fn a_cell_is_read() {
        let mut cell = Cell::new(-35.0, 149.0, 4, 3);
        cell.set(0, 0, 100);
        cell.set(1, 0, 200);
        cell.set(2, 0, 300);
        cell.set(3, 0, 400);

        let file = DtedFile::parse(&cell.finish()).expect("the cell is read");

        assert_eq!(file.level(), DtedLevel::Level1);
        assert_eq!(file.header().columns, 4);
        assert_eq!(file.header().rows, 3);
        assert!((file.header().origin_lat + 35.0).abs() < 1e-9);
        assert!((file.header().origin_lng - 149.0).abs() < 1e-9);
        assert_eq!(file.layout(), DtedLayout::Standard);
        assert_eq!(file.checksum_failures(), 0);
    }

    #[test]
    fn the_height_of_a_post_is_read() {
        let mut cell = Cell::new(-35.0, 149.0, 4, 3);
        cell.set(0, 0, 100);
        cell.set(3, 2, 400);

        let file = DtedFile::parse(&cell.finish()).expect("the cell is read");

        assert_eq!(file.post(0, 0), Some(100));
        assert_eq!(file.post(3, 2), Some(400));
        assert_eq!(file.post(1, 1), Some(0));
    }

    #[test]
    fn the_height_between_the_posts_is_interpolated() {
        let mut cell = Cell::new(-35.0, 149.0, 2, 2);
        cell.set(0, 0, 0);
        cell.set(1, 0, 100);
        cell.set(0, 1, 0);
        cell.set(1, 1, 100);

        let file = DtedFile::parse(&cell.finish()).expect("the cell is read");

        let spacing = file.header().spacing_lng;
        let middle = file
            .elevation_at(-35.0, 149.0 + spacing / 2.0)
            .expect("there is a height in the middle");

        assert!((middle - 50.0).abs() < 1e-6, "the height was {middle}");
    }

    #[test]
    fn a_place_outside_the_cell_has_no_height() {
        let cell = Cell::new(-35.0, 149.0, 4, 3);
        let file = DtedFile::parse(&cell.finish()).expect("the cell is read");

        let error = file
            .elevation_at(10.0, 149.0)
            .expect_err("the place is far away");

        assert!(matches!(error, DtedError::NoData { .. }));
        assert!(!file.contains(10.0, 149.0));
    }

    #[test]
    fn the_heights_of_a_piece_of_ground_are_read() {
        let mut cell = Cell::new(-35.0, 149.0, 4, 3);

        for column in 0..4 {
            for row in 0..3 {
                cell.set(column, row, 100 * (column as i16 + 1) + row as i16);
            }
        }

        let file = DtedFile::parse(&cell.finish()).expect("the cell is read");

        let bounds = DtedBounds::new(
            -35.0,
            149.0,
            -35.0 + file.header().spacing_lat * 2.0,
            149.0 + file.header().spacing_lng * 2.0,
        );

        let elevation = file.elevation_in(bounds).expect("there are heights");

        // The rectangle holds the nine posts of the three first columns and rows.
        assert_eq!(elevation.samples, 9);
        assert!((elevation.min_m - 100.0).abs() < 1e-9);
        assert!((elevation.max_m - 302.0).abs() < 1e-9);
        assert!((elevation.mean_m - 201.0).abs() < 1e-9);
    }

    #[test]
    fn a_negative_height_is_read() {
        let mut cell = Cell::new(-35.0, 149.0, 2, 2);
        cell.set(0, 0, -120);

        let file = DtedFile::parse(&cell.finish()).expect("the cell is read");

        assert_eq!(file.post(0, 0), Some(-120));
    }

    #[test]
    fn a_post_with_no_height_is_not_a_height() {
        let mut cell = Cell::new(-35.0, 149.0, 2, 2);
        cell.set(0, 0, NODATA);

        let file = DtedFile::parse(&cell.finish()).expect("the cell is read");

        assert_eq!(file.post(0, 0), None);
        assert!(file.elevation_at(-35.0, 149.0).is_err());
    }

    #[test]
    fn a_file_that_is_not_dted_is_refused() {
        let bytes = vec![0_u8; 4096];

        let error = DtedFile::parse(&bytes).expect_err("this is not a DTED file");

        assert!(matches!(error, DtedError::NotDted(_)));
    }

    #[test]
    fn a_file_whose_data_does_not_fit_the_header_is_refused() {
        let mut cell = Cell::new(-35.0, 149.0, 4, 3);
        cell.bytes.truncate(DATA_OFFSET + 10);

        let error = DtedFile::parse(&cell.bytes).expect_err("the data is cut");

        assert!(matches!(error, DtedError::MalformedData(_)));
    }

    #[test]
    fn a_cell_whose_posts_are_padded_is_read() {
        // The same cell, but with the posts of six bytes.
        let cell = Cell::new(-35.0, 149.0, 4, 3);
        let record_size = RECORD_HEADER_SIZE + 6 * 3 + CHECKSUM_SIZE;
        let mut bytes = vec![0_u8; DATA_OFFSET + 4 * record_size];
        bytes[0..80].copy_from_slice(&cell.bytes[0..80]);

        for column in 0..4 {
            let record = DATA_OFFSET + column * record_size;
            bytes[record] = 0xaa;
            bytes[record + 4] = (column >> 8) as u8;
            bytes[record + 5] = column as u8;

            for row in 0..3 {
                let offset = record + RECORD_HEADER_SIZE + row * 6;
                let height = (column * 100 + row) as i16;
                bytes[offset..offset + 2].copy_from_slice(&height.to_be_bytes());
            }
        }

        let file = DtedFile::parse(&bytes).expect("the cell is read");

        assert_eq!(file.layout(), DtedLayout::Padded);
        assert_eq!(file.post(2, 1), Some(201));
    }

    #[test]
    fn a_header_with_the_name_of_the_file_is_read() {
        // The first edition of the standard: the name of the file is where the level is,
        // and the fields of the cell follow it.
        let mut cell = Cell::new(-35.0, 149.0, 121, 121);
        let mut bytes = cell.bytes.clone();

        let mut header = vec![0_u8; 80];
        header[0..3].copy_from_slice(b"UHL");
        header[4..9].copy_from_slice(b"DTED0");
        write_dms(&mut header[9..17], 149.0, 'E');
        write_dms(&mut header[17..25], -35.0, 'S');
        write_number(&mut header[25..29], 30);
        write_number(&mut header[29..33], 30);
        bytes[0..80].copy_from_slice(&header);

        cell.bytes = bytes.clone();
        cell.set(0, 0, 250);

        let file = DtedFile::parse(&cell.finish()).expect("the cell is read");

        assert_eq!(file.level(), DtedLevel::Level0);
        assert!((file.header().origin_lng - 149.0).abs() < 1e-9);
        assert!((file.header().origin_lat + 35.0).abs() < 1e-9);
        assert_eq!(file.post(0, 0), Some(250));
    }

    #[test]
    fn a_file_with_a_volume_label_before_it_is_read() {
        let cell = Cell::new(-35.0, 149.0, 4, 3);
        let mut bytes = vec![0_u8; 80];
        bytes[0..3].copy_from_slice(b"VOL");

        bytes.extend_from_slice(&cell.finish());

        let file = DtedFile::parse(&bytes).expect("the cell is read");

        assert_eq!(file.level(), DtedLevel::Level1);
    }

    #[test]
    fn a_library_of_files_is_read() {
        let directory = std::env::temp_dir().join("mpcraft-dted-test");

        // The folder of a previous run may still be there.
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("the folder is made");

        let cell = Cell::new(-35.0, 149.0, 4, 3);
        std::fs::write(directory.join("cell.dt1"), cell.finish()).expect("the file is written");

        // A file that is not a DTED file is left out and counted.
        std::fs::write(directory.join("broken.dt1"), b"not a dted file")
            .expect("the file is written");

        // A file that is not one of the files of the agency is not even looked at.
        std::fs::write(directory.join("notes.txt"), b"hello").expect("the file is written");

        let library = DtedLibrary::from_directory(&directory).expect("the folder is read");

        assert_eq!(library.len(), 1);
        assert_eq!(library.failures(), 1);
        assert!(library.cell_at(-35.0, 149.0).is_some());
        assert!(library.cell_at(10.0, 149.0).is_none());

        let height = library
            .elevation(LatLng::new(-35.0, 149.0))
            .expect("there is a height");

        assert_eq!(height.source, ElevationSource::Dted);

        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn the_level_of_the_header_is_read() {
        let mut cell = Cell::new(-35.0, 149.0, 4, 3);
        cell.bytes[3] = b'2';

        let file = DtedFile::parse(&cell.finish()).expect("the cell is read");

        assert_eq!(file.level(), DtedLevel::Level2);
    }

    #[test]
    fn the_level_is_read_from_the_name_of_the_file() {
        let mut cell = Cell::new(-35.0, 149.0, 4, 3);
        cell.bytes[3] = b' ';
        cell.bytes[4..9].copy_from_slice(b"DTED1");

        let file = DtedFile::parse(&cell.finish()).expect("the cell is read");

        assert_eq!(file.level(), DtedLevel::Level1);
    }

    #[test]
    fn the_level_gives_the_shape_of_the_cell() {
        assert_eq!(DtedLevel::Level0.rows(), 121);
        assert_eq!(DtedLevel::Level1.rows(), 1201);
        assert_eq!(DtedLevel::Level2.rows(), 3601);

        // The zones of latitude make the posts of the east and the west further apart.
        assert_eq!(DtedLevel::Level1.columns(10.0), 1201);
        assert_eq!(DtedLevel::Level1.columns(60.0), 601);
        assert_eq!(DtedLevel::Level1.columns(85.0), 201);
    }
}
