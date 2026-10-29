//! DroneCAN over a vehicle's MAVLink link, by asking the autopilot to forward the bus.
//!
//! The frames of a CAN bus can be carried over MAVLink: the station asks for the bus with
//! `MAV_CMD_CAN_FORWARD`, the autopilot answers with a `CAN_FRAME` message for every frame it sees,
//! and the station writes its own frames back the same way. That is how Mission Planner reaches the
//! nodes of a vehicle (`ConfigDroneCAN.cs`), and it is what this module does, with the protocol
//! itself in `mpcraft-dronecan`.
//!
//! Two things are worth saying about the shape here:
//!
//! * the frames of one request go out **after** the matcher is registered, because the answer can
//!   begin arriving before the last frame of the request has been written;
//! * a service answer is not one message but several frames, so what the matcher collects is
//!   reassembled, and the exchange ends when the transfer is whole rather than when a message
//!   arrives.
//!
//! Nothing is asked of a bus the operator did not ask for: forwarding is a request of its own, and a
//! vehicle that is not forwarding answers nothing at all.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpcraft_dronecan::{CanId, DataType, Frame, Kind, Reassembler};

use crate::error::LinkError;
use crate::session::{RawMessage, Session};
use mpcraft_vehicle::VehicleId;

/// The message the frames travel in.
pub const CAN_FRAME_ID: u32 = 386;

/// The command that asks for a bus to be forwarded.
pub const CAN_FORWARD: u16 = 32000;

/// The number of bytes a `CAN_FRAME` payload takes.
const CAN_FRAME_BYTES: usize = 16;

impl Session {
    /// Asks the vehicle to forward one of its CAN buses over this link.
    ///
    /// A bus number of zero stops forwarding, which is what the dictionary says and what a station
    /// does when it is finished.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the command.
    pub async fn forward_can(&self, vehicle: VehicleId, bus: u8) -> Result<(), LinkError> {
        let payload = encode_command_long(
            CAN_FORWARD,
            f64::from(bus),
            vehicle.system_id,
            vehicle.component_id,
        );

        self.write_raw(COMMAND_LONG_ID, &payload).await
    }

    /// Writes one frame of a CAN bus.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry it.
    pub async fn write_can_frame(
        &self,
        vehicle: VehicleId,
        bus: u8,
        frame: &Frame,
    ) -> Result<(), LinkError> {
        let payload = encode_can_frame(vehicle, bus, frame);

        self.write_raw(CAN_FRAME_ID, &payload).await
    }

    /// Reads one node of a bus: asks it for what it is, and hands back the bytes it answered with.
    ///
    /// The frames of the request are written after the matcher is registered, and the exchange ends
    /// when the reassembled transfer is whole — a service answer of more than seven bytes is several
    /// frames, and a station that stopped at the first one would read half a name.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the request, or when the node does not answer
    /// within the budget.
    pub async fn dronecan_call(
        &self,
        vehicle: VehicleId,
        bus: u8,
        node: u8,
        request: &[Frame],
        expect: (Kind, DataType),
        timeout: Duration,
    ) -> Result<Vec<u8>, LinkError> {
        let reassembler = Arc::new(Mutex::new(Reassembler::default()));
        let answered: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));

        let collected = Arc::clone(&reassembler);
        let found = Arc::clone(&answered);
        let wanted = expect.0;
        let wanted_type = expect.1.id();

        let mut handle = self
            .begin_raw(CAN_FRAME_ID, move |message: &RawMessage| {
                if message.system_id != vehicle.system_id {
                    return false;
                }

                let Some((frame_bus, frame)) = decode_can_frame(message) else {
                    return false;
                };

                if frame_bus != bus {
                    return false;
                }

                let Ok(mut reassembler) = collected.lock() else {
                    return false;
                };

                let Some(received) = reassembler.accept(&frame, DataType::signature_of) else {
                    return false;
                };

                let can_id: CanId = received.can_id;

                if can_id.kind != wanted || can_id.type_id != wanted_type || can_id.source != node {
                    return false;
                }

                if let Ok(mut slot) = found.lock() {
                    *slot = Some(received.payload);
                }

                true
            })
            .await?;

        for frame in request {
            self.write_can_frame(vehicle, bus, frame).await?;
        }

        match handle.wait_for(timeout).await {
            Ok(_) => answered
                .lock()
                .ok()
                .and_then(|slot| slot.clone())
                .ok_or(LinkError::Timeout(timeout)),
            Err(error) => Err(error),
        }
    }
}

/// The command id of a `COMMAND_LONG`, which the link already writes for the other commands.
const COMMAND_LONG_ID: u32 = 76;

