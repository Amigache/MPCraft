# MPCraft — Parity Matrix

Feature-by-feature comparison with Mission Planner. Every upstream feature must be marked
**Done**, **Planned**, or **Dropped** (with rationale). No feature may be silently omitted.

**Legend:** ✅ Done · 🔜 Planned (phase) · ⬜ Dropped · 🟡 Partial · 🔽 Partial, the work is under way

Reference: `ardupilot/MissionPlanner` at commit `1f4a2e0` (analysed from `_ref_missionplanner/`).

---

## 1. Connection & link management

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 1.1 | Serial port connection | `ExtLibs/Comms/CommsSerialPort.cs` | ✅ | 0 |
| 1.2 | Serial port enumeration | `ExtLibs/Controls/ConnectionControl.cs` | ✅ | 1 |
| 1.3 | Baud-rate selection & auto-baud | `ExtLibs/Controls/ConnectionControl.cs` | ✅ | 1 |
| 1.4 | UDP client / server mode | `ExtLibs/Comms/CommsUdpSerial.cs` | ✅ | 0 |
| 1.5 | UDP multi-endpoint fan-out | `CommsUdpSerial.EndPointList` | ✅ | 0 |
| 1.6 | TCP client / server mode | `ExtLibs/Comms/CommsTCPSerial.cs` | ✅ | 0 |
| 1.7 | TCP auto-reconnect | `TcpSerial.autoReconnect` | ✅ | 1 |
| 1.8 | Heartbeat negotiation & vehicle selection | `MAVLinkInterface.OpenBg` | ✅ | 0 |
| 1.9 | Vehicle-type detection (Copter/Plane/Rover/Sub/Tracker) | `MAVLinkInterface.setAPType` | ✅ | 0 |
| 1.10 | Multi-vehicle registry | `ExtLibs/ArduPilot/Mavlink/MAVList.cs` | ✅ | 0 |
| 1.11 | Packet-loss & reorder accounting | `MAVLinkInterface.readPacketAsync` | ✅ | 0 |
| 1.12 | Plaintext / boot-output sniffing | `MAVLinkInterface.plaintxtline` | ✅ | 0 |
| 1.13 | MAVLink 2 signing (out & in) | `MAVLinkInterface.generatePacket`, `CheckSignature` | ✅ | 0 |
| 1.14 | Signing keyring persistence | `ExtLibs/ArduPilot/Mavlink/MAVAuthKeys.cs` | ✅ | 0 |
| 1.15 | MAVLink FTP client | `ExtLibs/ArduPilot/Mavlink/MAVFtp.cs` | ✅ | 5 |
| 1.16 | Auto-connect profiles | `ExtLibs/Utilities/AutoConnect.cs` | ✅ | 7 |
| 1.17 | NTRIP correction source | `ExtLibs/Comms/CommsNTRIP.cs` | ✅ | 5 |
| 1.18 | Bluetooth LE transport | `ExtLibs/Comms/CommsBLE.cs` | 🔽 | 5 |
| 1.19 | File / tlog replay as a transport | `ExtLibs/Comms/CommsFile.cs` | ✅ | 0 |
| 1.20 | Serial passthrough (`SERIAL_CONTROL`) | `MAVLinkSerialPort.cs` | ✅ | 5 |

## 2. Flight data & telemetry

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 2.1 | Live telemetry readouts | `ExtLibs/ArduPilot/CurrentState.cs` | ✅ | 1 |
| 2.2 | Artificial-horizon HUD | `ExtLibs/Controls/HUD.cs` | ✅ | 1 |
| 2.3 | Live multi-channel graphs | `GCSViews/FlightData.cs` (ZedGraph) | ✅ | 1 |
| 2.4 | Map with live vehicle position | `GCSViews/FlightData.cs` (myGMAP) | ✅ | 1 |
| 2.5 | Flight trail / track history | `FlightData.cs` | ✅ | 1 |
| 2.6 | Quick-view configurable tiles | `ExtLibs/Controls/QuickView.cs` | ✅ | 1 |
| 2.7 | Gauges (speed / altitude / vspeed) | `Controls/AGauge.cs`, `HSI.cs` | ✅ | 1 |
| 2.8 | STATUSTEXT message console | `FlightData.cs` | ✅ | 1 |
| 2.9 | Pre-flight checklist | `Controls/PreFlight/CheckListControl.cs` | ✅ | 1 |
| 2.10 | Battery banks 1–9 monitoring | `CurrentState.cs` | ✅ | 1 |
| 2.11 | Multi-GPS status | `CurrentState.cs` | ✅ | 1 |
| 2.12 | EKF status & vibration display | `Controls/EKFStatus.cs`, `Vibration.cs` | ✅ | 3 |
| 2.13 | Servo & relay output display | `Controls/ServoOptions.cs`, `RelayOptions.cs` | ✅ | 3 |
| 2.14 | Auxiliary function display | `Controls/AuxOptions.cs` | ✅ | 3 |
| 2.15 | Transponder (Mode-S) control | `FlightData.cs` tabTransponder | ✅ | 5 |
| 2.16 | EFI / generator telemetry | `CurrentState.cs` | ✅ | 5 |
| 2.17 | OpenDroneID display | `ExtLibs/ArduPilot/OpenDroneID.cs` | ✅ | 5 |
| 2.18 | Wind direction indicator | `Controls/WindDir.cs` | ✅ | 2 |
| 2.19 | Distance-to-target bar | `Controls/DistanceBar.cs` | ✅ | 2 |

## 3. Actions & commands

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 3.1 | Arm / disarm | `MAVLinkInterface.doARM` | ✅ | 1 |
| 3.2 | Mode change (custom_mode table) | `Common.getModesList`, `translateMode` | ✅ | 1 |
| 3.3 | RTL / Land / Loiter / Auto shortcuts | `FlightData.cs` | ✅ | 1 |
| 3.4 | Takeoff command | `MAVLinkInterface.doCommand` | ✅ | 1 |
| 3.5 | Generic MAV_CMD long / int executor | `doCommand`, `doCommandInt` | ✅ | 1 |
| 3.6 | Guided mode — fly to waypoint | `setGuidedModeWP` | ✅ | 2 |
| 3.7 | Guided mode — "go here" from map | `FlightData.cs` goHereToolStripMenuItem | ✅ | 2 |
| 3.8 | Position target global-int | `setPositionTargetGlobalInt` | ✅ | 2 |
| 3.9 | Motor test | `doMotorTest` | ✅ | 3 |
| 3.10 | Reboot autopilot | `doReboot` | ✅ | 5 |
| 3.11 | RC override | `SendRCOverride` | ✅ | 3 |
| 3.12 | Manual control | `SendManualControl` | ✅ | 3 |
| 3.13 | Camera / gimbal control | `CameraProtocol`, `GimbalManagerProtocol` | ✅ | 5 |
| 3.14 | Serial port pass-through | `SendSerialControl` | ✅ | 5 |
| 3.15 | Flight-mode presets & custom modes | `ConfigFlightModes.cs` | ✅ | 3 |

## 4. Mission planning

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 4.1 | Mission download / upload | `ExtLibs/ArduPilot/mav_mission.cs` | ✅ | 2 |
| 4.2 | `MISSION_ITEM_INT` support | `getWPAsync` (capability-gated) | ✅ | 2 |
| 4.3 | Partial mission update | `mav_mission.uploadPartial` | ✅ | 2 |
| 4.4 | Waypoint table editor | `GCSViews/FlightPlanner.cs` (Commands grid) | ✅ | 2 |
| 4.5 | Map-based waypoint add / drag / delete | `FlightPlanner.cs` | ✅ | 2 |
| 4.6 | MAV_CMD parameter metadata & units | `mavcmd.xml` | ✅ | 2 |
| 4.7 | Altitude modes (absolute / relative / terrain) | `FlightPlanner.cs` altmode | ✅ | 2 |
| 4.8 | Geo-fence polygon drawing & upload | `FlightPlanner.cs` polygon tools | ✅ | 2 |
| 4.9 | Fence inclusion / exclusion lists | `FenceInclusion/Exclusion` | ✅ | 2 |
| 4.10 | Rally point upload / download | `getRallyPoint`, `setRallyPoint` | ✅ | 2 |
| 4.11 | Home position management | `getHomePositionAsync` | ✅ | 2 |
| 4.12 | QGC `.waypoints` import / export | `FlightPlanner.cs` readQGC110wpfile | ✅ | 2 |
| 4.13 | KML import / export | `FlightPlanner.cs` processKML | ✅ | 2 |
| 4.14 | SHP / DXF import | `FlightPlanner.cs` LoadSHPFile | ✅ | 2 |
| 4.15 | Circle / spline / survey mission generators | `FlightPlanner.cs` circle tools | ✅ | 2 |
| 4.16 | Mission validation & warnings | `FlightPlanner.cs` checkZeroAlts | ✅ | 2 |
| 4.17 | Undo / redo buffer | `FlightPlanner.cs` history | ✅ | 2 |
| 4.18 | Elevation profile graph | `Controls/ElevationProfile.cs` | ✅ | 2 |
| 4.19 | Waypoint radius (loiter) editing | `FlightPlanner.cs` loiter radius | ✅ | 2 |
| 4.20 | Spline / smooth path display | `FlightPlanner.cs` | ✅ | 2 |
| 4.21 | MAVFTP mission transfer | `chk_usemavftp` path | ✅ | 5 |

## 5. Parameters & configuration

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 5.1 | Full parameter download with progress | `MAVLinkInterface.getParamListAsync` | ✅ | 3 |
| 5.2 | Parameter write with echo confirmation | `setParamAsync` | ✅ | 3 |
| 5.3 | Batched write with `*_ENABLE` ordering | `ConfigRawParams.SortENABLE` | ✅ | 3 |
| 5.4 | Live parameter updates | `processInfoFromStream` PARAM_VALUE | ✅ | 3 |
| 5.5 | Parameter metadata (ranges, bitmasks, options, units) | `ParameterMetaData.xml` | ✅ | 3 |
| 5.6 | Parameter tree by prefix | `ConfigRawParams.BuildTree` | ✅ | 3 |
| 5.7 | Parameter search | `ConfigRawParams.txt_search` | ✅ | 3 |
| 5.8 | Favourites | `ConfigRawParams` Fav column | ✅ | 3 |
| 5.9 | Friendly (metadata-driven) parameter UI | `ConfigFriendlyParams.cs` | ✅ | 3 |
| 5.10 | `.param` file import / export | `ExtLibs/Utilities/ParamFile.cs` | ✅ | 3 |
| 5.11 | Parameter diff against a file | `Controls/paramcompare.cs` | ✅ | 3 |
| 5.12 | Write-to-flash / reset-to-defaults | `ConfigRawParams` | ✅ | 3 |
| 5.13 | Reboot-required detection & prompt | `GetParameterRebootRequired` | ✅ | 3 |
| 5.14 | Frame type selection | `ConfigFrameType.cs` | ✅ | 3 |
| 5.15 | Accelerometer calibration | `ConfigAccelerometerCalibration.cs` | ✅ | 3 |
| 5.16 | Compass calibration & motor compensation | `ConfigCompassMot.cs` | ✅ | 3 |
| 5.17 | Motor test | `ConfigMotorTest.cs` | ✅ | 3 |
| 5.18 | GPS ordering / auto-detect | `ConfigGPSOrder.cs` | ✅ | 3 |
| 5.19 | Serial port configuration | `ConfigSerial.cs` | ✅ | 3 |
| 5.20 | RC input / output configuration | `ConfigRadioInput.cs`, `ConfigRadioOutput.cs` | ✅ | 3 |
| 5.21 | Failsafe configuration | `ConfigFailSafe.cs` | ✅ | 3 |
| 5.22 | Flight modes configuration | `ConfigFlightModes.cs` | ✅ | 3 |
| 5.23 | OSD configuration | `ConfigOSD.cs` | ✅ | 3 |
| 5.24 | Mount / gimbal configuration | `ConfigMount.cs` | ✅ | 5 |
| 5.25 | ESC calibration | `ConfigESCCalibration.cs` | ✅ | 3 |
| 5.26 | Hardware device detection | `ExtLibs/ArduPilot/BoardDetect.cs` | ✅ | 5 |
| 5.27 | DroneCAN device configuration | `ConfigDroneCAN.cs` | 🔽 | 5 |

## 6. Logs & analysis

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 6.1 | DataFlash `.log` parser | `ExtLibs/Utilities/DFLog.cs` | ✅ | 4 |
| 6.2 | DataFlash `.bin` parser | `ExtLibs/Utilities/BinaryLog.cs` | ✅ | 4 |
| 6.3 | MAVLink `.tlog` parser | `Log/MavlinkLog.cs` | ✅ | 4 |
| 6.4 | PX4 ULog parser | `ExtLibs/Utilities/ULog.cs` (upstream: unwired) | ✅ | 4 |
| 6.5 | Log download over MAVLink | `Log/LogDownloadMavLink.cs` | ✅ | 4 |
| 6.6 | Log download over SCP | `Log/LogDownloadscp.cs` | 🔜 | 5 |
| 6.7 | Log browser — message tree + sample grid | `Log/LogBrowse.cs` | ✅ | 4 |
| 6.8 | Log graphing with multi-axis | `LogBrowse.CreateChart` | ✅ | 4 |
| 6.9 | Graph preset definitions | `graphs/mavgraphs.xml` | ✅ | 4 |
| 6.10 | Cross-highlight grid ↔ graph ↔ map | `LogBrowse.GoToSample` | ✅ | 4 |
| 6.11 | tlog playback with scrub timeline | `FlightData.cs` tracklog | ✅ | 4 |
| 6.12 | KML export of flight track | `Log/LogOutput.cs` | ✅ | 4 |
| 6.13 | CSV export of samples | `LogBrowse` export | ✅ | 4 |
| 6.14 | Python LogAnalyzer integration | `LogAnalyzer/` | ⬜ | Dropped — depends on Python 2 toolchain; replaced by native analysis in 6.7–6.8 |

## 7. Mapping & geospatial

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 7.1 | MapLibre base map with provider selection | `ExtLibs/Maps/*` | ✅ | 1 |
| 7.2 | OSM / satellite / hybrid tile providers | `GMapProviders` registry | ✅ | 1 |
| 7.3 | Tile caching on disk | `ExtLibs/Maps/MyImageCache.cs` | ✅ | 1 |
| 7.4 | Offline map injection (custom tile folder) | `FlightPlanner.BUT_InjectCustomMap_Click` | ✅ | 2 |
| 7.5 | Tile prefetch for a region | `ExtLibs/GMap.NET.WindowsForms/TilePrefetcher.cs` | ✅ | 2 |
| 7.6 | Geofence / rally / home markers | `ExtLibs/Maps/GMapMarker*.cs` | ✅ | 2 |
| 7.7 | UTM / MGRS coordinate conversion | `ExtLibs/Utilities/utmpos.cs`, `GeoUtility` | ✅ | 2 |
| 7.8 | SRTM terrain elevation lookup | `ExtLibs/Utilities/srtm.cs` | ✅ | 2 |
| 7.9 | DTED local elevation | `ExtLibs/Utilities/DTED.cs` | ✅ | 5 |
| 7.10 | Terrain-follow route generation | `srtm.getIntersectionWithTerrain` | ✅ | 5 |
| 7.11 | Terrain data protocol server | `ExtLibs/Utilities/TerrainFollow.cs` | ✅ | 5 |
| 7.12 | Geo-referenced image placement | `ExtLibs/GeoRef/*` | ✅ | 5 |
| 7.13 | ADS-B display layer | `ExtLibs/Utilities/adsb.cs` | ✅ | 5 |
| 7.14 | ADS-B relay to other stations | `MainV2.ADSBRunner` | ✅ | 5 |

