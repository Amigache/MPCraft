//! MAVLink FTP client for the messages the dialect does not carry.
//!
//! The protocol speaks with [`crate::session::RawMessage`]s of id
//! [`FILE_TRANSFER_PROTOCOL`](mpcraft_mavlink::dialect) — 110. Every payload is a header of
//! twelve bytes followed by up to 239 bytes of data, padded on the wire to a total of 254
//! bytes.

use std::time::Duration;

use crate::error::LinkError;
use crate::session::{RawMessage, Session};

/// The id of the `FILE_TRANSFER_PROTOCOL` message.
const MESSAGE_ID: u32 = 110;
/// The length of the header of the message itself, in bytes.
///
/// The message carries the network, the system and the component the request is for
/// before the transfer begins.
const TARGET_LEN: usize = 3;
/// The length of the header of the transfer, in bytes.
const HEADER_LEN: usize = 12;
/// The total length of the payload of the message, in bytes.
const PAYLOAD_LEN: usize = 254;
/// The bytes of data a payload can carry.
const DATA_LEN: usize = PAYLOAD_LEN - TARGET_LEN - HEADER_LEN;

/// The opcodes of the FTP protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    /// Ignored, always acked.
    None = 0,
    /// Terminates open Read session.
    TerminateSession = 1,
    /// Terminates all open Read sessions.
    ResetSessions = 2,
    /// List files in a path.
    ListDirectory = 3,
    /// Opens file at path for reading, returns session.
    OpenFileRo = 4,
    /// Reads bytes from an open session.
    ReadFile = 5,
    /// Creates file for writing, returns session.
    CreateFile = 6,
    /// Writes bytes to an open session.
    WriteFile = 7,
    /// Remove file.
    RemoveFile = 8,
    /// Creates directory.
    CreateDirectory = 9,
    /// Removes directory.
    RemoveDirectory = 10,
    /// Opens file for writing, returns session.
    OpenFileWo = 11,
    /// Truncate file to offset.
    TruncateFile = 12,
    /// Rename path1 to path2.
    Rename = 13,
    /// Calculate CRC32.
    CalcFileCrc32 = 14,
    /// Burst read.
    BurstReadFile = 15,
    /// List with times.
    ListDirectoryWithTime = 16,
    /// Ack response.
    Ack = 128,
    /// Nak response.
    Nak = 129,
}

impl Opcode {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::None),
            1 => Some(Self::TerminateSession),
            2 => Some(Self::ResetSessions),
            3 => Some(Self::ListDirectory),
            4 => Some(Self::OpenFileRo),
            5 => Some(Self::ReadFile),
            6 => Some(Self::CreateFile),
            7 => Some(Self::WriteFile),
            8 => Some(Self::RemoveFile),
            9 => Some(Self::CreateDirectory),
            10 => Some(Self::RemoveDirectory),
            11 => Some(Self::OpenFileWo),
            12 => Some(Self::TruncateFile),
            13 => Some(Self::Rename),
            14 => Some(Self::CalcFileCrc32),
            15 => Some(Self::BurstReadFile),
            16 => Some(Self::ListDirectoryWithTime),
            128 => Some(Self::Ack),
            129 => Some(Self::Nak),
            _ => None,
        }
    }
}

/// The error code returned in a NAK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FtpError {
    /// No error.
    None = 0,
    /// The command failed.
    Fail = 1,
    /// The command failed, and the errno is in the second byte of the data.
    FailErrno = 2,
    /// The size of the data is not valid.
    InvalidDataSize = 3,
    /// The session is not open.
    InvalidSession = 4,
    /// Every session is in use.
    NoSessionsAvailable = 5,
    /// The offset is past the end of the file.
    Eof = 6,
    /// The vehicle does not know the command.
    UnknownCommand = 7,
    /// The file exists already.
    FailFileExists = 8,
    /// The file is protected.
    FailFileProtected = 9,
    /// The file was not found.
    FileNotFound = 10,
}

