//! The video streams of a camera on the vehicle.
//!
//! A camera can tell the ground station the address of its video stream, its size and
//! its frame rate. The stream itself is not MAVLink: the ground station uses the URI
//! the camera gives it to connect to the video separately.
//!
//! Upstream makes the same request in `CameraProtocol`: `MAV_CMD_REQUEST_MESSAGE` asks
//! for `VIDEO_STREAM_INFORMATION`, and the stream is started or stopped with the
//! corresponding video command.

use std::time::Duration;

use mpcraft_mavlink::dialect::messages::{COMMAND_LONG_DATA, VIDEO_STREAM_INFORMATION_DATA};
use mpcraft_mavlink::dialect::{MavCmd, MavMessage};
use mpcraft_mavlink::CharArray;
use serde::Serialize;

use crate::error::LinkError;
use crate::session::{ReceivedMessage, Session};
use crate::vehicle::VehicleId;

/// The MAVLink id of `VIDEO_STREAM_INFORMATION`.
pub const VIDEO_STREAM_INFORMATION_ID: u32 = 269;

/// How long to wait for the camera to describe its streams.
const STREAM_TIMEOUT: Duration = Duration::from_secs(5);

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
    let bytes: Vec<u8> = value.iter().copied().collect();
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());

    String::from_utf8_lossy(&bytes[..end]).trim().to_owned()
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
}
