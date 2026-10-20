import { AppShell } from "./layout/AppShell";

/**
 * The root application component.
 *
 * Routing is added in phase 1; for now the shell renders the flight data view, which is
 * enough to see the link and the live telemetry working end to end.
 */
export function App() {
  return <AppShell />;
}
