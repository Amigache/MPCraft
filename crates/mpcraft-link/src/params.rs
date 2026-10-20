//! The parameter protocol.
//!
//! The protocol is documented at <https://mavlink.io/en/services/parameter.html>. The three
//! operations are:
//!
//! | Operation | Request | Response |
//! | --- | --- | --- |
//! | Read all | `PARAM_REQUEST_LIST` | a stream of `PARAM_VALUE` |
//! | Read one | `PARAM_REQUEST_READ` | one `PARAM_VALUE` |
//! | Write one | `PARAM_SET` | one `PARAM_VALUE` carrying the value that was stored |
//!
//! The write is acknowledged by the vehicle *broadcasting* the value it stored. The
//! acknowledgement is how the ground station learns whether the write was accepted: the vehicle
//! must answer even when the write failed, in which case the answer carries the unchanged
//! value.

use std::time::Duration;

use mpcraft_mavlink::dialect::messages::{
    PARAM_REQUEST_LIST_DATA, PARAM_REQUEST_READ_DATA, PARAM_SET_DATA, PARAM_VALUE_DATA,
};
use mpcraft_mavlink::{MavMessage, MavProtocolCapability};
use mpcraft_params::{resolve_encoding, ParamEncoding, ParamType, Parameter, ParameterStore};

use crate::error::LinkError;
use crate::session::{ReceivedMessage, Session};
use crate::vehicle::VehicleId;

/// The message id of `PARAM_VALUE`.
pub const PARAM_VALUE_ID: u32 = 22;

/// The message id of `PARAM_ERROR`.
pub const PARAM_ERROR_ID: u32 = 345;

/// The message id of `PARAM_REQUEST_LIST`.
pub const PARAM_REQUEST_LIST_ID: u32 = 21;

/// The message id of `PARAM_REQUEST_READ`.
pub const PARAM_REQUEST_READ_ID: u32 = 20;

/// The message id of `PARAM_SET`.
pub const PARAM_SET_ID: u32 = 23;

/// The parameter index PX4 uses to announce a hash of its parameter set.
///
/// PX4 sends a `PARAM_VALUE` with this index to let a ground station reuse a cached parameter
/// set. It is not a real parameter.
pub const PX4_PARAM_HASH_INDEX: u16 = u16::MAX;

/// How long to wait for the parameter list before giving up.
pub const DEFAULT_LIST_TIMEOUT: Duration = Duration::from_secs(30);

/// How long to wait for a single parameter.
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(5);

/// How long to wait for a write to be acknowledged.
pub const DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_secs(3);

/// The reason a parameter operation failed.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamError {
    /// The vehicle answered with a `PARAM_ERROR`.
    Rejected {
        /// The parameter the error was about.
        name: String,
        /// The error the vehicle reported.
        code: u8,
    },
    /// The parameter is not known to the vehicle.
    DoesNotExist {
        /// The parameter name.
        name: String,
    },
    /// The write was acknowledged, but the value that came back is not the one that was set.
    ValueNotStored {
        /// The parameter name.
        name: String,
        /// The value that was requested.
        requested: f64,
        /// The value the vehicle reported.
        actual: f64,
    },
}

impl std::fmt::Display for ParamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected { name, code } => {
                write!(
                    f,
                    "the vehicle rejected the parameter {name} (error {code})"
                )
            }
            Self::DoesNotExist { name } => {
                write!(f, "the vehicle has no parameter named {name}")
            }
            Self::ValueNotStored {
                name,
                requested,
                actual,
            } => {
                write!(
                    f,
                    "the vehicle stored {actual} for {name}, not the requested {requested}"
                )
            }
        }
    }
}

impl std::error::Error for ParamError {}

/// The parameter operations of a session.
impl Session {
    /// The encoding this vehicle uses for its parameter values.
    ///
    /// The answer depends on the capabilities the vehicle advertised and on the autopilot it
    /// reported. See [`mpcraft_params::resolve_encoding`].
    #[must_use]
    pub fn param_encoding(&self) -> ParamEncoding {
        let state = self.state();
        resolve_encoding(
            state
                .capabilities
                .map(MavProtocolCapability::from_bits_truncate),
            state.vehicles.first().and_then(|v| v.autopilot),
        )
    }

