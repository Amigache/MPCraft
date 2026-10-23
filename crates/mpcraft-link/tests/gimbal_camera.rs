//! The gimbal manager and the camera protocols over a real socket.
//!
//! The suite runs a simulated vehicle that says what its devices can do. Two things are
//! checked here. What a device reports is read out of the message in the units the interface
//! shows — the wire carries radians, and the operator reads degrees. And something a device
//! cannot do is refused before anything is sent: a station that asks a gimbal with no pitch
//! axis to pitch, or a camera without zoom to zoom, is wrong on its own, and should not wait
//! for the vehicle to say so.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpcraft_link::{
    LinkError, Session, SessionOptions, VehicleId, CAMERA_INFORMATION_ID,
    GIMBAL_MANAGER_INFORMATION_ID,
};
use mpcraft_mavlink::dialect::messages::{
    CAMERA_INFORMATION_DATA, COMMAND_ACK_DATA, COMMAND_LONG_DATA, GIMBAL_MANAGER_INFORMATION_DATA,
};
use mpcraft_mavlink::dialect::{CameraCapFlags, CameraZoomType, GimbalManagerCapFlags};
use mpcraft_mavlink::frame::{encode_v2, FrameParser, MavHeader};
use mpcraft_mavlink::{MavCmd, MavMessage, MavResult, Message};

/// The messages the vehicle received, so the test can assert on them.
type Received = Arc<Mutex<Vec<MavMessage>>>;

/// The component the simulated gimbal answers on, and the camera its own.
const GIMBAL_COMPONENT: u8 = 171;
const CAMERA_COMPONENT: u8 = 154;

/// A simulated vehicle with a gimbal and a camera on it.
struct Vehicle {
    socket: tokio::net::UdpSocket,
    sequence: u8,
    /// The messages the vehicle received.
    received: Received,
}

impl Vehicle {
    /// Starts the vehicle on its own port.
    async fn start() -> (SocketAddr, Received) {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();

        let received: Received = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&received);

        let mut vehicle = Self {
            socket,
            sequence: 0,
            received: log,
        };

        tokio::spawn(async move {
            vehicle.run().await;
        });

        (address, received)
    }

    async fn run(&mut self) {
        let mut buffer = [0u8; 2048];

        loop {
            let Ok((len, peer)) = self.socket.recv_from(&mut buffer).await else {
                return;
            };

            let Some(start) = buffer[..len].iter().position(|&b| b == 0xFD || b == 0xFE) else {
                continue;
            };

            let mut parser = FrameParser::new();
            let mut frames = Vec::new();
            if parser.feed(&buffer[start..len], &mut frames).is_err() {
                continue;
            }

            for frame in &frames {
                let Ok(decoded) =
                    MavMessage::parse(frame.version, frame.message_id, &frame.payload)
                else {
                    continue;
                };

                // The ground station announces itself with a heartbeat when it connects.
                // It is not part of what the test is about.
                if matches!(decoded, MavMessage::HEARTBEAT(_)) {
                    continue;
                }

                if let Ok(mut log) = self.received.lock() {
                    log.push(decoded.clone());
                }

                self.answer(&decoded, peer).await;
            }
        }
    }

    /// Answers a request with the information its devices would give, and acknowledges it.
    async fn answer(&mut self, request: &MavMessage, peer: SocketAddr) {
        let MavMessage::COMMAND_LONG(command) = request else {
            return;
        };

        if command.command == MavCmd::MAV_CMD_REQUEST_MESSAGE {
            if command.param1 == GIMBAL_MANAGER_INFORMATION_ID as f32 {
                self.send(
                    GIMBAL_COMPONENT,
                    peer,
                    MavMessage::GIMBAL_MANAGER_INFORMATION(gimbal_information()),
                )
                .await;
            } else if command.param1 == CAMERA_INFORMATION_ID as f32 {
                self.send(
                    CAMERA_COMPONENT,
                    peer,
                    MavMessage::CAMERA_INFORMATION(camera_information()),
                )
                .await;
            }
        }

        // The acknowledgement comes from the component the command was addressed to: a camera
        // answers for itself, and the station matches an answer to the vehicle it asked.
        self.send(
            command.target_component,
            peer,
            MavMessage::COMMAND_ACK(COMMAND_ACK_DATA {
                command: command.command,
                result: MavResult::MAV_RESULT_ACCEPTED,
                ..Default::default()
            }),
        )
        .await;
    }

    async fn send(&mut self, component_id: u8, peer: SocketAddr, message: MavMessage) {
        let header = MavHeader {
            system_id: 1,
            component_id,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.wrapping_add(1);

        let bytes = encode_v2(&message, header);
        let _ = self.socket.send_to(&bytes, peer).await;
    }
}

