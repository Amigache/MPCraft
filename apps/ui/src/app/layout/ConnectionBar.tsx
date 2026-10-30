import { useCallback, useEffect, useState } from "react";

import {
  AUTO_BAUD,
  BAUD_RATES,
  DEFAULT_BAUD,
} from "../../features/connection/serialTarget";
import { listSerialPorts, useConnection } from "../../lib/ipc/bridge";
import type { SerialPort } from "../../lib/ipc/types";
import { useLocaleStore } from "../../lib/i18n/localeStore";
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

/** Where the bar remembers what was last chosen. */
const STORAGE_KEY = "mpcraft.connection";

/** What the bar remembers between two runs. */
interface Remembered {
  readonly method: Method;
  readonly port: string;
  /** The rate, or {@link AUTO_BAUD} when the station is left to find it. */
  readonly baud: number | typeof AUTO_BAUD;
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
        parsed.baud === AUTO_BAUD ||
        (typeof parsed.baud === "number" && parsed.baud > 0)
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
  const t = useLocaleStore((state) => state.t);

  /**
   * Reads the serial ports of the machine, and keeps reading them.
   *
   * A board is usually plugged in with the window already open, and a list read once kept saying
   * the machine had no ports while the one just connected was sitting there. It is a registry
   * read — nothing is opened — so it is done every two seconds, and the button beside the list
   * reads it again at once for an operator who does not want to wait.
   */
  const readPorts = useCallback(async () => {
    try {
      const found = await listSerialPorts();

      setPorts(found);
      setState((before) => ({
        ...before,
        // A port that is already chosen is left chosen: the list changing under the operator's
        // hand is not a reason to move the link somewhere else.
        port: before.port === "" ? (found[0]?.device ?? "") : before.port,
      }));
    } catch {
      setPorts([]);
    }
  }, []);

  useEffect(() => {
    void readPorts();

    const timer = window.setInterval(() => {
      void readPorts();
    }, 2000);

    return () => {
      window.clearInterval(timer);
    };
  }, [readPorts]);

  // What is chosen is kept between two runs.
  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  }, [state]);

  const linked = status === "connected" || status === "reconnecting";

  const target = (() => {
    switch (state.method) {
      case "serial":
        if (state.port === "") {
          return "";
        }
        // The rate is part of the target, and a station that is left to find it says so in the
        // option the parser reads — the number in front is only where the search starts.
        return state.baud === AUTO_BAUD
          ? `serial:${state.port}:${DEFAULT_BAUD}|baud=auto`
          : `serial:${state.port}:${state.baud}`;
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
        aria-label={t("The way of connecting")}
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
            {t(method.label)}
          </option>
        ))}
      </select>

      {state.method === "serial" ? (
        <>
          <select
            className={`select ${styles.port}`}
            value={state.port}
            aria-label={t("The port the vehicle is on")}
            disabled={linked}
            onChange={(event) => {
              setState((before) => ({ ...before, port: event.target.value }));
            }}
          >
            {ports.length === 0 ? (
              <option value="">{t("no ports")}</option>
            ) : null}
            {ports.map((port) => (
              <option key={port.device} value={port.device}>
                {port.device}
                {port.description ? ` — ${port.description}` : ""}
              </option>
            ))}
          </select>

          <button
            type="button"
            className={`button ${styles.refresh}`}
            title={t("Look for the serial ports again")}
            aria-label={t("Look for the serial ports again")}
            disabled={linked}
            onClick={() => {
              void readPorts();
            }}
          >
            ⟳
          </button>

          <select
            className={`select ${styles.baud}`}
            value={state.baud}
            aria-label={t("The speed of the port")}
            disabled={linked}
            onChange={(event) => {
              setState((before) => ({
                ...before,
                baud:
                  event.target.value === AUTO_BAUD
                    ? AUTO_BAUD
                    : Number(event.target.value),
              }));
            }}
          >
            <option value={AUTO_BAUD}>{t("Automatic")}</option>
            {BAUD_RATES.map((rate) => (
              <option key={rate} value={rate}>
                {rate}
              </option>
            ))}
          </select>
        </>
      ) : (
        <input
          className={`input ${styles.host}`}
          type="text"
          spellCheck={false}
          value={state.host}
          aria-label={t("The address of the vehicle")}
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
        aria-label={linked ? t("Close the link") : t("Open the link")}
        title={linked ? t("Close the link") : t("Open the link")}
        onClick={toggle}
      >
        {linked ? t("Disconnect") : t("Connect")}
      </button>
    </div>
  );
}
