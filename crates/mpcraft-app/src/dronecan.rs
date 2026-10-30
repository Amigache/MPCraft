//! The nodes of a DroneCAN bus, and what the station asks the vehicle for.
//!
//! The link keeps the nodes — the registry is the state the bus reported. This says when the ask
//! goes out, which is a decision about what the interface shows rather than a rule of the protocol.
//!
//! The published dictionary says the command makes the vehicle answer with one information message
//! per node that is online, and that some of those answers can be lost: the receiver notices by
//! seeing a status with no matching information, and asks again. So the rule is this — ask once when
//! the link opens, the way the tab of the outputs asks for the relays and the flight-data window asks
//! for its streams; and ask once more for a node that has reported how it is without saying what it
//! is. It is not asked forever: a bus that never answers would otherwise put a command on the wire
//! every time the sheet is read.
//!
//! The count belongs to the link it was made on. A link that was restored after dropping is a new
//! bus, so the rule starts again there — which is what the loop of the formation does for the same
//! reason.

use std::sync::Arc;

use mpcraft_dronecan::{
    encode, CanId, DataType, ExecuteOpcode, GetSetRequest, GetSetResponse, Kind, NodeInfo,
    ParameterValue, RestartNode, NOMINAL_PRIORITY,
};
use mpcraft_link::{DronecanNode, Session, VehicleId};
use serde::Serialize;

use crate::error::{CommandError, CommandResult};

/// The node id this station speaks as on a bus.
///
/// DroneCAN numbers its nodes 1 to 127, and a ground station is one of them: it has to have a number
/// to send a service request, and 127 is the last one, which is where a station belongs.
const STATION_NODE_ID: u8 = 127;

/// How many times the ask goes out on one link: once at open, and once more for a status that has no
/// matching information.
pub const MAX_ASKS: usize = 2;

/// What the sheet shows: the nodes as the bus said them, and what the station asked for.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DronecanReport {
    /// The nodes the bus has reported, in the order the pairs sort in.
    pub nodes: Vec<DronecanNode>,
    /// How many times the ask went out on this link.
    pub asked: usize,
    /// Whether this reading put the ask on the wire.
    pub sent_now: bool,
    /// The last failure, when there was one.
    pub error: Option<String>,
}

/// The rule the ask follows, written apart from the wire so that it can be read without a peer.
///
/// The first reading asks. After that the ask goes out only for a node that has reported how it is
/// without saying what it is, and only once more. A third reading says nothing: a bus that will not
/// answer is not answered by asking again.
#[must_use]
pub fn should_ask(asked: usize, nodes: &[DronecanNode]) -> bool {
    if asked == 0 {
        return true;
    }

    asked < MAX_ASKS && nodes.iter().any(|node| node.info_missing)
}

/// What the station asks for its nodes.
#[derive(Clone, Default)]
pub struct DronecanService {
    inner: Arc<std::sync::Mutex<Inner>>,
}

/// One parameter of a node, as the sheet shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeParameter {
    /// The name the node gives it.
    pub name: String,
    /// The value it holds.
    pub value: String,
    /// The value it had before anybody changed it.
    pub default_value: String,
    /// The smallest value it accepts, when the node says.
    pub min_value: String,
    /// The largest value it accepts, when the node says.
    pub max_value: String,
}

/// How long one exchange with a node is given.
///
/// A node on a bus answers in milliseconds; the budget is what a station waiting on a link with
/// other traffic gives it before saying it did not answer.
const CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1_500);

/// What a node says it is, as the interface shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfoView {
    /// The name the node gives itself.
    pub name: String,
    /// Seconds since it started.
    pub uptime_sec: u32,
    /// How it is, in the dictionary's words when it has them.
    pub health: String,
    /// What it is doing, in the dictionary's words when it has them.
    pub mode: String,
    /// The major version of its software.
    pub software_major: u8,
    /// The minor version of its software.
    pub software_minor: u8,
    /// The major version of its hardware.
    pub hardware_major: u8,
    /// The minor version of its hardware.
    pub hardware_minor: u8,
    /// The 128-bit id that is unique to it, in hexadecimal.
    pub unique_id: String,
}

