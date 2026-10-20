//! The mission protocol.
//!
//! The protocol is documented at <https://mavlink.io/en/services/mission.html>. There are
//! two directions, and they are not symmetric.
//!
//! # Downloading
//!
//! The ground station asks for the whole plan, the vehicle answers with the number of items,
//! and the ground station then asks for each item in turn.
//!
//! ```text
//! GCS  ──MISSION_REQUEST_LIST──▶  vehicle
//! GCS  ◀────MISSION_COUNT──────  vehicle      (how many items there are)
//! GCS  ──MISSION_REQUEST(seq)──▶  vehicle
//! GCS  ◀────MISSION_ITEM(seq)────  vehicle      (repeated for each item)
//! ```
//!
//! # Uploading
//!
//! The ground station announces the number of items, and then the **vehicle asks for each
//! item in turn**. The ground station cannot simply push the items: the vehicle drives the
//! exchange, which is what lets it abort the transfer by simply not asking for the next one.
//!
//! ```text
//! GCS  ────MISSION_COUNT──────▶  vehicle      (how many items will follow)
//! GCS  ◀────MISSION_REQUEST────  vehicle      (which item does it want next?)
//! GCS  ────MISSION_ITEM──────▶  vehicle      (repeated for each item)
//! GCS  ◀────MISSION_ACK───────  vehicle      (accepted, or an error)
//! ```
//!
//! # The integer encoding
//!
//! A vehicle that advertises `MAV_PROTOCOL_CAPABILITY::MISSION_INT` wants the integer form
//! of the items, whose coordinates are scaled by `1e7`. Mission Planner does the same.
//!
//! # A limitation of the generated dialect
//!
//! `MAV_MISSION_TYPE` is a MAVLink 2 *extension* field, and the generated bindings do not
//! expose extension fields for the mission messages. A plain mission travels in the short
//! form — `mission_type` is zero and is truncated away — so it works. The fence and rally
//! plans carry a non-zero `mission_type`, so their frames are one byte longer than the
//! bindings expect.
//!
//! The frames still *validate*: the checksum covers the whole payload, extension included.
//! What is lost is the mission type itself, which the protocol cannot express through these
//! bindings.

use std::time::Duration;

#[allow(deprecated)]
use mpcraft_mavlink::dialect::messages::{
    MISSION_COUNT_DATA, MISSION_REQUEST_DATA, MISSION_REQUEST_INT_DATA, MISSION_REQUEST_LIST_DATA,
};
use mpcraft_mavlink::{MavMessage, MavMissionResult, MavProtocolCapability};
use mpcraft_mission::{MissionItem, MissionType, Plan};

use crate::error::LinkError;
use crate::session::{ReceivedMessage, Session};
use crate::vehicle::VehicleId;

/// The message id of `MISSION_ITEM`.
pub const MISSION_ITEM_ID: u32 = 39;
/// The message id of `MISSION_REQUEST`.
pub const MISSION_REQUEST_ID: u32 = 40;
/// The message id of `MISSION_REQUEST_LIST`.
pub const MISSION_REQUEST_LIST_ID: u32 = 43;
/// The message id of `MISSION_COUNT`.
pub const MISSION_COUNT_ID: u32 = 44;
/// The message id of `MISSION_ACK`.
pub const MISSION_ACK_ID: u32 = 47;
/// The message id of `MISSION_REQUEST_INT`.
pub const MISSION_REQUEST_INT_ID: u32 = 51;
/// The message id of `MISSION_ITEM_INT`.
pub const MISSION_ITEM_INT_ID: u32 = 73;

/// How long to wait for one item.
pub const DEFAULT_ITEM_TIMEOUT: Duration = Duration::from_secs(3);

/// The reasons a mission operation can fail.
#[derive(Debug, Clone, PartialEq)]
pub enum MissionError {
    /// The vehicle refused the plan.
    Rejected {
        /// The result the vehicle reported.
        result: MavMissionResult,
    },
    /// The vehicle asked for an item that was not in the plan.
    UnexpectedSequence {
        /// The sequence the vehicle asked for.
        seq: u16,
        /// The number of items in the plan.
        count: usize,
    },
    /// The plan is empty.
    Empty,
}

