//! Packet inspection: per-message-id traffic statistics.
//!
//! Mission Planner keeps this in `PacketInspector<T>`. MPCraft keeps the same capability but
//! exposes it as a flat, serialisable snapshot the UI can render directly.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Traffic statistics for a single message id.
#[derive(Debug, Clone, PartialEq)]
pub struct MessageStats {
    /// The message id.
    pub message_id: u32,
    /// The message name, if known.
    pub name: String,
    /// Total frames observed.
    pub count: u64,
    /// Total bytes observed, including headers and CRC.
    pub bytes: u64,
    /// Observed rate in frames per second.
    pub rate_hz: f64,
    /// Gap since the last frame of this type.
    pub last_seen: Option<Duration>,
}

/// Accumulates per-message-id traffic statistics.
#[derive(Debug, Default)]
pub struct PacketInspector {
    stats: HashMap<u32, MessageStats>,
    window_start: Option<Instant>,
    window_count: u64,
}

impl PacketInspector {
    /// Creates an empty inspector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
            window_start: None,
            window_count: 0,
        }
    }

    /// Records one observed frame.
    pub fn observe(&mut self, message_id: u32, name: &str, bytes: usize, now: Instant) {
        if self.window_start.is_none() {
            self.window_start = Some(now);
            self.window_count = 0;
        }
        self.window_count += 1;

        let entry = self
            .stats
            .entry(message_id)
            .or_insert_with(|| MessageStats {
                message_id,
                name: name.to_owned(),
                count: 0,
                bytes: 0,
                rate_hz: 0.0,
                last_seen: None,
            });

        if entry.name.is_empty() {
            entry.name = name.to_owned();
        }
        entry.count += 1;
        entry.bytes += bytes as u64;
        entry.last_seen = Some(Duration::ZERO);
    }

    /// Recomputes the observed rates over the elapsed window.
    ///
    /// Call this at the UI tick; it is what turns frame counts into frames per second.
    pub fn refresh_rates(&mut self, now: Instant) {
        let Some(start) = self.window_start else {
            return;
        };
        let elapsed = now.saturating_duration_since(start).as_secs_f64();
        if elapsed <= 0.0 {
            return;
        }

        let rate = self.window_count as f64 / elapsed;
        for stats in self.stats.values_mut() {
            stats.rate_hz = rate;
        }
        self.window_start = Some(now);
        self.window_count = 0;
    }

    /// Returns a snapshot of the statistics, sorted by message id.
    #[must_use]
    pub fn snapshot(&self) -> Vec<MessageStats> {
        let mut out: Vec<MessageStats> = self.stats.values().cloned().collect();
        out.sort_by_key(|s| s.message_id);
        out
    }

    /// Returns the statistics for one message id, if any.
    #[must_use]
    pub fn get(&self, message_id: u32) -> Option<&MessageStats> {
        self.stats.get(&message_id)
    }

    /// Removes all accumulated statistics.
    pub fn reset(&mut self) {
        self.stats.clear();
        self.window_start = None;
        self.window_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_frames_per_message() {
        let mut inspector = PacketInspector::new();
        let now = Instant::now();

        inspector.observe(0, "HEARTBEAT", 21, now);
        inspector.observe(0, "HEARTBEAT", 21, now);
        inspector.observe(30, "ATTITUDE", 28, now);

        let snapshot = inspector.snapshot();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0].message_id, 0);
        assert_eq!(snapshot[0].count, 2);
        assert_eq!(snapshot[0].bytes, 42);
    }

    #[test]
    fn rate_is_computed_over_the_window() {
        let mut inspector = PacketInspector::new();
        let start = Instant::now();

        for _ in 0..10 {
            inspector.observe(0, "HEARTBEAT", 21, start);
        }

        inspector.refresh_rates(start + Duration::from_secs(1));
        assert!(inspector.get(0).unwrap().rate_hz > 0.0);
    }
}
