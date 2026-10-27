//! The same surface a plugin is plugged into, fed by a vehicle that answers for itself.
//!
//! `plugins_wire.rs` puts the numbers on a socket from a script written to carry them, which proves the
//! path from the frames to the surface but not that an autopilot reports them. Here the numbers come from
//! a live simulator: the parameter is the value the vehicle holds for itself, the plan count is the answer
//! the vehicle gave when it was asked, and the satellites and the height are what the vehicle chose to
//! stream.
//!
//! Three numbers can be written down before the run, and they are the three this area puts on the wire: the
//! plan it uploads, the item it asks the vehicle to be on, and the parameter value it writes. A live vehicle
//! cannot be predicted for the other readings — its height and its satellite count are its own — so those
//! halves are watched until the vehicle says them, and the comparison afterwards is said for what it is: the
//! surface holds a copy of the snapshot the aggregator filled from the frames, so what this proves is that
//! the number crossed the boundary through the import, not that a second reader would have got the same one
//! from somewhere else.
//!
//! ```text
//! cargo test -p mpcraft-app --features sitl-tests
//! ```
//!
//! The target comes from `MPCRAFT_SITL_PLUGINS_TARGET`, which falls back to the address the suite already
//! uses. One instance serves one connection and stops when that connection goes away, so this area gets an
//! instance of its own when it runs next to another live binary.
//!
//! What cannot be asserted here is said rather than assumed: there is no physical board on this machine, so
//! the run shows a simulator answering rather than a board, and without a peer named by the environment the
//! scripted peer in `plugins_wire.rs` stands in for the gate — it answers the shapes but does not decide its
//! own values.

#![cfg(feature = "sitl-tests")]

use std::path::PathBuf;
use std::time::Duration;

use tokio::time::Instant;

use mpcraft_app::plugins::{PluginRuntime, PluginState};
use mpcraft_app::streams::table;
use mpcraft_link::{Session, SessionOptions};
use mpcraft_mavlink::MavCmd;
use mpcraft_mission::{MissionItem, MissionType, Plan};
use mpcraft_plugin::{HostApi, TelemetryField};
use mpcraft_settings::{PluginEntry, Settings};
use mpcraft_transport::ConnectionSpec;

/// How long a simulator is allowed to take to introduce itself.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// How long one transaction is allowed to take.
const TRANSACTION_TIMEOUT: Duration = Duration::from_secs(5);

/// How long the whole list of parameters is allowed to take: a vehicle answers its list item by item, and
/// this build reports well over a thousand of them.
const PARAM_LIST_TIMEOUT: Duration = Duration::from_secs(25);

/// How long a plan is allowed to take to go up, item by item.
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the two rows the surface reads are watched for traffic.
const STREAM_TIMEOUT: Duration = Duration::from_secs(10);

/// The plan this area puts on the vehicle: three items, so that the count the plugin reads is the answer the
/// vehicle gave rather than a number the reader counted.
fn plan() -> Plan {
    let mut plan = Plan::new(MissionType::Mission);

    plan.push(MissionItem {
        command: MavCmd::MAV_CMD_NAV_TAKEOFF,
        params: [15.0, 0.0, 0.0, 0.0],
        x: -35.2809,
        y: 149.1300,
        z: 584.0,
        ..MissionItem::default()
    });
    plan.push(MissionItem {
        command: MavCmd::MAV_CMD_NAV_WAYPOINT,
        params: [0.0, 0.0, 0.0, 0.0],
        x: -35.2815,
        y: 149.1320,
        z: 200.0,
        ..MissionItem::default()
    });
    plan.push(MissionItem {
        command: MavCmd::MAV_CMD_NAV_WAYPOINT,
        params: [0.0, 0.0, 0.0, 0.0],
        x: -35.2820,
        y: 149.1335,
        z: 200.0,
        ..MissionItem::default()
    });

    plan
}

/// The plan this area puts on the vehicle has three items, so that the count the plugin reads is the answer
/// the vehicle gave when it was asked rather than a number the reader counted.
const COUNT: u16 = 3;

/// The item the vehicle is asked to be on. It is written into the module before anything runs, because it is
/// one of the three numbers this area itself puts on the wire.
const CURRENT: u16 = 2;

/// The value written to the parameter. It is exactly representable as a single precision number, so the echo
/// the vehicle gives can be compared with what was asked without a rounding dispute.
const FENCE_ALT_MAX: f64 = 42.5;

/// The code the plugin returns when what it read is not what the wire said.
const DISAGREED: i32 = 3;

/// The address the simulator is listening on for this area.
fn target() -> String {
    std::env::var("MPCRAFT_SITL_PLUGINS_TARGET")
        .or_else(|_| std::env::var("MPCRAFT_SITL_TARGET"))
        .unwrap_or_else(|_| "tcp:127.0.0.1:5760".to_owned())
}

