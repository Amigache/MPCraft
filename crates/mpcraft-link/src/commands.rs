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

use std::time::{Duration, Instant};

// `SET_MODE` is deprecated in favour of `MAV_CMD_DO_SET_MODE`, but the legacy message
// carries the custom mode exactly — as a `u32`, without the float rounding the command
// suffers — and every autopilot still understands it. Both are sent.
#[allow(deprecated)]
use mpcraft_mavlink::dialect::messages::{
    AttitudeTargetTypemask, PositionTargetTypemask, COMMAND_INT_DATA, COMMAND_LONG_DATA,
    MANUAL_CONTROL_DATA, RC_CHANNELS_OVERRIDE_DATA, SET_ATTITUDE_TARGET_DATA, SET_MODE_DATA,
    SET_POSITION_TARGET_GLOBAL_INT_DATA,
};
use mpcraft_mavlink::{MavCmd, MavFrame, MavMessage, MavModeFlag, MavResult};
use mpcraft_vehicle::{relays_of, Relays};

use crate::error::LinkError;
use crate::session::{ReceivedMessage, Session};
use crate::vehicle::VehicleId;

/// The message id of `COMMAND_LONG`.
pub const COMMAND_LONG_ID: u32 = 76;

/// The command to reboot into the bootloader.
pub const MAV_CMD_FLASH_BOOTLOADER: u16 = 42650;

/// The message id of `COMMAND_INT`.
pub const COMMAND_INT_ID: u32 = 75;

/// The message id of `COMMAND_ACK`.
pub const COMMAND_ACK_ID: u32 = 77;

/// The message id of `SET_MODE`.
pub const SET_MODE_ID: u32 = 11;

/// The message id of `SET_POSITION_TARGET_GLOBAL_INT`.
pub const SET_POSITION_TARGET_ID: u32 = 86;

/// The message id of `SET_ATTITUDE_TARGET`.
pub const SET_ATTITUDE_TARGET_ID: u32 = 82;

/// The message id of `RC_CHANNELS_OVERRIDE`.
pub const RC_CHANNELS_OVERRIDE_ID: u32 = 70;

/// The message id of `MANUAL_CONTROL`.
pub const MANUAL_CONTROL_ID: u32 = 69;

/// The message id of `RELAY_STATUS`, the report of the relays.
pub const RELAY_STATUS_ID: u32 = 376;

/// The message id of `AUTOPILOT_VERSION`, the report of what the vehicle is capable of.
pub const AUTOPILOT_VERSION_ID: u32 = 148;

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
    /// The vehicle understood the command and refused it.
    ///
    /// The command was sent as a number rather than as a variant the library names — one the
    /// metadata table does not describe — so the number itself is what is said.
    RefusedRaw {
        /// The number of the command that was sent.
        command: u32,
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
            Self::RefusedRaw { command, result } => {
                write!(f, "the vehicle refused the command {command}: {result:?}")
            }
        }
    }
}

impl std::error::Error for CommandError {}

/// A calibration the vehicle is asked to do.
///
/// The autopilot calibrates itself: the order says which sensor, and the vehicle
/// answers when the calibration has begun. Upstream sends the same order — the second
/// parameter is what the compass does and the fifth is what the accelerometer does — and
/// the numbers it uses are the autopilot's, not ours to invent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Calibration {
    /// The accelerometer, in the six positions.
    Accelerometer,
    /// The accelerometer, level with the ground.
    AccelerometerLevel,
    /// The compass.
    Compass,
}