impl FtpError {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::None),
            1 => Some(Self::Fail),
            2 => Some(Self::FailErrno),
            3 => Some(Self::InvalidDataSize),
            4 => Some(Self::InvalidSession),
            5 => Some(Self::NoSessionsAvailable),
            6 => Some(Self::Eof),
            7 => Some(Self::UnknownCommand),
            8 => Some(Self::FailFileExists),
            9 => Some(Self::FailFileProtected),
            10 => Some(Self::FileNotFound),
            _ => None,
        }
    }
}

/// One entry of a listing of the vehicle's files.
///
/// The vehicle describes each entry with a letter and a name: `F` for a file, `D` for a
/// folder, `S` for something that is skipped. The name of a file carries the size after a
/// tab, and the time of the last change after another one when the listing asked for
/// the times.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FtpEntry {
    /// The name of the entry, without the folder it is in.
    pub name: String,
    /// The whole path of the entry.
    pub path: String,
    /// How many bytes the entry has, or `-1` when the vehicle did not say.
    pub size: i64,
    /// Whether the entry is a folder.
    pub is_directory: bool,
}

/// A parsed FTP message.
#[derive(Debug, Clone)]
pub struct FtpMessage {
    /// The sequence number of the transfer.
    pub seq_number: u16,
    /// The session id.
    pub session: u8,
    /// The opcode of the command or response.
    pub opcode: Opcode,
    /// The size of the data in the payload.
    pub size: u8,
    /// The opcode that this response is for.
    pub req_opcode: u8,
    /// Burst complete flag.
    pub burst_complete: u8,
    /// Padding byte.
    pub padding: u8,
    /// The offset.
    pub offset: u32,
    /// The data payload.
    pub data: Vec<u8>,
}

fn encode_ftp(message: &FtpMessage) -> [u8; PAYLOAD_LEN] {
    let mut payload = [0_u8; PAYLOAD_LEN];

    // The network, the system and the component the request is for. A vehicle that is
    // not the one that was asked answers nothing, so the vehicle of the link is named.
    payload[0] = 0;
    payload[1] = 1;
    payload[2] = 1;

    let start = TARGET_LEN;

    payload[start] = message.seq_number.to_le_bytes()[0];
    payload[start + 1] = message.seq_number.to_le_bytes()[1];
    payload[start + 2] = message.session;
    payload[start + 3] = message.opcode as u8;
    payload[start + 4] = message.size;
    payload[start + 5] = message.req_opcode;
    payload[start + 6] = message.burst_complete;
    payload[start + 7] = message.padding;

    let offset = message.offset.to_le_bytes();
    payload[start + 8] = offset[0];
    payload[start + 9] = offset[1];
    payload[start + 10] = offset[2];
    payload[start + 11] = offset[3];

    // The size of the header says how many bytes are wanted, which is not the same as
    // how many bytes are carried: a request to read a file asks for a block and carries
    // nothing at all.
    let to_copy = message.data.len().min(DATA_LEN);
    payload[start + HEADER_LEN..start + HEADER_LEN + to_copy]
        .copy_from_slice(&message.data[..to_copy]);

    payload
}

fn parse_ftp(raw: &RawMessage) -> Option<FtpMessage> {
    let payload = raw.padded(PAYLOAD_LEN);

    if payload.len() < PAYLOAD_LEN {
        return None;
    }

    let start = TARGET_LEN;

    let seq_number = u16::from_le_bytes([payload[start], payload[start + 1]]);
    let session = payload[start + 2];
    let opcode = Opcode::from_u8(payload[start + 3])?;
    let size = payload[start + 4];
    let req_opcode = payload[start + 5];
    let burst_complete = payload[start + 6];
    let padding = payload[start + 7];
    let offset = u32::from_le_bytes([
        payload[start + 8],
        payload[start + 9],
        payload[start + 10],
        payload[start + 11],
    ]);

    let to_copy = (size as usize).min(DATA_LEN);
    let mut data = vec![0_u8; to_copy];
    data.copy_from_slice(&payload[start + HEADER_LEN..start + HEADER_LEN + to_copy]);

    Some(FtpMessage {
        seq_number,
        session,
        opcode,
        size,
        req_opcode,
        burst_complete,
        padding,
        offset,
        data,
    })
}

