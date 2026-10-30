# What is not closed

The port is closed for now. `docs/PARITY.md` says what each row of Mission Planner became; this file
says what is **still open**, what each thing needs before it can be closed, and how to prove it when
that thing is at hand. It is written for whoever picks this up next, and it assumes nothing about what
that person remembers.

The state when this was written: **149 rows done, 1 planned, 5 partial, 2 dropped** of 157.

---

## 1.18 Bluetooth LE — partial

**Done.** A Bluetooth serial channel (RFCOMM) is read as the COM port the machine's map gives it, with
the prefix rule and the two refusals that fall before anything travels. A Bluetooth **LE** device is
reached through its GATT serial service on **Windows** (`btleplug`, the system stack): the Nordic UART
(`6e400001`, write `6e400002`, notify `6e400003`) and the HM-10 `FFE1` profile are discovered, written
to and read from, a scan lists the devices, and the link opens a `gatt:<address>` target.

**Proved here.** nRF Connect on a phone as the peripheral: the scan saw the service, the connection
opened, `MPCraft BLE` reached the RX characteristic and a `hola` notification came back through TX.
That is a test peripheral, not a vehicle.

**What is missing.**

1. **A vehicle over BLE** — telemetry from a real module, not an echo from a phone. Nothing in the code
   is waiting for it; what is missing is the device.
2. **Linux and macOS.** The GATT path says `Unsupported` there (`crates/mpcraft-ble/src/stub.rs`).
   `btleplug` supports both, so the work is a dependency and an adapter, not a design: BlueZ over
   D-Bus on Linux, CoreBluetooth on macOS. It was parked because it could not be tested here.

**Where.** `crates/mpcraft-ble/` (lib, `windows.rs`, `stub.rs`, `tests/live.rs`), the `gatt:` scheme in
`crates/mpcraft-transport/src/spec.rs`, the supervisor's `gatt:` branch, the scan button in
`apps/ui/src/features/connection/ConnectionPanel.tsx`.

**To prove a closure.** `MPCRAFT_BLE_LIVE=1 MPCRAFT_BLE_ADDRESS=<address> cargo test -p mpcraft-ble
--test live -- --nocapture`, plus a link that reaches a vehicle over BLE and shows telemetry.

---

## 5.27 DroneCAN — partial

**Done.** The station speaks DroneCAN instead of waiting for what the autopilot forwards: it asks for
the bus (`MAV_CMD_CAN_FORWARD`), carries the protocol in `CAN_FRAME`, reads a node's information, lists
its parameters by index until it stops naming them, writes one and shows what the node **stored**, saves
and restarts it. The sheet does all of that. SLCAN — the adapter on a serial port — is a codec and a
client, tested against a scripted adapter.

**What is missing.**

1. **A real bus.** No CAN bus or SLCAN adapter has been opened here. What the evidence proves is the
   protocol against another implementation, not that a node of a vehicle answers.
2. ~~**SLCAN on screen.**~~ It has an entry now: `dronecan_slcan_open`/`_close`/`_status` keep the bus in
   `AppState`, the five calls have their `dronecan_slcan_*` commands, and the sheet has a section for
   the port, the rate and the node, with the same table the forwarded bus uses.

**Where.** `crates/mpcraft-dronecan/` (`can.rs`, `bits.rs`, `messages.rs`, `crc.rs`, `slcan.rs`),
`crates/mpcraft-link/src/can.rs`, `crates/mpcraft-app/src/dronecan.rs`, `crates/mpcraft-app/src/slcan.rs`,
`apps/ui/src/features/dronecan/DronecanSheet.tsx`, `crates/mpcraft-app/tests/dronecan_live.rs`,
`scripts/dronecan-node.py`.

**To prove a closure.** With an adapter: open it at the bus rate and see a node answer. Without a
vehicle: `pip install pymavlink; python scripts/dronecan-node.py --port 14650` and
`MPCRAFT_DRONECAN_LIVE=1 MPCRAFT_DRONECAN_WRITE=1 cargo test -p mpcraft-app --test dronecan_live --
--nocapture`, which is what was done here.

---

## 8.7 Secure firmware — partial

**Done.** An image is signed offline (Ed25519) in the layout the published signer uses, the document is
filled again with the image, its size, the room left and the SHA-512; a bootloader is given the keys it
checks with; and a signed `.apj` can now be **checked** — the block, the signature over everything
outside it and the SHA-512 the document carries, with "cannot be read" and "does not hold" kept apart.

**What is missing.**

1. **The vendor's hosted signing** (`ConfigSecure.cs`): signing through the service needs an account
   and a call to a third party. The offline signer makes the same document without either, so this is a
   choice about convenience rather than a gap in the format.
2. **A board with secure boot.** No such board was flashed here, so nothing proves the bootloader
   accepts what this signer writes.

**Where.** `crates/mpcraft-app/src/secure.rs` (signer, reader, keys), the firmware sheet.

**To prove a closure.** Flash a signed image to a secure-boot board and watch it start; the reader can
be pointed at any signed `.apj` today.

---

## 9.5 Formation — partial

