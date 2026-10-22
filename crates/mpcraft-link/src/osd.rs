//! The slots of the on-screen display.
//!
//! A vehicle draws an on-screen display over the picture of the camera. What it draws
//! is a set of **slots**, and each slot holds one thing: an altitude, a battery, a
//! parameter of the vehicle, or a function of the radio.
//!
//! The slots of the screens five and six are the ones the operator chooses: the vehicle
//! is asked what a slot holds, and it is told what to hold. Upstream has the same
//! thing (`ConfigOSD.cs`, and the `OSDConfigurator` library behind it).
//!
//! ```text
//! GCS  â”€â”€OSD_PARAM_SHOW_CONFIGâ”€â”€â”€â”€â–¶  vehicle
//! GCS  â—€â”€â”€OSD_PARAM_SHOW_CONFIG_REPLY  vehicle      (the name of the parameter)
//!
//! GCS  â”€â”€OSD_PARAM_CONFIGâ”€â”€â”€â”€â”€â”€â”€â”€â–¶  vehicle
//! GCS  â—€â”€â”€OSD_PARAM_CONFIG_REPLYâ”€â”€â”€  vehicle      (was it taken?)
//! ```

use std::time::Duration;

use mpcraft_mavlink::dialect::messages::{OSD_PARAM_CONFIG_DATA, OSD_PARAM_SHOW_CONFIG_DATA};
use mpcraft_mavlink::dialect::{MavMessage, OsdParamConfigError, OsdParamConfigType};
use mpcraft_mavlink::CharArray;

use crate::error::LinkError;
use crate::session::{ReceivedMessage, Session};
use crate::vehicle::VehicleId;

/// The id of the `OSD_PARAM_CONFIG` message.
pub const OSD_PARAM_CONFIG_ID: u32 = 11033;
/// The id of the `OSD_PARAM_CONFIG_REPLY` message.
pub const OSD_PARAM_CONFIG_REPLY_ID: u32 = 11034;
/// The id of the `OSD_PARAM_SHOW_CONFIG` message.
pub const OSD_PARAM_SHOW_CONFIG_ID: u32 = 11035;
/// The id of the `OSD_PARAM_SHOW_CONFIG_REPLY` message.
pub const OSD_PARAM_SHOW_CONFIG_REPLY_ID: u32 = 11036;

/// How long to wait for the vehicle to answer.
const OSD_TIMEOUT: Duration = Duration::from_secs(3);

/// One slot of the on-screen display.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsdSlot {
    /// The screen the slot is on: five and six are the ones the operator chooses.
    pub screen: u8,
    /// The number of the slot on that screen.
    pub index: u8,
    /// The name of the parameter the slot holds, when the vehicle says it.
    pub name: Option<String>,
    /// Whether the vehicle knows the slot.
    pub known: bool,
}

/// What a slot of the on-screen display is to hold.
#[derive(Debug, Clone)]
pub struct OsdSlotConfig {
    /// The screen the slot is on.
    pub screen: u8,
    /// The number of the slot on that screen.
    pub index: u8,
    /// The name of the parameter the slot is to hold.
    pub name: String,
    /// The kind of thing the slot holds.
    pub config_type: OsdParamConfigType,
    /// The lowest value the parameter takes.
    pub min: f32,
    /// The highest value the parameter takes.
    pub max: f32,
    /// The step the value moves by.
    pub increment: f32,
}

impl Session {
    /// Asks the vehicle what a slot of the on-screen display holds.
    ///
    /// The vehicle answers with the name of the parameter, or with nothing when the
    /// slot is one it does not know.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle does not answer in time.
    pub async fn osd_slot_show(
        &self,
        vehicle: VehicleId,
        screen: u8,
        index: u8,
        timeout: Duration,
    ) -> Result<Option<String>, LinkError> {
        let request_id = next_request_id();

        let mut collector = self
            .begin_collect(
                move |message: &ReceivedMessage| {
                    message.message_id == OSD_PARAM_SHOW_CONFIG_REPLY_ID
                        && message.system_id == vehicle.system_id
                        && message.component_id == vehicle.component_id
                },
                move |messages: &[ReceivedMessage]| {
                    messages.iter().any(|message| {
                        matches!(&message.message, MavMessage::OSD_PARAM_SHOW_CONFIG_REPLY(reply)
                            if reply.request_id == request_id)
                    })
                },
            )
            .await?;

        self.write_message(&MavMessage::OSD_PARAM_SHOW_CONFIG(
            OSD_PARAM_SHOW_CONFIG_DATA {
                request_id,
                target_system: vehicle.system_id,
                target_component: vehicle.component_id,
                osd_screen: screen,
                osd_index: index,
            },
        ))
        .await?;

        let messages = collector.wait_for(timeout).await?;

        for message in messages.iter().rev() {
            let MavMessage::OSD_PARAM_SHOW_CONFIG_REPLY(reply) = &message.message else {
                continue;
            };

            if reply.request_id != request_id {
                continue;
            }

            if reply.result != OsdParamConfigError::OSD_PARAM_SUCCESS {
                return Ok(None);
            }

            let name = char_array_to_string(&reply.param_id);

            return Ok(if name.is_empty() { None } else { Some(name) });
        }

        Ok(None)
    }

