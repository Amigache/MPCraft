# MPCraft — Parity Matrix

Feature-by-feature comparison with Mission Planner. Every upstream feature must be marked
**Done**, **Planned**, or **Dropped** (with rationale). No feature may be silently omitted.

**Legend:** ✅ Done · 🔜 Planned (phase) · ⬜ Dropped · 🟡 Partial

Reference: `ardupilot/MissionPlanner` at commit `1f4a2e0` (analysed from `_ref_missionplanner/`).

---

## 1. Connection & link management

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 1.1 | Serial port connection | `ExtLibs/Comms/CommsSerialPort.cs` | ✅ | 0 |
| 1.2 | Serial port enumeration | `ExtLibs/Controls/ConnectionControl.cs` | ✅ | 1 |
| 1.3 | Baud-rate selection & auto-baud | `ExtLibs/Controls/ConnectionControl.cs` | 🔜 | 1 |
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
| 1.14 | Signing keyring persistence | `ExtLibs/ArduPilot/Mavlink/MAVAuthKeys.cs` | 🔜 | 0 |
| 1.15 | MAVLink FTP client | `ExtLibs/ArduPilot/Mavlink/MAVFtp.cs` | 🔜 | 5 |
| 1.16 | Auto-connect profiles | `ExtLibs/Utilities/AutoConnect.cs` | 🟡 | 7 |
| 1.17 | NTRIP correction source | `ExtLibs/Comms/CommsNTRIP.cs` | 🔜 | 5 |
| 1.18 | Bluetooth LE transport | `ExtLibs/Comms/CommsBLE.cs` | 🔜 | 5 |
| 1.19 | File / tlog replay as a transport | `ExtLibs/Comms/CommsFile.cs` | ✅ | 0 |
| 1.20 | Serial passthrough (`SERIAL_CONTROL`) | `MAVLinkSerialPort.cs` | 🔜 | 5 |

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
| 2.9 | Pre-flight checklist | `Controls/PreFlight/CheckListControl.cs` | 🔜 | 1 |
| 2.10 | Battery banks 1–9 monitoring | `CurrentState.cs` | 🔜 | 1 |
| 2.11 | Multi-GPS status | `CurrentState.cs` | 🔜 | 1 |
| 2.12 | EKF status & vibration display | `Controls/EKFStatus.cs`, `Vibration.cs` | 🔜 | 3 |
| 2.13 | Servo & relay output display | `Controls/ServoOptions.cs`, `RelayOptions.cs` | 🔜 | 3 |
| 2.14 | Auxiliary function display | `Controls/AuxOptions.cs` | 🔜 | 3 |
| 2.15 | Transponder (Mode-S) control | `FlightData.cs` tabTransponder | 🔜 | 5 |
| 2.16 | EFI / generator telemetry | `CurrentState.cs` | 🔜 | 5 |
| 2.17 | OpenDroneID display | `ExtLibs/ArduPilot/OpenDroneID.cs` | 🔜 | 5 |
| 2.18 | Wind direction indicator | `Controls/WindDir.cs` | 🔜 | 2 |
| 2.19 | Distance-to-target bar | `Controls/DistanceBar.cs` | 🔜 | 2 |

## 3. Actions & commands

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 3.1 | Arm / disarm | `MAVLinkInterface.doARM` | ✅ | 1 |
| 3.2 | Mode change (custom_mode table) | `Common.getModesList`, `translateMode` | ✅ | 1 |
| 3.3 | RTL / Land / Loiter / Auto shortcuts | `FlightData.cs` | ✅ | 1 |
| 3.4 | Takeoff command | `MAVLinkInterface.doCommand` | ✅ | 1 |
| 3.5 | Generic MAV_CMD long / int executor | `doCommand`, `doCommandInt` | ✅ | 1 |
| 3.6 | Guided mode — fly to waypoint | `setGuidedModeWP` | 🔜 | 2 |
| 3.7 | Guided mode — "go here" from map | `FlightData.cs` goHereToolStripMenuItem | 🔜 | 2 |
| 3.8 | Position target global-int | `setPositionTargetGlobalInt` | 🔜 | 2 |
| 3.9 | Motor test | `doMotorTest` | 🔜 | 3 |
| 3.10 | Reboot autopilot | `doReboot` | 🔜 | 5 |
| 3.11 | RC override | `SendRCOverride` | 🔜 | 3 |
| 3.12 | Manual control | `SendManualControl` | 🔜 | 3 |
| 3.13 | Camera / gimbal control | `CameraProtocol`, `GimbalManagerProtocol` | 🔜 | 5 |
| 3.14 | Serial port pass-through | `SendSerialControl` | 🔜 | 5 |
| 3.15 | Flight-mode presets & custom modes | `ConfigFlightModes.cs` | 🔜 | 3 |

