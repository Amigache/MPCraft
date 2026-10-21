//! The logs of a vehicle, fetched over the link.
//!
//! # What this is
//!
//! A vehicle keeps the logs of its flights in its own memory, and the ground station
//! fetches them over the link: the vehicle is asked for the list of what it has, and
//! then for the pieces of the log that is wanted.
//!
//! The protocol is the one upstream uses: `LOG_REQUEST_LIST` asks for the list, the
//! vehicle answers with a `LOG_ENTRY` for each log it holds, `LOG_REQUEST_DATA` asks
//! for a piece of a log and the vehicle answers with a `LOG_DATA`.
//!
//! # What it is for
//!
//! A log that is only on the vehicle is a log that is lost when the vehicle is
//! reformatted. The ground station that can fetch a log is the ground station that can
//! keep it.

use std::time::Duration;

use mpcraft_mavlink::dialect::messages::{
    LOG_DATA_DATA, LOG_ENTRY_DATA, LOG_REQUEST_DATA_DATA, LOG_REQUEST_LIST_DATA,
};
use mpcraft_mavlink::MavMessage;

use crate::error::LinkError;
use crate::session::Session;
use crate::vehicle::VehicleId;

/// The message id of `LOG_REQUEST_LIST`.
pub const LOG_REQUEST_LIST_ID: u32 = 117;

/// The message id of `LOG_ENTRY`.
pub const LOG_ENTRY_ID: u32 = 118;

/// The message id of `LOG_REQUEST_DATA`.
pub const LOG_REQUEST_DATA_ID: u32 = 119;

/// The message id of `LOG_DATA`.
pub const LOG_DATA_ID: u32 = 120;

/// How much of a log is asked for at a time.
///
/// The message carries ninety bytes, and asking for more would be asking for more than
/// the protocol allows.
const PIECE: u32 = 90;

/// One of the logs a vehicle holds.
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    /// The number of the log, which is what the data is asked for with.
    pub id: u16,
    /// When the log was started, in seconds since the epoch.
    pub time_utc: u32,
    /// How many bytes the log holds.
    pub size: u32,
}

/// Asks the vehicle for the list of the logs it holds.
///
/// The list is a stream: the vehicle answers with one `LOG_ENTRY` for each log it has,
/// and the last one says how many there are.
///
/// # Errors
///
/// Returns an error when the link cannot carry the request, or when the vehicle does
/// not answer in time.
pub async fn list_logs(
    session: &Session,
    vehicle: VehicleId,
    timeout: Duration,
) -> Result<Vec<LogEntry>, LinkError> {
    // The collector is registered before the request is sent: a vehicle on a fast link
    // can answer before the request has even left the session.
    let collector = session
        .begin_collect(
            |message| matches!(message.message, MavMessage::LOG_ENTRY(_)),
            |messages| {
                messages.iter().any(|message| match &message.message {
                    MavMessage::LOG_ENTRY(entry) => entry.id == entry.last_log_num,
                    _ => false,
                })
            },
        )
        .await?;

    session
        .write_message(&MavMessage::LOG_REQUEST_LIST(LOG_REQUEST_LIST_DATA {
            start: 0,
            end: 0xFFFF,
            target_system: vehicle.system_id,
            target_component: vehicle.component_id,
        }))
        .await?;

    let messages = collector.wait(timeout).await?;

    let mut entries: Vec<LogEntry> = messages
        .iter()
        .filter_map(|message| match &message.message {
            MavMessage::LOG_ENTRY(entry) => Some(entry_of(entry)),
            _ => None,
        })
        .collect();

    entries.sort_by_key(|entry| entry.id);

    Ok(entries)
}

/// Fetches a log of the vehicle, and gives the bytes back.
///
/// The log is fetched in pieces of ninety bytes, and the pieces are put together in the
/// order they arrive: the vehicle may answer them in any order, and the offsets are
/// what say where each piece goes.
///
/// # Errors
///
/// Returns an error when the link cannot carry the request, or when the vehicle does
/// not answer in time.
pub async fn read_log(
    session: &Session,
    vehicle: VehicleId,
    id: u16,
    size: u32,
    timeout: Duration,
) -> Result<Vec<u8>, LinkError> {
    let mut bytes: Vec<u8> = vec![0; size as usize];
    let mut fetched = 0_u32;

    while fetched < size {
        let count = PIECE.min(size - fetched);

        // The offsets the piece is expected at are copies, so that the closure
        // outlives the loop that made it.
        let expected_offset = fetched;
        let expected_count = count;

        let collector = session
            .begin_collect(
                |message| matches!(message.message, MavMessage::LOG_DATA(_)),
                move |messages| {
                    messages.iter().any(|message| match &message.message {
                        MavMessage::LOG_DATA(data) => {
                            data.ofs == expected_offset && data.count as u32 >= expected_count
                        }
                        _ => false,
                    })
                },
            )
            .await?;

        session
            .write_message(&MavMessage::LOG_REQUEST_DATA(LOG_REQUEST_DATA_DATA {
                id,
                ofs: fetched,
                count,
                target_system: vehicle.system_id,
                target_component: vehicle.component_id,
            }))
            .await?;

        let messages = collector.wait(timeout).await?;

        let Some(piece) = messages.iter().find_map(|message| match &message.message {
            MavMessage::LOG_DATA(data) => Some(data),
            _ => None,
        }) else {
            return Err(LinkError::Protocol(
                mpcraft_mavlink::ProtocolError::Malformed("the vehicle did not send the log"),
            ));
        };

        let offset = piece.ofs as usize;
        let length = (piece.count as usize).min(piece.data.len());

        if offset + length > bytes.len() {
            return Err(LinkError::Protocol(
                mpcraft_mavlink::ProtocolError::Malformed(
                    "the vehicle sent more than the log holds",
                ),
            ));
        }

        bytes[offset..offset + length].copy_from_slice(&piece.data[..length]);
        fetched += length as u32;
    }

    Ok(bytes)
}

/// One of the logs a vehicle holds, as the message says it.
fn entry_of(entry: &LOG_ENTRY_DATA) -> LogEntry {
    LogEntry {
        id: entry.id,
        time_utc: entry.time_utc,
        size: entry.size,
    }
}

/// The message of a piece of a log, for the tests.
#[allow(dead_code)]
fn piece(id: u16, offset: u32, count: u8) -> MavMessage {
    MavMessage::LOG_DATA(LOG_DATA_DATA {
        ofs: offset,
        id,
        count,
        data: [0; 90],
    })
}
