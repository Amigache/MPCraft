//! The link copied to another program.
//!
//! A second ground station, a log recorder or a script can follow the flight without a link of
//! its own: every chunk of bytes the station reads from the vehicle is copied to each mirror.
//! When a mirror is told to write, what it receives is put back on the link, which is how a
//! program on the same cable can talk to the board.
//!
//! Upstream keeps the same list in `Controls/SerialOutputPass.cs`: a serial port, a TCP
//! connection or a UDP socket, opened here (outbound) or waited for (inbound). The copy is made
//! at the byte level, before a frame is parsed, because a mirror carries the stream rather than a
//! re-encoding of it.
//!
//! A mirror is best effort by design: a program that stops reading must not stop the link. The
//! channel to a mirror is bounded, and a mirror that falls behind loses chunks rather than
//! growing without a bound; a failure is kept in the status the sheet shows.

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpcraft_link::Session;
use mpcraft_settings::{MirrorDirection, MirrorProtocol, MirrorSpec};
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc;

use crate::error::CommandError;

/// How long a mirror waits before trying an endpoint again after a failure.
const RETRY: Duration = Duration::from_millis(500);

/// How long a mirror waits for the other program before it looks at the link again.
const RECEIVE_SLICE: Duration = Duration::from_millis(20);

/// Why a mirror could not be started or stopped.
#[derive(Debug, thiserror::Error)]
pub enum MirrorError {
    /// The port is not one the protocol can use.
    #[error("the port is not one that can be used: {0}")]
    BadPort(String),
    /// The baud rate of a serial mirror is not a number.
    #[error("the baud rate is not one that can be used: {0}")]
    BadBaud(String),
    /// The host of an outbound mirror is empty.
    #[error("an outbound mirror needs a host")]
    NoHost,
    /// The mirror is already running.
    #[error("the mirror {0} is already running")]
    AlreadyRunning(String),
    /// The endpoint could not be opened.
    #[error("the mirror could not be opened: {0}")]
    Open(String),
    /// The state of the service cannot be reached.
    #[error("the state of the mirrors cannot be reached")]
    Locked,
}

impl MirrorError {
    /// The stable code the frontend branches on.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadPort(_) | Self::BadBaud(_) | Self::NoHost => {
                CommandError::CODE_INVALID_REQUEST
            }
            Self::AlreadyRunning(_) | Self::Open(_) | Self::Locked => CommandError::CODE_INTERNAL,
        }
    }
}

/// One mirror, as the sheet shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorStatus {
    /// The name the row is known by.
    pub key: String,
    /// What the row says.
    pub spec: MirrorSpec,
    /// Whether its task is running.
    pub running: bool,
    /// How many bytes were copied to it.
    pub sent: u64,
    /// How many bytes it sent back to the vehicle.
    pub received: u64,
    /// The last failure, when there was one.
    pub error: Option<String>,
}

/// What is running, and the rows the sheet saved.
#[derive(Default)]
struct Inner {
    /// The rows, in the order the operator wrote them.
    specs: Vec<MirrorSpec>,
    /// The tasks of the rows that are running.
    active: HashMap<String, Active>,
}

/// A running mirror.
struct Active {
    /// The task.
    task: tokio::task::JoinHandle<()>,
    /// The bytes copied to the mirror.
    sent: Arc<AtomicU64>,
    /// The bytes the mirror sent back to the vehicle.
    received: Arc<AtomicU64>,
    /// The last failure the task saw.
    error: Arc<Mutex<Option<String>>>,
}

/// The mirrors and their tasks.
#[derive(Clone, Default)]
pub struct MirrorService {
    inner: Arc<Mutex<Inner>>,
}

