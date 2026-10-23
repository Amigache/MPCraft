//! The georeference of a picture.
//!
//! A picture taken by the camera of a vehicle says where it was taken: the EXIF block of a
//! JPEG or a TIFF carries the latitude, the longitude and the height above the sea, and the
//! instant the camera says the picture was made. Mission Planner reads that block in its
//! georef screen (`GeoRefImageBase`) and puts the pictures on the map; MPCraft does the same.
//!
//! The size of the picture is read too, because a picture placed on a map is a footprint and
//! not a point: without the width and the height there is nothing to draw but the place.
//!
//! # Why the whole file is read
//!
//! A tag of an EXIF block does not hold its value: it points at the bytes that hold it, and
//! those bytes can be anywhere inside the block, so the block cannot be read a record at a
//! time the way a log is. A picture is a few megabytes, which is what a ground station can
//! hold, and the block is read in one piece.
//!
//! # What the time means
//!
//! The time the camera wrote is taken as it is written, with no timezone: a camera clock is
//! not a log clock, and the difference between the two is what the ground station estimates
//! from the pictures and the log together.

use serde::{Deserialize, Serialize};

use crate::GeoPoint;

/// Why a picture could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GeorefError {
    /// The file is not a picture that carries an EXIF block.
    #[error("the file is not a picture with an EXIF block")]
    NotAPicture,

    /// The file is cut short.
    #[error("the picture is cut short")]
    Truncated,

    /// The picture has no EXIF block at all.
    #[error("the picture has no EXIF block")]
    NoExif,

    /// Something inside the block is not what it says it is.
    #[error("the EXIF block is not one that can be read: {0}")]
    Malformed(String),

    /// The georeference is written into a picture of the JPEG kind.
    #[error("the georeference is written into a picture of the JPEG kind")]
    NotAJpeg,
}

/// The kinds of value a tag can hold.
const TYPE_BYTE: u16 = 1;
const TYPE_ASCII: u16 = 2;
const TYPE_SHORT: u16 = 3;
const TYPE_LONG: u16 = 4;
const TYPE_RATIONAL: u16 = 5;
const TYPE_SLONG: u16 = 9;
const TYPE_SRATIONAL: u16 = 10;

/// The tags of the first block of the block.
const TAG_WIDTH: u16 = 256;
const TAG_HEIGHT: u16 = 257;
const TAG_ORIENTATION: u16 = 274;
const TAG_DATETIME: u16 = 306;
const TAG_EXIF_IFD: u16 = 34665;
const TAG_GPS_IFD: u16 = 34853;

/// The tag of the instant the picture was made.
const TAG_DATETIME_ORIGINAL: u16 = 36867;

/// The tags of the block of the position.
const GPS_TAG_LAT_REF: u16 = 1;
const GPS_TAG_LAT: u16 = 2;
const GPS_TAG_LON_REF: u16 = 3;
const GPS_TAG_LON: u16 = 4;
const GPS_TAG_ALT: u16 = 5;
const GPS_TAG_ALT_REF: u16 = 6;

/// The way a picture is turned, as the camera wrote it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orientation {
    /// The top of the picture is the top of the scene.
    Normal,
    /// The picture is flipped across the vertical.
    MirrorHorizontal,
    /// The picture is turned upside down.
    Rotate180,
    /// The picture is flipped across the horizontal.
    MirrorVertical,
    /// The picture is flipped and turned a quarter to the left.
    MirrorHorizontalRotate270,
    /// The picture is turned a quarter to the right.
    Rotate90,
    /// The picture is flipped and turned a quarter to the right.
    MirrorHorizontalRotate90,
    /// The picture is turned a quarter to the left.
    Rotate270,
}

impl Orientation {
    /// The way the picture is turned, from the number the file gives it.
    #[must_use]
    pub const fn from_code(code: u16) -> Option<Self> {
        match code {
            1 => Some(Orientation::Normal),
            2 => Some(Orientation::MirrorHorizontal),
            3 => Some(Orientation::Rotate180),
            4 => Some(Orientation::MirrorVertical),
            5 => Some(Orientation::MirrorHorizontalRotate270),
            6 => Some(Orientation::Rotate90),
            7 => Some(Orientation::MirrorHorizontalRotate90),
            8 => Some(Orientation::Rotate270),
            _ => None,
        }
    }