    /// Downloads every parameter from the vehicle.
    ///
    /// The vehicle answers with a stream of `PARAM_VALUE` messages. The stream is complete when
    /// every parameter the vehicle reported has arrived; a vehicle that stops sending is
    /// detected by the timeout.
    ///
    /// # Errors
    ///
    /// Returns an error when the timeout expires or the session closes before the list is
    /// complete.
    pub async fn download_parameters(
        &self,
        vehicle: VehicleId,
        timeout: Duration,
    ) -> Result<ParameterStore, LinkError> {
        // The encoding of the values follows the autopilot the vehicle reports, so the
        // vehicle has to have introduced itself before the first value is decoded.
        self.wait_for_vehicle(vehicle, timeout).await?;

        let request = MavMessage::PARAM_REQUEST_LIST(PARAM_REQUEST_LIST_DATA {
            target_system: vehicle.system_id,
            target_component: vehicle.component_id,
        });

        // The collector is registered before the request is sent: a vehicle on a fast link
        // can answer before the request has even left the session.
        let collector = self
            .begin_collect(
                |message| message.message_id == PARAM_VALUE_ID,
                |messages| {
                    let total = messages
                        .last()
                        .map_or(0, |m| param_value_of(m).map_or(0, |v| v.param_count));
                    let received = messages
                        .iter()
                        .filter_map(param_value_of)
                        .filter(|v| v.param_index != PX4_PARAM_HASH_INDEX)
                        .map(|v| v.param_index)
                        .collect::<std::collections::HashSet<_>>()
                        .len();

                    total != 0 && received >= usize::from(total)
                },
            )
            .await?;

        self.write_message(&request).await?;

        let collected = collector.wait(timeout).await?;

        Ok(self.build_store(&collected))
    }

    /// Reads a single parameter from the vehicle.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle answers with a `PARAM_ERROR`, or when the timeout
    /// expires.
    pub async fn read_parameter(
        &self,
        vehicle: VehicleId,
        name: &str,
        timeout: Duration,
    ) -> Result<Parameter, LinkError> {
        // The encoding of the value follows the autopilot the vehicle reports.
        self.wait_for_vehicle(vehicle, timeout).await?;

        let request = MavMessage::PARAM_REQUEST_READ(PARAM_REQUEST_READ_DATA {
            param_index: -1,
            target_system: vehicle.system_id,
            target_component: vehicle.component_id,
            param_id: param_id(name),
        });

        let collector = self
            .begin_collect(
                |message| {
                    message.message_id == PARAM_VALUE_ID || message.message_id == PARAM_ERROR_ID
                },
                |messages| !messages.is_empty(),
            )
            .await?;

        self.write_message(&request).await?;

        let collected = collector.wait(timeout).await?;

        let Some(message) = collected.first() else {
            return Err(LinkError::Timeout(timeout));
        };

        if message.message_id == PARAM_ERROR_ID {
            return Err(LinkError::Rejected(
                ParamError::Rejected {
                    name: name.to_owned(),
                    code: param_error_code(message).unwrap_or(0),
                }
                .to_string(),
            ));
        }

        let value = param_value_of(message).ok_or(LinkError::Protocol(
            mpcraft_mavlink::ProtocolError::Malformed("the parameter answer carried no value"),
        ))?;

        if param_name_of(value) != name {
            return Err(LinkError::Rejected(
                ParamError::DoesNotExist {
                    name: name.to_owned(),
                }
                .to_string(),
            ));
        }

        Ok(self.build_parameter(value))
    }

