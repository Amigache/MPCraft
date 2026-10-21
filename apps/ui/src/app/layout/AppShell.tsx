import { useUiStore } from "../../lib/ui/uiStore";
import { MenuBar } from "./MenuBar";
import { StatusBar } from "./StatusBar";
import { FlightData } from "../../features/flight-data/FlightData";
import { ParameterSheet } from "../../features/params/ParameterSheet";
import styles from "./AppShell.module.css";

/**
 * The window: a menu bar, the screen, and a status bar.
 *
 * This is the shape Mission Planner's window has — a menu strip at the top and a status
 * strip at the bottom — and the screen in between is the flight data view.
 *
 * The sheets lie over the whole window: the parameters are opened from the Setup menu
 * and cover the screen rather than opening a window of their own.
 */
export function AppShell() {
  // While a sheet lies over the window, the window is not there for the person:
  // its controls cannot be reached, and a screen reader is told so.
  const covered = useUiStore((state) => state.parametersOpen);

  return (
    <div className={styles.shell}>
      <div
        className={styles.under}
        inert={covered}
        aria-hidden={covered ? true : undefined}
      >
        <MenuBar />
        <main className={styles.body}>
          <FlightData />
        </main>
        <StatusBar />
      </div>
      <ParameterSheet />
    </div>
  );
}
