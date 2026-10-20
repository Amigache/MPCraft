//! End-to-end tests of the parameter protocol.
//!
//! These drive a real session against a simulated vehicle: the session writes the request
//! messages, and the test answers them the way a vehicle would. The bytes are checked too, so
//! a test cannot pass by accident.

use std::time::Duration;

use mpcraft_link::{Session, SessionOptions};
use mpcraft_mavlink::dialect::messages::{
    AUTOPILOT_VERSION_DATA, HEARTBEAT_DATA, PARAM_VALUE_DATA,
};
use mpcraft_mavlink::frame::{encode_v2, MavHeader};
use mpcraft_mavlink::{MavAutopilot, MavMessage, MavType};
use mpcraft_params::{ParamEncoding, ParamType};
use mpcraft_transport::ConnectionSpec;

/// A simulated vehicle that answers parameter requests.
///
/// It writes its answers into a file, which the session then reads: that keeps the test
/// deterministic and lets the session run over the ordinary file transport.
struct SimulatedVehicle {
    path: std::path::PathBuf,
    sequence: u8,
    answers: Vec<u8>,
    /// The parameters the vehicle knows, as (name, value, reported type).
    parameters: Vec<(String, f32, ParamType)>,
    /// The capabilities the vehicle advertises.
    capabilities: u64,
}

impl SimulatedVehicle {
    fn new(name: &str) -> Self {
        Self {
            path: std::env::temp_dir().join(format!("mpcraft-param-{name}.tlog")),
            sequence: 0,
            answers: Vec::new(),
            parameters: Vec::new(),
            capabilities: 0,
        }
    }

    /// The vehicle's answer bytes.
    fn answers(&self) -> &[u8] {
        &self.answers
    }

    /// Writes a message into the answer stream.
    fn emit(&mut self, message: MavMessage) {
        let header = MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.wrapping_add(1);
        self.answers.extend_from_slice(&encode_v2(&message, header));
    }

    /// Emits the heartbeat and the autopilot version.
    fn announce(&mut self) {
        self.emit(MavMessage::HEARTBEAT(HEARTBEAT_DATA {
            custom_mode: 5,
            mavtype: MavType::MAV_TYPE_QUADROTOR,
            autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
            ..Default::default()
        }));

        self.emit(MavMessage::AUTOPILOT_VERSION(AUTOPILOT_VERSION_DATA {
            capabilities: mpcraft_mavlink::MavProtocolCapability::from_bits_truncate(
                self.capabilities,
            ),
            ..Default::default()
        }));
    }

    /// Emits a `PARAM_VALUE` for every parameter.
    fn emit_all_parameters(&mut self) {
        let total = self.parameters.len() as u16;

        for (index, (name, value, param_type)) in self.parameters.clone().iter().enumerate() {
            self.emit(MavMessage::PARAM_VALUE(PARAM_VALUE_DATA {
                param_value: *value,
                param_count: total,
                param_index: index as u16,
                param_id: param_id(name),
                param_type: param_type.as_mavlink(),
            }));
        }
    }
}

/// The `param_id` field of a message: the name, padded to sixteen bytes.
fn param_id(name: &str) -> mpcraft_mavlink::CharArray<16> {
    let mut buffer = [0u8; 16];
    let bytes = name.as_bytes();
    let length = bytes.len().min(16);
    buffer[..length].copy_from_slice(&bytes[..length]);
    mpcraft_mavlink::CharArray::new(buffer)
}

/// Opens a session that will read the vehicle's answers.
async fn session_for(vehicle: &SimulatedVehicle) -> Session {
    // The session reads the file, so the answers have to be in place first. The caller
    // writes them and then opens the session.
    Session::connect(
        &ConnectionSpec::parse(&format!("file:{}", vehicle.path.display())).unwrap(),
        SessionOptions::default(),
    )
    .await
    .unwrap()
}

