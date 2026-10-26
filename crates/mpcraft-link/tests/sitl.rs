//! Integration against a live ArduPilot simulator.
//!
//! These tests are behind a feature flag so that `cargo test` does not need a simulator: the unit tests
//! already prove the protocols against a scripted peer, and what a live vehicle adds is the thing a script
//! cannot imitate — a real autopilot choosing its own values, answering with its own encoding, and streaming
//! its own telemetry.
//!
//! ```text
//! cargo test -p mpcraft-link --features sitl-tests
//! ```
//!
//! The target comes from `MPCRAFT_SITL_TARGET`, which is the address the simulator is listening on, and
//! defaults to the port the protocol reserves for it. It is a socket rather than a datagram port because
//! this build of the simulator does not take a device string for UDP, and because it stops when the station
//! it was serving goes away: the run therefore holds one connection and walks through the areas inside it,
//! rather than opening a connection per area and taking the vehicle down after the first one.
//!
//! What is asserted here is behaviour, not duration: each wait has a bounded deadline and polls, so a
//! machine under load is not read as a failure of the code. The timeouts are generous — thirty seconds to see
//! a vehicle, five for a transaction — because a simulator is slower than hardware without being instant.
//!
//! What is not covered is said rather than assumed: the disconnect when the simulator is stopped, because a
//! test that connects to a peer cannot stop that peer; and the radio check, because these builds of the
//! simulator do not bind the RC input port, so nothing can be sent through it. An arming is read as whatever
//! the vehicle answers — this build refused before its checks had settled, for a reason of its own — and the
//! acceptance is proved afterwards by taking the check away, which is the state the vehicle confirms by
//! echoing the write.

#![cfg(feature = "sitl-tests")]

use std::time::Duration;

use tokio::time::Instant;

use mpcraft_link::{
    CommandError, LinkError, Session, SessionOptions, SessionState, VehicleId, VehicleKind,
};
use mpcraft_mavlink::{MavCmd, MavModeFlag, MavResult};
use mpcraft_mission::{MissionItem, MissionType, Plan};
use mpcraft_transport::ConnectionSpec;

/// How long a simulator is allowed to take to introduce itself.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// How long one transaction is allowed to take.
const TRANSACTION_TIMEOUT: Duration = Duration::from_secs(5);

/// How long a streaming message is allowed to take to arrive.
const STREAM_TIMEOUT: Duration = Duration::from_secs(5);

/// How long a simulator is allowed to take to say it can see the satellites and power itself: the state the
/// pre-arm check reads, which hardware has at once and a simulated vehicle works out over the first seconds.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(20);

/// How long the same simulator is allowed to take to accept the arming once its checks are taken away. The
/// runs recorded an acceptance between ten and thirty seconds after the write — one run refused inside twenty
/// — so the window is wider than the health wait rather than the same.
const ARMING_WINDOW: Duration = Duration::from_secs(45);

/// The address the simulator is listening on.
fn target() -> String {
    std::env::var("MPCRAFT_SITL_TARGET").unwrap_or_else(|_| "tcp:127.0.0.1:5760".to_owned())
}

#[tokio::test]
async fn a_live_vehicle_is_read_and_answers() {
    let spec = ConnectionSpec::parse(&target()).expect("the target is a connection specification");
    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .expect("the simulator can be reached");

    let vehicle = session
        .wait_for_any_vehicle(CONNECT_TIMEOUT)
        .await
        .expect("a vehicle answers its heartbeat");

    read_the_family(&session, vehicle).await;
    reads_what_it_streams(&session, vehicle).await;
    answers_its_parameters(&session, vehicle).await;
    gives_a_plan_back(&session, vehicle).await;
    reports_the_mode_it_is_in(&session, vehicle).await;
    arms_when_its_checks_pass(&session, vehicle).await;
    answers_a_command(&session, vehicle).await;

    session.stop().await;
}

