//! The operations the interface can ask of a vehicle.
//!
//! The operations live here rather than in the shell, so that the rule that a command
//! needs an open link is written once and tested without a display server. The shell
//! wires them to the interface and does nothing else.

use std::sync::Arc;
use std::time::Duration;

use mpcraft_link::{
    zoom_options, zoom_type_of, Calibration, CameraDevice, FtpClient, FtpEntry, GimbalDevice,
    HomeRequest, OsdSlot, OsdSlotConfig, PartialUpdate, PartialUpdatePolicy, Session, VehicleId,
    VideoStream, ZoomOption,
};
use mpcraft_mavlink::MavCmd;
use mpcraft_mission::{MissionItem, MissionType, Plan};
use mpcraft_vehicle::GeoPoint;

use crate::error::{CommandError, CommandResult};
use crate::supervisor::SessionSupervisor;

/// How long to wait for the vehicle to answer a command.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// How long to wait for the vehicle to say where its home is.
///
/// The answer is one message, and upstream waits less than a second for it.
const HOME_TIMEOUT: Duration = Duration::from_millis(700);

/// How long to wait for a plan to be transferred.
///
/// A plan is many messages, and the vehicle asks for them one at a time, so the
/// allowance is generous.
const PLAN_TIMEOUT: Duration = Duration::from_secs(30);

/// How many bytes are asked for at a time when a file is read over the file transfer.
///
/// The payload of a message carries 239 bytes of data, and fewer are asked for so that
/// the answer fits in one frame.
const FTP_BLOCK: u8 = 230;

/// The screens of the on-screen display whose slots the operator chooses.
///
/// The screens one to four are fixed by the vehicle; the fifth and the sixth are the
/// ones that are configured.
const OSD_SCREENS: [u8; 2] = [5, 6];

/// How many slots each of those screens has.
const OSD_SLOTS: u8 = 9;

/// How long to wait for the vehicle to answer a question about a slot.
const OSD_TIMEOUT: Duration = Duration::from_secs(3);

/// How often a camera says what it is doing.
///
/// The reports are asked for so that the sheet shows the mode the camera is in without the
/// operator asking again; one a second is enough for a mode and a capture state.
const CAMERA_REPORT_RATE: u32 = 1;

/// The reason an angle is not one the command can carry, or nothing when it is.
fn check_degrees(value: f64, what: &str) -> CommandResult<()> {
    if !value.is_finite() || value < -180.0 || value > 180.0 {
        return Err(CommandError::invalid_request(format!(
            "{what} is not an angle between -180 and 180 degrees"
        )));
    }

    Ok(())
}

/// The reason a place is outside the picture, or nothing when it is not.
///
/// The camera a command is addressed to.
///
/// A camera answers on its own component, and the registry keeps it under that component, so
/// a command for a camera goes to the component that reported it rather than to the autopilot.
fn camera_of(vehicle: VehicleId, camera_component: u8) -> VehicleId {
    VehicleId::new(vehicle.system_id, camera_component)
}

/// The picture is read from `-1` to `1`: `-1` is the left or the top, `1` is the right or
/// the bottom. A number outside that range is not a place in the picture.
fn check_picture_place(value: f64, what: &str) -> CommandResult<()> {
    if !value.is_finite() || value < -1.0 || value > 1.0 {
        return Err(CommandError::invalid_request(format!(
            "{what} is outside the picture, which reads from -1 to 1"
        )));
    }

    Ok(())
}

impl SessionSupervisor {
    /// The session that is open, if there is one.
    fn open_session(&self) -> CommandResult<Arc<Session>> {
        self.session().ok_or_else(CommandError::not_connected)
    }

    /// Arms the vehicle.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the vehicle refuses the
    /// command or does not answer.
    pub async fn arm(&self, vehicle: VehicleId) -> CommandResult<()> {
        let session = self.open_session()?;
        session.arm(vehicle, COMMAND_TIMEOUT).await?;
        Ok(())
    }

    /// Disarms the vehicle.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the vehicle refuses the
    /// command or does not answer.
    pub async fn disarm(&self, vehicle: VehicleId) -> CommandResult<()> {
        let session = self.open_session()?;
        session.disarm(vehicle, COMMAND_TIMEOUT).await?;
        Ok(())
    }

    /// Takes off to an altitude, relative to the home position.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the vehicle refuses the
    /// command or does not answer.
    pub async fn takeoff(&self, vehicle: VehicleId, altitude: f32) -> CommandResult<()> {
        let session = self.open_session()?;
        session.takeoff(vehicle, altitude, COMMAND_TIMEOUT).await?;
        Ok(())
    }

