//! The formation loop, over a real link.
//!
//! The chain the row depends on is: the settings that say who sits where, the per-vehicle state of
//! each board, the decision that reads it, and the message that carries the decision to the vehicle
//! it belongs to. Testing it over a UDP socket exercises all four at once.
//!
//! What can be proved here is that the right branch runs for each member, that what reaches the
//! wire is what the loop hand computed, and that the loop stops when it is told to. What cannot be
//! proved without a live fleet is that the vehicles fly into their slots, which is why the parity
//! row stays partial.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpcraft_app::supervisor::SessionSupervisor;
use mpcraft_app::swarm::SwarmService;
use mpcraft_link::{Session, SessionOptions, VehicleId};
use mpcraft_mavlink::dialect::messages::{ATTITUDE_DATA, GLOBAL_POSITION_INT_DATA, HEARTBEAT_DATA};
use mpcraft_mavlink::frame::{encode_v2, FrameParser, MavHeader};
use mpcraft_mavlink::{MavAutopilot, MavMessage, MavType, Message};
use mpcraft_settings::Settings;
use mpcraft_transport::ConnectionSpec;

/// The leader: on the central meridian of its zone, where a direction measured on the grid is a
/// true direction, facing north.
const LEADER_LAT: f64 = -35.0;
const LEADER_LNG: f64 = 3.0; // central meridian of zone 31
const LEADER_ALT: f64 = 100.0;

/// A place sixty metres due south of the leader, on the same meridian.
fn south_of_leader(metres: f64) -> f64 {
    LEADER_LAT - metres * 180.0 / (std::f64::consts::PI * 6_371_000.0)
}

/// Writes one frame to the ground station, from the board named by `system_id`.
async fn send(
    socket: &Arc<tokio::net::UdpSocket>,
    peer: SocketAddr,
    system_id: u8,
    message: &MavMessage,
    sequence: u8,
) {
    let header = MavHeader {
        system_id,
        component_id: 1,
        sequence,
    };

    socket
        .send_to(&encode_v2(message, header), peer)
        .await
        .unwrap();
}

fn heartbeat(airframe: MavType) -> MavMessage {
    MavMessage::HEARTBEAT(HEARTBEAT_DATA {
        mavtype: airframe,
        autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
        ..Default::default()
    })
}

fn position(lat: f64, lng: f64, alt: f64, north: f64) -> MavMessage {
    MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
        lat: (lat * 1e7).round() as i32,
        lon: (lng * 1e7).round() as i32,
        alt: (alt * 1e3).round() as i32,
        vx: (north * 100.0).round() as i16,
        ..Default::default()
    })
}

fn attitude(yaw_deg: f64) -> MavMessage {
    MavMessage::ATTITUDE(ATTITUDE_DATA {
        // The wire carries radians; the model reads degrees.
        yaw: (yaw_deg * std::f64::consts::PI / 180.0) as f32,
        ..Default::default()
    })
}

/// Records what the ground station sends back — only the orders, so that the ground station's own
/// announcements are not counted with them.
fn spawn_recorder(socket: Arc<tokio::net::UdpSocket>, sink: Arc<Mutex<Vec<MavMessage>>>) {
    tokio::spawn(async move {
        let mut recorder = FrameParser::new();
        let mut buffer = [0u8; 2048];

        while let Ok((len, _)) = socket.recv_from(&mut buffer).await {
            let mut frames = Vec::new();

            if recorder.feed(&buffer[..len], &mut frames).is_err() {
                continue;
            }

            for frame in frames {
                let Ok(message) = mpcraft_mavlink::dialect::Dialect::parse(
                    frame.version,
                    frame.message_id,
                    &frame.payload,
                ) else {
                    continue;
                };

                let is_an_order = matches!(
                    message,
                    MavMessage::SET_ATTITUDE_TARGET(_)
                        | MavMessage::SET_POSITION_TARGET_GLOBAL_INT(_)
                        | MavMessage::COMMAND_LONG(_)
                );

                if is_an_order {
                    sink.lock().unwrap().push(message);
                }
            }
        }
    });
}

