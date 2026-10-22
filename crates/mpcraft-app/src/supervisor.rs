//! The session supervisor: the link lifecycle and the reconnect policy.
//!
//! The supervisor owns one link. It opens it, publishes its state to the interface, and —
//! when the link drops — decides whether the link is restored. The *mechanism* of
//! restoring a link lives in the transport; the *policy* lives here, where it can be
//! tested without a network peer.
//!
//! A restored link is a **new session on the same target**, not a `reconnect()` on the
//! old transport: the session owns the transport, and by the time the supervisor notices
//! that the link has dropped the transport is gone with it. Opening the target again is
//! what the transport's own `reconnect` does, one layer lower and without the policy.
//!
//! Three things are injected into the supervisor, and each of them exists so that the
//! policy can be tested without waiting:
//!
//! * The [`Connector`] opens the sessions. A test supplies one that fails on demand.
//! * The [`Sleeper`] provides the waits between two attempts. A test supplies one that
//!   records the delays and returns at once.
//! * The [`ReconnectPolicy`] says how many attempts there are, how long the waits are and
//!   how long a single attempt may take.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use mpcraft_link::{LinkError, Session, SessionOptions, SessionState, SigningSetup};
use mpcraft_transport::ConnectionSpec;
use serde::{Deserialize, Serialize};
use tokio::sync::{watch, Notify};

use crate::error::{CommandError, CommandResult};
use crate::ConnectResponse;

/// How often the supervisor publishes the state to the interface.
const PUBLISH_INTERVAL: Duration = Duration::from_millis(200);

/// How long the supervisor waits for the publisher to notice that it was asked to stop.
///
/// The wait is bounded on purpose: a publisher that is stuck in a connection attempt must
/// not hold the caller of `disconnect` hostage.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);

/// The connection state of the application.
///
/// The states describe the *link*, not the vehicle: a link can be open while the vehicle
/// is still silent on it. The vehicles that are on the link travel with the snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    /// No link is open.
    #[default]
    Disconnected,
    /// A link is being opened.
    Connecting,
    /// The link is open.
    Connected,
    /// The link dropped and the supervisor is restoring it.
    Reconnecting,
    /// The link could not be opened, or the supervisor gave up restoring it.
    Failed,
}

/// What the supervisor publishes to the interface.
#[derive(Debug, Clone)]
pub struct LinkSnapshot {
    /// The state of the link.
    pub status: ConnectionStatus,
    /// The state of the session, when a link is open.
    pub session: Option<SessionState>,
}

impl LinkSnapshot {
    /// A snapshot of a link that is not open.
    #[must_use]
    pub fn disconnected() -> Self {
        Self {
            status: ConnectionStatus::Disconnected,
            session: None,
        }
    }
}

/// What the supervisor does when the link drops.
///
/// The policy is data, not code: the supervisor reads it, and a test can state it exactly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReconnectPolicy {
    /// How many times the supervisor tries to restore a dropped link.
    ///
    /// Zero means that a dropped link stays dropped. The link that was opened when the
    /// operator asked for it is never re-opened behind their back: only a link that *was*
    /// open is restored.
    ///
    /// The attempts are counted over one unstable period rather than over one drop. A
    /// link that stays up for longer than [`ReconnectPolicy::max_backoff`] has proved
    /// itself, and the next drop starts a new period with a fresh budget.
    pub attempts: u32,

    /// The wait before the first attempt.
    pub initial_backoff: Duration,

    /// The factor the wait grows by after every failed attempt.
    ///
    /// A wait that does not grow turns a long outage into a hammering of the transport.
    pub growth: f64,

    /// The longest the supervisor waits between two attempts.
    pub max_backoff: Duration,

    /// The longest a single attempt may take.
    ///
    /// A transport that cannot be opened — a serial port that is not there, a host that
    /// does not answer — must not hold the caller hostage. The attempt is abandoned and
    /// the supervisor moves on to the next one.
    pub attempt_timeout: Duration,

    /// Whether giving up is reported to the interface as `failed`.
    ///
    /// An operator watching a vehicle would rather be told that the link failed than see
    /// it quietly go back to "disconnected": the two mean very different things when the
    /// vehicle is a thousand kilometres away.
    pub surface_failure: bool,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            attempts: 5,
            initial_backoff: Duration::from_millis(500),
            growth: 2.0,
            max_backoff: Duration::from_secs(10),
            attempt_timeout: Duration::from_secs(5),
            surface_failure: true,
        }
    }
}