    /// Changes the flight mode.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the vehicle refuses the
    /// command or does not answer.
    pub async fn set_mode(&self, vehicle: VehicleId, custom_mode: u32) -> CommandResult<()> {
        let session = self.open_session()?;
        session
            .set_mode(
                vehicle,
                custom_mode,
                mpcraft_mavlink::MavModeFlag::empty(),
                COMMAND_TIMEOUT,
            )
            .await?;
        Ok(())
    }

    /// Asks the vehicle to write its parameters to its own memory.
    ///
    /// A parameter the vehicle has in its memory but not in the flash is lost when the
    /// vehicle loses power, and the vehicle only writes the flash when it is told to.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the vehicle refuses the
    /// command or does not answer.
    pub async fn write_parameters_to_flash(&self, vehicle: VehicleId) -> CommandResult<()> {
        let session = self.open_session()?;

        // The first parameter of the command says what to do: one is to write the
        // settings of the vehicle to its own memory.
        session
            .command_long(
                vehicle,
                MavCmd::MAV_CMD_PREFLIGHT_STORAGE,
                [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                COMMAND_TIMEOUT,
            )
            .await?;

        Ok(())
    }

    /// What the outputs of the vehicle are for.
    ///
    /// The function of each output is read from the parameters the vehicle has reported.
    /// An output whose function the vehicle has not said has no function here: it is
    /// said as a number, or not at all.
    pub fn output_functions(&self) -> CommandResult<Vec<OutputFunction>> {
        let Some(session) = self.session() else {
            return Err(CommandError::not_connected());
        };

        let state = session.state();
        let mut functions = Vec::new();

        for output in 1..=16_u32 {
            let name = format!("SERVO{output}_FUNCTION");

            let Some(parameter) = state.parameters.get(&name) else {
                continue;
            };

            functions.push(OutputFunction {
                output,
                value: parameter.value,
                name: mpcraft_params::functions::servo_function_name(parameter.value)
                    .map(str::to_owned),
                motor: mpcraft_params::functions::motor_number(parameter.value),
            });
        }

        Ok(functions)
    }

    /// Runs the motors of the vehicle, to see whether they turn.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the vehicle refuses the
    /// command or does not answer.
    pub async fn motor_test(
        &self,
        vehicle: VehicleId,
        motor: u8,
        throttle: f32,
        seconds: f32,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .motor_test(vehicle, motor, throttle, seconds, COMMAND_TIMEOUT)
            .await?;

        Ok(())
    }

    /// Asks the vehicle to calibrate one of its sensors.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the vehicle refuses the
    /// command or does not answer.
    pub async fn calibrate(
        &self,
        vehicle: VehicleId,
        calibration: Calibration,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .calibrate(vehicle, calibration, COMMAND_TIMEOUT)
            .await?;

        Ok(())
    }

    /// Turns one of the relays of the vehicle on or off.
    ///
    /// The refusal of a relay the vehicle has said it does not have happens in the link, which is
    /// where the report of the relays is kept; this layer only requires an open link, so that the
    /// rule is written once.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the vehicle has said it does not have the
    /// relay, or when it refuses the command or does not answer.
    pub async fn set_relay(
        &self,
        vehicle: VehicleId,
        instance: u16,
        on: bool,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .set_relay(vehicle, instance, on, COMMAND_TIMEOUT)
            .await?;

        Ok(())
    }

    /// Asks the vehicle once for the report of its relays.
    ///
    /// The report is not streamed unless it is asked for, so the sheet has to ask before it can show
    /// what the vehicle says about its relays. What the sheet shows afterwards is the state the event
    /// carries rather than this answer: the request only makes the vehicle say something.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the vehicle does not answer in time.
    pub async fn relay_status(&self, vehicle: VehicleId) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .request_relay_status(vehicle, COMMAND_TIMEOUT)
            .await?;

        Ok(())
    }

    /// The logs the vehicle holds in its own memory.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the vehicle does not answer.
    pub async fn vehicle_logs(&self, vehicle: VehicleId) -> CommandResult<Vec<VehicleLog>> {
        let session = self.open_session()?;

        let entries = mpcraft_link::logs::list_logs(&session, vehicle, COMMAND_TIMEOUT).await?;

        Ok(entries
            .into_iter()
            .map(|entry| VehicleLog {
                id: entry.id,
                time_utc: entry.time_utc,
                size: entry.size,
            })
            .collect())
    }

    /// Fetches a log of the vehicle and writes it to a file.
    ///
    /// The log is fetched in pieces and put together as it arrives, and the file is
    /// written when every piece is there.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the vehicle does not answer,
    /// or when the file cannot be written.
    pub async fn download_log(
        &self,
        vehicle: VehicleId,
        id: u16,
        size: u32,
        path: &std::path::Path,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        let bytes =
            mpcraft_link::logs::read_log(&session, vehicle, id, size, COMMAND_TIMEOUT).await?;

        std::fs::write(path, bytes).map_err(|error| {
            CommandError::internal(format!("cannot write {}: {error}", path.display()))
        })?;

        Ok(())
    }