## 8. Firmware & flashing

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 8.1 | Firmware manifest download | `GCSViews/ConfigurationView/ConfigFirmwareManifest.cs` | ✅ | 5 |
| 8.2 | Firmware selection by board | `ConfigFirmware.cs` | ✅ | 5 |
| 8.3 | Firmware upload over MAVLink | `ExtLibs/px4uploader` | ✅ | 5 |
| 8.4 | DFU upload over USB | `ExtLibs/UAVCANFlasher`, `NetDFULib` | 🔽 | 5 |
| 8.5 | Board auto-detect | `BoardDetect.cs` | ✅ | 5 |
| 8.6 | Firmware erase / bootloader mode | `ConfigFirmware.cs` | ✅ | 5 |
| 8.7 | Secure firmware (signed) | `ConfigSecure.cs`, `ConfigSecureAP.cs` | 🔜 | 5 |

## 9. Extensibility & integration

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 9.1 | Plugin lifecycle (Init/Loaded/Loop/Exit) | `Plugin/Plugin.cs` | ✅ | 6 |
| 9.2 | Plugin host API surface | `Plugin/Plugin.cs` (PluginHost) | ✅ | 6 |
| 9.3 | Plugin management UI | `Plugin/PluginUI.cs` | ✅ | 6 |
| 9.4 | Scripting host | `Script.cs` (IronPython) | ⬜ | Dropped — a language engine that loads every assembly in the process; what its API offers has an equivalent in the capability-scoped plugin host (9.1–9.3), while a text file the operator pastes is not a module with a manifest a person reads before running it |
| 9.5 | Swarm / formation control | `Swarm/` | 🔽 | 6 |
| 9.6 | Joystick + RC override mapping | `Joystick/JoystickSetup.cs` | ✅ | 5 |
| 9.7 | Video stream / camera integration | `Controls/Video.cs`, `GimbalVideoControl.cs` | 🟡 | 5 |
| 9.8 | HTTP server (MJPEG / KML stream) | `ExtLibs/Utilities/httpserver.cs` | ✅ | 7 |
| 9.9 | KML live output | `FlightPlanner.writeKML` | ✅ | 2 |

## 10. Application-level

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 10.1 | Screen-switching shell | `ExtLibs/Controls/MainSwitcher.cs` | ✅ | 1 |
| 10.2 | Theme system (light / dark / high-contrast) | `ThemeManager.cs`, `*.mpsystheme` | ✅ | 7 |
| 10.3 | Internationalisation | `Strings.resx` + Crowdin | ✅ | 7 |
| 10.4 | Unit system switching (metric / imperial) | `CurrentState.multiplier*` | ✅ | 1 |
| 10.5 | Settings persistence | `ExtLibs/Utilities/Settings.cs` | ✅ | 0 |
| 10.6 | Auto-updater | `Updater/` | 🔜 | 7 |
| 10.7 | Crash handler / diagnostics bundle | `Program.cs` | ✅ | 7 |
| 10.8 | Windows / Linux / macOS packaging | `wix/`, `Msi/` | 🔜 | 7 |
| 10.9 | Code signing & notarisation | `wix/` | 🔜 | 7 |

---

## 11. Summary

| Status | Count |
| --- | --- |
| ✅ Done | 143 |
| 🔜 Planned | 5 |
| ⬜ Dropped | 2 |
| Partial (🟡 / 🔽) | 5 |

The 143 rows marked done are the connection, transport, parameter, mission, command and
plugin layers, and the flight data window that uses them. The transports, the heartbeat
negotiation, the vehicle registry, the loss accounting, the board-output capture, the
parameter protocol, the mission protocol, the command executor and the file formats all
work and are covered by tests.

The flight data window has upstream's distribution: a menu strip, a fixed column on the
left with the artificial horizon above a set of tabs - quick view, actions, messages, the
check list before the flight, the dials, the state of the vehicle, the outputs and the
auxiliary functions - the map filling everything else, and a status strip. The live readouts,
the attitude indicator, the map with the vehicle and the track, the plan drawn on it, the
dials, the tiles and the vehicle's own messages are all on screen.

The window is read at 1024x640 and at 1920x1080, and the boxes are measured rather than looked
at: nothing is drawn over anything else and nothing is cut off. A tile of the quick view keeps
the line its number is written on even when the name above it wraps in another language, because
the number is what the operator reads first — the tile grows before the number is cut.

The **screens** are what upstream's main switcher is: the flight data, the plan, the
configuration and the logs. The window is on one of them at a time, and the switcher above
the body chooses which; the arrow keys move between them, and the body is the panel labelled
by the one that is open. What a screen holds is kept when the operator leaves it and comes
back - a log that was opened, a search that was typed, a graph that was chosen - because the
screens stay mounted and only draw while they are the screen that is open, which is what
upstream's tabs do with the panels underneath them. The short tasks are not screens: a
calibration, a joystick mapping, a file transfer, the transponder or the video sheet are
sheets that lie over whichever screen is open, because the operator does them and comes back
to where they were, and while a sheet lies over the window its controls cannot be reached.
The parameters and the logs used to be sheets of that kind; they became two of the four
screens, and Escape takes the window back to the flight data. The screen the window was on is
remembered between two runs.

The **parameter editor** is the screen of the configuration: the tree of the groups by prefix on
the left, the parameters on the right, a search box, and a value that can be edited in
place. The write waits for the vehicle to confirm it, and the interface shows the value
the vehicle stored, which is not always the one that was asked for: a vehicle clamps to
its range, and the operator has to see what the vehicle has rather than what they
wanted.

The parameters the operator marks as **favourites** come first in the list and are kept
between two runs, and the Setup menu carries the **groups** the parameters belong to, the
way upstream's carries its configuration screens. The description, the unit, the range
and the value a parameter had before anybody changed it are shown with the parameter, and
the parameters of a file can be read, written and compared with the vehicle's.

The **plan editor** takes the place of the graphs, the way upstream's planner puts its grid
above the map. The waypoints are dragged on the map, the plan is checked before it is
flown, and the edits are kept apart from the plan the vehicle holds, so that a plan with
unsent changes never looks like the plan on the vehicle. The rows and the drawing of the
ground share the room the bar and the status line leave, and the drawing gives way before
the rows do, so a short editor still shows a plan that can be read.

The **home** of the vehicle is read and moved from the actions tab: the vehicle is
asked where its home is, and the home can be moved to the place the vehicle is in,
which is what an operator does when the vehicle was armed in the wrong place. The
vehicle confirms the move by reporting the home it now holds, and a vehicle that
refuses it keeps the one it had.

A **part of the plan** can be written to the vehicle without touching the rest of it:
the editor writes the plan from the item that is selected onwards, and the items
before it stay on the vehicle as they are.

A **drawing** can be read into the plan, which is how a route that was drawn on a map
is flown without drawing it again: the operator picks a shapefile of a GIS or the DXF
of a CAD program, and the places of the drawing become waypoints. The projection beside
a shapefile is read — the `.prj` file of the same name — so a drawing that was made in
a UTM zone comes back in degrees rather than in metres, and a drawing that says nothing
of its projection is taken to be in the coordinates the ground station speaks. What a
drawing cannot say is how high to fly, so its items start at a hundred metres above the
home position, and the operator checks that before the plan goes to the vehicle.

The **ground** can also be read from a folder of DTED cells, which is what the heights
of the military format are called. The folder is opened from the Setup menu, the sheet
says what is in it, and the height of the ground under the vehicle is read from it. From
those heights a **survey that follows the ground** is worked out: the strips run across
the line that joins the two places, one after the other, and the vehicle flies at the
same height over the ground everywhere, which is what a camera that needs the same
distance to the ground everywhere needs. The survey is added to the plan.

The vehicle that follows the ground asks for it: the request says where the vehicle is
and which squares of the grid are wanted, and the ground station answers with the
heights of the squares, read from the cells that are open.

The **firmware** of the vehicles is chosen from the Setup menu: the manifest that
ArduPilot publishes is read, and the operator picks the vehicle, the version and the
board, and the file of that board is fetched to the disk. The file is the firmware of
the autopilot, the one that is written to a board.

The **firmware** can also be written to a board, from the same sheet: the board has to
be in its bootloader, and the file is written to it over the port serie — the board is
waited for, asked what it is, compared with the file before anything is erased, its flash
is erased, the image is written block by block, what the board says about its own flash is
read back, and the board is told to start again.

The file the manifest points at is **read as the document it is** rather than as bytes for
the flash: an `.apj` says which board the image is for, how big the image is, and carries the
image compressed and written in base64, so a path that took the file as it stands on the disk
would put the text of the document into the board. The two numbers are kept apart because they
are not the same thing — the length the document declares, and the bytes as they go to the
flash, which are padded to a multiple of four because the protocol counts a length in a byte
and the board wants blocks of four. The length the sheet shows is the key the published
document carries (`image_size`); `size` is one it does not carry, and `sha256` is another, so
the sheet says nothing about a hash rather than inventing one — the number that was shown
before came from keys this reader looked for and the published document never has.

The check the board can be asked for after writing is worked out with the rule the bootloader
counts with, which is not the ordinary CRC32: the reflected table, starting at zero, without
the starting mask or the final inversion — what `crc32_small(sum, word, 4)` over the flashable
area gives, which is what the published bootloader does. A zlib reader gives a different number
for the same bytes, so the golden values here come from an implementation written from the
published polynomial, whose table was then compared entry by entry with the table the reference
clone carries: `a1848b49` for the fixture image alone and `4b98510b` for it inside a flash of
1000. And the number depends on the board rather than only on the file, because the part of the
flash that is not written is read as erased, and erased reads as fours of `0xFF` up to the size
the board says — which is why the rule counts a block that runs past the end of the flash.

A board that already holds this image is not erased for nothing: the station asks the board for
the number its flash holds before erasing, and refuses when that number is the one the file
gives, unless the operator says otherwise with the box on the sheet — which is what upstream has
as `allowSameUpload`. A board of revision three answers the number; a board of revision two cannot
be asked for one, so its blocks are read back one by one instead, and that is decided from what the
board says about itself rather than assumed from the file. Both refusals happen before the erase,
because the erase is the half that cannot be taken back, and a file for another board would leave
this one with nothing on it.

The command that writes the sector of the bootloader carries **290876 in slot five**, which is what
the published dictionary says the slot is for — "Magic number - set to 290876 to actually flash" —
and the code was sending zeros there. What the published handler does with the number is a write: it
compares the copy the vehicle carries in its own filesystem with the first page of the flash and
writes it when they differ, and counts a sector that already matches as a success, which is why
upstream warns the operator at the same moment ("This can brick your board"). It leaves nothing
behind for the next boot, so the board goes on speaking as a vehicle after answering, and that is
what the bench showed: the command with the slots empty was answered `MAV_RESULT_IN_PROGRESS` — which
is the acknowledgement of a long operation rather than a statement about the number — and the one with
the number was answered `ACCEPTED` without the board taking over the port. The half that erases is
therefore not asserted here, and it is said rather than assumed: the gate that reads `GET_DEVICE` and
`GET_CRC` can only run against a board that is on the bootloader side.

Which side of the protocol a board speaks is decided by another command, and the two are kept apart
now: `MAV_CMD_PREFLIGHT_REBOOT_SHUTDOWN` with the action that holds the board in its bootloader
(three), which on this board is the only way from this machine because it has no button and taking the
power away does not carry the flag the loader reads. The number of the command is **246**, read from
the published dictionary rather than trusted to the dialect, and that comparison is what caught an
earlier probe sending 42760 — a number the dictionary does not have — and reading the answer as a
refusal of the action when it was an answer about a number the board did not know. The handler denies
a restart addressed to any component other than all components or the system's own, so the command
goes to the pair the link chose for the board rather than to an address somebody assumes. With 246 and
the action that holds it there this board answered `MAV_RESULT_ACCEPTED` and then answered as its own
bootloader, and the numbers it gave are the ones the rule and the published document give without
being built from each other: revision 5, board 9, flash 2 080 768, crc `b690e491` read from the board,
and the number the published manifest gives for it (9) against the number the vehicle reports for
itself (`board_version` 589824, which is `0x09 << 16`), and the git hash the vehicle reports is the one
the published entry for `Plane/stable-4.7.1/fmuv3` carries, so the board holds the published image byte
for byte. What the live area asserts is the agreement between the two readers and the bytes the helper
puts on the wire; what it records is the answer kept apart from silence, and it sends only when someone
says so, because a board that answers this stays on the bootloader side until somebody tells it to
start again or takes the power away, and the other live areas read this same board as a vehicle.

The message that carries the command (`MAV_CMD_FLASH_BOOTLOADER`, 42650) is built byte by byte rather
than through the enumeration of commands of the protocol library, so the number travels as it was
written and the answer of the vehicle is read as an answer is read. That is what a command the metadata
table does not describe needs, because the generic executor cannot say what its slots mean and does not
offer them — and the dialect this project uses does name 42650, so the raw path is kept for the numbers
it does not. The link stops carrying MAVLink when the board is in the bootloader, because the board is
then waiting for the protocol of the bootloader. The sheet says what each of its two buttons does,
because they are two different things: one writes a sector of the flash and the board keeps speaking as
a vehicle after answering, and the other decides which protocol the board speaks from there on.

The flash of a board can also be **erased**, from the same sheet: the board is asked
what it is and its memory is left clean, which is what the operator wants when the
board does not start.

La placa en su modo de recuperacion se alcanza por el protocolo DFU, y la capa que la
alcanza es la del dispositivo: un rasgo que envia, recibe, pide una cadena y se describe,
sin tipos del sistema operativo, porque lo que devuelve es el descriptor funcional y las
cadenas que la interfaz ofrece — y el vinculo que llega a un bus real vive en
`crates/mpcraft-winusb`, alcanzado desde la aplicacion detras de `hardware-tests`,
porque una capa por debajo de la aplicacion no puede saber que WinUSB existe y porque
`mpcraft-app` declara `forbid(unsafe_code)` mientras que las llamadas del sistema son
funciones unsafe, lo que le da al vinculo su propio crate. El descriptor que una lectura
publicada escribe para un STM32 es `09 21 0B FF 00 00 08 1A 01`, y ese es el literal con
el que el area compara lo que la placa ponga en el cable cuando haya una interfaz que
abrir; el numero que ese cuerpo guarda (`0x011a`) y el que la misma herramienta da para la
familia (`0x0413`) son los que STM32CubeProgrammer dice de esta placa, asi que el acuerdo
entre los dos caminos es un hecho y no una suposicion — y hoy, con cero filas publicadas,
esa comparacion sigue pendiente de que la placa publique una ruta.

Los numeros guardados vienen de una segunda implementacion del formato
(`scripts/check-dfu-crc.js`), escrita desde la formula publicada y no desde la lectura que
se comprueba: `bcebc9b4` para el cuerpo del fixture y `a02b6bff` para los mismos bytes
leidos con la regla que el espectro describe. Los dos se conservan, porque el desacuerdo
es un hecho sobre las dos lecturas y no algo que la mano pueda cerrar; lo que este
proyecto lleva es la primera, que es la que cuatro implementaciones escritas por caminos
distintos escriben igual — la tabla reflejada de `crc32_table` en `src/dfu_file.c` de
dfu-util, `tools/pydfu.py`, `dfuse-pack.py` de betaflight y el `dfu.py` de tinyusb para el
fomu —, acumulando desde `0xFFFFFFFF` sin inversion final, mientras que la redaccion del
espectro, con un polinomio no reflejado y inversion al final, da otro numero para los
mismos bytes. El numero se trabaja sobre todo lo que esta delante del campo del CRC, lo
cual incluye los once bytes del prefijo, y las cinco muestras se comparan con las dos
tablas, que es lo que hace que una fila escrita como si las dos lecturas coincidieran sea
un hallazgo.

