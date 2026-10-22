//! The plan of a vehicle, kept in a file of its own.
//!
//! The vehicle can hold a plan in a file, and the file transfer protocol is how a
//! program reaches it. Upstream has the same way of doing it (`MAVFtp.cs`, and the
//! `chk_usemavftp` box of the plan editor), and it is the way a plan is carried when
//! the vehicle does not want to talk the mission protocol.
//!
//! ```text
//! GCS  ──FILE_TRANSFER_PROTOCOL(list)──▶  vehicle
//! GCS  ◀────FILE_TRANSFER_PROTOCOL──────  vehicle      (the names of the files)
//! GCS  ──FILE_TRANSFER_PROTOCOL(read)───▶  vehicle
//! GCS  ◀────FILE_TRANSFER_PROTOCOL──────  vehicle      (the bytes of the file)
//! ```
//!
//! The file is the one the vehicle keeps the plan in: `@MISSION/mission.dat` for the
//! mission, and one of its own for the fence and for the rally points. Its format is
//! described in [`mpcraft_mission::missionpck`].
//!
//! The functions take the session by reference because the file transfer client needs
//! the session to be shared: the transfer is a long exchange, and the session is what
//! owns the link.

use std::sync::Arc;
use std::time::Duration;

use mpcraft_mission::missionpck;
use mpcraft_mission::MissionType;
use mpcraft_mission::Plan;

use crate::error::LinkError;
use crate::ftp::FtpClient;
use crate::session::Session;

/// How many bytes are asked for at a time when a file is read.
const READ_BLOCK: u8 = 230;

/// How long to wait for the vehicle to answer the file transfer.
const FTP_TIMEOUT: Duration = Duration::from_secs(10);

/// Downloads the plan the vehicle keeps in a file of its own.
///
/// The file is read in blocks and put together again, and the plan is what the file
/// holds.
///
/// # Errors
///
/// Returns an error when the vehicle refuses the request, when it does not answer, or
/// when the file does not hold a plan.
pub async fn download_plan(
    session: &Arc<Session>,
    mission_type: MissionType,
) -> Result<Plan, LinkError> {
    let client = FtpClient::new(Arc::clone(session)).with_timeout(FTP_TIMEOUT);

    let path = missionpck::path_of(mission_type);
    let (session_id, size) = client.open_file_read(&path).await?;

    let mut content = Vec::new();
    let mut offset: u32 = 0;

    while offset < size {
        let block = client.read_file(session_id, offset, READ_BLOCK).await?;

        if block.is_empty() {
            break;
        }

        offset += block.len() as u32;
        content.extend_from_slice(&block);
    }

    let _ = client.terminate_session(session_id).await;

    missionpck::unpack(&content).map_err(|error| {
        LinkError::Rejected(format!("the file of the plan could not be read: {error}"))
    })
}

/// Writes a plan to the file the vehicle keeps it in.
///
/// The file is created — which empties the one that was there — and the plan goes into
/// it in blocks.
///
/// # Errors
///
/// Returns an error when the vehicle refuses the request or when it does not answer.
pub async fn upload_plan(session: &Arc<Session>, plan: &Plan) -> Result<(), LinkError> {
    let client = FtpClient::new(Arc::clone(session)).with_timeout(FTP_TIMEOUT);

    let path = missionpck::path_of(plan.mission_type);
    let content = missionpck::pack(plan);

    client.upload_file(&path, &content).await
}