impl NodeInfoView {
    /// Reads the view out of what the node answered.
    #[must_use]
    pub fn of(info: &NodeInfo) -> Self {
        Self {
            name: info.name.clone(),
            uptime_sec: info.status.uptime_sec,
            health: mpcraft_link::health_name(info.status.health)
                .map_or_else(|| info.status.health.to_string(), str::to_owned),
            mode: mpcraft_link::mode_name(info.status.mode)
                .map_or_else(|| info.status.mode.to_string(), str::to_owned),
            software_major: info.software_major,
            software_minor: info.software_minor,
            hardware_major: info.hardware_major,
            hardware_minor: info.hardware_minor,
            unique_id: info
                .unique_id
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        }
    }
}

/// How many parameters a listing reads before it stops, so an answer that never empties cannot
/// keep the sheet waiting for ever.
const MAX_PARAMETERS: usize = 512;

#[derive(Default)]
struct Inner {
    /// The link the count belongs to, so that a restored link starts the rule again.
    session: Option<Arc<Session>>,
    asked: usize,
    error: Option<String>,
}

impl DronecanService {
    /// An empty service.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads the bus and applies the rule: the ask goes out when the rule says it should.
    ///
    /// Nothing is waited for, because the command has no acknowledgement — the outcome is what the
    /// bus answers with, and the sheet shows the nodes that still have no matching information.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the command.
    pub async fn scan(
        &self,
        session: &Arc<Session>,
        vehicle: VehicleId,
    ) -> CommandResult<DronecanReport> {
        let nodes = session.state().dronecan;

        let ask_now = match self.inner.lock() {
            Ok(mut inner) => {
                if !inner
                    .session
                    .as_ref()
                    .is_some_and(|held| Arc::ptr_eq(held, session))
                {
                    // A new link is a new bus: the count is not carried over from the one that went away.
                    inner.session = Some(Arc::clone(session));
                    inner.asked = 0;
                    inner.error = None;
                }

                should_ask(inner.asked, &nodes)
            }
            Err(_poisoned) => false,
        };

        if ask_now {
            match session.send_node_info_request(vehicle).await {
                Ok(()) => {
                    if let Ok(mut inner) = self.inner.lock() {
                        inner.asked += 1;
                    }
                }
                Err(error) => {
                    if let Ok(mut inner) = self.inner.lock() {
                        inner.error = Some(error.to_string());
                    }

                    return Err(error.into());
                }
            }
        }

        let asked = self.inner.lock().map(|inner| inner.asked).unwrap_or(0);
        let error = self.inner.lock().ok().and_then(|inner| inner.error.clone());

        Ok(DronecanReport {
            nodes,
            asked,
            sent_now: ask_now,
            error,
        })
    }

    /// Reads what a node says it is, over a bus the vehicle forwards.
    ///
    /// The forwarding is asked for every time rather than remembered: it is a command the vehicle
    /// either honours at this moment or does not, and a station that assumed it still held would be
    /// reading a bus that had stopped being forwarded.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the request, when the node does not answer, or
    /// when what it answered is not the information this crate knows.
    pub async fn node_info(
        &self,
        session: &Arc<Session>,
        vehicle: VehicleId,
        bus: u8,
        node: u8,
    ) -> CommandResult<NodeInfo> {
        let answer = self
            .ask(
                session,
                vehicle,
                bus,
                node,
                &[],
                (Kind::Response, DataType::GetNodeInfo),
            )
            .await?;

        NodeInfo::decode(&answer).ok_or_else(|| {
            CommandError::new(
                CommandError::CODE_INTERNAL,
                "the node's information could not be read",
            )
        })
    }

