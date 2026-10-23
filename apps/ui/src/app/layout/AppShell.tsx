import { useEffect } from "react";

import { useUiStore } from "../../lib/ui/uiStore";
import { MenuBar } from "./MenuBar";
import { ScreenSwitcher } from "./ScreenSwitcher";
import { StatusBar } from "./StatusBar";
import { FlightData } from "../../features/flight-data/FlightData";
import { CalibrationSheet } from "../../features/calibration/CalibrationSheet";
import { JoystickSheet } from "../../features/joystick/JoystickSheet";
import { FirmwareSheet } from "../../features/firmware/FirmwareSheet";
import { FilesSheet } from "../../features/files/FilesSheet";
import { OsdSheet } from "../../features/osd/OsdSheet";
import { NtripSheet } from "../../features/ntrip/NtripSheet";
import { TransponderSheet } from "../../features/transponder/TransponderSheet";
import { AdsbSheet } from "../../features/adsb/AdsbSheet";
import { CameraSheet } from "../../features/camera/CameraSheet";
import { PictureSheet } from "../../features/pictures/PictureSheet";
import { PluginSheet } from "../../features/plugins/PluginSheet";
import { VideoSheet } from "../../features/video/VideoSheet";
import { PassthroughSheet } from "../../features/passthrough/PassthroughSheet";
import { TerrainSheet } from "../../features/terrain/TerrainSheet";
import { LogsScreen } from "../../features/logs/LogsScreen";
import { ParametersScreen } from "../../features/params/ParametersScreen";
import styles from "./AppShell.module.css";

/**
 * The window: a menu bar, the switcher of the screens, the screen, and a status bar.
 *
 * This is the shape Mission Planner's window has — a menu strip at the top and a status
 * strip at the bottom — and between them the switcher chooses which of the four screens
 * fills the body: the flight data, the plan, the configuration, or the logs.
 *
 * Every screen stays mounted, so that a log that was opened or a search that was typed
 * is not forgotten when the operator moves to another screen; each screen draws nothing
 * while the window is on a different one.
 *
 * The short tasks are not screens. They are sheets that lie over the whole window — a
 * calibration, a joystick mapping, a file transfer — and while one of them is open the
 * window is not there for the person: its controls cannot be reached, and a screen reader
 * is told so. A screen does not cover the window; a screen is the window.
 */
export function AppShell() {
  const theme = useUiStore((state) => state.theme);
  const screen = useUiStore((state) => state.screen);

  // The theme is a property of the document, not of a component: the whole
  // interface reads it, and the sheets over the window are of it too.
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  const covered = useUiStore(
    (state) =>
      state.calibrationOpen ||
      state.joystickOpen ||
      state.terrainOpen ||
      state.passthroughOpen ||
      state.firmwareOpen ||
      state.filesOpen ||
      state.osdOpen ||
      state.ntripOpen ||
      state.transponderOpen ||
      state.adsbOpen ||
      state.videoOpen ||
      state.cameraOpen ||
      state.picturesOpen ||
      state.pluginsOpen,
  );

  return (
    <div className={styles.shell}>
      <div
        className={styles.under}
        inert={covered}
        aria-hidden={covered ? true : undefined}
      >
        <MenuBar />
        <ScreenSwitcher />
        <main
          className={styles.body}
          role="tabpanel"
          id={`panel-${screen}`}
          aria-labelledby={`tab-${screen}`}
        >
          <FlightData />
          <ParametersScreen />
          <LogsScreen />
        </main>
        <StatusBar />
      </div>
      <CalibrationSheet />
      <JoystickSheet />
      <TerrainSheet />
      <PassthroughSheet />
      <FirmwareSheet />
      <FilesSheet />
      <OsdSheet />
      <NtripSheet />
      <TransponderSheet />
      <AdsbSheet />
      <VideoSheet />
      <CameraSheet />
      <PictureSheet />
      <PluginSheet />
    </div>
  );
}
