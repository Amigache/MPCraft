//! Devices that answer requests rather than moving a stream of bytes.
//!
//! A [`crate::Transport`] is a byte stream with a cursor: there is something to read whether or not
//! anything was asked. A device on a USB bus is not that. Nothing comes back until a request goes out,
//! and what comes back is the answer of one request rather than a run of bytes, so this layer speaks in
//! requests: the number, its value, the payload going out, and the length asked for back.
//!
//! What it knows is the shape of a request and the shape of the functional descriptor the interface
//! reports about itself. It does not know what a state is, what a block number means, or what a CRC is:
//! those are rules of the protocol and live above, in `mpcraft-app`, which is where the DFU rules are.
//!
//! The trait carries no operating-system types, which is what keeps it testable: the scripted peer in
//! `crates/mpcraft-app/tests/dfu.rs` answers it without a bus. The binding that reaches a real device
//! lives behind `hardware-tests`, because a device is not the plugin sandbox and the sandbox is the one
//! place this project carries a runtime dependency.

use std::fmt;

use async_trait::async_trait;

/// The result of a request to a device.
pub type DeviceResult<T> = Result<T, DeviceError>;

/// A failure at the device layer.
///
/// The code is stable, as the codes of the link are, because the interface branches on it and never
/// reads a message for meaning.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeviceError {
    /// Nothing on the bus answers to that address.
    #[error("no device on the bus: {0}")]
    Unreachable(String),
    /// The interface refused one request with a stall.
    #[error("the interface refused request 0x{number:02x} with a stall")]
    Stalled {
        /// The number of the request that was refused.
        number: u8,
    },
    /// The answer is not the shape the request asks for.
    #[error("the answer to request 0x{number:02x} is not the shape it asks for: {reason}")]
    BadAnswer {
        /// The number of the request whose answer is wrong.
        number: u8,
        /// What is wrong with the answer.
        reason: String,
    },
    /// The device is on the bus and answers to that address, but this program cannot use it: the interface
    /// is bound to another driver, or its handle is held by another process. That is a different fact from
    /// "nothing is there", and the operator does a different thing with each one — one closes the program
    /// that has the board, the other puts power to a board that is not answering.
    #[error("the device at {address} is there but is not ours: {reason}")]
    NotOurs {
        /// What the bus says about the device.
        address: String,
        /// Why it cannot be used, said as the reading found it.
        reason: String,
    },
}

impl DeviceError {
    /// The stable code the frontend branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Unreachable(_) => "device_unreachable",
            Self::Stalled { .. } => "device_stalled",
            Self::BadAnswer { .. } => "device_bad_answer",
            Self::NotOurs { .. } => "device_not_ours",
        }
    }
}

/// The functional descriptor of a DFU interface, as the interface writes it.
///
/// Nine bytes: the length and the type, the attributes, the detach timeout, the transfer size, and the
/// version. The numbers here are what a published implementation puts on the wire for an STM32 in DFU
/// mode — `struct.pack("<BBBHHH", 9, 0x21, 0x0B, 255, 2048, 0x011A)` — which is what the test below
/// reads, so the fields are not a guess about the order they are read in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionalDescriptor {
    /// What the interface says it can do.
    pub attributes: u8,
    /// How long the device is given to detach before the host gives up, in milliseconds.
    pub detach_timeout: u16,
    /// How many bytes one request can carry.
    pub transfer_size: u16,
    /// The version of the protocol the interface speaks.
    pub version: u16,
}

/// The descriptor type the interface uses for it.
pub const FUNCTIONAL_DESCRIPTOR_TYPE: u8 = 0x21;

impl FunctionalDescriptor {
    /// Reads the descriptor as the interface wrote it.
    ///
    /// # Errors
    ///
    /// Returns an error when the bytes are not nine, or when they are not the descriptor type the
    /// interface says they are. A shape that is not the one asked for is not read as a descriptor with
    /// some fields missing.
    pub fn read(bytes: &[u8]) -> DeviceResult<Self> {
        if bytes.len() != 9 {
            return Err(DeviceError::BadAnswer {
                number: 0x06,
                reason: format!("the descriptor is {} bytes rather than 9", bytes.len()),
            });
        }

        if bytes[1] != FUNCTIONAL_DESCRIPTOR_TYPE {
            let type_byte = bytes[1];
            return Err(DeviceError::BadAnswer {
                number: 0x06,
                reason: format!("the descriptor type is 0x{type_byte:02x}, which is not 0x21"),
            });
        }

        Ok(Self {
            attributes: bytes[2],
            detach_timeout: u16::from_le_bytes([bytes[3], bytes[4]]),
            transfer_size: u16::from_le_bytes([bytes[5], bytes[6]]),
            version: u16::from_le_bytes([bytes[7], bytes[8]]),
        })
    }

    /// The things the interface says it can do, named the way the dictionary names them.
    #[must_use]
    pub fn capabilities(&self) -> Vec<Capability> {
        [
            (0x08, Capability::WillDetach),
            (0x04, Capability::ManifestationTolerant),
            (0x02, Capability::CanUpload),
            (0x01, Capability::CanDownload),
        ]
        .into_iter()
        .filter(|(bit, _)| self.attributes & bit != 0)
        .map(|(_, capability)| capability)
        .collect()
    }
}

/// One of the four bits the descriptor carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// The device lets go of the interface by itself.
    WillDetach,
    /// The device does the work of applying what was written before it answers.
    ManifestationTolerant,
    /// The device can send bytes back.
    CanUpload,
    /// The device can take bytes.
    CanDownload,
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::WillDetach => "will-detach",
            Self::ManifestationTolerant => "manifestation-tolerant",
            Self::CanUpload => "can-upload",
            Self::CanDownload => "can-download",
        };
        f.write_str(name)
    }
}