La cadena de memoria se lee como la lee dfu-util: el numero hasta el primer caracter que
no es digito, y la multiplicacion solo cuando la letra de unidad esta escrita. Lo que se
habia dicho antes sobre las dos formas publicadas de la misma cadena — que su lectura da
los mismos sectores para 1024K — era incorrecto y se corrige aqui: no lo da.
`04*016Kg,01*064Kg,07*128Kg` hace los doce sectores que la geometria que el propio
escritor de ArduPilot declara para esta familia (f405, 1024K) produce, 4 de 16K, 1 de 64K
y 7 de 128K; `000016*1024,0001*64*1024,0007*128*1024`, leida con esa regla, reparte en 24
tramos que suman 17 344 bytes. El desacuerdo se registra y no se cierra: la lectura de
fwupd (`plugins/dfu/fu-dfu-target.c`) rechaza esos tokens porque pide una letra de unidad
y un tamano entre 1 y 999, la expresion de pydfu no los lee porque pide unidad y permiso
los dos, y aqui el token se lee y la letra que ninguno de los dos diccionarios da se
conserva como dato, con el sector sin permiso, porque "no dicho" no es un permiso. El
diccionario de permisos es el de las siete letras que dfu-util y fwupd escriben igual —
`a` lee, `b` borra, `c` lee y borra, `d` escribe, `e` lee y escribe, `f` borra y escribe,
`g` hace los tres — y las siete se comparan letra por letra contra las dos tablas
publicadas, que es lo que permite afirmar el significado sin escribirlo dos veces. Un
sector que no dice permiso no se borra, y esa negativa cae antes de que nada viaje por el
cable, que es lo que el lector publicado hace con `find_segment` y `DFUSE_ERASABLE`.

La orden de borrar todo es otro desacuerdo entre lecturas: dfu-util escribe el numero con
longitud 1 para el borrado del dispositivo entero — su propio comentario dice "Mass erase
command when length = 1" — y pydfu manda `"\x41"` solo, mientras que la lectura publicada
de ArduPilot entiende solo la forma de cinco bytes y contesta la otra como una orden que
no se entiende. Este codigo pone el byte solo en el cable, y el test afirma las dos
mitades: el numero que escribe y lo que la otra lectura habria contestado con los mismos
bytes. Lo que se afirma del estado es lo que el peer publicado implementa: la marca de
ocupado se pone en cada peticion — no una sola vez por peer —, el primer GETSTATUS
responde ocupado con el milimetro que esa lectura escribe y el segundo responde libre, y
la transicion de MANIFEST_SYNC a MANIFEST ocurre al contestar y no al recibir.

Lo que se lee sin placa son las reglas, y las llamadas estan separadas de ellas: el crate
lleva el rasgo del dispositivo contestado con las llamadas del sistema detras de
`cfg(windows)`, y lo que se afirma sin bus es como se lee la pareja (`0483:df11`, escrita
por el entorno como el bus la escribe, porque una placa en su cargador no es una
especificacion de conexion), como se clasifica una negativa al abrir, como se lee el
enumerador a partir de una ruta de instancia, y el nombre del servicio detras de la clave
de clase que hace que una interfaz sea nuestra: `SPDRP_DRIVER` dice la clave y
`SPDRP_SERVICE` dice el servicio detras de ella, que son dos cosas distintas y una clave
no puede ser nunca el nombre con el que la regla compara — en esta placa la clave dice
`{88bae032-5a81-49f0-bc3d-a4ff138216d6}\0011` y el servicio dice `WinUSB`, asi que la
regla lee el segundo. El cuarto codigo es `device_not_ours`: la placa esta y no se puede
abrir, que es otra cosa que `device_unreachable`, y las dos se leen por caminos distintos
— la segunda cae cuando nada presente responde, la primera
cuando el servicio no es el nuestro, cuando el asa no se puede tomar, o cuando la instancia
esta y no publica ninguna interfaz que se pueda leer. Un paro y otra
negativa se parecen por este camino, asi que el vinculo dice que la interfaz rechazo la
peticion y dice que no puede decir cual de las dos fue; el numero con que el sistema
contesto no tiene donde viajar dentro de ese codigo, y es lo que el area viva escribe
cuando una placa contesta, con el diccionario nombrado junto al numero porque el diccionario
lo decide la llamada y no el numero: 13 es `ERROR_INVALID_DATA` cuando lo contesto
`SetupDiGetClassDevsW` — que es lo que pasa cuando se le da la ruta de la instancia donde
la llama pide el enumerador — y es `CR_NO_SUCH_DEVNODE` cuando lo contesto
`CM_Locate_DevNodeW`, y esas son dos hechos sobre la misma placa que un verde anterior
confundio. Un elemento se alcanza por su id con `SetupDiOpenDeviceInfoW`, que es otra lectura
que enumerar un conjunto armado a partir de una ruta de instancia, y el numero que cada llamada
contesta se atribuye a esa llamada dentro del codigo que lo lleva. El numero de la interfaz y el
indice de la cadena de memoria se leen de la interfaz, que es lo que pydfu hace por
`intf.iInterface` y no por un numero que el lector elige. Y la regla que decide que una interfaz
es de esta clase es otra lectura que la del vinculo: pydfu pide el par 0xFE y 1, aqui se decide
por el nombre del servicio, y las dos se comparan contra la misma placa en lugar de tratarse como
una; una tercera lectura es lo que la declaracion de la instancia dice antes de que nada se abra,
donde el par aparece como texto (`USB\Class_FE&SubClass_01&Prot_02`) y se lee con una regla que
rechaza una cadena que no lo dice en lugar de inventarlo.

Lo que se dijo antes sobre dos clases se corrige aqui: no son dos nombres. El que pydfu usa para
su filtro es el mismo que el INF de clase que Windows publica para esta familia de dispositivos
(`c_usbdevice.inf`, leido en esta maquina) escribe como su `ClassGUID`, asi que en esta maquina hay
un solo nombre, y el acuerdo entre el filtro y el instalador es un hecho sobre dos implementaciones
que no se hicieron la una desde la otra. Lo que decide que exista una fila para una instancia no es
ese nombre: es el valor que la instalacion escribe sobre la clave del dispositivo,
`DeviceInterfaceGUIDs`. El paquete publicado para esta placa nombra el guid que escribiria
(`{01105872-BF45-43BE-8B67-3C0F2B8CF0D9}`) y tiene una rama cuyo propio comentario dice que el
camino generico lo salta, y esa es la rama que esta maquina tiene: la clave no lleva el valor, y la
lectura por `reg query` lo dice igual que la lectura del crate, que lo lee sin elevacion. Leer esa
clave no pide privilegios; escribirla si, y por eso la puerta nombra esa escritura como lo que el
operador pone, junto con BOOT sostenido al encender: el crate lee y no escribe.

El area viva (`crates/mpcraft-app/tests/dfu_live.rs`, detras de `hardware-tests` y `windows`)
afirma cada mitad por sus propios hechos y no por un resultado esperado: el registro conserva
cinco filas para `USB\VID_0483&PID_DF11` y hoy una de ellas contesta como estando — la que la
placa pone ahora en el cable, `USB\VID_0483&PID_DF11\3357316C3536` — mientras las otras cuatro
responden `CR_NO_SUCH_DEVNODE` (13), porque recordar no es estar: `CM_Get_Device_ID_ListW` da las
filas que el registro tiene y la presencia se decide con `CM_Locate_DevNodeW`, que es lo que
separa las dos lecturas y lo que el area afirma fila por fila. Una segunda lectura del mismo
registro, la propiedad que Windows expone por WMI, da las mismas cinco filas, asi que el conteo no
depende del camino por el que se lee, y esa lectura dice `Status OK` para la fila que hoy esta, con
el nombre que el elemento lleva: `STM32  BOOTLOADER`. Lo que el elemento contesta por su id es lo
que el area compara, y alli la clave de clase `{88bae032-5a81-49f0-bc3d-a4ff138216d6}\0011` y el
servicio `WinUSB` son dos nombres del mismo hecho: la regla lee el segundo, y el nombre que el
elemento dice que su clase es es el que el instalador de clase escribe, que es el mismo nombre con
el que pydfu filtra. La declaracion del elemento lleva el par 0xFE y 1 como texto, que es la
tercera lectura y coincide con el par que el filtro publicado pide aunque por otro camino. El area
lee tambien el valor que la instalacion dejo en la clave del dispositivo y afirma la regla que lo
une con la fila, no un resultado esperado: si no hay valor las dos lecturas de la lista tienen que
dar cero filas, y si lo hay tienen que dar al menos una para la misma instancia; hoy el valor
contesta con nada y las dos listas dan cero, que es lo que `reg query` dice para la misma clave.

Lo que no se puede leer hoy por este vinculo se dice en lugar de suponerse: la instancia esta y su
servicio es el nuestro, pero no publica ninguna interfaz — la lista da cero filas tanto para el
nombre que el elemento dice como para el guid que WinUSB escribe como muestra, que es lo que se
espera cuando el valor que decide la fila no esta escrito, y `SetupDiEnumDeviceInterfaces` contesto
259 (no hay mas elementos) para los dos, que es como lo lee la segunda lectura del mismo hecho; por
eso el descriptor, el indice de la cadena y el estado no se pueden afirmar por este camino, y lo que
el area afirma hoy es la negativa `device_not_ours`, que cae antes de que nada viaje por el cable y
que es el codigo correcto porque la placa si esta. pydfu tampoco encontraria una fila: su filtro mira
su propia clase, que es la misma que aqui da cero. La lectura por todas las clases no la lleva este
vinculo — la bandera de "todas las clases" no es una llamada que este crate use, y la segunda lectura
que la pidio fue contestada con 4 como numero del diccionario de Config Manager, no con una lista —
asi que lo afirmado descansa en los dos nombres publicados. Lo que la placa podria decir cuando haya
una interfaz que abrir sigue escrito contra las dos lecturas publicadas: los nueve bytes del
descriptor, el numero `0x011a` que STM32CubeProgrammer da para esta placa, y los doce sectores de la
geometria que el propio escritor de ArduPilot declara para esta familia (4 de 16K, 1 de 64K, 7 de
128K, un megabyte entero), y esa mitad corre cuando una instancia publique una ruta. Poner la
placa alli no depende de esta maquina: se alcanza sosteniendo BOOT al encender o con la
orden que la sostiene, y la puerta nombra eso como lo que el operador pone; y un solo
proceso tiene el dispositivo a la vez, por eso el area corre sola y las comparaciones
con STM32CubeProgrammer van de una en una. El entorno nombra la pareja
(`MPCRAFT_DFU_TARGET`) y la mitad que borra esta detras de `MPCRAFT_DFU_ALLOW_ERASE`, que
es la regla que ya estaba para la mitad que borra una placa.

La fila de la puerta para ese area nombra su sustituto — el peer escrito desde la lectura
publicada en `crates/mpcraft-app/tests/dfu.rs`, que contesta las peticiones sin ser una
placa que las decida — y el control lo lee con un transcripto donde la mitad de las pruebas
del vinculo paso y el area de la placa fallo, con las dos mitades leidas como sus propios
informes, que es lo que hace que un verde del vinculo no valga por un area que no alcanzo
ninguna placa.

Lo que no se puede afirmar aqui se dice en lugar de suponerse: no hay un bus fisico en
esta maquina, asi que lo que el test prueba es la forma — el descriptor, la cadena, la
maquina de estados, la regla del bloque y las negativas antes de enviar — respondida por
un peer escrito desde la lectura publicada, no que una placa real conteste esas peticiones;
esa mitad vive detras de `hardware-tests`, donde la placa decide cual de las dos formas de
la orden de borrado entiende. La puerta (`scripts/check-dfu-crc.js`) nombra su caso
conocido-malo (`scripts/check-dfu-crc-controls.js`: un literal que no coincide con su
propia tabla, una pareja escrita como si las dos lecturas coincidieran, un escritor cuyo
tamano del objetivo va sin su prefijo) y lo que no alcanza: lee los literales como texto,
no ejecuta el modulo, y la comparacion entre la regla del codigo y estas tablas es lo que
el test del propio modulo lleva. Y la copia de la forma que el area viva lleva no la lee
esa puerta, que lee los literales de `tests/dfu.rs`: lo que el area compara es lo que la
placa pone en el cable contra lo que la lectura publicada escribe, y eso se sabe solo
cuando una placa contesta. Por eso la fila sigue parcial.

The **files of the vehicle** are read from the File menu: the vehicle holds a filesystem
of its own, and the file transfer protocol is how a program reaches it. The folders are
listed the way the vehicle says them, the operator walks into them, and a file is
brought to the disk of the computer. The protocol is written by hand as well — the header of
the transfer, the opcodes, and the payload of each answer — because the message is the 110th and
the seed is worked out by the rule rather than copied from the generator. What was said before
about this is corrected now: the compiled dialect does carry the 110th, the 310th and the 311th,
because it is generated from the canonical XML at build time rather than from an older snapshot,
so the table of seeds is not what the runtime uses — `extra_crc` takes the seed from the generator
first and reaches the table only for an id it does not carry. The table is there for the comparison,
and the tests say the two agree, which is what makes the order the fields are read in a fact rather
than a guess: a different order would give a different number.

A refusal from that protocol travels as the numbers the vehicle named rather than as a sentence: the opcode the
answer is for, the code from the dictionary, and the file-system number when the code says one is present. That is
what the interface needs, because it branches on codes and never reads a message for meaning — and the decision
about what to do next reads those numbers too, which is what an earlier rule could not do: a vehicle that says it
does not know an opcode is asked again with the older command, while a refusal that names a number is the file
system speaking and is not asked again, because asking with another opcode would say the same thing twice. The case
that shows the two are different is a refusal whose words contain "Fail" while its code names an error number: read
as text it would be retried, and read as data it is not.

What this board says is not what the specification says, and the two are kept apart rather than merged: the
specification gives the shape of a refusal for a server that does not implement a command, and this build answers
an unknown opcode with a generic failure that names no number, so the rule accepts both — which is why the number
is read from the answer rather than from the rendering. The number this board gives for every folder is the one
ArduPilot's mapping gives a drive that cannot work, taken from the published source rather than copied from what
the board answered, and it is the same fact the startup text gave for the card that failed to start, said in two
channels by the same machine. A card that can work changes what this area can assert: with one inserted, the folder
answers with the entries it holds rather than with a number, and the walk is read against those entries — five
folders at the root, one inside `APM`, and an empty `LOGS` — so the fall-back and the number are still read, but as
one of two states the card decides, and the run holds in both.

The **end of a file is a code rather than a failure**: the byte the vehicle names when there is nothing more is not
a refusal of what was asked, and both readers stop there — the reader of a file instead of counting to the number
the open answered with, and the reader of a folder instead of turning the answer after its last entry into an error.
That number is what the vehicle says about the file, and for the files it generates when asked it is not what the
file has — this board said 100000 bytes for one of its text files and answered with 122 — so the two numbers are
kept apart in what the reader gives back, and the interface says when they differ rather than showing a size that is
not there. What the reader returns is the bytes the vehicle gave; the number is kept as the vehicle said it rather
than overwritten by what arrived.

The shape of an entry on this build is what decided what a reader has to skip: the format puts a zero after the name
and the writer that builds the entry adds its own terminator on top of it, so two zeros separate the entries. Read
as an entry, that second zero takes the letter of the next one into its name — the root of this card came back as
"DAPM" rather than "APM" before the rule existed — and it makes the number the next ask carries wrong, because the
vehicle counts entries rather than separators. The names are checked against what the same bytes say to a client
built from the published format rather than from this crate: pymavlink's own reader gives the same five names, and
the same code ends its listing.

