import { useEffect, useId, useRef } from "react";

import type { Telemetry } from "../../lib/ipc/types";
import { liveTelemetry } from "../../lib/link/liveTelemetry";
import { formatNumber } from "../../lib/format";
import styles from "./Gauge.module.css";

/**
 * One dial.
 *
 * A thin arc, a needle and a digital readout. The needle and the readout are written
 * directly when a snapshot arrives, so a dial that moves twenty times a second costs
 * two attribute writes rather than a re-render.
 */

/** The size of the drawing. */
const VIEW = 120;

/** The centre of the dial. */
const CENTRE = 60;

/** The radius of the scale. */
const RADIUS = 46;

/** Where the arc starts and ends, in degrees, with zero at the top. */
const ARC_START = 135;
const ARC_END = 405;

/** A point on a circle, with zero at the top and the angle growing clockwise. */
function polar(radius: number, angle: number): { x: number; y: number } {
  const radians = ((angle - 90) * Math.PI) / 180;
  return {
    x: CENTRE + radius * Math.cos(radians),
    y: CENTRE + radius * Math.sin(radians),
  };
}

/** The path of an arc. */
function arc(radius: number, from: number, to: number): string {
  const start = polar(radius, from);
  const end = polar(radius, to);
  const large = Math.abs(to - from) > 180 ? 1 : 0;
  return `M ${start.x.toFixed(2)} ${start.y.toFixed(2)} A ${radius} ${radius} 0 ${large} 1 ${end.x.toFixed(2)} ${end.y.toFixed(2)}`;
}

export interface GaugeProps {
  /** What the dial measures. */
  readonly label: string;
  /** The unit of the reading. */
  readonly unit: string;
  /** Picks the value out of a snapshot. */
  readonly select: (telemetry: Telemetry) => number | null | undefined;
  /** The smallest reading the scale shows. */
  readonly min: number;
  /** The largest reading the scale shows. */
  readonly max: number;
  /** How many decimals the readout has. */
  readonly digits?: number;
  /**
   * A plain scale, or a compass rose.
   *
   * A compass rose points at the heading and carries the cardinal points, which is how
   * a direction is read; a plain scale is read against the ends of its arc.
   */
  readonly kind?: "scale" | "compass";
}

export function Gauge({
  label,
  unit,
  select,
  min,
  max,
  digits = 1,
  kind = "scale",
}: GaugeProps) {
  const titleId = useId();
  const needle = useRef<SVGGElement | null>(null);
  const readout = useRef<SVGTextElement | null>(null);
  const accessible = useRef<SVGTextElement | null>(null);

  useEffect(() => {
    const draw = (telemetry: Telemetry) => {
      const value = select(telemetry);
      const known = typeof value === "number" && Number.isFinite(value);

      let angle: number;
      if (kind === "compass") {
        angle = known ? (((value as number) % 360) + 360) % 360 : 0;
      } else {
        const fraction = known
          ? Math.min(1, Math.max(0, ((value as number) - min) / (max - min)))
          : 0;
        angle = ARC_START + fraction * (ARC_END - ARC_START);
      }

      needle.current?.setAttribute(
        "transform",
        `rotate(${angle.toFixed(2)} ${CENTRE} ${CENTRE})`,
      );

      const text = known ? formatNumber(value, digits) : "—";
      if (readout.current && readout.current.textContent !== text) {
        readout.current.textContent = text;
      }
      if (accessible.current) {
        accessible.current.textContent = known
          ? `${label}: ${text} ${unit}`
          : `${label}: unknown`;
      }
    };

    draw(liveTelemetry.value);

    return liveTelemetry.subscribe(draw);
  }, [kind, min, max, digits, label, unit, select]);

  return (
    <svg
      className={styles.gauge}
      viewBox={`0 0 ${VIEW} ${VIEW}`}
      role="img"
      aria-labelledby={titleId}
    >
      <title id={titleId}>{label}</title>
      <text className={styles.accessible} ref={accessible} x="0" y="0">
        {label}
      </text>

      {kind === "compass" ? <CompassScale /> : <Scale min={min} max={max} />}

      <g ref={needle}>
        <path
          className={styles.needle}
          d={`M ${CENTRE} ${CENTRE - 36} L ${CENTRE - 3} ${CENTRE + 6} L ${CENTRE + 3} ${CENTRE + 6} Z`}
        />
      </g>
      <circle className={styles.hub} cx={CENTRE} cy={CENTRE} r="3" />

      <text className={styles.label} x={CENTRE} y="16" textAnchor="middle">
        {label}
      </text>
      <text
        className={styles.readout}
        x={CENTRE}
        y="88"
        textAnchor="middle"
        ref={readout}
      >
        —
      </text>
      <text className={styles.unit} x={CENTRE} y="101" textAnchor="middle">
        {unit}
      </text>
    </svg>
  );
}

/** The scale of a plain dial: an arc, the ticks and the two ends. */
function Scale({ min, max }: { min: number; max: number }) {
  const ticks = [];
  const divisions = 4;

  for (let index = 0; index <= divisions * 2; index += 1) {
    const fraction = index / (divisions * 2);
    const angle = ARC_START + fraction * (ARC_END - ARC_START);
    const major = index % 2 === 0;
    const outer = polar(RADIUS, angle);
    const inner = polar(RADIUS - (major ? 7 : 4), angle);
    ticks.push(
      <line
        key={index}
        className={styles.tick}
        x1={inner.x.toFixed(2)}
        y1={inner.y.toFixed(2)}
        x2={outer.x.toFixed(2)}
        y2={outer.y.toFixed(2)}
      />,
    );
  }

  const low = polar(RADIUS + 11, ARC_START);
  const high = polar(RADIUS + 11, ARC_END);

  return (
    <g>
      <path className={styles.scale} d={arc(RADIUS, ARC_START, ARC_END)} />
      {ticks}
      <text
        className={styles.endLabel}
        x={low.x.toFixed(2)}
        y={(low.y + 4).toFixed(2)}
        textAnchor="middle"
      >
        {min}
      </text>
      <text
        className={styles.endLabel}
        x={high.x.toFixed(2)}
        y={(high.y + 4).toFixed(2)}
        textAnchor="middle"
      >
        {max}
      </text>
    </g>
  );
}

/** The rose of a compass: the cardinal points and a tick every ten degrees. */
function CompassScale() {
  const cardinals: readonly [number, string][] = [
    [0, "N"],
    [90, "E"],
    [180, "S"],
    [270, "W"],
  ];

  const ticks = [];
  for (let angle = 0; angle < 360; angle += 10) {
    const major = angle % 30 === 0;
    const outer = polar(RADIUS, angle);
    const inner = polar(RADIUS - (major ? 8 : 4), angle);
    ticks.push(
      <line
        key={angle}
        className={styles.tick}
        x1={inner.x.toFixed(2)}
        y1={inner.y.toFixed(2)}
        x2={outer.x.toFixed(2)}
        y2={outer.y.toFixed(2)}
      />,
    );
  }

  return (
    <g>
      <circle
        className={styles.scale}
        cx={CENTRE}
        cy={CENTRE}
        r={RADIUS}
        fill="none"
      />
      {ticks}
      {cardinals.map(([angle, name]) => {
        const point = polar(RADIUS - 15, angle);
        return (
          <text
            key={name}
            className={styles.cardinal}
            x={point.x.toFixed(2)}
            y={(point.y + 4).toFixed(2)}
            textAnchor="middle"
          >
            {name}
          </text>
        );
      })}
    </g>
  );
}
