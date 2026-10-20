import { useEffect, useState } from "react";

import { call, listen, normaliseError } from "../../lib/ipc/bridge";
import type { ConnectionStatus } from "../../lib/ipc/types";

/**
 * The connection controls.
 *
 * The status comes from the backend as an event, and the errors are surfaced by their stable
 * code rather than by their message.
 */
export function ConnectionPanel() {
  const [target, setTarget] = useState("udp:127.0.0.1:14550");
  const [status, setStatus] = useState<ConnectionStatus>("disconnected");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const unlisten = listen<{ status: ConnectionStatus }>("link-state", (event) => {
      setStatus(event.payload.status);
    });

    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  const connect = async () => {
    setBusy(true);
    setError(null);
    try {
      await call<unknown>("connect", { req: { target } });
    } catch (cause) {
      const error = normaliseError(cause);
      setError(`${error.code}: ${error.message}`);
    } finally {
      setBusy(false);
    }
  };

  const disconnect = async () => {
    setBusy(true);
    setError(null);
    try {
      await call<unknown>("disconnect");
    } catch (cause) {
      const error = normaliseError(cause);
      setError(`${error.code}: ${error.message}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="connection-panel">
      <input
        className="connection-panel__target"
        value={target}
        spellCheck={false}
        aria-label="Connection target"
        onChange={(event) => setTarget(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            void connect();
          }
        }}
      />

      {status === "disconnected" || status === "failed" ? (
        <button
          type="button"
          className="button button--primary"
          disabled={busy}
          onClick={() => void connect()}
        >
          Connect
        </button>
      ) : (
        <button
          type="button"
          className="button"
          disabled={busy}
          onClick={() => void disconnect()}
        >
          Disconnect
        </button>
      )}

      <span className={`status status--${status}`}>{status}</span>

      {error ? (
        <span className="connection-panel__error" role="alert">
          {error}
        </span>
      ) : null}
    </div>
  );
}
