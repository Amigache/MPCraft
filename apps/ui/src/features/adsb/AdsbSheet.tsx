import { useCallback, useEffect, useState } from "react";

import {
  adsbRelayStart,
  adsbRelayStatus,
  adsbRelayStop,
  normaliseError,
} from "../../lib/ipc/bridge";
import type { GeoPoint } from "../../lib/ipc/types";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useLinkStore } from "../../lib/link/linkStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./AdsbSheet.module.css";

/**
 * The sheet of the traffic of the air.
 *
 * The vehicle tells the ground station what its receiver hears, and the station draws that
 * on the map. But the station can hear the traffic somewhere else as well — from an API that
 * publishes what a network of receivers has seen — and then those aircraft are not on the
 * link, so the other stations listening to this link do not see them.
 *
 * The relay is what puts them on the link: once a second it takes the aircraft that came from
 * its own source, keeps the ten closest to the vehicle, and sends one of them, going through
 * the list in order. What the vehicle already sent is not sent back, because that would say
 * the same thing twice.
 *
 * Upstream does the same (`MainV2.cs`, `ADSBRunner`), with the same rule of the ten closest,
 * one second apart.
 */

/** The source the traffic is read from when the operator has not chosen one. */
const DEFAULT_SERVER = "https://api.adsb.lol";

/** How far round the vehicle the list is asked for, in nautical miles. */
const DEFAULT_RADIUS_NM = 100;

export function AdsbSheet() {
  const open = useUiStore((state) => state.adsbOpen);
  const hide = useUiStore((state) => state.hideAdsb);

  const t = useLocaleStore((state) => state.t);

  const status = useLinkStore((state) => state.status);
  const linked = status === "connected" || status === "reconnecting";

  const [server, setServer] = useState(DEFAULT_SERVER);
  const [radius, setRadius] = useState(String(DEFAULT_RADIUS_NM));

  const [running, setRunning] = useState(false);
  const [sent, setSent] = useState(0);
  const [index, setIndex] = useState(0);
  const [aircraft, setAircraft] = useState(0);
  const [position, setPosition] = useState<GeoPoint | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const read = useCallback(async () => {
    try {
      const state = await adsbRelayStatus();
      setRunning(state.running);
      setSent(state.sent);
      setIndex(state.index);
      setAircraft(state.aircraft);
      setPosition(state.position);
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

    const address = server.trim();

    if (
      address !== "" &&
      !address.startsWith("http://") &&
      !address.startsWith("https://")
    ) {
      setBusy(false);
      setError(
        t(
          "The address of the source is not one that can be used. MPCraft reads the traffic from an HTTP address; a receiver that speaks over a plain connection is not read.",
        ),
      );
      return;
    }

    const distance = Number.parseInt(radius.trim(), 10);

    if (!Number.isFinite(distance) || distance < 1) {
      setBusy(false);
      setError(
        t("The distance round the vehicle is not one that can be used."),
      );
      return;
    }

    try {
      await adsbRelayStart(address, distance);
      setNotice(
        t("The aircraft the station reads are being sent onto the link."),
      );
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
      await adsbRelayStop();
      setNotice(t("The relay has been stopped."));
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
        aria-label={t("The traffic of the air")}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>{t("The traffic of the air")}</h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "The aircraft the vehicle sent are already on the link. What the station reads from a source of its own is added to it, one each second, from the ten closest to the vehicle, so that the other stations listening here see them too.",
            )}
          </p>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>
                {t("The address of the source")}
              </span>
              <input
                className="input"
                type="text"
                value={server}
                spellCheck={false}
                placeholder="https://api.adsb.lol"
                aria-label={t("The address of the source of the traffic")}
                onChange={(event) => {
                  setServer(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>
                {t("How far round the vehicle")}
              </span>
              <input
                className="input"
                type="number"
                min={1}
                max={250}
                value={radius}
                aria-label={t(
                  "How far round the vehicle the list is asked for, in nautical miles",
                )}
                onChange={(event) => {
                  setRadius(event.target.value);
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
              {running
                ? t("Sending one aircraft each second")
                : t("Not sending")}
            </dd>
            <dt>{t("How many aircraft the station reads")}</dt>
            <dd>{aircraft}</dd>
            <dt>{t("How many have been sent")}</dt>
            <dd>{sent}</dd>
            <dt>{t("Which place of the list comes next")}</dt>
            <dd>{index}</dd>
            <dt>{t("Where the vehicle is")}</dt>
            <dd>
              {position !== null
                ? `${position.lat.toFixed(4)}, ${position.lng.toFixed(4)}`
                : t("the vehicle has not said")}
            </dd>
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