impl std::fmt::Display for MissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected { result } => {
                write!(f, "the vehicle rejected the plan: {result:?}")
            }
            Self::UnexpectedSequence { seq, count } => {
                write!(
                    f,
                    "the vehicle asked for item {seq} of a plan with {count} items"
                )
            }
            Self::Empty => write!(f, "the plan has no items"),
        }
    }
}

impl std::error::Error for MissionError {}

/// The mission operations of a session.
impl Session {
    /// Whether this vehicle wants the integer form of the mission items.
    #[must_use]
    pub fn uses_mission_int(&self) -> bool {
        self.state()
            .capabilities
            .map(MavProtocolCapability::from_bits_truncate)
            .is_some_and(|capabilities| {
                capabilities.contains(MavProtocolCapability::MAV_PROTOCOL_CAPABILITY_MISSION_INT)
            })
    }

    /// Downloads a plan from the vehicle.
    ///
    /// The vehicle reports how many items it has, and the items are then requested one by
    /// one.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle does not answer, or when it answers with the wrong
    /// item.
    pub async fn download_mission(
        &self,
        vehicle: VehicleId,
        mission_type: MissionType,
        timeout: Duration,
    ) -> Result<Plan, LinkError> {
        let _ = mission_type;

        // The collector is registered before the request is sent.
        let collector = self
            .begin_collect(
                |message| message.message_id == MISSION_COUNT_ID,
                |messages| !messages.is_empty(),
            )
            .await?;

        self.write_message(&MavMessage::MISSION_REQUEST_LIST(
            MISSION_REQUEST_LIST_DATA {
                target_system: vehicle.system_id,
                target_component: vehicle.component_id,
            },
        ))
        .await?;

        let counted = collector.wait(timeout).await?;
        let count = mission_count_of(counted.first()).ok_or(LinkError::Protocol(
            mpcraft_mavlink::ProtocolError::Malformed(
                "the vehicle did not report how many items it has",
            ),
        ))?;

        let mut plan = Plan::new(mission_type);
        let use_int = self.uses_mission_int();

        for seq in 0..count {
            let item = self
                .request_mission_item(vehicle, seq, use_int, timeout)
                .await?;
            plan.push(item);
        }

        Ok(plan)
    }

    /// Uploads a plan to the vehicle.
    ///
    /// The vehicle asks for each item in turn; the ground station answers. The vehicle
    /// finishes with an acknowledgement, which says whether it accepted the plan.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle rejects the plan, or when it asks for an item that
    /// the plan does not have.
    pub async fn upload_mission(
        &self,
        vehicle: VehicleId,
        plan: &Plan,
        timeout: Duration,
    ) -> Result<MavMissionResult, LinkError> {
        if plan.is_empty() {
            return Err(LinkError::Rejected(MissionError::Empty.to_string()));
        }

        let use_int = self.uses_mission_int();

        // The matcher is registered before the plan is announced: the vehicle answers the
        // announcement with its first request, and a fast vehicle would answer before
        // anybody was listening.
        let mut request = self
            .begin_request(|message| {
                message.message_id == MISSION_REQUEST_ID
                    || message.message_id == MISSION_REQUEST_INT_ID
                    || message.message_id == MISSION_ACK_ID
            })
            .await?;

        self.write_message(&MavMessage::MISSION_COUNT(MISSION_COUNT_DATA {
            count: plan.len() as u16,
            target_system: vehicle.system_id,
            target_component: vehicle.component_id,
        }))
        .await?;

        loop {
            let message = request.wait(timeout).await?;

            // The next answer needs a fresh matcher, registered before the next item goes
            // out, so that a fast vehicle cannot get ahead of us.
            request = self
                .begin_request(|message| {
                    message.message_id == MISSION_REQUEST_ID
                        || message.message_id == MISSION_REQUEST_INT_ID
                        || message.message_id == MISSION_ACK_ID
                })
                .await?;

            #[allow(deprecated)]
            match message.message_id {
                MISSION_ACK_ID => {
                    let ack = mission_ack_of(&message).ok_or(LinkError::Protocol(
                        mpcraft_mavlink::ProtocolError::Malformed(
                            "the acknowledgement carried no result",
                        ),
                    ))?;

                    if ack != MavMissionResult::MAV_MISSION_ACCEPTED {
                        return Err(LinkError::Rejected(
                            MissionError::Rejected { result: ack }.to_string(),
                        ));
                    }

                    return Ok(ack);
                }

                MISSION_REQUEST_ID | MISSION_REQUEST_INT_ID => {
                    #[allow(deprecated)]
                    let seq = mission_request_seq_of(&message).ok_or(LinkError::Protocol(
                        mpcraft_mavlink::ProtocolError::Malformed(
                            "the request carried no sequence number",
                        ),
                    ))?;

                    let item = plan.items.get(seq as usize).ok_or_else(|| {
                        LinkError::Rejected(
                            MissionError::UnexpectedSequence {
                                seq,
                                count: plan.len(),
                            }
                            .to_string(),
                        )
                    })?;

                    #[allow(deprecated)]
                    let item_message = if use_int {
                        MavMessage::MISSION_ITEM_INT(item.to_int_message(seq))
                    } else {
                        MavMessage::MISSION_ITEM(item.to_float_message(seq))
                    };

                    self.write_message(&item_message).await?;
                }

                _ => {}
            }
        }
    }

