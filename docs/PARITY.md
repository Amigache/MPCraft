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
| 1.18 | Bluetooth LE transport | `ExtLibs/Comms/CommsBLE.cs` | 🔜 | 5 |
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
| 8.4 | DFU upload over USB | `ExtLibs/UAVCANFlasher`, `NetDFULib` | 🔜 | 5 |
| 8.5 | Board auto-detect | `BoardDetect.cs` | ✅ | 5 |
| 8.6 | Firmware erase / bootloader mode | `ConfigFirmware.cs` | ✅ | 5 |
| 8.7 | Secure firmware (signed) | `ConfigSecure.cs`, `ConfigSecureAP.cs` | 🔜 | 5 |

## 9. Extensibility & integration

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 9.1 | Plugin lifecycle (Init/Loaded/Loop/Exit) | `Plugin/Plugin.cs` | ✅ | 6 |
| 9.2 | Plugin host API surface | `Plugin/Plugin.cs` (PluginHost) | ✅ | 6 |
| 9.3 | Plugin management UI | `Plugin/PluginUI.cs` | ✅ | 6 |
| 9.4 | Scripting host | `Script.cs` (IronPython) | 🔜 | 6 |
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
| 🔜 Planned | 8 |
| ⬜ Dropped | 1 |
| Partial (🟡 / 🔽) | 3 |

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
waited for, asked what it is, its flash is erased, the firmware is written block by
block and the board is told to start again.

The board can also be **restarted into its bootloader** from the same sheet: the vehicle
is asked to do it, and it does. The command that asks for it (`MAV_CMD_FLASH_BOOTLOADER`,
42650) is not in the enumeration of commands of the protocol library the project uses,
which stops at 32000, so it is written by hand: the message that carries it is built
byte by byte, the number of the command goes in it as it is, and the answer of the
vehicle is read as an answer is read. The link stops carrying MAVLink when the board is
in the bootloader, because the board is then waiting for the protocol of the bootloader.

The flash of a board can also be **erased**, from the same sheet: the board is asked
what it is and its memory is left clean, which is what the operator wants when the
board does not start.

The **files of the vehicle** are read from the File menu: the vehicle holds a filesystem
of its own, and the file transfer protocol is how a program reaches it. The folders are
listed the way the vehicle says them, the operator walks into them, and a file is
brought to the disk of the computer. The protocol is not one the library of the project
carries — the message is the 110th, and its seed is not among the ones the library
knows — so it is written by hand as well: the header of the transfer, the opcodes, and
the payload of each answer.

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
the wrong key does not work at all.

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
player connects to. Drawing that video inside MPCraft is not done yet.

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
was last asked for. Picking a place to follow from the picture itself waits on drawing the
video, so the place is typed: that is the one thing upstream's sheet does which this one
cannot do yet.

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
What arrives as data stays as the data says it: the names of the modes, of the camera states,
of the gamepads, of the parameters and of the tile providers are what the vehicle or the file
says, and translating them would say something the vehicle does not say. What the library of the
map writes is outside the reach of the bundles: the titles of its own navigation control — zoom,
rotation — are the library's words, not ours.

The **identification of the vehicle** is in the tab of the state: what it is, its
number, what kind of vehicle it is, where it says it is and what the operator says about
it. The parts arrive in three different messages, and each one keeps what the others
said.

The transponder, the generator telemetry and the folder of the operator's own map are
available elsewhere in the application: in Setup, the vehicle state and the tile
settings respectively. The video sheet describes and controls the stream, but does not
draw the video inside MPCraft.

The **generators of a survey** are three: the circle, the reticle of a field — the
strips the vehicle flies one after the other, apart by the distance its sensor sweeps —
and the smooth path, the curve the vehicle follows when it is told to follow a spline.
The plan draws the path as a straight line from one place to the next, and the smooth
one when the operator asks for it.

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
bytes for it, and the session sends what the decision produced to a simulated vehicle. What is not
there yet is the dispatch: the sending of that decision to each vehicle ten times a second, which
needs a live fleet, so the row stays partial.

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
the vehicles fly into their slots, so the row stays partial.

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

The single dropped feature is the Python LogAnalyzer integration (6.14) — it depends on a
Python 2 toolchain that is no longer maintainable. Its capability is covered by the native
graphing and analysis in 6.7–6.8, which is a strictly better outcome.

---

[Phase plan]: docs/PLAN.md
