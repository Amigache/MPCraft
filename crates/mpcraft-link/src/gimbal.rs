//! The gimbal manager protocol.
//!
//! A gimbal is a mount the ground station can point. The **manager** is the component that
//! owns one gimbal device: it says what the device can do and how far its axes go, and it
//! answers with where the device is pointing.
//!
//! Upstream has the same protocol (`GimbalManagerProtocol.cs`), and MPCraft keeps the two
//! decisions upstream makes: a capability the device does not have is refused **before**
//! anything is sent, and an angle is read in the frame the device says it uses.
//!
//! ```text
//! GCS  ────REQUEST_MESSAGE (GIMBAL_MANAGER_INFORMATION)──▶  vehicle
//! GCS  ◀────GIMBAL_MANAGER_INFORMATION────────────────────   what it can do, its limits
//! GCS  ◀────GIMBAL_MANAGER_STATUS──────────────────────────  the flags in force
//! GCS  ◀────GIMBAL_DEVICE_ATTITUDE_STATUS──────────────────  where it points
//! ```

use std::collections::BTreeMap;
use std::time::Duration;

use mpcraft_mavlink::dialect::messages::{
    COMMAND_LONG_DATA, GIMBAL_DEVICE_ATTITUDE_STATUS_DATA, GIMBAL_MANAGER_INFORMATION_DATA,
    GIMBAL_MANAGER_SET_PITCHYAW_DATA, GIMBAL_MANAGER_STATUS_DATA,
};
use mpcraft_mavlink::dialect::{
    GimbalDeviceFlags, GimbalManagerCapFlags, GimbalManagerFlags, MavCmd, MavFrame, MavMessage,
};
use serde::Serialize;

use crate::error::LinkError;
use crate::session::{ReceivedMessage, Session};
use crate::vehicle::VehicleId;

/// The message that says what a gimbal can do and how far its axes go.
pub const GIMBAL_MANAGER_INFORMATION_ID: u32 = 280;

/// The message that says which flags a gimbal is running with.
pub const GIMBAL_MANAGER_STATUS_ID: u32 = 281;

/// The message that points a gimbal at an angle without waiting for confirmation.
pub const GIMBAL_MANAGER_SET_PITCHYAW_ID: u32 = 287;

/// The message that says where a gimbal is pointing.
pub const GIMBAL_DEVICE_ATTITUDE_STATUS_ID: u32 = 285;

/// How long to wait for the vehicle to describe its gimbals.
const GIMBAL_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// How long to wait for the acknowledgement of a gimbal command.
pub const DEFAULT_GIMBAL_TIMEOUT: Duration = Duration::from_secs(2);

/// How many gimbals are kept: a link that reports more than this is not believed.
const MAX_DEVICES: usize = 16;

/// What a gimbal can do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GimbalCapabilities {
    /// Whether the gimbal can be put in its safe position.
    pub has_retract: bool,
    /// Whether the gimbal can be put in its default position.
    pub has_neutral: bool,
    /// Whether the gimbal has a roll axis.
    pub has_roll_axis: bool,
    /// Whether the gimbal has a pitch axis.
    pub has_pitch_axis: bool,
    /// Whether the gimbal has a yaw axis.
    pub has_yaw_axis: bool,
    /// Whether the yaw can be held against the ground.
    pub has_yaw_lock: bool,
    /// Whether the yaw can follow the vehicle.
    pub has_yaw_follow: bool,
    /// Whether the gimbal can be pointed at an angle.
    pub can_point_local: bool,
    /// Whether the gimbal can be pointed at a place on the ground.
    pub can_point_global: bool,
}