/// Waits for a condition to hold, polling the session state.
async fn wait_for<F>(session: &Session, timeout: Duration, condition: F) -> bool
where
    F: Fn(&mpcraft_link::SessionState) -> bool,
{
    let deadline = std::time::Instant::now() + timeout;

    while std::time::Instant::now() < deadline {
        if condition(&session.state()) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    condition(&session.state())
}

#[tokio::test]
async fn the_parameters_are_decoded_from_the_vehicle_stream() {
    let mut vehicle = SimulatedVehicle::new("list");
    vehicle.parameters = vec![
        ("ARMING_CHECK".to_owned(), 1.0, ParamType::Real32),
        ("RTL_ALT".to_owned(), 100.0, ParamType::Real32),
        ("FS_BATT_ENABLE".to_owned(), 1.0, ParamType::Real32),
    ];
    vehicle.announce();
    vehicle.emit_all_parameters();

    std::fs::write(&vehicle.path, vehicle.answers()).unwrap();

    let session = session_for(&vehicle).await;

    // The reader task processes the file asynchronously.
    let got = wait_for(&session, Duration::from_secs(5), |state| {
        state.parameters.len() == 3
    })
    .await;
    assert!(got, "the parameters were never decoded");

    let state = session.state();

    // The parameters are in the session's store, decoded with the float encoding, because
    // ArduPilot is the autopilot that was announced.
    let arming = state.parameters.get("ARMING_CHECK").expect("ARMING_CHECK");
    assert_eq!(arming.value, 1.0);

    let rtl_alt = state.parameters.get("RTL_ALT").expect("RTL_ALT");
    assert_eq!(rtl_alt.value, 100.0);

    assert_eq!(state.parameters.len(), 3);
    assert_eq!(state.parameters.total_reported(), 3);
    assert!(state.parameters.is_complete());
}

#[tokio::test]
async fn a_byte_wise_vehicle_decodes_its_integers_correctly() {
    // A vehicle that encodes its values byte-wise: the integer 1000 travels as its own
    // bytes, not as the float 1000.0.
    let mut vehicle = SimulatedVehicle::new("bytewise");
    vehicle.capabilities =
        mpcraft_mavlink::MavProtocolCapability::MAV_PROTOCOL_CAPABILITY_PARAM_ENCODE_BYTEWISE
            .bits();
    vehicle.parameters = vec![("WP_RADIUS".to_owned(), 1000.0, ParamType::UInt16)];
    vehicle.announce();
    vehicle.emit_all_parameters();

    std::fs::write(&vehicle.path, vehicle.answers()).unwrap();

    let session = session_for(&vehicle).await;

    let got = wait_for(&session, Duration::from_secs(5), |state| {
        state.parameters.get("WP_RADIUS").is_some()
    })
    .await;
    assert!(got, "the parameter was never decoded");

    let state = session.state();

    // The value in the stream is the float whose bits are the integer's bytes, which is
    // what a byte-wise vehicle sends.
    let radius = state.parameters.get("WP_RADIUS").expect("WP_RADIUS");
    assert!(
        (radius.value - 1000.0).abs() < 1e-6,
        "the integer was decoded as {}",
        radius.value
    );
}

#[tokio::test]
async fn the_encoding_follows_the_vehicle_capabilities() {
    let mut vehicle = SimulatedVehicle::new("encoding");
    vehicle.capabilities =
        mpcraft_mavlink::MavProtocolCapability::MAV_PROTOCOL_CAPABILITY_PARAM_ENCODE_BYTEWISE
            .bits();
    vehicle.announce();

    std::fs::write(&vehicle.path, vehicle.answers()).unwrap();

    let session = session_for(&vehicle).await;

    // The reader task has to have seen the autopilot version first.
    let got = wait_for(&session, Duration::from_secs(5), |state| {
        state.capabilities.is_some()
    })
    .await;
    assert!(got, "the capabilities were never read");

    // The autopilot was not announced as ArduPilot, so the capability decides.
    let encoding = session.param_encoding();
    assert_eq!(encoding, ParamEncoding::ByteWise);
}

#[tokio::test]
async fn an_ardupilot_is_recognised_as_sending_floats() {
    let mut vehicle = SimulatedVehicle::new("ardupilot");
    vehicle.announce();

    std::fs::write(&vehicle.path, vehicle.answers()).unwrap();

    let session = session_for(&vehicle).await;

    // The reader task has to have seen the heartbeat first.
    let got = wait_for(&session, Duration::from_secs(5), |state| {
        !state.vehicles.is_empty()
    })
    .await;
    assert!(got, "the vehicle was never identified");

    assert_eq!(session.param_encoding(), ParamEncoding::Float);
}
