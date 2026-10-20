---
name: Add a Tauri Command
description: Use when adding or changing a Tauri command, IPC event, or the Rust-TypeScript contract in MPCraft — covers error codes, typed payloads, batching, and the frontend store update.
metadata:
  opencode/autoinvoke: true
---

# Adding a Tauri command to MPCraft

The IPC contract is a public API. Treat every change as a versioned interface change.

## Layering

```
Frontend (TS)
   │ invokes command / listens to event
   ▼
apps/desktop/src-tauri/src/commands.rs   ← thin: unwrap state, call mpcraft-app
   ▼
crates/mpcraft-app/src/commands.rs        ← real logic, typed errors
   ▼
crates/mpcraft-link, -mission, -params    ← domain crates
```

`apps/desktop` must contain no business logic. If logic lands there, move it to `mpcraft-app`.

## Command shape

```rust
// crates/mpcraft-app/src/commands.rs
#[derive(Debug, Deserialize)]
pub struct SetModeRequest {
    pub vehicle: VehicleKey,
    pub mode: FlightMode,
}

#[tauri::command]
pub async fn set_mode(
    state: State<'_, AppState>,
    req: SetModeRequest,
) -> Result<SetModeResponse, CommandError> {
    state
        .supervisor
        .set_mode(req)
        .await
        .map_err(|e| CommandError::from_link(e))
}
```

### Rules

1. **Every command returns `Result<T, CommandError>`.** Never `Option`, never a bare `bool`.
2. **`CommandError` carries a stable `code`.** The frontend branches on the code, never on the
   message text. Codes are snake_case and permanent: `link_timeout`, `no_vehicle`, `not_connected`,
   `rejected_by_vehicle`, `invalid_request`, `internal`.
3. **The request type is deserialised, not flattened.** One request object per command.
4. **No `unwrap`.** Convert errors explicitly.
5. **Commands must not block.** If the work is long, spawn it and return a job handle.

## Event shape

Events push state. They are the high-frequency path.

```rust
#[derive(Clone, Serialize)]
pub struct TelemetryEvent {
    pub vehicle: VehicleKey,
    pub seq: u64,
    pub t: Telemetry,
}
```

### Batching rules

- **One event per logical batch, not per field.** At 20 Hz with 40 fields, per-field events mean
  800 IPC messages per second. Do not do it.
- **Telemetry events are emitted on a UI tick timer**, not on every packet. Default 20 Hz.
- **Discrete events** (mode changed, mission uploaded, vehicle disconnected) fire immediately.

## Frontend side

1. **Type definitions** live in `apps/ui/src/lib/ipc/types.ts`, mirroring the Rust types exactly.

2. **The store** is updated by a single subscription point, not scattered across components:

   ```ts
   // apps/ui/src/lib/ipc/bridge.ts
   listen<TelemetryEvent>('telemetry', (e) => {
     vehicleStore.getState().applyTelemetry(e.payload)
   })
   ```

3. **Stores use fine-grained selectors.** A telemetry tick must not re-render the parameter table.

   ```ts
   const alt = useVehicleStore(s => s.get(vehicleId)?.telemetry.position?.alt)
   ```

4. **Errors are handled centrally.** A wrapper turns `CommandError` into a typed rejection, so
   components handle `code` rather than parsing strings.

## Adding a new command

- [ ] Rust request/response types defined in `mpcraft-app`
- [ ] Command implemented with typed error mapping
- [ ] Registered in the Tauri invoke handler in `apps/desktop`
- [ ] TypeScript types added to `apps/ui/src/lib/ipc/types.ts`
- [ ] Frontend call site added, using the typed bridge
- [ ] Error codes handled by the caller
- [ ] A test asserting the error mapping
- [ ] `docs/PARITY.md` updated if this completes a parity row

## Changing an existing command

- **Additive change** (new optional field) — fine, but make the Rust field `#[serde(default)]` or
  `Option<T>` so old clients still work.
- **Breaking change** — bump the API version, update every call site, and note it in the commit
  message. There is no wire compatibility guarantee with an older frontend build, but a stale
  frontend against a new backend must fail with a clear error, not silently misbehave.
