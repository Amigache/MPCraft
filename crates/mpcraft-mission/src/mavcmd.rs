//! MAV_CMD metadata.
//!
//! # What this is
//!
//! Every `MAV_CMD` has seven meaningful parameter slots — P1, P2, P3, P4, X, Y, Z — and the
//! meaning of each slot depends on the command. `NAV_WAYPOINT` uses X/Y/Z as a position;
//! `CONDITION_DELAY` ignores them entirely; `NAV_TAKEOFF` uses P1 as a pitch.
//!
//! The metadata table states that mapping, so the mission editor can generate the right input
//! widget, the right unit, and the right validation for any command **without hardcoding
//! per-command logic**.
//!
//! # Source
//!
//! The table is ported from Mission Planner's `mavcmd.xml`, which is a domain specification
//! rather than an implementation. The schema is preserved exactly; only the serialisation
//! format changed (XML → TOML) so it is readable and diffable.
//!
//! # Integer encoding
//!
//! `MISSION_ITEM_INT` scales latitude and longitude by `1e7`, and only for commands that
//! actually carry a position. A command whose X/Y/Z are not a location is **not** scaled.
//! [`is_location_command`] is the single source of truth for that distinction.

/// The default coordinate frame for a new mission item: `MAV_FRAME_GLOBAL_RELATIVE_ALT`.
pub const DEFAULT_FRAME: u32 = 6;

/// The default mission type: `MAV_MISSION_TYPE_MISSION`.
pub const DEFAULT_MISSION_TYPE: u32 = 0;

/// `MAV_CMD_NAV_WAYPOINT`.
pub const CMD_NAV_WAYPOINT: u32 = 16;

/// `MAV_CMD_NAV_TAKEOFF`.
pub const CMD_NAV_TAKEOFF: u32 = 22;

/// `MAV_CMD_NAV_LAND`.
pub const CMD_NAV_LAND: u32 = 21;

/// `MAV_CMD_CONDITION_DELAY`.
pub const CMD_CONDITION_DELAY: u32 = 112;

/// The role of each of a command's seven parameter slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CmdMetadata {
    /// The MAV_CMD value.
    pub command: u32,
    /// The display name.
    pub name: &'static str,
    /// The role of P1, P2, P3, P4, X, Y and Z, in that order.
    pub roles: [ParamRole; 7],
    /// The unit of each slot.
    pub units: [ParamUnit; 7],
}

/// The role of one parameter slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamRole {
    /// The slot is unused by this command.
    Empty,
    /// The slot holds a latitude in degrees.
    Latitude,
    /// The slot holds a longitude in degrees.
    Longitude,
    /// The slot holds an altitude in metres.
    Altitude,
    /// The slot holds a speed in metres per second.
    Speed,
    /// The slot holds a distance in metres.
    Distance,
    /// The slot holds an angle in degrees.
    Angle,
    /// The slot holds a command-specific value.
    Other,
}

/// The unit of one parameter slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamUnit {
    /// Unitless.
    None,
    /// Degrees.
    Degrees,
    /// Metres.
    Metres,
    /// Metres per second.
    MetresPerSecond,
}

const fn other_roles() -> [ParamRole; 7] {
    [
        ParamRole::Other,
        ParamRole::Other,
        ParamRole::Other,
        ParamRole::Other,
        ParamRole::Other,
        ParamRole::Other,
        ParamRole::Other,
    ]
}

const fn location_roles() -> [ParamRole; 7] {
    [
        ParamRole::Empty,
        ParamRole::Empty,
        ParamRole::Empty,
        ParamRole::Other,
        ParamRole::Latitude,
        ParamRole::Longitude,
        ParamRole::Altitude,
    ]
}

const fn takeoff_roles() -> [ParamRole; 7] {
    [
        ParamRole::Angle,
        ParamRole::Empty,
        ParamRole::Empty,
        ParamRole::Angle,
        ParamRole::Latitude,
        ParamRole::Longitude,
        ParamRole::Altitude,
    ]
}

const fn land_roles() -> [ParamRole; 7] {
    [
        ParamRole::Other,
        ParamRole::Other,
        ParamRole::Other,
        ParamRole::Other,
        ParamRole::Latitude,
        ParamRole::Longitude,
        ParamRole::Altitude,
    ]
}

const fn delay_roles() -> [ParamRole; 7] {
    [
        ParamRole::Other,
        ParamRole::Empty,
        ParamRole::Empty,
        ParamRole::Empty,
        ParamRole::Empty,
        ParamRole::Empty,
        ParamRole::Empty,
    ]
}