/// The gimbal the vehicle reports: a pitch axis that goes thirty degrees each way.
fn gimbal_information() -> GIMBAL_MANAGER_INFORMATION_DATA {
    GIMBAL_MANAGER_INFORMATION_DATA {
        gimbal_device_id: 171,
        cap_flags: GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_HAS_PITCH_AXIS
            | GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_HAS_YAW_AXIS
            | GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_HAS_YAW_FOLLOW
            | GimbalManagerCapFlags::GIMBAL_MANAGER_CAP_FLAGS_CAN_POINT_LOCATION_LOCAL,
        pitch_min: -std::f32::consts::FRAC_PI_6,
        pitch_max: std::f32::consts::FRAC_PI_6,
        yaw_min: -std::f32::consts::PI,
        yaw_max: std::f32::consts::PI,
        ..Default::default()
    }
}

/// The camera the vehicle reports: it can follow a point, but it cannot be zoomed.
fn camera_information() -> CAMERA_INFORMATION_DATA {
    CAMERA_INFORMATION_DATA {
        flags: CameraCapFlags::CAMERA_CAP_FLAGS_HAS_TRACKING_POINT,
        resolution_h: 4_032,
        resolution_v: 3_024,
        vendor_name: bytes::<32>("Simulated"),
        model_name: bytes::<32>("Test Cam"),
        ..Default::default()
    }
}

/// A name as the message carries it: padded with nothing to the length the field has.
fn bytes<const N: usize>(value: &str) -> [u8; N] {
    let mut array = [0_u8; N];
    for (slot, byte) in array.iter_mut().zip(value.bytes()) {
        *slot = byte;
    }
    array
}

/// Opens a session to the vehicle.
async fn session_to(address: SocketAddr) -> Session {
    let spec = mpcraft_transport::ConnectionSpec::parse(&format!("udp:{address}")).unwrap();
    Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap()
}

/// The autopilot the session talks to.
fn vehicle() -> VehicleId {
    VehicleId::new(1, 1)
}