impl ReconnectPolicy {
    /// A policy that never restores a dropped link.
    #[must_use]
    pub const fn never() -> Self {
        Self {
            attempts: 0,
            initial_backoff: Duration::ZERO,
            growth: 1.0,
            max_backoff: Duration::ZERO,
            attempt_timeout: Duration::from_secs(5),
            surface_failure: true,
        }
    }

    /// The wait before the given attempt.
    ///
    /// The attempt number starts at one. The wait grows geometrically and is capped by
    /// [`ReconnectPolicy::max_backoff`], so a long outage does not push the next attempt
    /// hours into the future.
    #[must_use]
    pub fn backoff(&self, attempt: u32) -> Duration {
        let factor = if attempt <= 1 {
            1.0
        } else {
            self.growth
                .powi(i32::try_from(attempt - 1).unwrap_or(i32::MAX))
        };

        // A policy with a silly growth factor must not produce a nonsense wait.
        if !factor.is_finite() || factor <= 0.0 {
            return self.initial_backoff.min(self.max_backoff);
        }

        let seconds =
            (self.initial_backoff.as_secs_f64() * factor).min(self.max_backoff.as_secs_f64());

        if !seconds.is_finite() || seconds <= 0.0 {
            return Duration::ZERO;
        }

        Duration::from_secs_f64(seconds)
    }
}

/// The source of the waits the supervisor makes.
///
/// The waits are injected so that the reconnect policy can be tested without the test
/// having to wait: a test supplies a sleeper that records the delays and returns at once.
#[async_trait]
pub trait Sleeper: Send + Sync {
    /// Waits for the given duration.
    async fn sleep(&self, duration: Duration);
}

/// A sleeper that waits on the Tokio timer.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioSleeper;

#[async_trait]
impl Sleeper for TokioSleeper {
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// Opens the sessions the supervisor drives.
#[async_trait]
pub trait Connector: Send + Sync {
    /// Opens a session on the target.
    ///
    /// # Errors
    ///
    /// Returns the reason the link could not be opened.
    async fn connect(
        &self,
        spec: &ConnectionSpec,
        options: SessionOptions,
    ) -> Result<Session, LinkError>;
}

/// The connector that opens real transports.
#[derive(Debug, Clone, Copy, Default)]
pub struct TransportConnector;

#[async_trait]
impl Connector for TransportConnector {
    async fn connect(
        &self,
        spec: &ConnectionSpec,
        options: SessionOptions,
    ) -> Result<Session, LinkError> {
        Session::connect(spec, options).await
    }
}

/// The open link and the means to stop it.
struct SessionHandle {
    /// The session that is open. It is replaced when the link is restored.
    session: Arc<Session>,
    /// Tells the publisher that it should stop.
    stop: watch::Sender<bool>,
    /// Signals that the publisher has published its last state.
    done: Arc<Notify>,
}

/// The state the supervisor shares with the task that publishes it.
struct Inner {
    /// The keys the link is signed with.
    signing: std::sync::Mutex<Option<SigningSetup>>,
    /// The open link, if there is one.
    handle: Mutex<Option<SessionHandle>>,
    /// The state of the link, as the interface sees it.
    status: Mutex<ConnectionStatus>,
    /// Counts the links the supervisor has opened.
    ///
    /// A task that is restoring a link that has since been replaced must not install its
    /// own session: the number tells it that the link it was working on is not the link
    /// any more.
    generation: AtomicU64,
    /// What to do when the link drops.
    policy: ReconnectPolicy,
    /// Opens the sessions.
    connector: Arc<dyn Connector>,
    /// Provides the waits between two attempts.
    sleeper: Arc<dyn Sleeper>,
}

impl Inner {
    /// The state of the link.
    fn status(&self) -> ConnectionStatus {
        self.status
            .lock()
            .map(|status| *status)
            .unwrap_or(ConnectionStatus::Disconnected)
    }