/// Whether the message is the answer to a request.
///
/// The vehicle answers with the number of the request plus one, which is how the answer
/// is told from the answer to another request that happens to be in flight.
fn matches_response(request_seq: u16, request_opcode: Opcode) -> impl Fn(&RawMessage) -> bool {
    move |raw: &RawMessage| {
        let Some(message) = parse_ftp(raw) else {
            return false;
        };

        if message.seq_number != request_seq.wrapping_add(1) {
            return false;
        }

        matches!(message.opcode, Opcode::Ack | Opcode::Nak)
            && message.req_opcode == request_opcode as u8
    }
}

/// Reads the entries of a listing and adds them to the list.
///
/// The vehicle describes each entry with a letter and a name that ends in a zero. The
/// name of a file carries the size after a tab, and the time of the last change after
/// another one when the listing was made with the times.
///
/// The number of entries that were read is returned, because that is what the next
/// request of a listing that continues has to say.
fn parse_entries(data: &[u8], folder: &str, with_time: bool, entries: &mut Vec<FtpEntry>) -> usize {
    let mut index = 0;
    let mut read = 0;

    while index < data.len() {
        let kind = data[index];
        index += 1;

        let start = index;
        while index < data.len() && data[index] != 0 {
            index += 1;
        }

        let name = String::from_utf8_lossy(&data[start..index]).to_string();

        // The zero that ends the name is part of the answer.
        if index < data.len() {
            index += 1;
        }

        read += 1;

        match kind {
            // A file: the name carries the size, and the time when it was asked for.
            b'F' => {
                let mut fields = name.split('\t');
                let file_name = fields.next().unwrap_or_default().to_string();
                let size = fields
                    .next()
                    .and_then(|value| value.trim().parse::<u64>().ok())
                    .map(|value| value as i64)
                    .unwrap_or(-1);

                entries.push(FtpEntry {
                    path: join_path(folder, &file_name),
                    name: file_name,
                    size,
                    is_directory: false,
                });
            }
            // A folder: in a listing with the times it carries the same fields as a file.
            b'D' => {
                let file_name = if with_time {
                    name.split('\t').next().unwrap_or_default().to_string()
                } else {
                    name.clone()
                };

                entries.push(FtpEntry {
                    path: join_path(folder, &file_name),
                    name: file_name,
                    size: 0,
                    is_directory: true,
                });
            }
            // Something the vehicle does not want to talk about.
            b'S' => {}
            // Anything else is a file whose name is all the vehicle says.
            _ => {
                if !name.is_empty() {
                    entries.push(FtpEntry {
                        path: join_path(folder, &name),
                        name,
                        size: -1,
                        is_directory: false,
                    });
                }
            }
        }
    }

    read
}

/// The path of an entry of a folder.
fn join_path(folder: &str, name: &str) -> String {
    if folder.is_empty() || folder == "/" {
        format!("/{name}")
    } else {
        format!("{folder}/{name}")
    }
}

/// An FTP client for a vehicle.
#[derive(Debug, Clone)]
pub struct FtpClient {
    session: std::sync::Arc<Session>,
    /// The sequence number for the next transfer.
    seq: std::sync::Arc<std::sync::atomic::AtomicU16>,
    timeout: Duration,
}

impl FtpClient {
    /// Creates a new FTP client over the session of the link.
    #[must_use]
    pub fn new(session: std::sync::Arc<Session>) -> Self {
        Self {
            session,
            seq: std::sync::Arc::new(std::sync::atomic::AtomicU16::new(0)),
            timeout: Duration::from_secs(5),
        }
    }

