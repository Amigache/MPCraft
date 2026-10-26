//! The video streams of a camera on the vehicle.
//!
//! A camera can tell the ground station the address of its video stream, its size and
//! its frame rate. The stream itself is not MAVLink: the ground station uses the URI
//! the camera gives it to connect to the video separately.
//!
//! Upstream makes the same request in `CameraProtocol`: `MAV_CMD_REQUEST_MESSAGE` asks
//! for `VIDEO_STREAM_INFORMATION`, and the stream is started or stopped with the
//! corresponding video command.

use std::collections::BTreeMap;
use std::time::Duration;

use mpcraft_mavlink::dialect::messages::{
    CAMERA_CAPTURE_STATUS_DATA, CAMERA_INFORMATION_DATA, CAMERA_SETTINGS_DATA, COMMAND_LONG_DATA,
    VIDEO_STREAM_INFORMATION_DATA,
};
use mpcraft_mavlink::dialect::{CameraCapFlags, CameraMode, CameraZoomType, MavCmd, MavMessage};
use mpcraft_mavlink::CharArray;
use serde::Serialize;

use crate::error::LinkError;
use crate::session::{ReceivedMessage, Session};
use crate::vehicle::VehicleId;

/// The MAVLink id of `VIDEO_STREAM_INFORMATION`.
pub const VIDEO_STREAM_INFORMATION_ID: u32 = 269;

/// The message that says what a camera can do.
pub const CAMERA_INFORMATION_ID: u32 = 259;

/// The message that says which mode a camera is in.
pub const CAMERA_SETTINGS_ID: u32 = 260;

/// The message that says what a camera has captured.
pub const CAMERA_CAPTURE_STATUS_ID: u32 = 262;

/// How long to wait for the camera to describe its streams.
const STREAM_TIMEOUT: Duration = Duration::from_secs(5);

/// How long to wait for the camera to describe itself.
const CAMERA_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// How many cameras are kept: a link that reports more than this is not believed.
const MAX_CAMERAS: usize = 8;

/// A video stream the camera has described.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoStream {
    /// The component that owns the stream.
    pub component_id: u8,
    /// The id of the stream.
    pub stream_id: u8,
    /// The name of the stream.
    pub name: String,
    /// The URI or port of the stream.
    pub uri: String,
    /// The horizontal resolution, in pixels.
    pub width: u16,
    /// The vertical resolution, in pixels.
    pub height: u16,
    /// How many frames the stream carries in a second.
    pub frames_per_second: f32,
    /// How many bits the stream carries in a second.
    pub bitrate: u32,
    /// Whether the stream is running.
    pub running: bool,
}

impl From<(u8, &VIDEO_STREAM_INFORMATION_DATA)> for VideoStream {
    fn from((component_id, message): (u8, &VIDEO_STREAM_INFORMATION_DATA)) -> Self {
        Self {
            component_id,
            stream_id: message.stream_id,
            name: string_of(&message.name),
            uri: string_of(&message.uri),
            width: message.resolution_h,
            height: message.resolution_v,
            frames_per_second: message.framerate,
            bitrate: message.bitrate,
            running: message.flags.bits() & 1 != 0,
        }
    }
}