    /// Takes the radio channels over, one at a time.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the link cannot carry the
    /// message.
    pub async fn rc_override(
        &self,
        vehicle: VehicleId,
        channels: [Option<u16>; 8],
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session.rc_override(vehicle, channels).await?;

        Ok(())
    }

    /// Lets go of the radio channels.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the link cannot carry the
    /// message.
    pub async fn rc_override_release(&self, vehicle: VehicleId) -> CommandResult<()> {
        let session = self.open_session()?;

        session.rc_override_release(vehicle).await?;

        Ok(())
    }

    /// Flies the vehicle with the sticks of the ground station.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the link cannot carry the
    /// message.
    pub async fn manual_control(
        &self,
        vehicle: VehicleId,
        roll: f32,
        pitch: f32,
        thrust: f32,
        yaw: f32,
        buttons: u16,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .manual_control(vehicle, roll, pitch, thrust, yaw, buttons)
            .await?;

        Ok(())
    }

    /// Tells the vehicle to go to a place.
    ///
    /// This is the guided mode: the vehicle flies to the place and stays there. The
    /// vehicle has to be in the guided mode for it to obey.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the link cannot carry the
    /// message.
    pub async fn set_position_target(
        &self,
        vehicle: VehicleId,
        latitude: f64,
        longitude: f64,
        altitude: f32,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .set_position_target(vehicle, latitude, longitude, altitude)
            .await?;

        Ok(())
    }

    /// Writes bytes to a serial port of the vehicle.
    ///
    /// The vehicle has several serial ports ? the telemetry ones, the ones of the GPS ? and
    /// this is how a program reaches them: the bytes travel inside a MAVLink message and
    /// the vehicle hands them over to the port.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the link cannot carry the
    /// message.
    pub async fn serial_control(
        &self,
        vehicle: VehicleId,
        device: u8,
        data: &[u8],
        baudrate: u32,
        timeout: u16,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .serial_control(vehicle, device, data, baudrate, timeout)
            .await?;

        Ok(())
    }

    /// Asks the vehicle to restart.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the link cannot carry the
    /// command.
    pub async fn reboot(&self, vehicle: VehicleId) -> CommandResult<()> {
        let session = self.open_session()?;

        session.reboot(vehicle).await?;

        Ok(())
    }

    /// Reboots the vehicle into its bootloader, so that a firmware can be written to it.
    ///
    /// The command is one ArduPilot gives to itself — `MAV_CMD_FLASH_BOOTLOADER`, whose
    /// number is larger than the commands the protocol library knows — so it travels the
    /// raw way.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the vehicle refuses the command,
    /// or when the vehicle does not answer.
    pub async fn flash_bootloader(&self, vehicle: VehicleId) -> CommandResult<()> {
        let session = self.open_session()?;

        session.flash_bootloader(vehicle, COMMAND_TIMEOUT).await?;

        Ok(())
    }

    /// Lists the files the vehicle holds, over the file transfer protocol.
    ///
    /// The vehicle answers with the entries of the folder: the name of each one, whether
    /// it is a folder, and how big it is.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the vehicle refuses the request,
    /// or when the vehicle does not answer.
    pub async fn ftp_list(&self, path: &str) -> CommandResult<Vec<FtpEntry>> {
        let session = self.open_session()?;

        let client = FtpClient::new(session.clone());

        Ok(client.list_directory(path).await?)
    }

    /// Asks the vehicle what every slot of the on-screen display holds.
    ///
    /// The screens five and six are the ones the operator chooses: each of them has
    /// nine slots, and each slot holds one thing.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the link cannot carry the
    /// question.
    pub async fn osd_slots(&self, vehicle: VehicleId) -> CommandResult<Vec<OsdSlot>> {
        let session = self.open_session()?;

        Ok(session.osd_slots(vehicle, &OSD_SCREENS, OSD_SLOTS).await?)
    }

    /// Tells the vehicle what a slot of the on-screen display is to hold.
    ///
    /// The slot holds a parameter, and the range of the parameter is said with it.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the vehicle refuses the
    /// change, or when the vehicle does not answer.
    pub async fn osd_slot_config(
        &self,
        vehicle: VehicleId,
        config: OsdSlotConfig,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .osd_slot_config(vehicle, config, OSD_TIMEOUT)
            .await?;

        Ok(())
    }

