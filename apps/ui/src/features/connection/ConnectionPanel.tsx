import { useState } from "react";

import { useConnection } from "../../lib/ipc/bridge";
import styles from "./ConnectionPanel.module.css";

/**
 * The connection controls.
 *
 * The target is a single string, the way the backend takes it: `serial:COM3:57600`,
 * `udp:127.0.0.1:14550`, `tcp:192.168.1.10:5760`. The panel is opened from the File
 * menu; the state of the link is always on the menu bar and in the status bar.
 */
export function ConnectionPanel() {
  const { status, error, connect, disconnect } = useConnection();
  const [target, setTarget] = useState("udp:127.0.0.1:14550");
  const [busy, setBusy] = useState(false);

  const open = status === "disconnected" || status === "failed";

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    try {
      await action();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={styles.panel}>
      <label className={styles.field}>
        <span className={styles.label}>Target</span>
        <input
          className="input"
          value={target}
          spellCheck={false}
          autoComplete="off"
          aria-label="Connection target"
          onChange={(event) => setTarget(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && open) {
              void run(() => connect(target));
            }
          }}
        />
      </label>

      <div className={styles.row}>
        <button
          type="button"
          className="button button--primary"
          disabled={busy || !open}
          onClick={() => void run(() => connect(target))}
        >
          Connect
        </button>
        <button
          type="button"
          className="button"
          disabled={busy || open}
          onClick={() => void run(() => disconnect())}
        >
          Disconnect
        </button>
        <span className={`${styles.status} ${styles[status] ?? ""}`}>
          {status}
        </span>
      </div>

      {error ? (
        <p className={styles.error} role="alert">
          <span className={styles.code}>{error.code}</span> {error.message}
        </p>
      ) : null}
    </div>
  );
}