impl Session {
    /// Asks the vehicle to describe its video streams.
    ///
    /// The request is registered before the message is sent, because the camera may
    /// answer before the request has left the link. A camera can be a component of its
    /// own, so the answer is matched on the system id, not on the component id.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle does not answer within the timeout, or the
    /// link cannot carry the request.
    pub async fn request_video_streams(
        &self,
        vehicle: VehicleId,
    ) -> Result<Vec<VideoStream>, LinkError> {
        let messages = self
            .exchange(
                STREAM_TIMEOUT,
                move |message: &ReceivedMessage| {
                    message.message_id == VIDEO_STREAM_INFORMATION_ID
                        && message.system_id == vehicle.system_id
                },
                |messages: &[ReceivedMessage]| {
                    let expected = messages
                        .iter()
                        .filter_map(|message| match &message.message {
                            MavMessage::VIDEO_STREAM_INFORMATION(stream) => {
                                Some(usize::from(stream.count.max(1)))
                            }
                            _ => None,
                        })
                        .max()
                        .unwrap_or(1);
                    messages
                        .iter()
                        .filter(|message| {
                            matches!(message.message, MavMessage::VIDEO_STREAM_INFORMATION(_))
                        })
                        .count()
                        >= expected
                },
                move |confirmation| {
                    MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
                        param1: VIDEO_STREAM_INFORMATION_ID as f32,
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

        let streams = messages
            .iter()
            .filter_map(|message| match &message.message {
                MavMessage::VIDEO_STREAM_INFORMATION(stream) => {
                    Some(VideoStream::from((message.component_id, stream)))
                }
                _ => None,
            })
            .collect();

        Ok(streams)
    }

    /// Starts one video stream.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command or does not answer.
    pub async fn start_video_stream(
        &self,
        vehicle: VehicleId,
        component_id: u8,
        stream_id: u8,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        self.command_long(
            VehicleId::new(vehicle.system_id, component_id),
            MavCmd::MAV_CMD_VIDEO_START_STREAMING,
            [f32::from(stream_id), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            timeout,
        )
        .await?;

        Ok(())
    }

    /// Stops one video stream.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses the command or does not answer.
    pub async fn stop_video_stream(
        &self,
        vehicle: VehicleId,
        component_id: u8,
        stream_id: u8,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        self.command_long(
            VehicleId::new(vehicle.system_id, component_id),
            MavCmd::MAV_CMD_VIDEO_STOP_STREAMING,
            [f32::from(stream_id), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            timeout,
        )
        .await?;

        Ok(())
    }
}

/// Reads the text a MAVLink `char[]` carries.
fn string_of<const N: usize>(value: &CharArray<N>) -> String {
    name_of(&value.iter().copied().collect::<Vec<u8>>())
}

/// Reads the text an array of bytes carries.
fn name_of(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());

    String::from_utf8_lossy(&bytes[..end]).trim().to_owned()
}

/// What a camera can do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraCapabilities {
    /// Whether the camera records video.
    pub records_video: bool,
    /// Whether the camera takes pictures.
    pub takes_pictures: bool,
    /// Whether the camera has separate image and video modes.
    pub has_modes: bool,
    /// Whether a picture can be taken while the camera is recording.
    pub can_take_picture_while_recording: bool,
    /// Whether video can be recorded while the camera is taking pictures.
    pub can_record_while_imaging: bool,
    /// Whether the camera can be zoomed.
    pub has_zoom: bool,
    /// Whether the camera can follow a point in its picture.
    pub can_track_point: bool,
    /// Whether the camera can follow an area of its picture.
    pub can_track_rectangle: bool,
}

impl CameraCapabilities {
    /// The capabilities the camera reports.
    #[must_use]
    pub fn of(flags: CameraCapFlags) -> Self {
        Self {
            records_video: flags.contains(CameraCapFlags::CAMERA_CAP_FLAGS_CAPTURE_VIDEO),
            takes_pictures: flags.contains(CameraCapFlags::CAMERA_CAP_FLAGS_CAPTURE_IMAGE),
            has_modes: flags.contains(CameraCapFlags::CAMERA_CAP_FLAGS_HAS_MODES),
            can_take_picture_while_recording: flags
                .contains(CameraCapFlags::CAMERA_CAP_FLAGS_CAN_CAPTURE_IMAGE_IN_VIDEO_MODE),
            can_record_while_imaging: flags
                .contains(CameraCapFlags::CAMERA_CAP_FLAGS_CAN_CAPTURE_VIDEO_IN_IMAGE_MODE),
            has_zoom: flags.contains(CameraCapFlags::CAMERA_CAP_FLAGS_HAS_BASIC_ZOOM),
            can_track_point: flags.contains(CameraCapFlags::CAMERA_CAP_FLAGS_HAS_TRACKING_POINT),
            can_track_rectangle: flags
                .contains(CameraCapFlags::CAMERA_CAP_FLAGS_HAS_TRACKING_RECTANGLE),
        }
    }
}

/// A kind of zoom, as the interface shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomOption {
    /// The value the command carries.
    pub value: u32,
    /// The word the interface shows.
    pub name: String,
}

/// The kinds of zoom the protocol knows.
#[must_use]
pub fn zoom_options() -> Vec<ZoomOption> {
    [
        (CameraZoomType::ZOOM_TYPE_STEP, "one step"),
        (
            CameraZoomType::ZOOM_TYPE_CONTINUOUS,
            "a rate until it stops",
        ),
        (
            CameraZoomType::ZOOM_TYPE_RANGE,
            "a share of the whole range",
        ),
        (CameraZoomType::ZOOM_TYPE_FOCAL_LENGTH, "a focal length"),
        (CameraZoomType::ZOOM_TYPE_HORIZONTAL_FOV, "a field of view"),
    ]
    .into_iter()
    .map(|(kind, name)| ZoomOption {
        value: kind as u32,
        name: name.to_owned(),
    })
    .collect()
}

/// The kind of zoom a number names, or nothing when it names none.
#[must_use]
pub fn zoom_type_of(value: u32) -> Option<CameraZoomType> {
    match value {
        0 => Some(CameraZoomType::ZOOM_TYPE_STEP),
        1 => Some(CameraZoomType::ZOOM_TYPE_CONTINUOUS),
        2 => Some(CameraZoomType::ZOOM_TYPE_RANGE),
        3 => Some(CameraZoomType::ZOOM_TYPE_FOCAL_LENGTH),
        4 => Some(CameraZoomType::ZOOM_TYPE_HORIZONTAL_FOV),
        _ => None,
    }
}

/// One camera the vehicle has reported.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraDevice {
    /// The component that reports this camera.
    pub component_id: u8,
    /// Who made the camera.
    pub vendor: String,
    /// Which camera it is.
    pub model: String,
    /// The width of the picture, in pixels.
    pub width: u16,
    /// The height of the picture, in pixels.
    pub height: u16,
    /// Where the camera says its definition is, when it says one.
    pub definition_uri: String,
    /// What the camera can do.
    pub capabilities: CameraCapabilities,
    /// The mode the camera is in, as a word the interface shows.
    pub mode: String,
    /// Whether a picture can be taken in the mode it is in.
    pub takes_picture_now: bool,
    /// Whether video can be recorded in the mode it is in.
    pub records_video_now: bool,
    /// What the camera says about its pictures, as a word.
    pub image_status: String,
    /// What the camera says about its video, as a word.
    pub video_status: String,
}

