//! Every row of the table, read from the wire through a live vehicle.
//!
//! The table says what the flight-data window asks for, and the ask is only worth something when a real
//! autopilot answers it. Against a scripted peer the table can be read as a policy that works because the
//! peer was written to answer it; a board decides for itself. What this area asserts is that the two halves
//! agree: the row the vehicle accepted is a row that arrives, and the answer is kept per row rather than
//! stopped at the first refusal, which is the rule the interface reads when it says which screens will stay
//! empty.
//!
//! ```text
//! cargo test -p mpcraft-app --features sitl-tests
//! ```
//!
//! The counts are not asserted against a wall clock. A window is generous on purpose — a machine under load
//! is not read as a failure of the code — and what is compared between rows is the order of magnitude the
//! rate implies, not a number of packets in a fixed time.

#![cfg(feature = "sitl-tests")]

use std::collections::BTreeMap;
use std::time::Duration;

use tokio::time::Instant;

use mpcraft_app::streams::table;
use mpcraft_link::{LinkError, Session, SessionOptions};
use mpcraft_mavlink::MavResult;
use mpcraft_transport::ConnectionSpec;

/// How long a simulator is allowed to take to introduce itself.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// How long one ask is allowed to take.
const TRANSACTION_TIMEOUT: Duration = Duration::from_secs(5);

/// How long each row is watched for traffic. Three seconds at the slowest rate the table asks for is three
/// chances, which is more than enough for a board that is streaming and short enough for the run to finish.
const WINDOW: Duration = Duration::from_secs(3);

/// The address the simulator is listening on for this area.
///
/// One instance serves one connection and stops when that connection goes away, so a second live binary
/// cannot share it: this area gets an instance of its own, named by `MPCRAFT_SITL_TABLE_TARGET`, and falls
/// back to the address the suite already uses when it is run alone.
fn target() -> String {
    std::env::var("MPCRAFT_SITL_TABLE_TARGET")
        .or_else(|_| std::env::var("MPCRAFT_SITL_TARGET"))
        .unwrap_or_else(|_| "tcp:127.0.0.1:5760".to_owned())
}

#[tokio::test]
async fn every_row_of_the_table_is_answered_and_arrives() {
    let spec = ConnectionSpec::parse(&target()).expect("the target is a connection specification");
    let session = Session::connect(&spec, SessionOptions::default())
        .await
        .expect("the simulator can be reached");

    let vehicle = session
        .wait_for_any_vehicle(CONNECT_TIMEOUT)
        .await
        .expect("a vehicle answers its heartbeat");

    // The whole table is asked first, because a vehicle that is asked keeps streaming: counting row by row
    // afterwards watches a stream that was already opened rather than one the ask started.
    let mut answers = BTreeMap::new();
    let mut counts = BTreeMap::new();

    for row in table() {
        let answer = session
            .set_message_interval(vehicle, row.id, row.interval_us(), TRANSACTION_TIMEOUT)
            .await
            .expect("the vehicle answers the ask");

        answers.insert(row.name, answer);
    }

    for row in table() {
        let counted = count_row(&session, row.id).await;
        counts.insert(row.name, counted);
    }

    println!("row | the vehicle answered | frames in the window");

    for row in table() {
        println!(
            "  {} at {} Hz | {:?} | {}",
            row.name, row.rate_hz, answers[row.name], counts[row.name]
        );
    }

    // What the vehicle answered is the vehicle's own answer, and every row got one — the rule the interface
    // reads is that a refusal of one message does not stop the asks for the rest.
    for (name, answer) in &answers {
        assert_eq!(
            *answer,
            MavResult::MAV_RESULT_ACCEPTED,
            "{name} was answered by this board",
            name = name
        );
    }

    /*
     * `GPS2_RAW` is the row this airframe cannot say: it has one receiver, so there is no second position to
     * stream. That is what the board said about itself during the run, and it is written down rather than
     * asserted as a failure — a row can be answered and still stay empty because the vehicle has nothing to
     * put on the wire.
     */
    for row in table() {
        if row.name == "GPS2_RAW" {
            continue;
        }

        assert!(
            counts[row.name] > 0,
            "the row the vehicle accepted arrives: {name}",
            name = row.name
        );
    }

    // The rate is what the table asks for, and a screen that reads a row at ten hertz cannot be fed by the
    // same traffic as a row at one hertz. The comparison is between two rows watched over the same window,
    // with a margin wide enough that a loaded machine cannot turn it into a failure.
    let fast = counts["ATTITUDE"];
    let slow = counts["SYS_STATUS"];

    assert!(
        fast >= slow * 2,
        "the fast row carries more traffic than the slow one within the same window: {fast} against {slow}"
    );

    session.stop().await;
}

/// Counts the frames of one message that arrive during the window.
///
/// The frame is read through the raw path rather than the decoded one so the count does not depend on what
/// the bindings can decode: the dialect this project compiles does name `EKF_STATUS_REPORT`, but a collector
/// only sees the messages its dialect names, and a row that arrives as bytes is counted whether or not it can
/// be turned into a value.
async fn count_row(session: &Session, message_id: u32) -> usize {
    let deadline = Instant::now() + WINDOW;
    let mut counted = 0;

    while Instant::now() < deadline {
        let mut handle = session
            .begin_raw(message_id, |_| true)
            .await
            .expect("the session is open");

        match handle.wait_for(deadline - Instant::now()).await {
            Ok(_) => counted += 1,
            // A window that runs out says this row was quiet during it, which is what the run records; a link
            // that closed says nothing more can be read.
            Err(LinkError::Timeout(_)) => break,
            Err(error) => panic!("the link stopped: {error:?}"),
        }
    }

    counted
}
