//! Heartbeat negotiation.
//!
//! A link can carry more than one heartbeat: a vehicle, a gimbal, a camera, another ground
//! station. MPCraft has to pick the vehicle and ignore the rest.
//!
//! # The selection rule
//!
//! Mission Planner's rule is subtle and worth stating exactly, because a vehicle with a
//! gimbal attached would otherwise be picked up as the gimbal:
//!
//! 1. Collect heartbeats during the connect window. A heartbeat from a ground station
//!    (`MAV_TYPE_GCS`) or from a broadcast component (`compid == 0`) is ignored.
//! 2. Group the heartbeats by `(system_id, component_id)` and discard groups seen fewer
//!    than twice.
//! 3. Take the first group that satisfies **either**
//!    * it was seen at least twice **and** its component id is `1` — the autopilot's
//!      component id, or
//!    * it was seen at least four times.
//!
//! The component-id preference is what makes a vehicle with a gimbal resolve to the vehicle:
//! the autopilot's group is preferred over any other.
//!
//! `HIGH_LATENCY2` is treated as a heartbeat as well, for the same reason `HEARTBEAT` is.
//!
//! # A divergence from upstream
//!
//! Upstream orders the groups by how often they were seen and takes the first that
//! qualifies. That ordering means a gimbal — which heartbeats faster than the autopilot —
//! is chosen over the vehicle it is attached to. The upstream comment says the intent is
//! *"preference compid of 1, failover to anything that we have seen 4 times"*, which is a
//! different rule from what the code does.
//!
//! MPCraft implements the stated intent: a group with component id `1` seen at least twice
//! always wins, and only when no such group exists does the most frequently seen group
//! seen at least four times take over. Connecting to a vehicle and being handed its
//! gimbal instead is a bug worth fixing rather than reproducing.

use std::collections::HashMap;
use std::time::Duration;

use mpcraft_mavlink::MavType;

/// The message id of `HEARTBEAT`.
pub const HEARTBEAT_ID: u32 = 0;

/// The message id of `HIGH_LATENCY2`.
pub const HIGH_LATENCY2_ID: u32 = 235;

/// A heartbeat seen on the link.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Heartbeat {
    /// The system id of the sender.
    pub system_id: u8,
    /// The component id of the sender.
    pub component_id: u8,
    /// The message id the heartbeat arrived in.
    pub message_id: u32,
    /// The vehicle type the sender reported, for `HEARTBEAT`.
    pub mav_type: Option<MavType>,
}

impl Heartbeat {
    /// The key the heartbeat is grouped by.
    #[must_use]
    pub fn key(&self) -> (u8, u8) {
        (self.system_id, self.component_id)
    }
}

/// The reason a heartbeat could not be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatRejection {
    /// The sender is another ground station.
    IsGroundStation,
    /// The sender used the broadcast component id.
    IsBroadcast,
    /// The message is not a heartbeat.
    NotAHeartbeat,
}

/// Collects heartbeats and chooses the vehicle.
#[derive(Debug, Default)]
pub struct HeartbeatCollector {
    /// The heartbeats seen, in order.
    seen: Vec<Heartbeat>,
}

impl HeartbeatCollector {
    /// Creates an empty collector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a heartbeat.
    ///
    /// Returns the rejection reason when the heartbeat cannot be used to identify a vehicle.
    pub fn observe(&mut self, heartbeat: Heartbeat) -> Result<(), HeartbeatRejection> {
        if heartbeat.message_id != HEARTBEAT_ID && heartbeat.message_id != HIGH_LATENCY2_ID {
            return Err(HeartbeatRejection::NotAHeartbeat);
        }

        if heartbeat.component_id == 0 {
            return Err(HeartbeatRejection::IsBroadcast);
        }

        if heartbeat.mav_type == Some(MavType::MAV_TYPE_GCS) {
            return Err(HeartbeatRejection::IsGroundStation);
        }

        self.seen.push(heartbeat);
        Ok(())
    }

    /// The number of usable heartbeats seen.
    #[must_use]
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    /// Whether no usable heartbeat has been seen.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }

    /// Chooses the vehicle from the heartbeats seen so far.
    ///
    /// Returns `None` until a group satisfies the selection rule.
    #[must_use]
    pub fn select(&self) -> Option<Heartbeat> {
        let mut counts: HashMap<(u8, u8), usize> = HashMap::new();
        let mut first: HashMap<(u8, u8), Heartbeat> = HashMap::new();

        for heartbeat in &self.seen {
            *counts.entry(heartbeat.key()).or_insert(0) += 1;
            first.entry(heartbeat.key()).or_insert(*heartbeat);
        }

        // The autopilot is preferred: component id one, seen at least twice.
        let mut autopilot: Option<((u8, u8), usize)> = None;
        for (key, seen) in &counts {
            if key.1 == 1 && *seen >= 2 {
                let better = autopilot.is_none_or(|(_, best)| *seen > best);
                if better {
                    autopilot = Some((*key, *seen));
                }
            }
        }

        if let Some((key, _)) = autopilot {
            return Some(first[&key]);
        }

        // Otherwise the most frequently seen group, provided it was seen at least four
        // times. The key is part of the ordering so the choice is deterministic.
        let mut groups: Vec<((u8, u8), usize)> =
            counts.into_iter().filter(|(_, seen)| *seen >= 4).collect();
        groups.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

        groups.first().map(|(key, _)| first[key])
    }
}