    /// Records the state of the link.
    fn set_status(&self, status: ConnectionStatus) {
        if let Ok(mut slot) = self.status.lock() {
            *slot = status;
        }
    }

    /// The number of the link that is open.
    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
}

/// Owns the link to the vehicle and publishes its state.
///
/// The supervisor is deliberately free of any interface type: it calls a callback with the
/// state, and the shell decides how to deliver it. That keeps this crate headless.
#[derive(Clone)]
pub struct SessionSupervisor {
    inner: Arc<Inner>,
}

impl Default for SessionSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionSupervisor {
    /// Creates a supervisor with no link open.
    #[must_use]
    pub fn new() -> Self {
        Self::with_policy(ReconnectPolicy::default())
    }

    /// Creates a supervisor that follows the given policy.
    #[must_use]
    pub fn with_policy(policy: ReconnectPolicy) -> Self {
        Self::with_parts(policy, Arc::new(TransportConnector), Arc::new(TokioSleeper))
    }

    /// Creates a supervisor whose every part is supplied by the caller.
    ///
    /// This is the seam the tests use: a connector that fails on demand and a sleeper that
    /// records the delays turn the reconnect policy into something that can be asserted on
    /// without a network peer and without waiting.
    #[must_use]
    pub fn with_parts(
        policy: ReconnectPolicy,
        connector: Arc<dyn Connector>,
        sleeper: Arc<dyn Sleeper>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                handle: Mutex::new(None),
                signing: Mutex::new(None),
                status: Mutex::new(ConnectionStatus::Disconnected),
                generation: AtomicU64::new(0),
                policy,
                connector,
                sleeper,
            }),
        }
    }

    /// The policy the supervisor follows.
    #[must_use]
    pub fn policy(&self) -> ReconnectPolicy {
        self.inner.policy
    }

    /// The state of the link.
    #[must_use]
    pub fn status(&self) -> ConnectionStatus {
        self.inner.status()
    }

    /// Opens a link to a vehicle.
    ///
    /// The state is published through `publish` every [`PUBLISH_INTERVAL`], so the
    /// interface sees the link come up, the telemetry arrive, and the link drop and come
    /// back.
    ///
    /// A link that cannot be opened is an error, and it is returned promptly: the attempt
    /// is bounded by [`ReconnectPolicy::attempt_timeout`], and the supervisor does not
    /// retry behind the caller's back. What happens to a link that drops *later* is the
    /// policy's business, and the policy runs in the background.
    ///
    /// # Errors
    ///
    /// Returns an error when the target cannot be parsed or the link cannot be opened.
    /// The keys the link is signed with, when it is signed.
    ///
    /// The keys are set before the link is opened, and they stay until they are changed:
    /// a link that is opened again is signed the same way.
    pub fn set_signing(&self, signing: Option<SigningSetup>) {
        if let Ok(mut slot) = self.inner.signing.lock() {
            *slot = signing;
        }
    }

    /// The keys the link is signed with.
    #[must_use]
    pub fn signing(&self) -> Option<SigningSetup> {
        self.inner.signing.lock().ok().and_then(|slot| slot.clone())
    }

    /// Opens a link to a vehicle.
    ///
    /// The keys of the link are taken from the state of the supervisor, so a link that
    /// is opened again is signed the same way.
    ///
    /// # Errors
    ///
    /// Returns an error when the target cannot be parsed or the link cannot be opened.
    pub async fn connect<F>(&self, target: &str, mut publish: F) -> CommandResult<ConnectResponse>
    where
        F: FnMut(&LinkSnapshot) + Send + 'static,
    {
        let target = target.trim();

        if target.is_empty() {
            return Err(CommandError::invalid_request(
                "the connection target is empty",
            ));
        }

        let spec = ConnectionSpec::parse(target)
            .map_err(|error| CommandError::invalid_request(error.to_string()))?;

        // The supervisor drives one link: a new one replaces the old.
        self.stop_current().await;

        self.inner.set_status(ConnectionStatus::Connecting);
        publish(&LinkSnapshot {
            status: ConnectionStatus::Connecting,
            session: None,
        });

        let session = match open_with_rate(&self.inner, &spec).await {
            Ok(session) => session,
            Err(error) => {
                self.inner.set_status(ConnectionStatus::Failed);
                publish(&LinkSnapshot {
                    status: ConnectionStatus::Failed,
                    session: None,
                });

                return Err(CommandError::from(error));
            }
        };

        let session = Arc::new(session);
        let (stop, stop_receiver) = watch::channel(false);
        let done = Arc::new(Notify::new());
        let generation = self.inner.generation.fetch_add(1, Ordering::Relaxed) + 1;

        if let Ok(mut slot) = self.inner.handle.lock() {
            *slot = Some(SessionHandle {
                session: Arc::clone(&session),
                stop,
                done: Arc::clone(&done),
            });
        }

        self.inner.set_status(ConnectionStatus::Connected);

        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            let mut publish = publish;
            let mut stop = stop_receiver;

            supervise(&inner, session, generation, &mut stop, &spec, &mut publish).await;

            // The publisher has published its last state: whoever asked for the link to be
            // closed may go on.
            done.notify_one();
        });

        Ok(ConnectResponse {
            status: ConnectionStatus::Connected,
            vehicle: None,
        })
    }

    /// Closes the link.
    pub async fn disconnect(&self) -> CommandResult<ConnectResponse> {
        self.stop_current().await;

        Ok(ConnectResponse {
            status: ConnectionStatus::Disconnected,
            vehicle: None,
        })
    }

    /// The state of the current session, if there is one.
    #[must_use]
    pub fn state(&self) -> Option<SessionState> {
        self.inner
            .handle
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(|handle| handle.session.state()))
    }

    /// The session that is open, if there is one.
    ///
    /// The operations in [`crate::operations`] need the session itself, and this is how
    /// they reach it without the shell ever touching the link layer.
    #[must_use]
    pub fn session(&self) -> Option<Arc<Session>> {
        self.inner
            .handle
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(|handle| Arc::clone(&handle.session)))
    }

    /// Closes the link that is open, if there is one.
    async fn stop_current(&self) {
        // The handle is taken out under the lock and awaited outside it: a guard must never
        // be held across an await point.
        let handle = self
            .inner
            .handle
            .lock()
            .ok()
            .and_then(|mut slot| slot.take());

        let Some(handle) = handle else {
            return;
        };

        // The publisher must not outlive the disconnect: a "connected" state published
        // after the operator asked to disconnect would be a lie.
        let _ = handle.stop.send(true);
        handle.session.stop().await;

        // The publisher publishes the last state itself, so that the interface is told
        // that the link is gone by the code that knows it. The wait is bounded, because a
        // publisher that is stuck in a connection attempt must not hold the caller.
        let _ = tokio::time::timeout(SHUTDOWN_TIMEOUT, handle.done.notified()).await;
    }
}