    /// Sets the timeout for FTP operations.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn next_seq(&self) -> u16 {
        self.seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    async fn exchange(&self, mut message: FtpMessage) -> Result<FtpMessage, LinkError> {
        message.seq_number = self.next_seq();
        let seq = message.seq_number;
        let req_opcode = message.opcode;
        let payload = encode_ftp(&message);

        let raw = self
            .session
            .exchange_raw(
                self.timeout,
                MESSAGE_ID,
                &payload,
                matches_response(seq, req_opcode),
            )
            .await?;

        let response = parse_ftp(&raw).ok_or(LinkError::Protocol(
            mpcraft_mavlink::ProtocolError::Malformed("the FTP response could not be parsed"),
        ))?;

        if response.opcode == Opcode::Nak {
            let error_code = response
                .data
                .first()
                .copied()
                .and_then(FtpError::from_u8)
                .map(|error| format!("{error:?}"))
                .unwrap_or_else(|| "unknown".to_string());

            let errno_value = response.data.get(1).copied();

            let detail = if matches!(
                FtpError::from_u8(response.data.first().copied().unwrap_or(0)),
                Some(FtpError::FailErrno)
            ) {
                format!("FTP {req_opcode:?} failed: {error_code} errno={errno_value:?}")
            } else {
                format!("FTP {req_opcode:?} failed: {error_code}")
            };

            return Err(LinkError::Rejected(detail));
        }

        Ok(response)
    }

    /// Terminates all open sessions.
    pub async fn reset_sessions(&self) -> Result<(), LinkError> {
        let _ = self
            .exchange(FtpMessage {
                seq_number: 0,
                session: 0,
                opcode: Opcode::ResetSessions,
                size: 0,
                req_opcode: 0,
                burst_complete: 0,
                padding: 0,
                offset: 0,
                data: vec![],
            })
            .await?;
        Ok(())
    }

    /// Lists the contents of a directory.
    ///
    /// The vehicle answers with the entries of the folder, and it may need several
    /// answers for a folder with many files: the listing continues with a request that
    /// says how many entries have already been read.
    ///
    /// A vehicle that does not know the listing with the times is asked for the plain
    /// one instead.
    pub async fn list_directory(&self, path: &str) -> Result<Vec<FtpEntry>, LinkError> {
        let mut entries: Vec<FtpEntry> = Vec::new();
        let mut offset: u32 = 0;
        let mut used_list_directory_with_time = true;

        // The folder is asked for as it was written, without a bar at the end. The root
        // is the one folder that keeps its bar, because it is nothing else but it.
        let folder = if path.len() > 1 {
            path.trim_end_matches('/')
        } else {
            path
        };

        loop {
            let opcode = if used_list_directory_with_time {
                Opcode::ListDirectoryWithTime
            } else {
                Opcode::ListDirectory
            };

            let data = folder.as_bytes().to_vec();
            let size = (data.len().min(DATA_LEN)) as u8;

            let result = self
                .exchange(FtpMessage {
                    seq_number: 0,
                    session: 0,
                    opcode,
                    size,
                    req_opcode: 0,
                    burst_complete: 0,
                    padding: 0,
                    offset,
                    data,
                })
                .await;

            let response = match result {
                Ok(response) => response,
                Err(LinkError::Rejected(error)) => {
                    // A vehicle that does not know the listing with the times refuses it.
                    // Only the first answer can mean that: a refusal in the middle of a
                    // listing is a real error.
                    if used_list_directory_with_time
                        && entries.is_empty()
                        && (error.contains("UnknownCommand") || error.contains("Fail"))
                    {
                        used_list_directory_with_time = false;
                        continue;
                    }

                    return Err(LinkError::Rejected(error));
                }
                Err(error) => return Err(error),
            };

            let read = parse_entries(
                &response.data,
                folder,
                used_list_directory_with_time,
                &mut entries,
            );

            if read == 0 {
                break;
            }

            offset += read as u32;
        }

        Ok(entries)
    }

    /// Opens a file for reading.
    pub async fn open_file_read(&self, path: &str) -> Result<(u8, u32), LinkError> {
        let data = path.as_bytes().to_vec();
        let size = ((data.len() as u8) as usize).min(DATA_LEN) as u8;

        let response = self
            .exchange(FtpMessage {
                seq_number: 0,
                session: 0,
                opcode: Opcode::OpenFileRo,
                size,
                req_opcode: 0,
                burst_complete: 0,
                padding: 0,
                offset: 0,
                data,
            })
            .await?;

        let file_size = if response.data.len() >= 4 {
            u32::from_le_bytes([
                response.data[0],
                response.data[1],
                response.data[2],
                response.data[3],
            ])
        } else {
            0
        };

        Ok((response.session, file_size))
    }

