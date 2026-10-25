# MPCraft — Architecture

## 1. Layering

```
┌───────────────────────────────────────────────────────────────────────┐
│ apps/ui            React 19 · TypeScript · Vite                       │
│                   MapLibre GL JS · uPlot · TanStack Table/Virtual     │
│                   (no protocol knowledge; consumes typed events)      │
└───────────────────────────────┬───────────────────────────────────────┘
                                │ Tauri IPC
                                │   commands: request/response
                                │   events:   telemetry, status, notifications
┌───────────────────────────────┴───────────────────────────────────────┐
│ apps/desktop      Tauri v2 binary — thin. Wires commands to the app. │
├───────────────────────────────────────────────────────────────────────┤
│ mpcraft-app       AppState, SessionSupervisor, EventBus, commands,   │
│                   connection profiles, auto-reconnect                 │
├───────────────────────────────────────────────────────────────────────┤
│ mpcraft-link      Session: heartbeat negotiation, transactions,      │
│                   params, missions, commands, packet-loss accounting  │
├───────────────────────────────────────────────────────────────────────┤
│ mpcraft-vehicle   Vehicle, Telemetry snapshot, derived state, modes  │
├───────────────────────────────────────────────────────────────────────┤
│ mpcraft-mission   MissionItem, MAV_CMD metadata, fences, rallies      │
│ mpcraft-params    Parameter metadata repository, type/range/bitmask   │
│ mpcraft-geo       Geodesy, UTM/MGRS, terrain (SRTM/DTED)             │
│ mpcraft-log       tlog · DataFlash · ULog parsers, KML/CSV export     │
├───────────────────────────────────────────────────────────────────────┤
│ mpcraft-mavlink   Framing, CRC, signing, dialect re-exports          │
├───────────────────────────────────────────────────────────────────────┤
│ mpcraft-transport Transport trait: serial · UDP · TCP · file · NTRIP │
└───────────────────────────────────────────────────────────────────────┘
```

Rules:

- Dependencies point **downward only**. A crate never depends on one above it.
- Only `mpcraft-app` and `apps/desktop` know about application-level concerns.
- `apps/ui` has **no** protocol knowledge. It receives typed JSON payloads.
- Every crate is `no_std`-compatible in intent (no mandatory alloc-heavy framework types),
  headless-testable, and free of Tauri types.

---

## 2. The session model

Mission Planner's central flaw is that *two* consumers compete for the socket: a background
`SerialReader` loop and every blocking request/response transaction, arbitrated by a cooperative
boolean flag (`giveComport`) plus a `SemaphoreSlim`. A transaction that forgets the flag deadlocks
the whole link.

MPCraft replaces this with an actor model.

```
                    ┌──────────────────────────────────┐
  Transport bytes ──►  Reader task (one per session)  │
                    │  • owns the byte stream         │
                    │  • parses frames                 │
                    │  • validates CRC / signature     │
                    │  • publishes to broadcast        │
                    └───────────────┬──────────────────┘
                                    │ broadcast::Sender<Packet>
              ┌─────────────────────┼─────────────────────┐
              ▼                     ▼                     ▼
      ┌───────────────┐     ┌───────────────┐     ┌───────────────┐
      │ Telemetry     │     │ Transaction   │     │ Packet        │
      │ aggregator    │     │ matchers      │     │ inspector     │
      │ → Vehicle     │     │ → oneshot     │     │ → UI feed     │
      │   registry    │     │   responders  │     │               │
      └───────────────┘     └───────────────┘     └───────────────┘
```

### 2.1 Transaction model

A transaction is a *matcher* plus a *responder channel*.

```rust
pub struct Transaction {
    pub matcher: PacketMatcher,   // msgid + optional payload predicate
    pub timeout: Duration,
    pub retries: u32,
}
```

`PacketMatcher` is a closure over the decoded payload. Because the broadcast channel is lossy
under load, a transaction registers itself with a dedicated waiter list rather than relying on
channel buffering alone — this is the key detail that makes request/response reliable at 20 Hz+
telemetry rates.

```rust
impl Session {
    pub async fn request<T>(&self, tx: Transaction) -> Result<T, LinkError> {
        for attempt in 0..=tx.retries {
            let rx = self.register(tx.matcher.clone()).await?;
            self.send(tx.packet()).await?;
            match tokio::time::timeout(tx.timeout, rx).await {
                Ok(Ok(msg)) => return Ok(decode(msg)),
                Ok(Err(_)) => continue,              // waiter cancelled
                Err(_) => continue,                  // timeout -> retry
            }
        }
        Err(LinkError::Timeout { .. })
    }
}
```

The `giveComport` bottleneck is gone: concurrent transactions are safe, and a slow transaction
cannot starve telemetry.

### 2.2 Telemetry aggregation

Decoded packets update a `Telemetry` struct held in an `ArcSwap`. The aggregator owns the
per-vehicle state and publishes snapshots only when a field actually changed.