A refusal that was read once on the bench did not reproduce on a fresh connection: a read asked while nothing was
open was refused with the code that says the file was not found, and the same ask answered with bytes on the next
run. That is an observation about the state a previous process left on the port — the vehicle keeps one open file
per channel and closes it after three seconds of nothing sent on it — rather than a rule, so nothing here depends on
which of the two this build chooses.

The address a request carries is the pair the link chose from the boards that answered, not the address a station
would assume: on the bench this board answered nothing addressed to `1:1` and answered the pair it had chosen for
itself, which is what the encoder writes now. And each exchange takes its own sequence number, because the board
re-sends the answer for a number it has already seen — a probe that reused one read the same refusal four times,
which is how that rule was found.

A **plan** can also be carried in the file the vehicle keeps it in, over that same
protocol, the way upstream offers it with its `chk_usemavftp` box. The editor has the
box too: with it marked, the plan is read from the file of the vehicle and written to
it, instead of travelling with the mission protocol. The file is the one ArduPilot
keeps — `@MISSION/mission.dat` for the mission, and one of its own for the fence and
for the rally points — and its format is a header of twelve bytes and the items, each
one the payload of a `MISSION_ITEM_INT` message.

The **board** that is on the other end of the link is told by the output it prints when
it starts, and that output is already captured: the link keeps the text that is not
part of a MAVLink frame, and the board is recognised in it. The name it is given is the
one the manifest of the firmware uses, so that the board that is detected is the one
the firmware is chosen for.

The **flight** can be served on the network, from the View menu: another program can
follow it without a link of its own. The server answers three things — the page that
shows the flight, the state of the vehicle as JSON, and the track of the flight as the
KML document that is being written. It is a small server, on purpose: it answers those
three things and nothing else.

The **serial ports** of the vehicle can be reached from the File menu: the bytes travel
inside a MAVLink message and the vehicle hands them over to the port, which is how a
program talks to what is on it. The port, the speed and the time the vehicle waits are
chosen, and the text that is written goes as it is. What comes back from the port is
shown in the same sheet: the vehicle answers with the bytes its port had, and they are
read as they came.

The **keys** of a signed link are kept in the settings file, next to the connection
profiles, and the file is meant to be edited by hand: every key has a name and its
secret in hexadecimal. The link is signed with the first key that can be read, and a
key whose secret is not hexadecimal is left out and said so, because a link signed with
the wrong key does not work at all. The sheet that shows them is opened from Setup, and it
says which names the keyring could not read and which one signs; the reading of a secret is
done once, by the keyring, so the interface cannot disagree with the link about which key is
the key — a sheet that decided for itself would show a link signed with a key it is not
signed with.

The **link** can be signed, with the keys of MAVLink 2: every frame that goes out is
signed, and every frame that comes in is verified against the keys that are known. A
frame that arrives without a good signature is not from the vehicle.

The **diagnostics** of the application are gathered in one file, from the Help menu: the
machine it runs on, the connection profiles that are saved and the state of the link.
When the application falls over, the reason is written down where it can be found
afterwards.

The **Setup menu** carries the screens of the configuration, the way upstream's does:
the frame of the vehicle, the receivers, the serial ports, the radio, the failsafe and
the flight modes. Each of them is the sheet of the parameters of that subject, and the
parameters that are a choice are drawn as the list of what they can be — the mode of a
slot, the protocol of a port, the action of a failsafe.

The **bar of the connection** carries the way of connecting, the port and the rate. The
rate can be left to the program: a vehicle that is talking at a rate nobody wrote down
is found by trying them one after another, and the rate at which the vehicle answers is
the one it talks at.

The **calibrations** are a sheet of their own, opened from the Setup menu, the way
upstream has a screen for the accelerometer and another for the compass. The order is
sent to the vehicle and the vehicle counts the positions itself: the accelerometer in its
six positions, the compass with the vehicle away from the metal of the bench. The
compensation of the motors is a parameter, and the sheet takes the operator to it.

The **on-screen display** is a sheet of its own too, opened from the Setup menu: a
vehicle draws a display over the picture of its camera, and what it draws is a set of
slots. The screens five and six are the ones the operator chooses, and the vehicle is
asked what each of them holds. A slot is given a parameter, and the range of that
parameter is said with it, so that the vehicle knows how to draw it. The two messages
that carry this are the ones upstream uses as well.

The **corrections of a base station** are a sheet of their own, opened from the Setup
menu: a base station watches the satellites it can see and works out how far each of
them is, and it sends those corrections to a caster. The caster passes them to whoever
asks, and the way of asking is NTRIP: an HTTP request that is answered with the stream
of the corrections. The operator gives the address of the caster and the port of the
GPS of the vehicle, and the corrections are given to it. The position of the vehicle
travels with the request, as a GGA sentence, so that the caster knows which corrections
are wanted.

The **nodes of a DroneCAN bus** are a sheet of their own, opened from the Setup menu: a vehicle
with a CAN bus has devices on it that are not the autopilot, and the bus says what they are — how
long each one has been up and how it is, and, when the station asks, what it says about itself: its
name, its hardware and software versions, and the number its build carries. The two messages are read
from the bytes rather than through the dialect, and not because the dialect does not carry them: it
does, but it turns their health and mode into enumerations and refuses a number the dictionary does
not give, so a node that reported `health = 5` would never reach the sheet — the generator answers
`Invalid enum value for enum type "UavcanNodeHealth", got 5`. Upstream reads the same payload as
bytes, and the order used here is the order the seed is worked out over: the rule gives 28 for the
status and 95 for the information, and those are the numbers pymavlink's own generator publishes, so
the order is a fact rather than a guess. The words the sheet shows are the names the dictionary gives
the numbers, checked against the enumerations the generator builds rather than written twice, and a
number the dictionary does not give is shown as the number rather than as a word invented for it.

The **ask** goes to the pair the link chose for the vehicle, and nothing is waited for, because the
dictionary says these answers can be lost and that a station notices by seeing a status with no
matching information. The command is the one the published dictionary reserves for UAVCAN (5200), and
it travels as it was written, through the raw path, because the dictionary also marks it superseded —
by `MAV_CMD_REQUEST_MESSAGE`, since 2026-05 — so the code cannot name it without turning the
dictionary's own note into a warning; the test reads the name from the dictionary instead of writing
it down. When the ask goes out is a decision about what the interface shows, so it lives in the
application: once when the link opens, once more for a node that reported how it is without saying
what it is, and then stop, because a bus that will not answer is not answered by asking again. The
count belongs to the link it was made on, since a link that was restored after dropping is a new bus.
What the sheet shows is what the bus said and what the station asked, not what was last clicked, and
the counts are written by the formatting module, as they are everywhere else.

What the scripted peer can answer is what is asserted here: two nodes report themselves, one answers
with what it is and the other does not, so the second reading asks and the third puts nothing on the
wire, and the count starts again on a second link. What cannot be asserted is said rather than
assumed: there is no physical CAN bus on this machine, so nothing here proves that a real node
answers, and the row stays partial. The sheet is measured at 1920x1080 and at 1024x640 in Spanish.

La **placa Bluetooth** se alcanza por el camino que no pide un runtime: un canal serie de
Bluetooth es una puerto COM, y los bytes los mueve el mismo código que los mueve para
cualquier otra puerto, porque la pertenencia no cambia lo que viaja. Lo que decide qué fila
pertenece a Bluetooth son tres hechos, cada uno con una fuente publicada que no es este
archivo: `HKLM\HARDWARE\DEVICEMAP\SERIALCOMM` es lo que un lector lee para listar puertos —
el `SerialPort` de Microsoft lo abre (`System/sys/system/IO/ports/SerialPort.cs`) —, así que una
fila es siempre el nombre del dispositivo y el puerto que contesta; la pertenencia viene del
**prefijo** que el servicio RFCOMM escribe en ese mapa y del servicio que la instalación ligó a
la instancia (`BTHMODEM`, como las listas publicadas lo escriben), y no de la clase, porque una
fila serie no dice la clase Bluetooth: dice la clase Ports
(`{4d36e978-e325-11ce-bfc1-08002be10318}`), y una fila de manos libres dice la clase audio
(`{4d36e97d-e325-11ce-bfc1-08002be10318}`) con otro servicio detrás (`BthHFEnum`), así que una
regla que decidiera por la clase no diría nada sobre los puertos que le preguntaron; y el
perfil que lleva la corriente es `00001101-…` (zephyr lo nombra en su propia ayuda de SPP),
mientras las filas de esta máquina contestan perfiles de audio, AVRCP y manos libres, que no
llevan corriente. SuperCollider muestra por qué el prefijo completo es la regla: su parser casa
cualquier nombre de puerto, así que un par virtual nombrado `com0comBluetooth0` sería leído como
fila de Bluetooth por una comparación más floja, y aquí se rechaza. Un grupo de ceros no nombra
nada: es lo que una instalación deja donde nada se escribió, no un valor que la máquina decidió,
y esta máquina muestra las dos mitades de una misma fila a la vez — `LOCALMFG&0000` junto a doce
ceros para la fila que da COM41 —, así que la lectura rechaza en lugar de inventar una dirección,
con su propio código (`bluetooth_no_device_named`), que dice algo distinto de
`bluetooth_row_not_the_shape`: una cosa es una fila que la máquina presenta sin dispositivo
detrás y otra es un texto que el lector no puede leer; y las dos negativas caen antes de que nada
viaje por el cable, que es lo que el peer scriptado muestra, porque nada viajó antes de la
lectura.

El área viva (`crates/mpcraft-app/tests/bluetooth_live.rs`, detrás de `hardware-tests` y
`windows`) lee lo que la máquina contesta hoy, y cada estado se sostiene como dato en lugar de
afirmar un resultado esperado: el mapa contestó siete filas y dos llevan el prefijo del servicio
(`\Device\BthModem0` = COM40 para el módulo Matek, `\Device\BthModem1` = COM41), con los números
atribuidos a la llamada que los contestó a través del diccionario de esa llamada —
`RegOpenKeyExW` dio 0 para la clave y `RegEnumValueW` dio 259 (no hay más elementos) al final del
bucle, los dos por el diccionario Win32, que es el que este vínculo nombra para esas llamadas y
no para las `CM_*` —, así que un conteo no vale solo: lo que terminó la lista se conserva con él y
un paro por el propio límite (512) se dice como hueco. Para la instancia que el entorno nombra
(`MPCRAFT_BT_ROW`) la máquina escribe `ClassGUID` `{4d36e978-e325-11ce-bfc1-08002be10318}` y
`Service` `BTHMODEM`, que es el nombre que las listas publicadas escriben, y el nombre de clase
que `tdibth.inf` lee en esta máquina es el mismo que Microsoft usa como
`System.Devices.Aep.ProtocolId` en su propio ejemplo de enumeración: dos implementaciones que no
se hicieron la una desde la otra, así que el acuerdo entre los dos caminos es un hecho y no una
regla escrita dos veces. Bajo ese nombre la instancia publica cero filas, que es lo que se espera
cuando la instalación no escribió el valor que decide que una fila exista, y aquí se lee sin
elevación porque leer una clave de dispositivo no la pide; escribir esa clave sí la pide, y por eso
la puerta nombra esa escritura como lo que el operador pone.

Lo que no se puede afirmar aquí se dice en lugar de suponerse: ningún periférico emparejado en
esta máquina anuncia el servicio Nordic UART `6e400001-b5a3-f393-e0a9-e50e24dcca9e`, así que el
filtro de enumeración de upstream no encuentra nada para `Makeblock_LE001b1062c13c` (contesta
cuatro servicios y ninguno es ese), y la mitad GATT no está portada porque no se lleva un runtime
— el sandbox de plugins es el único lugar donde se permite llevar uno —, así que no hay escaneo,
no hay lista de periféricos y no hay característica que escribir o leer, y la regla del nombre que
upstream construye (`BLE_<identificador>_<dirección sin separadores>`) se conserva como dato: por
una puerto COM la máquina nombra el puerto, y esa regla solo dice lo que un lector con el camino
GATT habría escrito para el mismo periférico. Dos diferencias se conservan como diferencias:
upstream lleva una velocidad en su clase BLE donde su propio camino no tiene velocidad ninguna, y
su `toggleDTR()` está vacío porque esa clase no tiene línea; aquí una puerto COM sí la tiene, así
que `dtr` y `nodtr` significan lo que dicen, con el valor de la casa todavía bajo, y la velocidad
se conserva para el esquema `ble` como el número que se da al driver, que el medio no usa, mientras
`baud=auto` se rechaza (`SpecError::UnknownOption`) porque una búsqueda de velocidades es una
opción que promete lo que el medio no puede contestar. Lo que el peer scriptado puede responder es
lo que se afirma en `crates/mpcraft-app/tests/bluetooth_peer.rs`: `Ok(0)` al final del stream en
lugar de un error, la escritura que casa tres bytes a la vez y repite hasta completar el buffer, y
la lectura de la fila que sobrevive a un reconnect, porque la fila es lo que se le dijo al operador
sobre el puerto y un reintento no puede convertirla en otro hecho. Lo que no puede mostrar es lo
que una máquina escribe sobre sí misma, por eso la fila sigue parcial, y la puerta nombra ese
sustituto; el control lo lee con un transcripto donde la mitad scriptada pasó y el área que lee la
máquina falló, con las dos mitades leídas como sus propios informes, que es lo que hace que un
verde del peer no valga por un área que no leyó ninguna fila. Y nada se abre en COM40: esa puerto
pertenece a un módulo que puede estar usándola.

El mapa lista puertos, así que una periferica que solo habla GATT no tiene fila: esta máquina la escribe
bajo `BTHLE\Dev_001b1062c13c` — tres instancias de servicio, ninguna es el perfil serie — y no bajo el
enumerador del que salen las filas RFCOMM, por eso la regla rechaza esa ruta en lugar de inventar el
enumerador, y ninguna de las siete filas nombra esa dirección, porque una fila del mapa nunca lleva una
dirección: lleva el nombre del dispositivo y el puerto, y la mitad del dispositivo solo puede venir de la
ruta de la instancia. Lo que sí puede cerrar la fila es lo que la máquina escribiría para un dispositivo que
ofrece el perfil Serial Port: el soporte está aquí (`mdmbtmdm.inf`, servicio `BTHMODEM`) y los perfiles que
las filas contestan son `1101`, `110B`, `110E` y `111E`, pero solo `1101` lleva corriente, y ese es un perfil
clásico, así que nRF Connect no puede escribir esa fila — es una herramienta GATT — mientras una app de
terminal serie sobre el teléfono sí podría, porque eso es lo que hace que Android publique el perfil por SDP.
Con esa fila escrita el área lee lo mismo que lee hoy, pero por una instancia que la máquina escribió para un
dispositivo que decidió emparejar, y los números siguen atribuidos a la llamada que los contestó; lo que
falta entonces es abrir la puerto, que es lo que el operador dice, porque esa puerto pertenece a un
dispositivo que puede estar usándola, y sin abrir nada los bytes cruzarían por un peer escrito para responder
en lugar de por un canal.

La lectura que nombra los servicios de un periférico emparejado vive dentro del subkey `Properties` de su
instancia, y ese subkey contesta 5 por el diccionario Win32 de `RegOpenKeyExW` — el mismo 5 que el camino de
abrir llama `HANDLE_HELD`, porque el diccionario pertenece a la llamada y no al número — mientras las claves de
la instancia se leen sin elevación: la ruta que la máquina escribe para
`BTHLE\Dev_001b1062c13c\b&548bea1&0&001b1062c13c` dice la clase Bluetooth (`{e0cbf06c-cd8b-4647-bb8a-263b43f0f974}`)
con el servicio `BthLEEnum`, que no es el de una fila serie, así que esta instancia no es un canal por el que
viajan bytes aunque diga el nombre de Bluetooth. Las dos rutas son dos lecturas y se mantienen aparte: la carpeta
que la clave del dispositivo llama por su nombre contesta 2 (no hay nada ahí), que es otra cosa que la negativa,
y por eso cada número se nombra junto a la llamada que lo contestó. Lo que cerraría la fila es una lectura
elevada de esa clave, o el permiso del operador para abrir la puerto de un dispositivo que ofrece el perfil
serie, y ninguna de las dos depende de este archivo.

