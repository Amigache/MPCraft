//! Packet-loss accounting.
//!
//! MAVLink sequence numbers are eight bits and wrap, so a plain difference cannot distinguish
//! "one packet was reordered" from "one packet was lost". Mission Planner solves this with a
//! rolling mask of recently received sequence numbers, and MPCraft ports that algorithm
//! exactly rather than inventing a subtly different one.
//!
//! # The algorithm
//!
//! For each vehicle, a 64-bit mask records which of the last 64 sequence numbers have been
//! received. When a packet arrives:
//!
//! * `delta` is the forward distance from the last sequence number.
//! * `delta == 0` is a duplicate, which some 3D radios emit; it is ignored.
//! * A packet that arrives *behind* the newest one and fills a hole in the mask is a
//!   **reordered** packet, not a lost one. The loss count is decremented.
//! * Otherwise, `delta - 1` packets were lost between the two.

/// The number of sequence numbers tracked by the rolling mask.
const LATE_PACKET_WINDOW: u32 = 64;

/// The loss statistics of one vehicle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LossStats {
    /// The number of packets received.
    pub packets_received: u64,
    /// The number of packets lost.
    pub packets_lost: u64,
    /// The number of times a run of packets was lost.
    ///
    /// This counts *events*, not packets: one gap of three packets is one sync loss.
    pub sync_losses: u64,
    /// The last sequence number received.
    pub last_sequence: u8,
}

/// Tracks packet loss for one vehicle.
#[derive(Debug, Clone, Copy, Default)]
pub struct LossAccountant {
    /// The last sequence number seen.
    sequence: u8,
    /// The rolling mask of recently received sequence numbers.
    mask: u64,
    /// Whether any packet has been seen yet.
    started: bool,
    /// The accumulated statistics.
    stats: LossStats,
}

impl LossAccountant {
    /// Creates an accountant that has not yet seen a packet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The accumulated statistics.
    #[must_use]
    pub fn stats(&self) -> LossStats {
        self.stats
    }

    /// Records an arriving packet.
    ///
    /// `since_previous` tells the accountant whether the previous packet arrived recently.
    /// A late packet only fills a hole if the gap it fills was counted moments ago; after a
    /// pause, the sequence has wrapped through a real outage instead.
    pub fn observe(&mut self, sequence: u8, since_previous: bool) {
        if !self.started {
            self.started = true;
            self.sequence = sequence;
            self.stats.last_sequence = sequence;
            self.stats.packets_received += 1;
            return;
        }

        let delta = sequence.wrapping_sub(self.sequence) as u32;
        let packets_behind = 0x100 - delta;
        let late_bit = if packets_behind <= LATE_PACKET_WINDOW {
            1u64 << (packets_behind - 1)
        } else {
            0
        };

        if delta == 0 {
            // A duplicate sequence number. Some 3D radios emit these; they are not losses.
        } else if late_bit != 0 && (self.mask & late_bit) == 0 && since_previous {
            // A late packet filled a hole that had been counted as lost.
            self.mask |= late_bit;
            self.stats.packets_lost = self.stats.packets_lost.saturating_sub(1);
        } else {
            let num_lost = delta - 1;
            if num_lost != 0 {
                self.stats.sync_losses += 1;
                self.stats.packets_lost += u64::from(num_lost);
            }

            // Shift the mask by the distance travelled. A shift of 64 or more clears it.
            let mut mask = if delta < 64 { self.mask << delta } else { 0 };
            if delta <= LATE_PACKET_WINDOW {
                mask |= 1u64 << (delta - 1);
            }
            self.mask = mask;
            self.sequence = sequence;
        }

        self.stats.last_sequence = sequence;
        self.stats.packets_received += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_order_packets_are_not_losses() {
        let mut accountant = LossAccountant::new();

        for sequence in 0..10u8 {
            accountant.observe(sequence, true);
        }

        assert_eq!(accountant.stats().packets_received, 10);
        assert_eq!(accountant.stats().packets_lost, 0);
        assert_eq!(accountant.stats().sync_losses, 0);
    }

    #[test]
    fn a_gap_is_counted_as_lost_packets_and_one_sync_loss() {
        let mut accountant = LossAccountant::new();

        accountant.observe(0, true);
        accountant.observe(1, true);
        // Three packets are missing.
        accountant.observe(5, true);

        assert_eq!(accountant.stats().packets_lost, 3);
        assert_eq!(accountant.stats().sync_losses, 1);
    }

    #[test]
    fn a_reordered_packet_is_not_a_loss() {
        let mut accountant = LossAccountant::new();

        accountant.observe(0, true);
        accountant.observe(2, true); // 1 is missing
        assert_eq!(accountant.stats().packets_lost, 1);

        // The missing packet arrives late and fills the hole.
        accountant.observe(1, true);

        assert_eq!(accountant.stats().packets_lost, 0);
        assert_eq!(accountant.stats().packets_received, 3);
    }

    #[test]
    fn a_duplicate_sequence_number_is_ignored() {
        let mut accountant = LossAccountant::new();

        accountant.observe(7, true);
        accountant.observe(7, true);

        assert_eq!(accountant.stats().packets_received, 2);
        assert_eq!(accountant.stats().packets_lost, 0);
        assert_eq!(accountant.stats().sync_losses, 0);
    }

    #[test]
    fn the_sequence_number_wraps_cleanly() {
        let mut accountant = LossAccountant::new();

        for sequence in 250..=255u8 {
            accountant.observe(sequence, true);
        }
        accountant.observe(0, true);
        accountant.observe(1, true);

        assert_eq!(accountant.stats().packets_lost, 0);
        assert_eq!(accountant.stats().packets_received, 8);
    }

    #[test]
    fn a_late_packet_after_a_pause_does_not_cancel_the_loss() {
        let mut accountant = LossAccountant::new();

        accountant.observe(0, true);
        accountant.observe(2, true); // 1 is missing
        assert_eq!(accountant.stats().packets_lost, 1);

        // The late packet arrives long after the gap. The loss is not cancelled: after a
        // pause the sequence has wrapped through a real outage rather than being reordered.
        accountant.observe(1, false);

        assert!(
            accountant.stats().packets_lost >= 1,
            "the loss was wrongly cancelled"
        );
    }

    #[test]
    fn a_large_gap_is_counted_in_full() {
        let mut accountant = LossAccountant::new();

        accountant.observe(0, true);
        accountant.observe(100, true);

        assert_eq!(accountant.stats().packets_lost, 99);
        assert_eq!(accountant.stats().sync_losses, 1);
    }
}