/// A device that answers class-specific requests.
#[async_trait]
pub trait Device: Send + Sync {
    /// What the interface says about itself.
    fn functional_descriptor(&self) -> &FunctionalDescriptor;

    /// A request in the direction that carries a payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the device is not there or when it refuses the request.
    async fn send(&mut self, number: u8, value: u16, payload: &[u8]) -> DeviceResult<()>;

    /// A request in the direction that answers with bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the device is not there or when it refuses the request.
    async fn receive(&mut self, number: u8, value: u16, length: u16) -> DeviceResult<Vec<u8>>;

    /// A string descriptor, decoded from the UTF-16 the interface writes it in.
    ///
    /// # Errors
    ///
    /// Returns an error when the device is not there, when it refuses the request, or when the bytes are
    /// not a string of the length they say.
    async fn string(&mut self, index: u16) -> DeviceResult<String>;

    /// A human-readable description, shown in the sheet.
    fn describe(&self) -> String;
}

/// Reads a string descriptor as the interface writes it: the length, the type, the language, and the text
/// in UTF-16 little-endian.
///
/// # Errors
///
/// Returns an error when the shape is not what the format says it is.
pub fn read_string(bytes: &[u8]) -> DeviceResult<String> {
    if bytes.len() < 4 || bytes[1] != 3 {
        return Err(DeviceError::BadAnswer {
            number: 0x06,
            reason: "the bytes are not a string descriptor".to_owned(),
        });
    }

    let body = &bytes[2 + 2..];

    if (body.len() & 1) != 0 {
        return Err(DeviceError::BadAnswer {
            number: 0x06,
            reason: format!("the text is {} bytes, which is not a pair", body.len()),
        });
    }

    let units: Vec<u16> = body
        .chunks(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();

    String::from_utf16(&units).map_err(|error| DeviceError::BadAnswer {
        number: 0x06,
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bytes a published implementation puts on the wire for an STM32 in DFU mode.
    const GOLDEN: &[u8] = &[9, 0x21, 0x0B, 0xFF, 0x00, 0x00, 0x08, 0x1A, 0x01];

    #[test]
    fn the_descriptor_is_read_as_the_interface_writes_it() {
        let descriptor = FunctionalDescriptor::read(GOLDEN).expect("nine bytes of the right type");

        assert_eq!(descriptor.attributes, 0x0B);
        assert_eq!(descriptor.detach_timeout, 255);
        assert_eq!(descriptor.transfer_size, 2048);
        assert_eq!(descriptor.version, 0x011A);
    }

    #[test]
    fn the_bits_of_the_descriptor_are_named_by_the_dictionary() {
        // 0x0B is will-detach, can-upload and can-download: it is not manifestation tolerant, which is
        // the bit a device that applies what was written before answering would carry.
        let descriptor = FunctionalDescriptor::read(GOLDEN).unwrap();
        assert_eq!(
            descriptor.capabilities(),
            vec![
                Capability::WillDetach,
                Capability::CanUpload,
                Capability::CanDownload
            ]
        );
    }

    #[test]
    fn a_shape_that_is_not_the_descriptor_is_refused_rather_than_read_as_fields() {
        // The same nine bytes with the type changed, and eight bytes of the right type: both are shapes
        // that cannot be read as fields, so they are named rather than filled with zeros.
        let wrong_type = {
            let mut bytes = GOLDEN.to_vec();
            bytes[1] = 0x22;
            bytes
        };

        assert!(FunctionalDescriptor::read(&wrong_type).is_err());
        assert!(FunctionalDescriptor::read(&GOLDEN[..8]).is_err());
    }

    #[test]
    fn a_string_is_read_from_the_utf16_the_interface_writes_it_in() {
        // Written from the rule of the format rather than taken from a decoder: the length, the type 3,
        // the language, and the text in pairs.
        let text = "@Internal Flash /0x08000000";
        let units: Vec<u8> = text
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect();

        let mut bytes = vec![units.len() as u8 + 4, 3, 0x09, 0x04];
        bytes.extend(units);

        assert_eq!(read_string(&bytes).unwrap(), text);
    }

    #[test]
    fn an_odd_number_of_bytes_that_arrived_cannot_be_read_as_whole_characters() {
        // The header is four bytes — the length, the type, and the language — so the text is what follows, and
        // three bytes of it is not a whole character. The declared length is not what decides this: the reader
        // reads the bytes that arrived, which is the same rule the DFU document carries for a field that lies
        // about its own size.
        let error =
            read_string(&[7, 3, 0x09, 0x04, b'a', b'b', b'c']).expect_err("an odd number of bytes");

        assert_eq!(error.code(), "device_bad_answer");
    }

    #[test]
    fn the_codes_of_the_device_layer_are_stable() {
        assert_eq!(
            DeviceError::Unreachable("nothing".to_owned()).code(),
            "device_unreachable"
        );
        assert_eq!(
            DeviceError::Stalled { number: 0x01 }.code(),
            "device_stalled"
        );
        assert_eq!(
            DeviceError::BadAnswer {
                number: 0x03,
                reason: "not six bytes".to_owned()
            }
            .code(),
            "device_bad_answer"
        );

        // The fourth is what a binding can say that the first cannot: a board that is on the bus and answers
        // to the address is not a board that this program may open. A code that covered both would tell the
        // operator to look for a board that is already there.
        assert_eq!(
            DeviceError::NotOurs {
                address: "0483:df11".to_owned(),
                reason: "the handle is held by another process".to_owned()
            }
            .code(),
            "device_not_ours"
        );
    }
}
