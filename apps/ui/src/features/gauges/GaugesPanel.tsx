import type { Telemetry } from "../../lib/ipc/types";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { Gauge } from "./Gauge";
import styles from "./GaugesPanel.module.css";

/**
 * The four dials upstream keeps in its gauges tab: the ground speed, the altitude, the
 * vertical speed and the heading.
 */

/** The ground speed, in metres per second. */
function selectSpeed(telemetry: Telemetry): number | null {
  return telemetry.velocity?.groundSpeed ?? null;
}

/** The altitude above mean sea level, in metres. */
function selectAltitude(telemetry: Telemetry): number | null {
  return telemetry.position?.alt ?? null;
}

/**
 * The climb rate, in metres per second.
 *
 * The velocity arrives in the north-east-down frame, in which the descent is positive;
 * a climb rate is the other way up.
 */
function selectVerticalSpeed(telemetry: Telemetry): number | null {
  const down = telemetry.velocity?.down;
  return typeof down === "number" ? -down : null;
}

/** The heading, in degrees. */
function selectHeading(telemetry: Telemetry): number | null {
  return telemetry.attitude?.yaw ?? null;
}

export function GaugesPanel() {
  const t = useLocaleStore((state) => state.t);

  return (
    <div className={styles.grid}>
      <div className={styles.cell}>
        <Gauge
          label={t("Speed")}
          unit="m/s"
          select={selectSpeed}
          min={0}
          max={30}
        />
      </div>
      <div className={styles.cell}>
        <Gauge
          label={t("Altitude")}
          unit="m"
          select={selectAltitude}
          min={0}
          max={1000}
        />
      </div>
      <div className={styles.cell}>
        <Gauge
          label={t("V-speed")}
          unit="m/s"
          select={selectVerticalSpeed}
          min={-10}
          max={10}
          digits={2}
        />
      </div>
      <div className={styles.cell}>
        <Gauge
          label={t("Heading")}
          unit="deg"
          select={selectHeading}
          min={0}
          max={360}
          digits={0}
          kind="compass"
        />
      </div>
    </div>
  );
}
