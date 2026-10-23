import { LiveValue } from "../../components/LiveValue";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { formatCount } from "../../lib/format";
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
 */

/** The outputs and the radio channels a vehicle has. */
const CHANNELS = 8;

export function ServoPanel() {
  const t = useLocaleStore((state) => state.t);

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
    </div>
  );
}