    /// The number the file gives the way the picture is turned.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Orientation::Normal => 1,
            Orientation::MirrorHorizontal => 2,
            Orientation::Rotate180 => 3,
            Orientation::MirrorVertical => 4,
            Orientation::MirrorHorizontalRotate270 => 5,
            Orientation::Rotate90 => 6,
            Orientation::MirrorHorizontalRotate90 => 7,
            Orientation::Rotate270 => 8,
        }
    }

    /// What the way the picture is turned is called, in the words an operator reads.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Orientation::Normal => "the top of the scene",
            Orientation::MirrorHorizontal => "flipped across the vertical",
            Orientation::Rotate180 => "turned upside down",
            Orientation::MirrorVertical => "flipped across the horizontal",
            Orientation::MirrorHorizontalRotate270 => "flipped and turned a quarter to the left",
            Orientation::Rotate90 => "turned a quarter to the right",
            Orientation::MirrorHorizontalRotate90 => "flipped and turned a quarter to the right",
            Orientation::Rotate270 => "turned a quarter to the left",
        }
    }
}

/// A picture and what its block says about where it was taken.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoreferencedImage {
    /// The width of the picture, in pixels.
    pub width: u32,
    /// The height of the picture, in pixels.
    pub height: u32,
    /// The way the picture is turned.
    pub orientation: Orientation,
    /// When the camera says the picture was made, in seconds since the epoch.
    pub taken_at: Option<i64>,
    /// Where the picture says it was taken, with the height above the sea in metres.
    pub position: Option<GeoPoint>,
}

impl GeoreferencedImage {
    /// Reads what a picture says about itself.
    ///
    /// # Errors
    ///
    /// Returns an error when the bytes are not a picture, when they are cut short, or when the
    /// block inside them is not one that can be read.
    pub fn parse(bytes: &[u8]) -> Result<Self, GeorefError> {
        let tiff = Tiff::new(exif_block(bytes)?)?;
        let root = tiff.entries(tiff.root())?;

        let mut width = 0_u32;
        let mut height = 0_u32;
        let mut orientation = Orientation::Normal;
        let mut written = None;
        let mut made = None;
        let mut gps = None;

        for entry in &root {
            match entry.tag {
                TAG_WIDTH => width = tiff.number(entry)?,
                TAG_HEIGHT => height = tiff.number(entry)?,
                TAG_ORIENTATION => orientation = orientation_of(tiff.number(entry)?)?,
                TAG_DATETIME => written = Some(tiff.text(entry)?),
                TAG_EXIF_IFD => {
                    made = Some(usize::try_from(tiff.number(entry)?).unwrap_or(usize::MAX))
                }
                TAG_GPS_IFD => {
                    gps = Some(usize::try_from(tiff.number(entry)?).unwrap_or(usize::MAX))
                }
                _ => {}
            }
        }

        // A picture of the JPEG kind says its size where the pixels begin, and the block does
        // not always say it.
        if width == 0 || height == 0 {
            if let Some((found_width, found_height)) = jpeg_size(bytes) {
                width = found_width;
                height = found_height;
            }
        }

        // The instant the camera says the picture was made is in the block the first block
        // points to; a camera that only wrote the first one is read with what it wrote there.
        let taken_at = match made {
            Some(ifd) => tiff
                .entries(ifd)?
                .iter()
                .find(|entry| entry.tag == TAG_DATETIME_ORIGINAL)
                .map(|entry| tiff.text(entry))
                .transpose()?
                .or(written),
            None => written,
        };

        let position = match gps {
            Some(ifd) => read_position(&tiff, &tiff.entries(ifd)?)?,
            None => None,
        };

        Ok(Self {
            width,
            height,
            orientation,
            taken_at: taken_at.as_deref().and_then(parse_time),
            position,
        })
    }
}

/// One tag of a block: what it is, what kind its value is, and the four bytes that say where
/// the value is.
struct Entry {
    tag: u16,
    kind: u16,
    count: u32,
    /// The four bytes of the field of the value, as they are written in the file.
    raw: [u8; 4],
}

/// The block of EXIF, which is a small TIFF inside the picture.
struct Tiff<'a> {
    /// The bytes of the block.
    bytes: &'a [u8],
    /// Whether the numbers of the block are written with the smallest digit first.
    little: bool,
    /// Where the first block of tags begins.
    root: usize,
}

impl<'a> Tiff<'a> {
    /// A block, with the order its numbers are written in.
    fn new(bytes: &'a [u8]) -> Result<Self, GeorefError> {
        if bytes.len() < 8 {
            return Err(GeorefError::Truncated);
        }

        let little = match &bytes[0..2] {
            b"II" => true,
            b"MM" => false,
            _ => {
                return Err(GeorefError::Malformed(
                    "the block does not say which way its numbers are written".to_string(),
                ))
            }
        };

        if read_u16(bytes, 2, little)? != 42 {
            return Err(GeorefError::Malformed(
                "the block does not begin with the number that says it is one".to_string(),
            ));
        }

        let root = usize::try_from(read_u32(bytes, 4, little)?).unwrap_or(usize::MAX);

        if root + 2 > bytes.len() {
            return Err(GeorefError::Truncated);
        }

        Ok(Self {
            bytes,
            little,
            root,
        })
    }