/// Reads the announcement of the ground station and says where it spoke from.
async fn peer_of(socket: &Arc<tokio::net::UdpSocket>) -> SocketAddr {
    let mut buffer = [0u8; 2048];
    let (len, peer) = socket.recv_from(&mut buffer).await.unwrap();

    assert!(
        buffer[..len].iter().any(|&b| b == 0xFD || b == 0xFE),
        "the ground station announced itself with a frame"
    );

    peer
}

#[tokio::test]
async fn the_loop_tells_each_member_what_the_hand_computed() {
    // The test plays four boards on one link: the leader, a plane in a slot, and two
    // multicopters — one past the heading band and one inside it.
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
    let address = socket.local_addr().unwrap();
    let spec = ConnectionSpec::parse(&format!("udp:{address}")).unwrap();

    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap();

    let peer = peer_of(&socket).await;
    let recorded: Arc<Mutex<Vec<MavMessage>>> = Arc::new(Mutex::new(Vec::new()));
    spawn_recorder(Arc::clone(&socket), Arc::clone(&recorded));

    let plane = MavType::MAV_TYPE_FIXED_WING;
    let quad = MavType::MAV_TYPE_QUADROTOR;

    let mut sequence = 0u8;

    // Two sightings each, so that every board qualifies as an autopilot seen twice and the
    // choice falls on the first that qualified — the leader.
    for (system_id, airframe) in [(1, plane), (2, plane), (3, quad), (4, quad)] {
        for _ in 0..2 {
            send(&socket, peer, system_id, &heartbeat(airframe), sequence).await;
            sequence += 1;
        }
    }

    // The states the loop needs: a place, a heading, and for the leader a speed.
    send(
        &socket,
        peer,
        1,
        &position(LEADER_LAT, LEADER_LNG, LEADER_ALT, 8.0),
        sequence,
    )
    .await;
    sequence += 1;
    send(&socket, peer, 1, &attitude(0.0), sequence).await;
    sequence += 1;

    // The plane sits sixty metres due south of its own slot, which is the leader's place.
    send(
        &socket,
        peer,
        2,
        &position(south_of_leader(60.0), LEADER_LNG, LEADER_ALT, 0.0),
        sequence,
    )
    .await;
    sequence += 1;
    send(&socket, peer, 2, &attitude(0.0), sequence).await;
    sequence += 1;

    send(
        &socket,
        peer,
        3,
        &position(-35.2, LEADER_LNG, 90.0, 0.0),
        sequence,
    )
    .await;
    sequence += 1;
    send(&socket, peer, 3, &attitude(10.0), sequence).await;
    sequence += 1;

    send(
        &socket,
        peer,
        4,
        &position(-35.3, LEADER_LNG, 95.0, 0.0),
        sequence,
    )
    .await;
    sequence += 1;
    send(&socket, peer, 4, &attitude(2.0), sequence).await;

    let file = Settings {
        connection_profiles: Vec::new(),
        favourite_parameters: Vec::new(),
        signing_keys: Vec::new(),
        plugins: Vec::new(),
        map_injection_folder: None,
        swarm_leader: Some("1:1".to_owned()),
        swarm_enabled: false,
        swarm_slots: vec![
            mpcraft_settings::SwarmSlot::with_offset("2:1", 0.0, 0.0, 0.0),
            mpcraft_settings::SwarmSlot::with_offset("3:1", 0.0, 40.0, 5.0),
            mpcraft_settings::SwarmSlot::with_offset("4:1", 0.0, 40.0, 0.0),
        ],
    };

    let deadline = std::time::Instant::now() + Duration::from_secs(5);

    loop {
        let state = session.state();

        if state.telemetries.len() == 4 && state.telemetries.values().all(|t| t.position.is_some())
        {
            break;
        }

        if std::time::Instant::now() > deadline {
            panic!("the four boards were never read apart: {state:?}");
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // The choice fell on the leader, which is what the settings name.
    assert_eq!(session.state().selected, Some(VehicleId::new(1, 1)));

    let service = SwarmService::new();
    service.apply(&file);

    let told = service.dispatch(&session).await.expect("the loop runs");
    assert_eq!(told, 3, "three members were told");

    let deadline = std::time::Instant::now() + Duration::from_secs(5);

    while recorded.lock().unwrap().len() < 4 && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let seen = recorded.lock().unwrap().clone();
    assert_eq!(seen.len(), 4, "what reached the wire: {seen:?}");

    // The plane's answer, as the hand worked it out: the identity quaternion, because the
    // vehicle already points where the slot puts it, and the thrust demand
    // 60 × 0.01 + 60 × 0.001 × 0.1 = 0.606 with the mask 0x38.
    let answer = seen
        .iter()
        .find(|message| matches!(message, MavMessage::SET_ATTITUDE_TARGET(_)))
        .expect("the plane branch answers with the quaternion");

    let MavMessage::SET_ATTITUDE_TARGET(plane_answer) = answer else {
        panic!("the plane branch answers with the quaternion");
    };

    assert_eq!(
        (plane_answer.target_system, plane_answer.target_component),
        (2, 1)
    );
    assert_eq!(plane_answer.type_mask.bits(), 0x38);
    assert!(
        (f64::from(plane_answer.thrust) - 0.606).abs() < 0.01,
        "thrust: {}",
        plane_answer.thrust
    );
    assert!((f64::from(plane_answer.q[0]) - 1.0).abs() < 1e-9);
    for term in &plane_answer.q[1..] {
        assert!(f64::from(*term).abs() < 1e-9, "term: {term}");
    }

    // The multicopter past the band: its own slot, the leader's speeds, and its heading asked
    // for separately. Forty metres of grid north read as forty divided by the scale factor of
    // the projection, which on the central meridian is 0.9996.
    let expected_lat = LEADER_LAT + 40.0 * 180.0 / (std::f64::consts::PI * 0.9996 * 6_371_000.0);

    // The two targets are read by who they were addressed to, not by the order they arrived in.
    let target_for = |system_id: u8| -> &MavMessage {
        seen.iter()
            .find(|message| matches!(message, MavMessage::SET_POSITION_TARGET_GLOBAL_INT(target) if target.target_system == system_id))
            .expect("a multicopter was told its slot")
    };

    let targets = [target_for(3), target_for(4)];
    assert_eq!(targets.len(), 2, "two multicopters were told their slots");

    let MavMessage::SET_POSITION_TARGET_GLOBAL_INT(target) = targets[0] else {
        panic!("the multicopter branch answers with the position target");
    };

    assert_eq!((target.target_system, target.target_component), (3, 1));
    assert!(
        (f64::from(target.lat_int) - expected_lat * 1e7).abs() < 100.0,
        "lat_int: {}",
        target.lat_int
    );
    assert!((f64::from(target.lon_int) - LEADER_LNG * 1e7).abs() < 2.0);
    assert_eq!(target.alt, 105.0);
    assert_eq!((target.vx, target.vy, target.vz), (8.0, 0.0, 0.0));
    // The frame as the number upstream sends it, not as its modern synonym.
    assert_eq!(target.coordinate_frame as u32, 6);
    assert_eq!(target.type_mask.bits(), 3520);

    // And vehicle 4, inside the band: its slot, and no command for its heading.
    let MavMessage::SET_POSITION_TARGET_GLOBAL_INT(quiet) = targets[1] else {
        panic!("vehicle 4 was told its slot");
    };

    assert_eq!((quiet.target_system, quiet.target_component), (4, 1));
    assert_eq!(quiet.alt, 100.0);

    // One command, sent because ten degrees is past the three-degree band, and it went to
    // vehicle 3 rather than to vehicle 4.
    let commands: Vec<&MavMessage> = seen
        .iter()
        .filter(|message| matches!(message, MavMessage::COMMAND_LONG(_)))
        .collect();
    assert_eq!(
        commands.len(),
        1,
        "only the vehicle past the band was asked to turn"
    );

    let MavMessage::COMMAND_LONG(turn) = commands[0] else {
        panic!("the heading travels as a command");
    };

    assert_eq!(turn.command as u32, 115);
    assert_eq!((turn.target_system, turn.target_component), (3, 1));
    assert_eq!(turn.param1, 0.0);
    assert_eq!(turn.param2, 100.0);

    // The leader is never told anything.
    assert!(
        !seen.iter().any(|message| {
            matches!(
                message,
                MavMessage::SET_ATTITUDE_TARGET(plane_answer) if plane_answer.target_system == 1
            )
        }),
        "the loop skips the leader"
    );

    session.stop().await;
}

#[tokio::test]
async fn the_loop_ticks_on_its_own_and_stops_when_it_is_told_to() {
    // Two boards on one link: the leader and one member in a slot. The loop is started through the
    // supervisor rather than by hand, so what is being checked is that it asks for the link on
    // every tick and that a stop is honoured between the sends and the next tick.
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
    let address = socket.local_addr().unwrap();

    let supervisor = SessionSupervisor::new();
    supervisor
        .connect(&format!("udp:{address}"), |_snapshot| {})
        .await
        .expect("the link opens");

    let peer = peer_of(&socket).await;

    let recorded: Arc<Mutex<Vec<MavMessage>>> = Arc::new(Mutex::new(Vec::new()));
    spawn_recorder(Arc::clone(&socket), Arc::clone(&recorded));

    let mut sequence = 0u8;

    for system_id in [1, 2] {
        for _ in 0..2 {
            send(
                &socket,
                peer,
                system_id,
                &heartbeat(MavType::MAV_TYPE_QUADROTOR),
                sequence,
            )
            .await;
            sequence += 1;
        }
    }

    send(
        &socket,
        peer,
        1,
        &position(LEADER_LAT, LEADER_LNG, LEADER_ALT, 8.0),
        sequence,
    )
    .await;
    sequence += 1;
    send(&socket, peer, 1, &attitude(0.0), sequence).await;
    sequence += 1;

    send(
        &socket,
        peer,
        2,
        &position(south_of_leader(60.0), LEADER_LNG, LEADER_ALT, 0.0),
        sequence,
    )
    .await;
    sequence += 1;
    send(&socket, peer, 2, &attitude(0.0), sequence).await;

    let file = Settings {
        connection_profiles: Vec::new(),
        favourite_parameters: Vec::new(),
        signing_keys: Vec::new(),
        plugins: Vec::new(),
        map_injection_folder: None,
        swarm_leader: Some("1:1".to_owned()),
        swarm_enabled: true,
        swarm_slots: vec![mpcraft_settings::SwarmSlot::with_offset(
            "2:1", 0.0, 0.0, 0.0,
        )],
    };

    let service = SwarmService::new();
    service.apply(&file);

    // The loop is started by the decision that was kept, and a second start is refused rather
    // than running two loops over the same link.
    service.start(&supervisor).expect("the loop starts");
    service
        .start(&supervisor)
        .expect_err("the loop is not started twice");

    let deadline = std::time::Instant::now() + Duration::from_secs(5);

    // More than one order means more than one tick: the loop is running on its own, not because
    // something called it once.
    while recorded.lock().unwrap().len() < 2 && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let ticking = recorded.lock().unwrap().len();
    assert!(ticking >= 2, "the loop ticked on its own: {ticking} orders");

    service.stop();

    // What was sent before the stop is kept; what comes after it is nothing, which is what a stop
    // means for a loop that waits on the wire.
    let stopped_at = recorded.lock().unwrap().len();
    tokio::time::sleep(Duration::from_millis(400)).await;

    assert_eq!(
        recorded.lock().unwrap().len(),
        stopped_at,
        "the loop stopped when it was told to"
    );

    supervisor.disconnect().await.expect("the link closes");
}