impl GimbalCapabilities {
    /// The capabilities the manager reports.
    #[must_use]
    pub fn of(flags: GimbalManagerCapFlags) -> Self {
        Self {
            has_retract: flags
                .contains(GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_HAS_RETRACT),
            has_neutral: flags
                .contains(GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_HAS_NEUTRAL),
            has_roll_axis: flags
                .contains(GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_HAS_ROLL_AXIS),
            has_pitch_axis: flags
                .contains(GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_HAS_PITCH_AXIS),
            has_yaw_axis: flags
                .contains(GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_HAS_YAW_AXIS),
            has_yaw_lock: flags
                .contains(GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_HAS_YAW_LOCK),
            has_yaw_follow: flags
                .contains(GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_HAS_YAW_FOLLOW),
            can_point_local: flags
                .contains(GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_CAN_POINT_LOCATION_LOCAL),
            can_point_global: flags.contains(
                GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_CAN_POINT_LOCATION_GLOBAL,
            ),
        }
    }
}

/// How far the axes of a gimbal go, in degrees.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GimbalLimits {
    /// The lowest roll angle.
    pub roll_min: f64,
    /// The highest roll angle.
    pub roll_max: f64,
    /// The lowest pitch angle.
    pub pitch_min: f64,
    /// The highest pitch angle.
    pub pitch_max: f64,
    /// The lowest yaw angle.
    pub yaw_min: f64,
    /// The highest yaw angle.
    pub yaw_max: f64,
}

impl GimbalLimits {
    /// The limits the manager reports, read from the radians the message carries.
    #[must_use]
    pub fn of(message: &GIMBAL_MANAGER_INFORMATION_DATA) -> Self {
        Self {
            roll_min: to_degrees(f64::from(message.roll_min)),
            roll_max: to_degrees(f64::from(message.roll_max)),
            pitch_min: to_degrees(f64::from(message.pitch_min)),
            pitch_max: to_degrees(f64::from(message.pitch_max)),
            yaw_min: to_degrees(f64::from(message.yaw_min)),
            yaw_max: to_degrees(f64::from(message.yaw_max)),
        }
    }
}

/// The flags a gimbal is running with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GimbalFlags {
    /// Whether the gimbal is in its safe position.
    pub retracted: bool,
    /// Whether the gimbal is in its default position.
    pub neutral: bool,
    /// Whether the roll is held against the horizon.
    pub roll_lock: bool,
    /// Whether the pitch is held against the horizon.
    pub pitch_lock: bool,
    /// Whether the yaw is held against the ground.
    pub yaw_lock: bool,
    /// Whether the yaw is measured from the way the vehicle is pointing.
    pub yaw_in_vehicle_frame: bool,
    /// Whether the yaw is measured from the ground.
    pub yaw_in_earth_frame: bool,
}

impl GimbalFlags {
    /// The flags the manager says are in force.
    #[must_use]
    pub fn of(flags: GimbalDeviceFlags) -> Self {
        Self {
            retracted: flags.contains(GimbalDeviceFlags::GIMBAL_DEVICE_FLAGS_RETRACT),
            neutral: flags.contains(GimbalDeviceFlags::GIMBAL_DEVICE_FLAGS_NEUTRAL),
            roll_lock: flags.contains(GimbalDeviceFlags::GIMBAL_DEVICE_FLAGS_ROLL_LOCK),
            pitch_lock: flags.contains(GimbalDeviceFlags::GIMBAL_DEVICE_FLAGS_PITCH_LOCK),
            yaw_lock: flags.contains(GimbalDeviceFlags::GIMBAL_DEVICE_FLAGS_YAW_LOCK),
            yaw_in_vehicle_frame: flags
                .contains(GimbalDeviceFlags::GIMBAL_DEVICE_FLAGS_YAW_IN_VEHICLE_FRAME),
            yaw_in_earth_frame: flags
                .contains(GimbalDeviceFlags::GIMBAL_DEVICE_FLAGS_YAW_IN_EARTH_FRAME),
        }
    }
}

/// Where a gimbal points, in degrees.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GimbalAttitude {
    /// The roll angle.
    pub roll: f64,
    /// The pitch angle.
    pub pitch: f64,
    /// The yaw angle, always read against the ground.
    pub yaw: f64,
}

