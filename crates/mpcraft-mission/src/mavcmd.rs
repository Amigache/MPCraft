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
//!
//! # Names and values
//!
//! A command travels on the wire as its `MAV_CMD` enumeration value, but the file formats
//! name it: a `.waypoints` file writes the value, a KML file writes the name.
//! [`command_from_value`], [`command_name`] and [`command_from_name`] translate between
//! the two, and all three go through the generated bindings so they cannot drift away
//! from the dialect.

use std::collections::HashMap;
use std::sync::OnceLock;

use num_traits::FromPrimitive;

use mpcraft_mavlink::MavCmd;
/// The default coordinate frame for a new mission item: `MAV_FRAME_GLOBAL_RELATIVE_ALT`.
///
/// The `MAV_FRAME_GLOBAL_RELATIVE_ALT_INT` synonym is deprecated and is not what a vehicle
/// expects.
pub const DEFAULT_FRAME: u32 = 3;

/// The default mission type: `MAV_MISSION_TYPE_MISSION`.
pub const DEFAULT_MISSION_TYPE: u32 = 0;

/// `MAV_CMD_NAV_WAYPOINT`.
pub const CMD_NAV_WAYPOINT: MavCmd = MavCmd::MAV_CMD_NAV_WAYPOINT;

/// `MAV_CMD_NAV_TAKEOFF`.
pub const CMD_NAV_TAKEOFF: MavCmd = MavCmd::MAV_CMD_NAV_TAKEOFF;

/// `MAV_CMD_NAV_LAND`.
pub const CMD_NAV_LAND: MavCmd = MavCmd::MAV_CMD_NAV_LAND;

/// `MAV_CMD_CONDITION_DELAY`.
pub const CMD_CONDITION_DELAY: MavCmd = MavCmd::MAV_CMD_CONDITION_DELAY;

/// `MAV_CMD_NAV_RALLY_POINT`.
pub const CMD_NAV_RALLY_POINT: MavCmd = MavCmd::MAV_CMD_NAV_RALLY_POINT;

/// `MAV_CMD_NAV_FENCE_RETURN_POINT`.
pub const CMD_FENCE_RETURN_POINT: MavCmd = MavCmd::MAV_CMD_NAV_FENCE_RETURN_POINT;

/// `MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION`.
pub const CMD_FENCE_VERTEX_INCLUSION: MavCmd = MavCmd::MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION;

/// `MAV_CMD_NAV_FENCE_POLYGON_VERTEX_EXCLUSION`.
pub const CMD_FENCE_VERTEX_EXCLUSION: MavCmd = MavCmd::MAV_CMD_NAV_FENCE_POLYGON_VERTEX_EXCLUSION;

/// `MAV_CMD_NAV_FENCE_CIRCLE_INCLUSION`.
pub const CMD_FENCE_CIRCLE_INCLUSION: MavCmd = MavCmd::MAV_CMD_NAV_FENCE_CIRCLE_INCLUSION;

/// `MAV_CMD_NAV_FENCE_CIRCLE_EXCLUSION`.
pub const CMD_FENCE_CIRCLE_EXCLUSION: MavCmd = MavCmd::MAV_CMD_NAV_FENCE_CIRCLE_EXCLUSION;

/// The role of each of a command's seven parameter slots.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CmdMetadata {
    /// The MAV_CMD value.
    pub command: MavCmd,
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

/// The roles of the vertices of a fence's polygon.
///
/// A vertex of the polygon is on the ground: it has a place, but no height.
const fn fence_vertex_roles() -> [ParamRole; 7] {
    [
        ParamRole::Empty,
        ParamRole::Empty,
        ParamRole::Empty,
        ParamRole::Other,
        ParamRole::Latitude,
        ParamRole::Longitude,
        ParamRole::Empty,
    ]
}