```rust
pub struct Telemetry {
    pub attitude: Attitude,        // roll/pitch/yaw + quaternion
    pub position: Option<GeoPoint>,
    pub velocity: Velocity,
    pub battery: Battery,
    pub mode: FlightMode,
    pub armed: bool,
    pub home: Option<HomePosition>,
    pub ekf: EkfStatus,
    // ...
}
```

Derived quantities (e.g. ground speed from velocity, distance to home) are computed **once** when
the packet is processed, not on every UI access.

### 2.3 Vehicle registry

A session can carry multiple vehicles (companion computers, gimbals, cameras). The registry is
keyed by `(system_id, component_id)` — mirroring upstream's `MAVList`, but typed.

```rust
pub struct VehicleId { pub system_id: u8, pub component_id: u8 }
```

---

## 3. MAVLink layer

Built on the upstream `mavlink` crate (v0.20), which generates message definitions from the
canonical XML dialects via `mavlink-bindgen`.

### 3.1 What we take from upstream

- Message and enum definitions (`MAV_CMD`, `MAV_FRAME`, `MAV_PARAM_TYPE`, `MAV_MISSION_TYPE`, …)
- CRC-16/MCRF4XX implementation and CRC_EXTRA seed handling
- Frame serialisation/deserialisation for MAVLink v1 and v2
- MAVLink 2 signing (`mav2-message-signing` feature)
- Connection helpers for TCP/UDP/serial

### 3.2 What we implement ourselves

- A **buffered, state-machine frame parser** over an async byte stream. Upstream's reader is
  byte-at-a-time; ours reads into a buffer and advances a cursor.
- **Resynchronisation** on CRC failure: scan forward for the next valid STX, with a bounded search
  window and a resync counter exposed in diagnostics.
- **Packet-loss accounting**: sequence deltas per `(vehicle, msgid)`, distinguishing reordering from
  genuine loss, matching upstream's `recvpacketmask` behaviour.
- **Plaintext sniffing**: non-MAVLink printable bytes are captured into a bounded ring buffer and
  surfaced as `LinkDiagnostic::Plaintext` — this reproduces upstream's genuinely useful "here is the
  board's boot output" error message.
- **Signing key management**: persistent keyring, per-vehicle keys, monotonic timestamps.

### 3.3 Frame format (reference)

```
MAVLink v2 (STX 0xFD)
┌──────┬──────┬──────────────┬────────┬────────┬───────────┬────────────┬─────────┬─────────┐
│ 0xFD │ len  │ incompat     │ compat │  seq   │ sysid     │ compid    │ msgid(3) │ payload │ crc16  │
│      │      │ flags        │ flags  │        │ (1 or 4)  │           │  LE      │         │         │
└──────┴──────┴──────────────┴────────┴────────┴───────────┴────────────┴─────────┴─────────┴─────────┘
                                          optional: target_system (4 bytes if TARGET32 flag)
                                          optional: signature block (13 bytes if SIGNED flag)
```

CRC is computed over bytes `[1 .. header+payload)`, then `crc_accumulate(crc_extra_seed, crc)`.

---

