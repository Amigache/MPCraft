import { ConnectionPanel } from "../../features/connection/ConnectionPanel";
import { FlightData } from "../../features/flight-data/FlightData";
import { MessageConsole } from "../../features/messages/MessageConsole";

/**
 * The application chrome: the top bar, the connection controls and the screen area.
 */
export function AppShell() {
  return (
    <div className="app-shell">
      <header className="app-shell__bar">
        <span className="app-shell__brand">
          MPCraft<span className="app-shell__version">0.1.0</span>
        </span>
        <ConnectionPanel />
      </header>

      <main className="app-shell__body">
        <FlightData />
        <MessageConsole />
      </main>
    </div>
  );
}