impl CameraDevice {
    /// A camera as it first describes itself.
    #[must_use]
    pub fn of(component_id: u8, message: &CAMERA_INFORMATION_DATA) -> Self {
        let capabilities = CameraCapabilities::of(message.flags);

        Self {
            component_id,
            vendor: name_of(&message.vendor_name),
            model: name_of(&message.model_name),
            width: message.resolution_h,
            height: message.resolution_v,
            definition_uri: string_of(&message.cam_definition_uri),
            capabilities,
            mode: "not said".to_owned(),
            takes_picture_now: capabilities.takes_pictures,
            records_video_now: capabilities.records_video,
            image_status: "not said".to_owned(),
            video_status: "not said".to_owned(),
        }
    }

    /// The mode the camera is in, as a word.
    #[must_use]
    pub fn mode_name(mode: CameraMode) -> String {
        match mode {
            CameraMode::CAMERA_MODE_IMAGE => "image",
            CameraMode::CAMERA_MODE_IMAGE_SURVEY => "image survey",
            CameraMode::CAMERA_MODE_VIDEO => "video",
        }
        .to_owned()
    }

    /// Whether a picture can be taken in the mode the camera is in.
    ///
    /// A capability and a mode are two different things: a camera that can take pictures
    /// cannot always take one while it is recording. Upstream works out the same answer in
    /// `CanCaptureImage`.
    #[must_use]
    pub fn can_take_picture(&self) -> bool {
        if !self.capabilities.takes_pictures {
            return false;
        }

        match self.mode.as_str() {
            "image" | "image survey" => true,
            "video" => self.capabilities.can_take_picture_while_recording,
            _ => !self.capabilities.has_modes,
        }
    }

