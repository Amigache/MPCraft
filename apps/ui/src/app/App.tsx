import { AppShell } from "./layout/AppShell";

/**
 * The root application component.
 *
 * Routing is added in phase 1; for now the shell renders the connection state so the
 * backend can be exercised end to end.
 */
export function App() {
  return <AppShell />;
}