    /// Reads from an open session.
    pub async fn read_file(
        &self,
        session: u8,
        offset: u32,
        size: u8,
    ) -> Result<Vec<u8>, LinkError> {
        let response = self
            .exchange(FtpMessage {
                seq_number: 0,
                session,
                opcode: Opcode::ReadFile,
                size,
                req_opcode: 0,
                burst_complete: 0,
                padding: 0,
                offset,
                data: vec![],
            })
            .await?;

        Ok(response.data)
    }

    /// Terminates a session.
    pub async fn terminate_session(&self, session: u8) -> Result<(), LinkError> {
        let _ = self
            .exchange(FtpMessage {
                seq_number: 0,
                session,
                opcode: Opcode::TerminateSession,
                size: 0,
                req_opcode: 0,
                burst_complete: 0,
                padding: 0,
                offset: 0,
                data: vec![],
            })
            .await?;
        Ok(())
    }

    /// Creates a file for writing, and returns the session it is written in.
    ///
    /// The answer carries how many bytes the file has, which is zero for a file that
    /// did not exist.
    pub async fn create_file(&self, path: &str) -> Result<u8, LinkError> {
        let data = path.as_bytes().to_vec();
        let size = data.len().min(DATA_LEN) as u8;

        let response = self
            .exchange(FtpMessage {
                seq_number: 0,
                session: 0,
                opcode: Opcode::CreateFile,
                size,
                req_opcode: 0,
                burst_complete: 0,
                padding: 0,
                offset: 0,
                data,
            })
            .await?;

        Ok(response.session)
    }

    /// Writes a block to a file that is open.
    ///
    /// The answer says how many bytes the file has altogether.
    pub async fn write_file(
        &self,
        session: u8,
        offset: u32,
        data: &[u8],
    ) -> Result<u32, LinkError> {
        let response = self
            .exchange(FtpMessage {
                seq_number: 0,
                session,
                opcode: Opcode::WriteFile,
                size: data.len().min(DATA_LEN) as u8,
                req_opcode: 0,
                burst_complete: 0,
                padding: 0,
                offset,
                data: data.to_vec(),
            })
            .await?;

        Ok(response.offset)
    }