/// One gimbal the vehicle has reported.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GimbalDevice {
    /// The component that reports this gimbal.
    pub component_id: u8,
    /// The device id the commands name.
    pub device_id: u8,
    /// What the gimbal can do.
    pub capabilities: GimbalCapabilities,
    /// How far its axes go.
    pub limits: GimbalLimits,
    /// The flags it is running with.
    pub flags: GimbalFlags,
    /// Where it points, when the vehicle has said.
    pub attitude: Option<GimbalAttitude>,
    /// Whether the gimbal says something is wrong with it.
    pub failed: bool,
    /// The component that holds the control of the gimbal, `0` when nobody does.
    pub controller_component_id: u8,
}

impl GimbalDevice {
    /// A gimbal as the manager first describes it.
    #[must_use]
    pub fn of(component_id: u8, message: &GIMBAL_MANAGER_INFORMATION_DATA) -> Self {
        Self {
            component_id,
            device_id: message.gimbal_device_id,
            capabilities: GimbalCapabilities::of(message.cap_flags),
            limits: GimbalLimits::of(message),
            flags: GimbalFlags::default(),
            attitude: None,
            failed: false,
            controller_component_id: 0,
        }
    }
}

/// The gimbals the vehicle has reported, by device id.
#[derive(Debug, Clone, Default)]
pub struct GimbalRegistry {
    devices: BTreeMap<u8, GimbalDevice>,
}

impl GimbalRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records what a gimbal can do.
    pub fn observe_information(
        &mut self,
        component_id: u8,
        message: &GIMBAL_MANAGER_INFORMATION_DATA,
    ) {
        if self.devices.len() >= MAX_DEVICES
            && !self.devices.contains_key(&message.gimbal_device_id)
        {
            return;
        }

        self.devices.insert(
            message.gimbal_device_id,
            GimbalDevice::of(component_id, message),
        );
    }

    /// Records the flags a gimbal is running with.
    pub fn observe_status(&mut self, message: &GIMBAL_MANAGER_STATUS_DATA) {
        let Some(device) = self.devices.get_mut(&message.gimbal_device_id) else {
            return;
        };

        device.flags = GimbalFlags::of(as_device_flags(message.flags.bits()));
        device.controller_component_id = message.primary_control_compid;
    }

    /// Records where a gimbal points.
    pub fn observe_attitude(&mut self, message: &GIMBAL_DEVICE_ATTITUDE_STATUS_DATA) {
        let Some(device) = self.devices.get_mut(&message.gimbal_device_id) else {
            return;
        };

        let (roll, pitch, yaw) = euler_of(&message.q);
        device.attitude = Some(GimbalAttitude { roll, pitch, yaw });
        // The way the yaw is measured is part of what the attitude means, so the flag is
        // read from the same message that carries the angle.
        device.flags = GimbalFlags::of(message.flags);
        device.failed = message.failure_flags.bits() != 0;
    }

    /// The gimbals that are known, by device id.
    #[must_use]
    pub fn devices(&self) -> Vec<GimbalDevice> {
        self.devices.values().cloned().collect()
    }

    /// The gimbal a command is checked against.
    ///
    /// A device id of `0` means *all* gimbals, which upstream reads as the lowest one —
    /// the same rule `ManagerInfo[0]` follows.
    #[must_use]
    pub fn device_for(&self, requested: u8) -> Option<&GimbalDevice> {
        resolve(&self.devices.values().collect::<Vec<_>>(), requested)
    }

    /// Whether the yaw of an angle is read in the frame of the vehicle.
    ///
    /// The older protocols do not say which frame the yaw is in; with those, upstream
    /// infers it from whether the yaw is locked, and MPCraft keeps that inference.
    #[must_use]
    pub fn yaw_in_vehicle_frame(&self, requested: u8) -> bool {
        let Some(device) = self.device_for(requested) else {
            return false;
        };

        if !device.flags.yaw_in_earth_frame && !device.flags.yaw_in_vehicle_frame {
            return !device.flags.yaw_lock;
        }

        device.flags.yaw_in_vehicle_frame
    }
}

/// The gimbal a command is checked against, out of the ones the vehicle has reported.
fn resolve<'a>(devices: &[&'a GimbalDevice], requested: u8) -> Option<&'a GimbalDevice> {
    devices
        .iter()
        .find(|device| device.device_id == requested)
        .or_else(|| devices.first())
        .copied()
}

