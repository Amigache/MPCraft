//! The two checksums DroneCAN is built on.
//!
//! There are two, and they answer different questions, which is why they are kept apart:
//!
//! * **CRC-16/CCITT-FALSE** guards one multi-frame *transfer*: the sender puts the checksum of the
//!   payload in the first frame and the receiver recomputes it over the frames it collects. A single
//!   frame carries no checksum of its own, because the CAN bus already checks it.
//! * **CRC-64-WE** identifies a *data type*: it is applied to the normalized definition of the type,
//!   and two nodes may exchange a message only if their signatures agree. It is not used on the wire
//!   for single frames, and only as the seed of the transfer checksum for multi-frame ones.
//!
//! Both are written from their published parameters rather than copied from a table, and each is
//! checked against the value its catalogue publishes for the ASCII string `123456789`.

/// The CRC-16/CCITT-FALSE of a payload: polynomial `0x1021`, initial value `0xFFFF`, no final
/// inversion.
///
/// # Examples
///
/// ```
/// use mpcraft_dronecan::crc::crc16;
///
/// assert_eq!(crc16(b"123456789"), 0x29B1);
/// ```
#[must_use]
pub fn crc16(bytes: &[u8]) -> u16 {
    crc16_from(0xFFFF, bytes)
}

/// The same checksum, continued from a value that was already computed.
#[must_use]
pub fn crc16_from(seed: u16, bytes: &[u8]) -> u16 {
    let mut crc = seed;

    for byte in bytes {
        crc ^= u16::from(*byte) << 8;

        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }

    crc
}

/// The CRC-64-WE of a normalized definition: polynomial `0x42F0E1EBA9EA3693`, initial value and
/// final inversion both `0xFFFF_FFFF_FFFF_FFFF`.
///
/// # Examples
///
/// ```
/// use mpcraft_dronecan::crc::crc64_we;
///
/// // The check value the CRC catalogue publishes for this polynomial.
/// assert_eq!(crc64_we(b"123456789"), 0x62EC_59E3_F1A4_F00A);
/// ```
#[must_use]
pub fn crc64_we(bytes: &[u8]) -> u64 {
    let mut crc = u64::MAX;

    add64(&mut crc, bytes);

    crc ^ u64::MAX
}

/// The same signature hash, continued from a value that was already computed.
///
/// The signature of a type that nests another is the hash of the first extended with the second, so
/// a value has to be able to start from another: that is what this does, and it is why the seed is
/// inverted on the way in, as the specification's own implementation does.
#[must_use]
pub fn crc64_we_from(previous: u64, bytes: &[u8]) -> u64 {
    let mut crc = previous ^ u64::MAX;

    add64(&mut crc, bytes);

    crc ^ u64::MAX
}

/// Feeds bytes into the running signature, without the final inversion.
fn add64(crc: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *crc ^= u64::from(*byte) << 56;

        for _ in 0..8 {
            if *crc & (1 << 63) != 0 {
                *crc = (*crc << 1) ^ 0x42F0_E1EB_A9EA_3693;
            } else {
                *crc <<= 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_transfer_checksum_is_the_one_its_catalogue_publishes() {
        // The check value of CRC-16/CCITT-FALSE, which is what DroneCAN uses for a transfer.
        assert_eq!(crc16(b"123456789"), 0x29B1);
        assert_eq!(crc16(b""), 0xFFFF);
    }

    #[test]
    fn the_signature_hash_is_the_one_its_catalogue_publishes() {
        // The check value of CRC-64-WE, which is what a data type signature is built on.
        assert_eq!(crc64_we(b"123456789"), 0x62EC_59E3_F1A4_F00A);

        // Nothing fed in leaves the initial value inverted, which is the specification's own
        // implementation read back out: the value it starts from is all ones, and the final
        // inversion turns that into nothing.
        assert_eq!(crc64_we(b""), 0);
    }

    #[test]
    fn a_checksum_that_is_continued_is_the_checksum_of_both_parts() {
        assert_eq!(crc16_from(crc16(b"123"), b"456789"), crc16(b"123456789"));
        assert_eq!(
            crc64_we_from(crc64_we(b"123"), b"456789"),
            crc64_we(b"123456789")
        );
    }
}