/// Opens a session, within the bounds of the policy.
async fn open(inner: &Inner, spec: &ConnectionSpec) -> Result<Session, LinkError> {
    // The keys of the link, when the link is signed.
    let signing = inner.signing.lock().ok().and_then(|slot| slot.clone());

    let options = SessionOptions {
        signing,
        ..SessionOptions::default()
    };

    match tokio::time::timeout(
        inner.policy.attempt_timeout,
        inner.connector.connect(spec, options),
    )
    .await
    {
        Ok(result) => result,
        Err(_elapsed) => Err(LinkError::Timeout(inner.policy.attempt_timeout)),
    }
}

/// Opens the link, finding the rate when the target leaves it to the program.
///
/// A vehicle that is talking at a rate nobody wrote down is found by trying them one
/// after another: the port is opened at each rate in turn, and the rate at which the
/// vehicle answers is the one it talks at.
async fn open_with_rate(inner: &Inner, spec: &ConnectionSpec) -> Result<Session, LinkError> {
    if !spec.options.auto_baud {
        return open(inner, spec).await;
    }

    let mut last = None;

    for baud in AUTO_BAUD_RATES {
        let mut attempt = spec.clone();
        attempt.options.baud_rate = baud;
        attempt.options.auto_baud = false;

        let session = open(inner, &attempt).await?;

        // The vehicle has a moment to say that it is there. A port that is opened at
        // the wrong rate answers with nothing at all.
        match session
            .wait_for_any_vehicle(inner.policy.attempt_timeout)
            .await
        {
            Ok(_vehicle) => return Ok(session),
            Err(error) => {
                tracing::info!("nothing answered at {baud} bits per second");
                last = Some(error);
            }
        }
    }

    Err(last.unwrap_or(LinkError::Timeout(Duration::from_secs(1))))
}

