//! Vehicle commands.
//!
//! The protocol is documented at <https://mavlink.io/en/services/command.html>. A command
//! is a `COMMAND_LONG` or a `COMMAND_INT` message, and the vehicle answers with a
//! `COMMAND_ACK` that carries the outcome.
//!
//! # The acknowledgement is the answer
//!
//! Upstream's `doCommand` returns a `bool`, and the caller cannot tell a refused command
//! from a vehicle that never answered. MPCraft keeps the two apart: a refusal is an error
//! that carries the [`MavResult`] the vehicle reported, and silence is a timeout.
//!
//! ```text
//! GCS  ────COMMAND_LONG────▶  vehicle
//! GCS  ◀────COMMAND_ACK────  vehicle      (the result the vehicle reported)
//! ```
//!
//! # An acknowledgement for another command is ignored
//!
//! A link can carry several vehicles, and a vehicle can be executing a command that
//! another ground station sent. An acknowledgement whose command is not the one that was
//! sent is therefore **ignored**, and the transaction keeps waiting — the same choice
//! upstream makes at `MAVLinkInterface.cs:2805`. The acknowledgement is matched on the
//! sending vehicle as well as on the command, so a transaction never steals the answer of
//! another vehicle.
//!
//! # `MAV_RESULT_IN_PROGRESS`
//!
//! A command that takes a while — a calibration, a reboot — is acknowledged with
//! `MAV_RESULT_IN_PROGRESS` first, and with the final result later. The transaction waits
//! for the final result; the interim one is not an answer.

use std::time::Duration;

// `SET_MODE` is deprecated in favour of `MAV_CMD_DO_SET_MODE`, but the legacy message
// carries the custom mode exactly — as a `u32`, without the float rounding the command
// suffers — and every autopilot still understands it. Both are sent.
#[allow(deprecated)]
use mpcraft_mavlink::dialect::messages::{
    PositionTargetTypemask, COMMAND_INT_DATA, COMMAND_LONG_DATA, MANUAL_CONTROL_DATA,
    RC_CHANNELS_OVERRIDE_DATA, SET_MODE_DATA, SET_POSITION_TARGET_GLOBAL_INT_DATA,
};
use mpcraft_mavlink::{MavCmd, MavFrame, MavMessage, MavModeFlag, MavResult};

use crate::error::LinkError;
use crate::session::{ReceivedMessage, Session};
use crate::vehicle::VehicleId;

/// The message id of `COMMAND_LONG`.
pub const COMMAND_LONG_ID: u32 = 76;

/// The message id of `COMMAND_INT`.
pub const COMMAND_INT_ID: u32 = 75;

/// The message id of `COMMAND_ACK`.
pub const COMMAND_ACK_ID: u32 = 77;

/// The message id of `SET_MODE`.
pub const SET_MODE_ID: u32 = 11;

/// The message id of `SET_POSITION_TARGET_GLOBAL_INT`.
pub const SET_POSITION_TARGET_ID: u32 = 86;

/// The message id of `RC_CHANNELS_OVERRIDE`.
pub const RC_CHANNELS_OVERRIDE_ID: u32 = 70;

/// The message id of `MANUAL_CONTROL`.
pub const MANUAL_CONTROL_ID: u32 = 69;

/// How long to wait for the acknowledgement of a command.
///
/// Upstream waits two seconds for a command to be acknowledged.
pub const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(2);

/// How long to wait for the acknowledgement of an arming command.
///
/// Arming can trigger a compass or accelerometer calibration, and the vehicle does not
/// answer until that is done. Upstream waits ten seconds for this one command.
pub const DEFAULT_ARM_TIMEOUT: Duration = Duration::from_secs(10);