impl MirrorService {
    /// Replaces the rows the sheet keeps and stops the ones that are no longer there.
    ///
    /// A row that is still in the list keeps running: saving the list must not stop a mirror
    /// that the operator did not touch.
    pub fn set_specs(&self, specs: Vec<MirrorSpec>) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };

        let keys: std::collections::HashSet<String> = specs.iter().map(MirrorSpec::key).collect();

        inner.active.retain(|key, active| {
            if keys.contains(key) {
                true
            } else {
                active.task.abort();
                false
            }
        });

        inner.specs = specs;
    }

    /// The rows and what they are doing.
    #[must_use]
    pub fn status(&self) -> Vec<MirrorStatus> {
        let Ok(mut inner) = self.inner.lock() else {
            return Vec::new();
        };

        inner.active.retain(|_, active| !active.task.is_finished());

        inner
            .specs
            .iter()
            .map(|spec| {
                let key = spec.key();
                let active = inner.active.get(&key);

                MirrorStatus {
                    key: key.clone(),
                    spec: spec.clone(),
                    running: active.is_some(),
                    sent: active.map_or(0, |active| active.sent.load(Ordering::Relaxed)),
                    received: active.map_or(0, |active| active.received.load(Ordering::Relaxed)),
                    error: active.and_then(|active| {
                        active.error.lock().ok().and_then(|error| error.clone())
                    }),
                }
            })
            .collect()
    }

    /// The row with this name, when it is one of the saved rows.
    #[must_use]
    pub fn find(&self, key: &str) -> Option<MirrorSpec> {
        let inner = self.inner.lock().ok()?;

        inner.specs.iter().find(|spec| spec.key() == key).cloned()
    }

    /// Starts one row.
    ///
    /// # Errors
    ///
    /// Returns an error when the row is not one that can be used, when it is already running, or
    /// when the endpoint cannot be opened.
    pub async fn start(&self, session: Arc<Session>, spec: &MirrorSpec) -> Result<(), MirrorError> {
        check(spec)?;

        let key = spec.key();

        {
            let inner = self.inner.lock().map_err(|_| MirrorError::Locked)?;

            if inner.active.contains_key(&key) {
                return Err(MirrorError::AlreadyRunning(key));
            }
        }

        let receiver = session
            .mirror_bytes()
            .await
            .map_err(|error| MirrorError::Open(error.to_string()))?;

        let endpoint = Endpoint::open(spec).await?;

        let sent = Arc::new(AtomicU64::new(0));
        let received = Arc::new(AtomicU64::new(0));
        let error = Arc::new(Mutex::new(None));

        let task = tokio::spawn(run(
            endpoint,
            receiver,
            spec.write,
            Arc::clone(&session),
            Arc::clone(&sent),
            Arc::clone(&received),
            Arc::clone(&error),
        ));

        let abort = task.abort_handle();

        let mut inner = self.inner.lock().map_err(|_| MirrorError::Locked)?;

        inner.active.retain(|_, active| !active.task.is_finished());

        if inner.active.contains_key(&key) {
            abort.abort();

            return Err(MirrorError::AlreadyRunning(key));
        }

        inner.active.insert(
            key,
            Active {
                task,
                sent,
                received,
                error,
            },
        );

        Ok(())
    }

    /// Stops one row, if it is running.
    pub fn stop(&self, key: &str) {
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(active) = inner.active.remove(key) {
                active.task.abort();
            }
        }
    }

    /// Stops everything.
    pub fn stop_all(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            for (_, active) in inner.active.drain() {
                active.task.abort();
            }
        }
    }
}

/// Whether a row is one a mirror can be opened with.
///
/// # Errors
///
/// Returns the first thing that cannot be used, named as it is in the row.
pub fn check(spec: &MirrorSpec) -> Result<(), MirrorError> {
    match spec.protocol {
        MirrorProtocol::Serial => {
            if spec.port.trim().is_empty() {
                return Err(MirrorError::BadPort(spec.port.clone()));
            }

            if spec
                .extra
                .trim()
                .parse::<u32>()
                .map_or(true, |baud| baud == 0)
            {
                return Err(MirrorError::BadBaud(spec.extra.clone()));
            }
        }
        MirrorProtocol::Tcp | MirrorProtocol::Udp => {
            let port = spec.port.trim();

            if port.parse::<u16>().map_or(true, |port| port == 0) {
                return Err(MirrorError::BadPort(spec.port.clone()));
            }

            if spec.direction == MirrorDirection::Outbound && spec.extra.trim().is_empty() {
                return Err(MirrorError::NoHost);
            }
        }
    }

    Ok(())
}