impl Calibration {
    /// The parameters the order carries.
    ///
    /// Zero is nothing, one is the full calibration and two is the level one.
    const fn params(self) -> [f32; 7] {
        match self {
            Calibration::Accelerometer => [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            Calibration::AccelerometerLevel => [0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0],
            Calibration::Compass => [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// The command operations of a session.
/// The message the plane branch's answer travels in.
///
/// This is the one place the decision becomes bytes: the quaternion is rounded to the four
/// floats the wire carries here and nowhere else. The rates are zero because the mask says the
/// vehicle is to leave them alone, but they still travel — MAVLink 2 only removes the *trailing*
/// zeros of a payload, and the thrust is not zero.
#[must_use]
pub fn attitude_target_message(
    vehicle: VehicleId,
    command: &mpcraft_vehicle::formation::AttitudeCommand,
) -> MavMessage {
    MavMessage::SET_ATTITUDE_TARGET(SET_ATTITUDE_TARGET_DATA {
        time_boot_ms: 0,
        q: [
            command.quaternion[0] as f32,
            command.quaternion[1] as f32,
            command.quaternion[2] as f32,
            command.quaternion[3] as f32,
        ],
        body_roll_rate: 0.0,
        body_pitch_rate: 0.0,
        body_yaw_rate: 0.0,
        thrust: command.thrust as f32,
        target_system: vehicle.system_id,
        target_component: vehicle.component_id,
        // Retained rather than truncated: two of the bits upstream's arithmetic leaves set have
        // no name in the dialect as it is defined today, and dropping them would send a different
        // byte than upstream sends.
        type_mask: AttitudeTargetTypemask::from_bits_retain(command.type_mask),
        ..Default::default()
    })
}

/// The message the multicopter branch puts on the wire for one vehicle.
///
/// The mask keeps the position and the speeds and leaves the accelerations, the heading and the
/// turning rate alone, which is what upstream's four flags say (`pos=true, vel=true,
/// acc=false, yaw=false`). Upstream builds the mask from `ushort.MaxValue`, which leaves four
/// bits set that today's dialect has no names for; ArduPilot ignores those bits, so the mask is
/// built from the names it has here rather than from the number upstream's arithmetic produced.
///
/// The frame travels as the number upstream sends, 6, and not as its modern synonym: the
/// synonym carries a different byte, and a vehicle that reads the frame byte reads this one as
/// sea level rather than as height over home.
#[must_use]
#[allow(deprecated)]
pub fn swarm_position_target_message(
    vehicle: VehicleId,
    decision: &mpcraft_vehicle::formation::MulticopterDecision,
) -> MavMessage {
    // The message carries zero speeds when the leader has not reported any.
    let velocity = match decision.velocity {
        Some(velocity) => velocity,
        None => mpcraft_vehicle::Velocity {
            north: 0.0,
            east: 0.0,
            down: 0.0,
            ground_speed: 0.0,
        },
    };

    MavMessage::SET_POSITION_TARGET_GLOBAL_INT(SET_POSITION_TARGET_GLOBAL_INT_DATA {
        time_boot_ms: 0,
        // The integer form scales the coordinates by 1e7.
        lat_int: (decision.target.lat * 1e7).round() as i32,
        lon_int: (decision.target.lng * 1e7).round() as i32,
        alt: decision.target.alt as f32,
        // The speeds are in metres per second in the north-east-down frame, which is what the
        // message carries and what the decision already holds.
        vx: velocity.north as f32,
        vy: velocity.east as f32,
        vz: velocity.down as f32,
        afx: 0.0,
        afy: 0.0,
        afz: 0.0,
        yaw: 0.0,
        yaw_rate: 0.0,
        // The position and the speeds are what the vehicle is told to change; the accelerations
        // and the heading are left as the vehicle has them, and the heading travels in its own
        // command when it is sent at all.
        type_mask: PositionTargetTypemask::POSITION_TARGET_TYPEMASK_AX_IGNORE
            | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_AY_IGNORE
            | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_AZ_IGNORE
            | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_YAW_IGNORE
            | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_YAW_RATE_IGNORE,
        target_system: vehicle.system_id,
        target_component: vehicle.component_id,
        coordinate_frame: MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT_INT,
    })
}

/// The command that points a vehicle at the leader's heading, as upstream sends it: the angle
/// first and one hundred as the second parameter.
#[must_use]
pub fn swarm_condition_yaw_message(vehicle: VehicleId, yaw_deg: f64) -> MavMessage {
    MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
        param1: yaw_deg as f32,
        param2: 100.0,
        param3: 0.0,
        param4: 0.0,
        param5: 0.0,
        param6: 0.0,
        param7: 0.0,
        command: MavCmd::MAV_CMD_CONDITION_YAW,
        target_system: vehicle.system_id,
        target_component: vehicle.component_id,
        confirmation: 0,
    })
}

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
    /// The custom mode is only read when the custom-mode flag is set in the mode flags, which is
    /// what the dictionary says for `MAV_CMD_DO_SET_MODE` parameter 2, so this sets it: a caller
    /// that passes the mode without the flag sends a command whose second parameter the vehicle
    /// ignores, and a live board answers that with `MAV_RESULT_DENIED` — which is what the run
    /// against the simulator found before this line existed.
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
        let base_mode = base_mode | MavModeFlag::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED;

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

    /// Sends the attitude the formation decided for one vehicle.
    ///
    /// The plane branch of the loop answers with a quaternion built through the 3-1-2 matrix,
    /// not with three angles, and with the mask upstream's arithmetic leaves behind. This is a
    /// write rather than a transaction: the protocol has no acknowledgement for it, so there is
    /// nothing to wait for and nothing to retry — the same kind of send `set_position_target`
    /// is. What the vehicle does with the command is not reported back.
    ///
    /// The rates are sent as zero because the mask says the vehicle is to leave them alone; the
    /// bytes still travel, which is what the fixture shows.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the message.
    pub async fn send_attitude_target(
        &self,
        vehicle: VehicleId,
        command: &mpcraft_vehicle::formation::AttitudeCommand,
    ) -> Result<(), LinkError> {
        self.write_message(&attitude_target_message(vehicle, command))
            .await
    }

    /// Sends the place and the speed the formation decided for one multicopter.
    ///
    /// The frame is the relative-altitude one, which is what upstream's multicopter branch
    /// uses (`Formation.cs:300`): the height of a slot is a height over the ground, not over
    /// sea level.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the message.
    pub async fn send_swarm_position_target(
        &self,
        vehicle: VehicleId,
        decision: &mpcraft_vehicle::formation::MulticopterDecision,
    ) -> Result<(), LinkError> {
        self.write_message(&swarm_position_target_message(vehicle, decision))
            .await
    }

    /// Points one vehicle at the leader's heading.
    ///
    /// This is sent only when the difference is past the dead band, which is what the decision
    /// says; the decision itself does not travel, this order does. Like the position target it
    /// is a write rather than a transaction: upstream sends it with `doCommand(..., false)`, so
    /// there is no acknowledgement to wait for at ten hertz.
    ///
    /// # Errors
    ///
    /// Returns an error when the link cannot carry the command.
    pub async fn send_swarm_condition_yaw(
        &self,
        vehicle: VehicleId,
        yaw_deg: f64,
    ) -> Result<(), LinkError> {
        self.write_message(&swarm_condition_yaw_message(vehicle, yaw_deg))
            .await
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

    /// Sets a relay of the vehicle.
    ///
    /// The vehicle says which relays it has, and MPCraft does not ask for one it has said it does
    /// not have: that is not something the vehicle would do at another moment, it is something this
    /// board cannot do, which is the same rule as an angle a gimbal cannot reach. A vehicle that
    /// has never reported its relays is asked anyway, because the report is not streamed unless the
    /// station asks for it — refusing on silence would refuse a board that can do it.
    ///
    /// # Errors
    ///
    /// Returns `unsupported_by_vehicle` for a relay the vehicle has said it does not have, and the
    /// error of the command when the vehicle refuses it or does not answer in time.
    pub async fn set_relay(
        &self,
        vehicle: VehicleId,
        instance: u16,
        on: bool,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        // The report is read through the snapshot, so it is cloned rather than moved out of it.
        let reported = self
            .state()
            .telemetries
            .get(&vehicle)
            .and_then(|state| state.relays.clone());

        if let Some(relays) = reported {
            if !mpcraft_vehicle::has_relay(&relays, instance) {
                return Err(LinkError::Unsupported(format!(
                    "the vehicle has no relay {instance}"
                )));
            }
        }

        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_DO_SET_RELAY,
            [
                instance as f32,
                if on { 1.0 } else { 0.0 },
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ],
            timeout,
        )
        .await?;

        Ok(())
    }

    /// Asks the vehicle once for the report of its relays.
    ///
    /// The specification says this report is streamed only when the station asks for it, so a sheet
    /// that waited for something to arrive would show nothing on a vehicle that has relays and has
    /// said nothing about them. The answer reads into the state of the vehicle through the
    /// aggregator, which is what the sheet shows; this returns the same report so that the caller
    /// can say whether the vehicle answered at all.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle does not answer in time.
    pub async fn request_relay_status(
        &self,
        vehicle: VehicleId,
        timeout: Duration,
    ) -> Result<Relays, LinkError> {
        let matcher = move |message: &ReceivedMessage| {
            message.message_id == RELAY_STATUS_ID && message.system_id == vehicle.system_id
        };

        let collected = self
            .exchange(
                timeout,
                matcher,
                |messages: &[ReceivedMessage]| !messages.is_empty(),
                move |confirmation| {
                    MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
                        param1: RELAY_STATUS_ID as f32,
                        param2: 0.0,
                        param3: 0.0,
                        param4: 0.0,
                        param5: 0.0,
                        param6: 0.0,
                        param7: 0.0,
                        command: MavCmd::MAV_CMD_REQUEST_MESSAGE,
                        target_system: vehicle.system_id,
                        target_component: vehicle.component_id,
                        confirmation,
                    })
                },
            )
            .await?;

        let answer = collected.first().ok_or(LinkError::Timeout(timeout))?;

        match &answer.message {
            MavMessage::RELAY_STATUS(report) => Ok(relays_of(report.present, report.on)),
            _ => Err(LinkError::Protocol(
                mpcraft_mavlink::ProtocolError::Malformed(
                    "the answer to the request for the relays is not a report of the relays",
                ),
            )),
        }
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

    /// Asks the vehicle to calibrate one of its sensors.
    ///
    /// The calibration is carried out by the vehicle: the order says which sensor, and
    /// the vehicle answers when it has begun. The answer is the vehicle's, not the
    /// interface's.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command, or when it does not answer
    /// in time.
    pub async fn calibrate(
        &self,
        vehicle: VehicleId,
        calibration: Calibration,
        timeout: Duration,
    ) -> Result<MavResult, LinkError> {
        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_PREFLIGHT_CALIBRATION,
            calibration.params(),
            timeout,
        )
        .await
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

    /// Reboots a vehicle into the bootloader so it can be flashed.
    ///
    /// The command is sent as a raw `COMMAND_LONG`: the number travels as it was written, rather
    /// than as a variant of the enumeration. That is what a command the metadata table does not
    /// describe needs, because the generic executor cannot say what its slots mean and so does not
    /// offer it. The dialect the project uses does name 42650 as `MAV_CMD_FLASH_BOOTLOADER`; the
    /// raw path is kept for the numbers it does not. The vehicle answers with `COMMAND_ACK`.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command or does not answer in time.
    pub async fn flash_bootloader(
        &self,
        vehicle: VehicleId,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let payload_base = encode_command_long(
            vehicle.system_id,
            vehicle.component_id,
            MAV_CMD_FLASH_BOOTLOADER as u32,
            0,
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        );

        let mut collector = self
            .begin_collect(
                move |message: &ReceivedMessage| is_acknowledgement_of(message, vehicle),
                |messages: &[ReceivedMessage]| acknowledgement_result_raw(messages).is_some(),
            )
            .await?;

        let start = Instant::now();
        let mut attempt: u8 = 0;
        loop {
            let mut payload = payload_base;
            payload[32] = attempt;
            self.write_raw(COMMAND_LONG_ID, &payload).await?;
            attempt += 1;

            // The first attempt gets half of the budget, the retry the other half.
            let share = if attempt == 1 { timeout / 2 } else { timeout };
            let budget = share.saturating_sub(start.elapsed());

            match collector.wait_for(budget).await {
                Ok(messages) => {
                    if let Some(result) = acknowledgement_result_raw(&messages) {
                        if result == MavResult::MAV_RESULT_ACCEPTED {
                            return Ok(());
                        }

                        if result == MavResult::MAV_RESULT_IN_PROGRESS {
                            continue;
                        }

                        return Err(LinkError::from(CommandError::RefusedRaw {
                            command: MAV_CMD_FLASH_BOOTLOADER as u32,
                            result,
                        }));
                    }

                    if attempt < 2 {
                        continue;
                    }

                    return Err(LinkError::Timeout(timeout));
                }
                Err(LinkError::Timeout(_)) if attempt < 2 => continue,
                Err(error) => return Err(error),
            }
        }
    }

    /// Writes bytes to a serial port of the vehicle.
    ///
    /// The vehicle has several serial ports ? the telemetry ones, the ones of the GPS ? and
    /// this is how a program reaches them: the bytes travel inside a MAVLink message,
    /// and the vehicle hands them over to the port.
    ///
    /// The data goes in blocks of seventy bytes, the most a message carries, and the
    /// vehicle is asked to answer with what the port had to say.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is closed or the write fails.
    pub async fn serial_control(
        &self,
        vehicle: VehicleId,
        device: u8,
        data: &[u8],
        baudrate: u32,
        timeout: u16,
    ) -> Result<(), LinkError> {
        use mpcraft_mavlink::dialect::messages::SERIAL_CONTROL_DATA;
        use mpcraft_mavlink::dialect::{MavMessage, SerialControlDev, SerialControlFlag};

        let device = match device {
            1 => SerialControlDev::SERIAL_CONTROL_DEV_TELEM2,
            2 => SerialControlDev::SERIAL_CONTROL_DEV_GPS1,
            3 => SerialControlDev::SERIAL_CONTROL_DEV_GPS2,
            4 => SerialControlDev::SERIAL_CONTROL_DEV_TELEM3,
            _ => SerialControlDev::SERIAL_CONTROL_DEV_TELEM1,
        };

        let mut flags = SerialControlFlag::SERIAL_CONTROL_FLAG_EXCLUSIVE;

        let mut rest = data;
        let mut blocks = rest.len().div_ceil(70);

        while !rest.is_empty() {
            if blocks == 1 {
                flags |= SerialControlFlag::SERIAL_CONTROL_FLAG_RESPOND;
            }

            let count = rest.len().min(70);
            let mut payload = [0_u8; 70];
            payload[..count].copy_from_slice(&rest[..count]);

            let message = MavMessage::SERIAL_CONTROL(SERIAL_CONTROL_DATA {
                baudrate,
                timeout,
                device,
                flags,
                count: count as u8,
                data: payload,
                target_system: vehicle.system_id,
                target_component: vehicle.component_id,
            });

            self.write_message(&message).await?;

            rest = &rest[count..];
            blocks -= 1;
        }

        Ok(())
    }

    /// Asks the vehicle what it is capable of.
    ///
    /// The capability bits are not something a station can assume: they decide how the items of a plan
    /// travel — a vehicle that advertises `MISSION_INT` wants the integer form, where a place keeps the
    /// precision the operator typed, and a station that never asks sends and receives the float form and
    /// loses it. So the station asks, the way upstream does when the link opens, and what the plan editor
    /// uses afterwards is the state the answer filled.
    ///
    /// The refusal and the silence are kept apart here as they are in the generic executor: the command
    /// is answered first, and the report itself afterwards, so a board that says it cannot do this is not
    /// reported as a board that never answered.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command, or when it does not answer in time.
    pub async fn request_capabilities(
        &self,
        vehicle: VehicleId,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        // The matcher for the report is registered before the ask goes out: a vehicle can answer the
        // acknowledgement and the report in the same breath, and an answer that arrives before anybody
        // is listening is lost.
        let mut report = self
            .begin_request(move |received| {
                received.message_id == AUTOPILOT_VERSION_ID
                    && received.system_id == vehicle.system_id
                    && received.component_id == vehicle.component_id
            })
            .await?;

        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_REQUEST_MESSAGE,
            [AUTOPILOT_VERSION_ID as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            timeout,
        )
        .await?;

        report.wait_for(timeout).await?;

        Ok(())
    }

    /// Asks the vehicle to send one message at a given rate.
    ///
    /// A vehicle decides what it streams by the rates configured on the port it is talking on, and those
    /// rates can all be zero — a station that reads a screen then waits for something that will not come.
    /// So the station asks for what it reads, the way upstream does through this command or through its
    /// data-stream request. The answer is what the vehicle said to the ask, which is what the caller has
    /// to show when the vehicle refuses.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command or does not answer in time.
    pub async fn set_message_interval(
        &self,
        vehicle: VehicleId,
        message_id: u32,
        interval_us: f32,
        timeout: Duration,
    ) -> Result<MavResult, LinkError> {
        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_SET_MESSAGE_INTERVAL,
            [message_id as f32, interval_us, 0.0, 0.0, 0.0, 0.0, 0.0],
            timeout,
        )
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
        self.command_int_in_frame(
            vehicle,
            command,
            params,
            MavFrame::MAV_FRAME_GLOBAL,
            timeout,
        )
        .await
    }