    /// Reads a file the vehicle holds, over the file transfer protocol.    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the vehicle refuses the request,
    /// or when the vehicle does not answer.
    pub async fn ftp_read(&self, path: &str) -> CommandResult<Vec<u8>> {
        let session = self.open_session()?;

        let client = FtpClient::new(session.clone());

        let (session_id, size) = client.open_file_read(path).await?;

        let mut content = Vec::new();
        let mut offset: u32 = 0;

        // The vehicle answers with at most 239 bytes at a time.
        while offset < size {
            let block = client.read_file(session_id, offset, FTP_BLOCK).await?;

            if block.is_empty() {
                break;
            }

            offset += block.len() as u32;
            content.extend_from_slice(&block);
        }

        let _ = client.terminate_session(session_id).await;

        Ok(content)
    }

    /// Asks the camera of the vehicle what video streams it has.
    ///
    /// The camera answers with a URI, and the size and frame rate of each stream. The
    /// video travels outside MAVLink, over that URI.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the camera does not answer,
    /// or when the link cannot carry the request.
    pub async fn video_streams(&self, vehicle: VehicleId) -> CommandResult<Vec<VideoStream>> {
        let session = self.open_session()?;

        Ok(session.request_video_streams(vehicle).await?)
    }

    /// Starts one of the camera's video streams.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the vehicle refuses the
    /// command, or when it does not answer.
    pub async fn video_stream_start(
        &self,
        vehicle: VehicleId,
        component_id: u8,
        stream_id: u8,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .start_video_stream(vehicle, component_id, stream_id, COMMAND_TIMEOUT)
            .await?;

        Ok(())
    }

    /// Stops one of the camera's video streams.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the vehicle refuses the
    /// command, or when it does not answer.
    pub async fn video_stream_stop(
        &self,
        vehicle: VehicleId,
        component_id: u8,
        stream_id: u8,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .stop_video_stream(vehicle, component_id, stream_id, COMMAND_TIMEOUT)
            .await?;

        Ok(())
    }

    /// Asks the vehicle what its gimbals can do.
    ///
    /// What the gimbal says about itself decides which controls the sheet offers: an angle
    /// is refused before anything is sent, because a device without a pitch axis cannot be
    /// pitched.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the vehicle does not answer, or
    /// when the link cannot carry the request.
    pub async fn gimbal_devices(&self, vehicle: VehicleId) -> CommandResult<Vec<GimbalDevice>> {
        let session = self.open_session()?;

        Ok(session.request_gimbal_information(vehicle).await?)
    }

    /// Points a gimbal at an angle.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the angle is not a number the
    /// command can carry, when the gimbal cannot do this, when the vehicle refuses the
    /// command, or when it does not answer.
    pub async fn set_gimbal_angles(
        &self,
        vehicle: VehicleId,
        device_id: u8,
        pitch: f64,
        yaw: f64,
        yaw_lock: bool,
    ) -> CommandResult<()> {
        check_degrees(pitch, "the pitch")?;
        check_degrees(yaw, "the yaw")?;

        let session = self.open_session()?;

        session
            .set_gimbal_angles(vehicle, device_id, pitch, yaw, yaw_lock, COMMAND_TIMEOUT)
            .await?;

        Ok(())
    }

    /// Moves a gimbal at a rate, without waiting for the vehicle to say it took it.
    ///
    /// This is the path a joystick drives: the message goes out at the rate the stick moves,
    /// and the vehicle does not answer it.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when a number is not one the command can
    /// carry, or when the gimbal cannot do this.
    pub async fn slew_gimbal(
        &self,
        vehicle: VehicleId,
        device_id: u8,
        pitch: f64,
        yaw: f64,
        pitch_rate: f64,
        yaw_rate: f64,
    ) -> CommandResult<()> {
        check_degrees(pitch, "the pitch")?;
        check_degrees(yaw, "the yaw")?;

        if !pitch_rate.is_finite() || !yaw_rate.is_finite() {
            return Err(CommandError::invalid_request(
                "the rates of the gimbal are not numbers",
            ));
        }

        let session = self.open_session()?;

        session
            .set_gimbal_angles_now(vehicle, device_id, pitch, yaw, pitch_rate, yaw_rate)
            .await?;

        Ok(())
    }

    /// Puts a gimbal in its safe position.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the gimbal cannot be retracted,
    /// when the vehicle refuses the command, or when it does not answer.
    pub async fn retract_gimbal(&self, vehicle: VehicleId, device_id: u8) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .retract_gimbal(vehicle, device_id, COMMAND_TIMEOUT)
            .await?;