/// The family the vehicle belongs to, from what its heartbeat says.
async fn read_the_family(session: &Session, vehicle: VehicleId) {
    // Row 1.9: a board that reports ArduPilot and a quadrotor is read as a copter.
    let state = session.state();
    let seen = state.vehicles.iter().find(|entry| entry.id == vehicle);
    let seen = seen.expect("the vehicle that answered is on the registry");

    println!(
        "seen: {} | kind {:?} | mav_type {:?} | autopilot {:?}",
        seen.display_name(),
        seen.kind,
        seen.mav_type,
        seen.autopilot
    );

    assert_eq!(
        seen.kind,
        Some(VehicleKind::ArduCopter),
        "the vehicle is named by the family it belongs to"
    );
}

/// What the vehicle streams once the station asks for it. A scripted peer cannot show this: the
/// rates a live autopilot has configured on the port it is talking on are all zero here, so nothing
/// arrives until the ask — which is what the experiment that wrote this area found. Before the ask
/// the same run counted six packets, every one of them a heartbeat; after it, a hundred and twenty-one.
async fn reads_what_it_streams(session: &Session, vehicle: VehicleId) {
    let before = session.state();
    println!(
        "before the ask: position {} | attitude {} | gps {} | received {} packets",
        before
            .telemetry
            .position
            .map_or("nothing".to_owned(), |p| format!(
                "{:.5},{:.5}",
                p.lat, p.lng
            )),
        before
            .telemetry
            .attitude
            .map_or("nothing".to_owned(), |a| format!("{:.1} deg", a.yaw)),
        before
            .telemetry
            .gps
            .map_or("nothing".to_owned(), |g| format!("{} sats", g.satellites)),
        before.loss.packets_received
    );

    // The messages the quick view and the map read, asked at the rate the table gives them.
    for (name, rate_hz) in [
        ("ATTITUDE", 10),
        ("GLOBAL_POSITION_INT", 4),
        ("GPS_RAW_INT", 2),
        ("HOME_POSITION", 1),
    ] {
        let id = mpcraft_mavlink::dialect::message_id(name)
            .expect("the dialect names the message the station reads");
        let interval_us = 1_000_000.0 / f64::from(rate_hz);

        let answer = session
            .set_message_interval(vehicle, id, interval_us as f32, TRANSACTION_TIMEOUT)
            .await
            .expect("the vehicle answers the ask");

        println!("asked for {name}: the vehicle answered {answer:?}");
    }

    let arrived = wait_until(session, STREAM_TIMEOUT, |state| {
        state.telemetry.position.is_some()
            && state.telemetry.attitude.is_some()
            && state.telemetry.gps.is_some()
            && state.telemetry.home.is_some()
    })
    .await;

    let after = session.state();
    println!(
        "after the ask: position {} | attitude {} | gps {} | home {} | received {} packets",
        after
            .telemetry
            .position
            .map_or("nothing".to_owned(), |p| format!(
                "{:.5},{:.5}",
                p.lat, p.lng
            )),
        after
            .telemetry
            .attitude
            .map_or("nothing".to_owned(), |a| format!("{:.1} deg", a.yaw)),
        after
            .telemetry
            .gps
            .map_or("nothing".to_owned(), |g| format!("{} sats", g.satellites)),
        after
            .telemetry
            .home
            .map_or("nothing".to_owned(), |h| format!(
                "{:.5},{:.5}",
                h.lat, h.lng
            )),
        after.loss.packets_received
    );

    assert!(arrived, "the vehicle streams what it was asked to stream");

    let telemetry = session.state().telemetry;
    let position = telemetry.position.expect("the position is known");
    let attitude = telemetry.attitude.expect("the attitude is known");

    // The attitude model speaks degrees, so a wire value read as radians would show a heading of 0.15 rather
    // than 9 — this is the conversion at the wire boundary.
    assert!(
        attitude.yaw.abs() <= 180.0,
        "the heading is said in degrees, not as the radians the wire carries"
    );

    assert!(
        position.lat.abs() <= 90.0 && position.lng.abs() <= 180.0,
        "the place is said in degrees"
    );

    let gps = telemetry.gps.expect("the satellites are known");
    assert!(
        gps.satellites > 0,
        "the vehicle says how many satellites it sees"
    );
}