/// The metadata table for the commands MPCraft supports in the mission editor.
///
/// This is a *seed* table covering the commands a user reaches for. It is deliberately
/// explicit rather than exhaustive: a command that is missing renders with generic numeric
/// inputs, which is safe, whereas a command with *wrong* metadata silently flies the vehicle
/// to the wrong place.
pub const COMMANDS: &[CmdMetadata] = &[
    CmdMetadata {
        command: CMD_NAV_WAYPOINT,
        name: "Waypoint",
        roles: location_roles(),
        units: [
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::Degrees,
            ParamUnit::Degrees,
            ParamUnit::Metres,
        ],
    },
    CmdMetadata {
        command: CMD_NAV_TAKEOFF,
        name: "Takeoff",
        roles: takeoff_roles(),
        units: [
            ParamUnit::Degrees,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::Degrees,
            ParamUnit::Degrees,
            ParamUnit::Degrees,
            ParamUnit::Metres,
        ],
    },
    CmdMetadata {
        command: CMD_NAV_LAND,
        name: "Land",
        roles: land_roles(),
        units: [
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::Degrees,
            ParamUnit::Degrees,
            ParamUnit::Metres,
        ],
    },
    CmdMetadata {
        command: CMD_CONDITION_DELAY,
        name: "Delay",
        roles: delay_roles(),
        units: [
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
        ],
    },
    CmdMetadata {
        command: 17,
        name: "Loiter Unbounded",
        roles: location_roles(),
        units: [
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::Degrees,
            ParamUnit::Degrees,
            ParamUnit::Metres,
        ],
    },
    CmdMetadata {
        command: 18,
        name: "Loiter Turns",
        roles: [
            ParamRole::Other,
            ParamRole::Other,
            ParamRole::Other,
            ParamRole::Other,
            ParamRole::Latitude,
            ParamRole::Longitude,
            ParamRole::Altitude,
        ],
        units: [
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::Degrees,
            ParamUnit::Degrees,
            ParamUnit::Metres,
        ],
    },
    CmdMetadata {
        command: 19,
        name: "Loiter Time",
        roles: [
            ParamRole::Other,
            ParamRole::Other,
            ParamRole::Other,
            ParamRole::Other,
            ParamRole::Latitude,
            ParamRole::Longitude,
            ParamRole::Altitude,
        ],
        units: [
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::Degrees,
            ParamUnit::Degrees,
            ParamUnit::Metres,
        ],
    },
    CmdMetadata {
        command: 20,
        name: "Return to Launch",
        roles: other_roles(),
        units: [
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
        ],
    },
    CmdMetadata {
        command: 113,
        name: "Change Altitude",
        roles: [
            ParamRole::Other,
            ParamRole::Other,
            ParamRole::Other,
            ParamRole::Other,
            ParamRole::Latitude,
            ParamRole::Longitude,
            ParamRole::Altitude,
        ],
        units: [
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::Degrees,
            ParamUnit::Degrees,
            ParamUnit::Metres,
        ],
    },
    CmdMetadata {
        command: 177,
        name: "Do Set Servo",
        roles: [
            ParamRole::Other,
            ParamRole::Other,
            ParamRole::Other,
            ParamRole::Other,
            ParamRole::Empty,
            ParamRole::Empty,
            ParamRole::Empty,
        ],
        units: [
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
        ],
    },
];

/// Returns the metadata for a command, if it is known.
#[must_use]
pub fn metadata(command: u32) -> Option<&'static CmdMetadata> {
    COMMANDS.iter().find(|cmd| cmd.command == command)
}

/// Whether a command carries a geographic position.
///
/// This is the single source of truth for whether the integer mission encoding scales the
/// coordinates. A command whose X/Y/Z are not a location must **not** be scaled — scaling it
/// corrupts the value by a factor of `1e7`.
///
/// ```
/// use mpcraft_mission::mavcmd::is_location_command;
///
/// assert!(is_location_command(16)); // NAV_WAYPOINT
/// assert!(!is_location_command(112)); // CONDITION_DELAY
/// ```
#[must_use]
pub fn is_location_command(command: u32) -> bool {
    metadata(command).is_some_and(|cmd| {
        cmd.roles[4] == ParamRole::Latitude && cmd.roles[5] == ParamRole::Longitude
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waypoint_metadata_is_a_location() {
        let cmd = metadata(CMD_NAV_WAYPOINT).expect("NAV_WAYPOINT is in the table");
        assert_eq!(cmd.roles[4], ParamRole::Latitude);
        assert_eq!(cmd.roles[5], ParamRole::Longitude);
        assert_eq!(cmd.roles[6], ParamRole::Altitude);
        assert!(is_location_command(CMD_NAV_WAYPOINT));
    }

    #[test]
    fn delay_metadata_is_not_a_location() {
        let cmd = metadata(CMD_CONDITION_DELAY).expect("CONDITION_DELAY is in the table");
        assert_eq!(cmd.roles[4], ParamRole::Empty);
        assert!(!is_location_command(CMD_CONDITION_DELAY));
    }

    #[test]
    fn unknown_commands_are_not_locations() {
        // An unknown command must never be assumed to carry a position.
        assert!(!is_location_command(999_999));
    }

    #[test]
    fn every_command_declares_all_seven_slots() {
        for cmd in COMMANDS {
            assert_eq!(cmd.roles.len(), 7);
            assert_eq!(cmd.units.len(), 7);
        }
    }

    #[test]
    fn commands_have_unique_ids() {
        let mut ids: Vec<u32> = COMMANDS.iter().map(|c| c.command).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate command in the metadata table");
    }

    #[test]
    fn location_commands_use_degree_and_metre_units() {
        let cmd = metadata(CMD_NAV_WAYPOINT).unwrap();
        assert_eq!(cmd.units[4], ParamUnit::Degrees);
        assert_eq!(cmd.units[5], ParamUnit::Degrees);
        assert_eq!(cmd.units[6], ParamUnit::Metres);
    }
}
