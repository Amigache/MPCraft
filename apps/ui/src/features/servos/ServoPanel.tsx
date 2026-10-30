import { useCallback, useEffect, useState } from "react";

import { LiveValue } from "../../components/LiveValue";
import {
  normaliseError,
  requestRelayStatus,
  setRelay,
} from "../../lib/ipc/bridge";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { formatCount } from "../../lib/format";
import { useLiveValue } from "../../lib/link/liveTelemetry";
import { useLinkStore } from "../../lib/link/linkStore";
import { useActivityStore } from "../../lib/ui/activityStore";
import styles from "./ServoPanel.module.css";

/**
 * The outputs of the vehicle.
 *
 * The pulse width of each output and the value of each radio channel, the way upstream
 * shows them in its servo tab.
 *
 * A vehicle that does not report its outputs has none: the values are shown as a dash
 * rather than as a zero, because a servo at zero microseconds is a fault and not the
 * absence of a report.
 *
 * The **relays** are the part of this tab that can be acted on. The vehicle says which
 * relays it has — the report carries two masks, and the backend reads them into a list
 * before anything reaches here — so the rows are the relays the vehicle has, and a relay
 * it has not said it has cannot be asked for. The state shown is what the vehicle said,
 * not what was last clicked, because the vehicle can answer with a relay in another state
 * than the one that was asked for. The report is not streamed on its own, so the panel asks
 * for it once when the link is open; a station that is not connected asks nothing.
 */

/** The outputs and the radio channels a vehicle has. */
const CHANNELS = 8;

export function ServoPanel() {
  const t = useLocaleStore((state) => state.t);
  const report = useActivityStore((state) => state.report);

  // A station that is not connected cannot ask anything of a vehicle, so the request for the report
  // is made only when the link is open — and then once, because the vehicle does not say what its
  // relays are on its own. What the panel draws afterwards is what the event carries.
  const status = useLinkStore((state) => state.status);
  const vehicles = useLinkStore((state) => state.vehicles.length);
  // The report is the vehicle's, and a port with nobody on it has none to ask for.
  const open = status === "connected" && vehicles > 0;

  useEffect(() => {
    if (!open) {
      return;
    }

    void requestRelayStatus().catch((cause) => {
      report(normaliseError(cause), null);
    });
  }, [open, report]);

  const relays = useLiveValue((telemetry) => telemetry.relays ?? null);
  const [busy, setBusy] = useState(false);

  const turn = useCallback(
    async (instance: number, on: boolean) => {
      setBusy(true);

      try {
        await setRelay(instance, on);
        report(null, t("The relay was told what to do."));
      } catch (cause) {
        report(normaliseError(cause), null);
      } finally {
        setBusy(false);
      }
    },
    [report, t],
  );

  return (
    <div className={styles.panel}>
      <section className={styles.section}>
        <h3 className={styles.heading}>{t("Outputs")}</h3>
        <div className={styles.grid}>
          {Array.from({ length: CHANNELS }, (_, index) => (
            <div key={index} className={styles.row}>
              <span className={styles.label}>
                {t("Output {number}", { number: index + 1 })}
              </span>
              <LiveValue
                className={styles.value}
                select={(telemetry) =>
                  telemetry.controls?.outputs[index] ?? null
                }
                format={(value) => `${Math.round(value as number)} µs`}
              />
            </div>
          ))}
        </div>
      </section>

      <section className={styles.section}>
        <h3 className={styles.heading}>{t("Radio channels")}</h3>
        <div className={styles.grid}>
          {Array.from({ length: CHANNELS }, (_, index) => (
            <div key={index} className={styles.row}>
              <span className={styles.label}>
                {t("Channel {number}", { number: index + 1 })}
              </span>
              <LiveValue
                className={styles.value}
                select={(telemetry) =>
                  telemetry.controls?.inputs[index] ?? null
                }
                format={(value) => `${Math.round(value as number)} µs`}
              />
            </div>
          ))}
        </div>
      </section>

      <section className={styles.section}>
        <h3 className={styles.heading}>{t("Signal")}</h3>
        <div className={styles.row}>
          <span className={styles.label}>{t("Strength")}</span>
          <LiveValue
            className={styles.value}
            select={(telemetry) => telemetry.controls?.signal ?? null}
            format={(value) => formatCount(value as number)}
          />
          <span className={styles.unit}>%</span>
        </div>
      </section>

      <section className={styles.section}>
        <h3 className={styles.heading}>{t("Relays")}</h3>

        {relays === null || relays.length === 0 ? (
          <p className={styles.note}>
            {t(
              "Nothing has been said about the relays. The station asks for the report when the link is open.",
            )}
          </p>
        ) : (
          <div className={styles.grid}>
            {relays.map((relay) => (
              <div key={relay.number} className={styles.row}>
                {/* The number is the one the vehicle counts its relays with, which is the number
                    the command carries rather than a name the panel gives. */}
                <span className={styles.label}>
                  {t("Relay {number}", { number: relay.number })}
                </span>
                <span
                  className={`${styles.state} ${relay.on ? styles.on : ""}`}
                >
                  {relay.on ? t("on") : t("off")}
                </span>
                <span className={styles.actions}>
                  <button
                    type="button"
                    className="button"
                    disabled={busy || !open}
                    title={t("Turn relay {number} on", {
                      number: relay.number,
                    })}
                    onClick={() => {
                      void turn(relay.number, true);
                    }}
                  >
                    {t("Turn on")}
                  </button>
                  <button
                    type="button"
                    className="button"
                    disabled={busy || !open}
                    title={t("Turn relay {number} off", {
                      number: relay.number,
                    })}
                    onClick={() => {
                      void turn(relay.number, false);
                    }}
                  >
                    {t("Turn off")}
                  </button>
                </span>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