/// Builds the payload of a `COMMAND_LONG` carrying one slot.
fn encode_command_long(
    command: u16,
    param1: f64,
    target_system: u8,
    target_component: u8,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(33);

    // Seven floats, then the command, the target and the confirmation, in the order MAVLink sorts
    // the fields of this message: the wider types first.
    for value in [param1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0] {
        payload.extend_from_slice(&(value as f32).to_le_bytes());
    }

    payload.extend_from_slice(&command.to_le_bytes());
    payload.push(target_system);
    payload.push(target_component);
    payload.push(0);

    payload
}

/// Builds the payload of a `CAN_FRAME`.
fn encode_can_frame(vehicle: VehicleId, bus: u8, frame: &Frame) -> Vec<u8> {
    let mut payload = Vec::with_capacity(CAN_FRAME_BYTES);

    payload.extend_from_slice(&frame.id.to_le_bytes());
    payload.push(vehicle.system_id);
    payload.push(vehicle.component_id);
    payload.push(bus);
    payload.push(frame.data.len() as u8);

    let mut data = [0_u8; 8];
    let copied = frame.data.len().min(8);
    data[..copied].copy_from_slice(&frame.data[..copied]);

    payload.extend_from_slice(&data);

    payload
}

/// Reads a `CAN_FRAME` back into the bus number and the frame.
fn decode_can_frame(message: &RawMessage) -> Option<(u8, Frame)> {
    let payload = message.padded(CAN_FRAME_BYTES);

    let id = u32::from_le_bytes(payload[0..4].try_into().ok()?);
    let bus = payload[6];
    let length = usize::from(payload[7]).min(8);

    Some((
        bus,
        Frame {
            id,
            data: payload[8..8 + length].to_vec(),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpcraft_dronecan::NOMINAL_PRIORITY;

    fn vehicle() -> VehicleId {
        VehicleId::new(1, 1)
    }

    #[test]
    fn a_frame_carries_the_id_and_the_bytes_the_bus_needs() {
        let frame = Frame {
            id: CanId::service(NOMINAL_PRIORITY, 11, true, 42, 127),
            data: vec![1, 2, 3, 4],
        };

        let payload = encode_can_frame(vehicle(), 1, &frame);

        assert_eq!(payload.len(), CAN_FRAME_BYTES);
        assert_eq!(
            u32::from_le_bytes(payload[0..4].try_into().unwrap()),
            frame.id
        );
        assert_eq!(payload[4], 1); // target system
        assert_eq!(payload[5], 1); // target component
        assert_eq!(payload[6], 1); // bus
        assert_eq!(payload[7], 4); // length
        assert_eq!(&payload[8..12], &[1, 2, 3, 4]);
        assert_eq!(&payload[12..16], &[0, 0, 0, 0]);
    }

    #[test]
    fn a_frame_read_from_the_wire_is_the_frame_that_went_in() {
        let frame = Frame {
            id: 0x1032_1A7F,
            data: vec![9, 8, 7, 6, 5],
        };

        let payload = encode_can_frame(vehicle(), 2, &frame);
        let message = RawMessage {
            system_id: 1,
            component_id: 1,
            message_id: CAN_FRAME_ID,
            payload,
        };

        let (bus, read) = decode_can_frame(&message).expect("the frame is read");

        assert_eq!(bus, 2);
        assert_eq!(read, frame);
    }

    #[test]
    fn a_payload_trimmed_by_mavlink_is_still_a_frame() {
        // MAVLink 2 trims the zeros at the end, so a frame of one byte arrives as nine bytes.
        let message = RawMessage {
            system_id: 1,
            component_id: 1,
            message_id: CAN_FRAME_ID,
            payload: vec![0x00, 0x00, 0x00, 0x00, 1, 1, 1, 1, 0xC0],
        };

        let (bus, frame) = decode_can_frame(&message).expect("the frame is read");

        assert_eq!(bus, 1);
        assert_eq!(frame.id, 0);
        assert_eq!(frame.data, vec![0xC0]);
    }

    #[test]
    fn the_forward_command_carries_the_bus_in_its_first_slot() {
        let payload = encode_command_long(CAN_FORWARD, 2.0, 1, 1);

        assert_eq!(payload.len(), 33);
        assert_eq!(f32::from_le_bytes(payload[0..4].try_into().unwrap()), 2.0);
        assert_eq!(
            u16::from_le_bytes(payload[28..30].try_into().unwrap()),
            32000
        );
        assert_eq!(payload[30], 1);
        assert_eq!(payload[31], 1);
    }
}