    /// Where the first block of tags begins.
    fn root(&self) -> usize {
        self.root
    }

    /// The tags of one block.
    fn entries(&self, ifd: usize) -> Result<Vec<Entry>, GeorefError> {
        let count = usize::from(read_u16(self.bytes, ifd, self.little)?);

        if ifd + 2 + count * 12 > self.bytes.len() {
            return Err(GeorefError::Truncated);
        }

        let mut entries = Vec::with_capacity(count);

        for index in 0..count {
            let at = ifd + 2 + index * 12;

            entries.push(Entry {
                tag: read_u16(self.bytes, at, self.little)?,
                kind: read_u16(self.bytes, at + 2, self.little)?,
                count: read_u32(self.bytes, at + 4, self.little)?,
                raw: [
                    self.bytes[at + 8],
                    self.bytes[at + 9],
                    self.bytes[at + 10],
                    self.bytes[at + 11],
                ],
            });
        }

        Ok(entries)
    }

    /// Where the bytes that hold the value of a tag begin.
    fn value_at(&self, entry: &Entry) -> Result<usize, GeorefError> {
        Ok(usize::try_from(read_u32(&entry.raw, 0, self.little)?).unwrap_or(usize::MAX))
    }

    /// The value of a tag that holds a number.
    fn number(&self, entry: &Entry) -> Result<u32, GeorefError> {
        // A value that fits inside the four bytes of the field is written there, and one that
        // does not lives at the place the field points to.
        if entry.count * size_of_kind(entry.kind) > 4 {
            let at = self.value_at(entry)?;
            let bytes = self.bytes.get(at..at + 4).ok_or(GeorefError::Truncated)?;
            return read_u32(bytes, 0, self.little);
        }

        match entry.kind {
            TYPE_LONG => read_u32(&entry.raw, 0, self.little),
            TYPE_SHORT => Ok(u32::from(read_u16(&entry.raw, 0, self.little)?)),
            TYPE_BYTE => Ok(u32::from(entry.raw[0])),
            _ => Err(GeorefError::Malformed(format!(
                "the tag {} holds a number of a kind that is not known",
                entry.tag
            ))),
        }
    }

    /// The text of a tag.
    fn text(&self, entry: &Entry) -> Result<String, GeorefError> {
        if entry.kind != TYPE_ASCII {
            return Err(GeorefError::Malformed(format!(
                "the tag {} holds text of a kind that is not known",
                entry.tag
            )));
        }

        let count = usize::try_from(entry.count).map_err(|_| {
            GeorefError::Malformed("a tag asks for more text than a file can hold".to_string())
        })?;

        let bytes = if count > 4 {
            let at = self.value_at(entry)?;
            self.bytes
                .get(at..at + count)
                .ok_or(GeorefError::Truncated)?
        } else {
            // A short value is written inside the field of the tag itself.
            entry.raw.get(..count).ok_or(GeorefError::Truncated)?
        };

        Ok(String::from_utf8_lossy(bytes)
            .trim_end_matches('\0')
            .to_string())
    }

    /// The numbers of a tag that holds fractions.
    fn rationals(&self, entry: &Entry) -> Result<Vec<(i64, i64)>, GeorefError> {
        if entry.kind != TYPE_RATIONAL && entry.kind != TYPE_SRATIONAL {
            return Err(GeorefError::Malformed(format!(
                "the tag {} holds fractions of a kind that is not known",
                entry.tag
            )));
        }

        let count = usize::try_from(entry.count).map_err(|_| {
            GeorefError::Malformed("a tag asks for more fractions than a file can hold".to_string())
        })?;

        let at = self.value_at(entry)?;
        let signed = entry.kind == TYPE_SRATIONAL;

        let mut values = Vec::with_capacity(count);

        for index in 0..count {
            let where_it_is = at + index * 8;
            let bytes = self
                .bytes
                .get(where_it_is..where_it_is + 8)
                .ok_or(GeorefError::Truncated)?;

            let top = if signed {
                i64::from(read_i32(bytes, 0, self.little)?)
            } else {
                i64::from(read_u32(bytes, 0, self.little)?)
            };

            values.push((top, i64::from(read_u32(bytes, 4, self.little)?)));
        }

        Ok(values)
    }
}

/// The way the picture is turned, from the number the block gives it.
fn orientation_of(code: u32) -> Result<Orientation, GeorefError> {
    let code = u16::try_from(code).map_err(|_| {
        GeorefError::Malformed("the way the picture is turned is not one that is known".to_string())
    })?;

    Orientation::from_code(code).ok_or(GeorefError::Malformed(
        "the way the picture is turned is not one that is known".to_string(),
    ))
}

/// The number of bytes of a value of the given kind.
fn size_of_kind(kind: u16) -> u32 {
    match kind {
        TYPE_BYTE | TYPE_ASCII => 1,
        TYPE_SHORT | TYPE_LONG | TYPE_SLONG => 4,
        _ => 8,
    }
}

