import type { ReactNode } from "react";

import { LiveValue } from "../../components/LiveValue";
import {
  formatAltitude,
  formatCurrent,
  formatPercent,
  formatSpeed,
  formatText,
  formatVoltage,
  formatYesNo,
} from "../../lib/format";
import { useLinkStore } from "../../lib/link/linkStore";
import { currentItemNumber, usePlanStore } from "../../lib/plan/planStore";
import styles from "./StatusPanel.module.css";

/**
 * The status tab: what the vehicle is and what the link is doing.
 *
 * Every value is read from the backend. A value the vehicle has not reported is shown as
 * a dash, because a ground station that shows a zero where it knows nothing is a ground
 * station that lies.
 */
export function StatusPanel() {
  const vehicles = useLinkStore((state) => state.vehicles);
  const loss = useLinkStore((state) => state.loss);
  const status = useLinkStore((state) => state.status);
  const plan = usePlanStore((state) => state.plans.mission);

  const vehicle = vehicles[0] ?? null;
  const item = currentItemNumber(plan);

  return (
    <div className={styles.panel}>
      <section className={styles.section}>
        <h3 className={styles.heading}>Vehicle</h3>
        <Row label="System">{vehicle ? vehicle.systemId : "—"}</Row>
        <Row label="Component">{vehicle ? vehicle.componentId : "—"}</Row>
        <Row label="Family">{formatText(vehicle?.kind ?? null)}</Row>
        <Row label="Mode">
          <LiveValue select={(telemetry) => telemetry.mode} />
        </Row>
        <Row label="Armed">
          <LiveValue
            select={(telemetry) => telemetry.armed}
            format={(value) => formatYesNo(value as boolean)}
          />
        </Row>
      </section>

      <section className={styles.section}>
        <h3 className={styles.heading}>Global positioning</h3>
        <Row label="Fix">
          <LiveValue
            select={(telemetry) => telemetry.gps?.fixType ?? null}
            format={(value) => formatFixType(value as number)}
          />
        </Row>
        <Row label="Satellites">
          <LiveValue
            select={(telemetry) => telemetry.gps?.satellites ?? null}
            format={(value) => (value as number).toString()}
          />
        </Row>
        <Row label="HDOP">
          <LiveValue
            select={(telemetry) => telemetry.gps?.hdop ?? null}
            format={(value) => (value as number).toFixed(2)}
          />
        </Row>
        <Row label="Latitude">
          <LiveValue
            select={(telemetry) => telemetry.position?.lat ?? null}
            format={(value) => `${(value as number).toFixed(6)}°`}
          />
        </Row>
        <Row label="Longitude">
          <LiveValue
            select={(telemetry) => telemetry.position?.lng ?? null}
            format={(value) => `${(value as number).toFixed(6)}°`}
          />
        </Row>
      </section>

      <section className={styles.section}>
        <h3 className={styles.heading}>Battery</h3>
        <Row label="Voltage">
          <LiveValue
            select={(telemetry) => telemetry.battery?.voltage ?? null}
            format={(value) => formatVoltage(value as number)}
          />
        </Row>
        <Row label="Current">
          <LiveValue
            select={(telemetry) => telemetry.battery?.current ?? null}
            format={(value) => formatCurrent(value as number)}
          />
        </Row>
        <Row label="Remaining">
          <LiveValue
            select={(telemetry) => telemetry.battery?.remaining ?? null}
            format={(value) => formatPercent(value as number)}
          />
        </Row>
        <Row label="Altitude">
          <LiveValue
            select={(telemetry) => telemetry.position?.alt ?? null}
            format={(value) => formatAltitude(value as number)}
          />
        </Row>
        <Row label="Ground speed">
          <LiveValue
            select={(telemetry) => telemetry.velocity?.groundSpeed ?? null}
            format={(value) => formatSpeed(value as number)}
          />
        </Row>
      </section>

      <section className={styles.section}>
        <h3 className={styles.heading}>Plan</h3>
        <Row label="Item">
          {item === null ? "—" : `${item} of ${plan?.items.length ?? 0}`}
        </Row>
        <Row label="Items">{plan ? plan.items.length : "—"}</Row>
      </section>

      <section className={styles.section}>
        <h3 className={styles.heading}>Link</h3>
        <Row label="State">{status}</Row>
        <Row label="Received">{loss.received.toLocaleString("en-US")}</Row>
        <Row label="Lost">{loss.lost.toLocaleString("en-US")}</Row>
      </section>
    </div>
  );
}

/** One labelled value. */
function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={styles.row}>
      <span className={styles.rowLabel}>{label}</span>
      <span className={styles.rowValue}>{children}</span>
    </div>
  );
}

/** The name of the GPS fix, as the MAVLink specification calls it. */
function formatFixType(value: number): string {
  switch (value) {
    case 0:
      return "no GPS";
    case 1:
      return "no fix";
    case 2:
      return "2D";
    case 3:
      return "3D";
    case 4:
      return "DGPS";
    case 5:
      return "RTK float";
    case 6:
      return "RTK fixed";
    case 7:
      return "static";
    case 8:
      return "PPP";
    default:
      return `fix ${value}`;
  }
}
