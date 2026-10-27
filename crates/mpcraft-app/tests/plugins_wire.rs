//! The surface a plugin is plugged into, fed by a scripted peer rather than by a hand-written one.
//!
//! The sandbox tests read a surface that answers with numbers written in the test itself, which proves
//! the shape crosses the boundary but not where its value comes from. Here the numbers are put on a real
//! socket by a simulated vehicle, and the same surface is read afterwards: the plan count and the item the
//! vehicle says it is on come from the frames that arrived, the parameter from the answer the vehicle gave
//! about itself, and the altitude and the satellites from the telemetry the link decoded.
//!
//! The two halves are kept apart on purpose. Before anything arrives the surface has nothing to give — a
//! place in the plan the reader invents is not a place the vehicle is at, and a value the reader invents is
//! not a value the vehicle has — so the plugin sees the number that says *nothing was said* rather than
//! zero, and that is what makes the second half an assertion: the same plugin, called again after the wire
//! answered, reads the numbers the wire carried. What this cannot say is said rather than assumed: a real
//! board is not on the wire here, so what the run proves is the path from the frames to the surface, not
//! that a physical vehicle reports these numbers.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use mpcraft_app::plugins::{PluginRuntime, PluginState};
use mpcraft_link::{Session, SessionOptions, VehicleId};
use mpcraft_mavlink::dialect::messages::{
    MissionState, GLOBAL_POSITION_INT_DATA, GPS_RAW_INT_DATA, HEARTBEAT_DATA, MISSION_CURRENT_DATA,
    PARAM_VALUE_DATA,
};
use mpcraft_mavlink::frame::{encode_v2, MavHeader};
use mpcraft_mavlink::{CharArray, MavAutopilot, MavMessage, MavType};
use mpcraft_plugin::{HostApi, TelemetryField};
use mpcraft_settings::{PluginEntry, Settings};
use mpcraft_transport::ConnectionSpec;

/// What the peer puts on the wire, written down before the test calls anything, so that the numbers
/// asserted afterwards are the ones the frames carry rather than the ones the reader gives back.
const SEQUENCE: u16 = 3;
const TOTAL: u16 = 12;
const FENCE_ALT_MAX: f64 = 61.5;
const ALTITUDE: f64 = 584.0;
const SATELLITES: u8 = 14;

/// The code the plugin returns when what it read is not what the wire said.
const DISAGREED: i32 = 3;

/// A simulated vehicle that answers with the numbers the plugin will be asked to read.
async fn vehicle() -> SocketAddr {
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();

    tokio::spawn(async move {
        let mut buffer = [0_u8; 2048];

        // The station announces itself when the link opens; that is how the vehicle learns where to
        // send, and the answer goes back to the peer that asked.
        let Ok((_, peer)) = socket.recv_from(&mut buffer).await else {
            return;
        };

        let header = MavHeader {
            system_id: 2,
            component_id: 1,
            sequence: 0,
        };

        // The vehicle introduces itself as 2:1, an ArduPilot board, which is what lets the link choose
        // the pair it heard rather than an address somebody assumes. The rule reads a board seen at
        // least twice, so the introduction is not a single frame: it keeps saying what it is.
        for _ in 0..30 {
            for message in [
                MavMessage::HEARTBEAT(HEARTBEAT_DATA {
                    custom_mode: 5,
                    mavtype: MavType::MAV_TYPE_QUADROTOR,
                    autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
                    ..Default::default()
                }),
                MavMessage::MISSION_CURRENT(MISSION_CURRENT_DATA {
                    seq: SEQUENCE,
                    total: TOTAL,
                    mission_state: MissionState::MISSION_STATE_ACTIVE,
                    mission_mode: 1,
                    ..Default::default()
                }),
                MavMessage::PARAM_VALUE(PARAM_VALUE_DATA {
                    param_count: 1,
                    param_index: 0,
                    param_id: name_on_the_wire("FENCE_ALT_MAX"),
                    param_value: FENCE_ALT_MAX as f32,
                    param_type: mpcraft_params::ParamType::Real32.as_mavlink(),
                }),
                MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
                    alt: (ALTITUDE * 1e3) as i32,
                    ..Default::default()
                }),
                MavMessage::GPS_RAW_INT(GPS_RAW_INT_DATA {
                    satellites_visible: SATELLITES,
                    ..Default::default()
                }),
            ] {
                let _ = socket.send_to(&encode_v2(&message, header), peer).await;
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });

    address
}

/// The `param_id` field of a message: the name, padded to sixteen bytes.
fn name_on_the_wire(name: &str) -> CharArray<16> {
    let mut buffer = [0_u8; 16];
    buffer[..name.len()].copy_from_slice(name.as_bytes());
    CharArray::new(buffer)
}

async fn session_to(address: SocketAddr) -> Session {
    let spec = ConnectionSpec::parse(&format!("udp:{address}")).unwrap();
    Session::connect(&spec, SessionOptions::default())
        .await
        .expect("the link opens")
}