**Done.** The slots, the places they put vehicles in, the input-filtered PID and the bearing rules, the
three answers (position target, quaternion for a plane, heading inside the band), the dispatch at ten
hertz over each vehicle's own state, the settings kept between runs and the sheet with Save, Start and
Stop. Proved over a real socket with four scripted vehicles: what reaches the wire is what the hand
computes.

**What is missing.** A fleet. These builds of the simulator refuse a device string for UDP and
`--slave` puts extra airframes on JSON control ports rather than on the link, so one wire cannot see two
vehicles; and the application opens one link. Closing it needs either real vehicles or a multi-link
feature that does not exist yet.

**Where.** `crates/mpcraft-vehicle/src/formation.rs` (pure decision), `crates/mpcraft-app/src/swarm.rs`
(dispatch, settings), `apps/ui/src/features/swarm/SwarmSheet.tsx`.

**To prove a closure.** Two or more vehicles on links the station can reach at once, told to fly into
their slots.

---

## 9.7 Video and cameras — partial

**Done.** A camera's stream is drawn through `ffmpeg` (RTSP, RTSPS, RTMP, UDP, SRT → MJPEG on the
loopback), the binary travels as a sidecar with its notice and licence, and the cameras that announce
themselves are found: `_rtsp._udp` by mDNS, with the name, the path and the sizes each one advertises,
one button per size. The same search finds a vehicle (`_mavlink._udp`) and offers its target.

**What is missing.** A camera that **advertises itself**. Every reading here was against an
advertisement this station made (`scripts/advertise-camera.py`) with a real camera behind it (a
Logitech Brio 100 through `mediamtx`). What that does not prove is the discovery of a device nobody
here configured.

**Where.** `crates/mpcraft-app/src/video.rs` (relay), `cameras.rs`, `vehicles.rs`, `announce.rs`
(browsing), `apps/ui/src/features/video/VideoSheet.tsx`, `scripts/fetch-ffmpeg.js`,
`scripts/advertise-camera.py`, `scripts/advertise-vehicle.py`, `licenses/`.

**To prove a closure.** An IP camera (or a board) publishing `_rtsp._udp` on the same network; open the
video sheet, press **Search the network** and draw what appears.

---

## 10.9 Code signing and notarisation — planned

No installer is signed and nothing is notarised. It needs a Windows code-signing certificate and an
Apple Developer account, so it cannot be done in this repository without the operator.

---

## One thing still pending in the tree itself

**`docs/PARITY.md` mixes languages.** The rows are written in English, but several paragraphs — the
whole DFU account and the Bluetooth one — were written in Spanish and never translated. The project's
rule is English everywhere; this is a real inconsistency, not a decision, and it is the next thing to
close. It is text only: nothing depends on it.

---

## Running the evidence again

The areas that read something outside this machine are behind a flag, so a machine without the thing
still runs the rest of the suite. What each needs, and what it proves, is in the file it lives in.

| Area | Flag | Also needs |
| --- | --- | --- |
| Video relay | `MPCRAFT_VIDEO_LIVE=1` | `MPCRAFT_VIDEO_SOURCE=rtsp://…` and `ffmpeg` |
| Camera discovery | `MPCRAFT_MDNS_LIVE=1` | `MPCRAFT_MDNS_NAME`, `_PORT`, `_PATH`; optionally `MPCRAFT_MDNS_STREAM=1` |
| Vehicle discovery | `MPCRAFT_MDNS_LIVE=1` | `MPCRAFT_MDNS_VEHICLE`, `_VEHICLE_PORT`; `MPCRAFT_MDNS_LINK=1` to connect |
| DroneCAN | `MPCRAFT_DRONECAN_LIVE=1` | `MPCRAFT_DRONECAN_TARGET` (default `udp:127.0.0.1:14650`), `_WRITE=1` |
| Bluetooth LE | `MPCRAFT_BLE_LIVE=1` | `MPCRAFT_BLE_ADDRESS` |
| SITL | `MPCRAFT_SITL_LIVE=1` | a simulator, or `scripts/sitl.ps1` |
| The board on a port | `MPCRAFT_HARDWARE_TARGET` | the port |
| DFU | `MPCRAFT_DFU_TARGET`, `MPCRAFT_DFU_IMAGE` | a board in its bootloader; `MPCRAFT_DFU_ALLOW_ERASE=1` to erase |
| Bluetooth rows | `MPCRAFT_BT_ROW` | the instance path |

## Building here, in case it helps

- The repository pins `stable-x86_64-pc-windows-msvc` in `rust-toolchain.toml`, and this machine has no
  `link.exe`; every command that compiles the application was run with
  `cargo +stable-x86_64-pc-windows-gnu`, and `tauri dev` with
  `RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-gnu`.
- The `ffmpeg` sidecar is fetched by `node scripts/fetch-ffmpeg.js`; the desktop crate's build script
  copies it, so the file has to be there before anything is compiled.
- A link that fails with `No space left on device` is the disk, not the code: `cargo clean` and run the
  gates again.
- `node scripts/run-gates.js` runs everything this machine can run and says which layer it did not read
  (`NO-APLICA`, `HERRAMIENTA-AUSENTE`, `SUSTITUIDA`).
