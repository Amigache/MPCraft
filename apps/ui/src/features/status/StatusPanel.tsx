import { useEffect, useState, type ReactNode } from "react";

import { LiveValue } from "../../components/LiveValue";
import { vehicleCoordinates } from "../../lib/ipc/bridge";
import {
  formatAltitude,
  formatCurrent,
  formatPercent,
  formatSpeed,
  formatText,
  formatVoltage,
  formatYesNo,
} from "../../lib/format";
import type { BatteryBank, Coordinates } from "../../lib/ipc/types";
import { useLinkStore } from "../../lib/link/linkStore";
import { useLiveValue } from "../../lib/link/liveTelemetry";
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
  const linked = status === "connected" || status === "reconnecting";

  // The battery banks are read from the live telemetry rather than from the state, so
  // that a bank the vehicle reports appears without the panel having to re-render.
  // The grid coordinates are made here, once, and not twenty times a second: the
  // projection is the same while the vehicle flies.
  const [coordinates, setCoordinates] = useState<Coordinates | null>(null);

  useEffect(() => {
    if (!linked) {
      setCoordinates(null);
      return;
    }

    let cancelled = false;

    const read = () => {
      void vehicleCoordinates()
        .then((found) => {
          if (!cancelled) {
            setCoordinates(found);
          }
        })
        .catch(() => {
          // The vehicle has not said where it is yet: the coordinates are simply
          // not there.
          if (!cancelled) {
            setCoordinates(null);
          }
        });
    };

    read();
    const timer = window.setInterval(read, 2000);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [linked]);

  const banks = useLiveValue(
    (telemetry) => telemetry.batteries,
    sameBanks,
  );

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
        <Row label="UTM">
          {coordinates?.utm === null || coordinates?.utm === undefined
            ? "—"
            : `${coordinates.utm.zone}${coordinates.utm.band} ${Math.round(
                coordinates.utm.easting,
              ).toLocaleString("en-US")} E ${Math.round(
                coordinates.utm.northing,
              ).toLocaleString("en-US")} N`}
        </Row>
        <Row label="MGRS">{coordinates?.mgrs ?? "—"}</Row>

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
        {banks.map((bank) => (
          <Row key={bank.id} label={`Bank ${bank.id + 1}`}>
            <BankCells id={bank.id} />
          </Row>
        ))}
      </section>

      <section className={styles.section}>
        <h3 className={styles.heading}>Estimator</h3>
        <Row label="Velocity">
          <LiveValue
            select={(telemetry) => telemetry.ekf?.velocityVariance ?? null}
            format={(value) => formatVariance(value as number)}
          />
        </Row>
        <Row label="Horizontal">
          <LiveValue
            select={(telemetry) => telemetry.ekf?.horizontalVariance ?? null}
            format={(value) => formatVariance(value as number)}
          />
        </Row>
        <Row label="Vertical">
          <LiveValue
            select={(telemetry) => telemetry.ekf?.verticalVariance ?? null}
            format={(value) => formatVariance(value as number)}
          />
        </Row>
        <Row label="Compass">
          <LiveValue
            select={(telemetry) => telemetry.ekf?.compassVariance ?? null}
            format={(value) => formatVariance(value as number)}
          />
        </Row>
      </section>

      <section className={styles.section}>
        <h3 className={styles.heading}>Vibration</h3>
        <Row label="X">
          <LiveValue
            select={(telemetry) => telemetry.vibration?.x ?? null}
            format={(value) => formatAcceleration(value as number)}
          />
        </Row>
        <Row label="Y">
          <LiveValue
            select={(telemetry) => telemetry.vibration?.y ?? null}
            format={(value) => formatAcceleration(value as number)}
          />
        </Row>
        <Row label="Z">
          <LiveValue
            select={(telemetry) => telemetry.vibration?.z ?? null}
            format={(value) => formatAcceleration(value as number)}
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

/**
 * The cells of one battery bank.
 *
 * The voltages drift, so they are written straight into the text of the element: a
 * battery that discharges twenty times a second must not re-render the panel twenty
 * times a second.
 */
function BankCells({ id }: { id: number }) {
  return (
    <LiveValue
      select={(telemetry) => {
        const bank = telemetry.batteries.find(
          (candidate) => candidate.id === id,
        );

        return bank === undefined
          ? null
          : `${bank.cells.map((cell) => cell.toFixed(2)).join(" ")} V  ${formatPercent(bank.remaining)}`;
      }}
      format={(value) => String(value)}
    />
  );
}

/**
 * Whether two sets of banks are the same.
 *
 * Only the shape matters: the banks themselves and how many cells each of them has.
 * The values drift, and they are written into the text of the rows.
 */
function sameBanks(left: readonly BatteryBank[], right: readonly BatteryBank[]) {
  return (
    left.length === right.length &&
    left.every(
      (bank, index) =>
        bank.id === right[index]?.id &&
        bank.cells.length === right[index]?.cells.length,
    )
  );
}

/** A variance, which is the square of a standard deviation. */
function formatVariance(value: number): string {
  return value.toFixed(3);
}

/** An acceleration, in metres per second squared. */
function formatAcceleration(value: number): string {
  return `${value.toFixed(2)} m/s²`;
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
