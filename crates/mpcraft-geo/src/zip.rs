//! The zip that the terrain comes in.
//!
//! The service serves each tile of the terrain compressed, in a zip with a single
//! entry: the tile of elevations itself. The zip is a container, and this is a reader
//! for the one shape of it that the terrain uses: a single entry, stored or deflated.
//!
//! The reading of the container is here, and the inflation is `flate2`'s, which the
//! workspace already compiles.

use std::io::Read;

use flate2::read::DeflateDecoder;

use crate::terrain::TerrainError;

/// The signature of a local file header.
const LOCAL_HEADER: u32 = 0x0403_4b50;

/// The signature of the central directory of a file.
const CENTRAL_HEADER: u32 = 0x0201_4b50;

/// The signature of the end of the central directory.
const END_OF_DIRECTORY: u32 = 0x0605_4b50;

/// The compression method of an entry that is stored as it is.
const STORED: u16 = 0;

/// The compression method of an entry that is deflated.
const DEFLATED: u16 = 8;

/// The bytes of the four numbers of an offset.
fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

/// The bytes of the two numbers of an offset.
fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

/// One entry of the central directory.
struct Entry {
    /// The compression method.
    method: u16,
    /// The size of the entry as it is stored.
    compressed: usize,
    /// The size of the entry once it is decompressed.
    uncompressed: usize,
    /// The offset of the local header of the entry.
    offset: usize,
}

/// Reads the single entry of a zip.
///
/// The entry whose name ends with `.hgt` is the one that is wanted: it is the tile of
/// elevations, and the zip may hold other things beside it.
///
/// # Errors
///
/// Returns an error when the bytes are not a zip, when the zip holds no tile, or when
/// the tile cannot be decompressed.
pub fn read_tile(bytes: &[u8]) -> Result<Vec<u8>, TerrainError> {
    let entry = find_tile(bytes)?;

    // The local header holds the name and the extra fields again, and the data follows
    // them.
    let header = bytes.get(entry.offset..).ok_or_else(|| {
        TerrainError::Malformed("the local header of the entry is outside the file".to_owned())
    })?;

    if u32_at(header, 0) != Some(LOCAL_HEADER) {
        return Err(TerrainError::Malformed(
            "the local header of the entry is not one".to_owned(),
        ));
    }

    let name_length = u16_at(header, 26).unwrap_or(0) as usize;
    let extra_length = u16_at(header, 28).unwrap_or(0) as usize;

    let start = entry.offset + 30 + name_length + extra_length;
    let compressed = bytes
        .get(start..start + entry.compressed.min(bytes.len().saturating_sub(start)))
        .unwrap_or(&[]);

    match entry.method {
        STORED => Ok(compressed.to_vec()),
        DEFLATED => {
            let mut out = Vec::with_capacity(entry.uncompressed);

            DeflateDecoder::new(compressed)
                .read_to_end(&mut out)
                .map_err(|error| {
                    TerrainError::Malformed(format!("the tile could not be unpacked: {error}"))
                })?;

            Ok(out)
        }
        other => Err(TerrainError::Malformed(format!(
            "the tile is compressed in a way nobody reads: {other}"
        ))),
    }
}