/// The whole list, and a value that comes back as the vehicle stored it.
async fn answers_its_parameters(session: &Session, vehicle: VehicleId) {
    let store = session
        .download_parameters(vehicle, TRANSACTION_TIMEOUT)
        .await
        .expect("the vehicle answers the request for its parameters");

    // A copter reports more than two hundred parameters; a list that stopped at the first page would be read
    // as complete, which is what this asserts against.
    assert!(
        store.len() > 200,
        "the whole list arrives, not the first page"
    );

    let before = session
        .read_parameter(vehicle, "FENCE_ENABLE", TRANSACTION_TIMEOUT)
        .await
        .expect("the vehicle knows its fence setting");

    // The value that is written is chosen so that it differs from what the vehicle holds: writing the value it
    // already had would prove nothing about the echo. The parameter is put back afterwards, because a test that
    // changes a vehicle has to leave it as it found it.
    let asked = if before.value == 0.0 { 1.0 } else { 0.0 };

    let stored = session
        .write_parameter(vehicle, "FENCE_ENABLE", asked, TRANSACTION_TIMEOUT)
        .await
        .expect("the vehicle confirms the write");

    assert_eq!(
        stored.value, asked,
        "the vehicle stores what it was asked for here"
    );

    let restored = session
        .write_parameter(vehicle, "FENCE_ENABLE", before.value, TRANSACTION_TIMEOUT)
        .await
        .expect("the parameter is put back as it was");

    assert_eq!(
        restored.value, before.value,
        "the vehicle is left as it was found"
    );
}