## 4. Transport layer

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    async fn write_some(&mut self, buf: &[u8]) -> io::Result<usize>;
    fn kind(&self) -> TransportKind;
    fn is_open(&self) -> bool;
    async fn reconnect(&mut self) -> io::Result<()>;
    fn describe(&self) -> String;
    // Provided method: loops over partial writes.
    async fn write(&mut self, buf: &[u8]) -> io::Result<()>;
}
```

Implementations:

| Transport | Notes |
| --- | --- |
| `SerialTransport` | `tokio-serial`; DTR asserted low on open by default, which resets the ESP32-class boards that use it as a reset line; port enumeration |
| `UdpTransport` | Client and server mode; **fan-out** - a frame is written to every peer heard from, matching upstream's `UdpSerial.EndPointList` |
| `TcpTransport` | Client and server mode; the accept is deferred to the first read so a listening station never races its own startup |
| `FileTransport` | Replays a recorded byte stream at a configurable rate - powers log playback and deterministic tests |
| `NtripTransport` | RTCM correction source |

Auto-reconnect is implemented **in the transport** as `reconnect()`, matching upstream's
`TcpSerial.autoReconnect`. The *policy* - how many attempts, with what backoff, and whether
to surface the failure - lives in the supervisor in `mpcraft-app`, where it can be tested
without a real network peer.

### 4.1 Connection specifications

A connection target is a single string:

```text
serial:COM3:57600
serial:/dev/ttyUSB0:115200|nodtr
udp:127.0.0.1:14550
udp::14550|server
tcp:192.168.1.10:5760
file:/logs/flight.tlog|rate=1.0
```

The `|` separator keeps Windows paths such as `C:\logs\flight.tlog` unambiguous. Parsing
never guesses: an unrecognised option is a typed error rather than a silently ignored token.

### 4.2 What the transport layer does not know

A transport moves bytes. It does not parse MAVLink, does not know what a heartbeat is, and
does not interpret the contents of a file it replays. Replaying a `.tlog` with its *original*
timing requires understanding the log's record structure, which belongs to the log format;
that lives in `mpcraft-log`, which implements the same trait.

---

## 5. IPC contract

Tauri v2 commands are request/response; events are the push channel.

### 5.1 Command shape

```rust
#[tauri::command]
pub async fn connect(state: State<'_, AppState>, req: ConnectRequest) -> Result<ConnectResponse, CommandError> {
    state.supervisor.connect(req).await.map_err(Into::into)
}
```

Every command returns `Result<T, CommandError>`, where `CommandError` is a serialisable enum with
a stable `code` string. The frontend switches on `code`, never on message text.

### 5.2 Event shape

Telemetry is pushed as **batched snapshots**, not per-field events.

```rust
#[derive(Clone, Serialize)]
pub struct TelemetryEvent {
    pub vehicle: VehicleKey,
    pub seq: u64,
    pub t: Telemetry,
}
```

Batching matters: at 20 Hz with 40 fields, per-field events would mean 800 IPC messages/second.
MPCraft emits one batched event per UI frame tick (default 20 Hz, configurable).

### 5.3 Frontend state

The React app keeps a normalised store:

```
vehicleStore: Map<VehicleKey, Telemetry>
missionStore: MissionState
paramsStore:   ParamState
uiStore:       UiState (selection, panels, map viewport)
```

The Rust side is authoritative for vehicle data; the frontend never derives telemetry values.

---

## 6. Mission model

```rust
pub struct MissionItem {
    pub id: MissionItemId,     // stable identity across edits — critical for React keys
    pub command: MAV_CMD,
    pub frame: MAV_FRAME,
    pub params: [f64; 4],
    pub x: f64, pub y: f64, pub z: f64,
    pub mission_type: MAV_MISSION_TYPE,
    pub autocontinue: bool,
    pub current: bool,
    pub display_name: Option<String>,
}
```

`MAV_CMD` metadata (which parameters are lat/lon/alt/speed, their units, their ranges) is loaded
from `mavcmd.toml`, mirroring upstream's `mavcmd.xml` schema so the same knowledge transfers.

Round-trip correctness is enforced by a property test: serialise → parse → serialise must be
idempotent, and every `MAV_CMD` must survive the trip with its frame and parameters intact.

---

## 7. Error handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("transport: {0}")]
    Transport(#[from] io::Error),
    #[error("no vehicle answered within {0:?}")]
    Timeout(Duration),
    #[error("vehicle rejected the request: {0}")]
    Rejected(String),
    #[error("protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("cancelled")]
    Cancelled,
}
```

`mpcraft-app` converts these into `CommandError` with stable codes. The frontend maps codes to
localised messages. **A message is never parsed for meaning.**

---

## 8. Testing strategy

| Layer | Technique |
| --- | --- |
| Pure logic | Unit tests next to the code; `proptest` for the framer and parsers |
| Wire format | Golden files: real captured frames → expected decoded values |
| Round-trip | `serialise → parse → serialise` byte equality |
| Integration | A SITL container (ArduPilot `sitl.py` in Docker) driven by `mpcraft-cli` in CI |
| Compatibility | Upstream's own test corpus: real `.tlog`, `.param`, `.waypoints` files |
| Frontend | Vitest for stores and reducers; Playwright for the critical flows |
| Gates | A home-grown gate is shown failing on a known-bad case and passing on a known-good one, before its green is trusted (`scripts/check-overlaps-controls.js`) |

The SITL container is the linchpin: it lets the whole stack be tested without hardware, and the
same harness serves the demo builds.

---

## 9. Performance budget

| Path | Budget |
| --- | --- |
| Frame parse + CRC | < 5 µs |
| Telemetry decode + snapshot publish | < 20 µs |
| IPC event serialise | < 50 µs |
| UI frame at 20 Hz telemetry | < 8 ms |
| Idle CPU with an idle vehicle | < 1 % |

Achieved with a buffered parser, `ArcSwap` snapshots, batched events, and a `uPlot` renderer that
downsamples rather than replays raw samples.

---

## 10. Security

- MAVLink 2 signing is supported for both directions; keys stored encrypted in the OS keychain
  where available, otherwise obfuscated at rest.
- The WASM plugin sandbox (phase 6) grants capabilities explicitly, never ambient authority. It is
  the one place a runtime dependency is allowed, and the crate that holds it is the only one that
  knows the runtime exists.
- The app makes no unauthenticated network calls except tile/terrain fetches against configured
  providers, which the user can disable.
- Arm/disarm, mission write, and firmware flash require explicit confirmation.

---

[`docs/PLAN.md`]: docs/PLAN.md