/// The angles a quaternion carries, in degrees.
///
/// The quaternion is the one the protocol carries: `w, x, y, z`, with `[1, 0, 0, 0]` as the
/// null rotation.
#[must_use]
pub fn euler_of(q: &[f32; 4]) -> (f64, f64, f64) {
    let (w, x, y, z) = (
        f64::from(q[0]),
        f64::from(q[1]),
        f64::from(q[2]),
        f64::from(q[3]),
    );

    let roll = (2.0 * (w * x + y * z)).atan2(1.0 - 2.0 * (x * x + y * y));
    let pitch = (2.0 * (w * y - z * x)).clamp(-1.0, 1.0).asin();
    let yaw = (2.0 * (w * z + x * y)).atan2(1.0 - 2.0 * (y * y + z * z));

    (to_degrees(roll), to_degrees(pitch), to_degrees(yaw))
}

/// The flags of a gimbal device, from the flags its manager carries.
///
/// The first sixteen bits of the manager flags are the device flags, which is what the
/// protocol says.
fn as_device_flags(bits: u32) -> GimbalDeviceFlags {
    GimbalDeviceFlags::from_bits_truncate(u16::try_from(bits).unwrap_or(0))
}

/// An angle folded into the range the command carries.
#[must_use]
pub fn wrap_180(angle: f64) -> f64 {
    let mut angle = angle;

    while angle > 180.0 {
        angle -= 360.0;
    }
    while angle < -180.0 {
        angle += 360.0;
    }

    angle
}

/// The degrees a value in radians carries.
const fn to_degrees(radians: f64) -> f64 {
    radians * 180.0 / std::f64::consts::PI
}

/// The reason an angle cannot be asked for, or nothing when it can.
fn check_angles(device: &GimbalDevice, pitch: f64, yaw: f64, yaw_lock: bool) -> Option<String> {
    if !device.capabilities.can_point_local {
        return Some("the gimbal cannot be pointed at an angle".to_owned());
    }
    if pitch != 0.0 && !device.capabilities.has_pitch_axis {
        return Some("the gimbal has no pitch axis".to_owned());
    }
    if yaw != 0.0 && !device.capabilities.has_yaw_axis {
        return Some("the gimbal has no yaw axis".to_owned());
    }
    if yaw != 0.0 && yaw_lock && !device.capabilities.has_yaw_lock {
        return Some("the gimbal cannot hold its yaw".to_owned());
    }
    if yaw != 0.0 && !yaw_lock && !device.capabilities.has_yaw_follow {
        return Some("the gimbal cannot follow the vehicle".to_owned());
    }

    None
}

/// The reason a rate cannot be asked for, or nothing when it can.
fn check_rates(
    device: &GimbalDevice,
    pitch_rate: f64,
    yaw_rate: f64,
    yaw_in_earth_frame: bool,
) -> Option<String> {
    if pitch_rate != 0.0 && !device.capabilities.has_pitch_axis {
        return Some("the gimbal has no pitch axis".to_owned());
    }
    if yaw_rate != 0.0 && !device.capabilities.has_yaw_axis {
        return Some("the gimbal has no yaw axis".to_owned());
    }
    if yaw_rate != 0.0 && yaw_in_earth_frame && !device.capabilities.has_yaw_lock {
        return Some("the gimbal cannot hold its yaw".to_owned());
    }
    if yaw_rate != 0.0 && !yaw_in_earth_frame && !device.capabilities.has_yaw_follow {
        return Some("the gimbal cannot follow the vehicle".to_owned());
    }

    None
}