/// A plugin that compares what it reads through its imports with the three numbers this area put on the
/// wire, and says that the other two readings are numbers rather than the absence of one: before the wire
/// answers there is nothing to read, and the number that means *nothing was said* is not a satellite count.
const MODULE: &str = r#"
    (module
        (import "host" "mission_current" (func $current (result i32)))
        (import "host" "mission_count" (func $count (result i32)))
        (import "host" "param" (func $param (param i32 i32) (result f64)))
        (import "host" "telemetry" (func $telemetry (param i32) (result f64)))
        (memory (export "memory") 1)
        (data (i32.const 0) "FENCE_ALT_MAX")
        (func (export "init") (result i32)
            (if (i32.eq (call $count) (i32.const 3))
                (then
                    (if (i32.eq (call $current) (i32.const 2))
                        (then
                            (if (f64.eq (call $param (i32.const 0) (i32.const 13)) (f64.const 42.5))
                                (then
                                    (if (f64.gt (call $telemetry (i32.const 6)) (f64.const 0))
                                        (then
                                            (if (f64.ge (call $telemetry (i32.const 0)) (f64.const 0))
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

/// Waits for something only a vehicle can answer to.
async fn wait_for(what: &dyn Fn() -> bool) {
    let deadline = Instant::now() + STREAM_TIMEOUT;

    while Instant::now() < deadline && !what() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn a_live_vehicle_answers_the_numbers_a_plugin_reads() {
    let spec = ConnectionSpec::parse(&target()).expect("the target is a connection specification");
    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .expect("the simulator can be reached");

    let vehicle = session
        .wait_for_any_vehicle(CONNECT_TIMEOUT)
        .await
        .expect("a vehicle answers its heartbeat");

    let chosen = Settings {
        plugins: vec![PluginEntry::enabled("battery")],
        ..Settings::default()
    };

    let folder = PluginFolder::new("live");
    folder.write(
        "name = \"battery\"\nversion = \"1.0.0\"\nmodule = \"battery.wasm\"\ncapabilities = [\"mission_access\", \"read_params\", \"read_telemetry\"]\n",
        MODULE,
    );

    let mut runtime = PluginRuntime::new(folder.0.clone());
    let surface = runtime.surface().clone();

    // The first reading is the one that says nothing was said, which is what makes the second half an
    // assertion rather than a restatement: the plugin saw the number that means nothing, not a plan of
    // three items, not a parameter, and not a satellite count.
    runtime.scan(&chosen).unwrap();
    let info = runtime.open().unwrap();
    assert_eq!(info[0].state, PluginState::Failed, "{info:?}");
    assert_eq!(info[0].error_code.as_deref(), Some("plugin_stage"));
    assert!(
        info[0]
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(&format!("failed (code {DISAGREED})"))),
        "the stage failed because there was nothing to read, not because the vehicle agreed"
    );

    // The two rows the surface reads are asked for, because a vehicle streams by the rates it was given and
    // this run had not been asked before now.
    for row in table() {
        if matches!(row.name, "GLOBAL_POSITION_INT" | "GPS_RAW_INT") {
            session
                .set_message_interval(vehicle, row.id, row.interval_us(), TRANSACTION_TIMEOUT)
                .await
                .expect("the vehicle answers the ask for the row");
        }
    }

    // The parameter is read as the vehicle holds it, before anything is written: what its own list says about
    // FENCE_ALT_MAX is the vehicle's number, and this area does not know it in advance.
    let store = session
        .download_parameters(vehicle, PARAM_LIST_TIMEOUT)
        .await
        .expect("the vehicle answers its list of parameters");

    assert!(
        store.get("FENCE_ALT_MAX").is_some(),
        "the vehicle said something about its own parameter"
    );

    // The write is waited for, and what is compared afterwards is the number the vehicle stored, which is not
    // always the number that was asked for.
    let echoed = session
        .write_parameter(vehicle, "FENCE_ALT_MAX", FENCE_ALT_MAX, TRANSACTION_TIMEOUT)
        .await
        .expect("the vehicle answers the write");
    assert_eq!(
        echoed.value, FENCE_ALT_MAX,
        "the vehicle stored the number it was given"
    );

    // The plan goes up item by item, and the place the vehicle is on is answered by the vehicle rather than
    // assumed by the reader.
    let items = plan();

    session
        .upload_mission(vehicle, &items, UPLOAD_TIMEOUT)
        .await
        .expect("the vehicle accepts the plan");

    session
        .set_current_item(vehicle, CURRENT, TRANSACTION_TIMEOUT)
        .await
        .expect("the vehicle answers with the item it is on");

    surface.record(&session.state());

    // The readings a live vehicle decides for itself are watched until they arrive rather than written down
    // before the run.
    wait_for(&|| {
        surface.mission_count() == u32::from(COUNT)
            && surface.mission_current() == Some(u32::from(CURRENT))
            && surface.param("FENCE_ALT_MAX") == Some(FENCE_ALT_MAX)
            && surface
                .telemetry(TelemetryField::Satellites)
                .is_some_and(|value| value > 0.0)
    })
    .await;

    assert_eq!(
        surface.mission_count(),
        u32::from(COUNT),
        "the count is the answer the vehicle gave when it was asked"
    );
    assert_eq!(
        surface.mission_current(),
        Some(u32::from(CURRENT)),
        "the item is the one the vehicle reported, not the one the reader assumed"
    );
    assert_eq!(surface.param("FENCE_ALT_MAX"), Some(FENCE_ALT_MAX));

    let snapshot = session.state().telemetry;

    // Both readers hold the same snapshot, so this says that the copy path carries the number from the frame
    // to the import — which is the whole of what can be said here, and is said rather than written as an
    // independent reading.
    assert_eq!(
        surface.telemetry(TelemetryField::Satellites),
        snapshot.gps.map(|gps| f64::from(gps.satellites)),
        "the satellites are the ones the positioning message carried"
    );
    assert_eq!(
        surface.telemetry(TelemetryField::Altitude),
        snapshot.position.map(|position| position.alt),
        "the height is the one the position message carried, said in metres"
    );

    // The same plugin, called again over the same surface, agrees with what it reads through its imports:
    // that is a live vehicle's number crossing the boundary, read by a module that has no way to know it
    // except through the import it was granted.
    let info = runtime.open().unwrap();
    assert_eq!(info[0].state, PluginState::Ready, "{info:?}");

    // A name the vehicle has not said still answers with nothing on a surface fed by the wire.
    assert_eq!(runtime.surface().param("NOT_A_PARAMETER"), None);
}
