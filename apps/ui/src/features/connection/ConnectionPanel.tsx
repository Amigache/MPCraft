import { useConnection } from "../../lib/ipc/bridge";

/**
 * The connection indicator and the connect / disconnect controls.
 *
 * Errors are surfaced by their stable `code`, never by parsing the message text.
 */
export function ConnectionPanel() {
  const { status, error, connect, disconnect } = useConnection();

  return (
    <div className="connection-panel">
      <span className={`connection-panel__status connection-panel__status--${status}`}>
        {status}
      </span>

      {status === "disconnected" ? (
        <button type="button" onClick={() => void connect("udp:127.0.0.1:14550")}>
          Connect
        </button>
      ) : (
        <button type="button" onClick={() => void disconnect()}>
          Disconnect
        </button>
      )}

      {error ? (
        <span className="connection-panel__error" role="alert">
          {error.code}: {error.message}
        </span>
      ) : null}
    </div>
  );
}
