//! The files of the vehicle, read as the vehicle says them.
//!
//! A file is opened, read block by block, and a folder is listed entry by entry; in both cases the end is a code
//! rather than a failure. Three things are shown here, and all three came from a real board.
//!
//! The number a vehicle gives for a file it generates when asked is not the length of the file: the board on the
//! bench said 100000 bytes for one of its text files and answered with 122. A reader that counts to the number it
//! was given keeps asking after the file is done, so what decides when the reading stops is the code the vehicle
//! names — the one that says there is nothing more — rather than the number the open carried. The same code ends a
//! listing: the ask after the last entry is answered with it, so a reader that read it as a failure would throw
//! away a folder the vehicle had listed.
//!
//! And the two numbers are kept apart in what the reader returns, because an interface that showed only the number
//! would show a file that is not there.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use mpcraft_link::{
    says_the_end, FtpClient, FtpError, LinkError, Session, SessionOptions, VehicleId,
};
use mpcraft_mavlink::dialect::messages::HEARTBEAT_DATA;
use mpcraft_mavlink::frame::{encode_v2, encode_v2_bytes, FrameParser, MavHeader};
use mpcraft_mavlink::{MavAutopilot, MavMessage, MavType};

/// The message the transfer protocol speaks with, as the published dictionary numbers it.
const MESSAGE_ID: u32 = 110;

/// The three bytes that name where a request is for, before the header of the transfer.
const TARGET_LEN: usize = 3;

/// The length of the header of the transfer.
const HEADER_LEN: usize = 12;

/// The bytes a payload can carry.
const DATA_LEN: usize = 239;

/// The opcodes, as the published dictionary numbers them.
mod opcode {
    pub const TERMINATE_SESSION: u8 = 1;
    pub const LIST_DIRECTORY: u8 = 3;
    pub const LIST_DIRECTORY_WITH_TIME: u8 = 16;
    pub const OPEN_FILE_RO: u8 = 4;
    pub const READ_FILE: u8 = 5;
    pub const ACK: u8 = 128;
    pub const NAK: u8 = 129;
}

/// The codes of a refusal, as the published dictionary numbers them.
mod error {
    pub const GENERIC_FAIL: u8 = 1;
    pub const END_OF_FILE: u8 = 6;
    pub const FILE_NOT_FOUND: u8 = 10;
}

/// A file the vehicle holds: what it says about it and what it gives.
struct File {
    /// The path the file is asked for.
    path: String,
    /// How many bytes the vehicle says the file has when it is opened.
    announced: u32,
    /// The bytes it answers with, which for a file it generates when asked are fewer than the number.
    content: Vec<u8>,
}

/// A folder the vehicle holds: what it is called and what it answers with.
#[derive(Clone)]
struct Folder {
    /// The path the folder is asked for.
    path: String,
    /// The entries it holds, each one with the letter the vehicle uses and its name.
    entries: Vec<(u8, String)>,
}

/// A simulated vehicle with the files it holds.
struct Vehicle {
    socket: tokio::net::UdpSocket,
    sequence: u8,
    files: Vec<File>,
    folders: Vec<Folder>,
    /// The path of the file that is open, because a read is answered only while one is.
    open: Option<String>,
}