/// The rates a serial port is tried at when the target leaves the rate to the program.
///
/// The rates are the ones vehicles talk at, and the ones Mission Planner offers.
const AUTO_BAUD_RATES: [u32; 9] = [
    57600, 115200, 230400, 460800, 921600, 38400, 19200, 9600, 14400,
];

/// Publishes the state of the link until it is closed, and publishes the last state.
async fn supervise<F>(
    inner: &Arc<Inner>,
    session: Arc<Session>,
    generation: u64,
    stop: &mut watch::Receiver<bool>,
    spec: &ConnectionSpec,
    publish: &mut F,
) where
    F: FnMut(&LinkSnapshot) + Send + 'static,
{
    let final_status = run(inner, session, generation, stop, spec, publish).await;

    // The last word belongs to the supervisor: the interface must never be left with a
    // "connected" state for a link that is gone.
    inner.set_status(final_status);
    publish(&LinkSnapshot {
        status: final_status,
        session: None,
    });
}

/// Drives the link until it is closed.
async fn run<F>(
    inner: &Arc<Inner>,
    session: Arc<Session>,
    generation: u64,
    stop: &mut watch::Receiver<bool>,
    spec: &ConnectionSpec,
    publish: &mut F,
) -> ConnectionStatus
where
    F: FnMut(&LinkSnapshot) + Send + 'static,
{
    let mut session = session;
    let mut attempts = 0u32;
    let mut linked_at = Instant::now();

    loop {
        // 1. Publish the state of the open link until it drops.
        loop {
            if *stop.borrow() {
                return ConnectionStatus::Disconnected;
            }

            let state = session.state();

            if !state.connected {
                break;
            }

            // A link that has been up for longer than the longest wait is a link that
            // works: the next drop starts a new period with a fresh budget of attempts.
            if linked_at.elapsed() >= inner.policy.max_backoff {
                attempts = 0;
            }

            inner.set_status(ConnectionStatus::Connected);
            publish(&LinkSnapshot {
                status: ConnectionStatus::Connected,
                session: Some(state),
            });

            tokio::select! {
                // The operator asked to stop: leave at once.
                _ = stop.changed() => return ConnectionStatus::Disconnected,
                _ = tokio::time::sleep(PUBLISH_INTERVAL) => {}
            }
        }

        if *stop.borrow() {
            return ConnectionStatus::Disconnected;
        }

        // 2. The link dropped. The policy decides whether it is restored.
        match restore(inner, spec, generation, stop, &mut attempts, publish).await {
            Restore::Session(restored) => {
                session = restored;
                linked_at = Instant::now();
            }
            Restore::Stopped => return ConnectionStatus::Disconnected,
            Restore::GaveUp => {
                return if inner.policy.surface_failure {
                    ConnectionStatus::Failed
                } else {
                    ConnectionStatus::Disconnected
                };
            }
        }
    }
}

/// What came of an attempt to restore the link.
enum Restore {
    /// The link was restored.
    Session(Arc<Session>),
    /// The operator asked to stop.
    Stopped,
    /// The policy gave up.
    GaveUp,
}