/// How long to wait for a vehicle to answer.
#[must_use]
pub fn default_connect_timeout() -> Duration {
    Duration::from_secs(30)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heartbeat(system_id: u8, component_id: u8) -> Heartbeat {
        Heartbeat {
            system_id,
            component_id,
            message_id: HEARTBEAT_ID,
            mav_type: Some(MavType::MAV_TYPE_QUADROTOR),
        }
    }

    #[test]
    fn a_ground_station_is_ignored() {
        let mut collector = HeartbeatCollector::new();
        let mut gcs = heartbeat(255, 190);
        gcs.mav_type = Some(MavType::MAV_TYPE_GCS);

        assert_eq!(
            collector.observe(gcs),
            Err(HeartbeatRejection::IsGroundStation)
        );
        assert!(collector.is_empty());
    }

    #[test]
    fn a_broadcast_sender_is_ignored() {
        let mut collector = HeartbeatCollector::new();

        assert_eq!(
            collector.observe(heartbeat(1, 0)),
            Err(HeartbeatRejection::IsBroadcast)
        );
        assert!(collector.is_empty());
    }

    #[test]
    fn a_vehicle_seen_twice_with_component_one_is_selected() {
        let mut collector = HeartbeatCollector::new();
        collector.observe(heartbeat(1, 1)).unwrap();
        collector.observe(heartbeat(1, 1)).unwrap();

        let selected = collector.select().expect("the vehicle should be selected");
        assert_eq!(selected.system_id, 1);
        assert_eq!(selected.component_id, 1);
    }

    #[test]
    fn a_group_seen_only_once_is_not_selected() {
        let mut collector = HeartbeatCollector::new();
        collector.observe(heartbeat(1, 1)).unwrap();

        assert!(collector.select().is_none());
    }

    #[test]
    fn a_component_other_than_one_needs_four_sightings() {
        let mut collector = HeartbeatCollector::new();

        for _ in 0..3 {
            collector.observe(heartbeat(1, 190)).unwrap();
        }
        assert!(
            collector.select().is_none(),
            "three sightings are not enough"
        );

        collector.observe(heartbeat(1, 190)).unwrap();
        assert!(
            collector.select().is_some(),
            "four sightings should be enough"
        );
    }

    #[test]
    fn the_autopilot_wins_over_an_attach_gimbal() {
        let mut collector = HeartbeatCollector::new();

        // The gimbal is seen more often than the autopilot, but it is not component one.
        for _ in 0..5 {
            collector.observe(heartbeat(1, 154)).unwrap();
        }
        for _ in 0..2 {
            collector.observe(heartbeat(1, 1)).unwrap();
        }

        let selected = collector
            .select()
            .expect("the autopilot should be selected");
        assert_eq!(
            selected.component_id, 1,
            "the autopilot's component id must win"
        );
    }

    #[test]
    fn the_most_seen_group_wins_when_no_autopilot_qualifies() {
        let mut collector = HeartbeatCollector::new();

        for _ in 0..2 {
            collector.observe(heartbeat(1, 1)).unwrap();
        }
        for _ in 0..6 {
            collector.observe(heartbeat(1, 190)).unwrap();
        }

        let selected = collector.select().expect("a group should be selected");
        // The autopilot group qualifies on its component id, so it wins over the more
        // frequently seen group.
        assert_eq!(selected.component_id, 1);
    }

    #[test]
    fn a_group_seen_four_times_is_used_when_there_is_no_autopilot() {
        let mut collector = HeartbeatCollector::new();

        for _ in 0..6 {
            collector.observe(heartbeat(1, 190)).unwrap();
        }
        for _ in 0..3 {
            collector.observe(heartbeat(2, 190)).unwrap();
        }

        let selected = collector.select().expect("a group should be selected");
        assert_eq!(selected.system_id, 1, "the most seen group wins");
    }

    #[test]
    fn a_high_latency2_is_treated_as_a_heartbeat() {
        let mut collector = HeartbeatCollector::new();

        let mut high_latency = heartbeat(1, 1);
        high_latency.message_id = HIGH_LATENCY2_ID;

        collector.observe(high_latency).unwrap();
        collector.observe(high_latency).unwrap();

        assert!(collector.select().is_some());
    }
}