## 4. Mission planning

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 4.1 | Mission download / upload | `ExtLibs/ArduPilot/mav_mission.cs` | ✅ | 2 |
| 4.2 | `MISSION_ITEM_INT` support | `getWPAsync` (capability-gated) | ✅ | 2 |
| 4.3 | Partial mission update | `mav_mission.uploadPartial` | 🔜 | 2 |
| 4.4 | Waypoint table editor | `GCSViews/FlightPlanner.cs` (Commands grid) | 🔜 | 2 |
| 4.5 | Map-based waypoint add / drag / delete | `FlightPlanner.cs` | 🔜 | 2 |
| 4.6 | MAV_CMD parameter metadata & units | `mavcmd.xml` | 🔜 | 2 |
| 4.7 | Altitude modes (absolute / relative / terrain) | `FlightPlanner.cs` altmode | 🔜 | 2 |
| 4.8 | Geo-fence polygon drawing & upload | `FlightPlanner.cs` polygon tools | 🟡 | 2 |
| 4.9 | Fence inclusion / exclusion lists | `FenceInclusion/Exclusion` | 🟡 | 2 |
| 4.10 | Rally point upload / download | `getRallyPoint`, `setRallyPoint` | ✅ | 2 |
| 4.11 | Home position management | `getHomePositionAsync` | 🔜 | 2 |
| 4.12 | QGC `.waypoints` import / export | `FlightPlanner.cs` readQGC110wpfile | ✅ | 2 |
| 4.13 | KML import / export | `FlightPlanner.cs` processKML | ✅ | 2 |
| 4.14 | SHP / DXF import | `FlightPlanner.cs` LoadSHPFile | 🔜 | 2 |
| 4.15 | Circle / spline / survey mission generators | `FlightPlanner.cs` circle tools | 🔜 | 2 |
| 4.16 | Mission validation & warnings | `FlightPlanner.cs` checkZeroAlts | 🔜 | 2 |
| 4.17 | Undo / redo buffer | `FlightPlanner.cs` history | 🔜 | 2 |
| 4.18 | Elevation profile graph | `Controls/ElevationProfile.cs` | 🔜 | 2 |
| 4.19 | Waypoint radius (loiter) editing | `FlightPlanner.cs` loiter radius | 🔜 | 2 |
| 4.20 | Spline / smooth path display | `FlightPlanner.cs` | 🔜 | 2 |
| 4.21 | MAVFTP mission transfer | `chk_usemavftp` path | 🔜 | 5 |

## 5. Parameters & configuration

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 5.1 | Full parameter download with progress | `MAVLinkInterface.getParamListAsync` | ✅ | 3 |
| 5.2 | Parameter write with echo confirmation | `setParamAsync` | ✅ | 3 |
| 5.3 | Batched write with `*_ENABLE` ordering | `ConfigRawParams.SortENABLE` | 🔜 | 3 |
| 5.4 | Live parameter updates | `processInfoFromStream` PARAM_VALUE | ✅ | 3 |
| 5.5 | Parameter metadata (ranges, bitmasks, options, units) | `ParameterMetaData.xml` | 🔜 | 3 |
| 5.6 | Parameter tree by prefix | `ConfigRawParams.BuildTree` | 🔜 | 3 |
| 5.7 | Parameter search | `ConfigRawParams.txt_search` | 🔜 | 3 |
| 5.8 | Favourites | `ConfigRawParams` Fav column | 🔜 | 3 |
| 5.9 | Friendly (metadata-driven) parameter UI | `ConfigFriendlyParams.cs` | 🔜 | 3 |
| 5.10 | `.param` file import / export | `ExtLibs/Utilities/ParamFile.cs` | 🔜 | 3 |
| 5.11 | Parameter diff against a file | `Controls/paramcompare.cs` | 🔜 | 3 |
| 5.12 | Write-to-flash / reset-to-defaults | `ConfigRawParams` | 🔜 | 3 |
| 5.13 | Reboot-required detection & prompt | `GetParameterRebootRequired` | 🔜 | 3 |
| 5.14 | Frame type selection | `ConfigFrameType.cs` | 🔜 | 3 |
| 5.15 | Accelerometer calibration | `ConfigAccelerometerCalibration.cs` | 🔜 | 3 |
| 5.16 | Compass calibration & motor compensation | `ConfigCompassMot.cs` | 🔜 | 3 |
| 5.17 | Motor test | `ConfigMotorTest.cs` | 🔜 | 3 |
| 5.18 | GPS ordering / auto-detect | `ConfigGPSOrder.cs` | 🔜 | 3 |
| 5.19 | Serial port configuration | `ConfigSerial.cs` | 🔜 | 3 |
| 5.20 | RC input / output configuration | `ConfigRadioInput.cs`, `ConfigRadioOutput.cs` | 🔜 | 3 |
| 5.21 | Failsafe configuration | `ConfigFailSafe.cs` | 🔜 | 3 |
| 5.22 | Flight modes configuration | `ConfigFlightModes.cs` | 🔜 | 3 |
| 5.23 | OSD configuration | `ConfigOSD.cs` | 🔜 | 3 |
| 5.24 | Mount / gimbal configuration | `ConfigMount.cs` | 🔜 | 5 |
| 5.25 | ESC calibration | `ConfigESCCalibration.cs` | 🔜 | 3 |
| 5.26 | Hardware device detection | `ExtLibs/ArduPilot/BoardDetect.cs` | 🔜 | 5 |
| 5.27 | DroneCAN device configuration | `ConfigDroneCAN.cs` | 🔜 | 5 |