/// What a mirror writes to and reads from.
enum Endpoint {
    /// A serial port of this computer.
    Serial(Box<dyn mpcraft_transport::Transport>),
    /// A connection this station opens, and opens again when it drops.
    TcpClient {
        /// The address as the row writes it.
        address: String,
        /// The connection, when it is open.
        stream: Option<TcpStream>,
    },
    /// A connection this station waits for.
    TcpServer {
        /// The listener.
        listener: TcpListener,
        /// The program that is connected, when one is.
        client: Option<TcpStream>,
    },
    /// A datagram socket this station sends from.
    UdpClient {
        /// The socket, connected to the peer.
        socket: UdpSocket,
    },
    /// A datagram socket this station waits on.
    UdpServer {
        /// The socket.
        socket: UdpSocket,
        /// Where the last datagram came from, and where the next one goes.
        peer: Option<std::net::SocketAddr>,
    },
}

impl Endpoint {
    /// Opens the endpoint a row names.
    async fn open(spec: &MirrorSpec) -> Result<Self, MirrorError> {
        let port = spec.port.trim();
        let extra = spec.extra.trim();

        match spec.protocol {
            MirrorProtocol::Serial => {
                let target = format!("serial:{port}:{extra}");
                let connection = mpcraft_transport::ConnectionSpec::parse(&target)
                    .map_err(|error| MirrorError::BadPort(error.to_string()))?;
                let transport = mpcraft_transport::open(&connection)
                    .await
                    .map_err(|error| MirrorError::Open(error.to_string()))?;

                Ok(Self::Serial(transport))
            }
            MirrorProtocol::Tcp => match spec.direction {
                MirrorDirection::Outbound => {
                    let address = format!("{extra}:{port}");
                    let stream = TcpStream::connect(&address)
                        .await
                        .map_err(|error| MirrorError::Open(error.to_string()))?;
                    let _ = stream.set_nodelay(true);

                    Ok(Self::TcpClient {
                        address,
                        stream: Some(stream),
                    })
                }
                MirrorDirection::Inbound => {
                    let listener = TcpListener::bind(("0.0.0.0", parse_port(port)?))
                        .await
                        .map_err(|error| MirrorError::Open(error.to_string()))?;

                    Ok(Self::TcpServer {
                        listener,
                        client: None,
                    })
                }
            },
            MirrorProtocol::Udp => match spec.direction {
                MirrorDirection::Outbound => {
                    let peer = tokio::net::lookup_host((extra, parse_port(port)?))
                        .await
                        .map_err(|error| MirrorError::Open(error.to_string()))?
                        .next()
                        .ok_or_else(|| MirrorError::Open("the host names no address".to_owned()))?;

                    let socket = UdpSocket::bind("0.0.0.0:0")
                        .await
                        .map_err(|error| MirrorError::Open(error.to_string()))?;
                    socket
                        .connect(peer)
                        .await
                        .map_err(|error| MirrorError::Open(error.to_string()))?;

                    Ok(Self::UdpClient { socket })
                }
                MirrorDirection::Inbound => {
                    let socket = UdpSocket::bind(("0.0.0.0", parse_port(port)?))
                        .await
                        .map_err(|error| MirrorError::Open(error.to_string()))?;

                    Ok(Self::UdpServer { socket, peer: None })
                }
            },
        }
    }