async fn wait_for_selection(session: &Session) {
    let deadline = Instant::now() + Duration::from_secs(3);

    while Instant::now() < deadline {
        if session.state().selected == Some(VehicleId::new(2, 1)) {
            return;
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    panic!(
        "the link did not choose the pair it heard: {:?}",
        session.state().selected
    );
}

/// Waits for something only a peer can answer to.
async fn wait_for(what: &dyn Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);

    while Instant::now() < deadline && !what() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// A plugin that compares what it reads with the numbers the frames carry and says it agreed by
/// returning zero. Before the wire it cannot agree, because there is nothing to read: the number that
/// means "nothing was said" is not item three of a plan, and a value the reader invents is not a value
/// the vehicle has.
const MODULE: &str = r#"
    (module
        (import "host" "mission_current" (func $current (result i32)))
        (import "host" "param" (func $param (param i32 i32) (result f64)))
        (import "host" "telemetry" (func $telemetry (param i32) (result f64)))
        (memory (export "memory") 1)
        (data (i32.const 0) "FENCE_ALT_MAX")
        (func (export "init") (result i32)
            (if (i32.eq (call $current) (i32.const 3))
                (then
                    (if (f64.eq (call $param (i32.const 0) (i32.const 13)) (f64.const 61.5))
                        (then
                            (if (f64.eq (call $telemetry (i32.const 0)) (f64.const 584.0))
                                (then
                                    (if (f64.eq (call $telemetry (i32.const 6)) (f64.const 14.0))
                                        (then
                                            (return (i32.const 0))
                                        )
                                    )
                                )
                            )
                        )
                    )
                )
            )
            (i32.const 3))
    )
"#;

struct PluginFolder(PathBuf);

impl PluginFolder {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "mpcraft-plugins-{name}-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));

        std::fs::create_dir_all(&path).expect("the folder of plugins should be creatable");

        Self(path)
    }

    /// Writes the manifest and the module it names.
    fn write(&self, manifest: &str, wat: &str) {
        std::fs::write(self.0.join("battery.toml"), manifest)
            .expect("the manifest should be writable");

        // The module is written as text, and text is only read as text when it begins with the module,
        // so the spaces are taken off before the file is written.
        std::fs::write(self.0.join("battery.wasm"), wat.trim())
            .expect("the module should be writable");
    }
}

impl Drop for PluginFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn the_numbers_a_plugin_reads_come_from_the_wire_and_not_from_the_reader() {
    let address = vehicle().await;
    let session = session_to(address).await;

    let chosen = Settings {
        plugins: vec![PluginEntry::enabled("battery")],
        ..Settings::default()
    };

    // The surface is read through the driver, and the driver is called mutably between the reads: the two
    // are the same object, so each reading takes the surface again rather than holding it across a call.
    let folder = PluginFolder::new("wire");
    folder.write(
        "name = \"battery\"\nversion = \"1.0.0\"\nmodule = \"battery.wasm\"\ncapabilities = [\"mission_access\", \"read_params\", \"read_telemetry\"]\n",
        MODULE,
    );

    let mut runtime = PluginRuntime::new(folder.0.clone());

    // The first reading is the one that says nothing was said, which is what makes the second half an
    // assertion rather than a restatement: the plugin saw the number that means nothing, not item three.
    runtime.scan(&chosen).unwrap();
    let info = runtime.open().unwrap();
    assert_eq!(info[0].state, PluginState::Failed, "{info:?}");
    assert_eq!(info[0].error_code.as_deref(), Some("plugin_stage"));
    assert!(
        info[0]
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(&format!("failed (code {DISAGREED})"))),
        "the stage failed because there was nothing to read, not because the plugin agreed"
    );

    wait_for_selection(&session).await;

    {
        let surface = runtime.surface();
        surface.record(&session.state());

        // From here the numbers are the ones the frames carried.
        wait_for(&|| {
            surface.param("FENCE_ALT_MAX") == Some(FENCE_ALT_MAX)
                && surface.mission_current() == Some(u32::from(SEQUENCE))
        })
        .await;

        assert_eq!(
            surface.mission_count(),
            u32::from(TOTAL),
            "the count is what the frame said"
        );
        assert_eq!(surface.mission_current(), Some(u32::from(SEQUENCE)));
        assert_eq!(surface.param("FENCE_ALT_MAX"), Some(FENCE_ALT_MAX));
        assert_eq!(
            surface.telemetry(TelemetryField::Altitude),
            Some(ALTITUDE),
            "the altitude is the one the position message carried, said in metres"
        );
        assert_eq!(
            surface.telemetry(TelemetryField::Satellites),
            Some(f64::from(SATELLITES)),
            "the satellites are the ones the positioning message carried"
        );
    }

    // The same plugin, called again over the same surface, agrees with them: that is the value crossing
    // the boundary as the wire gave it, read through the stage rather than through a surface written by
    // the test.
    let info = runtime.open().unwrap();
    assert_eq!(info[0].state, PluginState::Ready, "{info:?}");

    // A name the vehicle has not reported still answers with nothing, on a surface that is fed by the
    // wire rather than by a hand-written map.
    assert_eq!(runtime.surface().param("NOT_A_PARAMETER"), None);
}