/// The commands the vehicle received.
fn commands_of(received: &Received) -> Vec<MavCmd> {
    received
        .lock()
        .map(|log| {
            log.iter()
                .filter_map(|message| match message {
                    MavMessage::COMMAND_LONG(long) => Some(long.command),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The command the vehicle received with the given id, when it received one.
fn long_of(received: &Received, command: MavCmd) -> Option<COMMAND_LONG_DATA> {
    received
        .lock()
        .ok()?
        .iter()
        .find_map(|message| match message {
            MavMessage::COMMAND_LONG(long) if long.command == command => Some(long.clone()),
            _ => None,
        })
}

#[tokio::test]
async fn the_limits_of_a_gimbal_are_read_in_the_units_the_interface_shows() {
    let (address, _) = Vehicle::start().await;
    let session = session_to(address).await;

    let devices = session
        .request_gimbal_information(vehicle())
        .await
        .expect("the gimbal answered");

    let device = devices.first().expect("the vehicle reported a gimbal");

    // The wire carries radians; the sheet shows degrees.
    assert!((device.limits.pitch_min + 30.0).abs() < 0.01);
    assert!((device.limits.pitch_max - 30.0).abs() < 0.01);
    assert!(device.capabilities.has_pitch_axis);
}

#[tokio::test]
async fn an_angle_outside_the_range_of_the_gimbal_is_refused_before_anything_is_sent() {
    let (address, received) = Vehicle::start().await;
    let session = session_to(address).await;

    session
        .request_gimbal_information(vehicle())
        .await
        .expect("the gimbal answered");

    // Only the request for what the gimbal can do has gone out so far.
    assert_eq!(
        commands_of(&received),
        vec![MavCmd::MAV_CMD_REQUEST_MESSAGE]
    );

    let error = session
        .set_gimbal_angles(vehicle(), 171, 45.0, 0.0, false, Duration::from_secs(2))
        .await
        .expect_err("the angle is outside the range the gimbal reported");

    assert!(matches!(error, LinkError::Unsupported(_)));

    // Nothing else went out: the refusal is the station's own, not the vehicle's answer.
    assert_eq!(
        commands_of(&received),
        vec![MavCmd::MAV_CMD_REQUEST_MESSAGE]
    );
}

#[tokio::test]
async fn an_angle_the_gimbal_can_reach_reaches_the_vehicle_as_it_was_asked() {
    let (address, received) = Vehicle::start().await;
    let session = session_to(address).await;

    session
        .request_gimbal_information(vehicle())
        .await
        .expect("the gimbal answered");

    session
        .set_gimbal_angles(vehicle(), 171, 10.0, 20.0, false, Duration::from_secs(5))
        .await
        .expect("the vehicle acknowledged the angle");

    let command = long_of(&received, MavCmd::MAV_CMD_DO_GIMBAL_MANAGER_PITCHYAW)
        .expect("the vehicle received the command");

    assert!((command.param1 - 10.0).abs() < 0.01);
    assert!((command.param2 - 20.0).abs() < 0.01);
    assert_eq!(command.param7, 171.0);
}

#[tokio::test]
async fn a_camera_that_cannot_be_zoomed_is_refused_before_anything_is_sent() {
    let (address, received) = Vehicle::start().await;
    let session = session_to(address).await;

    let cameras = session
        .request_camera_information(vehicle())
        .await
        .expect("the camera answered");

    let camera = cameras.first().expect("the vehicle reported a camera");

    // The camera answers on its own component, and that is what a command for it names.
    assert_eq!(camera.component_id, CAMERA_COMPONENT);
    assert!(camera.capabilities.can_track_point);
    assert!(!camera.capabilities.has_zoom);

    let camera_key = VehicleId::new(1, camera.component_id);

    let error = session
        .set_camera_zoom(
            camera_key,
            50.0,
            CameraZoomType::ZOOM_TYPE_RANGE,
            Duration::from_secs(2),
        )
        .await
        .expect_err("the camera cannot be zoomed");

    assert!(matches!(error, LinkError::Unsupported(_)));

    // Only the request for what the camera can do has gone out.
    assert_eq!(
        commands_of(&received),
        vec![MavCmd::MAV_CMD_REQUEST_MESSAGE]
    );
}

#[tokio::test]
async fn a_place_in_the_picture_reaches_the_vehicle_in_the_range_the_message_carries() {
    let (address, received) = Vehicle::start().await;
    let session = session_to(address).await;

    let cameras = session
        .request_camera_information(vehicle())
        .await
        .expect("the camera answered");

    let camera = cameras.first().expect("the vehicle reported a camera");
    let camera_key = VehicleId::new(1, camera.component_id);

    session
        .track_point(camera_key, 0.0, 1.0, Duration::from_secs(5))
        .await
        .expect("the vehicle acknowledged the point");

    // The interface reads the picture from -1 to 1, and the message carries it from 0 to 1:
    // the middle of the picture is 0.5, and the bottom edge is 1.
    let command = long_of(&received, MavCmd::MAV_CMD_CAMERA_TRACK_POINT)
        .expect("the vehicle received the point");

    assert!((command.param1 - 0.5).abs() < 0.01);
    assert!((command.param2 - 1.0).abs() < 0.01);
}