    /// Writes a whole file, and closes it.
    ///
    /// The file is created first — which empties a file that was there — and the bytes
    /// go in blocks of the size the protocol carries.
    pub async fn upload_file(&self, path: &str, content: &[u8]) -> Result<(), LinkError> {
        // Every session is closed first: the vehicle has a few of them, and a transfer
        // that was interrupted holds one of them.
        let _ = self.reset_sessions().await;

        let session = self.create_file(path).await?;

        let mut offset: u32 = 0;

        while offset < content.len() as u32 {
            let end = ((offset as usize) + DATA_LEN).min(content.len());
            let block = &content[offset as usize..end];

            self.write_file(session, offset, block).await?;

            offset = end as u32;
        }

        let _ = self.terminate_session(session).await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ftp_header_is_encoded_and_decoded() {
        let message = FtpMessage {
            seq_number: 7,
            session: 1,
            opcode: Opcode::ListDirectory,
            size: 2,
            req_opcode: 0,
            burst_complete: 0,
            padding: 0,
            offset: 0,
            data: b"/\0".to_vec(),
        };

        let encoded = encode_ftp(&message);
        assert_eq!(encoded[0], 0);
        assert_eq!(encoded[1], 1);
        assert_eq!(encoded[2], 1);
        assert_eq!(encoded[TARGET_LEN], 7);
        assert_eq!(encoded[TARGET_LEN + 1], 0);
        assert_eq!(encoded[TARGET_LEN + 2], 1);
        assert_eq!(encoded[TARGET_LEN + 3], Opcode::ListDirectory as u8);
        assert_eq!(encoded[TARGET_LEN + 4], 2);

        let raw = RawMessage {
            system_id: 1,
            component_id: 1,
            message_id: MESSAGE_ID,
            payload: encoded[..].to_vec(),
        };

        let decoded = parse_ftp(&raw).unwrap();
        assert_eq!(decoded.seq_number, 7);
        assert_eq!(decoded.session, 1);
        assert_eq!(decoded.opcode, Opcode::ListDirectory);
        assert_eq!(decoded.data[0], b'/');
    }

    #[test]
    fn a_nak_is_recognised() {
        let mut payload = [0_u8; PAYLOAD_LEN];
        payload[TARGET_LEN] = 3;
        payload[TARGET_LEN + 1] = 0;
        payload[TARGET_LEN + 2] = 0;
        payload[TARGET_LEN + 3] = Opcode::Nak as u8;
        payload[TARGET_LEN + 4] = 2;
        payload[TARGET_LEN + 5] = Opcode::OpenFileRo as u8;
        payload[TARGET_LEN + 8] = 7;
        payload[TARGET_LEN + HEADER_LEN] = FtpError::FileNotFound as u8;
        payload[TARGET_LEN + HEADER_LEN + 1] = 0xFF;

        let raw = RawMessage {
            system_id: 1,
            component_id: 1,
            message_id: MESSAGE_ID,
            payload: payload[..].to_vec(),
        };

        let decoded = parse_ftp(&raw).unwrap();
        assert_eq!(decoded.opcode, Opcode::Nak);
        assert_eq!(decoded.req_opcode, Opcode::OpenFileRo as u8);
        assert_eq!(decoded.data[0], FtpError::FileNotFound as u8);
    }

    #[test]
    fn a_request_that_asks_for_bytes_and_carries_none_is_encoded() {
        // A request to read a block asks for two hundred and thirty bytes and carries
        // nothing: the size of the header is what is wanted, not what is carried.
        let message = FtpMessage {
            seq_number: 3,
            session: 1,
            opcode: Opcode::ReadFile,
            size: 230,
            req_opcode: 0,
            burst_complete: 0,
            padding: 0,
            offset: 230,
            data: vec![],
        };

        let encoded = encode_ftp(&message);
        assert_eq!(encoded[TARGET_LEN + 4], 230);

        let raw = RawMessage {
            system_id: 1,
            component_id: 1,
            message_id: MESSAGE_ID,
            payload: encoded[..].to_vec(),
        };

        let decoded = parse_ftp(&raw).unwrap();
        assert_eq!(decoded.opcode, Opcode::ReadFile);
        assert_eq!(decoded.size, 230);
        assert_eq!(decoded.offset, 230);
    }

    #[test]
    fn a_listing_is_read_as_entries() {
        // A file with its size, a folder, and something the vehicle skips.
        let data = b"Fboot.txt\t2048\t0\0Dlogs\0Ssecret\0";

        let mut entries = Vec::new();
        let read = parse_entries(data, "/", true, &mut entries);

        assert_eq!(read, 3);
        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].name, "boot.txt");
        assert_eq!(entries[0].size, 2048);
        assert!(!entries[0].is_directory);
        assert_eq!(entries[0].path, "/boot.txt");

        assert_eq!(entries[1].name, "logs");
        assert!(entries[1].is_directory);
        assert_eq!(entries[1].path, "/logs");
    }

    #[test]
    fn a_plain_listing_gives_the_name_of_the_folder_whole() {
        // Without the times, a tab is part of the name.
        let data = b"Dodd\tname\0";

        let mut entries = Vec::new();
        parse_entries(data, "/logs", false, &mut entries);

        assert_eq!(entries[0].name, "odd\tname");
        assert_eq!(entries[0].path, "/logs/odd\tname");
    }

    #[test]
    fn the_answer_carries_the_number_of_the_request_plus_one() {
        let mut payload = [0_u8; PAYLOAD_LEN];
        payload[TARGET_LEN] = 8;
        payload[TARGET_LEN + 3] = Opcode::Ack as u8;
        payload[TARGET_LEN + 5] = Opcode::ListDirectory as u8;

        let raw = RawMessage {
            system_id: 1,
            component_id: 1,
            message_id: MESSAGE_ID,
            payload: payload[..].to_vec(),
        };

        assert!(matches_response(7, Opcode::ListDirectory)(&raw));
        assert!(!matches_response(8, Opcode::ListDirectory)(&raw));
    }
}