    /// Whether video can be recorded in the mode the camera is in.
    #[must_use]
    pub fn can_record_video(&self) -> bool {
        if !self.capabilities.records_video {
            return false;
        }

        match self.mode.as_str() {
            "video" => true,
            "image" | "image survey" => self.capabilities.can_record_while_imaging,
            _ => !self.capabilities.has_modes,
        }
    }
}

/// The cameras the vehicle has reported, by the component that reports them.
#[derive(Debug, Clone, Default)]
pub struct CameraRegistry {
    cameras: BTreeMap<u8, CameraDevice>,
}

impl CameraRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records what a camera says about itself.
    pub fn observe_information(&mut self, component_id: u8, message: &CAMERA_INFORMATION_DATA) {
        if self.cameras.len() >= MAX_CAMERAS && !self.cameras.contains_key(&component_id) {
            return;
        }

        self.cameras
            .insert(component_id, CameraDevice::of(component_id, message));
    }

    /// Records the mode a camera is in.
    pub fn observe_settings(&mut self, component_id: u8, message: &CAMERA_SETTINGS_DATA) {
        let Some(camera) = self.cameras.get_mut(&component_id) else {
            return;
        };

        camera.mode = CameraDevice::mode_name(message.mode_id);
        camera.takes_picture_now = camera.can_take_picture();
        camera.records_video_now = camera.can_record_video();
    }

    /// Records what a camera says about its captures.
    pub fn observe_capture_status(
        &mut self,
        component_id: u8,
        message: &CAMERA_CAPTURE_STATUS_DATA,
    ) {
        let Some(camera) = self.cameras.get_mut(&component_id) else {
            return;
        };

        // The number of pictures taken is a MAVLink v2 extension field, and the bindings this
        // project compiles do not carry it, so only the state is kept.
        camera.image_status = image_status_word(message.image_status);
        camera.video_status = video_status_word(message.video_status);
    }

    /// The cameras that are known, by component.
    #[must_use]
    pub fn cameras(&self) -> Vec<CameraDevice> {
        self.cameras.values().cloned().collect()
    }
}

/// What the camera says about its pictures, as a word.
fn image_status_word(status: u8) -> String {
    match status {
        0 => "idle",
        1 => "taking one",
        2 => "taking at an interval",
        3 => "taking at an interval, now",
        _ => "not said",
    }
    .to_owned()
}

/// What the camera says about its video, as a word.
fn video_status_word(status: u8) -> String {
    match status {
        0 => "idle",
        1 => "recording",
        _ => "not said",
    }
    .to_owned()
}

/// The camera the vehicle last reported, as the state published it.
fn reported_camera(session: &Session, component_id: u8) -> Option<CameraDevice> {
    session
        .state()
        .cameras
        .into_iter()
        .find(|camera| camera.component_id == component_id)
}

/// A place in the picture, from the range the operator gives to the range the message carries.
fn to_unit(value: f64) -> f32 {
    ((value + 1.0) / 2.0) as f32
}

/// The number that counts the pictures taken, so a retry does not take the same picture twice.
fn next_image_sequence() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(1);

    COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// The camera operations of a session.
