//! The file of the firmware that is written to a board.
//!
//! The manifest points at an `.apj`, and an `.apj` is not a stream of bytes for the flash. It is a
//! document: it says which board the image is for, how big the image is, and carries the image
//! compressed and written in base64. Writing the document as it stands would put its text into the
//! flash of the board, so the document is read here and what travels to the board is the image.
//!
//! Two numbers are kept apart because they are not the same thing: the length the document says for
//! the image, and the bytes as they go to the flash, which are padded to a multiple of four because
//! the protocol counts a length in a byte and the board wants blocks of four.
//!
//! The check that a board can be asked for after writing — the CRC over what its flash holds — is
//! worked out with the rule the published implementation gives: the image as it was written, then
//! the erased part of the flash read as fours of `0xFF` up to the size the board says. That is why
//! the number depends on the board and not only on the file.

use serde::Deserialize;

use crate::error::{CommandError, CommandResult};

/// The file of the firmware, as the document says it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FirmwareImage {
    /// The board the image is for, as the document names it.
    pub board_id: u32,
    /// How big the image is, as the document says.
    pub image_size: usize,
    /// The image as it goes to the flash: unpacked and padded to a multiple of four.
    pub image: Vec<u8>,
}

impl FirmwareImage {
    /// Reads the document that is already in memory.
    ///
    /// # Errors
    ///
    /// Returns an error when the bytes are not the document — which is what happens when a file that
    /// is a raw image is handed to a path that speaks the document — or when the image inside it
    /// cannot be unpacked.
    pub fn read(bytes: &[u8]) -> CommandResult<Self> {
        let document: Document = serde_json::from_slice(bytes).map_err(|error| {
            CommandError::invalid_request(format!(
                "the file is not the document the manifest points at: {error}"
            ))
        })?;

        let packed = base64_bytes(&document.image).map_err(|error| {
            CommandError::invalid_request(format!(
                "the image inside the document is not base64: {error}"
            ))
        })?;

        let image = unpack(&packed).map_err(|error| {
            CommandError::invalid_request(format!(
                "the image inside the document could not be unpacked: {error}"
            ))
        })?;

        if image.len() != document.image_size {
            return Err(CommandError::invalid_request(format!(
                "the document says {} bytes and the image inside it holds {}",
                document.image_size,
                image.len()
            )));
        }

        Ok(Self {
            board_id: document.board_id,
            image_size: document.image_size,
            image: padded(&image),
        })
    }

    /// The CRC the board should give for its flash after this image is written to it.
    ///
    /// `flash_size` is the number the board says for itself, which is what the rule pads to: the
    /// part of the flash that is not written is read as erased, and erased reads as fours of `0xFF`.
    #[must_use]
    pub fn expected_crc(&self, flash_size: u32) -> u32 {
        let mut state = crc32(&self.image, 0);
        let mut at = self.image.len();

        // The published rule counts fours while the place is before the size the board says, so a block
        // that runs past the end of the flash is still counted — which is why the number depends on the
        // board and not only on the file.
        while at < (flash_size as usize).saturating_sub(1) {
            state = crc32(&[0xFF, 0xFF, 0xFF, 0xFF], state);
            at += 4;
        }

        state
    }
}

/// The document, with the parts this path uses.
#[derive(Debug, Deserialize)]
struct Document {
    /// The board the image is for.
    #[serde(default)]
    board_id: u32,
    /// How big the image is, as the document says.
    #[serde(default)]
    image_size: usize,
    /// The image, compressed and written in base64.
    #[serde(default)]
    image: String,
}

/// Pads an image to a multiple of four, which is what the blocks of the protocol need.
fn padded(image: &[u8]) -> Vec<u8> {
    let remainder = image.len() % 4;

    if remainder == 0 {
        return image.to_vec();
    }

    let mut out = image.to_vec();
    out.resize(image.len() + (4 - remainder), 0);
    out
}

/// Decodes a base64 document.
fn base64_bytes(text: &str) -> Result<Vec<u8>, String> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = Vec::new();
    let mut group = 0_u32;
    let mut bits = 0_u32;

    for character in text.bytes() {
        if character == b'=' {
            break;
        }

        if character == b' ' || (character as char).is_whitespace() {
            continue;
        }

        let value = ALPHABET
            .iter()
            .position(|known| *known == character)
            .ok_or_else(|| format!("the letter {character:o} is not one of the alphabet"))?;

        group = (group << 6) | value as u32;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            out.push(((group >> bits) & 0xFF) as u8);
        }
    }

    Ok(out)
}

/// Unpacks the image, which is published compressed.
fn unpack(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Read as _;

    let mut out = Vec::new();

    flate2::read::ZlibDecoder::new(bytes)
        .read_to_end(&mut out)
        .map_err(|error| error.to_string())?;

    Ok(out)
}