    /// Lists the parameters a node holds.
    ///
    /// The list is read by index, which is what the standard says an index is for: a station asks
    /// for 0, 1, 2 and so on until the node answers with no name. Each read is one exchange, and the
    /// count is bounded so a node that never empties cannot hold the sheet.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry a request, or when a node that answered before
    /// stops answering in the middle of its list.
    pub async fn parameters(
        &self,
        session: &Arc<Session>,
        vehicle: VehicleId,
        bus: u8,
        node: u8,
    ) -> CommandResult<Vec<NodeParameter>> {
        let mut parameters = Vec::new();

        for index in 0..MAX_PARAMETERS {
            let request = GetSetRequest::list(index as u16);
            let answer = self
                .ask(
                    session,
                    vehicle,
                    bus,
                    node,
                    &request.encode(),
                    (Kind::Response, DataType::ParameterGetSet),
                )
                .await?;

            let Some(read) = GetSetResponse::decode(&answer) else {
                return Err(CommandError::new(
                    CommandError::CODE_INTERNAL,
                    "a parameter could not be read",
                ));
            };

            // The standard says an empty name means there is no such parameter, which is how a
            // node ends its list.
            if read.name.is_empty() {
                break;
            }

            parameters.push(NodeParameter {
                name: read.name,
                value: read.value.describe(),
                default_value: read.default_value.describe(),
                min_value: read.min_value.describe(),
                max_value: read.max_value.describe(),
            });
        }

        Ok(parameters)
    }

    /// Writes one parameter, and reads back what the node stored.
    ///
    /// A node may clamp what it is given, so the answer is what it holds rather than what was asked
    /// for — which is the value the operator has to see.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the request, when the node does not answer, or
    /// when it answers with no name, which is how it says it has no such parameter.
    pub async fn write_parameter(
        &self,
        session: &Arc<Session>,
        vehicle: VehicleId,
        bus: u8,
        node: u8,
        name: &str,
        value: ParameterValue,
    ) -> CommandResult<NodeParameter> {
        let request = GetSetRequest::write(name, value);
        let answer = self
            .ask(
                session,
                vehicle,
                bus,
                node,
                &request.encode(),
                (Kind::Response, DataType::ParameterGetSet),
            )
            .await?;

        let Some(read) = GetSetResponse::decode(&answer) else {
            return Err(CommandError::new(
                CommandError::CODE_INTERNAL,
                "the node's answer could not be read",
            ));
        };

        if read.name.is_empty() {
            return Err(CommandError::new(
                CommandError::CODE_REJECTED,
                format!("the node has no parameter called {name}"),
            ));
        }

        Ok(NodeParameter {
            name: read.name,
            value: read.value.describe(),
            default_value: read.default_value.describe(),
            min_value: read.min_value.describe(),
            max_value: read.max_value.describe(),
        })
    }

    /// Asks a node to keep what it holds across a power cycle.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the request, or when the node does not answer.
    pub async fn save_parameters(
        &self,
        session: &Arc<Session>,
        vehicle: VehicleId,
        bus: u8,
        node: u8,
    ) -> CommandResult<()> {
        self.ask(
            session,
            vehicle,
            bus,
            node,
            &ExecuteOpcode::Save.encode(),
            (Kind::Response, DataType::ParameterExecuteOpcode),
        )
        .await?;

        Ok(())
    }

    /// Restarts a node.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the request, or when the node does not answer.
    pub async fn restart_node(
        &self,
        session: &Arc<Session>,
        vehicle: VehicleId,
        bus: u8,
        node: u8,
    ) -> CommandResult<()> {
        self.ask(
            session,
            vehicle,
            bus,
            node,
            &RestartNode.encode(),
            (Kind::Response, DataType::RestartNode),
        )
        .await?;

        Ok(())
    }

