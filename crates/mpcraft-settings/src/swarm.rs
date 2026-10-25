//! The formation as the operator wrote it.
//!
//! The places are the numbers an operator gives — metres across the direction of travel, metres
//! along it, metres above the leader — and they are worth keeping between two runs for the same
//! reason the connection profiles are: a fleet is set up once and flown many times, and the slots
//! an operator worked out are the slots they will use again.
//!
//! A vehicle is kept here as the name the link uses for it (`1:2`), because this crate does not
//! know what a vehicle is: it keeps what was written. Whether a name can be read as a vehicle is
//! decided by the layer that speaks the protocol, and a name that cannot be read is left out and
//! said there — the same rule a secret that is not hexadecimal follows in the keyring, where a
//! misspelled key is not guessed at.
//!
//! Whether the loop is run is kept with it, apart from the slots, so that the file says two
//! different things: what the formation looks like, and whether anybody decided to run it. A file
//! with slots and no decision is a formation that is described and not run, which is the state a
//! fresh installation is in.

use serde::{Deserialize, Serialize};

/// One vehicle's place in the formation, as the file writes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct SwarmSlot {
    /// The vehicle the slot belongs to, named the way the link names it: `system:component`.
    pub vehicle: String,

    /// Metres across the direction of travel, positive to the right.
    #[serde(default)]
    pub x: f64,

    /// Metres along the direction of travel, positive ahead.
    #[serde(default)]
    pub y: f64,

    /// Metres above the leader.
    #[serde(default)]
    pub z: f64,
}

impl SwarmSlot {
    /// A slot at the leader's own place, for the vehicle named.
    #[must_use]
    pub fn new(vehicle: impl Into<String>) -> Self {
        Self {
            vehicle: vehicle.into(),
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    /// A slot with the numbers the operator gave.
    #[must_use]
    pub fn with_offset(vehicle: impl Into<String>, x: f64, y: f64, z: f64) -> Self {
        Self {
            vehicle: vehicle.into(),
            x,
            y,
            z,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slot_written_by_hand_is_read() {
        // The hand: the numbers are read as they were written, in metres, with nothing scaled
        // and nothing rounded.
        let slot: SwarmSlot =
            toml::from_str("vehicle = \"2:1\"\nx = 0.0\ny = 60.0\nz = 5.0\n").unwrap();

        assert_eq!(slot.vehicle, "2:1");
        assert_eq!(slot.x, 0.0);
        assert_eq!(slot.y, 60.0);
        assert_eq!(slot.z, 5.0);
    }

    #[test]
    fn a_slot_that_says_only_the_vehicle_is_the_leader_s_place() {
        // A person writing the file by hand does not write zeroes for a slot that is the
        // leader's own place, so the numbers have to be optional and mean what their absence
        // says: nothing across, nothing ahead, nothing above.
        let slot: SwarmSlot = toml::from_str("vehicle = \"3:1\"\n").unwrap();

        assert_eq!(slot.vehicle, "3:1");
        assert_eq!((slot.x, slot.y, slot.z), (0.0, 0.0, 0.0));
    }

    #[test]
    fn a_misspelled_key_is_rejected() {
        let error = toml::from_str::<SwarmSlot>("vehicle = \"2:1\"\nheigth = 5.0\n").unwrap_err();

        assert!(error.to_string().contains("heigth"), "{error}");
    }

    #[test]
    fn a_number_that_is_not_a_number_is_refused() {
        let error = toml::from_str::<SwarmSlot>("vehicle = \"2:1\"\ny = \"60\"\n").unwrap_err();

        assert!(error.to_string().contains("y"), "{error}");
    }
}