/// The roles of a circle of a fence.
///
/// The radius of the circle is the first parameter, in metres.
const fn fence_circle_roles() -> [ParamRole; 7] {
    [
        ParamRole::Distance,
        ParamRole::Empty,
        ParamRole::Empty,
        ParamRole::Other,
        ParamRole::Latitude,
        ParamRole::Longitude,
        ParamRole::Empty,
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
        command: MavCmd::MAV_CMD_NAV_LOITER_UNLIM,
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
        command: MavCmd::MAV_CMD_NAV_LOITER_TURNS,
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
        command: MavCmd::MAV_CMD_NAV_LOITER_TIME,
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
        command: CMD_NAV_RALLY_POINT,
        name: "Rally Point",
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
        command: CMD_FENCE_RETURN_POINT,
        name: "Fence Return Point",
        roles: fence_vertex_roles(),
        units: [
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::Degrees,
            ParamUnit::Degrees,
            ParamUnit::None,
        ],
    },
    CmdMetadata {
        command: CMD_FENCE_VERTEX_INCLUSION,
        name: "Fence Polygon Vertex",
        roles: fence_vertex_roles(),
        units: [
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::Degrees,
            ParamUnit::Degrees,
            ParamUnit::None,
        ],
    },
    CmdMetadata {
        command: CMD_FENCE_VERTEX_EXCLUSION,
        name: "Fence Polygon Vertex Exclusion",
        roles: fence_vertex_roles(),
        units: [
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::Degrees,
            ParamUnit::Degrees,
            ParamUnit::None,
        ],
    },
    CmdMetadata {
        command: CMD_FENCE_CIRCLE_INCLUSION,
        name: "Fence Circle Inclusion",
        roles: fence_circle_roles(),
        units: [
            ParamUnit::Metres,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::Degrees,
            ParamUnit::Degrees,
            ParamUnit::None,
        ],
    },
    CmdMetadata {
        command: CMD_FENCE_CIRCLE_EXCLUSION,
        name: "Fence Circle Exclusion",
        roles: fence_circle_roles(),
        units: [
            ParamUnit::Metres,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::None,
            ParamUnit::Degrees,
            ParamUnit::Degrees,
            ParamUnit::None,
        ],
    },
    CmdMetadata {
        command: MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
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
        command: MavCmd::MAV_CMD_CONDITION_CHANGE_ALT,
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
        command: MavCmd::MAV_CMD_DO_SET_SERVO,
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
pub fn metadata(command: MavCmd) -> Option<&'static CmdMetadata> {
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
/// use mpcraft_mavlink::MavCmd;
///
/// assert!(is_location_command(MavCmd::MAV_CMD_NAV_WAYPOINT));
/// assert!(!is_location_command(MavCmd::MAV_CMD_CONDITION_DELAY));
/// ```
#[must_use]
pub fn is_location_command(command: MavCmd) -> bool {
    metadata(command).is_some_and(|cmd| {
        cmd.roles[4] == ParamRole::Latitude && cmd.roles[5] == ParamRole::Longitude
    })
}

/// The largest value a `MAV_CMD` can take.
///
/// The command travels as a `uint16_t`, so the value space is bounded even though the
/// enumeration itself is sparse.
const MAX_COMMAND_VALUE: u32 = u16::MAX as u32;

/// The catalogue of command names, built on first use.
static COMMAND_NAMES: OnceLock<HashMap<String, MavCmd>> = OnceLock::new();

/// Returns the command with the given wire value, if the dialect defines one.
///
/// The value is the `MAV_CMD` enumeration value that travels in a mission item. A file
/// that names a value the dialect does not know is rejected rather than guessed at: an
/// unknown command is a command the vehicle would refuse to fly.
#[must_use]
pub fn command_from_value(value: u32) -> Option<MavCmd> {
    MavCmd::from_u32(value)
}

/// Returns the canonical name of a command, e.g. `MAV_CMD_NAV_WAYPOINT`.
///
/// The name is the one the MAVLink specification uses, which is what the file formats
/// carry: a plan that travels through a third-party tool still names its commands the
/// way the specification does.
#[must_use]
pub fn command_name(command: MavCmd) -> String {
    format!("{command:?}")
}

/// Returns the command with the given canonical name, if the dialect defines one.
///
/// The name is matched exactly, the way the specification spells it. A name the dialect
/// does not know is `None` — the caller decides whether that is an error or a default.
#[must_use]
pub fn command_from_name(name: &str) -> Option<MavCmd> {
    command_names().get(name).copied()
}

/// The catalogue of command names, indexed by name.
///
/// The dialect is the source of truth: the index is built by asking the bindings which
/// value every command has, so it cannot fall out of step with them the way a
/// hand-written table would. The scan is bounded by the wire type and paid once.
fn command_names() -> &'static HashMap<String, MavCmd> {
    COMMAND_NAMES.get_or_init(|| {
        let mut names = HashMap::new();
        for value in 0..=MAX_COMMAND_VALUE {
            if let Some(command) = MavCmd::from_u32(value) {
                names.insert(command_name(command), command);
            }
        }
        names
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
        // A command that is not in the table must never be assumed to carry a position.
        assert!(!is_location_command(MavCmd::MAV_CMD_NAV_SPLINE_WAYPOINT));
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
        // The enumeration is not ordered, so the identifiers are compared by their value.
        let mut ids: Vec<u32> = COMMANDS.iter().map(|c| c.command as u32).collect();
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

    #[test]
    fn a_command_is_found_by_its_wire_value() {
        assert_eq!(
            command_from_value(16),
            Some(MavCmd::MAV_CMD_NAV_WAYPOINT),
            "the wire value of a waypoint is 16"
        );
        assert_eq!(command_from_value(22), Some(MavCmd::MAV_CMD_NAV_TAKEOFF));
    }

    #[test]
    fn an_unknown_wire_value_is_not_a_command() {
        // 99 is not in the dialect: Mission Planner used to fold it onto the waypoint
        // command, but a command the specification does not define must not be guessed.
        assert_eq!(command_from_value(99), None);
    }

    #[test]
    fn a_command_is_found_by_its_name() {
        assert_eq!(
            command_from_name("MAV_CMD_NAV_WAYPOINT"),
            Some(MavCmd::MAV_CMD_NAV_WAYPOINT)
        );
        assert_eq!(
            command_from_name("MAV_CMD_NAV_TAKEOFF"),
            Some(MavCmd::MAV_CMD_NAV_TAKEOFF)
        );
        assert_eq!(command_from_name("WAYPOINT"), None, "the name is exact");
        assert_eq!(command_from_name("NOT_A_COMMAND"), None);
    }

    #[test]
    fn the_catalogue_covers_the_whole_value_space() {
        // The last command of the dialect is at 43005, so an index that stopped at the
        // commands a mission uses would miss it.
        assert_eq!(
            command_from_name("MAV_CMD_SET_HAGL"),
            Some(MavCmd::MAV_CMD_SET_HAGL)
        );
        assert_eq!(command_from_value(43005), Some(MavCmd::MAV_CMD_SET_HAGL));
    }

    #[test]
    fn the_name_of_a_command_is_the_one_the_specification_uses() {
        assert_eq!(command_name(CMD_NAV_WAYPOINT), "MAV_CMD_NAV_WAYPOINT");
        assert_eq!(command_name(CMD_NAV_TAKEOFF), "MAV_CMD_NAV_TAKEOFF");
    }

    #[test]
    fn every_command_in_the_metadata_table_has_a_name_and_a_value() {
        for cmd in COMMANDS {
            let name = command_name(cmd.command);
            assert_eq!(
                command_from_name(&name),
                Some(cmd.command),
                "{name} does not resolve back to its command"
            );
            assert_eq!(
                command_from_value(cmd.command as u32),
                Some(cmd.command),
                "the wire value of {name} does not resolve back to it"
            );
        }
    }
}