/// A number of two bytes, read the way the block writes them.
fn read_u16(bytes: &[u8], at: usize, little: bool) -> Result<u16, GeorefError> {
    let four = bytes.get(at..at + 2).ok_or(GeorefError::Truncated)?;

    Ok(if little {
        u16::from_le_bytes([four[0], four[1]])
    } else {
        u16::from_be_bytes([four[0], four[1]])
    })
}

/// A number of four bytes, read the way the block writes them.
fn read_u32(bytes: &[u8], at: usize, little: bool) -> Result<u32, GeorefError> {
    let four = bytes.get(at..at + 4).ok_or(GeorefError::Truncated)?;

    Ok(if little {
        u32::from_le_bytes([four[0], four[1], four[2], four[3]])
    } else {
        u32::from_be_bytes([four[0], four[1], four[2], four[3]])
    })
}

/// A number of four bytes that can be negative, read the way the block writes them.
fn read_i32(bytes: &[u8], at: usize, little: bool) -> Result<i32, GeorefError> {
    let four = bytes.get(at..at + 4).ok_or(GeorefError::Truncated)?;

    Ok(if little {
        i32::from_le_bytes([four[0], four[1], four[2], four[3]])
    } else {
        i32::from_be_bytes([four[0], four[1], four[2], four[3]])
    })
}

/// The bytes of the EXIF block of a picture.
fn exif_block(bytes: &[u8]) -> Result<&[u8], GeorefError> {
    if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        // The whole file is the block: a TIFF says its first block at the start.
        return Ok(bytes);
    }

    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return Err(GeorefError::NotAPicture);
    }

    let segment = exif_segment(bytes).ok_or(GeorefError::NoExif)?;

    // The six words after the length say that the part is an EXIF one, and the block begins
    // straight after them.
    Ok(&bytes[segment.0 + 10..segment.1])
}

/// The part of a picture that carries the EXIF block, from its marker to the end of it.
fn exif_segment(bytes: &[u8]) -> Option<(usize, usize)> {
    let mut at = 2_usize;

    while at + 4 <= bytes.len() {
        if bytes[at] != 0xFF {
            return None;
        }

        let marker = bytes[at + 1];

        // What comes after the start of the pixels is pixels, and no structure to read.
        if marker == 0xDA {
            return None;
        }

        // The markers of the restart are markers alone: they carry no length.
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            at += 2;
            continue;
        }

        let length = usize::from(read_u16(bytes, at + 2, false).ok()?);
        let end = at + 2 + length;

        if end > bytes.len() {
            return None;
        }

        if marker == 0xE1 && bytes[at + 4..end].starts_with(b"Exif\0\0") {
            return Some((at, end));
        }

        at = end;
    }

    None
}

/// The size of a picture of the JPEG kind, from the marker that says where the pixels begin.
fn jpeg_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return None;
    }

    let mut at = 2_usize;

    while at + 4 <= bytes.len() {
        if bytes[at] != 0xFF {
            return None;
        }

        let marker = bytes[at + 1];

        if marker == 0xDA {
            return None;
        }

        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            at += 2;
            continue;
        }

        let length = usize::from(read_u16(bytes, at + 2, false).ok()?);
        let payload = at + 4;
        let end = at + 2 + length;

        if end > bytes.len() {
            return None;
        }

        // The markers of the start of a frame say the height and then the width.
        if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            let height = read_u16(bytes, payload + 1, false).ok()?;
            let width = read_u16(bytes, payload + 3, false).ok()?;

            return Some((u32::from(width), u32::from(height)));
        }

        at = end;
    }

    None
}

/// Where the block of the position says the picture was taken.
fn read_position(tiff: &Tiff<'_>, entries: &[Entry]) -> Result<Option<GeoPoint>, GeorefError> {
    let Some(lat) = entries.iter().find(|entry| entry.tag == GPS_TAG_LAT) else {
        return Ok(None);
    };

    let Some(lon) = entries.iter().find(|entry| entry.tag == GPS_TAG_LON) else {
        return Ok(None);
    };

    let latitude = dms(
        &tiff.rationals(lat)?,
        sign(tiff, entries, GPS_TAG_LAT_REF, 'S')?,
    );
    let longitude = dms(
        &tiff.rationals(lon)?,
        sign(tiff, entries, GPS_TAG_LON_REF, 'W')?,
    );

    let altitude = match entries.iter().find(|entry| entry.tag == GPS_TAG_ALT) {
        Some(entry) => decimal(
            &tiff.rationals(entry)?,
            sign(tiff, entries, GPS_TAG_ALT_REF, 'B')?,
        ),
        None => 0.0,
    };

    Ok(Some(GeoPoint::with_alt(latitude, longitude, altitude)))
}

