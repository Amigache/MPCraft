//! The bit stream a DroneCAN payload is written into.
//!
//! Two rules, and both matter for a node to read what the station said:
//!
//! * a multi-byte number is **little-endian**: its least significant byte comes first;
//! * inside a byte the bits are filled from the **most significant** one down.
//!
//! A number whose width is not a whole number of bytes therefore does not sit at the bottom of the
//! bytes it spans: the first `n % 8` bits of it end up at the top of the last byte it touches, which
//! is what `libcanard` writes and what every node on a bus reads. This is written to match that
//! implementation rather than to look tidy.

/// Writes fields into a payload.
#[derive(Debug, Default)]
pub struct BitWriter {
    bytes: Vec<u8>,
    bits: usize,
}

impl BitWriter {
    /// An empty payload.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many bits have been written.
    #[must_use]
    pub const fn bits(&self) -> usize {
        self.bits
    }

    /// The bytes, with the last one padded with zeros.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    /// Pushes one bit.
    fn bit(&mut self, set: bool) {
        let index = self.bits / 8;
        let within = self.bits % 8;

        if index == self.bytes.len() {
            self.bytes.push(0);
        }

        if set {
            self.bytes[index] |= 1 << (7 - within);
        }

        self.bits += 1;
    }

    /// Pushes `count` bits, most significant first, from a slice whose bytes are already in wire
    /// order.
    pub fn bits_from(&mut self, bytes: &[u8], count: usize) {
        for position in 0..count {
            let byte = bytes[position / 8];
            let within = position % 8;

            self.bit(byte & (1 << (7 - within)) != 0);
        }
    }

    /// Writes an unsigned number of `width` bits.
    ///
    /// The value goes in little-endian, and the part of the last byte that carries no bits is
    /// shifted to the bottom of it, which is the layout `libcanard` reads back.
    pub fn uint(&mut self, value: u64, width: usize) {
        debug_assert!(width <= 64);
        debug_assert!(width == 64 || value < (1_u64 << width));

        let whole = width / 8;
        let partial = width % 8;
        let mut bytes = value.to_le_bytes().to_vec();

        if partial != 0 {
            bytes[whole] <<= 8 - partial;
        }

        self.bits_from(&bytes, width);
    }

    /// Writes a signed number of `width` bits, two's complement.
    pub fn int(&mut self, value: i64, width: usize) {
        let masked = if width == 64 {
            value as u64
        } else {
            (value as u64) & ((1_u64 << width) - 1)
        };

        self.uint(masked, width);
    }

    /// Writes `count` zero bits, which is what a `void` field of the definition is.
    pub fn padding(&mut self, count: usize) {
        for _ in 0..count {
            self.bit(false);
        }
    }

    /// Writes bytes as an array of eight-bit items.
    pub fn bytes(&mut self, value: &[u8]) {
        for byte in value {
            self.uint(u64::from(*byte), 8);
        }
    }

    /// Writes the length prefix of a variable-length array of at most `max` items.
    ///
    /// The width is what the largest count needs: an array of up to 128 items is prefixed by the
    /// eight bits a count in 0..=128 needs, and an array of up to 255 by the eight bits 255 needs.
    pub fn length(&mut self, count: usize, max: usize) {
        self.uint(count as u64, bits_for(max));
    }
}

/// Reads fields back out of a payload.
#[derive(Debug)]
pub struct BitReader<'a> {
    bytes: &'a [u8],
    bits: usize,
}

