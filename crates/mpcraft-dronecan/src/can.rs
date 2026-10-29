//! How DroneCAN lays a transfer out on a CAN bus.
//!
//! A CAN frame carries at most eight bytes, and DroneCAN uses the last of them for a *tail byte*
//! that says where the frame sits in the transfer and which transfer it belongs to:
//!
//! ```text
//! bit 7  start of transfer
//! bit 6  end of transfer
//! bit 5  toggle: 0 on the first frame, inverted on each frame after it
//! bits 0-4  transfer id, so two transfers of the same type are told apart
//! ```
//!
//! The 29-bit identifier is not an identifier of a frame but of the *thing being said*, and it is
//! read the way libcanard reads it, which is the implementation ArduPilot ships:
//!
//! ```text
//! bits 0-6    source node id
//! bit 7       service, not message
//! bits 8-14   destination node id (services only)
//! bit 15      request (services only; a response leaves it clear)
//! bits 16-23  service type id (services only)
//! bits 8-23   message type id (messages only)
//! bits 24-28  priority, lower is more urgent
//! ```
//!
//! A payload of up to seven bytes travels in one frame and needs no checksum, because the bus
//! already checks every frame. A longer payload is split, and the first frame carries the
//! CRC-16/CCITT-FALSE of the payload seeded with the data type's signature — the reason a multi-frame
//! transfer can only be read by a node that knows the type.

use crate::crc::crc16_from;

/// One frame of a CAN bus, as it is forwarded over MAVLink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The 29-bit identifier.
    pub id: u32,
    /// The bytes, of which the last is the tail byte.
    pub data: Vec<u8>,
}

/// What kind of transfer an identifier names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A message, addressed to whoever is listening.
    Message,
    /// A service request.
    Request,
    /// A service response.
    Response,
}

/// The fields an identifier carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanId {
    /// The priority, lower being more urgent.
    pub priority: u8,
    /// What kind of transfer it is.
    pub kind: Kind,
    /// The message or service type id.
    pub type_id: u16,
    /// The node that sent it.
    pub source: u8,
    /// The node it is for, for a service.
    pub destination: u8,
}

/// The priority a station uses unless it has a reason for another.
///
/// DroneCAN reserves 0 to 15 for the things that must be heard first; a service request from a
/// ground station is not one of them, and `16` is the nominal priority the specification names.
pub const NOMINAL_PRIORITY: u8 = 16;

impl CanId {
    /// The identifier of a message.
    #[must_use]
    pub fn message(priority: u8, type_id: u16, source: u8) -> u32 {
        (u32::from(priority & 0x1F) << 24) | (u32::from(type_id) << 8) | u32::from(source & 0x7F)
    }

    /// The identifier of a service request or response.
    #[must_use]
    pub fn service(
        priority: u8,
        service_id: u8,
        request: bool,
        destination: u8,
        source: u8,
    ) -> u32 {
        (u32::from(priority & 0x1F) << 24)
            | (u32::from(service_id) << 16)
            | (u32::from(request) << 15)
            | (u32::from(destination & 0x7F) << 8)
            | (1 << 7)
            | u32::from(source & 0x7F)
    }

    /// Reads an identifier, as the bus wrote it.
    #[must_use]
    pub fn parse(id: u32) -> Self {
        let service = id & (1 << 7) != 0;
        let request = id & (1 << 15) != 0;

        Self {
            priority: ((id >> 24) & 0x1F) as u8,
            kind: if !service {
                Kind::Message
            } else if request {
                Kind::Request
            } else {
                Kind::Response
            },
            type_id: if service {
                ((id >> 16) & 0xFF) as u16
            } else {
                ((id >> 8) & 0xFFFF) as u16
            },
            source: (id & 0x7F) as u8,
            destination: ((id >> 8) & 0x7F) as u8,
        }
    }
}