        Ok(())
    }

    /// Puts a gimbal in its default position.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the gimbal cannot go to neutral,
    /// when the vehicle refuses the command, or when it does not answer.
    pub async fn neutral_gimbal(&self, vehicle: VehicleId, device_id: u8) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .neutral_gimbal(vehicle, device_id, COMMAND_TIMEOUT)
            .await?;

        Ok(())
    }

    /// Says whether the yaw of a gimbal is held against the ground or follows the vehicle.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the gimbal cannot do this, when the
    /// vehicle refuses the command, or when it does not answer.
    pub async fn set_gimbal_yaw_lock(
        &self,
        vehicle: VehicleId,
        device_id: u8,
        yaw_lock: bool,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .set_gimbal_yaw_lock(vehicle, device_id, yaw_lock, COMMAND_TIMEOUT)
            .await?;

        Ok(())
    }

    /// Points a gimbal at a place on the ground.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the place is not a place, when the
    /// gimbal cannot point at one, when the vehicle refuses the command, or when it does not
    /// answer.
    pub async fn point_gimbal_at_location(
        &self,
        vehicle: VehicleId,
        device_id: u8,
        latitude: f64,
        longitude: f64,
        altitude: f64,
    ) -> CommandResult<()> {
        if !latitude.is_finite() || !longitude.is_finite() || !altitude.is_finite() {
            return Err(CommandError::invalid_request(
                "the point of interest is not a place",
            ));
        }

        let session = self.open_session()?;

        session
            .set_gimbal_roi_location(
                vehicle,
                device_id,
                latitude,
                longitude,
                altitude,
                COMMAND_TIMEOUT,
            )
            .await?;

        Ok(())
    }

    /// Takes the point of interest away from a gimbal.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the vehicle refuses the command, or
    /// when it does not answer.
    pub async fn clear_gimbal_roi(&self, vehicle: VehicleId, device_id: u8) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .clear_gimbal_roi(vehicle, device_id, COMMAND_TIMEOUT)
            .await?;

        Ok(())
    }

    /// Points a gimbal at another vehicle.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the system id is larger than the
    /// command can carry exactly, when the gimbal cannot point at a place, when the vehicle
    /// refuses the command, or when it does not answer.
    pub async fn point_gimbal_at_vehicle(
        &self,
        vehicle: VehicleId,
        device_id: u8,
        system_id: u32,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .set_gimbal_roi_sysid(vehicle, device_id, system_id, COMMAND_TIMEOUT)
            .await?;

        Ok(())
    }

    /// Asks the vehicle what its cameras can do, and asks them to say what they are doing.
    ///
    /// The camera answers with who made it, which camera it is, and what it can be asked to
    /// do. The reports are then kept coming, so that the sheet shows the mode the camera is
    /// in without the operator asking again.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the camera does not answer, or when
    /// the vehicle refuses one of the commands.
    pub async fn read_camera(&self, vehicle: VehicleId) -> CommandResult<Vec<CameraDevice>> {
        let session = self.open_session()?;

        let cameras = session.request_camera_information(vehicle).await?;

        for camera in &cameras {
            session
                .watch_camera(
                    camera_of(vehicle, camera.component_id),
                    camera,
                    CAMERA_REPORT_RATE,
                    COMMAND_TIMEOUT,
                )
                .await?;
        }

        Ok(cameras)
    }

    /// The kinds of zoom a camera can be asked for.
    ///
    /// The interface does not know the numbers the protocol uses: it asks for the list, and
    /// sends back one of the values it was given.
    #[must_use]
    pub fn zoom_options(&self) -> Vec<ZoomOption> {
        zoom_options()
    }

    /// Asks a camera to take one picture.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the camera cannot take pictures,
    /// when the vehicle refuses the command, or when it does not answer.
    pub async fn capture_photo(
        &self,
        vehicle: VehicleId,
        camera_component: u8,
        camera_index: u8,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .capture_photo(
                camera_of(vehicle, camera_component),
                camera_index,
                COMMAND_TIMEOUT,
            )
            .await?;

        Ok(())
    }

    /// Asks a camera to take pictures at an interval.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the interval is not a positive
    /// number of seconds, when the camera cannot take pictures, when the vehicle refuses the
    /// command, or when it does not answer.
    pub async fn start_interval_capture(
        &self,
        vehicle: VehicleId,
        camera_component: u8,
        camera_index: u8,
        interval_seconds: f64,
    ) -> CommandResult<()> {
        if !interval_seconds.is_finite() || interval_seconds <= 0.0 {
            return Err(CommandError::invalid_request(
                "the interval between pictures is not a number of seconds",
            ));
        }

        let session = self.open_session()?;

        session
            .start_interval_capture(
                camera_of(vehicle, camera_component),
                camera_index,
                interval_seconds,
                COMMAND_TIMEOUT,
            )
            .await?;

        Ok(())
    }

    /// Stops the pictures a camera was taking at an interval.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the camera cannot take pictures,
    /// when the vehicle refuses the command, or when it does not answer.
    pub async fn stop_interval_capture(
        &self,
        vehicle: VehicleId,
        camera_component: u8,
        camera_index: u8,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .stop_interval_capture(
                camera_of(vehicle, camera_component),
                camera_index,
                COMMAND_TIMEOUT,
            )
            .await?;

        Ok(())
    }

    /// Asks a camera to start recording video.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the camera cannot record video, when
    /// the vehicle refuses the command, or when it does not answer.
    pub async fn start_recording(
        &self,
        vehicle: VehicleId,
        camera_component: u8,
        stream_id: u8,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .start_recording(
                camera_of(vehicle, camera_component),
                stream_id,
                COMMAND_TIMEOUT,
            )
            .await?;

        Ok(())
    }

    /// Stops the video a camera was recording.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the camera cannot record video, when
    /// the vehicle refuses the command, or when it does not answer.
    pub async fn stop_recording(
        &self,
        vehicle: VehicleId,
        camera_component: u8,
        stream_id: u8,
    ) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .stop_recording(
                camera_of(vehicle, camera_component),
                stream_id,
                COMMAND_TIMEOUT,
            )
            .await?;

        Ok(())
    }

    /// Sets the zoom of a camera.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the level is not a number, when the
    /// kind of zoom is one the protocol does not know, when the camera cannot be zoomed, when
    /// the vehicle refuses the command, or when it does not answer.
    pub async fn set_camera_zoom(
        &self,
        vehicle: VehicleId,
        camera_component: u8,
        level: f64,
        zoom_type: u32,
    ) -> CommandResult<()> {
        if !level.is_finite() {
            return Err(CommandError::invalid_request(
                "the zoom level is not a number",
            ));
        }

        let kind = zoom_type_of(zoom_type).ok_or_else(|| {
            CommandError::invalid_request("the kind of zoom is not one the protocol knows")
        })?;

        let session = self.open_session()?;

        session
            .set_camera_zoom(
                camera_of(vehicle, camera_component),
                level,
                kind,
                COMMAND_TIMEOUT,
            )
            .await?;

        Ok(())
    }

    /// Asks a camera to follow a point in its picture.
    ///
    /// The place is given from `-1` to `1`, which is how the interface shows it. Drawing the
    /// picture so that the operator can pick a place from it waits on the video player.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the place is outside the picture,
    /// when the camera cannot follow a point, when the vehicle refuses the command, or when
    /// it does not answer.
    pub async fn track_point(
        &self,
        vehicle: VehicleId,
        camera_component: u8,
        x: f64,
        y: f64,
    ) -> CommandResult<()> {
        check_picture_place(x, "the horizontal position")?;
        check_picture_place(y, "the vertical position")?;

        let session = self.open_session()?;

        session
            .track_point(camera_of(vehicle, camera_component), x, y, COMMAND_TIMEOUT)
            .await?;

        Ok(())
    }

    /// Asks a camera to follow an area of its picture.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when a corner is outside the picture,
    /// when the camera cannot follow an area, when the vehicle refuses the command, or when
    /// it does not answer.
    pub async fn track_rectangle(
        &self,
        vehicle: VehicleId,
        camera_component: u8,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    ) -> CommandResult<()> {
        for (value, what) in [
            (x1, "the first corner"),
            (y1, "the first corner"),
            (x2, "the second corner"),
            (y2, "the second corner"),
        ] {
            check_picture_place(value, what)?;
        }

        let session = self.open_session()?;

        session
            .track_rectangle(
                camera_of(vehicle, camera_component),
                x1,
                y1,
                x2,
                y2,
                COMMAND_TIMEOUT,
            )
            .await?;

        Ok(())
    }

    /// Downloads the plan the vehicle keeps in a file of its own.
    ///
    /// The plan is not asked for with the mission protocol: it is read from the file
    /// the vehicle keeps it in, over the file transfer protocol. It is the way a plan is
    /// carried when the vehicle does not want to talk the mission protocol.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the vehicle refuses the
    /// request, or when the file does not hold a plan.
    pub async fn download_plan_ftp(&self, mission_type: MissionType) -> CommandResult<Plan> {
        let session = self.open_session()?;

        Ok(mpcraft_link::mission_ftp::download_plan(&session, mission_type).await?)
    }

    /// Writes a plan to the file the vehicle keeps it in.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the vehicle refuses the
    /// request, or when the vehicle does not answer.
    pub async fn upload_plan_ftp(&self, plan: &Plan) -> CommandResult<()> {
        let session = self.open_session()?;

        mpcraft_link::mission_ftp::upload_plan(&session, plan).await?;

        Ok(())
    }

    /// Asks the vehicle where its home is.
    ///
    /// The vehicle is asked with the command that upstream sends, and the answer is the
    /// `HOME_POSITION` message the vehicle sends.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the vehicle does not answer
    /// in time.
    pub async fn home_position(&self, vehicle: VehicleId) -> CommandResult<GeoPoint> {
        let session = self.open_session()?;

        let home = session
            .request_home_position(vehicle, HomeRequest::Command, HOME_TIMEOUT)
            .await?;

        Ok(home)
    }

    /// Moves the home of the vehicle.
    ///
    /// The vehicle is told where the home is, and it confirms the move by reporting the
    /// home it now holds. A vehicle that refuses the move keeps the one it had, and the
    /// error says so.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the vehicle does not confirm
    /// the move, or when it reports a different home.
    pub async fn set_home_position(&self, vehicle: VehicleId, home: GeoPoint) -> CommandResult<()> {
        let session = self.open_session()?;

        session
            .set_home_position(vehicle, home, HOME_TIMEOUT)
            .await?;

        Ok(())
    }

    /// Downloads the plan the vehicle holds.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the vehicle does not answer.
    pub async fn download_mission(
        &self,
        vehicle: VehicleId,
        mission_type: MissionType,
    ) -> CommandResult<Plan> {
        let session = self.open_session()?;
        Ok(session
            .download_mission(vehicle, mission_type, PLAN_TIMEOUT)
            .await?)
    }

    /// Uploads a plan to the vehicle.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the vehicle refuses the plan.
    pub async fn upload_mission(&self, vehicle: VehicleId, plan: &Plan) -> CommandResult<()> {
        let session = self.open_session()?;
        session.upload_mission(vehicle, plan, PLAN_TIMEOUT).await?;
        Ok(())
    }

    /// Writes a part of a plan to the vehicle, leaving the rest of it alone.
    ///
    /// The vehicle is told which range of the plan is coming, and it asks for the items
    /// one at a time. The answer says which items the vehicle took.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, when the range is refused, or when
    /// the vehicle does not answer.
    pub async fn upload_mission_partial(
        &self,
        vehicle: VehicleId,
        mission_type: MissionType,
        start: u16,
        items: &[MissionItem],
    ) -> CommandResult<PartialUpdate> {
        let session = self.open_session()?;

        let report = session
            .upload_mission_partial(
                vehicle,
                mission_type,
                start,
                items,
                PartialUpdatePolicy::default(),
            )
            .await?;

        Ok(report)
    }

    /// Says which item of the plan the vehicle is executing.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the vehicle does not answer.
    pub async fn set_current_item(&self, vehicle: VehicleId, seq: u16) -> CommandResult<()> {
        let session = self.open_session()?;
        session
            .set_current_item(vehicle, seq, COMMAND_TIMEOUT)
            .await?;
        Ok(())
    }

    /// Empties one of the vehicle's plans.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the vehicle refuses.
    pub async fn clear_mission(
        &self,
        vehicle: VehicleId,
        mission_type: MissionType,
    ) -> CommandResult<()> {
        let session = self.open_session()?;
        session
            .clear_mission(vehicle, mission_type, COMMAND_TIMEOUT)
            .await?;
        Ok(())
    }

    /// Makes the vehicle jump to an item of the plan.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no open link, or when the vehicle refuses the
    /// command or does not answer.
    pub async fn jump_to_item(&self, vehicle: VehicleId, seq: u16) -> CommandResult<()> {
        let session = self.open_session()?;
        session.jump_to_item(vehicle, seq, COMMAND_TIMEOUT).await?;
        Ok(())
    }
}