The **joystick** is a sheet too, opened from the Flight menu: the browser knows where
every axis of the stick is, the operator says which axis drives which channel of the
radio, and the channels are sent to the vehicle twenty times a second, the way a
transmitter sends them. The radio is let go of when the operator says so.

The **track of the flight** can be written to a file while the vehicle flies, from the
File menu: the file is a KML document, the one Google Earth reads, and it is written
again and again with the path so far. The track of a log is written the same way, and
the two are the same document.

The **engine** of a vehicle that has one is read out in the tab of the state: the
revolutions, the load, the fuel that flows and the fuel that has been burnt, the
pressures and the temperatures.

The **logs** are a screen of their own, opened from the switcher or from the View menu. A log is what a flight
is remembered by: the stream of the frames that crossed the link, each of them with the
instant it was seen. The screen says what the log holds — how many records, how long it
lasts and how many of each kind of message — and draws the graphs of the catalogue over
it. The graphs are the same ones the live view draws: the log is fed to the same
aggregator the vehicle feeds, and the samples are the same samples.

The **binary log of DataFlash** is read as well: a marker of two bytes, the number of
the message and its values, with the declarations in the log as well. The values are
read with the types they were declared with, and the scaled ones are said in the unit
they are in.

The reader of the binary log has been checked against bytes that came from a card: 8 280
bytes of the head were brought over the transfer protocol, which is what a parser needs,
because the declarations of DataFlash are at the top of the file. The same bytes were read
by pymavlink's own reader and by a walker written from the rules, and the two agreed on 299
fields — and what that comparison found was the map of types. It did not have `c` and `C`,
a half word and an unsigned half word said in hundredths, so those characters were skipped
and the values of a message that carries them were read from the wrong place: `XKF1` came
back with its roll where its instance should be. And `e` was read as four bytes but counted
as two, which shifts everything after it. The map is now the published table the logger
uses, one kind per field, and a character the table does not give keeps its place and is
named by the declaration rather than shifting the values that come after it; a message with
such a field cannot be read at all, because its field has no size, and is passed over. The
head is cut in the middle of its sixty-fourth declaration, which is what a partial fetch
gives: sixty-three declarations and sixty-three messages are read, and the place where the
bytes that could not be read begin is said — 8 209. A count that leaves a gap out is a count
that says the log is whole when it is not, so the number of records that could not be read is
filled from the reader instead of being written as zero, and the screen of the logs says it in
its summary line: the counts there are written with the separator of the language the window is
read in, because a count read as a word is not the same number to somebody who reads the other
language, and the sentence about what could not be read only appears when there is something to
say — a zero says nothing was dropped. What cannot be measured at the two sizes is the line with
a count on it, because the browser run has no backend to open a log; the screen was measured as
it stands without one, and the sentence is checked against the catalogue.

A declaration whose types do not fill its own length is the same kind of gap: the bytes
that would be read as values of that message belong to the next one, so the message is passed
over and what disagrees is written down rather than read as if it made sense.

The reader of the **text** log has the same rule, and it did not have it: a value that is not
the type its declaration says it is was turned into zero, which is a number a vehicle could
have said — a `Hdop` field declared as an unsigned word and written as `-1` would be drawn as
zero on the grid and plotted on the graph, and nothing on the screen would say that the reader
invented it. Such a value now keeps what the line wrote, as it is written, and the place is
named with the line, the message, the field and the text; the list is bounded at fifty places
and says how many it left out, which is the rule the captured board output follows. The count in
the summary is filled from this reader too, so the two readers say the same kind of thing. What
cannot be measured at the two sizes is a log open on the screen, because the browser run has no
backend to open one; the rules that keep the text and count the places are tested against lines
written down before the function was called.

The places the reader names travel with the count, because a number the interface knows and does not show
is a number the operator cannot look at: the contract carries the line, the message, the field and what the
line said, up to the fifty the reader keeps, and the screen draws them when there are any, with what the
bounded list left out said as its own number — a list that stops is not a list that is whole. In the binary
form and in the PX4 one there is nothing to name, because a field whose type the published table does not
give has no size, so the whole message cannot be read rather than one value of it: those counts arrive
without a list, and the code says why rather than sending an empty list as if the file had named none. What
cannot be measured at the two sizes is the list with rows on it, because the browser run has no backend to
open a log; the screen was measured as it stands without one and the words are checked against the
catalogue. The count of each message on the screen is written with the separator of the language the window
is read in, which is the rule the formatting module carries — before this it was drawn as the number itself,
which says one number to somebody who reads the other language.

The block was asked for without letting the channel go quiet, which is what the same rule
about an open file says when it is followed: fifteen seconds between the open and the read
was answered with the code for the end even though the file is 36 MB, because the vehicle
closes an open file after three seconds with nothing sent on its channel, while the same
read asked straight away answered with the block.

The **text log of DataFlash** is read as well as the telemetry log: the declarations
at the top of the file say what every message is, and the samples that follow are read
with the types they were declared with — a position in millionths of a degree is said in
degrees, and a value in hundredths is said in the unit it is in.

The **log of PX4** is read too, which upstream ships without ever wiring it to
anything. The file begins with a header, and what follows is a stream of messages: the
declarations of the topics, the information of the vehicle, its parameters, the strings
it printed, the places where messages were lost, and the samples of each topic. The
declaration of a topic says the name of the topic, its fields and the type of each of
them, and the samples that follow are read with those types. A topic that uses another
topic as the type of one of its fields is spread over the columns of both, and a list
of values is one column per value. The log does not say what a whole number means, so
the reader knows the convention of PX4 and gives the values in the unit they belong to:
a latitude in hundred-millionths of a degree is said in degrees, and a height in
millimetres is said in metres. A log that ends in the middle of a message is not
thrown away: what came before is kept and the place where the log is broken is said.

The **samples of a message** are a grid in the screen of the logs: the fields of the
message are the columns and the samples are the rows, with the instant each of them
happened at. Clicking a row moves the flight to that moment, so the grid, the map and
the play-back are the same thing seen from three sides. The **graph** is the fourth
side of it: a log that is being played back is what the graph draws, and the line that
says where the play-head is is the moment the grid and the map are showing too.

The grid **names the columns whose type the published table does not give**, so that a value kept
as the text of the file does not look like a number the reader read badly. The names come from the
declaration, not from the sample, and they are said as data — the field is called what the file
calls it — while the sentence that says it is a word of the interface. In the binary form the same
field makes the whole message unreadable, because a field with no type has no size, and there the
screen says why rather than reporting that the message is not there: the log does declare it, and
what is missing is not the message but the size of one of its fields. What cannot be measured at
the two sizes is a grid with rows on it, because the browser run has no backend to open a log; the
screen was measured as it stands without one, and both sentences are checked against the catalogue.

The line the map draws for a log is the whole path the file holds, not the part that has
been played: a moment at two minutes has to be a moment on a line that is there, and the
map knows the flight only through the store the screen reads. It is drawn in the colour of
what is known from a file rather than from the wire, because during a play-back the track
and the path are two things — one is what has happened so far and the other is the flight.
And a path is kept with the file it was read from, so a line of another flight is not drawn
under the log that is open.

The **flight can be let go of**. While a log is being played back the link does not take the
window, because the vehicle shown is the one in the file; before there was a control for
stopping it, an operator who opened a log could not get the live vehicle back without closing
the program. Letting go of the flight stops the play-back and the map goes back to the wire,
while the grid and the graph keep the log they read, because the screen does not forget what
was opened on it. What cannot be measured at the two sizes is the row that button sits on:
the scrub only exists once a log is open, and the browser run has no backend to open one.

The **tiles of the map** can be fetched before the flight and kept on the disk: a survey
is flown where there is no signal, and the map of the field is drawn with the pictures
that are there. The map's button says "Save the area", and the tiles of the region and
of the levels of zoom below the present one are fetched. The map does not ask the
service for the tiles: it asks the program, which takes them from the folder the
operator has brought, from the disk where the ones that were fetched are kept, and from
the service when it is in neither. A map that is drawn without a signal is a map that is
drawn with the pictures that are on the disk.

The **traffic of the air** is drawn on the map: a vehicle with a receiver for it sends
what it hears, and the aircraft around it are marked with the name they say. The traffic
is kept while it is heard, and an aircraft that is not heard any more is forgotten.

The station can also read the traffic from a source of its own, and what it reads there is
put onto the link, because the other stations listening to this link would not see it
otherwise. Once a second the relay takes the aircraft that came from its own source — not
the ones the vehicle sent, which are already on the link — keeps the ten closest to the
vehicle, and sends one of them, going through the list in order rather than sending the same
one again. The list is asked for every ten seconds, which is what the service that publishes
the traffic asks of its readers, and the client says who is asking, because a client that
says nothing about itself is refused. Upstream does the same (`MainV2.ADSBRunner`), with the
same rule of the ten closest within ten kilometres and one second apart. What upstream also
does — opening a plain connection to a receiver and decoding the messages itself — is not
ported here: an address that is not a URL is refused rather than guessed at. The sheet of the
traffic, opened from Setup, holds the address of the source and how far round the vehicle the
list is asked for, and says what the relay is doing.

The **transponder** of the vehicle is a sheet of its own: the vehicle is told where it
is and what to say, and it relays the message, so that the aircraft around it see it.
The operator gives the address of the aircraft, the name of the flight and the code.

The **video streams** of the camera are described in a sheet of their own: the camera
says the URI, the resolution and the frame rate of each stream, and the operator can
start or stop it. The stream travels outside MAVLink, so the URI is what a separate video
player connects to.

What is drawn inside MPCraft is what a picture element can render — an MJPEG stream or a single
picture over `http` or `https` — and what speaks another way is said as one that cannot be drawn
rather than tried, because there is no media runtime here. Upstream draws through GStreamer, which
carries `rtsp`, and finds cameras by service discovery on the local network; neither is ported, so
the row stays partial. And the picture is fetched only when the operator says so, because the
application makes no network call that nobody chose, while upstream draws as soon as a stream is
picked. The sheet opens without a link for the same reason: an address typed by hand needs no camera
behind it, and asking the camera still needs one, which the button there says.

A place is read against the picture that is drawn, not against the box it is drawn in, and a click
that falls in the band the letterboxing leaves over is refused rather than read as the nearest edge —
upstream clamps (`GimbalVideoControl.getMousePosition`, line 684), so a click on the band there sends
a place the operator did not mean. Upstream's divisor is also wrong on an axis that has a band: for a
box of 640 × 480 and a picture of 1280 × 720 the picture is drawn at 640 × 360, and a click at its
middle comes back as 0.333 rather than 0, which is asserted against numbers written down before the
rule was called. For an address typed by hand the place is shown as the click says it and nothing is
sent, because no device was described behind it, and the size the camera said is kept apart from the
size that arrived: for a stream they are not the same thing, and both are shown when they differ.

The picture the flow is read against is a fixture written by a second implementation of the format
(`scripts/write-test-picture.js`), which builds the chunks by hand and works the check out with the
rule the format gives rather than taking bytes from a decoder. That comparison caught this writer
starting its run at zero without the final inversion, which is not the rule: every chunk it wrote
disagreed with a PNG written by another tool, and the numbers the two readers agree on — `401f4a01`
for the declaration, `5f2ed914` for the pixels, `ae426082` for the end — are recorded as what an
independent reader gives. A real camera cannot be asserted here, so what the drawing proves is the
mapping and the refusal, read against a stream of one frame served for the run; the sheet is measured
at 1920x1080 and at 1024x640 in Spanish.

The **pictures** of a flight are read from a folder, from Setup: a picture carries the place it
was taken inside itself, and the instant its own camera says it was made. The clock of a camera
is not the clock of the ground station, and the difference between the two is worked out from the
pictures at both ends of a folder — the first three and the last three, each comparison given half
of what was worked out before it, which is what upstream does in `EstimateOffset`. A picture whose
instant is found in the log, within the two seconds upstream allows, is given the place the log
says the vehicle was at that instant; a picture that is not keeps only the place its own file
carries, and the two are shown next to each other rather than merged, because the ground station
knows one from the file and worked the other out from the flight. The places are drawn on the map
the same way, in two colours, for the same reason. The copies are written into a folder of their
own with the place they came from put into them, and a copy can be read back through the same
sheet: what it then says about itself is the place the flight said. The sheet needs no link,
because a folder of pictures and a log of a flight that has already happened are enough, so it is
the entry of the Setup menu that stays open without a vehicle on the wire.

The **camera and the gimbal** are a sheet of their own too, opened from Setup. What a
device says about itself decides what can be asked of it: the capabilities come from the
vehicle, and an angle the device cannot reach is refused before anything is sent, rather
than sending it and waiting for the vehicle to say it cannot — a gimbal with no pitch axis
cannot be pitched, and a camera that cannot be zoomed cannot be zoomed. What a camera can
capture is the capability and the mode together, which is why the mode is asked for as well
as the flags: a camera that can do both cannot take a picture while it is recording, unless
it says it can. The gimbal is pointed at an angle, at a rate, at a place on the ground, or
at another vehicle; the yaw is read in the frame the device says it uses, and the point of
interest is held until it is taken off. A camera answers on its own component, so a command
for a camera goes to that component rather than to the autopilot. Both devices keep saying
what they are doing once a second, so the sheet shows what the vehicle says rather than what
was last asked for. A place to follow can be picked from the picture when a stream has been
described and drawn, and the place a click says is read against the picture rather than against the
box; for an address typed by hand the place is shown as the click says it and nothing is sent,
because there is no device behind it to be asked.

The **theme** of the interface is chosen in the View menu: the dark one, the light one
for the sun and the one of the contrast for the eyes that need it. The theme is a set of
variables and nothing else: no component knows which one is in use.

The **language** the window is read in is chosen in the same menu and kept for the next run.
A string is looked up by what it says in English, because that is the language the code is
written in, so a part that has not been translated yet still says what it said before rather
than showing a blank, and a gap is a change that can be reviewed. Each language is named in its
own words, so that the two can be told apart by somebody who cannot read either of them. English is
the language the code speaks, so it is what a run that has chosen nothing reads; another language is
only what the next run remembers. The bar of the menus, the switcher of the screens and the tabs of
the left column speak from the bundles; the sheets and the panels follow one by one, and a test says
which names have no entry. Every word the interface writes now goes through the bundles, and the
test says which ones do not, so a gap is a change that can be reviewed rather than a hole.

The gate reads two things. It reads the words that go through the catalogue, whether or not the
parts of the sentence are named with them; and it reads the words that reach the screen without
being looked up, on the two channels where what is written is what is drawn: the note the bar
leaves, the report an action makes, and the label or hint of the bar itself, because the bar draws
its items as it is given them. A label written in another file is not the same thing - the quick
view, the actions panel and the plan editor hold names that belong to the vehicle or to the file,
and those are data, not words. So the rule is only for the bar, which is the file that draws its
own labels. What the gate cannot see is said rather than pretended otherwise: a word written as
text inside a component, and a wrong translation. The gate is shown failing on a known-bad word
before its green is read as a pass.