/// The tail byte of a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tail {
    /// This frame begins the transfer.
    pub start: bool,
    /// This frame ends the transfer.
    pub end: bool,
    /// The toggle bit, which is clear on the first frame.
    pub toggle: bool,
    /// Which transfer of this type it belongs to.
    pub transfer_id: u8,
}

impl Tail {
    /// The byte as it goes on the wire.
    #[must_use]
    pub fn byte(self) -> u8 {
        (u8::from(self.start) << 7)
            | (u8::from(self.end) << 6)
            | (u8::from(self.toggle) << 5)
            | (self.transfer_id & 0x1F)
    }

    /// The byte as it was read.
    #[must_use]
    pub fn parse(byte: u8) -> Self {
        Self {
            start: byte & 0x80 != 0,
            end: byte & 0x40 != 0,
            toggle: byte & 0x20 != 0,
            transfer_id: byte & 0x1F,
        }
    }
}

/// The bytes of a signature, least significant first, which is the order the checksum is fed in.
#[must_use]
pub fn signature_bytes(signature: u64) -> [u8; 8] {
    signature.to_le_bytes()
}

/// Cuts a payload into frames for one transfer.
///
/// `signature` is the signature of the data type, which the checksum of a multi-frame transfer is
/// seeded with; a payload that fits in one frame ignores it, because no checksum travels.
#[must_use]
pub fn encode(can_id: u32, transfer_id: u8, payload: &[u8], signature: u64) -> Vec<Frame> {
    let transfer_id = transfer_id & 0x1F;

    if payload.len() <= 7 {
        let mut data = payload.to_vec();
        data.push(
            Tail {
                start: true,
                end: true,
                toggle: false,
                transfer_id,
            }
            .byte(),
        );

        return vec![Frame { id: can_id, data }];
    }

    let checksum = crc16_from(0xFFFF, &signature_bytes(signature));
    let checksum = crc16_from(checksum, payload);

    let mut frames = Vec::new();
    let mut written = 0;
    let mut toggle = false;
    let mut first = true;

    while written < payload.len() {
        let mut data = Vec::with_capacity(8);

        if first {
            data.push((checksum & 0xFF) as u8);
            data.push((checksum >> 8) as u8);
        }

        let room = 7 - data.len();
        let take = room.min(payload.len() - written);

        data.extend_from_slice(&payload[written..written + take]);
        written += take;

        data.push(
            Tail {
                start: first,
                end: written == payload.len(),
                toggle,
                transfer_id,
            }
            .byte(),
        );

        frames.push(Frame { id: can_id, data });

        first = false;
        toggle = !toggle;
    }

    frames
}

/// Collects the frames of a multi-frame transfer, and hands back the payload when it is whole.
///
/// A transfer is identified by the node that sent it, the type it carries and whether it is a
/// message or a service — not by the transfer id, which changes with every transfer.
#[derive(Debug, Default)]
pub struct Reassembler {
    open: Vec<Open>,
}

#[derive(Debug)]
struct Open {
    key: (u8, u16, bool),
    transfer_id: u8,
    toggle: bool,
    checksum: u16,
    payload: Vec<u8>,
}

impl Open {
    fn key(can_id: &CanId) -> (u8, u16, bool) {
        (
            can_id.source,
            can_id.type_id,
            matches!(can_id.kind, Kind::Request | Kind::Response),
        )
    }
}

/// What a complete transfer carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    /// The identifier of the frames it came in.
    pub can_id: CanId,
    /// The bytes of the payload.
    pub payload: Vec<u8>,
}