    /// Copies bytes to the other program.
    async fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        match self {
            Self::Serial(transport) => transport.write(bytes).await,
            Self::TcpClient { address, stream } => {
                if stream.is_none() {
                    let opened = TcpStream::connect(address.as_str()).await?;
                    let _ = opened.set_nodelay(true);
                    *stream = Some(opened);
                }

                match stream.as_mut() {
                    Some(stream) => stream.write_all(bytes).await,
                    None => Ok(()),
                }
            }
            Self::TcpServer { client, .. } => match client.as_mut() {
                Some(client) => client.write_all(bytes).await,
                None => Ok(()),
            },
            Self::UdpClient { socket } => {
                socket.send(bytes).await?;

                Ok(())
            }
            Self::UdpServer { socket, peer } => match peer {
                Some(peer) => {
                    socket.send_to(bytes, *peer).await?;

                    Ok(())
                }
                None => Ok(()),
            },
        }
    }

    /// Reads what the other program sends.
    async fn receive(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Serial(transport) => transport.read(buffer).await,
            Self::TcpClient { stream, .. } => {
                let count = match stream.as_mut() {
                    Some(open) => open.read(buffer).await?,
                    None => return Ok(0),
                };

                if count == 0 {
                    *stream = None;
                }

                Ok(count)
            }
            Self::TcpServer { listener, client } => {
                if client.is_none() {
                    let (stream, _) = listener.accept().await?;
                    let _ = stream.set_nodelay(true);
                    *client = Some(stream);
                }

                match client.as_mut() {
                    Some(open) => {
                        let count = open.read(buffer).await?;

                        if count == 0 {
                            *client = None;
                        }

                        Ok(count)
                    }
                    None => Ok(0),
                }
            }
            Self::UdpClient { socket } => socket.recv(buffer).await,
            Self::UdpServer { socket, peer } => {
                let (count, from) = socket.recv_from(buffer).await?;
                *peer = Some(from);

                Ok(count)
            }
        }
    }
}

/// Copies the link to one endpoint, and what the endpoint sends back to the link.
async fn run(
    mut endpoint: Endpoint,
    mut sink: mpsc::Receiver<Vec<u8>>,
    write: bool,
    session: Arc<Session>,
    sent: Arc<AtomicU64>,
    received: Arc<AtomicU64>,
    error: Arc<Mutex<Option<String>>>,
) {
    let mut buffer = vec![0_u8; 8192];

    loop {
        match sink.try_recv() {
            Ok(chunk) => {
                match endpoint.send(&chunk).await {
                    Ok(()) => {
                        sent.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                        keep(&error, None);
                    }
                    Err(failure) => {
                        keep(&error, Some(failure.to_string()));
                        tokio::time::sleep(RETRY).await;
                    }
                }

                continue;
            }
            Err(mpsc::error::TryRecvError::Disconnected) => return,
            Err(mpsc::error::TryRecvError::Empty) => {}
        }

        match tokio::time::timeout(RECEIVE_SLICE, endpoint.receive(&mut buffer)).await {
            Ok(Ok(0)) => {
                tokio::time::sleep(RETRY).await;
            }
            Ok(Ok(count)) => {
                if write {
                    if session.write(buffer[..count].to_vec()).await.is_err() {
                        return;
                    }

                    received.fetch_add(count as u64, Ordering::Relaxed);
                }
            }
            Ok(Err(failure)) => {
                keep(&error, Some(failure.to_string()));
                tokio::time::sleep(RETRY).await;
            }
            Err(_) => {}
        }
    }
}

/// Keeps the last failure, or clears it when the mirror is working again.
fn keep(error: &Mutex<Option<String>>, failure: Option<String>) {
    if let Ok(mut held) = error.lock() {
        *held = failure;
    }
}

/// Reads the port number a row writes.
fn parse_port(port: &str) -> Result<u16, MirrorError> {
    port.parse::<u16>()
        .map_err(|_| MirrorError::BadPort(port.to_owned()))
}
