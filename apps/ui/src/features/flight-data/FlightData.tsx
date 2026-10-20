import { useEffect, useState } from "react";

import { listen } from "../../lib/ipc/bridge";

/** The telemetry the flight data view shows. */
interface Telemetry {
  position: { lat: number; lng: number; alt: number } | null;
  attitude: { roll: number; pitch: number; yaw: number } | null;
  velocity: { groundSpeed: number } | null;
  battery: { voltage: number; current: number; remaining: number } | null;
  mode: string | null;
  armed: boolean | null;
  distanceToHome: number | null;
}

/** The vehicles on the link. */
interface Vehicle {
  systemId: number;
  componentId: number;
  kind: string | null;
}

/** The loss statistics of the link. */
interface Loss {
  received: number;
  lost: number;
}

/**
 * The flight data view.
 *
 * Every value shown here comes from the backend: the frontend never derives a telemetry
 * value of its own.
 */
export function FlightData() {
  const [telemetry, setTelemetry] = useState<Telemetry | null>(null);
  const [vehicles, setVehicles] = useState<Vehicle[]>([]);
  const [loss, setLoss] = useState<Loss>({ received: 0, lost: 0 });

  useEffect(() => {
    const unlisten = listen<{
      status: string;
      vehicles: Vehicle[];
      telemetry: Telemetry;
      loss: Loss;
    }>("link-state", (event) => {
      setTelemetry(event.payload.telemetry);
      setVehicles(event.payload.vehicles);
      setLoss(event.payload.loss);
    });

    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  return (
    <section className="panel panel--flight">
      <header className="panel__header">
        <h2>Flight data</h2>
        {vehicles.length > 0 ? (
          <span className="panel__badge">
            {vehicles.map((v) => `${v.systemId}/${v.componentId}${v.kind ? ` ${v.kind}` : ""}`).join(", ")}
          </span>
        ) : null}
      </header>

      <div className="readouts">
        <Readout
          label="Latitude"
          value={telemetry?.position ? telemetry.position.lat.toFixed(7) : "—"}
          unit="°"
        />
        <Readout
          label="Longitude"
          value={telemetry?.position ? telemetry.position.lng.toFixed(7) : "—"}
          unit="°"
        />
        <Readout
          label="Altitude"
          value={telemetry?.position ? telemetry.position.alt.toFixed(1) : "—"}
          unit="m"
        />
        <Readout
          label="Speed"
          value={
            telemetry?.velocity ? telemetry.velocity.groundSpeed.toFixed(2) : "—"
          }
          unit="m/s"
        />
        <Readout
          label="Distance to home"
          value={
            telemetry?.distanceToHome != null
              ? telemetry.distanceToHome.toFixed(1)
              : "—"
          }
          unit="m"
        />
      </div>

      <div className="attitude">
        <AttitudeGauge
          label="Roll"
          value={telemetry?.attitude?.roll ?? null}
        />
        <AttitudeGauge
          label="Pitch"
          value={telemetry?.attitude?.pitch ?? null}
        />
        <AttitudeGauge label="Yaw" value={telemetry?.attitude?.yaw ?? null} />
      </div>

      <div className="readouts">
        <Readout label="Mode" value={telemetry?.mode ?? "—"} />
        <Readout
          label="Armed"
          value={
            telemetry?.armed == null ? "—" : telemetry.armed ? "yes" : "no"
          }
        />
        <Readout
          label="Battery"
          value={
            telemetry?.battery
              ? `${telemetry.battery.voltage.toFixed(2)} V · ${telemetry.battery.remaining.toFixed(0)} %`
              : "—"
          }
        />
        <Readout
          label="Packets"
          value={`${loss.received} received · ${loss.lost} lost`}
        />
      </div>
    </section>
  );
}

/** One labelled value. */
function Readout({
  label,
  value,
  unit,
}: {
  label: string;
  value: string;
  unit?: string;
}) {
  return (
    <div className="readout">
      <span className="readout__label">{label}</span>
      <span className="readout__value">
        {value}
        {unit ? <span className="readout__unit"> {unit}</span> : null}
      </span>
    </div>
  );
}

/** One attitude angle, drawn as a horizon line.
 *
 * The horizon tilts with the roll and moves with the pitch, which is how an attitude
 * indicator works.
 */
function AttitudeGauge({
  label,
  value,
}: {
  label: string;
  value: number | null;
}) {
  const roll = value ?? 0;
  const tilt = label === "Yaw" ? 0 : roll;

  return (
    <div className="attitude__gauge">
      <span className="readout__label">{label}</span>
      <div className="attitude__dial">
        <div
          className="attitude__horizon"
          style={{ transform: `rotate(${-tilt}deg) translateY(${label === "Pitch" ? (value ?? 0) * 1.6 : 0}px)` }}
        />
        <div className="attitude__needle" />
      </div>
      <span className="attitude__value">{value === null ? "—" : value.toFixed(1)}°</span>
    </div>
  );
}