    /// Requests one item of a plan and waits for it.
    ///
    /// The float form of the messages is deprecated in the MAVLink specification in favour of
    /// the integer form, but a vehicle that does not advertise `MISSION_INT` still expects
    /// it.
    #[allow(deprecated)]
    async fn request_mission_item(
        &self,
        vehicle: VehicleId,
        seq: u16,
        use_int: bool,
        timeout: Duration,
    ) -> Result<MissionItem, LinkError> {
        let collector = self
            .begin_collect(
                |message| {
                    message.message_id == MISSION_ITEM_ID
                        || message.message_id == MISSION_ITEM_INT_ID
                },
                |messages| !messages.is_empty(),
            )
            .await?;

        #[allow(deprecated)]
        let request = if use_int {
            MavMessage::MISSION_REQUEST_INT(MISSION_REQUEST_INT_DATA {
                seq,
                target_system: vehicle.system_id,
                target_component: vehicle.component_id,
            })
        } else {
            MavMessage::MISSION_REQUEST(MISSION_REQUEST_DATA {
                seq,
                target_system: vehicle.system_id,
                target_component: vehicle.component_id,
            })
        };

        self.write_message(&request).await?;

        let received = collector.wait(timeout).await?;
        let Some(message) = received.first() else {
            return Err(LinkError::Timeout(timeout));
        };

        #[allow(deprecated)]
        match &message.message {
            MavMessage::MISSION_ITEM(item) => {
                // An item that is not the one that was asked for is discarded: the vehicle
                // may be answering a request that was lost.
                if item.seq != seq {
                    return Err(LinkError::Rejected(
                        MissionError::UnexpectedSequence { seq, count: 0 }.to_string(),
                    ));
                }
                Ok(MissionItem::from_float_message(item))
            }
            MavMessage::MISSION_ITEM_INT(item) => {
                if item.seq != seq {
                    return Err(LinkError::Rejected(
                        MissionError::UnexpectedSequence { seq, count: 0 }.to_string(),
                    ));
                }
                Ok(MissionItem::from_int_message(item))
            }
            _ => Err(LinkError::Protocol(
                mpcraft_mavlink::ProtocolError::Malformed(
                    "the answer to a mission request was not a mission item",
                ),
            )),
        }
    }
}

/// The number of items a `MISSION_COUNT` reports.
fn mission_count_of(message: Option<&ReceivedMessage>) -> Option<u16> {
    match &message?.message {
        MavMessage::MISSION_COUNT(count) => Some(count.count),
        _ => None,
    }
}

/// The result a `MISSION_ACK` carries.
fn mission_ack_of(message: &ReceivedMessage) -> Option<MavMissionResult> {
    match &message.message {
        MavMessage::MISSION_ACK(ack) => Some(ack.mavtype),
        _ => None,
    }
}

/// The sequence a `MISSION_REQUEST` asks for.
#[allow(deprecated)]
fn mission_request_seq_of(message: &ReceivedMessage) -> Option<u16> {
    match &message.message {
        MavMessage::MISSION_REQUEST(request) => Some(request.seq),
        MavMessage::MISSION_REQUEST_INT(request) => Some(request.seq),
        _ => None,
    }
}