/// The sign a reference gives: one when it says what is expected, and the other when it does not.
fn sign(tiff: &Tiff<'_>, entries: &[Entry], tag: u16, opposite: char) -> Result<f64, GeorefError> {
    let entry = match entries.iter().find(|item| item.tag == tag) {
        Some(entry) => entry,
        None => return Ok(1.0),
    };

    if entry.kind == TYPE_SHORT {
        // The height is said with a number: nothing above the sea, and something below it.
        return Ok(if tiff.number(entry)? == 1 { -1.0 } else { 1.0 });
    }

    let text = tiff.text(entry)?;
    let first = text.chars().next().unwrap_or('\0');

    Ok(if first == opposite { -1.0 } else { 1.0 })
}

/// A place written as degrees, minutes and seconds, each of them a fraction.
fn dms(values: &[(i64, i64)], multiplier: f64) -> f64 {
    const WEIGHTS: [f64; 3] = [1.0, 1.0 / 60.0, 1.0 / 3_600.0];

    values
        .iter()
        .enumerate()
        .map(|(index, (top, bottom))| {
            let weight = WEIGHTS.get(index).copied().unwrap_or(0.0);

            if *bottom == 0 {
                0.0
            } else {
                ((*top as f64) / (*bottom as f64)) * weight
            }
        })
        .sum::<f64>()
        * multiplier
}

/// The value of a number that is written as a fraction.
fn decimal(values: &[(i64, i64)], multiplier: f64) -> f64 {
    values
        .iter()
        .map(|(top, bottom)| {
            if *bottom == 0 {
                0.0
            } else {
                (*top as f64) / (*bottom as f64)
            }
        })
        .sum::<f64>()
        * multiplier
}

/// The instant a camera wrote, in seconds since the epoch.
fn parse_time(text: &str) -> Option<i64> {
    // The camera writes the instant as it understands it: four digits, then two, then two,
    // a space, and three more pairs, with the parts separated by a colon or by a dash.
    let parts: Vec<&str> = text.split([':', '-', ' ']).collect();

    if parts.len() != 6 {
        return None;
    }

    let numbers: Result<Vec<i64>, _> = parts
        .iter()
        .map(|part| part.parse::<i64>().map(i64::abs))
        .collect();
    let numbers = numbers.ok()?;

    Some(
        days_from_civil(numbers[0], numbers[1], numbers[2]) * 86_400
            + numbers[3] * 3_600
            + numbers[4] * 60
            + numbers[5],
    )
}

/// The number of days between the epoch and a day of the calendar.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = (if year >= 0 { year } else { year - 399 }) / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era =
        year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + year_of_era / 400 + day_of_year;

    era * 146_097 + day_of_era - 719_468
}

/// The instant of a picture, written the way a camera writes it.
fn format_time(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);

    let (year, month, day) = civil_from_days(days);

    format!(
        "{:04}:{:02}:{:02} {:02}:{:02}:{:02}",
        year,
        month,
        day,
        rest / 3_600,
        (rest % 3_600) / 60,
        rest % 60
    )
}

/// The day of the calendar that the given number of days from the epoch is.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = (if days >= 0 { days } else { days - 146_096 }) / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_461 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year =
        day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100 + year_of_era / 400);
    let position_in_year = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * position_in_year + 2) / 5 + 1;
    let month = if position_in_year < 10 {
        position_in_year + 3
    } else {
        position_in_year - 9
    };

    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// A number written as a fraction, with the denominator that says how big it is.
fn fraction(value: f64) -> (i64, i64) {
    (value.round() as i64, 1)
}

/// The three parts of a degree, as a camera writes them: degrees, minutes and seconds.
fn to_parts(degrees: f64) -> [(i64, i64); 3] {
    let absolute = degrees.abs();
    let whole = absolute.floor();
    let minutes = (absolute - whole) * 60.0;
    let whole_minutes = minutes.floor();
    let seconds = (minutes - whole_minutes) * 60.0;

    [fraction(whole), fraction(whole_minutes), fraction(seconds)]
}