## 6. Logs & analysis

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 6.1 | DataFlash `.log` parser | `ExtLibs/Utilities/DFLog.cs` | 🔜 | 4 |
| 6.2 | DataFlash `.bin` parser | `ExtLibs/Utilities/BinaryLog.cs` | 🔜 | 4 |
| 6.3 | MAVLink `.tlog` parser | `Log/MavlinkLog.cs` | 🔜 | 4 |
| 6.4 | PX4 ULog parser | `ExtLibs/Utilities/ULog.cs` (upstream: unwired) | 🔜 | 4 |
| 6.5 | Log download over MAVLink | `Log/LogDownloadMavLink.cs` | 🔜 | 4 |
| 6.6 | Log download over SCP | `Log/LogDownloadscp.cs` | 🔜 | 5 |
| 6.7 | Log browser — message tree + sample grid | `Log/LogBrowse.cs` | 🔜 | 4 |
| 6.8 | Log graphing with multi-axis | `LogBrowse.CreateChart` | 🔜 | 4 |
| 6.9 | Graph preset definitions | `graphs/mavgraphs.xml` | 🔜 | 4 |
| 6.10 | Cross-highlight grid ↔ graph ↔ map | `LogBrowse.GoToSample` | 🔜 | 4 |
| 6.11 | tlog playback with scrub timeline | `FlightData.cs` tracklog | 🔜 | 4 |
| 6.12 | KML export of flight track | `Log/LogOutput.cs` | 🔜 | 4 |
| 6.13 | CSV export of samples | `LogBrowse` export | 🔜 | 4 |
| 6.14 | Python LogAnalyzer integration | `LogAnalyzer/` | ⬜ | Dropped — depends on Python 2 toolchain; replaced by native analysis in 6.7–6.8 |

## 7. Mapping & geospatial

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 7.1 | MapLibre base map with provider selection | `ExtLibs/Maps/*` | 🟡 | 1 |
| 7.2 | OSM / satellite / hybrid tile providers | `GMapProviders` registry | 🔜 | 1 |
| 7.3 | Tile caching on disk | `ExtLibs/Maps/MyImageCache.cs` | 🔜 | 1 |
| 7.4 | Offline map injection (custom tile folder) | `FlightPlanner.BUT_InjectCustomMap_Click` | 🔜 | 2 |
| 7.5 | Tile prefetch for a region | `ExtLibs/GMap.NET.WindowsForms/TilePrefetcher.cs` | 🔜 | 2 |
| 7.6 | Geofence / rally / home markers | `ExtLibs/Maps/GMapMarker*.cs` | 🔜 | 2 |
| 7.7 | UTM / MGRS coordinate conversion | `ExtLibs/Utilities/utmpos.cs`, `GeoUtility` | 🔜 | 2 |
| 7.8 | SRTM terrain elevation lookup | `ExtLibs/Utilities/srtm.cs` | 🔜 | 2 |
| 7.9 | DTED local elevation | `ExtLibs/Utilities/DTED.cs` | 🔜 | 5 |
| 7.10 | Terrain-follow route generation | `srtm.getIntersectionWithTerrain` | 🔜 | 5 |
| 7.11 | Terrain data protocol server | `ExtLibs/Utilities/TerrainFollow.cs` | 🔜 | 5 |
| 7.12 | Geo-referenced image placement | `ExtLibs/GeoRef/*` | 🔜 | 5 |
| 7.13 | ADS-B display layer | `ExtLibs/Utilities/adsb.cs` | 🔜 | 5 |
| 7.14 | ADS-B relay to other stations | `MainV2.ADSBRunner` | 🔜 | 5 |

## 8. Firmware & flashing

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 8.1 | Firmware manifest download | `GCSViews/ConfigurationView/ConfigFirmwareManifest.cs` | 🔜 | 5 |
| 8.2 | Firmware selection by board | `ConfigFirmware.cs` | 🔜 | 5 |
| 8.3 | Firmware upload over MAVLink | `ExtLibs/px4uploader` | 🔜 | 5 |
| 8.4 | DFU upload over USB | `ExtLibs/UAVCANFlasher`, `NetDFULib` | 🔜 | 5 |
| 8.5 | Board auto-detect | `BoardDetect.cs` | 🔜 | 5 |
| 8.6 | Firmware erase / bootloader mode | `ConfigFirmware.cs` | 🔜 | 5 |
| 8.7 | Secure firmware (signed) | `ConfigSecure.cs`, `ConfigSecureAP.cs` | 🔜 | 5 |