    /// Tells the vehicle what a slot of the on-screen display is to hold.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the change or does not answer.
    pub async fn osd_slot_config(
        &self,
        vehicle: VehicleId,
        config: OsdSlotConfig,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let request_id = next_request_id();

        let mut collector = self
            .begin_collect(
                move |message: &ReceivedMessage| {
                    message.message_id == OSD_PARAM_CONFIG_REPLY_ID
                        && message.system_id == vehicle.system_id
                        && message.component_id == vehicle.component_id
                },
                move |messages: &[ReceivedMessage]| {
                    messages.iter().any(|message| {
                        matches!(&message.message, MavMessage::OSD_PARAM_CONFIG_REPLY(reply)
                            if reply.request_id == request_id)
                    })
                },
            )
            .await?;

        self.write_message(&MavMessage::OSD_PARAM_CONFIG(OSD_PARAM_CONFIG_DATA {
            request_id,
            min_value: config.min,
            max_value: config.max,
            increment: config.increment,
            target_system: vehicle.system_id,
            target_component: vehicle.component_id,
            osd_screen: config.screen,
            osd_index: config.index,
            param_id: string_to_char_array(&config.name),
            config_type: config.config_type,
        }))
        .await?;

        let messages = collector.wait_for(timeout).await?;

        for message in messages.iter().rev() {
            let MavMessage::OSD_PARAM_CONFIG_REPLY(reply) = &message.message else {
                continue;
            };

            if reply.request_id != request_id {
                continue;
            }

            if reply.result != OsdParamConfigError::OSD_PARAM_SUCCESS {
                return Err(LinkError::Rejected(format!(
                    "the vehicle refused the slot: {reply:?}"
                )));
            }

            return Ok(());
        }

        Err(LinkError::Timeout(timeout))
    }

    /// Asks the vehicle what every slot of the given screens holds.
    ///
    /// The slots are numbered from one, the way the vehicle numbers them.
    ///
    /// A vehicle that does not know the messages is not an error: the slots come back
    /// empty, and the interface says that the vehicle did not answer.
    ///
    /// # Errors
    ///
    /// Returns an error when the link is closed.
    pub async fn osd_slots(
        &self,
        vehicle: VehicleId,
        screens: &[u8],
        slots: u8,
    ) -> Result<Vec<OsdSlot>, LinkError> {
        let mut found = Vec::new();

        for screen in screens {
            for index in 1..=slots {
                let name = self
                    .osd_slot_show(vehicle, *screen, index, OSD_TIMEOUT)
                    .await
                    .unwrap_or(None);

                found.push(OsdSlot {
                    screen: *screen,
                    index,
                    known: name.is_some(),
                    name,
                });
            }
        }

        Ok(found)
    }
}

/// The name of the parameter, as the answer carries it.
fn char_array_to_string(array: &CharArray<16>) -> String {
    let bytes: Vec<u8> = array.iter().copied().collect();

    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());

    String::from_utf8_lossy(&bytes[..end]).to_string()
}

/// The name of the parameter, as the message carries it.
fn string_to_char_array(name: &str) -> CharArray<16> {
    let mut array = CharArray::from([0_u8; 16]);

    for (index, byte) in name.as_bytes().iter().take(16).enumerate() {
        array[index] = *byte;
    }

    array
}

/// The number that names a request.
///
/// The vehicle answers with the number of the request it answers, so two requests in
/// flight cannot be told apart. The counter is per process, and it is enough for the
/// handful of requests a ground station has in flight.
fn next_request_id() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(1);

    COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_travels_in_the_array_of_sixteen_bytes() {
        let array = string_to_char_array("BAT1_VOLT");
        assert_eq!(char_array_to_string(&array), "BAT1_VOLT");
    }

    #[test]
    fn a_name_that_is_too_long_is_cut() {
        let array = string_to_char_array("A_NAME_THAT_IS_FAR_TOO_LONG");
        assert_eq!(char_array_to_string(&array), "A_NAME_THAT_IS_F");
    }

    #[test]
    fn an_empty_name_is_empty() {
        let array = string_to_char_array("");
        assert_eq!(char_array_to_string(&array), "");
    }
}
