---
name: Add a Transport
description: Use when adding a new byte transport to MPCraft — serial, UDP, TCP, file replay, NTRIP, or a new link type — including the Transport trait implementation, reconnection, and tests.
metadata:
  opencode/autoinvoke: true
---

# Adding a transport to MPCraft

A transport moves bytes. It knows nothing about MAVLink.

## The contract

`crates/mpcraft-transport/src/lib.rs`

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    async fn write(&mut self, buf: &[u8]) -> io::Result<()>;
    fn kind(&self) -> TransportKind;
    async fn reconnect(&mut self) -> io::Result<()>;
    fn describe(&self) -> String;
}
```

A transport must:

- **Never block the async runtime.** If the underlying API is blocking, spawn it on
  `tokio::task::spawn_blocking` or use the crate's async variant.
- **Return `Ok(0)` on a clean EOF**, not an error. The reader loop interprets `0` as "peer closed".
- **Survive a partial write.** `write` must loop until the buffer is fully written, or return the
  error.
- **Support reconnection** where the medium allows it. For a file, `reconnect` rewinds to the
  start.

## Steps

1. **Add the variant to `TransportKind`.** It is used for UI labelling and for capability checks.

2. **Implement the struct.** Keep it `Send + Sync` if the session shares it.

3. **Implement the trait.** For `read`, use a bounded buffer — the caller passes the size.

4. **Wire it into the connection factory** in `crates/mpcraft-transport/src/factory.rs`. The
   factory parses a connection string:

   ```
   serial:/dev/ttyUSB0:57600
   udp:127.0.0.1:14550
   tcp:192.168.1.10:5760
   file:/logs/flight.tlog:1.0
   ```

   The format is `scheme:target[:params]`. Parse it into a typed `ConnectionSpec` with a typed
   error — never `unwrap`.

5. **Handle reconnection policy.** For network transports, exponential backoff with a cap. The
   decision to restore the *session* (not just the socket) belongs to
   `crates/mpcraft-app/src/supervisor.rs`.

6. **Expose it in the Tauri command surface** via `mpcraft-app`. The frontend must never name a
   transport type in a way that bypasses the typed spec.

## Testing

- **Round-trip test** through a loopback: write bytes, read them back.
- **EOF test**: closing the peer yields `Ok(0)`.
- **Reconnect test**: drop the connection, verify `reconnect` restores it.
- **Partial-write test**: a mock that accepts only N bytes per call proves the write loop.
- **Parser test**: the connection string parser accepts valid specs and rejects invalid ones with
  typed errors.

## Existing transports to imitate

| Transport | File | Notes |
| --- | --- | --- |
| Serial | `crates/mpcraft-transport/src/serial.rs` | `serialport` crate, DTR/RTS toggling for ESP32 boards |
| UDP | `crates/mpcraft-transport/src/udp.rs` | Client/server modes, multi-endpoint fan-out |
| TCP | `crates/mpcraft-transport/src/tcp.rs` | Auto-reconnect with backoff |
| File | `crates/mpcraft-transport/src/file.rs` | Replays a `.tlog` at a configurable rate — powers log playback and deterministic tests |

## Checklist

- [ ] `TransportKind` variant added
- [ ] Trait implemented, `Send + Sync`, no blocking on the runtime
- [ ] `Ok(0)` on clean EOF
- [ ] Write loops until complete
- [ ] Connection string parsed with typed errors
- [ ] Reconnect implemented where applicable
- [ ] Round-trip, EOF, and reconnect tests pass
- [ ] Exposed via `mpcraft-app` to the frontend