/// The flight modes the interface offers.
///
/// # Errors
///
/// Never fails.
pub fn flight_modes() -> Vec<ModeInfo> {
    mpcraft_vehicle::modes::COMMON_MODES
        .iter()
        .map(|entry| ModeInfo {
            value: entry.value,
            name: entry.name.to_owned(),
        })
        .collect()
}

/// A flight mode, as the interface sees it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeInfo {
    /// The custom mode value the vehicle expects.
    pub value: u32,
    /// The name of the mode.
    pub name: String,
}

/// The kind of plan, as the interface names it.
///
/// The wire has three kinds of plan, and the interface names them the way an operator
/// does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlanKind {
    /// The plan the vehicle flies.
    Mission,
    /// The geofence.
    Fence,
    /// The rally points.
    Rally,
}

impl From<PlanKind> for MissionType {
    fn from(kind: PlanKind) -> Self {
        match kind {
            PlanKind::Mission => Self::Mission,
            PlanKind::Fence => Self::Fence,
            PlanKind::Rally => Self::Rally,
        }
    }
}

/// The item of a plan, as the interface sees it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    /// The position in the plan.
    pub seq: u16,
    /// The name of the command, for example `MAV_CMD_NAV_WAYPOINT`.
    pub command: String,
    /// The numeric command value.
    pub command_value: u32,
    /// The coordinate frame the position is expressed in.
    pub frame: u32,
    /// The four generic parameters.
    pub params: [f32; 4],
    /// The latitude, for a command that carries a position.
    pub x: f64,
    /// The longitude, for a command that carries a position.
    pub y: f64,
    /// The altitude.
    pub z: f64,
    /// Whether the item is the one being executed.
    pub current: bool,
    /// Whether the vehicle continues to the next item automatically.
    pub autocontinue: bool,
}