/// The block of EXIF that says where a picture was taken.
#[must_use]
pub fn build_exif(
    width: u32,
    height: u32,
    orientation: Orientation,
    taken_at: Option<i64>,
    position: GeoPoint,
) -> Vec<u8> {
    // The block is written the way a camera usually writes it: with the smallest digit first.
    let counts = (
        3 + usize::from(taken_at.is_some()) + 2,
        usize::from(taken_at.is_some()),
        5 + usize::from(position.alt != 0.0),
    );

    let size_of_block = |count: usize| 2 + count * 12 + 4;

    let root_at = 8_usize;
    let made_at = root_at + size_of_block(counts.0);
    let gps_at = made_at + size_of_block(counts.1);
    let data_at = gps_at + size_of_block(counts.2);

    // The places the values that do not fit inside four bytes live in.
    let mut next = data_at;
    let mut place = |value: &mut Option<usize>| {
        if value.is_some() {
            *value = Some(next);
            next += 20;
        }
    };

    let mut written_at = taken_at.map(|_| 0_usize);
    let mut made_text_at = taken_at.map(|_| 0_usize);
    place(&mut written_at);
    place(&mut made_text_at);

    let lat_at = next;
    next += 24;
    let lon_at = next;
    next += 24;
    let alt_at = if position.alt == 0.0 { 0 } else { next };

    let mut out = Vec::new();
    out.extend_from_slice(b"II");
    out.extend_from_slice(&42_u16.to_le_bytes());
    out.extend_from_slice(&(root_at as u32).to_le_bytes());

    let mut root = vec![
        (TAG_WIDTH, TYPE_LONG, 1, width),
        (TAG_HEIGHT, TYPE_LONG, 1, height),
        (
            TAG_ORIENTATION,
            TYPE_SHORT,
            1,
            u32::from(orientation.code()),
        ),
    ];

    if let Some(at) = written_at {
        root.push((TAG_DATETIME, TYPE_ASCII, 20, at as u32));
    }

    root.push((TAG_EXIF_IFD, TYPE_LONG, 1, made_at as u32));
    root.push((TAG_GPS_IFD, TYPE_LONG, 1, gps_at as u32));

    write_block(&mut out, &root);

    let made = taken_at
        .map(|_| {
            vec![(
                TAG_DATETIME_ORIGINAL,
                TYPE_ASCII,
                20,
                made_text_at.unwrap_or(0) as u32,
            )]
        })
        .unwrap_or_default();
    write_block(&mut out, &made);

    let lat_ref = if position.lat < 0.0 {
        b"S\0\0\0"
    } else {
        b"N\0\0\0"
    };
    let lon_ref = if position.lng < 0.0 {
        b"W\0\0\0"
    } else {
        b"E\0\0\0"
    };

    let mut gps = vec![
        (GPS_TAG_LAT_REF, TYPE_ASCII, 2, u32::from_le_bytes(*lat_ref)),
        (GPS_TAG_LAT, TYPE_RATIONAL, 3, lat_at as u32),
        (GPS_TAG_LON_REF, TYPE_ASCII, 2, u32::from_le_bytes(*lon_ref)),
        (GPS_TAG_LON, TYPE_RATIONAL, 3, lon_at as u32),
    ];

    if position.alt != 0.0 {
        gps.push((GPS_TAG_ALT, TYPE_RATIONAL, 1, alt_at as u32));
    }

    gps.push((GPS_TAG_ALT_REF, TYPE_SHORT, 1, 0));
    write_block(&mut out, &gps);

    // The values that live outside the field of their tag.
    if let (Some(seconds), Some(at)) = (taken_at, written_at) {
        out.resize(at, 0);
        out.extend_from_slice(format_time(seconds).as_bytes());
        out.push(0);
    }

    if let (Some(seconds), Some(at)) = (taken_at, made_text_at) {
        out.resize(at, 0);
        out.extend_from_slice(format_time(seconds).as_bytes());
        out.push(0);
    }

    for part in to_parts(position.lat) {
        out.extend_from_slice(&(part.0 as u32).to_le_bytes());
        out.extend_from_slice(&(part.1 as u32).to_le_bytes());
    }

    for part in to_parts(position.lng) {
        out.extend_from_slice(&(part.0 as u32).to_le_bytes());
        out.extend_from_slice(&(part.1 as u32).to_le_bytes());
    }

    if position.alt != 0.0 {
        out.extend_from_slice(&(position.alt.round() as u32).to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
    }

    out
}

/// One block of tags, with the places its values live in.
fn write_block(out: &mut Vec<u8>, entries: &[(u16, u16, u32, u32)]) {
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());

    for (tag, kind, count, value) in entries {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&value.to_le_bytes());
    }

    out.extend_from_slice(&0u32.to_le_bytes());
}

/// The bytes of a copy of a picture with its georeference written into it.
pub fn geotagged(
    bytes: &[u8],
    taken_at: Option<i64>,
    position: GeoPoint,
) -> Result<Vec<u8>, GeorefError> {
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return Err(GeorefError::NotAJpeg);
    }

    let (width, height) = jpeg_size(bytes).ok_or_else(|| {
        GeorefError::Malformed("the picture does not say how big it is".to_string())
    })?;

    // The way the picture is turned is kept as the camera wrote it.
    let orientation = GeoreferencedImage::parse(bytes)
        .map(|picture| picture.orientation)
        .unwrap_or(Orientation::Normal);

    let block = build_exif(width, height, orientation, taken_at, position);
    let mut out = Vec::new();

    match exif_segment(bytes) {
        Some((start, end)) => {
            out.extend_from_slice(&bytes[..start]);
            write_part(&mut out, &block);
            out.extend_from_slice(&bytes[end..]);
        }
        None => {
            // A picture that says nothing about where it was taken gets the block at the
            // front, which is where a camera puts it.
            out.extend_from_slice(&bytes[..2]);
            write_part(&mut out, &block);
            out.extend_from_slice(&bytes[2..]);
        }
    }

    Ok(out)
}