    /// Asks a node one thing and hands back the bytes it answered with.
    ///
    /// The frames are cut here, from the id the bus expects: the request is a service request from
    /// this station's own node id to the node being asked, at the nominal priority.
    async fn ask(
        &self,
        session: &Arc<Session>,
        vehicle: VehicleId,
        bus: u8,
        node: u8,
        payload: &[u8],
        expect: (Kind, DataType),
    ) -> CommandResult<Vec<u8>> {
        session
            .forward_can(vehicle, bus)
            .await
            .map_err(CommandError::from)?;

        let can_id = CanId::service(
            NOMINAL_PRIORITY,
            expect.1.id() as u8,
            true,
            node,
            STATION_NODE_ID,
        );

        let frames = encode(can_id, 0, payload, expect.1.signature());

        session
            .dronecan_call(vehicle, bus, node, &frames, expect, CALL_TIMEOUT)
            .await
            .map_err(CommandError::from)
    }

    /// What the station has asked, without reading the bus.
    #[must_use]
    pub fn status(&self) -> DronecanStatus {
        let inner = self.inner.lock();

        match inner {
            Ok(inner) => DronecanStatus {
                asked: inner.asked,
                error: inner.error.clone(),
            },
            Err(_poisoned) => DronecanStatus {
                asked: 0,
                error: Some("the state of the bus is locked".to_owned()),
            },
        }
    }
}

/// What the station has asked for its nodes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DronecanStatus {
    /// How many times the ask went out on this link.
    pub asked: usize,
    /// The last failure, when there was one.
    pub error: Option<String>,
}

/// What a value typed into the sheet is, read the way a node would read it.
///
/// A parameter's type is its name in the node's own model and is not carried by the read, so the
/// station reads what looks like a whole number as one, what looks like a number with a point as a
/// real, and anything else as text. The node is what decides in the end: its answer says what it
/// stored, and that is what the sheet shows after a write.
#[must_use]
pub fn value_of(text: &str) -> ParameterValue {
    let text = text.trim();

    if text.is_empty() {
        return ParameterValue::Empty;
    }

    if let Ok(value) = text.parse::<i64>() {
        return ParameterValue::Integer(value);
    }

    if let Ok(value) = text.parse::<f32>() {
        return ParameterValue::Real(value);
    }

    ParameterValue::Text(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    use mpcraft_link::{health_name, DronecanNodeStatus};
    use mpcraft_vehicle::VehicleId;

    /// A node as a status describes it, which is what the dictionary says has to be asked for again.
    fn node(health: u8) -> DronecanNode {
        let status = DronecanNodeStatus {
            time_usec: 1_700_000_000_000_000,
            uptime_sec: 54_321,
            health,
            mode: 0,
            sub_mode: 0,
            vendor_specific_status_code: 0,
        };

        DronecanNode::of(VehicleId::new(2, 40), &status)
    }

    #[test]
    fn the_first_reading_is_the_one_that_asks() {
        // Nothing has been reported yet, so the reading that opens the link is the one that asks —
        // the command answers with one message per node that is online.
        assert!(should_ask(0, &[]));
    }

    #[test]
    fn a_node_that_has_reported_without_saying_what_it_is_is_asked_for_once_more() {
        let missing = node(1);
        assert_eq!(health_name(missing.health), Some("WARNING"));
        assert!(missing.info_missing);

        assert!(should_ask(1, &[missing]));
    }

    #[test]
    fn the_rule_does_not_ask_a_third_time() {
        // A bus that will not answer is not answered by asking again: the sheet says which nodes are
        // still missing their information rather than putting a command on the wire for nothing.
        assert!(!should_ask(2, &[node(1)]));
        assert!(!should_ask(1, &[]));
    }

    #[test]
    fn a_node_that_has_said_what_it_is_is_not_asked_for_again() {
        let mut said = node(1);
        said.info_missing = false;

        assert!(!should_ask(1, &[said]));
    }
}