/// What a list of items says about itself, so that the run records the order they came back in.
fn describe(items: &[MissionItem]) -> String {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let command = item.command;
            let frame = item.frame;

            format!(
                "{index}: {command:?} frame {frame} at ({x:.7}, {y:.7}, {z:.1})",
                x = item.x,
                y = item.y,
                z = item.z
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// A plan that goes up and comes back item by item.
async fn gives_a_plan_back(session: &Session, vehicle: VehicleId) {
    // The station asks for the capabilities first, because they decide which encoding the items travel in.
    // Before the ask existed this run came back with the places rounded to single precision — the float form,
    // where a decimal an operator types does not survive — and the run against a live vehicle is what showed
    // it. A simulator advertising the integer capability answers with the integer form once the ask is made,
    // so the equality below is an exact one.
    session
        .request_capabilities(vehicle, TRANSACTION_TIMEOUT)
        .await
        .expect("the vehicle answers the ask for its capabilities");

    assert!(
        session.uses_mission_int(),
        "a live ArduPilot advertises the integer form, so the plan travels in it"
    );

    let mut plan = Plan::new(MissionType::Mission);

    let takeoff = MissionItem::new(MavCmd::MAV_CMD_NAV_TAKEOFF);
    plan.push(takeoff);

    let mut waypoint = MissionItem::new(MavCmd::MAV_CMD_NAV_WAYPOINT);
    // The place is written so that it survives the scaling by 1e7 exactly: a decimal the operator types is not
    // what an integer carries, and the round trip preserves the scaled value.
    waypoint.x = -35.366301;
    waypoint.y = 149.166608;
    waypoint.z = 60.0;
    plan.push(waypoint);

    session
        .upload_mission(vehicle, &plan, TRANSACTION_TIMEOUT)
        .await
        .expect("the vehicle accepts the plan");

    let read_back = session
        .download_mission(vehicle, MissionType::Mission, TRANSACTION_TIMEOUT)
        .await
        .expect("the vehicle gives the plan back");

    assert_eq!(read_back.len(), 2, "the plan comes back with its items");

    // The plan type travels on every message of the exchange, so a plan asked for as a mission comes back as
    // one — the fence used to be impossible here because the generated bindings were built without the
    // extension feature that carries this field.
    assert_eq!(read_back.mission_type, MissionType::Mission);

    println!("sent: {}", describe(&plan.items));
    println!("back: {}", describe(&read_back.items));

    // Sequence 0 is not one of the items: the specification says the first slot of a plan carries the home
    // position instead of the first item, and the live run confirms it — whatever was uploaded, the thing that
    // comes back at sequence 0 is the home, in the absolute frame and measured above mean sea level. So the
    // home is asserted as the home, and the items are compared from sequence 1 onwards.
    let home = &read_back.items[0];
    assert_eq!(
        home.frame,
        mpcraft_mavlink::MavFrame::MAV_FRAME_GLOBAL as u32,
        "the home arrives in the absolute frame, as the specification says"
    );

    let position = session
        .state()
        .telemetry
        .home
        .expect("the vehicle was asked for its home before the plan is read back");

    // Sequence 0 is not one of the items: the specification says the first slot of a plan carries the home
    // position instead of the first item, and the live run answers with the home the vehicle itself reports —
    // the same place the message the station asked for says the vehicle's home is. Before the ask existed this
    // run answered zeroes, because a simulator that has not decided where its home is has no home to say.
    assert_eq!(
        (home.x * 1e7).round() as i64,
        (position.lat * 1e7).round() as i64,
        "sequence 0 is the home the vehicle reports"
    );
    assert_eq!(
        (home.y * 1e7).round() as i64,
        (position.lng * 1e7).round() as i64,
        "sequence 0 is the home the vehicle reports"
    );

    println!(
        "sequence 0 is the home: {:?} at ({:.7}, {:.7}, {:.1})",
        home.command, home.x, home.y, home.z
    );

    // What was sent at sequence 0 does not survive: the vehicle stores its home there instead, so the command
    // that comes back at sequence 0 is the home, not the takeoff that was uploaded. The items that can be
    // compared are the ones from sequence 1 onwards, and on the integer path they come back exactly.
    for (index, item) in read_back.items.iter().enumerate().skip(1) {
        let sent = &plan.items[index];

        // The identity is not compared: `id` is this station's own key for the row, and a download builds its
        // items from the wire. What is compared is what travels.
        assert_eq!(item.command, sent.command, "the command survives the trip");
        assert_eq!(item.frame, sent.frame, "the frame survives the trip");
        assert_eq!(item.params, sent.params, "the parameters survive the trip");
        assert_eq!(item.autocontinue, sent.autocontinue);

        // The place travels as an integer scaled by 1e7, so the equality that holds is the scaled one — and on
        // the integer path it is exact, because nothing between the operator's keyboard and the vehicle narrows
        // the value. On the float path the same comparison failed by one unit of the last digit before the
        // station asked for the capabilities.
        assert_eq!((item.x * 1e7).round(), (sent.x * 1e7).round());
        assert_eq!((item.y * 1e7).round(), (sent.y * 1e7).round());
        assert_eq!(item.z, sent.z);
    }
}

/// The state afterwards, not the answer to the request: the vehicle says its mode in its heartbeat.
async fn reports_the_mode_it_is_in(session: &Session, vehicle: VehicleId) {
    // LOITER in the table the rotorcraft families share.
    let answer = session
        .set_mode(vehicle, 5, MavModeFlag::empty(), TRANSACTION_TIMEOUT)
        .await;

    println!("the vehicle answered the mode change with {answer:?}");

    assert!(
        answer.is_ok(),
        "a live board accepts the mode change it was given"
    );

    // The custom mode is only read when the flag that makes it be read is on the wire, which is what
    // the dictionary says for this command's second parameter; before the helper carried it this same
    // run answered `MAV_RESULT_DENIED`, because the mode the station sent was a parameter the vehicle
    // was told to ignore.
    let reported = wait_until(session, STREAM_TIMEOUT, |state| {
        state
            .telemetry
            .mode
            .as_ref()
            .is_some_and(|mode| mode.custom_mode == 5)
    })
    .await;

    assert!(
        reported,
        "a vehicle that accepts the mode reports it in its heartbeat"
    );
}

/// What a healthy simulator says about itself before anything is asked of it, and what it answers
/// when it is asked to arm.
///
/// Mission Planner's defaults leave `ARMING_CHECK` at 1 — the full checks, not none — and whether those
/// checks pass is the vehicle's own decision: before they settle this build refused with the reason it
/// printed on the wire ("Arm: Gyros inconsistent"), and answered with acceptance once its instruments
/// agreed and it had an estimate. So the area reads the first answer as the vehicle's own, either way,
/// and keeps a refusal apart from a board that never answered.
///
/// A refusal is then proved to be the vehicle's decision rather than a condition this machine invented:
/// the check is written off, the vehicle echoes the write, and the same vehicle arms, says that it is
/// armed in its heartbeat, and answers the disarming. The parameter is put back as it was, because a test
/// that changes a vehicle has to leave it as it found it.
async fn arms_when_its_checks_pass(session: &Session, vehicle: VehicleId) {
    let store = session
        .download_parameters(vehicle, TRANSACTION_TIMEOUT)
        .await
        .expect("the vehicle answers the request for its parameters");

    let check = store
        .iter()
        .find(|entry| entry.name == "ARMING_CHECK")
        .expect("the vehicle knows its arming check");

    println!(
        "health: ARMING_CHECK {} | GPS_TYPE {} | BATT_MONITOR {} | FENCE_ENABLE {}",
        check.value,
        store
            .iter()
            .find(|entry| entry.name == "GPS_TYPE")
            .map_or("nothing".to_owned(), |entry| format!("{}", entry.value)),
        store
            .iter()
            .find(|entry| entry.name == "BATT_MONITOR")
            .map_or("nothing".to_owned(), |entry| format!("{}", entry.value)),
        store
            .iter()
            .find(|entry| entry.name == "FENCE_ENABLE")
            .map_or("nothing".to_owned(), |entry| format!("{}", entry.value))
    );

    // The reason an arming refusal gives is in the words the board prints as it says them, which is why the
    // console draws what the open link is capturing rather than a stream the station asked for: the table does
    // not carry STATUSTEXT, and an ask for it was answered `MAV_RESULT_DENIED` in one run and accepted in
    // another, so nothing here depends on that answer.
    let locked = wait_until(session, HEALTH_TIMEOUT, |state| {
        state
            .telemetry
            .gps
            .is_some_and(|gps| gps.fix_type as u8 >= 2)
            && state.telemetry.battery.is_some()
    })
    .await;

    let state = session.state();
    let gps = state.telemetry.gps;
    let battery = state.telemetry.battery;

    println!(
        "health: gps {} with {} satellites | battery {:.2} V",
        gps.map_or("nothing".to_owned(), |g| format!("{:?}", g.fix_type)),
        gps.map_or(0, |g| g.satellites),
        battery.map_or(0.0, |b| b.voltage)
    );

    assert!(locked, "the vehicle says where it is and what powers it");

    // The answer is read as the vehicle's own. This build refuses the arming before its checks have
    // settled — the two simulated IMUs disagree, which is what the board said on the wire — and answers
    // it afterwards, so the area does not decide for the board which answer is right.
    let first = session.arm(vehicle, TRANSACTION_TIMEOUT).await;
    let mut changed = false;

    match &first {
        Ok(()) => {
            println!("the vehicle accepted the arming with its checks as the defaults left them")
        }
        Err(LinkError::Command(CommandError::Refused { result, .. })) => {
            println!("the vehicle refused the arming with {result:?}");

            for message in &session.state().messages {
                println!("the board said: {}", message.text);
            }

            // A refusal is the vehicle's decision rather than a fault of the link, and this is what shows
            // it: the check is written off — the vehicle echoes the write — and the same vehicle arms. The
            // attempts are polled because the state the check reads is the vehicle's own decision, so the
            // station cannot decide for the board that its instruments agree or that it has an estimate.
            let stored = session
                .write_parameter(vehicle, "ARMING_CHECK", 0.0, TRANSACTION_TIMEOUT)
                .await
                .expect("the vehicle confirms the write");

            assert_eq!(
                stored.value, 0.0,
                "the vehicle stores what it was asked for here"
            );

            changed = true;

            let second = tokio::time::timeout(ARMING_WINDOW, async {
                loop {
                    let answer = session.arm(vehicle, TRANSACTION_TIMEOUT).await;
                    println!("  the vehicle answered the arming again with {answer:?}");

                    if answer.is_ok() {
                        return true;
                    }

                    tokio::time::sleep(Duration::from_millis(1_000)).await;
                }
            })
            .await;

            assert!(
                second.is_ok(),
                "the same vehicle arms when its check is taken away"
            );
        }
        other => panic!("the arming was not read as the vehicle's answer: {other:?}"),
    }

    let says_armed = wait_until(session, STREAM_TIMEOUT, |state| {
        state.telemetry.armed == Some(true)
    })
    .await;

    assert!(
        says_armed,
        "a vehicle that accepts the arming says it is armed in its heartbeat"
    );

    session
        .disarm(vehicle, TRANSACTION_TIMEOUT)
        .await
        .expect("the vehicle answers the disarming");

    if changed {
        let restored = session
            .write_parameter(vehicle, "ARMING_CHECK", check.value, TRANSACTION_TIMEOUT)
            .await
            .expect("the parameter is put back as it was");

        assert_eq!(
            restored.value, check.value,
            "the vehicle is left as it was found"
        );
    }
}

/// What the generic executor reads back from a real autopilot — the result the vehicle chose, not a word this
/// project chose for it. The two answers are kept apart because they are two different things: a command the
/// vehicle carries out, a command the vehicle refuses, and a board that never answered at all.
async fn answers_a_command(session: &Session, vehicle: VehicleId) {
    // MAV_CMD_DO_SET_HOME asked for the home the vehicle already holds changes nothing on the bench, so the
    // answer here is the executor's rather than a state the board happens to be in. This is what the run
    // against the simulator answered with acceptance for a disarmed vehicle.
    let carried_out = session
        .command_long(
            vehicle,
            MavCmd::MAV_CMD_DO_SET_HOME,
            [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            TRANSACTION_TIMEOUT,
        )
        .await
        .expect("the vehicle answers the command");

    assert_eq!(carried_out, MavResult::MAV_RESULT_ACCEPTED);

    // MAV_CMD_DO_CHANGE_SPEED is refused by this build in every state it was tried in — disarmed and armed
    // both answered the same way — so what the area asserts is how the refusal is read, not an answer the
    // board does not give. A refusal must arrive as the vehicle's result rather than as a timeout, because
    // the interface shows the operator which of the two happened.
    match session
        .command_long(
            vehicle,
            MavCmd::MAV_CMD_DO_CHANGE_SPEED,
            [1.0, 12.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            TRANSACTION_TIMEOUT,
        )
        .await
    {
        Ok(result) => println!("the vehicle answered the speed change with {result:?}"),
        Err(LinkError::Command(CommandError::Refused { result, .. })) => {
            println!("the vehicle refused the speed change with {result:?}");
        }
        other => panic!("the speed change was not read as the vehicle's answer: {other:?}"),
    }
}

/// Waits for a condition to hold, polling the state rather than asserting on a duration.
async fn wait_until(
    session: &Session,
    budget: Duration,
    condition: impl Fn(&SessionState) -> bool,
) -> bool {
    let deadline = Instant::now() + budget;

    while Instant::now() < deadline {
        if condition(&session.state()) {
            return true;
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    false
}