/// The reasons a command can fail.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandError {
    /// The vehicle understood the command and refused it.
    ///
    /// The result says why: `MAV_RESULT_DENIED` for a command that is not allowed right
    /// now, `MAV_RESULT_UNSUPPORTED` for one the vehicle does not implement.
    Refused {
        /// The command that was sent.
        command: MavCmd,
        /// The result the vehicle reported.
        result: MavResult,
    },
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused { command, result } => {
                write!(f, "the vehicle refused {command:?}: {result:?}")
            }
        }
    }
}

impl std::error::Error for CommandError {}

/// The command operations of a session.
impl Session {
    /// Arms a vehicle.
    ///
    /// The vehicle refuses the command when it is not ready to fly — no GPS lock, a
    /// failed pre-arm check — and says so in the acknowledgement.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command or does not answer in time.
    pub async fn arm(&self, vehicle: VehicleId, timeout: Duration) -> Result<(), LinkError> {
        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
            [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            timeout,
        )
        .await
        .map(|_| ())
    }

    /// Disarms a vehicle.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command or does not answer in time.
    pub async fn disarm(&self, vehicle: VehicleId, timeout: Duration) -> Result<(), LinkError> {
        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            timeout,
        )
        .await
        .map(|_| ())
    }

    /// Takes off to the given altitude.
    ///
    /// The altitude is in metres, relative to the home position.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command or does not answer in time.
    pub async fn takeoff(
        &self,
        vehicle: VehicleId,
        altitude: f32,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_NAV_TAKEOFF,
            [altitude, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            timeout,
        )
        .await
        .map(|_| ())
    }

    /// Changes the flight mode of a vehicle.
    ///
    /// Two messages go out, and both are needed:
    ///
    /// * `SET_MODE` carries the custom mode exactly, as the legacy protocol requires.
    /// * `COMMAND_LONG` with `MAV_CMD_DO_SET_MODE` is what the vehicle **acknowledges** —
    ///   the legacy message has no answer of its own.
    ///
    /// Upstream sends the same two messages (`MAVLinkInterface.cs:4620`).
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command or does not answer in time.
    pub async fn set_mode(
        &self,
        vehicle: VehicleId,
        custom_mode: u32,
        base_mode: MavModeFlag,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        #[allow(deprecated)]
        self.write_message(&MavMessage::SET_MODE(SET_MODE_DATA {
            custom_mode,
            target_system: vehicle.system_id,
            base_mode,
        }))
        .await?;

        // The custom mode travels as a float parameter, which is what the protocol
        // allows; the `SET_MODE` message above carries the exact value.
        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_DO_SET_MODE,
            [
                base_mode.bits() as f32,
                custom_mode as f32,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ],
            timeout,
        )
        .await
        .map(|_| ())
    }

    /// Tells the vehicle to go to a place.
    ///
    /// This is the guided mode: the vehicle flies to the place it is told and stays
    /// there. The place is given in the integer form of the protocol, so that the
    /// coordinates do not lose their precision.
    ///
    /// The mask says what the vehicle is to change: the three speeds and the three
    /// accelerations are left alone, so that the vehicle keeps flying as it was flying
    /// and only the place changes.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the message.
    pub async fn set_position_target(
        &self,
        vehicle: VehicleId,
        latitude: f64,
        longitude: f64,
        altitude: f32,
    ) -> Result<(), LinkError> {
        let message =
            MavMessage::SET_POSITION_TARGET_GLOBAL_INT(SET_POSITION_TARGET_GLOBAL_INT_DATA {
                time_boot_ms: 0,
                // The integer form scales the coordinates by 1e7.
                lat_int: (latitude * 1e7).round() as i32,
                lon_int: (longitude * 1e7).round() as i32,
                alt: altitude,
                vx: 0.0,
                vy: 0.0,
                vz: 0.0,
                afx: 0.0,
                afy: 0.0,
                afz: 0.0,
                yaw: 0.0,
                yaw_rate: 0.0,
                // The position and the height are what changes; everything else is left
                // as the vehicle has it.
                type_mask: PositionTargetTypemask::POSITION_TARGET_TYPEMASK_VX_IGNORE
                    | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_VY_IGNORE
                    | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_VZ_IGNORE
                    | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_AX_IGNORE
                    | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_AY_IGNORE
                    | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_AZ_IGNORE
                    | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_YAW_IGNORE
                    | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_YAW_RATE_IGNORE,
                target_system: vehicle.system_id,
                target_component: vehicle.component_id,
                coordinate_frame: MavFrame::MAV_FRAME_GLOBAL,
            });

        self.write_message(&message).await
    }

    /// Runs the motors of the vehicle, to see whether they turn.
    ///
    /// The first parameter says which motor is tested, the second how hard it is driven —
    /// a number between minus one and one, where the sign is the direction — and the
    /// third for how long, in seconds.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command or does not answer in time.
    pub async fn motor_test(
        &self,
        vehicle: VehicleId,
        motor: u8,
        throttle: f32,
        seconds: f32,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_DO_MOTOR_TEST,
            [f32::from(motor), throttle, seconds, 0.0, 0.0, 0.0, 0.0],
            timeout,
        )
        .await?;

        Ok(())
    }

    /// Takes the radio channels over, one at a time.
    ///
    /// The channels that are not given are left as the radio has them: the operator
    /// overrides the stick that is being tested and lets go of the others.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the message.
    pub async fn rc_override(
        &self,
        vehicle: VehicleId,
        channels: [Option<u16>; 8],
    ) -> Result<(), LinkError> {
        // The value that means "the radio has this one".
        const UNCHANGED: u16 = u16::MAX;

        let message = MavMessage::RC_CHANNELS_OVERRIDE(RC_CHANNELS_OVERRIDE_DATA {
            chan1_raw: channels[0].unwrap_or(UNCHANGED),
            chan2_raw: channels[1].unwrap_or(UNCHANGED),
            chan3_raw: channels[2].unwrap_or(UNCHANGED),
            chan4_raw: channels[3].unwrap_or(UNCHANGED),
            chan5_raw: channels[4].unwrap_or(UNCHANGED),
            chan6_raw: channels[5].unwrap_or(UNCHANGED),
            chan7_raw: channels[6].unwrap_or(UNCHANGED),
            chan8_raw: channels[7].unwrap_or(UNCHANGED),
            chan9_raw: UNCHANGED,
            chan10_raw: UNCHANGED,
            chan11_raw: UNCHANGED,
            chan12_raw: UNCHANGED,
            chan13_raw: UNCHANGED,
            chan14_raw: UNCHANGED,
            chan15_raw: UNCHANGED,
            chan16_raw: UNCHANGED,
            chan17_raw: UNCHANGED,
            chan18_raw: UNCHANGED,
            target_system: vehicle.system_id,
            target_component: vehicle.component_id,
        });

        self.write_message(&message).await
    }

    /// Lets go of the radio channels, so that the vehicle flies with the radio again.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the message.
    pub async fn rc_override_release(&self, vehicle: VehicleId) -> Result<(), LinkError> {
        // The value that means "the radio has this one".
        const UNCHANGED: u16 = u16::MAX;

        let message = MavMessage::RC_CHANNELS_OVERRIDE(RC_CHANNELS_OVERRIDE_DATA {
            chan1_raw: UNCHANGED,
            chan2_raw: UNCHANGED,
            chan3_raw: UNCHANGED,
            chan4_raw: UNCHANGED,
            chan5_raw: UNCHANGED,
            chan6_raw: UNCHANGED,
            chan7_raw: UNCHANGED,
            chan8_raw: UNCHANGED,
            chan9_raw: UNCHANGED,
            chan10_raw: UNCHANGED,
            chan11_raw: UNCHANGED,
            chan12_raw: UNCHANGED,
            chan13_raw: UNCHANGED,
            chan14_raw: UNCHANGED,
            chan15_raw: UNCHANGED,
            chan16_raw: UNCHANGED,
            chan17_raw: UNCHANGED,
            chan18_raw: UNCHANGED,
            target_system: vehicle.system_id,
            target_component: vehicle.component_id,
        });

        self.write_message(&message).await
    }

    /// Flies the vehicle with the sticks of the ground station.
    ///
    /// The four values are the roll, the pitch, the thrust and the yaw, each of them
    /// between minus one and one, and the buttons say which ones are pressed.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the message.
    pub async fn manual_control(
        &self,
        vehicle: VehicleId,
        roll: f32,
        pitch: f32,
        thrust: f32,
        yaw: f32,
        buttons: u16,
    ) -> Result<(), LinkError> {
        // The values travel as sixteen bit numbers between minus one and one.
        let scaled = |value: f32| -> i16 { (value.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16 };

        let message = MavMessage::MANUAL_CONTROL(MANUAL_CONTROL_DATA {
            x: scaled(roll),
            y: scaled(pitch),
            z: scaled(thrust),
            r: scaled(yaw),
            buttons,
            target: vehicle.system_id,
            ..Default::default()
        });

        self.write_message(&message).await
    }

    /// Asks the vehicle to restart.
    ///
    /// The command is sent and nothing is waited for: the vehicle is restarting, and the
    /// answer it would give is the answer of a vehicle that is no longer there. The link
    /// notices that the vehicle is gone on its own.
    ///
    /// The first parameter says what is restarted: one is the autopilot.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the command.
    pub async fn reboot(&self, vehicle: VehicleId) -> Result<(), LinkError> {
        let message = MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
            param1: 1.0,
            param2: 0.0,
            param3: 0.0,
            param4: 0.0,
            param5: 0.0,
            param6: 0.0,
            param7: 0.0,
            command: MavCmd::MAV_CMD_PREFLIGHT_REBOOT_SHUTDOWN,
            target_system: vehicle.system_id,
            target_component: vehicle.component_id,
            confirmation: 0,
        });

        self.write_message(&message).await
    }

    /// Executes a command with floating-point parameters.
    ///
    /// This is the executor the other operations are built on. The vehicle's answer is
    /// returned: `MAV_RESULT_ACCEPTED` when the command was carried out.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command, or when it does not answer
    /// in time.
    pub async fn command_long(
        &self,
        vehicle: VehicleId,
        command: MavCmd,
        params: [f32; 7],
        timeout: Duration,
    ) -> Result<MavResult, LinkError> {
        self.execute_command(vehicle, command, timeout, |confirmation| {
            MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
                param1: params[0],
                param2: params[1],
                param3: params[2],
                param4: params[3],
                param5: params[4],
                param6: params[5],
                param7: params[6],
                command,
                target_system: vehicle.system_id,
                target_component: vehicle.component_id,
                confirmation,
            })
        })
        .await
    }

    /// Executes a command with integer parameters.
    ///
    /// The integer form exists for the commands whose position is scaled by `1e7` — a
    /// latitude and a longitude that would lose their precision as floats. The seven
    /// parameters map onto the message as they do in the protocol: the first four are the
    /// generic parameters, the last three are X, Y and Z.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command, or when it does not answer
    /// in time.
    pub async fn command_int(
        &self,
        vehicle: VehicleId,
        command: MavCmd,
        params: [i32; 7],
        timeout: Duration,
    ) -> Result<MavResult, LinkError> {
        self.execute_command(vehicle, command, timeout, |_confirmation| {
            MavMessage::COMMAND_INT(COMMAND_INT_DATA {
                param1: params[0] as f32,
                param2: params[1] as f32,
                param3: params[2] as f32,
                param4: params[3] as f32,
                x: params[4],
                y: params[5],
                z: params[6] as f32,
                command,
                target_system: vehicle.system_id,
                target_component: vehicle.component_id,
                frame: MavFrame::MAV_FRAME_GLOBAL,
                current: 0,
                autocontinue: 0,
            })
        })
        .await
    }

    /// Sends a command and waits for the acknowledgement that answers it.
    async fn execute_command(
        &self,
        vehicle: VehicleId,
        command: MavCmd,
        timeout: Duration,
        message: impl Fn(u8) -> MavMessage,
    ) -> Result<MavResult, LinkError> {
        let collected = self
            .exchange(
                timeout,
                move |received| is_acknowledgement_of(received, vehicle),
                move |received| acknowledgement_result(received, command).is_some(),
                message,
            )
            .await?;

        let result = acknowledgement_result(&collected, command).ok_or(LinkError::Protocol(
            mpcraft_mavlink::ProtocolError::Malformed(
                "the acknowledgement carried no result for the command",
            ),
        ))?;

        if result == MavResult::MAV_RESULT_ACCEPTED {
            return Ok(result);
        }

        Err(LinkError::Command(CommandError::Refused {
            command,
            result,
        }))
    }
}