impl Vehicle {
    async fn start(files: Vec<File>, folders: Vec<Folder>) -> SocketAddr {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();

        let mut vehicle = Self {
            socket,
            sequence: 0,
            files,
            folders,
            open: None,
        };

        tokio::spawn(async move {
            vehicle.run().await;
        });

        address
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
                // The ground station announces itself with a heartbeat when it connects; the test is not about
                // that, but the vehicle needs it to know where to send. One sighting is not enough for the
                // handshake to call a board the vehicle, so this one announces itself twice — which is what a real
                // vehicle gets by streaming its heartbeat.
                if frame.message_id == 0 {
                    self.announce(peer).await;
                    self.announce(peer).await;
                    continue;
                }

                if frame.message_id != MESSAGE_ID {
                    continue;
                }

                // A MAVLink 2 payload says how many bytes are there, and the zeros at the end of it are trimmed on
                // the wire, so the header is read from the buffer padded back to the length the message carries.
                // Without this the bytes of the header that happened to be zero would be missing, and the offsets
                // below would read past what arrived — which is the same rule `RawMessage::padded` follows.
                let mut padded = vec![0u8; TARGET_LEN + HEADER_LEN + DATA_LEN];
                let taken = frame.payload.len().min(padded.len());
                padded[..taken].copy_from_slice(&frame.payload[..taken]);

                let body = &padded[TARGET_LEN..];

                let seq = u16::from_le_bytes([body[0], body[1]]);
                let session = body[2];
                let asked = body[3];
                let size = usize::from(body[4]);
                let offset = u32::from_le_bytes([body[8], body[9], body[10], body[11]]);
                let path = String::from_utf8_lossy(&body[HEADER_LEN..HEADER_LEN + size])
                    .trim_end_matches('\0')
                    .to_string();

                match asked {
                    opcode::OPEN_FILE_RO => {
                        // The number is taken out of the vehicle before the answer goes out, because sending needs
                        // the vehicle to be mutable while the file was read from it.
                        let announced = match self.files.iter().find(|entry| entry.path == path) {
                            Some(entry) => entry.announced,
                            None => {
                                self.refused(seq, session, asked, error::FILE_NOT_FOUND, peer)
                                    .await;
                                continue;
                            }
                        };

                        self.open = Some(path);

                        // The answer carries the number the vehicle says for the file, as four bytes.
                        self.answered(seq, session, asked, &announced.to_le_bytes(), peer)
                            .await;
                    }
                    opcode::READ_FILE => {
                        let Some(path) = self.open.clone() else {
                            self.refused(seq, session, asked, error::FILE_NOT_FOUND, peer)
                                .await;
                            continue;
                        };

                        // The bytes are taken out of the vehicle before the answer goes out, because sending needs
                        // the vehicle to be mutable while the file was read from it.
                        let held = self
                            .files
                            .iter()
                            .find(|entry| entry.path == path)
                            .map(|entry| entry.content.clone())
                            .expect("the open file is one the vehicle holds");

                        if usize::try_from(offset).unwrap_or(usize::MAX) >= held.len() {
                            self.refused(seq, session, asked, error::END_OF_FILE, peer)
                                .await;
                            continue;
                        }

                        let from = usize::try_from(offset).unwrap();
                        let gave = (held.len() - from).min(size);
                        let bytes = held[from..from + gave].to_vec();

                        self.answered(seq, session, asked, &bytes, peer).await;
                    }
                    opcode::LIST_DIRECTORY_WITH_TIME => {
                        // A command this build does not know is answered with a generic failure that names no
                        // number, which is what the board on the bench did — and why the fall-back rule accepts a
                        // failure as well as the shape the specification gives for a server that does not implement
                        // a command.
                        self.refused(seq, session, asked, error::GENERIC_FAIL, peer)
                            .await;
                    }
                    opcode::LIST_DIRECTORY => {
                        if offset > 0 {
                            // The ask after the last entry is answered with the number that says there is nothing
                            // more, which is the same code that ends a file: the end travels as data rather than as
                            // a failure, and a reader that read it as a failure would report a folder it had read
                            // as one that failed.
                            self.refused(seq, session, asked, error::END_OF_FILE, peer)
                                .await;
                            continue;
                        }

                        let Some(folder) = self.folders.iter().find(|entry| entry.path == path)
                        else {
                            self.refused(seq, session, asked, error::FILE_NOT_FOUND, peer)
                                .await;
                            continue;
                        };

                        // The entries go out in the shape this build writes them: a letter, the name, the zero the
                        // format puts after it, and the zero the writer adds for itself — which is what a real board
                        // put on the wire.
                        let held = folder.clone();

                        let mut data = Vec::new();
                        for (letter, name) in held.entries {
                            data.push(letter);
                            data.extend_from_slice(name.as_bytes());
                            data.push(0);
                            data.push(0);
                        }

                        self.answered(seq, session, asked, &data, peer).await;
                    }
                    opcode::TERMINATE_SESSION => {
                        self.open = None;
                        self.answered(seq, session, asked, &[], peer).await;
                    }
                    other => {
                        // A command the vehicle does not know is answered with a generic failure that names no
                        // number, which is what a real board does.
                        self.refused(seq, session, other, 1, peer).await;
                    }
                }
            }
        }
    }

    async fn announce(&mut self, peer: SocketAddr) {
        let header = MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.wrapping_add(1);

        let heartbeat = MavMessage::HEARTBEAT(HEARTBEAT_DATA {
            custom_mode: 0,
            mavtype: MavType::MAV_TYPE_QUADROTOR,
            autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
            ..Default::default()
        });

        let _ = self
            .socket
            .send_to(&encode_v2(&heartbeat, header), peer)
            .await;
    }

    /// Answers an ask, with the number of the request plus one, which is how the station tells the answers apart.
    async fn answered(&mut self, seq: u16, session: u8, asked: u8, data: &[u8], peer: SocketAddr) {
        self.send(seq, session, asked, opcode::ACK, data, peer)
            .await;
    }

    /// Refuses an ask, keeping the code as a byte rather than as a sentence.
    async fn refused(&mut self, seq: u16, session: u8, asked: u8, code: u8, peer: SocketAddr) {
        self.send(seq, session, asked, opcode::NAK, &[code], peer)
            .await;
    }

    async fn send(
        &mut self,
        seq: u16,
        session: u8,
        asked: u8,
        kind: u8,
        data: &[u8],
        peer: SocketAddr,
    ) {
        let mut payload = vec![0u8; TARGET_LEN + HEADER_LEN + DATA_LEN];

        // The answer goes to the pair that asked for it, which is the vehicle the link chose rather than an
        // address this test assumes.
        payload[0] = 0;
        payload[1] = 1;
        payload[2] = 1;

        let start = TARGET_LEN;
        payload[start..start + 2].copy_from_slice(&(seq + 1).to_le_bytes());
        payload[start + 2] = session;
        payload[start + 3] = kind;
        payload[start + 4] = data.len().min(DATA_LEN) as u8;
        payload[start + 5] = asked;

        payload[start + HEADER_LEN..start + HEADER_LEN + data.len()].copy_from_slice(data);

        let header = MavHeader {
            system_id: 1,
            component_id: 1,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.wrapping_add(1);

        let _ = self
            .socket
            .send_to(&encode_v2_bytes(MESSAGE_ID, &payload, header), peer)
            .await;
    }
}

