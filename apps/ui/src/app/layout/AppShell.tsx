import { ConnectionPanel } from "../features/connection/ConnectionPanel";

/**
 * The application chrome: the top bar, the connection indicator and the screen area.
 */
export function AppShell() {
  return (
    <div className="app-shell">
      <header className="app-shell__bar">
        <span className="app-shell__brand">MPCraft</span>
        <ConnectionPanel />
      </header>
      <main className="app-shell__body" />
    </div>
  );
}