/// The reason an angle is outside the range the gimbal can reach, or nothing when it is not.
fn check_limits(device: &GimbalDevice, pitch: f64, yaw: f64) -> Option<String> {
    if pitch < device.limits.pitch_min || pitch > device.limits.pitch_max {
        return Some(format!(
            "the pitch is outside the range of the gimbal ({:.1} to {:.1})",
            device.limits.pitch_min, device.limits.pitch_max
        ));
    }

    if yaw < device.limits.yaw_min || yaw > device.limits.yaw_max {
        return Some(format!(
            "the yaw is outside the range of the gimbal ({:.1} to {:.1})",
            device.limits.yaw_min, device.limits.yaw_max
        ));
    }

    None
}

/// The gimbal operations of a session.
impl Session {
    /// Asks the vehicle what its gimbals can do.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle does not answer within the timeout, or the link
    /// cannot carry the request.
    pub async fn request_gimbal_information(
        &self,
        vehicle: VehicleId,
    ) -> Result<Vec<GimbalDevice>, LinkError> {
        let messages = self
            .exchange(
                GIMBAL_REQUEST_TIMEOUT,
                move |message: &ReceivedMessage| {
                    message.message_id == GIMBAL_MANAGER_INFORMATION_ID
                        && message.system_id == vehicle.system_id
                },
                |messages: &[ReceivedMessage]| !messages.is_empty(),
                move |confirmation| {
                    MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
                        param1: GIMBAL_MANAGER_INFORMATION_ID as f32,
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

        let found = messages
            .iter()
            .filter_map(|message| match &message.message {
                MavMessage::GIMBAL_MANAGER_INFORMATION(information) => {
                    Some(GimbalDevice::of(message.component_id, information))
                }
                _ => None,
            })
            .collect();

        Ok(found)
    }

    /// Points a gimbal at an angle and waits for the vehicle to say it took it.
    ///
    /// # Errors
    ///
    /// Returns an error when the gimbal cannot do this, when the vehicle refuses the
    /// command, or when it does not answer.
    pub async fn set_gimbal_angles(
        &self,
        vehicle: VehicleId,
        device_id: u8,
        pitch: f64,
        yaw: f64,
        yaw_lock: bool,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let device = reported_gimbal(self, device_id).ok_or_else(|| {
            LinkError::Unsupported("the vehicle has not said what its gimbal can do".to_owned())
        })?;

        if let Some(reason) = check_angles(&device, pitch, yaw, yaw_lock) {
            return Err(LinkError::Unsupported(reason));
        }

        if let Some(reason) = check_limits(&device, pitch, yaw) {
            return Err(LinkError::Unsupported(reason));
        }

        self.gimbal_command(vehicle, device_id, pitch, yaw, yaw_lock, timeout)
            .await
    }

    /// Points a gimbal at an angle without waiting for confirmation.
    ///
    /// This is the message a joystick drives: it goes out at the rate the operator moves
    /// the stick, and the vehicle does not answer it.
    ///
    /// # Errors
    ///
    /// Returns an error when the gimbal cannot do this, or when the link is closed.
    pub async fn set_gimbal_angles_now(
        &self,
        vehicle: VehicleId,
        device_id: u8,
        pitch: f64,
        yaw: f64,
        pitch_rate: f64,
        yaw_rate: f64,
    ) -> Result<(), LinkError> {
        let device = reported_gimbal(self, device_id).ok_or_else(|| {
            LinkError::Unsupported("the vehicle has not said what its gimbal can do".to_owned())
        })?;

        if let Some(reason) = check_rates(&device, pitch_rate, yaw_rate, true) {
            return Err(LinkError::Unsupported(reason));
        }

        let flags = GimbalManagerFlags::GIMBAL_MANAGER_FLAGS_YAW_LOCK;

        self.write_message(&MavMessage::GIMBAL_MANAGER_SET_PITCHYAW(
            GIMBAL_MANAGER_SET_PITCHYAW_DATA {
                target_system: vehicle.system_id,
                target_component: vehicle.component_id,
                gimbal_device_id: device_id,
                pitch: to_f32(pitch),
                yaw: to_f32(yaw),
                pitch_rate: to_f32(pitch_rate),
                yaw_rate: to_f32(yaw_rate),
                flags,
            },
        ))
        .await
    }

    /// Puts a gimbal in its safe position.
    ///
    /// # Errors
    ///
    /// Returns an error when the gimbal cannot be retracted, when the vehicle refuses the
    /// command, or when it does not answer.
    pub async fn retract_gimbal(
        &self,
        vehicle: VehicleId,
        device_id: u8,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let device = reported_gimbal(self, device_id).ok_or_else(|| {
            LinkError::Unsupported("the vehicle has not said what its gimbal can do".to_owned())
        })?;

        if !device.capabilities.has_retract {
            return Err(LinkError::Unsupported(
                "the gimbal cannot be retracted".to_owned(),
            ));
        }

        self.gimbal_command(vehicle, device_id, f64::NAN, f64::NAN, false, timeout)
            .await
    }

    /// Puts a gimbal in its default position.
    ///
    /// # Errors
    ///
    /// Returns an error when the gimbal cannot go to neutral, when the vehicle refuses the
    /// command, or when it does not answer.
    pub async fn neutral_gimbal(
        &self,
        vehicle: VehicleId,
        device_id: u8,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let device = reported_gimbal(self, device_id).ok_or_else(|| {
            LinkError::Unsupported("the vehicle has not said what its gimbal can do".to_owned())
        })?;

        if !device.capabilities.has_neutral {
            return Err(LinkError::Unsupported(
                "the gimbal cannot go to neutral".to_owned(),
            ));
        }

        self.gimbal_command(vehicle, device_id, f64::NAN, f64::NAN, false, timeout)
            .await
    }

    /// Says whether the yaw of the gimbal is held against the ground or follows the vehicle.
    ///
    /// # Errors
    ///
    /// Returns an error when the gimbal cannot do this, when the vehicle refuses the
    /// command, or when it does not answer.
    pub async fn set_gimbal_yaw_lock(
        &self,
        vehicle: VehicleId,
        device_id: u8,
        yaw_lock: bool,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let device = reported_gimbal(self, device_id).ok_or_else(|| {
            LinkError::Unsupported("the vehicle has not said what its gimbal can do".to_owned())
        })?;

        if yaw_lock && !device.capabilities.has_yaw_lock {
            return Err(LinkError::Unsupported(
                "the gimbal cannot hold its yaw".to_owned(),
            ));
        }

        if !yaw_lock && !device.capabilities.has_yaw_follow {
            return Err(LinkError::Unsupported(
                "the gimbal cannot follow the vehicle".to_owned(),
            ));
        }

        self.gimbal_command(vehicle, device_id, f64::NAN, f64::NAN, yaw_lock, timeout)
            .await
    }

    /// Points a gimbal at a place on the ground.
    ///
    /// # Errors
    ///
    /// Returns an error when the gimbal cannot point at a place, when the vehicle refuses
    /// the command, or when it does not answer.
    pub async fn set_gimbal_roi_location(
        &self,
        vehicle: VehicleId,
        device_id: u8,
        latitude: f64,
        longitude: f64,
        altitude: f64,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let device = reported_gimbal(self, device_id).ok_or_else(|| {
            LinkError::Unsupported("the vehicle has not said what its gimbal can do".to_owned())
        })?;

        if !device.capabilities.can_point_global {
            return Err(LinkError::Unsupported(
                "the gimbal cannot point at a place on the ground".to_owned(),
            ));
        }

        self.command_int_in_frame(
            vehicle,
            MavCmd::MAV_CMD_DO_SET_ROI_LOCATION,
            [
                i32::from(device_id),
                0,
                0,
                0,
                (latitude * 1e7) as i32,
                (longitude * 1e7) as i32,
                altitude as i32,
            ],
            MavFrame::MAV_FRAME_GLOBAL_TERRAIN_ALT,
            timeout,
        )
        .await
        .map(|_| ())
    }

    /// Takes the point of interest away from a gimbal.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command or does not answer.
    pub async fn clear_gimbal_roi(
        &self,
        vehicle: VehicleId,
        device_id: u8,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_DO_SET_ROI_NONE,
            [f32::from(device_id), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            timeout,
        )
        .await
        .map(|_| ())
    }

    /// Points a gimbal at another vehicle.
    ///
    /// # Errors
    ///
    /// Returns an error when the gimbal cannot point at a place, when the system id is
    /// larger than the command can carry exactly, when the vehicle refuses the command, or
    /// when it does not answer.
    pub async fn set_gimbal_roi_sysid(
        &self,
        vehicle: VehicleId,
        device_id: u8,
        system_id: u32,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let device = reported_gimbal(self, device_id).ok_or_else(|| {
            LinkError::Unsupported("the vehicle has not said what its gimbal can do".to_owned())
        })?;

        if !device.capabilities.can_point_global {
            return Err(LinkError::Unsupported(
                "the gimbal cannot point at a place on the ground".to_owned(),
            ));
        }

        // The system id travels in a float parameter, and a float stops being exact above
        // this number. Upstream refuses those, and MPCraft keeps the refusal.
        if system_id > 0x100_0000 {
            return Err(LinkError::Unsupported(
                "the system id is larger than the command can carry exactly".to_owned(),
            ));
        }

        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_DO_SET_ROI_SYSID,
            [
                system_id as f32,
                f32::from(device_id),
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

    /// Sends the command that points a gimbal, and waits for the answer.
    async fn gimbal_command(
        &self,
        vehicle: VehicleId,
        device_id: u8,
        pitch: f64,
        yaw: f64,
        yaw_lock: bool,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let flags = if yaw_lock {
            GimbalManagerFlags::GIMBAL_MANAGER_FLAGS_YAW_LOCK.bits()
        } else {
            0
        };

        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_DO_GIMBAL_MANAGER_PITCHYAW,
            [
                to_f32(pitch),
                to_f32(yaw),
                f32::NAN,
                f32::NAN,
                flags as f32,
                0.0,
                f32::from(device_id),
            ],
            timeout,
        )
        .await
        .map(|_| ())
    }
}

/// The gimbal the vehicle last reported, as the state published it.
fn reported_gimbal(session: &Session, requested: u8) -> Option<GimbalDevice> {
    let state = session.state();
    let devices: Vec<&GimbalDevice> = state.gimbals.iter().collect();
    resolve(&devices, requested).cloned()
}

/// A number as the command carries it.
const fn to_f32(value: f64) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::f32::consts::{FRAC_1_SQRT_2, FRAC_PI_2, FRAC_PI_6, PI};

    use mpcraft_mavlink::dialect::GimbalDeviceErrorFlags;

    fn information(device_id: u8, cap_flags: GimbalManagerCapFlags) -> GimbalDevice {
        let message = GIMBAL_MANAGER_INFORMATION_DATA {
            gimbal_device_id: device_id,
            cap_flags,
            pitch_min: -FRAC_PI_2,
            pitch_max: FRAC_PI_6,
            yaw_min: -PI,
            yaw_max: PI,
            ..Default::default()
        };

        GimbalDevice::of(171, &message)
    }

    #[test]
    fn a_quaternion_gives_the_angles_it_carries() {
        // The null rotation says nothing is turned.
        let null = euler_of(&[1.0, 0.0, 0.0, 0.0]);
        assert!((null.0 - 0.0).abs() < 1e-6);
        assert!((null.1 - 0.0).abs() < 1e-6);
        assert!((null.2 - 0.0).abs() < 1e-6);

        // A quarter turn around the yaw axis.
        let half = FRAC_1_SQRT_2;
        let yaw = euler_of(&[half, 0.0, 0.0, half]);
        // The quaternion is a float, so the angle it carries is not exact: the tolerance is
        // the size of the error the wire itself introduces.
        assert!((yaw.2 - 90.0).abs() < 0.02);

        // A quarter turn around the pitch axis.
        let pitch = euler_of(&[half, 0.0, half, 0.0]);
        assert!((pitch.1 - 90.0).abs() < 0.02);
    }

    #[test]
    fn an_angle_is_folded_into_the_range_the_command_carries() {
        assert_eq!(wrap_180(200.0), -160.0);
        assert_eq!(wrap_180(-200.0), 160.0);
        assert_eq!(wrap_180(45.0), 45.0);
    }

    #[test]
    fn the_limits_are_said_in_the_units_the_interface_reads() {
        let device = information(
            171,
            GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_CAN_POINT_LOCATION_LOCAL,
        );

        assert!((device.limits.pitch_min + 90.0).abs() < 0.01);
        assert!((device.limits.pitch_max - 30.0).abs() < 0.01);
    }

    #[test]
    fn a_gimbal_without_a_pitch_axis_refuses_a_pitch() {
        let device = information(
            171,
            GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_CAN_POINT_LOCATION_LOCAL
                | GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_HAS_YAW_AXIS,
        );

        assert_eq!(
            check_angles(&device, 10.0, 0.0, false),
            Some("the gimbal has no pitch axis".to_owned())
        );
    }

    #[test]
    fn an_angle_the_gimbal_cannot_reach_is_refused() {
        let device = information(
            171,
            GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_CAN_POINT_LOCATION_LOCAL
                | GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_HAS_PITCH_AXIS
                | GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_HAS_YAW_AXIS
                | GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_HAS_YAW_FOLLOW,
        );

        // The gimbal can pitch between -90 and 30 degrees: 45 is out of it, and -45 is not.
        assert!(check_limits(&device, 45.0, 0.0).is_some());
        assert!(check_limits(&device, -45.0, 0.0).is_none());
        assert!(check_angles(&device, -45.0, 0.0, false).is_none());
    }

    #[test]
    fn the_yaw_frame_is_inferred_from_the_lock_when_the_status_says_nothing() {
        let mut registry = GimbalRegistry::new();
        let message = GIMBAL_MANAGER_INFORMATION_DATA {
            gimbal_device_id: 171,
            cap_flags: GimbalManagerCapFlags::default(),
            ..Default::default()
        };
        registry.observe_information(171, &message);

        // A status with only the lock flag: the yaw is held against the ground, so it is not
        // read in the frame of the vehicle.
        let status = GIMBAL_MANAGER_STATUS_DATA {
            gimbal_device_id: 171,
            flags: GimbalManagerFlags::GIMBAL_MANAGER_FLAGS_YAW_LOCK,
            ..Default::default()
        };
        registry.observe_status(&status);

        assert!(!registry.yaw_in_vehicle_frame(171));
    }

    #[test]
    fn all_gimbals_mean_the_lowest_one() {
        let mut registry = GimbalRegistry::new();

        for device_id in [174_u8, 171_u8] {
            let message = GIMBAL_MANAGER_INFORMATION_DATA {
                gimbal_device_id: device_id,
                ..Default::default()
            };
            registry.observe_information(171, &message);
        }

        assert_eq!(
            registry.device_for(0).map(|device| device.device_id),
            Some(171)
        );
        assert_eq!(
            registry.device_for(174).map(|device| device.device_id),
            Some(174)
        );
    }

    #[test]
    fn the_attitude_of_a_gimbal_is_kept_with_the_reported_component() {
        let mut registry = GimbalRegistry::new();
        let message = GIMBAL_MANAGER_INFORMATION_DATA {
            gimbal_device_id: 171,
            ..Default::default()
        };
        registry.observe_information(171, &message);

        let half = FRAC_1_SQRT_2;
        let attitude = GIMBAL_DEVICE_ATTITUDE_STATUS_DATA {
            gimbal_device_id: 171,
            q: [half, 0.0, 0.0, half],
            failure_flags: GimbalDeviceErrorFlags::GIMBAL_DEVICE_ERROR_FLAGS_ENCODER_ERROR,
            ..Default::default()
        };
        registry.observe_attitude(&attitude);

        let devices = registry.devices();
        let device = devices.first().expect("the gimbal is known");

        // The quaternion is a quarter turn around the yaw axis, and the encoder error says
        // something is wrong with the device.
        let yaw = device
            .attitude
            .map(|attitude| attitude.yaw)
            .unwrap_or_default();
        assert!((yaw - 90.0).abs() < 0.02);
        assert!(device.failed);
    }
}