    /// Sends an integer command in the frame the command needs.
    ///
    /// The frame is part of what the command means: a height above the ground and a height
    /// above the sea are different numbers, and a point of interest for a gimbal is asked
    /// for in the frame of the ground.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command, or when it does not answer
    /// in time.
    pub(crate) async fn command_int_in_frame(
        &self,
        vehicle: VehicleId,
        command: MavCmd,
        params: [i32; 7],
        frame: MavFrame,
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
                frame,
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

/// Encodes a `COMMAND_LONG` payload for a command id the generated bindings cannot hold.
///
/// The layout is the one of the protocol: seven parameters as little endian floats, the
/// command as a 16-bit little endian number, and the vehicle the command is for.
fn encode_command_long(
    target_system: u8,
    target_component: u8,
    command: u32,
    confirmation: u8,
    params: [f32; 7],
) -> [u8; 33] {
    let mut payload = [0_u8; 33];

    for (index, param) in params.iter().enumerate() {
        payload[index * 4..(index + 1) * 4].copy_from_slice(&param.to_le_bytes());
    }

    payload[28] = (command & 0xFF) as u8;
    payload[29] = ((command >> 8) & 0xFF) as u8;
    payload[30] = target_system;
    payload[31] = target_component;
    payload[32] = confirmation;

    payload
}

/// The result of the last acknowledgement of a raw command.
///
/// A raw command may be a number the enumeration does not name, and the generated parser refuses an
/// acknowledgement whose command value it does not define, so the id cannot be compared here. The
/// acknowledgement is matched on the vehicle instead — the same way the ordinary commands are — and
/// the result is taken from the last one that is not interim.
fn acknowledgement_result_raw(messages: &[ReceivedMessage]) -> Option<MavResult> {
    messages
        .iter()
        .rev()
        .find_map(|message| match &message.message {
            MavMessage::COMMAND_ACK(ack) => match ack.result {
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
    fn command_long_raw_encodes_the_large_command_id() {
        let payload = encode_command_long(1, 1, 42650, 0, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(payload[28], (42650 & 0xFF) as u8);
        assert_eq!(payload[29], ((42650 >> 8) & 0xFF) as u8);
    }

    #[test]
    fn the_multicopter_branch_carries_the_slot_and_the_leader_speeds() {
        // The hand: the coordinates travel scaled by 1e7, the mask keeps what upstream's four
        // flags keep — the position and the speeds — and ignores the rest, which is bits 6, 7, 8
        // for the accelerations and 10 and 11 for the heading and its rate: 64 + 128 + 256 +
        // 1024 + 2048 = 3520.
        let decision = mpcraft_vehicle::formation::MulticopterDecision {
            target: mpcraft_vehicle::GeoPoint {
                lat: -35.0,
                lng: 3.0,
                alt: 105.0,
            },
            velocity: Some(mpcraft_vehicle::Velocity {
                north: 8.0,
                east: 0.0,
                down: 0.0,
                ground_speed: 8.0,
            }),
            yaw_deg: Some(0.0),
        };

        let message = swarm_position_target_message(VehicleId::new(3, 1), &decision);

        let MavMessage::SET_POSITION_TARGET_GLOBAL_INT(target) = message else {
            panic!("the multicopter branch answers with the position target");
        };

        assert_eq!(target.lat_int, -350_000_000);
        assert_eq!(target.lon_int, 30_000_000);
        assert_eq!(target.alt, 105.0);
        assert_eq!((target.vx, target.vy, target.vz), (8.0, 0.0, 0.0));
        assert_eq!(target.type_mask.bits(), 3520);
        // The frame as the number upstream sends it, 6, rather than as its modern synonym,
        // which carries a different byte.
        assert_eq!(target.coordinate_frame as u32, 6);
        assert_eq!((target.target_system, target.target_component), (3, 1));
    }

    #[test]
    fn a_leader_that_has_not_reported_speeds_sends_zeros() {
        let decision = mpcraft_vehicle::formation::MulticopterDecision {
            target: mpcraft_vehicle::GeoPoint {
                lat: -35.0,
                lng: 3.0,
                alt: 105.0,
            },
            velocity: None,
            yaw_deg: None,
        };

        let message = swarm_position_target_message(VehicleId::new(3, 1), &decision);

        let MavMessage::SET_POSITION_TARGET_GLOBAL_INT(target) = message else {
            panic!("the multicopter branch answers with the position target");
        };

        assert_eq!((target.vx, target.vy, target.vz), (0.0, 0.0, 0.0));
    }

    #[test]
    fn the_heading_command_carries_the_angle_and_the_offset() {
        // Upstream passes the leader's heading as the first parameter and one hundred as the
        // second; the command number is 115.
        let message = swarm_condition_yaw_message(VehicleId::new(3, 1), 12.345);

        let MavMessage::COMMAND_LONG(command) = message else {
            panic!("the heading travels as a command");
        };

        assert_eq!(command.command, MavCmd::MAV_CMD_CONDITION_YAW);
        assert_eq!(MavCmd::MAV_CMD_CONDITION_YAW as u32, 115);
        assert_eq!(command.param1, 12.345);
        assert_eq!(command.param2, 100.0);
        assert_eq!((command.target_system, command.target_component), (3, 1));
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