The numbers that go into a sentence are made into strings by the formatting module, as they are
everywhere else, and the separator is the one the language the window is read in uses. English
groups every three digits and puts a point before the decimals; Spanish does the other way round,
and it only starts grouping when a number has five digits, so the same count is "1,234,567" in one
and "1.234.567" in the other, and a count of 1234 is written with no separator at all in Spanish.
A count always written with the separator of English says one number to somebody who reads the
other language, where "1.234.567" is a number with decimals.
What arrives as data stays as the data says it: the names of the modes, of the camera states,
of the gamepads, of the parameters and of the tile providers are what the vehicle or the file
says, and translating them would say something the vehicle does not say. What the library of the
map writes is outside the reach of the bundles: the titles of its own navigation control — zoom,
rotation — are the library's words, not ours.

The **identification of the vehicle** is in the tab of the state: what it is, its
number, what kind of vehicle it is, where it says it is and what the operator says about
it. The parts arrive in three different messages, and each one keeps what the others
said.

The **vehicle the station speaks to** is chosen by the link rather than by the interface,
because the boards that answered are what the link knows. The connection sheet lists them and
marks the one the station speaks to, and the mark is what the event says rather than what was
last clicked: a rule that picked the first row would show a vehicle as the one being read while
the telemetry in the same store came from another board. An autopilot is named by the family it
belongs to, because that says more than an address does; another board keeps the pair, which is
all that is known about it before it speaks. A pair that has never spoken is refused rather than
chosen — there is no state behind a number somebody typed — and the choice that stands is left
where it was. The choice stands for the current link: a new link runs the rule again, which is
what upstream does, and within a link the rule runs once, so the choice does not move because a
second board happened to talk more. What cannot be measured at the two sizes is a row of the list
with a board on it, because the browser run has no backend to hear a vehicle; the sheet was
measured with an empty list, and the rule the store reads is tested against names written down
before the function was called.

The **relays** of the vehicle are read and set from the tab of the outputs. The report carries two
masks — which relays the vehicle has and which of them are on — and they are read into a list at the
wire boundary, so the interface draws the relays the vehicle has without having to know that a relay
is a bit. The rows are those relays, and the state shown is what the vehicle said rather than what
was last clicked, because the vehicle can answer with a relay in another state than the one that was
asked for. A relay the vehicle has said it does not have is refused before anything is sent: that is
not something this board would do at another moment, it is something it cannot do. A vehicle that has
never reported its relays is asked anyway, since the report is not streamed unless the station asks
for it, and refusing on silence would refuse a board that can do it; the tab asks once, when the link
is open, because a station that is not connected asks nothing. One button upstream has is not ported:
Repeat, which sends off and then on again after a pause, is the same command twice and is done here
with the two buttons that say what they do. What cannot be measured at the two sizes is a row with a
relay on it, because the browser run has no backend to hear a vehicle; the tab was measured with the
note that says nothing has been said, and the rule that reads the masks is tested against numbers
written down before the function was called.

The **words the board prints** outside of MAVLink frames reach the interface now, which is where they
were missing: the link captured them and nothing read them except the command line. They travel as one
shape — the lines, how many had to be dropped, and the decision about what the text looks like —
because a window that showed only the lines it kept would look like the whole output. Upstream puts
these words inside the message that says the connection failed, so MPCraft carries them as data rather
than as prose that has to be read for meaning: the console draws what the open link is capturing, and
the connection sheet draws what an attempt that gave up left behind, which is where the failure is read.
The words are kept after the attempt that heard them is gone, for two reasons: the operator reads them
after the session closed, and the rate that matches the board is one of several, so an attempt that
heard nothing does not erase what an earlier rate heard. They are forgotten when a new link is opened,
because the words of an attempt that has since been replaced are not what the operator is looking at.
What cannot be measured at the two sizes is a console with board output on it, because the browser run
has no backend to hear a board; the messages tab was measured as it stands without a link, and the rules
that keep the words and clear them are tested against scripted transports.

The **generic executor** has an entry point now: the actions tab chooses the command from the
catalogue, which is what says what each of the seven slots means, and reads what the vehicle answered
afterwards. Upstream reaches the same executor through its actions dropdown — `CMB_action`, bound to a
list of names written out by hand (`FlightData.cs` line 353), with the parameters decided name by name
at line 1873 — and a list of names is a thing that can fall out of step with the dialect. MPCraft has no
such list: the interface asks the backend for the table, so the meaning of a slot is not written twice,
and a value the table does not describe is refused before anything is sent, because the interface cannot
say what its slots mean. That is a difference rather than a gap: upstream sends such a command anyway,
and the bootloader command reaches the wire through the sheet that builds the frame byte by byte.

What is read after sending is what the vehicle reported, named the way the protocol names it, and a
refusal is kept apart from a board that never answered — upstream's `doCommand` answers with a bool, so
the operator cannot tell the two apart. The slots travel as the float form carries them: the integer
form, which is where a latitude and a longitude keep their precision, belongs to the plan editor (4.2),
and this entry point does not rescale a number the operator typed. What cannot be measured at the two
sizes is a row with a command chosen on it, or an answer from a vehicle, because the browser run has no
backend to hear one; the actions tab was measured as it stands without a link, and the rules — that the
wire carries what was typed, that the answer travels as data, and that an undescribed value is refused
before any wire is touched — are tested against scripted transports.

The transponder, the generator telemetry and the folder of the operator's own map are
available elsewhere in the application: in Setup, the vehicle state and the tile
settings respectively. The video sheet describes and controls the stream and draws what a
picture element can render, which is an MJPEG stream or a single picture over `http` or `https`;
what speaks another way is said as one that cannot be drawn, because no media runtime is carried.

The **generators of a survey** are three: the circle, the reticle of a field — the
strips the vehicle flies one after the other, apart by the distance its sensor sweeps —
and the smooth path, the curve the vehicle follows when it is told to follow a spline.
The plan draws the path as a straight line from one place to the next, and the smooth
one when the operator asks for it.

The **station asks for what its screens read**, because a vehicle sends by the rates configured on
the port it is talking on, and against a live simulator every one of those rates is zero: before the
ask the run counted six packets, every one of them a heartbeat, and after it a hundred and twenty-one.
The policy of what to ask lives in `mpcraft-app` as the table with its rates, and the wire helper stays
in `mpcraft-link`, because a table of rates is a decision about what the interface shows rather than a
rule of the protocol; the station asks once when the link opens, which is the pattern the tab of the
outputs already had for the relays. The rates follow upstream's categories — attitude at ten hertz,
position at four, sensors at two, state at one — and the wire carries microseconds between two sends,
where zero means stop. A refusal is kept per message rather than stopping the ask at the first, and is
kept apart from silence: a board that refuses one message is not a board that refuses all of them, and
the operator has to see which screens will stay empty. The number of the command is 511 rather than 52,
which is the number of the command this ask replaced, and the dialect is checked against the dictionary
rather than trusted. What the table reads for the EKF status is not the common dictionary but the ArduPilot
one — the common dialect does not name that message at all, and the ArduPilot one gives 193, which is the
number both sources agree on without being built from each other, because this project compiles the ArduPilot
dialect. A comment that said the compiled dialect did not carry it was wrong and is corrected: the aggregator
matches that message, and the live area counts a row through the raw path so the count does not depend on what
the bindings can decode.

The **home** is in the table because the map draws it and the distance bar reads it, and a live vehicle
sends it only once the station asks: before the ask the run had no home to draw, and after it the board
answered with acceptance and put the home on the wire at the rate it was given. What the console reads is
not in the table, because the words the board prints arrive as they are said rather than on a rate — and an
ask for that message was answered `MAV_RESULT_DENIED` in one run and accepted in another, so nothing here
depends on which of the two this build chooses.

The **table is read row by row** against a live board, and the two halves agree: every row was answered
`MAV_RESULT_ACCEPTED`, and every row the board answered put traffic on the wire inside its window — thirty frames
of `ATTITUDE` in three seconds against three of `SYS_STATUS`, which is the ten hertz and the one hertz the table
asks for. One row stayed empty: `GPS2_RAW`. The board answered the ask and then said nothing, because it has one
receiver — that is what the vehicle said about itself, and it is written down rather than asserted as a failure,
because a row can be accepted and still keep its screen empty. The counting reads the raw path rather than the
decoded one for this reason: `EKF_STATUS_REPORT` is a message the compiled dialect does not carry, so a collector
would never see it arrive. And the gate names each live area by its own report, because two binaries answer it
now and one that passed cannot stand for the other; each of them is read against an instance of its own, since
one instance serves one connection and stops when that connection goes away.

The **board** on the other end of a port says things a simulator cannot say. It names its model in the text it
prints when it starts — `Detected Pixhawk` — and the identifier it reports cannot name a model: vendor `4617`
and product `22337` are `0x1209` and `0x5741`, the pair ArduPilot uses as the default for non-composite boards,
so they say the family and nothing else, which is why the detection reads the startup text rather than the
number. The capture keeps that text as it was captured, with the frames taken out by the rule the sniffer uses,
and the expected name is the one the hardware list the firmware manifest follows gives this board rather than the
table in the module being checked. The board also chooses an address for itself: it introduces itself as `2:1`,
and its own parameter says `MAV_SYSID = 2`, so the key the station builds can be compared with what the vehicle
holds for itself instead of with the address a station would assume. What it says about what it does not have is
the same kind of fact — the airspeed sensor it did not find, the card that failed to start, and therefore no
logs. While a link answers as a vehicle its words come inside the frames, as `STATUSTEXT`, so the capture of the
text outside them stays empty on a running link; that is what the run recorded, and the gate names the peer that
answered it, because with no port named a scripted board stands in and a script has no model to name and no
address of its own.

A board with no logs answers the list with **one entry that says there are none**, which is the vehicle saying
something about itself rather than a log: the reader used to turn that answer into a flight with no records, and
upstream returns the empty list for exactly this answer (`num_logs == 0 && last_log_num == 0`). The rule now
matches, and the area that reads a real board asserts it against the answer the board gave.

The log protocol **reads names rather than folders**, which is what makes two channels say different things about
one place: this card holds a `.bin` in its log folder while the list answers with the entry that says there are
none, because an entry counts as a log only when its name ends in the extension the logger compares — case
sensitive, against `.BIN` — and begins with a number within the maximum the logger allows, which is five hundred on
a board and five thousand in the simulator. The name on this card is a date, so the folder says one thing and the
log protocol another, and both are kept as data rather than reconciled by hand. What cannot be asserted is said
rather than assumed: the transfer protocol had stopped answering after a request for the whole file put this board's
IO thread into an internal error, and it answered again after the board was restarted, so the head of that file was
brought to the disk of the computer and read — which is what closed the gap this paragraph used to say could not be
closed.

The catalogue carries `SYSID_THISMAV`, the name upstream's metadata still holds, while this build of ArduPilot
reports `MAV_SYSID`. A parameter the catalogue does not know still arrives as the vehicle said it, and the gap is
written down rather than closed by hand, because the catalogue is generated from upstream's files: a hand-written
entry would be a catalogue that disagrees with the tool that produces it.

The **station asks for the capabilities** too, because they decide which encoding the items of a plan
travel in. Upstream asks for the autopilot version when the link opens, through the command that
requests a message with the number 148 as its first slot; MPCraft decoded that report without ever
asking for it, so against a live vehicle the plan path used the legacy float form and lost the precision
of a place the operator typed. The plan editor now asks once when the link is open, the way the tab of
the outputs asks for the relays, and what it uses afterwards is the state the answer filled rather than
the answer of the call. The two waits are kept apart — the acknowledgement first and the report
afterwards — so a board that says it cannot do this is not reported as a board that never answered, and
the matcher for the report is registered before the ask goes out, because a vehicle answers the two in
the same breath and an answer that arrives before anybody is listening is lost.

The **mode change** carries the flag that makes the custom mode be read. The dictionary says the second
parameter of `MAV_CMD_DO_SET_MODE` is used only when the custom-mode flag is in the first, so a command that
sends a mode without it is a command whose mode the vehicle is told to ignore — and a live board answered one
with `MAV_RESULT_DENIED` before the line existed. The helper sets the flag whatever the caller passes, and the
test asserts the flag on the wire rather than the number the flag makes the vehicle ignore; the live run then
reads the mode back from the heartbeat, which is what the vehicle says rather than what was asked for.

A **plan goes up and comes back item by item**, and the live run is what recorded two things about the
trip. Sequence 0 is not one of the items: the specification says the first slot of a plan carries the
home position instead of the first item, and whatever was uploaded there — a takeoff — is replaced on the
read-back by the home, in the absolute frame and measured above mean sea level, so the round trip compares
the items from sequence 1 onwards and asserts sequence 0 as the home, where the vehicle is. And before
the station asked for the capabilities the places came back rounded to single precision, which is the
float form losing a decimal rather than the wire losing a value; on the integer path the equality that
holds is the exact scaled one; and the home sequence 0 is asserted against is the one the ask brings in,
because a simulator that has not been asked has no home to say. What the live run cannot assert is said
rather than assumed: the disconnect when the simulator is stopped, because a test that connects to a peer
cannot stop that peer. The battery and the home are read after the ask rather than before it, which is what
the run prints: four packets before, every one of them a heartbeat, and sixty-nine after.

