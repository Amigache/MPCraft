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

use mpcraft_link::{DronecanNode, Session, VehicleId};
use serde::Serialize;

use crate::error::CommandResult;

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
