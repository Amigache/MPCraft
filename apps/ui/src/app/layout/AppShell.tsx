import { MenuBar } from "./MenuBar";
import { StatusBar } from "./StatusBar";
import { FlightData } from "../../features/flight-data/FlightData";
import styles from "./AppShell.module.css";

/**
 * The window: a menu bar, the screen, and a status bar.
 *
 * This is the shape Mission Planner's window has — a menu strip at the top and a status
 * strip at the bottom — and the screen in between is the flight data view.
 */
export function AppShell() {
  return (
    <div className={styles.shell}>
      <MenuBar />
      <main className={styles.body}>
        <FlightData />
      </main>
      <StatusBar />
    </div>
  );
}
