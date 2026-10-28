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
│ mpcraft-transport Trait: serial · BLE · UDP · TCP · file · NTRIP     │
│ mpcraft-winusb    OS binding for its bootloader, plus its serial map  │
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

The pair crosses the boundary as `systemId` and `componentId`: the Rust names keep the underscores, and
the JSON carries the casing the contract reads (§5.2).

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
| `BluetoothTransport` | A Bluetooth serial channel, which is a COM port with a reading beside it: the bytes are moved by the same code that moves them for any other serial port, so what this adds is only which port belongs to Bluetooth — read through the prefix the RFCOMM service writes in the map, not through the class the row says it is in |

Membership comes from the **service** the install bound the instance to (`BTHMODEM`, as the published driver
lists write it), not from the class a row names: a serial row says the Ports class, a hands-free row says the
audio class with another service behind it, and a personal-area-network row says the network class, so a rule
that decided by class would say nothing about the ports it was asked about. The whole prefix is compared rather
than a word inside a name, which is what SuperCollider's parser shows: it matches any port name, so a virtual
pair named `com0comBluetooth0` would be read as an RFCOMM row by a looser comparison. A group of zeros names
nothing — that is what an install leaves where nothing was written, not a value the machine decided — and it
keeps its own code (`bluetooth_no_device_named`), which says something different from
`bluetooth_row_not_the_shape`: one fact is a row the machine presents with no device behind it and the other is
a text the reader cannot read. Both refusals fall before anything travels on the cable. For this scheme the rate
is the number the driver was given rather than something the peer hears, since a Bluetooth channel has no rate;
`baud=auto` is refused because searching for rates promises what the medium cannot answer. The half that scans
and reads characteristics is not carried here: it would need a runtime, and the plugin sandbox is the only place
one is allowed, so upstream's enumeration filter — which looks for the Nordic UART service — has nothing to find
on a machine that carries no runtime and no peripheral advertising it.

A board in its bootloader is not a transport: it answers requests rather than moving bytes, so
the device layer is its own trait — `send`, `receive`, `string`, `describe` — and it carries no
operating-system types, because what it hands back is the functional descriptor and the strings the
interface offers, with a stable code for a device that cannot be reached, stalls, or answers with
something that is not a descriptor, and a fourth code for a device that is there and cannot be
opened, which is a different fact from a device that is not there. The binding that reaches a
physical bus lives in `crates/mpcraft-winusb`: it is the one place outside the plugin sandbox where
a system dependency is carried, and it has its own crate because `mpcraft-app` forbids unsafe code
while these are unsafe functions. What can be read without a bus stays outside `cfg(windows)` — the
pair as the environment writes it, the classification of an opening failure, the service name behind
the class-install key that makes an interface ours (`SPDRP_DRIVER` names the key rather than the
driver, so the key can never be the name it compares with), the part of an instance path these calls
read as the enumerator, the pair a published reader uses to call an interface one of this kind, which
is a second rule and not the same one, and the shape of a class name — which is one name on this
machine, because pydfu's filter and Windows' own class-install INF for devices of this kind
(`c_usbdevice.inf`) write the same string — plus the value the install leaves on the device key,
`DeviceInterfaceGUIDs`, which is the fact that decides whether a row can exist for an instance and is
a different fact from the service name. Reading that key needs no elevation, so the binding reads it;
writing it does, and the gate names that as what the operator supplies, beside BOOT held at power-on.
The calls themselves are behind `cfg(windows)`. Whether the application reaches a board is decided in
`mpcraft-app`, behind `hardware-tests`, and the live area holds each state as data rather than
asserting one expected outcome: what the registry remembers and what the bus answers for are two
readings, and a number a refusal carries is read through the dictionary of the call that answered it,
because 13 says `CR_NO_SUCH_DEVNODE` through the Config Manager dictionary and `ERROR_INVALID_DATA`
through the Win32 one, which is the overlap that made an earlier green read a wrong address as a
missing board. Through the same `cfg(windows)` half the crate reads the machine's own map of serial ports
(`HKLM\HARDWARE\DEVICEMAP\SERIALCOMM`), which is what Microsoft's reader lists ports from, and each number
there belongs to the call that answered it through that call's dictionary — `RegOpenKeyExW` and
`RegEnumValueW` answer through the Win32 one, not through the Config Manager one — and what ended the list is
kept beside the count, so a reading that stopped at its own bound says it is bounded rather than being read as
a whole list. Nothing here writes to that key: reading it asks for no elevation.