impl Reassembler {
    /// Reads one frame, and returns the payload when the frame completes a transfer.
    ///
    /// A transfer whose checksum does not match is dropped rather than handed back: what the bus
    /// carried was not what the sender meant, and a station that guessed would show a node's name
    /// made of a frame that was lost.
    #[must_use]
    pub fn accept(
        &mut self,
        frame: &Frame,
        signature_of: impl Fn(u16) -> Option<u64>,
    ) -> Option<Received> {
        if frame.data.is_empty() || frame.data.len() > 8 {
            return None;
        }

        let tail = Tail::parse(*frame.data.last()?);
        let can_id = CanId::parse(frame.id);
        let key = Open::key(&can_id);
        let body = &frame.data[..frame.data.len() - 1];

        if tail.start && tail.end {
            return Some(Received {
                can_id,
                payload: body.to_vec(),
            });
        }

        if tail.start {
            // The type has to be one this station knows, because its signature is what the transfer
            // is checksummed with; a transfer of a type nobody knows is not collected at all.
            signature_of(can_id.type_id)?;

            if body.len() < 2 {
                return None;
            }

            let checksum = u16::from(body[0]) | (u16::from(body[1]) << 8);
            let payload = body[2..].to_vec();

            self.open
                .retain(|open| open.key != key || open.transfer_id != tail.transfer_id);

            self.open.push(Open {
                key,
                transfer_id: tail.transfer_id,
                toggle: true,
                checksum,
                payload,
            });

            return None;
        }

        let position = self
            .open
            .iter()
            .position(|open| open.key == key && open.transfer_id == tail.transfer_id);

        let Some(position) = position else {
            // The start of this transfer was lost, or it is not one this station asked for.
            return None;
        };

        if self.open[position].toggle != tail.toggle {
            self.open.remove(position);

            return None;
        }

        self.open[position].payload.extend_from_slice(body);

        if !tail.end {
            self.open[position].toggle = !tail.toggle;

            return None;
        }

        let open = self.open.remove(position);
        let signature = signature_of(can_id.type_id)?;
        let checksum = crc16_from(0xFFFF, &signature_bytes(signature));
        let checksum = crc16_from(checksum, &open.payload);

        if checksum != open.checksum {
            return None;
        }

        Some(Received {
            can_id,
            payload: open.payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The signature of `uavcan.protocol.param.GetSet`, read from a second implementation
    /// (`pydronecan`'s `get_data_type_signature`).
    const GET_SET_SIGNATURE: u64 = 0xA71A_635C_F734_A0DA;

    #[test]
    fn a_message_identifier_carries_the_type_the_source_and_the_priority() {
        let id = CanId::message(NOMINAL_PRIORITY, 341, 10);
        let read = CanId::parse(id);

        assert_eq!(read.kind, Kind::Message);
        assert_eq!(read.type_id, 341);
        assert_eq!(read.source, 10);
        assert_eq!(read.priority, NOMINAL_PRIORITY);

        // 16 << 24 | 341 << 8 | 10, worked out by hand from the layout above.
        assert_eq!(id, 0x1001_550A);
    }

    #[test]
    fn a_service_identifier_says_which_node_and_whether_it_asks() {
        let request = CanId::service(NOMINAL_PRIORITY, 11, true, 42, 127);
        let read = CanId::parse(request);

        assert_eq!(read.kind, Kind::Request);
        assert_eq!(read.type_id, 11);
        assert_eq!(read.destination, 42);
        assert_eq!(read.source, 127);

        let response = CanId::service(NOMINAL_PRIORITY, 11, false, 127, 42);
        let read = CanId::parse(response);

        assert_eq!(read.kind, Kind::Response);
        assert_eq!(read.destination, 127);
        assert_eq!(read.source, 42);
    }

    #[test]
    fn a_payload_that_fits_in_one_frame_is_one_frame() {
        let frames = encode(0x1234, 5, &[1, 2, 3], GET_SET_SIGNATURE);

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, vec![1, 2, 3, 0xC5]);
    }

    #[test]
    fn a_short_payload_of_exactly_seven_bytes_is_still_one_frame() {
        let frames = encode(0x1234, 0, &[0; 7], GET_SET_SIGNATURE);

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data.len(), 8);
        assert_eq!(*frames[0].data.last().unwrap(), 0xC0);
    }

    #[test]
    fn a_longer_payload_is_cut_with_the_checksum_in_front() {
        let payload: Vec<u8> = (0..20).collect();
        let frames = encode(0x1234, 7, &payload, GET_SET_SIGNATURE);

        // 2 checksum bytes + 20 payload bytes is 22, and each frame carries 7 after the first has
        // room for 5: 5, 7, 7, 3 bytes of payload, so four frames.
        assert_eq!(frames.len(), 4);

        let first = Tail::parse(*frames[0].data.last().unwrap());
        assert!(first.start && !first.end);
        assert!(!first.toggle);

        let second = Tail::parse(*frames[1].data.last().unwrap());
        assert!(!second.start && !second.end);
        assert!(second.toggle);

        let last = Tail::parse(*frames[3].data.last().unwrap());
        assert!(!last.start && last.end);

        for frame in &frames {
            assert_eq!(Tail::parse(*frame.data.last().unwrap()).transfer_id, 7);
        }
    }

    #[test]
    fn the_frames_of_a_transfer_come_back_as_the_payload_that_went_in() {
        let payload: Vec<u8> = (0..40).collect();
        let frames = encode(
            CanId::message(NOMINAL_PRIORITY, 1, 42),
            3,
            &payload,
            GET_SET_SIGNATURE,
        );

        let mut reassembler = Reassembler::default();
        let mut whole = None;

        for frame in &frames {
            if let Some(received) = reassembler.accept(frame, |_| Some(GET_SET_SIGNATURE)) {
                whole = Some(received);
            }
        }

        let received = whole.expect("the transfer is whole");

        assert_eq!(received.payload, payload);
        assert_eq!(received.can_id.source, 42);
        assert_eq!(received.can_id.type_id, 1);
    }

    #[test]
    fn a_transfer_whose_bytes_were_changed_is_dropped() {
        let payload: Vec<u8> = (0..20).collect();
        let mut frames = encode(
            CanId::message(NOMINAL_PRIORITY, 1, 42),
            3,
            &payload,
            GET_SET_SIGNATURE,
        );

        // A byte in the middle is corrupted, as a bus does when a frame is lost or a wire is long.
        frames[1].data[0] ^= 0xFF;

        let mut reassembler = Reassembler::default();
        let mut whole = None;

        for frame in &frames {
            if let Some(received) = reassembler.accept(frame, |_| Some(GET_SET_SIGNATURE)) {
                whole = Some(received);
            }
        }

        assert!(
            whole.is_none(),
            "a checksum that does not agree is not a transfer"
        );
    }

    #[test]
    fn a_frame_whose_toggle_does_not_agree_ends_the_transfer() {
        let payload: Vec<u8> = (0..20).collect();
        let mut frames = encode(
            CanId::message(NOMINAL_PRIORITY, 1, 42),
            3,
            &payload,
            GET_SET_SIGNATURE,
        );

        // The second frame arrives twice, which is what a duplicated frame looks like.
        let duplicated = frames[1].clone();
        frames.insert(2, duplicated);

        let mut reassembler = Reassembler::default();
        let mut whole = None;

        for frame in &frames {
            if let Some(received) = reassembler.accept(frame, |_| Some(GET_SET_SIGNATURE)) {
                whole = Some(received);
            }
        }

        assert!(
            whole.is_none(),
            "the transfer is not read from frames that do not follow"
        );
    }

    #[test]
    fn a_type_the_station_does_not_know_is_not_read() {
        let payload: Vec<u8> = (0..20).collect();
        let frames = encode(
            CanId::message(NOMINAL_PRIORITY, 999, 42),
            3,
            &payload,
            GET_SET_SIGNATURE,
        );

        let mut reassembler = Reassembler::default();

        for frame in &frames {
            assert!(reassembler.accept(frame, |_| None).is_none());
        }
    }
}
