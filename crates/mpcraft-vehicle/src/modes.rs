//! The flight modes a vehicle can be in.
//!
//! # The numbering
//!
//! ArduPilot identifies a mode by a number, and the number depends on the family of the
//! vehicle: a copter's `LOITER` is 5, and a plane's is 7. The table below is the
//! numbering the rotorcraft families share, which is the one a ground station shows by
//! default.
//!
//! The authoritative list lives in the autopilot's own parameter metadata, and a ground
//! station that knows the family of the vehicle it is talking to should prefer it. What
//! is here is the common table, so that the interface has something to show before the
//! metadata has been read.

/// A flight mode: the number the autopilot uses and the name it calls it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeEntry {
    /// The custom mode value.
    pub value: u32,
    /// The name of the mode.
    pub name: &'static str,
}

/// The flight modes the rotorcraft families share.
pub const COMMON_MODES: &[ModeEntry] = &[
    ModeEntry {
        value: 0,
        name: "STABILIZE",
    },
    ModeEntry {
        value: 1,
        name: "ACRO",
    },
    ModeEntry {
        value: 2,
        name: "ALT_HOLD",
    },
    ModeEntry {
        value: 3,
        name: "AUTO",
    },
    ModeEntry {
        value: 4,
        name: "GUIDED",
    },
    ModeEntry {
        value: 5,
        name: "LOITER",
    },
    ModeEntry {
        value: 6,
        name: "RTL",
    },
    ModeEntry {
        value: 7,
        name: "CIRCLE",
    },
    ModeEntry {
        value: 8,
        name: "LAND",
    },
    ModeEntry {
        value: 9,
        name: "FRTL",
    },
    ModeEntry {
        value: 10,
        name: "OFFBOARD",
    },
    ModeEntry {
        value: 11,
        name: "DRIFT",
    },
    ModeEntry {
        value: 13,
        name: "SPORT",
    },
    ModeEntry {
        value: 14,
        name: "EZLAND",
    },
    ModeEntry {
        value: 15,
        name: "THROW",
    },
    ModeEntry {
        value: 16,
        name: "SMART_RTL",
    },
    ModeEntry {
        value: 17,
        name: "MANUAL",
    },
    ModeEntry {
        value: 18,
        name: "TRAINING",
    },
];

/// The name of a mode, when the table knows it.
///
/// # Examples
///
/// ```
/// use mpcraft_vehicle::modes;
///
/// assert_eq!(modes::mode_name(4), Some("GUIDED"));
/// assert_eq!(modes::mode_name(9999), None);
/// ```
#[must_use]
pub fn mode_name(value: u32) -> Option<&'static str> {
    COMMON_MODES
        .iter()
        .find(|entry| entry.value == value)
        .map(|entry| entry.name)
}

/// The number of a mode, when the table knows it.
///
/// # Examples
///
/// ```
/// use mpcraft_vehicle::modes;
///
/// assert_eq!(modes::mode_value("RTL"), Some(6));
/// assert_eq!(modes::mode_value("NOT_A_MODE"), None);
/// ```
#[must_use]
pub fn mode_value(name: &str) -> Option<u32> {
    COMMON_MODES
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(name.trim()))
        .map(|entry| entry.value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mode_is_found_by_its_name_and_by_its_number() {
        assert_eq!(mode_name(4), Some("GUIDED"));
        assert_eq!(mode_value("GUIDED"), Some(4));
        assert_eq!(mode_value("guided"), Some(4));
    }

    #[test]
    fn an_unknown_mode_is_not_invented() {
        assert_eq!(mode_name(9999), None);
        assert_eq!(mode_value("NOT_A_MODE"), None);
    }

    #[test]
    fn the_numbers_are_the_ones_the_autopilot_uses() {
        // The numbers are the contract with the autopilot: they are not ours to choose.
        assert_eq!(mode_value("STABILIZE"), Some(0));
        assert_eq!(mode_value("AUTO"), Some(3));
        assert_eq!(mode_value("RTL"), Some(6));
        assert_eq!(mode_value("LAND"), Some(8));
    }
}
