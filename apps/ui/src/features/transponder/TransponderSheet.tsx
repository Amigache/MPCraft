import { useCallback, useEffect, useState } from "react";

import {
  normaliseError,
  transponderStart,
  transponderStatus,
  transponderStop,
} from "../../lib/ipc/bridge";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useLinkStore } from "../../lib/link/linkStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./TransponderSheet.module.css";

/**
 * The sheet of the transponder.
 *
 * A vehicle that is seen by the traffic of the air needs to say where it is: the
 * transponder is the thing that does it, and the ground station is what tells the
 * vehicle what to say. The vehicle relays the message, and the aircraft around it see
 * it.
 *
 * Upstream has the same thing (`FlightData.cs`, the transponder tab), with the same
 * fields: the address of the aircraft, the name of the flight and the code.
 */

/** The code the operator chooses by default. */
const DEFAULT_SQUAWK = 1200;

export function TransponderSheet() {
  const open = useUiStore((state) => state.transponderOpen);
  const hide = useUiStore((state) => state.hideTransponder);
  const t = useLocaleStore((state) => state.t);

  const status = useLinkStore((state) => state.status);
  const linked = status === "connected" || status === "reconnecting";

  const [address, setAddress] = useState("ABC123");
  const [flightId, setFlightId] = useState("");
  const [squawk, setSquawk] = useState(String(DEFAULT_SQUAWK));

  const [running, setRunning] = useState(false);
  const [sent, setSent] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const read = useCallback(async () => {
    try {
      const state = await transponderStatus();
      setRunning(state.running);
      setSent(state.sent);
    } catch {
      // The state is only read to be shown.
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

    // The address is written in hexadecimal, the way it is written in the aircraft.
    const parsed = Number.parseInt(address.trim(), 16);

    if (!Number.isFinite(parsed) || parsed < 0 || parsed > 0x00ff_ffff) {
      setBusy(false);
      setError(t("The address of the aircraft is not one that can be used."));
      return;
    }

    const code = Number.parseInt(squawk.trim(), 10);

    if (!Number.isFinite(code) || code < 0 || code > 7777) {
      setBusy(false);
      setError(t("The code is not one that can be used."));
      return;
    }

    try {
      await transponderStart(parsed, flightId.trim().toUpperCase(), code);
      setNotice(t("The vehicle is being told what its transponder is to say."));
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
      await transponderStop();
      setNotice(t("The transponder has been left alone."));
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
        aria-label={t("The transponder")}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>{t("The transponder")}</h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "The vehicle is told where it is and what to say, and it relays the message. The aircraft around it see it.",
            )}
          </p>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>
                {t("The address of the aircraft")}
              </span>
              <input
                className="input"
                type="text"
                value={address}
                spellCheck={false}
                placeholder="ABC123"
                aria-label={t("The address of the aircraft, in hexadecimal")}
                onChange={(event) => {
                  setAddress(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>
                {t("The name of the flight")}
              </span>
              <input
                className="input"
                type="text"
                value={flightId}
                spellCheck={false}
                maxLength={9}
                placeholder="MPCRAFT"
                aria-label={t("The name of the flight")}
                onChange={(event) => {
                  setFlightId(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("The code")}</span>
              <input
                className="input"
                type="text"
                value={squawk}
                spellCheck={false}
                placeholder="1200"
                aria-label={t("The code of the transponder, in octal")}
                onChange={(event) => {
                  setSquawk(event.target.value);
                }}
              />
            </label>
          </div>

          <div className={styles.row}>
            <button
              type="button"
              className="button button--primary"
              disabled={busy || running || !linked}
              onClick={() => {
                void start();
              }}
            >
              {t("Start")}
            </button>

            <button
              type="button"
              className="button"
              disabled={busy || !running}
              onClick={() => {
                void stop();
              }}
            >
              {t("Stop")}
            </button>
          </div>

          {!linked ? (
            <p className={styles.text}>{t("There is no link to a vehicle.")}</p>
          ) : null}
        </section>

        <section className={styles.block}>
          <dl className={styles.facts}>
            <dt>{t("What it is doing")}</dt>
            <dd>
              {running ? t("Saying where the vehicle is") : t("Not saying")}
            </dd>
            <dt>{t("How many times")}</dt>
            <dd>{sent}</dd>
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