/// The CRC the protocol of the bootloader counts with: the reflected CRC32, one byte at a time.
fn crc32(bytes: &[u8], start: u32) -> u32 {
    let mut state = start;

    for byte in bytes {
        let index = ((state ^ *byte as u32) & 0xFF) as usize;
        state = CRC_TABLE[index] ^ (state >> 8);
    }

    state
}

/// The table of the reflected CRC32, generated from the rule rather than copied.
static CRC_TABLE: [u32; 256] = build_table();

const fn build_table() -> [u32; 256] {
    let mut table = [0_u32; 256];
    let mut index = 0_usize;

    while index < 256 {
        let mut value = index as u32;
        let mut bit = 0;

        while bit < 8 {
            value = if value & 1 == 1 {
                0xEDB8_8320 ^ (value >> 1)
            } else {
                value >> 1
            };
            bit += 1;
        }

        table[index] = value;
        index += 1;
    }

    table
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture was written by a reader that is not this one — python's own base64 and zlib — so
    /// what it says about itself is a second implementation rather than this one agreeing with itself.
    const FIXTURE: &str = include_str!("../tests/fixtures/sample_apj.json");

    #[test]
    fn the_document_is_read_as_a_document_and_not_as_bytes_for_the_flash() {
        // A path that wrote the file as it is on disk would send the text of the document to the
        // board, which is bytes that no flash expects. The image inside is what travels.
        let file = FirmwareImage::read(FIXTURE.as_bytes()).expect("the fixture is read");

        assert_eq!(file.board_id, 9);
        assert_eq!(file.image_size, 305);
        assert_eq!(
            file.image.len(),
            308,
            "the image is padded to a multiple of four"
        );
        assert!(!String::from_utf8_lossy(&file.image).contains("board_id"));

        // The pattern the fixture was built from, written down before the function was called.
        assert_eq!(file.image[..256], (0_u8..=255).collect::<Vec<u8>>()[..]);
        assert_eq!(&file.image[256..305], b"MPCraft".repeat(7).as_slice());
        assert_eq!(
            &file.image[305..],
            &[0, 0, 0],
            "the padding is what the image does not fill"
        );
    }

    #[test]
    fn the_crc_is_the_number_the_published_rule_gives() {
        // The board counts the CRC over its flashable area word by word, starting at zero and without
        // the starting mask or the final inversion that the ordinary CRC32 has: that is what
        // `crc32_small(sum, word, 4)` in `Tools/AP_Bootloader/bl_protocol.cpp` does, and it is why the
        // number for the same bytes is not the number a zlib reader gives for them.
        //
        // 0x4b98510b is what an independent implementation of that rule — written from the published
        // polynomial, whose table was then compared entry by entry with the table the reference clone
        // carries — gives for this image inside a flash of 1000 bytes: the image as it was written,
        // then fours of `0xFF` from its end up to the size the board says.
        let file = FirmwareImage::read(FIXTURE.as_bytes()).expect("the fixture is read");

        assert_eq!(file.expected_crc(1000), 0x4b98_510b);

        // With no room after the image there is nothing erased to count, so the number is the CRC of the
        // padded image alone — the same independent implementation gives 0xa1848b49 for it.
        assert_eq!(file.expected_crc(308), 0xa184_8b49);
    }

    #[test]
    fn the_table_is_the_one_the_rule_works_out_to() {
        // A check value worked out by the independent implementation rather than copied: the raw rule
        // gives 0x2dfd2d88 for "123456789", which is not the standard CRC32 check value 0xcbf43926
        // because the standard one masks the starting state and inverts the result.
        assert_eq!(crc32(b"123456789", 0), 0x2dfd_2d88);
    }

    #[test]
    fn a_file_that_is_not_the_document_is_refused_before_anything_is_written() {
        // A raw image has no document to read, and a path that took its bytes would write them to a
        // board that expects the shape of the document. The refusal happens before any wire is touched.
        let error =
            FirmwareImage::read(b"\x00\x01\x02\x03").expect_err("the bytes are not a document");
        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);

        let say =
            FirmwareImage::read(br#"{"board_id": 9, "image_size": 4, "image": "not base64!"}"#)
                .expect_err("the image is not base64");
        assert_eq!(say.code, CommandError::CODE_INVALID_REQUEST);
    }

    #[test]
    fn an_image_that_is_not_the_length_the_document_says_is_said_as_a_gap() {
        // The document says nine bytes and the image inside it holds three. The compressed text was
        // written by an independent reader, so the length is not this one agreeing with itself.
        let error = FirmwareImage::read(
            br#"{"board_id": 9, "image_size": 9, "image": "eJzzDXAGAAHNAOE="}"#,
        )
        .expect_err("the document says a length the image does not have");

        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);
        assert!(error.message.contains("9"));
        assert!(error.message.contains("3"));
    }
}