impl Session {
    /// Asks the vehicle what its cameras can do.
    ///
    /// A camera is a component of its own, so the answer is matched on the system id rather
    /// than on the component id.
    ///
    /// # Errors
    ///
    /// Returns an error when the camera does not answer within the timeout, or the link
    /// cannot carry the request.
    pub async fn request_camera_information(
        &self,
        vehicle: VehicleId,
    ) -> Result<Vec<CameraDevice>, LinkError> {
        let matcher = move |message: &ReceivedMessage| {
            message.message_id == CAMERA_INFORMATION_ID && message.system_id == vehicle.system_id
        };

        let complete = |messages: &[ReceivedMessage]| !messages.is_empty();

        let collected = match self
            .exchange(
                CAMERA_REQUEST_TIMEOUT,
                matcher,
                complete,
                move |confirmation| {
                    MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
                        param1: CAMERA_INFORMATION_ID as f32,
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
            .await
        {
            Ok(messages) => messages,
            Err(LinkError::Timeout(_)) => {
                // The cameras that do not know the new request answer the old one, so the old
                // one is sent as well. Upstream does the same.
                #[allow(deprecated)]
                self.write_message(&MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
                    param1: 0.0,
                    param2: 0.0,
                    param3: 0.0,
                    param4: 0.0,
                    param5: 0.0,
                    param6: 0.0,
                    param7: 0.0,
                    command: MavCmd::MAV_CMD_REQUEST_CAMERA_INFORMATION,
                    target_system: vehicle.system_id,
                    target_component: vehicle.component_id,
                    confirmation: 0,
                }))
                .await?;

                let mut collector = self.begin_collect(matcher, complete).await?;
                collector.wait_for(CAMERA_REQUEST_TIMEOUT).await?
            }
            Err(error) => return Err(error),
        };

        let found = collected
            .iter()
            .filter_map(|message| match &message.message {
                MavMessage::CAMERA_INFORMATION(information) => {
                    Some(CameraDevice::of(message.component_id, information))
                }
                _ => None,
            })
            .collect();

        Ok(found)
    }

    /// Asks a camera to say what it is doing at a given rate.
    ///
    /// A rate of zero stops the reports, which is what the message says.
    ///
    /// # Errors
    ///
    /// Returns an error when the vehicle refuses a command or does not answer.
    pub async fn watch_camera(
        &self,
        vehicle: VehicleId,
        camera: &CameraDevice,
        rate_hz: u32,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        if rate_hz == 0 {
            return Ok(());
        }

        let interval_us = 1_000_000.0 / f64::from(rate_hz);

        for (message_id, wanted) in [
            (
                CAMERA_SETTINGS_ID,
                camera.capabilities.has_modes || camera.capabilities.has_zoom,
            ),
            (
                CAMERA_CAPTURE_STATUS_ID,
                camera.capabilities.takes_pictures || camera.capabilities.records_video,
            ),
        ] {
            if !wanted {
                continue;
            }

            self.set_message_interval(vehicle, message_id, interval_us as f32, timeout)
                .await?;
        }

        Ok(())
    }