## 9. Extensibility & integration

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 9.1 | Plugin lifecycle (Init/Loaded/Loop/Exit) | `Plugin/Plugin.cs` | 🔜 | 6 |
| 9.2 | Plugin host API surface | `Plugin/Plugin.cs` (PluginHost) | 🔜 | 6 |
| 9.3 | Plugin management UI | `Plugin/PluginUI.cs` | 🔜 | 6 |
| 9.4 | Scripting host | `Script.cs` (IronPython) | 🔜 | 6 |
| 9.5 | Swarm / formation control | `Swarm/` | 🔜 | 6 |
| 9.6 | Joystick + RC override mapping | `Joystick/JoystickSetup.cs` | 🔜 | 5 |
| 9.7 | Video stream / camera integration | `Controls/Video.cs`, `GimbalVideoControl.cs` | 🔜 | 5 |
| 9.8 | HTTP server (MJPEG / KML stream) | `ExtLibs/Utilities/httpserver.cs` | 🔜 | 7 |
| 9.9 | KML live output | `FlightPlanner.writeKML` | 🔜 | 2 |

## 10. Application-level

| # | Feature | Upstream location | Status | Phase |
| --- | --- | --- | --- | --- |
| 10.1 | Screen-switching shell | `ExtLibs/Controls/MainSwitcher.cs` | 🔜 | 1 |
| 10.2 | Theme system (light / dark / high-contrast) | `ThemeManager.cs`, `*.mpsystheme` | 🔜 | 7 |
| 10.3 | Internationalisation | `Strings.resx` + Crowdin | 🔜 | 7 |
| 10.4 | Unit system switching (metric / imperial) | `CurrentState.multiplier*` | 🔜 | 1 |
| 10.5 | Settings persistence | `ExtLibs/Utilities/Settings.cs` | 🔜 | 0 |
| 10.6 | Auto-updater | `Updater/` | 🔜 | 7 |
| 10.7 | Crash handler / diagnostics bundle | `Program.cs` | 🔜 | 7 |
| 10.8 | Windows / Linux / macOS packaging | `wix/`, `Msi/` | 🔜 | 7 |
| 10.9 | Code signing & notarisation | `wix/` | 🔜 | 7 |

---

## 11. Summary

| Status | Count |
| --- | --- |
| ✅ Done | 34 |
| 🔜 Planned | 116 |
| ⬜ Dropped | 1 |
| 🟡 Partial | 4 |

The 34 rows marked done are the connection, transport, parameter, mission and command
layers, and the flight data window that uses them. The transports, the heartbeat
negotiation, the vehicle registry, the loss accounting, the board-output capture, the
parameter protocol, the mission protocol, the command executor and the file formats all
work and are covered by tests.

The flight data window has upstream's distribution: a menu strip, a fixed column on the
left with the artificial horizon above a set of tabs - quick view, actions, messages,
gauges and status - the map filling everything else, and a status strip. The live readouts,
the attitude indicator, the map with the vehicle and the track, the plan drawn on it, the
dials, the tiles and the vehicle's own messages are all on screen.

The graphs are in the split above the map, as upstream has them, and they are drawn from
Mission Planner's own catalogue of presets: the expressions are evaluated over the
telemetry in the model, and the interface receives numbers with names.

What is still missing from the window: the parameter editor, the mission editor and the
pre-flight checklist.

The four rows marked partial are the fence, the connection profiles and the map.

The fence used to be impossible: `MAV_MISSION_TYPE` is a MAVLink 2 *extension* field, and
the generated bindings were built without the extension feature, so the plan type could not
travel. The bindings were never the limitation — the feature flag was. With
`mav2-message-extensions` switched on, the plan type reaches the wire on
`MISSION_REQUEST_LIST`, `MISSION_COUNT` and every item, and a fence plan downloads as a
fence plan. What is left of that row is the interface: drawing the polygon, and the
inclusion and exclusion lists, which are not modelled yet.

The connection profiles are stored, listed and opened at launch; what is missing is the
screen that edits them.

The map works, and it draws the vehicle, the track and the plan; what is missing is the
choice of the tile provider, which upstream offers as a registry of sources.

The single dropped feature is the Python LogAnalyzer integration (6.14) — it depends on a
Python 2 toolchain that is no longer maintainable. Its capability is covered by the native
graphing and analysis in 6.7–6.8, which is a strictly better outcome.

---

[Phase plan]: docs/PLAN.md