    /// Writes a parameter to the vehicle.
    ///
    /// The vehicle acknowledges the write by broadcasting the value it stored. If the value
    /// that comes back is not the one that was requested, the write failed — which the vehicle
    /// reports by answering with the unchanged value.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle rejects the write, when the value it stored is not
    /// the one requested, or when the timeout expires.
    pub async fn write_parameter(
        &self,
        vehicle: VehicleId,
        name: &str,
        value: f64,
        timeout: Duration,
    ) -> Result<Parameter, LinkError> {
        // The encoding of the value follows the autopilot the vehicle reports.
        self.wait_for_vehicle(vehicle, timeout).await?;

        let encoding = self.param_encoding();
        let reported_type = self
            .state()
            .parameters
            .get(name)
            .map_or(ParamType::Real32, |parameter| parameter.reported_type);

        let request = MavMessage::PARAM_SET(PARAM_SET_DATA {
            param_value: f32::from_bits(encoding.encode(reported_type, value)),
            target_system: vehicle.system_id,
            target_component: vehicle.component_id,
            param_id: param_id(name),
            param_type: reported_type.as_mavlink(),
        });

        let collector = self
            .begin_collect(
                |message| {
                    message.message_id == PARAM_VALUE_ID || message.message_id == PARAM_ERROR_ID
                },
                |messages| !messages.is_empty(),
            )
            .await?;

        self.write_message(&request).await?;

        let collected = collector.wait(timeout).await?;

        let Some(message) = collected.first() else {
            return Err(LinkError::Timeout(timeout));
        };

        if message.message_id == PARAM_ERROR_ID {
            return Err(LinkError::Rejected(
                ParamError::Rejected {
                    name: name.to_owned(),
                    code: param_error_code(message).unwrap_or(0),
                }
                .to_string(),
            ));
        }

        let answer = param_value_of(message).ok_or(LinkError::Protocol(
            mpcraft_mavlink::ProtocolError::Malformed("the parameter answer carried no value"),
        ))?;

        let stored = self.build_parameter(answer);

        // The vehicle answers with the value it actually stored. A mismatch means the write
        // was refused.
        if (stored.value - value).abs() > f64::EPSILON * value.abs().max(1.0) {
            return Err(LinkError::Rejected(
                ParamError::ValueNotStored {
                    name: name.to_owned(),
                    requested: value,
                    actual: stored.value,
                }
                .to_string(),
            ));
        }

        Ok(stored)
    }

    /// Builds a parameter store from the collected messages.
    fn build_store(&self, messages: &[ReceivedMessage]) -> ParameterStore {
        let encoding = self.param_encoding();
        let mut store = ParameterStore::new();
        let mut seen = std::collections::HashSet::new();

        for message in messages {
            let Some(value) = param_value_of(message) else {
                continue;
            };

            // PX4 announces a parameter hash with this index. It is not a parameter.
            if value.param_index == PX4_PARAM_HASH_INDEX {
                continue;
            }

            let name = param_name_of(value);
            if name.is_empty() {
                continue;
            }

            // The vehicle can send the same parameter more than once; the first answer wins.
            if !seen.insert(value.param_index) {
                continue;
            }

            store.set_total_reported(value.param_count);

            let reported_type =
                ParamType::from_mavlink(value.param_type).unwrap_or(ParamType::Real32);
            let wire_bits = value.param_value.to_bits();

            store.insert(Parameter::new(
                name,
                encoding.decode(reported_type, wire_bits),
                reported_type,
            ));
        }

        store
    }

    /// Builds a single parameter from a message.
    fn build_parameter(&self, value: &PARAM_VALUE_DATA) -> Parameter {
        let encoding = self.param_encoding();
        let reported_type = ParamType::from_mavlink(value.param_type).unwrap_or(ParamType::Real32);

        Parameter::new(
            param_name_of(value),
            encoding.decode(reported_type, value.param_value.to_bits()),
            reported_type,
        )
    }
}

/// The `param_id` field of a message: the name, padded to sixteen bytes.
///
/// A name of exactly sixteen characters is not null terminated, which is what the protocol
/// requires.
fn param_id(name: &str) -> mpcraft_mavlink::CharArray<16> {
    let mut buffer = [0u8; 16];
    let bytes = name.as_bytes();
    let length = bytes.len().min(16);
    buffer[..length].copy_from_slice(&bytes[..length]);
    mpcraft_mavlink::CharArray::new(buffer)
}

/// The `PARAM_VALUE` payload of a message, if it is one.
fn param_value_of(message: &ReceivedMessage) -> Option<&PARAM_VALUE_DATA> {
    match &message.message {
        MavMessage::PARAM_VALUE(value) => Some(value),
        _ => None,
    }
}

/// The error code of a `PARAM_ERROR` message, if it is one.
fn param_error_code(message: &ReceivedMessage) -> Option<u8> {
    match &message.message {
        MavMessage::PARAM_ERROR(error) => Some(error.error as u8),
        _ => None,
    }
}

/// The name carried by a `PARAM_VALUE` payload.
pub(crate) fn param_name_of(value: &PARAM_VALUE_DATA) -> String {
    let bytes: Vec<u8> = value.param_id.iter().copied().collect();
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}