Auto-reconnect is implemented **in the transport** as `reconnect()`, matching upstream's
`TcpSerial.autoReconnect`. The *policy* - how many attempts, with what backoff, and whether
to surface the failure - lives in the supervisor in `mpcraft-app`, where it can be tested
without a real network peer.

### 4.1 Connection specifications

A connection target is a single string:

```text
serial:COM3:57600
serial:/dev/ttyUSB0:115200|nodtr
ble:COM40:9600|nodtr
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

Telemetry is pushed as **batched snapshots**, not per-field events. The desktop publishes two
events: `link-state` carries the state of the link as one document (`StateEvent`), and
`playback-state` carries the telemetry of an instant of a log that is being played back
(`PlaybackState`).

```rust
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateEvent {
    pub status: ConnectionStatus,
    pub vehicles: Vec<VehicleEvent>,   // keyed by (system_id, component_id)
    pub telemetry: TelemetryEvent,      // the primary vehicle's snapshot
    pub loss: LossEvent,
    pub messages: Vec<MessageEvent>,
    pub board: Option<String>,
    pub output: BoardOutput,
    pub adsb: Vec<AdsbEvent>,
    pub gimbals: Vec<GimbalDevice>,
    pub cameras: Vec<CameraDevice>,
}
```

Batching matters: at 20 Hz with 40 fields, per-field events would mean 800 IPC messages/second.
MPCraft emits one batched event per UI frame tick (default 20 Hz, configurable).

The boundary speaks **one casing**: every struct whose JSON reaches the frontend writes its fields in
camelCase, and the contract (`apps/ui/src/lib/ipc/types.ts`) declares each event as one document that
every reader of that event reads through. A field spelled two ways is a value the interface never gets,
and `scripts/check-contract.js` reads both sides against each other — the events listened to against the
events emitted, and the documents field by field, including the ones that reach the frontend under another
name, where the pair is what the same field says on both sides (`Telemetry` read against `TelemetryEvent`).
A command crosses the boundary the same way, so the calls are read too: every call goes to a command that
answers, and the document a call reads the answer as is compared with the document that command answers with.
A command also *reads* a document through its argument, and the direction changes what a field means: a field the
struct carries and the contract does not declare is a fact on the way back and a finding on the way in, because on
the way in it is a value the backend expects and the interface never sends. The side is kept as its own fact rather
than assumed, since one document can be answered by one command and read by another, and the pair an argument makes
comes from what its name says — the contract's own name, or the pair its field says — never from a table written by
the gate.

A command takes its arguments by the **key** the message carries: Tauri reads the argument name as camelCase, and the
handles it supplies itself (`State<…>` and `AppHandle`) are not keys the interface ever sends. So the same gate reads
what each call puts on the wire against every key its command reads — through the keys a helper declares, and through
a shorthand property, which says its own name. A key the command needs and the call never sends is a call the
boundary refuses; a key the call sends that no argument reads is a value that travels and is never looked at. An
optional parameter left out is said as a fact, because a missing key is what an `Option` reads as.

The documents the interface **sends** are declared by the contract too, so a call site cannot decide a shape for
itself. `ParamChange`, `OsdSlotRequest`, `PlanItemRequest`, `SwarmRequest` and `SigningKeyEvent` are named now, and
the fields of those documents are read against the commands that read them through their arguments. What the compiler
enforces is that a call sends one of these; what the reading compares is the pair of names, not the values that sit
under a key, and that difference is kept because a shape a call site invents is a document one side holds and the
other cannot be checked against.

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
| Integration | A SITL container (ArduPilot `sitl.py` in Docker) driven by `mpcraft-cli`. In CI the half that needs no vehicle — the golden files — runs on the runner, and the live half is named as `HERRAMIENTA-AUSENTE` when no image can be pulled anonymously; it runs against a simulator started locally with `MPCRAFT_SITL_LIVE=1`. A live area that reaches a board names the scripted peer that stands in for it, and its green is read from the report of its own test binary, because the reports of the tests that ran before it say nothing about a device that was never there. The area that reads what a machine writes about itself names its stand-in the same way, because a peer that answers a transport contract cannot show what a registry says, so a green from the scripted half does not stand for an area that read no rows |
| Compatibility | Upstream's own test corpus: real `.tlog`, `.param`, `.waypoints` files |
| Frontend | Vitest for stores and reducers; Playwright for the critical flows; eslint for what a reader would have to notice by hand — a name written and never used, a dependency list that does not match what the effect reads. It reads the sources this project can change and names what it leaves out: the vendored MapLibre worker under `public/`, and the React JSX rules, because the plugin for those cannot run on the version of eslint declared in `apps/ui/package.json`. A rule that is not carried cannot be switched off, so a source file that disables one is a finding. Prettier is scoped to the files a change touched, so a line already outside the width says nothing until its own file is touched — which is what happened here: three lines of `apps/ui/src/lib/params/paramsStore.ts` were already wider than the rule asks, and they speak now only because this change touched that file, so the file is read as the formatting gate expects rather than left as it was |
| Gates | `scripts/run-gates.js` runs them and says which layer it did not read (`NO-APLICA`, `HERRAMIENTA-AUSENTE`, `SUSTITUIDA`), so a green that rests on nothing is not read as a pass. A home-grown gate is shown failing on a known-bad case and passing on a known-good one before its green is trusted (`scripts/check-overlaps-controls.js`, `scripts/run-gates-controls.js`, `scripts/check-lint-controls.js`, `scripts/check-parity-controls.js`, `scripts/check-dfu-crc-controls.js`). The parity matrix is read rather than remembered: its counts are worked out from the rows (`scripts/check-parity.js`). And every gate that reads a tool rather than a document names its own known-bad case (`scripts/check-tool-controls.js`), because a gate whose green is its exit code alone cannot fail; only one of those five shows the finding itself, since the runner keeps the first line of what a tool says, and the other four show the state turning. A busy artifact is read as a gap rather than as green, and that reading is shown with a genuine sharing violation held by a separate process (`scripts/run-gates-controls.js`), not with a string that looks like one. The control column is read as a document too: every gate names its known-bad case and the checker under `scripts/` it names is on the disk, with both halves shown firing on a copy of the runner. And the parity checker names what it cannot reach inside its own report, not only in its header, which is read as a rule with two known-bad copies (`scripts/check-parity-controls.js`). The runner carries the same rule for itself: what the whole run cannot see — no window, no vehicle, no board, and no tool reading beyond the file it was shown — is named inside what it reports, and that reading is shown firing on two copies of the runner (`scripts/run-gates-controls.js`), not on the runner as it stands. |

The SITL container is the linchpin: it lets the whole stack be tested without hardware, and the
same harness serves the demo builds. It is not published anonymously, so on the runner the live half is
said as a tool that is not there rather than failing at an initialization it cannot complete; the same
suite answers against a simulator started locally, where a vehicle is really on the wire.

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
- A system dependency is not the same thing as a runtime dependency. `mpcraft-winusb` resolves its
  import names through the loader (`raw-dylib`), so nothing is loaded while the program runs, and it
  is the only crate that knows WinUSB exists; the unsafe surface stops at that crate, because the
  device trait below it carries no operating-system types.
- A Bluetooth serial channel is reached through the machine's own writings, and the reading half asks for no
  elevation: `HKLM\HARDWARE\DEVICEMAP\SERIALCOMM` and the values on a device key are read as they are written,
  while writing a class key does ask for elevation and is named as what the operator supplies. So does the subkey
  that holds which services a paired peripheral announces: on this machine it answered with a refusal at the
  privilege the run has, and that refusal is the fact that keeps upstream's enumeration filter outside what can be
  asserted here. Nothing is opened on a port that belongs to a module that may be using it, and both refusals at
  the rules — a row with no device behind it, a row that is not the shape a reader can read — fall before anything
  travels on the cable. The GATT half is not carried because it would need a runtime, which only the plugin sandbox
  is allowed to carry.
- A board in its bootloader is named by the environment as the pair the bus writes it
  (`MPCRAFT_DFU_TARGET`), not as a connection specification, because it answers requests rather than
  moving bytes. The half that erases runs only when the operator names it with a flag
  (`MPCRAFT_DFU_ALLOW_ERASE`), and getting the board into its bootloader is something the operator
  does — holding BOOT at power-on, or the command that holds it there — which the gate says rather
  than assumes, because a live area cannot do it.
- The app makes no unauthenticated network calls except tile/terrain fetches against configured
  providers, which the user can disable.
- Arm/disarm, mission write, and firmware flash require explicit confirmation. The half that
  erases a board or writes an image to it runs only when the operator names it with a flag in the
  environment, because a board erased for nothing cannot be put back by anybody else.

---

[`docs/PLAN.md`]: docs/PLAN.md