impl<'a> BitReader<'a> {
    /// A reader over a payload.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, bits: 0 }
    }

    /// How many bits have been read.
    #[must_use]
    pub const fn bits(&self) -> usize {
        self.bits
    }

    /// Whether every bit of the payload has been read.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.bits >= self.bytes.len() * 8
    }

    /// One bit.
    fn bit(&mut self) -> Option<bool> {
        let index = self.bits / 8;
        let within = self.bits % 8;
        let byte = *self.bytes.get(index)?;

        self.bits += 1;

        Some(byte & (1 << (7 - within)) != 0)
    }

    /// Reads `width` bits into wire order.
    fn bits_into(&mut self, width: usize) -> Option<Vec<u8>> {
        let mut bytes = vec![0_u8; width.div_ceil(8)];

        for position in 0..width {
            if self.bit()? {
                bytes[position / 8] |= 1 << (7 - (position % 8));
            }
        }

        Some(bytes)
    }

    /// Reads an unsigned number of `width` bits, reversing what [`BitWriter::uint`] did.
    pub fn uint(&mut self, width: usize) -> Option<u64> {
        debug_assert!(width <= 64);

        let whole = width / 8;
        let partial = width % 8;
        let mut bytes = self.bits_into(width)?;

        if partial != 0 {
            bytes[whole] >>= 8 - partial;
        }

        let mut value: u64 = 0;

        for (index, byte) in bytes
            .iter()
            .enumerate()
            .take(whole + usize::from(partial != 0))
        {
            value |= u64::from(*byte) << (8 * index);
        }

        Some(value)
    }

    /// Reads a signed number of `width` bits.
    pub fn int(&mut self, width: usize) -> Option<i64> {
        let raw = self.uint(width)?;

        if width == 64 {
            return Some(raw as i64);
        }

        let sign = 1_u64 << (width - 1);

        Some(if raw & sign != 0 {
            (raw as i64) - (1_i64 << width)
        } else {
            raw as i64
        })
    }

    /// Reads `count` bytes as an array of eight-bit items.
    pub fn bytes(&mut self, count: usize) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(count);

        for _ in 0..count {
            out.push(self.uint(8)? as u8);
        }

        Some(out)
    }

    /// Reads the length prefix of a variable-length array of at most `max` items.
    pub fn length(&mut self, max: usize) -> Option<usize> {
        let width = bits_for(max);
        let count = self.uint(width)? as usize;

        // A count larger than the type allows is a payload that is not this type, and is refused
        // rather than used to allocate whatever the bytes say.
        (count <= max).then_some(count)
    }

    /// Skips padding bits.
    pub fn skip(&mut self, bits: usize) -> Option<()> {
        for _ in 0..bits {
            self.bit()?;
        }

        Some(())
    }
}

/// The number of bits a count in `0..=max` needs.
#[must_use]
pub const fn bits_for(max: usize) -> usize {
    usize::BITS as usize - max.leading_zeros() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_byte_aligned_number_is_little_endian() {
        let mut writer = BitWriter::new();
        writer.uint(0x1234, 16);

        assert_eq!(writer.finish(), vec![0x34, 0x12]);
    }

    #[test]
    fn a_thirteen_bit_number_keeps_its_high_bits_at_the_top_of_the_last_byte() {
        // The layout libcanard reads: the low byte occupies the first byte, and the remaining five
        // bits sit at the top of the second one.
        let mut writer = BitWriter::new();
        writer.uint(0x1FFF, 13);

        assert_eq!(writer.finish(), vec![0xFF, 0xF8]);

        let mut reader = BitReader::new(&[0xFF, 0xF8]);
        assert_eq!(reader.uint(13), Some(0x1FFF));
    }

    #[test]
    fn the_fields_of_a_status_pack_into_seven_bytes() {
        // uptime 0x01020304, health 2, mode 3, sub_mode 7, vendor code 0xBEEF.
        let mut writer = BitWriter::new();
        writer.uint(0x0102_0304, 32);
        writer.uint(2, 2);
        writer.uint(3, 3);
        writer.uint(7, 3);
        writer.uint(0xBEEF, 16);

        let payload = writer.finish();
        assert_eq!(payload.len(), 7);
        assert_eq!(&payload[..4], &[0x04, 0x03, 0x02, 0x01]);

        let mut reader = BitReader::new(&payload);
        assert_eq!(reader.uint(32), Some(0x0102_0304));
        assert_eq!(reader.uint(2), Some(2));
        assert_eq!(reader.uint(3), Some(3));
        assert_eq!(reader.uint(3), Some(7));
        assert_eq!(reader.uint(16), Some(0xBEEF));
    }

    #[test]
    fn a_signed_number_reads_back_as_it_went_in() {
        for value in [-1_i64, -2048, -1, 0, 1, 2047] {
            let mut writer = BitWriter::new();
            writer.int(value, 12);

            let payload = writer.finish();
            let mut reader = BitReader::new(&payload);

            assert_eq!(reader.int(12), Some(value), "{value} of twelve bits");
        }
    }

    #[test]
    fn the_length_of_an_array_takes_the_bits_its_maximum_needs() {
        // The width is what the largest count needs, which is what a node reads back.
        assert_eq!(bits_for(128), 8); // a string of the parameter value: up to 128 bytes
        assert_eq!(bits_for(255), 8); // a certificate: up to 255 bytes
        assert_eq!(bits_for(92), 7); // the name of a parameter
        assert_eq!(bits_for(80), 7); // the name of a node
    }

    #[test]
    fn bytes_written_as_an_array_come_back_as_they_were() {
        let mut writer = BitWriter::new();
        writer.uint(1, 3);
        writer.bytes(b"hi");
        writer.uint(0b101, 3);

        let payload = writer.finish();
        let mut reader = BitReader::new(&payload);

        assert_eq!(reader.uint(3), Some(1));
        assert_eq!(reader.bytes(2), Some(b"hi".to_vec()));
        assert_eq!(reader.uint(3), Some(0b101));

        // Twenty-two bits were written, and the last two of the third byte are padding.
        assert_eq!(reader.bits(), 22);
    }
}