    /// Asks a camera to take one picture.
    ///
    /// # Errors
    ///
    /// Returns an error when the camera cannot take pictures, when the vehicle refuses the
    /// command, or when it does not answer.
    pub async fn capture_photo(
        &self,
        vehicle: VehicleId,
        camera_index: u8,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let camera = reported_camera(self, vehicle.component_id).ok_or_else(|| {
            LinkError::Unsupported("the vehicle has not said what its camera can do".to_owned())
        })?;

        if !camera.can_take_picture() {
            return Err(LinkError::Unsupported(
                "the camera cannot take a picture in the mode it is in".to_owned(),
            ));
        }

        // The sequence number keeps a retry from taking the same picture twice, which is why
        // upstream counts it.
        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_IMAGE_START_CAPTURE,
            [
                f32::from(camera_index),
                0.0,
                1.0,
                next_image_sequence() as f32,
                0.0,
                0.0,
                0.0,
            ],
            timeout,
        )
        .await
        .map(|_| ())
    }

    /// Asks a camera to take pictures at an interval.
    ///
    /// # Errors
    ///
    /// Returns an error when the camera cannot take pictures, when the interval is not a
    /// positive number of seconds, when the vehicle refuses the command, or when it does not
    /// answer.
    pub async fn start_interval_capture(
        &self,
        vehicle: VehicleId,
        camera_index: u8,
        interval_seconds: f64,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let camera = reported_camera(self, vehicle.component_id).ok_or_else(|| {
            LinkError::Unsupported("the vehicle has not said what its camera can do".to_owned())
        })?;

        if !camera.capabilities.takes_pictures {
            return Err(LinkError::Unsupported(
                "the camera cannot take pictures".to_owned(),
            ));
        }

        if !interval_seconds.is_finite() || interval_seconds <= 0.0 {
            return Err(LinkError::Rejected(
                "the interval between pictures is not a number of seconds".to_owned(),
            ));
        }

        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_IMAGE_START_CAPTURE,
            [
                f32::from(camera_index),
                interval_seconds as f32,
                0.0,
                0.0,
                0.0,
                0.0,
                f32::NAN,
            ],
            timeout,
        )
        .await
        .map(|_| ())
    }

    /// Stops the pictures a camera was taking at an interval.
    ///
    /// # Errors
    ///
    /// Returns an error when the camera cannot take pictures, when the vehicle refuses the
    /// command, or when it does not answer.
    pub async fn stop_interval_capture(
        &self,
        vehicle: VehicleId,
        camera_index: u8,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let camera = reported_camera(self, vehicle.component_id).ok_or_else(|| {
            LinkError::Unsupported("the vehicle has not said what its camera can do".to_owned())
        })?;

        if !camera.capabilities.takes_pictures {
            return Err(LinkError::Unsupported(
                "the camera cannot take pictures".to_owned(),
            ));
        }

        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_IMAGE_STOP_CAPTURE,
            [
                f32::from(camera_index),
                f32::NAN,
                f32::NAN,
                f32::NAN,
                0.0,
                0.0,
                f32::NAN,
            ],
            timeout,
        )
        .await
        .map(|_| ())
    }

    /// Asks a camera to start recording video.
    ///
    /// # Errors
    ///
    /// Returns an error when the camera cannot record video, when the vehicle refuses the
    /// command, or when it does not answer.
    pub async fn start_recording(
        &self,
        vehicle: VehicleId,
        stream_id: u8,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let camera = reported_camera(self, vehicle.component_id).ok_or_else(|| {
            LinkError::Unsupported("the vehicle has not said what its camera can do".to_owned())
        })?;

        if !camera.can_record_video() {
            return Err(LinkError::Unsupported(
                "the camera cannot record video in the mode it is in".to_owned(),
            ));
        }

        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_VIDEO_START_CAPTURE,
            [
                f32::from(stream_id),
                f32::NAN,
                f32::NAN,
                f32::NAN,
                0.0,
                0.0,
                f32::NAN,
            ],
            timeout,
        )
        .await
        .map(|_| ())
    }

    /// Stops the video a camera was recording.
    ///
    /// # Errors
    ///
    /// Returns an error when the camera cannot record video, when the vehicle refuses the
    /// command, or when it does not answer.
    pub async fn stop_recording(
        &self,
        vehicle: VehicleId,
        stream_id: u8,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let camera = reported_camera(self, vehicle.component_id).ok_or_else(|| {
            LinkError::Unsupported("the vehicle has not said what its camera can do".to_owned())
        })?;

        if !camera.capabilities.records_video {
            return Err(LinkError::Unsupported(
                "the camera cannot record video".to_owned(),
            ));
        }

        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_VIDEO_STOP_CAPTURE,
            [
                f32::from(stream_id),
                f32::NAN,
                f32::NAN,
                f32::NAN,
                0.0,
                0.0,
                f32::NAN,
            ],
            timeout,
        )
        .await
        .map(|_| ())
    }

    /// Sets the zoom of a camera.
    ///
    /// # Errors
    ///
    /// Returns an error when the camera cannot be zoomed, when the vehicle refuses the
    /// command, or when it does not answer.
    pub async fn set_camera_zoom(
        &self,
        vehicle: VehicleId,
        level: f64,
        zoom_type: CameraZoomType,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let camera = reported_camera(self, vehicle.component_id).ok_or_else(|| {
            LinkError::Unsupported("the vehicle has not said what its camera can do".to_owned())
        })?;

        if !camera.capabilities.has_zoom {
            return Err(LinkError::Unsupported(
                "the camera cannot be zoomed".to_owned(),
            ));
        }

        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_SET_CAMERA_ZOOM,
            [
                zoom_type as u32 as f32,
                level as f32,
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

    /// Asks a camera to follow a point in its picture.
    ///
    /// The place is given from `-1` to `1`: `-1` is the left or the top of the picture, `1`
    /// is the right or the bottom, and `0` is the middle.
    ///
    /// # Errors
    ///
    /// Returns an error when the camera cannot follow a point, or when the vehicle refuses the
    /// command or does not answer.
    pub async fn track_point(
        &self,
        vehicle: VehicleId,
        x: f64,
        y: f64,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let camera = reported_camera(self, vehicle.component_id).ok_or_else(|| {
            LinkError::Unsupported("the vehicle has not said what its camera can do".to_owned())
        })?;

        if !camera.capabilities.can_track_point {
            return Err(LinkError::Unsupported(
                "the camera cannot follow a point in its picture".to_owned(),
            ));
        }

        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_CAMERA_TRACK_POINT,
            [to_unit(x), to_unit(y), 0.0, 0.0, 0.0, 0.0, 0.0],
            timeout,
        )
        .await
        .map(|_| ())
    }

    /// Asks a camera to follow an area of its picture.
    ///
    /// # Errors
    ///
    /// Returns an error when the camera cannot follow an area, or when the vehicle refuses the
    /// command or does not answer.
    pub async fn track_rectangle(
        &self,
        vehicle: VehicleId,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        let camera = reported_camera(self, vehicle.component_id).ok_or_else(|| {
            LinkError::Unsupported("the vehicle has not said what its camera can do".to_owned())
        })?;

        if !camera.capabilities.can_track_rectangle {
            return Err(LinkError::Unsupported(
                "the camera cannot follow an area of its picture".to_owned(),
            ));
        }

        // The corners may be given in any order; the message wants the upper-left one first.
        let (left, right) = (to_unit(x1.min(x2)), to_unit(x1.max(x2)));
        let (top, bottom) = (to_unit(y1.min(y2)), to_unit(y1.max(y2)));

        self.command_long(
            vehicle,
            MavCmd::MAV_CMD_CAMERA_TRACK_RECTANGLE,
            [left, top, right, bottom, 0.0, 0.0, 0.0],
            timeout,
        )
        .await
        .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpcraft_mavlink::dialect::VideoStreamStatusFlags;

    fn text<const N: usize>(value: &str) -> CharArray<N> {
        let mut array = CharArray::from([0_u8; N]);
        for (slot, byte) in array.iter_mut().zip(value.bytes()) {
            *slot = byte;
        }
        array
    }

    fn bytes<const N: usize>(value: &str) -> [u8; N] {
        let mut array = [0_u8; N];
        for (slot, byte) in array.iter_mut().zip(value.bytes()) {
            *slot = byte;
        }
        array
    }

    #[test]
    fn the_name_of_a_stream_is_read_out_of_its_bytes() {
        let name = CharArray::from(*b"front camera\0");
        assert_eq!(string_of(&name), "front camera");
    }

    #[test]
    fn an_empty_stream_name_is_empty() {
        let name = CharArray::from([0_u8; 32]);
        assert_eq!(string_of(&name), "");
    }

    #[test]
    fn stream_information_keeps_the_camera_component_and_the_uri() {
        let message = VIDEO_STREAM_INFORMATION_DATA {
            stream_id: 2,
            count: 2,
            framerate: 29.97,
            bitrate: 2_500_000,
            resolution_h: 1920,
            resolution_v: 1080,
            flags: VideoStreamStatusFlags::VIDEO_STREAM_STATUS_FLAGS_RUNNING,
            name: text::<32>("front camera"),
            uri: text::<160>("rtsp://camera.local/front"),
            ..Default::default()
        };

        let stream = VideoStream::from((100, &message));

        assert_eq!(stream.component_id, 100);
        assert_eq!(stream.stream_id, 2);
        assert_eq!(stream.name, "front camera");
        assert_eq!(stream.uri, "rtsp://camera.local/front");
        assert_eq!((stream.width, stream.height), (1920, 1080));
        assert!(stream.running);
    }

    #[test]
    fn a_camera_keeps_what_it_says_it_can_do() {
        let message = CAMERA_INFORMATION_DATA {
            flags: CameraCapFlags::CAMERA_CAP_FLAGS_CAPTURE_IMAGE
                | CameraCapFlags::CAMERA_CAP_FLAGS_HAS_MODES
                | CameraCapFlags::CAMERA_CAP_FLAGS_HAS_BASIC_ZOOM
                | CameraCapFlags::CAMERA_CAP_FLAGS_HAS_TRACKING_POINT,
            resolution_h: 4_032,
            resolution_v: 3_024,
            vendor_name: bytes::<32>("Hasselblad"),
            model_name: bytes::<32>("X1D-30"),
            ..Default::default()
        };

        let camera = CameraDevice::of(154, &message);

        assert_eq!(camera.component_id, 154);
        assert_eq!(camera.vendor, "Hasselblad");
        assert_eq!(camera.model, "X1D-30");
        assert_eq!((camera.width, camera.height), (4_032, 3_024));
        assert!(camera.capabilities.takes_pictures);
        assert!(camera.capabilities.has_zoom);
        assert!(camera.capabilities.can_track_point);
        // A camera that can take pictures but not record them cannot be asked to record.
        assert!(!camera.capabilities.records_video);
    }

    #[test]
    fn the_mode_decides_what_a_camera_can_capture() {
        let information = CAMERA_INFORMATION_DATA {
            flags: CameraCapFlags::CAMERA_CAP_FLAGS_CAPTURE_IMAGE
                | CameraCapFlags::CAMERA_CAP_FLAGS_CAPTURE_VIDEO
                | CameraCapFlags::CAMERA_CAP_FLAGS_HAS_MODES,
            ..Default::default()
        };

        let mut registry = CameraRegistry::new();
        registry.observe_information(154, &information);

        // The camera says it is recording video, and it cannot take a picture while it does.
        let settings = CAMERA_SETTINGS_DATA {
            mode_id: CameraMode::CAMERA_MODE_VIDEO,
            ..Default::default()
        };
        registry.observe_settings(154, &settings);

        let cameras = registry.cameras();
        let camera = cameras.first().expect("the camera is known");
        assert_eq!(camera.mode, "video");
        assert!(!camera.can_take_picture());
        assert!(camera.can_record_video());
    }

    #[test]
    fn a_camera_without_modes_can_capture_whatever_it_can_do() {
        let information = CAMERA_INFORMATION_DATA {
            flags: CameraCapFlags::CAMERA_CAP_FLAGS_CAPTURE_IMAGE
                | CameraCapFlags::CAMERA_CAP_FLAGS_CAPTURE_VIDEO,
            ..Default::default()
        };

        let camera = CameraDevice::of(154, &information);

        // Without separate modes, the capability is the whole answer.
        assert!(!camera.capabilities.has_modes);
        assert!(camera.can_take_picture());
        assert!(camera.can_record_video());
    }

    #[test]
    fn the_status_a_camera_reports_is_kept_as_a_word() {
        let information = CAMERA_INFORMATION_DATA {
            flags: CameraCapFlags::CAMERA_CAP_FLAGS_CAPTURE_IMAGE,
            ..Default::default()
        };

        let mut registry = CameraRegistry::new();
        registry.observe_information(154, &information);

        let status = CAMERA_CAPTURE_STATUS_DATA {
            image_status: 3,
            video_status: 1,
            ..Default::default()
        };
        registry.observe_capture_status(154, &status);

        let cameras = registry.cameras();
        let camera = cameras.first().expect("the camera is known");
        assert_eq!(camera.image_status, "taking at an interval, now");
        assert_eq!(camera.video_status, "recording");
    }

    #[test]
    fn a_place_in_the_picture_is_read_in_the_range_the_message_carries() {
        assert_eq!(to_unit(-1.0), 0.0);
        assert_eq!(to_unit(0.0), 0.5);
        assert_eq!(to_unit(1.0), 1.0);
    }
}