/// What the outputs of the vehicle are for.
///
/// The function of each output is told by a parameter — `SERVO1_FUNCTION` and its
/// brothers — and the number is named the way the specification names it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputFunction {
    /// The output, counted from one.
    pub output: u32,
    /// The number the parameter carries.
    pub value: f64,
    /// The name of the function, when the specification gives it one.
    pub name: Option<String>,
    /// The motor the function drives, when it drives one.
    pub motor: Option<u32>,
}

/// A plan, as the interface sees it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanInfo {
    /// The kind of plan.
    pub kind: PlanKind,
    /// The items, in order.
    pub items: Vec<PlanItem>,
}

impl From<&Plan> for PlanInfo {
    fn from(plan: &Plan) -> Self {
        let kind = match plan.mission_type {
            MissionType::Mission => PlanKind::Mission,
            MissionType::Fence => PlanKind::Fence,
            MissionType::Rally => PlanKind::Rally,
        };

        let items = plan
            .items
            .iter()
            .enumerate()
            .map(|(seq, item)| PlanItem {
                seq: seq as u16,
                command: mpcraft_mission::mavcmd::command_name(item.command),
                command_value: item.command as u32,
                frame: item.frame,
                params: item.params,
                x: item.x,
                y: item.y,
                z: item.z,
                current: item.current,
                autocontinue: item.autocontinue,
            })
            .collect();

        Self { kind, items }
    }
}