/// Whether the message is an acknowledgement from the vehicle.
///
/// The acknowledgement is matched on the vehicle as well as on the message id: a link can
/// carry several vehicles, and each of them acknowledges its own commands.
fn is_acknowledgement_of(message: &ReceivedMessage, vehicle: VehicleId) -> bool {
    message.message_id == COMMAND_ACK_ID
        && message.system_id == vehicle.system_id
        && message.component_id == vehicle.component_id
}

/// The result of the last acknowledgement of the given command.
///
/// An acknowledgement that reports `MAV_RESULT_IN_PROGRESS` is not an answer: the vehicle
/// will send the final result later.
fn acknowledgement_result(messages: &[ReceivedMessage], command: MavCmd) -> Option<MavResult> {
    messages
        .iter()
        .rev()
        .find_map(|message| match &message.message {
            MavMessage::COMMAND_ACK(ack) if ack.command == command => match ack.result {
                MavResult::MAV_RESULT_IN_PROGRESS => None,
                result => Some(result),
            },
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpcraft_mavlink::dialect::messages::COMMAND_ACK_DATA;
    /// Builds a received acknowledgement.
    fn ack(command: MavCmd, result: MavResult) -> ReceivedMessage {
        ReceivedMessage {
            system_id: 1,
            component_id: 1,
            message_id: COMMAND_ACK_ID,
            message_name: "COMMAND_ACK".to_owned(),
            message: MavMessage::COMMAND_ACK(COMMAND_ACK_DATA {
                command,
                result,
                ..Default::default()
            }),
        }
    }

    #[test]
    fn the_result_of_the_last_acknowledgement_wins() {
        let messages = vec![
            ack(
                MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                MavResult::MAV_RESULT_IN_PROGRESS,
            ),
            ack(
                MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                MavResult::MAV_RESULT_ACCEPTED,
            ),
        ];

        assert_eq!(
            acknowledgement_result(&messages, MavCmd::MAV_CMD_COMPONENT_ARM_DISARM),
            Some(MavResult::MAV_RESULT_ACCEPTED)
        );
    }

    #[test]
    fn an_acknowledgement_for_another_command_is_ignored() {
        let messages = vec![ack(
            MavCmd::MAV_CMD_NAV_TAKEOFF,
            MavResult::MAV_RESULT_ACCEPTED,
        )];

        assert_eq!(
            acknowledgement_result(&messages, MavCmd::MAV_CMD_COMPONENT_ARM_DISARM),
            None
        );
    }

    #[test]
    fn an_interim_acknowledgement_is_not_an_answer() {
        let messages = vec![ack(
            MavCmd::MAV_CMD_PREFLIGHT_CALIBRATION,
            MavResult::MAV_RESULT_IN_PROGRESS,
        )];

        assert_eq!(
            acknowledgement_result(&messages, MavCmd::MAV_CMD_PREFLIGHT_CALIBRATION),
            None
        );
    }

    #[test]
    fn a_refusal_carries_the_result_the_vehicle_reported() {
        let error = CommandError::Refused {
            command: MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
            result: MavResult::MAV_RESULT_DENIED,
        };

        assert!(error.to_string().contains("DENIED"));
    }
}
