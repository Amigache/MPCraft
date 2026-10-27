//! The surface a plugin is plugged into.
//!
//! A plugin does not get the link, the vehicle, or the settings. It gets this: a small number of
//! things it may ask for, each one behind a capability that was granted when it was loaded. The
//! shapes are numbers and names, never protocol messages, so that a plugin cannot reach a wire
//! message directly and the layer above stays free of protocol knowledge.
//!
//! Every call is short. Nothing here waits for the vehicle: an answer that has to come back from
//! the wire is a transaction, and a transaction inside a sandbox would hold the link while a piece
//! of somebody else's code sits on it.

use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use crate::error::PluginError;

/// One thing the vehicle last said.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TelemetryField {
    /// Height above the ground station, in metres.
    Altitude,

    /// Speed over the ground, in metres per second.
    GroundSpeed,

    /// Which way the vehicle is pointing, in degrees.
    Heading,

    /// The pack the vehicle is running on, in volts.
    BatteryVolts,

    /// How much of that pack is left, as a percentage.
    BatteryRemaining,

    /// How far the vehicle is from where it started, in metres.
    DistanceToHome,

    /// How many satellites it can see.
    Satellites,
}

impl TelemetryField {
    /// The number a plugin passes for a field.
    ///
    /// A number that is not one of these is not guessed at: the plugin gets nothing and can see
    /// that it asked wrong.
    #[must_use]
    pub fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Altitude),
            1 => Some(Self::GroundSpeed),
            2 => Some(Self::Heading),
            3 => Some(Self::BatteryVolts),
            4 => Some(Self::BatteryRemaining),
            5 => Some(Self::DistanceToHome),
            6 => Some(Self::Satellites),
            _ => None,
        }
    }

    /// The number a plugin passes for this field.
    #[must_use]
    pub fn index(self) -> i32 {
        match self {
            Self::Altitude => 0,
            Self::GroundSpeed => 1,
            Self::Heading => 2,
            Self::BatteryVolts => 3,
            Self::BatteryRemaining => 4,
            Self::DistanceToHome => 5,
            Self::Satellites => 6,
        }
    }
}

/// A command the plugin asks for. It is queued rather than run here, because the sandbox must not
/// be able to hold the link while it waits for an answer.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandIntent {
    /// The command, as the protocol numbers it.
    pub command: u32,

    /// The seven parameters of a command.
    pub params: [f64; 7],

    /// The integer parameters, which are used by commands that carry a place or a count.
    pub x: i32,
    pub y: i32,
    pub z: f32,
}

impl CommandIntent {
    /// A command with nothing in its parameters.
    #[must_use]
    pub fn new(command: u32) -> Self {
        Self {
            command,
            params: [0.0; 7],
            x: 0,
            y: 0,
            z: 0.0,
        }
    }
}

/// A parameter the plugin asks to be changed, queued for the same reason a command is: the sandbox
/// must not be able to hold the link while it waits for the vehicle to echo what it stored.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamIntent {
    /// The name the vehicle knows the parameter by.
    pub name: String,

    /// The value the plugin wants. What the vehicle stores can be another number, and the read
    /// afterwards shows what the vehicle has rather than what was asked for.
    pub value: f64,
}

/// What the sandbox can be given to talk to.
///
/// The application implements this, and the plugin crate never knows what it is talking to. That is
/// what keeps the layering: the sandbox is below the application, so it may be handed these shapes
/// but not the session itself.
pub trait HostApi: Debug + Send + Sync {
    /// What the vehicle last said about one thing, or nothing when the plugin asked for something
    /// that is not a field.
    fn telemetry(&self, field: TelemetryField) -> Option<f64>;

    /// A command the plugin asks for, when it was given commands.
    fn ask(&self, intent: CommandIntent) -> Result<(), PluginError>;

    /// How many items the plan on the vehicle holds, when the plugin was given the plan.
    fn mission_count(&self) -> u32;

    /// The sequence number of the item the vehicle is executing, or nothing when the vehicle has not
    /// said where it is in the plan — a place in the plan the reader invents is not a place the vehicle
    /// is at.
    fn mission_current(&self) -> Option<u32>;

    /// What the vehicle last said about a parameter, or nothing when it has not reported one with
    /// that name — a value the plugin invents is not a value the vehicle has.
    fn param(&self, name: &str) -> Option<f64>;

    /// A parameter change the plugin asks for, when it was given that as well.
    fn ask_param(&self, intent: ParamIntent) -> Result<(), PluginError>;

    /// Something to show the operator, when the plugin was given that as well.
    fn note(&self, text: &str) -> Result<(), PluginError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_and_its_number_are_the_same_thing_both_ways_round() {
        for index in 0..7 {
            let field = TelemetryField::from_index(index).expect("a known field");
            assert_eq!(field.index(), index);
        }
    }

    #[test]
    fn a_number_that_is_not_a_field_is_not_guessed_at() {
        assert_eq!(TelemetryField::from_index(9), None);
    }

    #[test]
    fn a_command_with_nothing_in_it_still_has_the_seven_parameters() {
        let intent = CommandIntent::new(208);

        assert_eq!(intent.command, 208);
        assert_eq!(intent.params, [0.0; 7]);
    }
}
