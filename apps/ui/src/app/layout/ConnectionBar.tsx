import { useEffect, useState } from "react";

import { listSerialPorts, useConnection } from "../../lib/ipc/bridge";
import type { SerialPort } from "../../lib/ipc/types";
import styles from "./ConnectionBar.module.css";

/**
 * The connection bar.
 *
 * Mission Planner keeps the link in the top right of the window: the way of connecting,
 * the port, and the button that opens or closes the link. It is the first thing an
 * operator touches, and it is the last thing they stop touching, so it is worth having
 * it in front of them at all times.
 *
 * The target is a single string, the way the backend takes it: `serial:COM3:57600`,
 * `udp:127.0.0.1:14550`, `tcp:192.168.1.10:5760`. The bar builds it from the pieces
 * the operator chooses.
 */

/** The ways of connecting. */
type Method = "serial" | "udp" | "tcp";

/** The label of each way of connecting. */
const METHODS: readonly { id: Method; label: string }[] = [
  { id: "serial", label: "Serial" },
  { id: "udp", label: "UDP" },
  { id: "tcp", label: "TCP" },
];

/** The baud rate that is used when nothing else is said. */
const DEFAULT_BAUD = 57600;

/** Where the bar remembers what was last chosen. */
const STORAGE_KEY = "mpcraft.connection";

/** What the bar remembers between two runs. */
interface Remembered {
  readonly method: Method;
  readonly port: string;
  readonly baud: number;
  readonly host: string;
}

/** Reads what the bar remembers, when there is something to read. */
function remembered(): Remembered {
  const fallback: Remembered = {
    method: "serial",
    port: "",
    baud: DEFAULT_BAUD,
    host: "127.0.0.1:14550",
  };

  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (raw === null) {
      return fallback;
    }

    const parsed = JSON.parse(raw) as Partial<Remembered>;

    return {
      method: METHODS.some((method) => method.id === parsed.method)
        ? (parsed.method as Method)
        : fallback.method,
      port: typeof parsed.port === "string" ? parsed.port : fallback.port,
      baud:
        typeof parsed.baud === "number" && parsed.baud > 0
          ? parsed.baud
          : fallback.baud,
      host: typeof parsed.host === "string" ? parsed.host : fallback.host,
    };
  } catch {
    return fallback;
  }
}

export function ConnectionBar() {
  const { status, connect, disconnect } = useConnection();
  const [state, setState] = useState<Remembered>(remembered);
  const [ports, setPorts] = useState<readonly SerialPort[]>([]);
  const [busy, setBusy] = useState(false);

  // The serial ports of the machine are read once: they do not change while the
  // application is open, and reading them opens the ports of the system.
  useEffect(() => {
    let cancelled = false;

    void listSerialPorts()
      .then((found) => {
        if (cancelled) {
          return;
        }

        setPorts(found);
        setState((before) => ({
          ...before,
          port: before.port === "" ? (found[0]?.device ?? "") : before.port,
        }));
      })
      .catch(() => {
        if (!cancelled) {
          setPorts([]);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  // What is chosen is kept between two runs.
  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  }, [state]);

  const linked = status === "connected" || status === "reconnecting";

  const target = (() => {
    switch (state.method) {
      case "serial":
        return state.port === "" ? "" : `serial:${state.port}:${state.baud}`;
      case "udp":
        return `udp:${state.host}`;
      case "tcp":
        return `tcp:${state.host}`;
    }
  })();

  const toggle = () => {
    setBusy(true);

    const action = linked ? disconnect() : connect(target);

    void action
      .catch(() => {
        // The failure is already in the state of the link, and the bar shows it.
      })
      .finally(() => {
        setBusy(false);
      });
  };

  return (
    <div className={styles.bar}>
      <select
        className={`select ${styles.method}`}
        value={state.method}
        aria-label="The way of connecting"
        disabled={linked}
        onChange={(event) => {
          setState((before) => ({
            ...before,
            method: event.target.value as Method,
          }));
        }}
      >
        {METHODS.map((method) => (
          <option key={method.id} value={method.id}>
            {method.label}
          </option>
        ))}
      </select>

      {state.method === "serial" ? (
        <>
          <select
            className={`select ${styles.port}`}
            value={state.port}
            aria-label="The port the vehicle is on"
            disabled={linked}
            onChange={(event) => {
              setState((before) => ({ ...before, port: event.target.value }));
            }}
          >
            {ports.length === 0 ? <option value="">no ports</option> : null}
            {ports.map((port) => (
              <option key={port.device} value={port.device}>
                {port.device}
                {port.description ? ` — ${port.description}` : ""}
              </option>
            ))}
          </select>

          <input
            className={`input ${styles.baud}`}
            type="number"
            min={1}
            value={state.baud}
            aria-label="The speed of the port"
            disabled={linked}
            onChange={(event) => {
              setState((before) => ({
                ...before,
                baud: Number(event.target.value) || DEFAULT_BAUD,
              }));
            }}
          />
        </>
      ) : (
        <input
          className={`input ${styles.host}`}
          type="text"
          spellCheck={false}
          value={state.host}
          aria-label="The address of the vehicle"
          disabled={linked}
          onChange={(event) => {
            setState((before) => ({ ...before, host: event.target.value }));
          }}
        />
      )}

      <button
        type="button"
        className={`button ${linked ? styles.disconnect : styles.connect}`}
        disabled={busy || (!linked && target === "")}
        aria-label={linked ? "Close the link" : "Open the link"}
        title={linked ? "Close the link" : "Open the link"}
        onClick={toggle}
      >
        {linked ? "Disconnect" : "Connect"}
      </button>
    </div>
  );
}