/// One of the logs a vehicle holds.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VehicleLog {
    /// The number of the log, which is what the data is asked for with.
    pub id: u16,
    /// When the log was started, in seconds since the epoch.
    pub time_utc: u32,
    /// How many bytes the log holds.
    pub size: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vehicle the tests address.
    const VEHICLE: VehicleId = VehicleId::new(1, 1);

    #[tokio::test]
    async fn an_operation_without_a_link_says_so() {
        let supervisor = SessionSupervisor::new();

        let error = supervisor
            .arm(VEHICLE)
            .await
            .expect_err("there is no link open");

        assert_eq!(error.code, CommandError::CODE_NOT_CONNECTED);
    }

    #[tokio::test]
    async fn every_operation_needs_a_link() {
        let supervisor = SessionSupervisor::new();

        let failures = [
            supervisor.arm(VEHICLE).await.err(),
            supervisor.disarm(VEHICLE).await.err(),
            supervisor.takeoff(VEHICLE, 10.0).await.err(),
            supervisor.set_mode(VEHICLE, 4).await.err(),
            supervisor
                .download_mission(VEHICLE, MissionType::Mission)
                .await
                .err(),
            supervisor
                .upload_mission(VEHICLE, &Plan::new(MissionType::Mission))
                .await
                .err(),
            supervisor.set_current_item(VEHICLE, 0).await.err(),
            supervisor
                .clear_mission(VEHICLE, MissionType::Mission)
                .await
                .err(),
            supervisor.jump_to_item(VEHICLE, 0).await.err(),
        ];

        for failure in failures.into_iter().flatten() {
            assert_eq!(failure.code, CommandError::CODE_NOT_CONNECTED);
        }
    }

    #[test]
    fn the_flight_modes_are_the_ones_the_autopilot_uses() {
        let modes = flight_modes();

        assert!(modes
            .iter()
            .any(|mode| mode.name == "GUIDED" && mode.value == 4));
        assert!(modes
            .iter()
            .any(|mode| mode.name == "RTL" && mode.value == 6));
        assert!(modes
            .iter()
            .any(|mode| mode.name == "LAND" && mode.value == 8));
    }

    #[test]
    fn a_plan_becomes_the_shape_the_interface_expects() {
        let mut plan = Plan::new(MissionType::Fence);
        plan.push(mpcraft_mission::MissionItem::new(
            mpcraft_mavlink::MavCmd::MAV_CMD_NAV_FENCE_RETURN_POINT,
        ));

        let info = PlanInfo::from(&plan);

        assert_eq!(info.kind, PlanKind::Fence);
        assert_eq!(info.items.len(), 1);
        assert_eq!(info.items[0].seq, 0);
        assert_eq!(info.items[0].command, "MAV_CMD_NAV_FENCE_RETURN_POINT");
    }
}
