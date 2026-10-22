import { useCallback, useEffect, useState } from "react";

import {
  normaliseError,
  ntripStart,
  ntripStatus,
  ntripStop,
} from "../../lib/ipc/bridge";
import { useLinkStore } from "../../lib/link/linkStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./NtripSheet.module.css";

/**
 * The sheet of the corrections of a base station.
 *
 * A base station watches the satellites it can see and works out how far each of them
 * is, and it sends those corrections to a caster. The caster passes them to whoever
 * asks, and the way of asking is NTRIP.
 *
 * The corrections are for the receiver of the vehicle, and the way to give them to it
 * is the serial port of its GPS. The position of the vehicle travels with the request,
 * so the caster knows which corrections are wanted.
 *
 * Upstream has the same thing (`CommsNTRIP.cs`), and it asks the operator for the
 * address of the caster in the same way.
 */

/** The ports of the vehicle whose receiver can take the corrections. */
const GPS_PORTS = [
  { id: 2, label: "GPS 1" },
  { id: 3, label: "GPS 2" },
] as const;

export function NtripSheet() {
  const open = useUiStore((state) => state.ntripOpen);
  const hide = useUiStore((state) => state.hideNtrip);

  const status = useLinkStore((state) => state.status);
  const linked = status === "connected" || status === "reconnecting";

  const [url, setUrl] = useState("");
  const [gpsPort, setGpsPort] = useState(2);
  const [version1, setVersion1] = useState(false);

  const [running, setRunning] = useState(false);
  const [bytes, setBytes] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const read = useCallback(async () => {
    try {
      const state = await ntripStatus();
      setRunning(state.running);
      setBytes(state.bytes);
    } catch {
      // The state is only read to be shown; a failure is not worth a message of its
      // own, and the sheet says what it knows.
    }
  }, []);

  useEffect(() => {
    if (!open) {
      return;
    }

    void read();

    const timer = window.setInterval(() => {
      void read();
    }, 1000);

    return () => {
      window.clearInterval(timer);
    };
  }, [open, read]);

  if (!open) {
    return null;
  }

  const start = async () => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      await ntripStart(url, gpsPort, version1);
      setNotice("The corrections are being sent to the vehicle.");
      await read();
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  const stop = async () => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      await ntripStop();
      setNotice("The corrections have stopped.");
      await read();
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={styles.backdrop} role="presentation" onClick={hide}>
      <div
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-label="The corrections of a base station"
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>The corrections of a base station</h2>
          <button type="button" className="button" onClick={hide}>
            Close
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            A base station sends the corrections of the satellites it sees to a caster,
            and the caster passes them to whoever asks. The corrections go to the
            receiver of the vehicle, and the position of the vehicle travels with the
            request.
          </p>

          <label className={styles.wide}>
            <span className={styles.label}>The address of the caster</span>
            <input
              className="input"
              type="text"
              value={url}
              spellCheck={false}
              placeholder="http://user:password@host:port/mount"
              aria-label="The address of the caster"
              onChange={(event) => {
                setUrl(event.target.value);
              }}
            />
          </label>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>The port of the vehicle</span>
              <select
                className="select"
                value={gpsPort}
                aria-label="The port of the vehicle the corrections go to"
                onChange={(event) => {
                  setGpsPort(Number(event.target.value));
                }}
              >
                {GPS_PORTS.map((entry) => (
                  <option key={entry.id} value={entry.id}>
                    {entry.label}
                  </option>
                ))}
              </select>
            </label>

            <label className={styles.spline}>
              <input
                type="checkbox"
                checked={version1}
                aria-label="Ask the caster the old way, with the headers of the first version"
                onChange={(event) => {
                  setVersion1(event.target.checked);
                }}
              />
              <span>The first version</span>
            </label>
          </div>

          <div className={styles.row}>
            <button
              type="button"
              className="button button--primary"
              disabled={busy || running || !linked || url.trim() === ""}
              onClick={() => {
                void start();
              }}
            >
              Start
            </button>

            <button
              type="button"
              className="button"
              disabled={busy || !running}
              onClick={() => {
                void stop();
              }}
            >
              Stop
            </button>
          </div>

          {!linked ? (
            <p className={styles.text}>There is no link to a vehicle.</p>
          ) : null}
        </section>

        <section className={styles.block}>
          <dl className={styles.facts}>
            <dt>What it is doing</dt>
            <dd>{running ? "Sending the corrections" : "Not sending"}</dd>
            <dt>How many bytes</dt>
            <dd>{bytes}</dd>
          </dl>
        </section>

        {notice !== null ? (
          <p className={styles.notice} role="status">
            {notice}
          </p>
        ) : null}

        {error !== null ? (
          <p className={styles.error} role="alert">
            {error}
          </p>
        ) : null}
      </div>
    </div>
  );
}