/// One part of a JPEG, with its marker and its length.
fn write_part(out: &mut Vec<u8>, block: &[u8]) {
    out.push(0xFF);
    out.push(0xE1);

    // The length of a part of a JPEG is written the other way round from the lengths inside
    // the block, which are written the way the block writes its numbers.
    out.extend_from_slice(&((8 + block.len()) as u16).to_be_bytes());
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(block);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One entry of a block, written the way the block writes its numbers.
    fn tag(number: u16, kind: u16, count: u32, raw: [u8; 4], little: bool) -> Vec<u8> {
        let mut bytes = Vec::new();

        if little {
            bytes.extend_from_slice(&number.to_le_bytes());
            bytes.extend_from_slice(&kind.to_le_bytes());
            bytes.extend_from_slice(&count.to_le_bytes());
        } else {
            bytes.extend_from_slice(&number.to_be_bytes());
            bytes.extend_from_slice(&kind.to_be_bytes());
            bytes.extend_from_slice(&count.to_be_bytes());
        }

        bytes.extend_from_slice(&raw);
        bytes
    }

    /// The four bytes of a number, written the way the block writes its numbers.
    fn bytes_of(value: u32, little: bool) -> [u8; 4] {
        if little {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        }
    }

    /// The number of tags in a block, written the way the block writes its numbers.
    fn count_of(count: u16, little: bool) -> [u8; 2] {
        if little {
            count.to_le_bytes()
        } else {
            count.to_be_bytes()
        }
    }

    /// A block that says where a picture was taken, with the places its values live in.
    fn block(little: bool) -> Vec<u8> {
        let root_at = 8_usize;
        let gps_at = root_at + 2 + 4 * 12 + 4;
        let data_at = gps_at + 2 + 6 * 12 + 4;

        let mut out = Vec::new();
        let magic = if little {
            42_u16.to_le_bytes()
        } else {
            42_u16.to_be_bytes()
        };

        out.extend_from_slice(if little { b"II" } else { b"MM" });
        out.extend_from_slice(&magic);
        out.extend_from_slice(&bytes_of(root_at as u32, little));

        out.extend_from_slice(&count_of(4, little));

        // A number of two bytes is written at the start of the field of the tag, so the two
        // empty places are left after it.
        let turned = if little { [6, 0, 0, 0] } else { [0, 6, 0, 0] };

        for (number, kind, count, raw) in [
            (TAG_WIDTH, TYPE_LONG, 1, bytes_of(640, little)),
            (TAG_HEIGHT, TYPE_LONG, 1, bytes_of(480, little)),
            (TAG_ORIENTATION, TYPE_SHORT, 1, turned),
            (TAG_GPS_IFD, TYPE_LONG, 1, bytes_of(gps_at as u32, little)),
        ] {
            out.extend_from_slice(&tag(number, kind, count, raw, little));
        }

        out.extend_from_slice(&bytes_of(0, little));

        out.extend_from_slice(&count_of(6, little));

        for (number, kind, count, raw) in [
            (GPS_TAG_LAT_REF, TYPE_ASCII, 2, [b'N', 0, 0, 0]),
            (
                GPS_TAG_LAT,
                TYPE_RATIONAL,
                3,
                bytes_of(data_at as u32, little),
            ),
            (GPS_TAG_LON_REF, TYPE_ASCII, 2, [b'W', 0, 0, 0]),
            (
                GPS_TAG_LON,
                TYPE_RATIONAL,
                3,
                bytes_of((data_at + 24) as u32, little),
            ),
            (
                GPS_TAG_ALT,
                TYPE_RATIONAL,
                1,
                bytes_of((data_at + 48) as u32, little),
            ),
            (GPS_TAG_ALT_REF, TYPE_SHORT, 1, bytes_of(0, little)),
        ] {
            out.extend_from_slice(&tag(number, kind, count, raw, little));
        }

        out.extend_from_slice(&bytes_of(0, little));

        for value in [40_u32, 26_u32, 30_u32, 73_u32, 58_u32, 0_u32, 1_200_u32] {
            out.extend_from_slice(&bytes_of(value, little));
            out.extend_from_slice(&bytes_of(1, little));
        }

        out
    }

    /// The bytes of a picture that carries the given block.
    fn picture(block: &[u8]) -> Vec<u8> {
        let mut out = vec![0xFF, 0xD8];
        out.push(0xFF);
        out.push(0xE1);
        out.extend_from_slice(&((8 + block.len()) as u16).to_be_bytes());
        out.extend_from_slice(b"Exif\0\0");
        out.extend_from_slice(block);

        // The marker that says where the pixels begin, with the size of the picture.
        out.push(0xFF);
        out.push(0xC0);
        out.extend_from_slice(&11u16.to_be_bytes());
        out.push(8);
        out.extend_from_slice(&480u16.to_be_bytes());
        out.extend_from_slice(&640u16.to_be_bytes());
        out.push(1);
        out.push(0);
        out.push(0);
        out.push(0);

        // Some bytes for the pixels, so that a part that says how long it is can be cut
        // without the file being shorter than what it says.
        out.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x06, 1, 0, 0, 1, 1]);

        out.extend_from_slice(&[0xFF, 0xD9]);
        out
    }

    #[test]
    fn reads_the_place_the_camera_wrote() {
        let picture = picture(&block(true));
        let image = GeoreferencedImage::parse(&picture).expect("the block can be read");

        assert_eq!(image.width, 640);
        assert_eq!(image.height, 480);
        assert_eq!(image.orientation, Orientation::Rotate90);

        let position = image
            .position
            .expect("the block says where the picture was taken");

        assert!(
            (position.lat - 40.441_666_667).abs() < 1e-9,
            "lat {}",
            position.lat
        );
        assert!(
            (position.lng + 73.966_666_667).abs() < 1e-9,
            "lng {}",
            position.lng
        );
        assert!(
            (position.alt - 1_200.0).abs() < 1e-9,
            "alt {}",
            position.alt
        );
    }

    #[test]
    fn reads_a_block_that_writes_its_numbers_the_other_way_round() {
        let picture = picture(&block(false));
        let image = GeoreferencedImage::parse(&picture).expect("the block can be read");

        assert_eq!(image.width, 640);
        assert_eq!(image.orientation, Orientation::Rotate90);

        let position = image
            .position
            .expect("the block says where the picture was taken");

        assert!(
            (position.lat - 40.441_666_667).abs() < 1e-9,
            "lat {}",
            position.lat
        );
    }

    #[test]
    fn the_georeference_that_is_written_comes_back_the_same() {
        let taken_at = parse_time("2026:10:23 11:04:09").expect("the instant can be read");
        let place = GeoPoint::with_alt(-33.870_7, 151.209_3, 92.0);

        let copy =
            geotagged(&picture(&block(true)), Some(taken_at), place).expect("the copy is written");
        let image = GeoreferencedImage::parse(&copy).expect("the copy can be read");

        assert_eq!(image.width, 640);
        assert_eq!(image.height, 480);
        assert_eq!(image.orientation, Orientation::Rotate90);
        assert_eq!(image.taken_at, Some(taken_at));

        let position = image
            .position
            .expect("the copy says where the picture was taken");

        // A second of arc is the smallest thing a camera writes as a whole number, so a place
        // that is written and read back can be a hundredth of a kilometre away from the one it
        // was written from.
        assert!(
            (position.lat - place.lat).abs() < 1e-3,
            "lat {}",
            position.lat
        );
        assert!(
            (position.lng - place.lng).abs() < 1e-3,
            "lng {}",
            position.lng
        );
        assert!(
            (position.alt - place.alt).abs() < 1.0,
            "alt {}",
            position.alt
        );
    }

    #[test]
    fn a_picture_that_carries_no_block_is_not_read_as_one_that_does() {
        // A JPEG with only the marker that says how big it is: no block to read.
        let bytes = [
            0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x0B, 8, 0x01, 0x2C, 0x02, 0x80, 1, 0, 0, 0xFF, 0xD9,
        ];

        assert_eq!(jpeg_size(&bytes), Some((640, 300)));
        assert_eq!(GeoreferencedImage::parse(&bytes), Err(GeorefError::NoExif));
    }

    #[test]
    fn a_picture_that_is_cut_short_is_not_read_as_a_whole_one() {
        // A file that is a block by itself, cut before the place its tags point to.
        let cut = &block(true)[..120];

        assert_eq!(GeoreferencedImage::parse(cut), Err(GeorefError::Truncated));
    }

    #[test]
    fn the_instant_a_camera_writes_is_the_one_the_ground_station_reads() {
        assert_eq!(parse_time("1970:01:01 00:00:00"), Some(0));
        assert_eq!(parse_time("2000-01-01 00:00:00"), Some(946_684_800));
        assert_eq!(parse_time("2026:10:23 11:04:09"), Some(1_792_753_449));
        assert_eq!(format_time(1_792_753_449), "2026:10:23 11:04:09");
        assert_eq!(parse_time("not an instant"), None);
    }
}