/// Finds the entry of the tile of elevations.
fn find_tile(bytes: &[u8]) -> Result<Entry, TerrainError> {
    // The end of the central directory is at the end of the file, unless the file has a
    // comment.
    let mut end = bytes.len();

    while end >= 22 {
        end -= 1;

        if u32_at(bytes, end) == Some(END_OF_DIRECTORY) {
            break;
        }

        if end == 0 {
            break;
        }
    }

    if u32_at(bytes, end) != Some(END_OF_DIRECTORY) {
        return Err(TerrainError::Malformed(
            "the file is not a zip: it has no directory".to_owned(),
        ));
    }

    let count = u16_at(bytes, end + 10).unwrap_or(0) as usize;
    let directory = u32_at(bytes, end + 16).unwrap_or(0) as usize;

    let mut offset = directory;

    for _ in 0..count {
        if u32_at(bytes, offset) != Some(CENTRAL_HEADER) {
            return Err(TerrainError::Malformed(
                "the directory of the zip is broken".to_owned(),
            ));
        }

        let method = u16_at(bytes, offset + 10).unwrap_or(0);
        let compressed = u32_at(bytes, offset + 20).unwrap_or(0) as usize;
        let uncompressed = u32_at(bytes, offset + 24).unwrap_or(0) as usize;
        let name_length = u16_at(bytes, offset + 28).unwrap_or(0) as usize;
        let extra_length = u16_at(bytes, offset + 30).unwrap_or(0) as usize;
        let comment_length = u16_at(bytes, offset + 32).unwrap_or(0) as usize;
        let local = u32_at(bytes, offset + 42).unwrap_or(0) as usize;

        let name = bytes
            .get(offset + 46..offset + 46 + name_length)
            .map(|raw| String::from_utf8_lossy(raw).into_owned())
            .unwrap_or_default();

        if name.ends_with(".hgt") {
            return Ok(Entry {
                method,
                compressed,
                uncompressed,
                offset: local,
            });
        }

        offset += 46 + name_length + extra_length + comment_length;
    }

    Err(TerrainError::Malformed(
        "the zip holds no tile of elevations".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::DeflateEncoder;
    use flate2::Compression;
    use std::io::Write;

    /// Builds a zip with one entry, the way the service builds them.
    fn zip(name: &str, content: &[u8], deflate: bool) -> Vec<u8> {
        let (method, data) = if deflate {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(content).expect("se comprime");
            (DEFLATED, encoder.finish().expect("se termina"))
        } else {
            (STORED, content.to_vec())
        };

        let crc = crc32(content);

        let mut out = Vec::new();

        // The local header: the signature, the version, the flags, the method, the
        // time, the date, the checksum, the two sizes, the name and the extra.
        out.extend_from_slice(&LOCAL_HEADER.to_le_bytes());
        out.extend_from_slice(&20_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(content.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&data);

        let local_offset = 0;
        let directory_offset = out.len();

        // The central directory: the signature, the two versions, the flags, the
        // method, the time, the date, the checksum, the two sizes, the three
        // lengths, the disk, the attributes, the offset and the name.
        out.extend_from_slice(&CENTRAL_HEADER.to_le_bytes());
        out.extend_from_slice(&20_u16.to_le_bytes());
        out.extend_from_slice(&20_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(content.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(&(local_offset as u32).to_le_bytes());
        out.extend_from_slice(name.as_bytes());

        let directory_size = out.len() - directory_offset;

        // The end of the directory.
        out.extend_from_slice(&END_OF_DIRECTORY.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&(directory_size as u32).to_le_bytes());
        out.extend_from_slice(&(directory_offset as u32).to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());

        out
    }

    /// The checksum of a zip: the one the format uses, which is the usual one with the
    /// bits turned round.
    fn crc32(content: &[u8]) -> u32 {
        let mut crc = 0xffff_ffff_u32;

        for byte in content {
            crc ^= u32::from(*byte);

            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xedb8_8320 & mask);
            }
        }

        !crc
    }

    #[test]
    fn a_stored_tile_is_read() {
        let content: Vec<u8> = (0..64_u16).flat_map(|v| v.to_be_bytes()).collect();
        let file = zip("S36E149.hgt", &content, false);

        let read = read_tile(&file).expect("la tesela se lee");

        assert_eq!(read, content);
    }

    #[test]
    fn a_deflated_tile_is_read() {
        let content: Vec<u8> = (0..64_u16).flat_map(|v| v.to_be_bytes()).collect();
        let file = zip("S36E149.hgt", &content, true);

        let read = read_tile(&file).expect("la tesela se lee");

        assert_eq!(read, content);
    }

    #[test]
    fn a_zip_without_a_tile_is_refused() {
        let file = zip("other.txt", b"nothing", false);

        let error = read_tile(&file).expect_err("no hay tesela");

        assert!(matches!(error, TerrainError::Malformed(_)));
    }

    #[test]
    fn something_that_is_not_a_zip_is_refused() {
        let error = read_tile(b"not a zip at all").expect_err("no es un zip");

        assert!(matches!(error, TerrainError::Malformed(_)));
    }
}