The **arming** is read as the vehicle's answer rather than as a success this machine decides. The number the
dictionary gives the command is 400; an earlier probe sent 40001 and was answered `MAV_RESULT_UNSUPPORTED`,
which is a different thing from a refusal and would have made the area assert a fact about a command the board
does not know. A healthy simulator started with Mission Planner's defaults leaves `ARMING_CHECK` at 1 — the
full checks, not none — and refused for a reason of its own, printing it on the wire ("Arm: Gyros
inconsistent", "Arm: Need Position Estimate"). Taking the check away does not make the vehicle arm at once: the
run refused twenty times and accepted on the twenty-first, because the estimate the check reads is still the
vehicle's decision, so the area polls rather than asserting a duration. What it asserts is that the answer
arrives as the vehicle's own — acceptance, or a refusal with the reason the board gave, kept apart from a board
that never answered — that the write is echoed, that a vehicle which accepts says it is armed in its heartbeat,
that it answers the disarming, and that the parameter is put back as it was found. One of the checks cannot be
fed at all: these binaries bind only the telemetry port of each instance (5760 + 10 × instance) and not the RC
input port (5501 + 10 × instance), so nothing travels through the radio.

What the **executor** reads back is the vehicle's result, and the live run recorded which commands answer which
way. `MAV_CMD_DO_SET_HOME` (179) asked for the home the vehicle already holds is answered with acceptance by a
disarmed board, so that answer is the executor's rather than a state the board happens to be in.
`MAV_CMD_DO_CHANGE_SPEED` (178) was refused with `MAV_RESULT_FAILED` in every state it was tried in — disarmed
and armed both answered the same way — so the area asserts how the refusal is read, kept apart from a board
that never answered, rather than asserting an acceptance this build does not give.

The **formation** is ported as far as this machine can verify it: the slots — which vehicle sits
where relative to the one it follows, in metres across and along the direction of travel and up —
and the places the slots put the vehicles, turned by the heading of the leader with the sign
upstream uses (`heading = -yaw`, so `y` stays ahead and `x` to the right however the leader is
pointed). The **decision** of the closed loop is ported as pure computation in `mpcraft-vehicle`:
the input-filtered PID in ArduPilot's shape, the point ahead of the target and the trailer behind
it, the bearing-stability rules, the turn worked out by the sine rule, the bank from the speed over
the ground, the thrust demand between its floor of 0.1 and its ceiling, the yaw band of three
degrees for the vehicles that are not planes, the flip that flies a plane in from behind when it
is facing the wrong way round the target, and the quaternion the plane's answer carries — built
through the 3-1-2 matrix as upstream builds it, with the mask `0xff` less its three terms. The
answer also **reaches the wire**: the fixture carries a `SET_ATTITUDE_TARGET` frame with the
quaternion the hand works out for a right angle of roll, the two implementations write the same
bytes for it, and the session sends what the decision produced to a simulated vehicle. What was not
there at that point is the loop described below: what still cannot be proved here is that the vehicles
fly into their slots, so the row stays partial.

The decision needs the state **of each vehicle**, and the aggregator did not hold it: it held one
snapshot, so a second board's position read as the primary's. It now keeps one state per sender,
keyed by the pair that identifies it, and the snapshot the interface reads is the primary vehicle's.
Before a vehicle has been selected there is nothing to tell the reports apart by, so the packets read
into a mixed view — which is what a link with one vehicle shows, and what the tests kept showing;
after the selection the view belongs to the primary alone. The selection is made **once**: a second
vehicle can satisfy the rule as well as the first, and upstream does not let the choice move
(`MAVList.GetID`), which is why the first vehicle that qualifies keeps it. Before this change the
choice could move on a later heartbeat, and the view switched to the vehicle that talked more — the
two-vehicle test over a real socket is what caught it.

The **dispatch** is the loop itself: ten times a second, for each member, read from that vehicle's
own state rather than from the primary's, and what the decision produced put on the wire. It belongs
to the application because it is policy — who leads, who sits where, and how often the loop runs —
while the decision stays pure computation in `mpcraft_vehicle::formation` and the wire shape stays in
`mpcraft_link::commands`. The branch is the vehicle's kind, once its heartbeat has been seen: a plane
gets the quaternion answer, the others get the position target, and the heading command only when the
difference is past the three-degree band. Nothing is waited for, because at ten hertz there is no time
to wait for an acknowledgement and the protocol does not give one for the position target either. The
vehicle the settings name as the leader is never told anything, which is what upstream's `SendCommand`
does with its own vehicle.

What the loop proves over a real socket — four boards played on it, the leader, a plane in its slot,
and two multicopters, one past the band and one inside it — is that what reaches the wire is what the
hand computed. The plane's answer is the identity quaternion, because it already points where its slot
puts it, with the thrust demand 0.606 and the mask `0x38`. The multicopter past the band gets forty
metres of grid north as forty divided by the scale factor of the projection, which on the central
meridian is 0.9996, the leader's speeds passed through rather than derived from its own, the frame as
the number upstream sends (6) rather than as its modern synonym, and a mask built from the names the
dialect has: 64 + 128 + 256 + 1024 + 2048 = 3520, which is not the number upstream's
`ushort.MaxValue` arithmetic leaves behind — four of the bits it sets have no names today, and
ArduPilot ignores them. One `COMMAND_LONG` goes out, for that vehicle alone; nothing for the vehicle
inside the band, and nothing for the leader. What the loop cannot prove without a live fleet is that
the vehicles fly into their slots, so the row stays partial — and a fleet cannot be assembled on this machine,
which is said rather than assumed: these builds of the simulator refuse a device string for UDP, and `--slave`
puts the extra airframes on JSON control ports (`9012`, `9022`, `9032`) rather than on the link, so one wire
cannot see two vehicles. The scripted peer stands in for the dispatch, which covers the arithmetic and not the
flying.

The **places** an operator wrote and the **decision** to run them are kept in the settings file, and
they are kept apart: a fleet is set up once and flown many times, and writing slots for a fleet is not
sending orders to it. A file with places and no decision is a formation that is described and not run,
which is the state a fresh installation is in. The settings crate does not know what a vehicle is, so it
keeps the name as the link names it (`2:1`) and the layer that speaks the protocol decides whether it can
be read; a name that cannot be read is left out and said, which is the rule a secret that is not
hexadecimal follows in the keyring. A half of `0` is refused because zero is not a board: it is the
broadcast address, and an order sent to it goes nowhere in particular. The metres come back as the
metres they were written as, and the equality of the document was given up when the first number
arrived — `Eq` is no longer derived on a file that holds numbers.

What the loop holds is asked for on every tick rather than kept: the link is the one the supervisor has
at that moment, because a link that was restored after dropping is the link the orders have to go to, and
a task that held the session it started with would keep sending to a link that was closed. A second start
is refused rather than running two loops over the same socket, and a stop is honoured between the sends
and the next tick, which is what a stop means for a loop that waits on the wire.

The **sheet** of the formation, opened from Setup, is upstream's `Swarm/FormationControl.cs` with the
arm, takeoff and land buttons left off, because those are the actions the actions tab already has and
being in a formation is not a reason to do them. It shows who leads, the places with their three numbers,
and the three buttons that say what the operator wants: Save writes what was typed without changing the
decision, Start runs the loop, Stop stops it. The names it knows are the vehicles the link has seen and
the names the file kept, so an unreadable name stays visible as what was written rather than being
guessed into a vehicle. The sheet opens without a link — the file can be read without a board on the
wire — and says that nothing is sent until one answers. What it shows is measured at 1920x1080 and at
1024x640 in Spanish, and the store that holds its state keeps what was typed apart from what the backend
says about it: the places that could be read, and the names that were left out.

A telemetry log carries the frames of every vehicle that crossed the link, and the header is the only
place that says which board a frame came from, so a record now carries its sender as well as its
message and its instant. A log of a flight with two boards reads as two flights rather than one.

Six differences are recorded. MPCraft refuses a vehicle that is in the formation without having
been given a place in it, rather than inventing an offset from how many vehicles were missing at
that moment, which is what upstream's fallback does. It projects through the zone its own
projection picks rather than upstream's `(lng + 186) / 6`. It keeps only the two of the four PIDs
upstream allocates per vehicle — the roll and yaw filters are constructed and never read, because
the roll comes from the geometry of the turn and the yaw error goes through raw — and it guards
the two divisions upstream lets run to infinity, which give the same outcome with the guard. The
yaw band is compared against the wrapped difference: upstream compares the raw one, which makes
179° and -179° three hundred and fifty-eight degrees apart. And the mask travels as the byte
upstream's arithmetic leaves behind, because two of the bits it leaves set — 8 and 16 — have no name
in the dialect as it is defined today; read through the current names the same byte means something
else, and a construction that keeps only the named flags would send 32 rather than `0x38`. Off the
central meridian the grid and the earth disagree by the convergence angle — a slot measured east on
the grid reads as a true bearing about 1.2 degrees off at Canberra — which upstream has too, and
which the tests record rather than paper over. And the model speaks **degrees** for an attitude while
the wire carries radians, so the conversion happens once, at the wire boundary; before this change the
decoder, the graphs and the desktop event each read the same field as radians, and the three-degree
heading band could not honestly fire on data that came from a vehicle.

The generator that writes the fixture works the CRC_EXTRA seed out from the field definitions — the
name, then each base field in wire order as `type name `, the array length, folded to a byte —
rather than copying a table, and it has to reproduce the five seeds the dialect publishes before it
is allowed to write anything. The sixth, the one for `SET_ATTITUDE_TARGET`, comes only from the rule;
it agrees with what the dialect generates, which is the kind of agreement worth having between two
implementations that were not built from each other.

The fence used to be impossible: `MAV_MISSION_TYPE` is a MAVLink 2 *extension* field, and
the generated bindings were built without the extension feature, so the plan type could not
travel. The bindings were never the limitation — the feature flag was. With
`mav2-message-extensions` switched on, the plan type reaches the wire on
`MISSION_REQUEST_LIST`, `MISSION_COUNT` and every item, and a fence plan downloads as a
fence plan.

The fence is now drawn as the polygon it is: the vertices that are included in it make
the ring, the points that are excluded from it are drawn in the colour of the danger, and
the circles that are in it or out of it are drawn as they are. The editor has the three
kinds of plan upstream's planner has - mission, fence and rally - and the plan of each
kind is edited with the commands that kind takes.

The connection profiles are stored, listed, edited and opened at launch.

The map draws the vehicle, the track, the plan and the fence, and the operator chooses
where the tiles come from: the three styles of OpenFreeMap, the ordinary map of
OpenStreetMap, the topography of OpenTopoMap, the satellite imagery of Esri and the pale
map of CartoDB. They are the sources that are free and that need neither an account nor a
key.

The plan editor keeps the edits apart from the plan the vehicle holds, and says so when
they differ, and a change can be taken back with the button or with control and Z.

The **metadata** of a parameter is shown with it: the description, the unit, the range
and the value it had before anybody changed it. A parameter that is a choice is drawn as
the list of what it can be, and a parameter that is a mask is drawn as the flags it
holds, each of them a switch the operator can throw. The catalogue carries the code of
every choice and the mask of every flag, so the interface does not have to guess which
number means what.

The **flash** can be written, so that a parameter the vehicle has in its memory but not
in its own memory survives a loss of power. The parameters the operator changed can also
be put back as they were: the catalogue knows the value every parameter had before
anybody changed it, and only the ones that were changed are written, in the order the
vehicle needs them - the ones that turn something on go first, the way upstream writes
them.

A value written to flash **survives the power going away**, and only a person can take the power away, so the check
is in two halves: the number is written, the board is restarted by hand, and what is compared afterwards is what the
board says about itself after its own boot. On this board `FENCE_ALT_MAX` was written as 61.5 and came back as 61.5,
and it was put back as this run found it — 42.5 — while the number it held before anybody changed it is not known,
because an earlier run on this bench wrote a value without recording what it replaced; that is said rather than
hidden.

The number written has to be different from the number the board already holds, because writing the number the board
had says nothing about flash, which is what an earlier run did. This board answers its whole list — 1 389 values —
and not a single request for one parameter, so the value before and the value after both come from the list, and the
echo a write is owed was given on one run and not on another, so nothing here depends on which of the two happens.

The **plugins** run inside a sandbox. A plugin is a WebAssembly module with a manifest a person can
read: the name, the module, what it wants to be allowed to do, and how often it wants to be called,
with the rate capped at the telemetry tick. A capability is not checked when the plugin calls — it is
what the module can *import*, so a plugin that was not given commands has no function for commands,
and a module that asks for one is never built. That is stronger than a check somebody could forget.
A plugin also gets a number of steps per call, which is how a loop that never ends is stopped without
stopping the application, a ceiling on the memory it may hold, a ceiling on the stack its calls go
through, and a ceiling on how much of its memory one string is read from, because a plugin that names a
length the size of its memory would otherwise read the whole of it. The four stages are the ones
upstream has — read, link open, tick, close — and only the first is required, because plenty of plugins
have nothing to do every second. What the operator decided is kept in the settings file, apart from what
the manifest asks for, so that a manifest cannot grant itself anything: a plugin nobody chose is not run,
which is the state a fresh installation is in. The sheet of the plugins, opened from Setup, lists what
the folder holds — the name, the version, who wrote it, what it asks for, how often it is called, and
where it stands — and the switch there is what decides whether one is run, which is the decision kept for
the next run. A module that names a path climbing out of the folder is refused before anything is read,
because `..` in a manifest is a plugin reaching past the folder somebody put it in. What a plugin asked
for is kept with a ceiling and run afterwards, outside the sandbox, because a call inside the sandbox must
never wait for the wire; what a plugin said is kept with the ceiling the console has, and the sheet shows
both, the asking and the saying.

The **shapes a plugin is given** are numbers and names, never protocol messages, and each one answers with what
the vehicle said rather than with what the reader expects. The shape of the plan now answers with two things:
how many items the vehicle holds, and which item the vehicle says it is on — before this change the capability
named `mission_access` answered only with the count, which is less than the name promises. A vehicle that has
not said where it is in the plan gets nothing rather than item zero, because "nothing was said" is not a place
in the plan, and the known-bad case is shown firing: a module that imports the plan shape under a manifest that
grants only the way of saying something is never built, and the failure names the capability it belongs to,
which means the import name has to be read as belonging to a capability — otherwise the operator would see the
runtime's own words instead of the name of what was refused. That the value crosses the boundary is read by
giving the number back through the stage, so the test sees the number the surface answered with rather than
that a call happened; and the item shown is the one the vehicle reported, taken from the same report the plan
count comes from.

The **surface is fed by a scripted peer** rather than by a hand-written one, which is what says where its
numbers come from: the tests that read a surface written by the test prove the shape crosses the boundary but
not the source of the value. The peer puts four things on a real socket, and they are written down before the
test calls anything — item three of twelve, from `MISSION_CURRENT`; `FENCE_ALT_MAX` as 61.5, from the answer the
vehicle gives about itself; 584.0 metres, from the position message; and fourteen satellites, from the
positioning message. Before anything arrives the surface has nothing to give, so the plugin sees the number that
says *nothing was said* rather than zero, and the first reading fails for that reason; after the wire answers,
the same plugin agrees with what it reads through its imports, which is the value crossing the boundary as the
wire gave it. What cannot be asserted here is said rather than assumed: there is no physical board on this
machine, so what the run proves is the path from the frames to the surface, not that a vehicle reports these
numbers.

The **same surface is read against a live vehicle** as well, behind the same flag
(`crates/mpcraft-app/tests/plugins_live.rs`), because a scripted peer was written to carry the numbers it answers
with, while a vehicle decides them for itself. Three numbers can still be written down before the run, and they are
the three this area puts on the wire: the plan of three items it uploads, the item it asks the vehicle to be on, and
the value it writes to `FENCE_ALT_MAX`, which is read back as the number the vehicle stored rather than as the
number that was asked for. The other two readings cannot be predicted, because a simulator chooses its own height and
its own satellite count, so they are watched until the vehicle says them, and the comparison afterwards is said for
what it is: the surface holds a copy of the snapshot the aggregator filled from the frames, so what this proves is
that the number crossed the boundary through the import, not that a second reader would have got the same one from
somewhere else. The first reading still fails with the code that says nothing was said, which is what keeps the
second half an assertion rather than a restatement. What cannot be asserted here is said rather than assumed: the
image the gate names cannot be pulled anonymously on this machine — the registry answers *unauthorized* — and nothing
is listening on the port the simulator reserves, so the area compiles and reports the stand-in rather than answering;
the runner names the scripted peer as what stands in for it, and the control shows both halves read as their own
reports, so a scripted peer passing does not stand for an area no vehicle answered.

The **application is launched** from an entry that names the folder holding the configuration, because the
CLI takes the project from the folder it runs in or a subfolder of it: run from `apps/ui` it says nothing
about the application and `tauri dev` panics ("Couldn't recognize the current folder as a Tauri project",
`crates/tauri-cli/src/helpers/app_paths.rs:140`), while run from `apps/desktop` the same project resolves
and prints where the frontend is built and what address the window is given. The commands run before the
build are read by npm from wherever npm finds its package.json, and with none anywhere between the app
folder and the root the launch failed with ENOENT on `apps/package.json`; a probe printing its own cwd
showed the CLI running its command from `apps` in that state and from `apps/desktop` once a package.json
sits next to the configuration, so that folder now holds one whose scripts delegate to the frontend
through `--prefix`, which is the form that puts a script's cwd at the folder it names. The entry stays
where the CLI is declared as a dependency, because that is the folder npm can find the binary from, and it
moves to the app folder rather than being written in two places.

What the launch gate asserts is the chain rather than a window: the entry names the CLI as something npm
can find, the folder it moves to holds the configuration, each command names a script that exists in the
package.json npm reads from where it runs, the port the configuration expects is the port the dev server
binds — insisted on, not asked for — and the port the browser run connects to, and the built frontend is
written where the configuration looks for it. Its known-bad cases are an entry that runs the CLI from the
folder whose package.json holds it, an entry naming the CLI without declaring it, a folder holding the
configuration with no package.json, a command naming a script nobody wrote, a delegation that comes back to
itself, a port that disagrees on each of the three places, and a build output the configuration does not
look at; the five documents are also shown passing untouched. What it cannot see is said rather than
assumed: the window itself, the compiler, whether an install is present in either folder, and a port set
through a variable or an environment override.

The **contract** is read from both sides now, because one document was spoken two ways and nothing read the
two against each other. The pair that identifies a vehicle was written with underscores by the structs that
carry it — the key in the application and the id in the vehicle state — while the interface read it with the
camel humps every other field of the boundary uses, so the value the interface asked for was not the value
that arrived; and the same pair was declared twice on the frontend side, once as the document the contract
declares and once as a `NodeKey` carrying the underscore spellings, which is a document that can fall out of
step with the struct that writes it. The events were read as more than one document: the state of the link
was read as a partial shape by the hook that shows the connection and as a wider one by the store that keeps
the state, and the play-back payload was declared inside the store rather than read from the contract, and one
hook listened to an event nothing emits — a subscription to a channel that will never answer, which is why it
was taken away rather than left as something that might be wired later. Each event is now read through one
document, declared by the contract, and each field that crosses the boundary is compared through the words it
says rather than through its casing, so `system_id` and `systemId` are read as the same field and then
required to agree; and the rule that the boundary speaks camelCase is read in both directions, because a
command reads a document as well as answers with one. What the reading covers is 108 documents reachable from
the boundary, 103 of them paired and 495 fields across those pairs, and what it does not cover is said rather
than pretended otherwise: a rename applied through a macro or a manual `Serialize` implementation, a document
the contract declares and nothing names — `CommandError`, reached as the error half of a result rather than as an
answer of its own, and `TileBounds`, which no call reads through it — and what the boundary does not reach — the
settings file and the log formats keep their own spellings and are not read here. The gate
(`scripts/check-contract.js`) is shown firing on each thing it claims to catch — a listener for a name nothing
emits, a listener holding a shape of its own, one event read as two documents, a contract field with an
underscore in it, a struct the boundary reaches writing the pair with underscores on both crates, a file
declaring its own copy of a document the contract already declares, a pair where the document the reader
declares for a field is not the one the writer sends through it, an anonymous shape put back inside a document,
a call to a name nothing answers, a shape written at a call site, and the pair a call makes compared against the
command that answers it — and the sources as they stand are shown passing. The runner scopes this gate with a
folder, because the documents that cross the boundary are reached through types the crates declare, and the
control shows a file inside that folder being read as touched rather than the gate saying it had nothing to
say. Both sides carry a test written down before the rule was called: the crates assert the keys they write for
the pair, and the store updates from a
payload read as bytes with those spellings — which is what a reader holding its own shape could not do — while
the play-back is read through the same document the contract declares, so a tick of the link during a
play-back does not take the window back from the flight being shown. Whether an event reaches a window is
outside of what a reading of two source files can say, and that is said rather than assumed.

The **pairs** are read now, which is what the first version of this gate could not do: 53 documents are named
the same on both sides and 50 are named differently, so `Telemetry` is read against `TelemetryEvent`,
`LossStats` against `LossEvent`, `VehicleMessage` against `MessageEvent`, `SigningKeyEvent` against
`SigningKeyView` and `DtedCell` against `DtedCellInfo`. The pairing is not a table somebody writes: it is what
the same field says on both sides — `StateEvent.telemetry` is declared as `Telemetry` and written as
`TelemetryEvent` — so a reader that declares another document for that field is read against what the writer
sends through it, and that is one of the known-bad cases shown firing. A field the contract declares and the
struct behind the pair does not write is a finding, because that is a value the interface cannot get; a field
the struct writes and the contract does not declare is said as a fact, because JSON carries what the interface
does not ask for, and nothing today is on that side. Four places that were shapes inside a document are named
now — `AdsbRelaySettings`, `MessageCount`, `TransponderSettings` and `UnreadPlace` — because an anonymous shape
has no struct to be read against, and the rule refuses one rather than skipping it: the control shows
`TransponderStatus.settings` refused when it is put back as a shape, and the sources are shown naming what they
declare. A call crosses the boundary the same way an event does — it answers with a document and reads one — so
the pair a call site makes is read like the pair an event carries, and that is the seventh thing the gate asserts:
of the 136 commands the backend answers, 135 are called from the frontend and every call goes to a name that is
answered; 103 documents are paired with 495 fields compared, and `PartialUpdateInfo` is named now because the call
that reads the answer of `upload_mission_partial` held its shape at the call site rather than a name, which is the
same class of drift as a listener holding a shape of its own and is refused rather than skipped. What is said as a
fact rather than as a fault: `connection_status` is a command nothing in the frontend calls, because the rule reads
calls from the interface towards the backend and not the other way round, and the two documents nothing names are
`CommandError`, reached as the error half of a result rather than as an answer of its own, and `TileBounds`, which
no call reads through it. Each of the three call controls is shown firing — a call to a name nothing answers, a
shape written at the call site, and the pair the call makes compared field by field against the command that
answers it (`Plan↔PlanInfo.items`) — and the sources are shown passing. The gate reads events from both sides, and
now reads calls from both sides too.

The **way in** is read as well, and the direction changes what a field means: on the way back a field the struct
carries and the contract does not declare is only a fact, because JSON carries what the interface does not ask for,
and on the way in it is a finding, because the backend expects a value the interface never sends — the same class as
a field the reader asks for and the writer never sends, seen from the other side. That is the eighth thing the gate
asserts, and the side is kept as its own fact rather than assumed, because the same document can be answered by one
command and read by another. Of the 239 keys the 136 commands read — the 116 arguments that are the state or the
application handle are not keys, because Tauri gives them to the command and the interface never sends them — 76 are
read through a document, and the pair is not a table written here: either the argument names a name the contract
declares (`ConnectRequest` and `VehicleRequest`, which together carry most of them) or a field says the pair for it —
`ConnectionProfile↔ProfileEvent` through `set_connection_profiles`, and `GeoPoint↔PositionEvent` through
`set_home_position` and `dted_path`. The other 163 are said as facts: 154 are primitives and nine carry `PlanKind`
(which the contract declares as a union of words rather than an interface). What was said before about this count is
corrected now: it named 148 arguments, which are only the ones whose type is a document,
because a Rust primitive begins with a lowercase letter and the reading skipped it — that is why `manual_control`
looked like a call that sent keys nobody read. What the reading cannot see is said rather than pretended otherwise:
the values that sit under a key and the type each value is read as are not compared, and a payload the call holds
under a name rather than writing as a literal would be said as a fact because nothing declares its shape, which none
is today. Both halves of the way in are shown firing — `VehicleRequest.nothingTheInterfaceSends`, expected by the
struct and never declared, and `VehicleRequest.nothingTheStructNeverReads`, declared as what the interface sends and
never read — and the sources are shown passing with the same 103 paired documents and 495 fields, which says that this
rule found nothing to complain about today: it is the rule that would say it if a request grew a field nobody sends.

The **five documents on the way in** were shapes the call sites decided for themselves, so the reading had no name to
compare them against; they are declared by the contract now, and the ten arguments that read through them are read
field by field: `ParamChange` through `write_parameters_batch`, `OsdSlotRequest` through `osd_slot_config`,
`PlanItemRequest` through the six plan calls (`upload_mission`, its file-transfer and partial forms, `validate_mission`
and `plan_terrain_profile`), `SigningKeyEvent` through `set_signing_keys` and `SwarmRequest` through
`set_swarm_settings`. The names follow what the boundary calls them: `FormationRequest` became `SwarmRequest` because
the argument of the command that reads it says so, and `SigningKeyEntry` became `SigningKeyEvent` because the same
document is answered as `SigningKeyView` by one command and read through a field of another, so a name that only
speaks the answer would say half of it. `FormationSlot` keeps the pair its field says (`FormationSlot↔SwarmSlot`) rather than
being renamed, because the settings crate writes it under that name and nothing new is pulled across the boundary.
The call sites send through the declared documents now — `osdSlotConfig` takes one, `paramsStore.writeBatch` and the
bridge read through `ParamChange`, the plan calls read their items through `PlanItemRequest`, and the keys sheet read
the keyring's answer through a shape it decided for itself, which is now `SigningStatus` — and what that changes is
what the compiler enforces, not what this reading compares: the gate reads a call as far as its keys go, so the values
under those keys are still not compared here, and a document that the contract declares and nothing reads through it
remains unpaired, which today is only `CommandError` and `TileBounds`.

The **ninth thing** the gate reads is what each call actually puts on the wire, against every key its command reads.
Tauri takes an argument by the key the message carries, so a key the command needs and the call never sends is a call
the boundary refuses, and a key the call sends that no argument reads is a value that travels and is never looked at:
both are findings, and an optional parameter left out is said as a fact, because a missing key is what an `Option`
reads as. The 135 calls cover 135 of the 136 commands, and every key those commands read arrives at its call site
now; no call leaves an optional key out, so the two that looked like that before were read wrong rather than counted
wrong — the shorthand property was not read, and `{ kind }` says its own name. What the rule found were two calls the
boundary refused, not two spelling disagreements. Eight camera commands — `capture_photo`, the two that carry the
interval, the two that carry the recording, `set_camera_zoom`, `track_point` and `track_rectangle` — read
`camera_component`, which Tauri reads as `cameraComponent`, while the interface wrote `cameraComponentId`: the same
word list with one word more, so the component the operator picked never reached the vehicle. And
`write_parameters_to_flash` read `req` while the call sent an empty payload with no key under it, which is what the
boundary answers as a key that is missing. The eight names now say `camera_component_id`, which is the key the
interface already wrote, and the flash write sends the request through the helper the other parameter calls use, so
the key is there rather than assumed. Both halves are shown firing — the flash write with no key under the request,
and a key at a call site that no argument reads — and the helper is read through the keys its declared shape carries,
because nothing names that shape, which is what lets a call built by it still be compared with the command that
answers it.

The **gates that read a tool** — typescript, prettier, the rust formatting, the lints, and the rust tests — had no
known-bad case, and a gate whose green is its exit code alone is a gate that cannot fail. Each of the five now has
one (`scripts/check-tool-controls.js`), and each is shown failing on its case and passing afterwards with the file as
it was. A tool gate can only be shown failing on something the tool reaches, and that is what decides how each case
is written: a Rust file that no module declares is neither compiled nor formatted, so the two Rust cases disturb a
file the crate already holds — a line with one space too many, which rustfmt reads as a difference and clippy does
not read at all, and a boolean expression that says itself twice, which `-D warnings` turns into a failure on a file
formatted as the formatting gate expects, so only the lint can speak — while `tsc` takes everything under `src`,
`cargo test` takes every file in a `tests` folder as its own target, and prettier takes whatever it is named. Only
one of the five shows the finding itself, because the runner keeps the first line of what a tool says: npm answers
before the compiler, prettier opens with its heading, cargo may open with what it compiled, and the report of the
test that failed comes behind whatever cargo printed first, so those four buy the flip and nothing more. And a
scratch file cannot be shown passing after it is gone, so the pass side names a file that is still there rather than
the one that was written.

BLOQUEADO is exercised too, and with a real lock rather than with a string that looks like one (`scripts/run-gates-controls.js`):
one file the prettier gate reads is held by a separate process that opens it with no sharing at all, and the control
waits until its own attempt to open the same file answers with the sharing violation — which is what makes the lock a
fact before anything is asserted — and then the same gate is measured again on the same file once nothing holds it.
The rule reads both spellings of a busy artifact, the sharing violation a tool reports when it cannot open a file and
the one cargo reports when it cannot overwrite a build output, and the state is a gap rather than a pass. What this
exercises is the rule that reads a busy artifact as a gap, not a file locked by MPCraft itself: Node cannot choose the
share mode of a handle, so the process holding it is the control's own, and the application is not running here. The
case shows the flip rather than the finding, because prettier opens with its heading.

The control column is read as a document now (`scripts/check-tool-controls.js`), because the runner's own note points at it and nothing read it: the fifteen gates name their known-bad case and the checker under `scripts/` they name is on the disk, since a claim that points at a file which is not there is a claim nobody can show. Both halves are shown firing — a gate that says nothing about its negative control, and a control that names a checker that does not exist — on a copy of the runner, so the runner as it stands is never disturbed, and the sources are shown passing. What the reading does not reach is said: a path named outside `scripts/` — the Playwright spec, a test file inside a crate — is not read as a path, because the reading cannot know which layer a sentence about a case belongs to.

The parity checker names what it cannot reach inside what it reports, not only in its header, because the gate reads that report through its verdict field and would not see the sentence otherwise (`scripts/check-parity.js`). The rule is shown firing on two copies of the checker — one that names no limit, and one whose limit says only what the table can be compared against — with the checker as it stands passing, and it is said what the reading does not do: it reads the sentence as text, so nothing of the checker is executed with its limit taken out.

The runner carries the same rule for itself (`scripts/run-gates-controls.js`): what the whole run cannot see is named inside what it reports — no window, no vehicle, no board, and no tool reaching beyond the file it was shown — and the reading fires on two copies of the runner, one that names no limit and one whose limit says only how the states are read, with the runner as it stands passing.

The **scripting host** is dropped, and the decision rests on what the published file does rather than on
what its name suggests. The clone named at the top of this file is not on this machine, so the row was read
against `Script.cs` as published (220 lines), and nothing here says anything about a running engine: the
reading is of the source. What that host does before it runs anything is load every assembly in the process
into the language runtime and put the application itself into the scope — `MainV2`, `FlightPlanner`,
`FlightData`, `Ports`, `MAV`, `Joystick` — so a script is not an automation but a program that can reach
whatever the host holds, including the window and the socket, which is the ambient authority ADR-007 refuses
for the extension surface and the reason rows 9.1–9.3 are the answer to this row. What its own API offers is
six calls: read and write a parameter, change mode, send a radio override, wait for a word in the messages,
sleep. Each has an equivalent in the plugin host — a value the vehicle last said is read through
`read_telemetry`, mode and the override go out through `send_commands`, a word in the messages is what
`notify` says, and the two parameter calls are `read_params` and `write_params`, which is the paragraph
below. Waiting has no equivalent by
design, because a call inside the sandbox must never wait for the wire — upstream's waiting is `Thread.Sleep`
on the calling thread, which is a plugin holding the link while a piece of somebody else's code sits on it.
Two functions of the surface it advertises answer with nothing in the published source: `mavlink_connection`
and `recv_match` return null, so the pymavlink compatibility a reader would expect from this row does not
exist. And one thing is kept as a difference rather than written as a gap: upstream runs a text file the
operator pastes, and MPCraft runs a module with a manifest a person reads before deciding to run it, so there
is no paste-and-run automation here, which is what the sandbox costs.

The **parameters** are part of what the sandbox can grant now, as two names rather than one: `read_params`
and `write_params`, because a plugin that can read a number is not a plugin that can change one, and the
known-bad case is what shows the difference firing — a module that imports `set_param` under a manifest that
grants only the read is never instantiated, so nothing of its code runs. The read answers with what the vehicle
said, and a name the vehicle has not said about itself gets nothing rather than a number the reader invented.
The change is queued rather than sent from inside the sandbox, for the same reason a command is: a call inside
the sandbox must never wait for the wire, and a parameter write is waited for, because the echo is what says
what the vehicle stored. What a plugin reads afterwards is the number the vehicle answered with and not the
number it asked for, and the writing is written in the message console, because a change to the configuration of
the vehicle made by somebody else's code has to be visible where the operator reads what the vehicle said. The
queue has its own ceiling of thirty-two askings, like the queue of commands, so a plugin that writes without
stopping cannot grow the state of the application, and the spelling the design document used
(`write_parameters`) is still refused rather than guessed at, which is what keeps a manifest from promising
something the sandbox cannot give.

The two dropped features are the Python LogAnalyzer integration (6.14) and the IronPython scripting host
(9.4). The first depends on a Python 2 toolchain that is no longer maintainable, and its capability is
covered by the native graphing and analysis in 6.7–6.8, which is a strictly better outcome. The second is
dropped for the reason the paragraph above gives, and the parity gate now reads that reason: a row marked
⬜ has to carry why beside the mark, because a decision that says only "Dropped" is a feature left out
without anything a reader can check.

---

[Phase plan]: docs/PLAN.md