/// Opens a session to the vehicle.
async fn session_to(address: SocketAddr) -> Session {
    let spec = mpcraft_transport::ConnectionSpec::parse(&format!("udp:{address}")).unwrap();

    Session::connect(&spec, SessionOptions::default())
        .await
        .unwrap()
}

/// Waits until the handshake has chosen the vehicle, because that is the pair an FTP request is addressed to.
async fn wait_until_selected(session: &Session, vehicle: VehicleId) {
    for _ in 0..100 {
        if session.state().selected == Some(vehicle) {
            return;
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    panic!("the handshake did not choose the vehicle");
}

/// The file the vehicle generates when asked: the number it says is not the length it gives.
fn generated_file() -> File {
    let mut content = b"MemInfoV1\nSTART=0x20000000 LEN=192k".to_vec();
    content.resize(122, b'0');

    File {
        path: "/@SYS/memory.txt".to_owned(),
        announced: 100_000,
        content,
    }
}

#[tokio::test]
async fn the_reader_stops_where_the_vehicle_says_the_file_ends() {
    let address = Vehicle::start(vec![generated_file()], vec![root_folder()]).await;
    let session = Arc::new(session_to(address).await);

    let vehicle = session
        .wait_for_any_vehicle(Duration::from_secs(5))
        .await
        .expect("the vehicle answers its heartbeat");

    assert_eq!(vehicle, VehicleId::new(1, 1));

    wait_until_selected(&session, vehicle).await;

    let client = FtpClient::new(session.clone());

    let file = client.read_whole("/@SYS/memory.txt", 230).await.unwrap();

    // What came back is what the file has, and the number the vehicle said for it is kept apart from the bytes
    // rather than overwritten — which is what lets the interface say that the two are not the same thing.
    assert_eq!(
        file.content.len(),
        122,
        "what the vehicle gave is what the file has"
    );
    assert_eq!(
        file.announced_size, 100_000,
        "the number the vehicle said is kept as it said it",
    );

    // The reading stopped because the vehicle named the end, not because a counter reached a number: without that
    // rule this read would have been reported as a refusal, which is what it was before.
    assert!(file.content.starts_with(b"MemInfoV1"));
}

#[tokio::test]
async fn a_refusal_is_not_read_as_the_end_of_a_file() {
    let address = Vehicle::start(vec![generated_file()], vec![root_folder()]).await;
    let session = Arc::new(session_to(address).await);

    let vehicle = session
        .wait_for_any_vehicle(Duration::from_secs(5))
        .await
        .expect("the vehicle answers its heartbeat");

    wait_until_selected(&session, vehicle).await;

    let client = FtpClient::new(session.clone());

    let error = client.read_whole("/@SYS/nothing.txt", 230).await;

    let refusal = match error {
        Err(LinkError::FtpRefused(refusal)) => refusal,
        other => panic!("the vehicle answered with something other than a refusal: {other:?}"),
    };

    assert_eq!(refusal.code, Some(FtpError::FileNotFound));
    assert!(
        !says_the_end(&refusal),
        "a refusal is not the end of what was asked for"
    );
}

#[tokio::test]
async fn a_listing_ends_with_the_code_that_names_the_end() {
    let address = Vehicle::start(vec![], vec![root_folder()]).await;
    let session = Arc::new(session_to(address).await);

    let vehicle = session
        .wait_for_any_vehicle(Duration::from_secs(5))
        .await
        .expect("the vehicle answers its heartbeat");

    wait_until_selected(&session, vehicle).await;

    let client = FtpClient::new(session.clone());

    // The first ask is answered with the generic failure this build gives an opcode it does not know, the second
    // with the entries, and the third — the ask after the last entry — with the number that says there is nothing
    // more. Read as a failure, that third answer would have thrown away a folder the vehicle had listed.
    let entries = client.list_directory("/").await.unwrap();

    assert_eq!(entries.len(), 2, "the folder gave what it holds");

    // The letter says what an entry is; it is not part of the name. Counting the zero the writer adds for itself as
    // an entry would have put the letter inside the names here, and would have made the number the next ask carries
    // wrong, because the vehicle counts entries rather than separators.
    assert_eq!(entries[0].name, "@SYS");
    assert_eq!(entries[1].name, "APM");

    assert!(
        entries.iter().all(|entry| entry.is_directory),
        "this card holds folders"
    );

    for entry in &entries {
        assert_eq!(entry.path, format!("/{}", entry.name));
    }
}

/// The folder at the root of the card, as the board on the bench answered it.
fn root_folder() -> Folder {
    Folder {
        path: "/".to_owned(),
        entries: vec![(b'D', "@SYS".to_owned()), (b'D', "APM".to_owned())],
    }
}

#[tokio::test]
async fn when_the_number_and_the_bytes_agree_they_are_one_fact() {
    let file = File {
        path: "/notes.txt".to_owned(),
        announced: 30,
        content: b"thirty bytes of a real file\n\n\n".to_vec(),
    };

    let address = Vehicle::start(vec![file], vec![]).await;
    let session = Arc::new(session_to(address).await);

    let vehicle = session
        .wait_for_any_vehicle(Duration::from_secs(5))
        .await
        .expect("the vehicle answers its heartbeat");

    wait_until_selected(&session, vehicle).await;

    let client = FtpClient::new(session.clone());

    let read = client.read_whole("/notes.txt", 230).await.unwrap();

    assert_eq!(read.announced_size, 30);
    assert_eq!(
        read.content.len(),
        30,
        "here the number and the bytes are the same thing"
    );
}