/// Restores a dropped link, following the policy.
async fn restore<F>(
    inner: &Arc<Inner>,
    spec: &ConnectionSpec,
    generation: u64,
    stop: &mut watch::Receiver<bool>,
    attempts: &mut u32,
    publish: &mut F,
) -> Restore
where
    F: FnMut(&LinkSnapshot) + Send + 'static,
{
    let policy = inner.policy;

    while *attempts < policy.attempts {
        if *stop.borrow() {
            return Restore::Stopped;
        }

        *attempts += 1;
        let attempt = *attempts;

        inner.set_status(ConnectionStatus::Reconnecting);
        publish(&LinkSnapshot {
            status: ConnectionStatus::Reconnecting,
            session: None,
        });

        // The wait between two attempts belongs to the policy, and the wait itself is
        // injected: a test asserts the sequence of delays without waiting for them.
        tokio::select! {
            _ = stop.changed() => return Restore::Stopped,
            _ = inner.sleeper.sleep(policy.backoff(attempt)) => {}
        }

        if *stop.borrow() {
            return Restore::Stopped;
        }

        match open(inner, spec).await {
            Ok(session) => {
                let session = Arc::new(session);

                if *stop.borrow() {
                    session.stop().await;
                    return Restore::Stopped;
                }

                // The link may have been replaced while the attempt was running: a
                // session that belongs to a link nobody is waiting for any more is
                // dropped rather than installed.
                if inner.generation() != generation {
                    session.stop().await;
                    return Restore::Stopped;
                }

                // The restored session takes the place of the dead one.
                if let Ok(mut slot) = inner.handle.lock() {
                    if let Some(handle) = slot.as_mut() {
                        handle.session = Arc::clone(&session);
                    }
                }

                inner.set_status(ConnectionStatus::Connected);

                return Restore::Session(session);
            }
            Err(error) => {
                tracing::warn!(
                    "the link could not be restored (attempt {attempt} of {}): {error}",
                    policy.attempts
                );
            }
        }
    }

    Restore::GaveUp
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use mpcraft_transport::{Transport, TransportKind};
    use tokio::sync::mpsc;

    use super::*;

    /// A transport whose peer is gone: every read reports the end of the file.
    struct DeadTransport;

    #[async_trait]
    impl Transport for DeadTransport {
        async fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Ok(0)
        }

        async fn write_some(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            Ok(buf.len())
        }

        fn kind(&self) -> TransportKind {
            TransportKind::Udp
        }

        fn is_open(&self) -> bool {
            false
        }

        async fn reconnect(&mut self) -> std::io::Result<()> {
            Ok(())
        }

        fn describe(&self) -> String {
            "a link whose peer is gone".to_owned()
        }
    }

    /// A sleeper that records the waits instead of making them.
    #[derive(Default)]
    struct RecordingSleeper {
        delays: Mutex<Vec<Duration>>,
    }

    impl RecordingSleeper {
        /// The waits the supervisor asked for, in order.
        fn delays(&self) -> Vec<Duration> {
            self.delays
                .lock()
                .map(|delays| delays.clone())
                .unwrap_or_default()
        }
    }

    #[async_trait]
    impl Sleeper for RecordingSleeper {
        async fn sleep(&self, duration: Duration) {
            if let Ok(mut delays) = self.delays.lock() {
                delays.push(duration);
            }
        }
    }

    /// A connector that opens a given number of sessions and then fails.
    struct ScriptedConnector {
        calls: AtomicUsize,
        sessions: usize,
    }

    impl ScriptedConnector {
        /// A connector that opens `sessions` sessions and fails after that.
        fn new(sessions: usize) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                sessions,
            }
        }

        /// How many times the supervisor asked for a session.
        fn calls(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl Connector for ScriptedConnector {
        async fn connect(
            &self,
            _spec: &ConnectionSpec,
            _options: SessionOptions,
        ) -> Result<Session, LinkError> {
            let call = self.calls.fetch_add(1, Ordering::Relaxed);

            if call < self.sessions {
                return Ok(Session::with_transport(
                    Box::new(DeadTransport),
                    SessionOptions::default(),
                ));
            }

            Err(LinkError::Transport("the peer is not there".to_owned()))
        }
    }

    /// Collects the published states until the wanted one is reached.
    async fn collect_until(
        receiver: &mut mpsc::UnboundedReceiver<ConnectionStatus>,
        wanted: ConnectionStatus,
    ) -> Vec<ConnectionStatus> {
        let collected = tokio::time::timeout(Duration::from_secs(10), async {
            let mut statuses = Vec::new();

            loop {
                let status = receiver
                    .recv()
                    .await
                    .expect("the supervisor should keep publishing");

                let reached = status == wanted;
                statuses.push(status);

                if reached {
                    return statuses;
                }
            }
        })
        .await;

        collected.expect("the supervisor should reach the wanted state")
    }

    /// A policy with three attempts and a backoff that doubles every time.
    fn policy() -> ReconnectPolicy {
        ReconnectPolicy {
            attempts: 3,
            initial_backoff: Duration::from_millis(100),
            growth: 2.0,
            max_backoff: Duration::from_secs(10),
            attempt_timeout: Duration::from_secs(1),
            surface_failure: true,
        }
    }

    #[test]
    fn the_backoff_grows_and_is_capped() {
        let policy = ReconnectPolicy {
            max_backoff: Duration::from_millis(500),
            ..policy()
        };

        assert_eq!(policy.backoff(1), Duration::from_millis(100));
        assert_eq!(policy.backoff(2), Duration::from_millis(200));
        assert_eq!(policy.backoff(3), Duration::from_millis(400));
        assert_eq!(policy.backoff(4), Duration::from_millis(500));
        assert_eq!(policy.backoff(9), Duration::from_millis(500));
    }

    #[test]
    fn a_policy_that_never_retries_has_no_backoff() {
        let policy = ReconnectPolicy::never();

        assert_eq!(policy.attempts, 0);
        assert_eq!(policy.backoff(1), Duration::ZERO);
    }

    #[tokio::test]
    async fn the_supervisor_asks_for_the_right_number_of_attempts_with_the_right_backoff() {
        let sleeper = Arc::new(RecordingSleeper::default());
        let connector = Arc::new(ScriptedConnector::new(1));

        let supervisor = SessionSupervisor::with_parts(
            policy(),
            Arc::clone(&connector) as Arc<dyn Connector>,
            Arc::clone(&sleeper) as Arc<dyn Sleeper>,
        );

        let (sender, mut receiver) = mpsc::unbounded_channel::<ConnectionStatus>();

        supervisor
            .connect("udp:127.0.0.1:14550", move |snapshot| {
                let _ = sender.send(snapshot.status);
            })
            .await
            .expect("the link should open");

        collect_until(&mut receiver, ConnectionStatus::Failed).await;

        // The waits between the attempts are the ones the policy asked for.
        assert_eq!(
            sleeper.delays(),
            vec![
                Duration::from_millis(100),
                Duration::from_millis(200),
                Duration::from_millis(400),
            ]
        );

        // One attempt to open the link, and three to restore it.
        assert_eq!(connector.calls(), 4);
    }

    #[tokio::test]
    async fn the_supervisor_reports_reconnecting_between_the_attempts_and_failed_when_it_gives_up()
    {
        let connector = Arc::new(ScriptedConnector::new(1));

        let supervisor = SessionSupervisor::with_parts(
            policy(),
            Arc::clone(&connector) as Arc<dyn Connector>,
            Arc::new(RecordingSleeper::default()),
        );

        let (sender, mut receiver) = mpsc::unbounded_channel::<ConnectionStatus>();

        supervisor
            .connect("udp:127.0.0.1:14550", move |snapshot| {
                let _ = sender.send(snapshot.status);
            })
            .await
            .expect("the link should open");

        let statuses = collect_until(&mut receiver, ConnectionStatus::Failed).await;

        // The interface is told that the link is being restored before every attempt.
        assert_eq!(
            statuses
                .iter()
                .filter(|status| **status == ConnectionStatus::Reconnecting)
                .count(),
            3
        );

        // The last word is the failure.
        assert_eq!(statuses.last(), Some(&ConnectionStatus::Failed));
        assert_eq!(supervisor.status(), ConnectionStatus::Failed);
    }

    #[tokio::test]
    async fn a_failure_that_is_not_surfaced_ends_in_disconnected() {
        let connector = Arc::new(ScriptedConnector::new(1));

        let supervisor = SessionSupervisor::with_parts(
            ReconnectPolicy {
                surface_failure: false,
                ..policy()
            },
            Arc::clone(&connector) as Arc<dyn Connector>,
            Arc::new(RecordingSleeper::default()),
        );

        let (sender, mut receiver) = mpsc::unbounded_channel::<ConnectionStatus>();

        supervisor
            .connect("udp:127.0.0.1:14550", move |snapshot| {
                let _ = sender.send(snapshot.status);
            })
            .await
            .expect("the link should open");

        let statuses = collect_until(&mut receiver, ConnectionStatus::Disconnected).await;

        assert!(!statuses.contains(&ConnectionStatus::Failed));
        assert_eq!(supervisor.status(), ConnectionStatus::Disconnected);
    }

    #[tokio::test]
    async fn a_link_that_cannot_be_opened_fails_with_a_typed_error() {
        let supervisor = SessionSupervisor::with_parts(
            policy(),
            Arc::new(ScriptedConnector::new(0)),
            Arc::new(RecordingSleeper::default()),
        );

        let error = supervisor
            .connect("udp:127.0.0.1:14550", |_| {})
            .await
            .expect_err("the link should not open");

        assert_eq!(error.code, "transport");
        assert_eq!(supervisor.status(), ConnectionStatus::Failed);
    }

    #[tokio::test]
    async fn a_link_that_cannot_be_opened_is_not_retried_behind_the_caller() {
        let connector = Arc::new(ScriptedConnector::new(0));

        let supervisor = SessionSupervisor::with_parts(
            policy(),
            Arc::clone(&connector) as Arc<dyn Connector>,
            Arc::new(RecordingSleeper::default()),
        );

        let _ = supervisor.connect("udp:127.0.0.1:14550", |_| {}).await;

        // The opening attempt is the only one: the retries belong to a link that was open
        // and dropped, not to a link that never came up.
        assert_eq!(connector.calls(), 1);
    }

    #[tokio::test]
    async fn an_empty_target_is_rejected() {
        let supervisor = SessionSupervisor::new();
        let error = supervisor
            .connect("   ", |_| {})
            .await
            .expect_err("the target should be rejected");

        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);
    }

    #[tokio::test]
    async fn a_malformed_target_is_rejected() {
        let supervisor = SessionSupervisor::new();
        let error = supervisor
            .connect("carrier-pigeon:COM3", |_| {})
            .await
            .expect_err("the target should be rejected");

        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);
    }

    #[tokio::test]
    async fn disconnecting_without_a_link_is_harmless() {
        let supervisor = SessionSupervisor::new();
        let response = supervisor.disconnect().await.unwrap();
        assert_eq!(response.status, ConnectionStatus::Disconnected);
    }

    #[tokio::test]
    async fn disconnecting_closes_the_link_and_says_so() {
        let supervisor = SessionSupervisor::with_parts(
            policy(),
            Arc::new(ScriptedConnector::new(1)),
            Arc::new(RecordingSleeper::default()),
        );

        let (sender, mut receiver) = mpsc::unbounded_channel::<ConnectionStatus>();

        supervisor
            .connect("udp:127.0.0.1:14550", move |snapshot| {
                let _ = sender.send(snapshot.status);
            })
            .await
            .expect("the link should open");

        let response = supervisor.disconnect().await.unwrap();

        assert_eq!(response.status, ConnectionStatus::Disconnected);

        let statuses = collect_until(&mut receiver, ConnectionStatus::Disconnected).await;
        assert_eq!(statuses.last(), Some(&ConnectionStatus::Disconnected));
        assert_eq!(supervisor.status(), ConnectionStatus::Disconnected);
    }
}
