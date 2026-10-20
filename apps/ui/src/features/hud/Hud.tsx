import { useEffect, useId, useRef, useState } from "react";

import type { Telemetry } from "../../lib/ipc/types";
import { liveTelemetry } from "../../lib/link/liveTelemetry";
import styles from "./Hud.module.css";

/**
 * The artificial horizon.
 *
 * The sky and the ground are one picture that turns with the roll and slides with the
 * pitch; the pitch ladder, the bank scale and the heading ribbon are drawn on top of it.
 *
 * The drawing is updated by writing attributes on the SVG elements, not by re-rendering
 * the component: a telemetry tick must not cost a reconciliation of the tree.
 */

/** The width of the drawing, in its own units. */
const DESIGN_WIDTH = 320;

/** The height of the drawing, in its own units. */
const DESIGN_HEIGHT = 216;

/** The height of the heading ribbon. */
const RIBBON_HEIGHT = 22;

/** The centre of the glass. */
const CENTRE_X = 160;
const CENTRE_Y = 126;

/** How many pixels a degree of pitch is worth. */
const PITCH_PIXELS = 3.4;

/** How many pixels a degree of heading is worth on the ribbon. */
const HEADING_PIXELS = 3;

/** The radius of the bank scale. */
const BANK_RADIUS = 96;

/** The pitch the ladder is marked at. */
const LADDER_PITCHES = [10, 20, 30] as const;

/** The bank the scale is marked at. */
const BANK_MARKS = [-60, -45, -30, -20, -10, 0, 10, 20, 30, 45, 60] as const;

/** A point on a circle, with zero at the top and the angle growing clockwise. */
function polar(radius: number, angle: number): { x: number; y: number } {
  const radians = ((angle - 90) * Math.PI) / 180;
  return {
    x: CENTRE_X + radius * Math.cos(radians),
    y: CENTRE_Y + radius * Math.sin(radians),
  };
}

/** The path of an arc, from one angle to another. */
function arc(radius: number, from: number, to: number): string {
  const start = polar(radius, from);
  const end = polar(radius, to);
  const large = Math.abs(to - from) > 180 ? 1 : 0;
  return `M ${start.x.toFixed(2)} ${start.y.toFixed(2)} A ${radius} ${radius} 0 ${large} 1 ${end.x.toFixed(2)} ${end.y.toFixed(2)}`;
}

/** The name a heading is given on the ribbon. */
function compassLabel(heading: number): string {
  const normalised = ((heading % 360) + 360) % 360;
  const cardinal: Record<number, string> = {
    0: "N",
    90: "E",
    180: "S",
    270: "W",
  };
  return cardinal[normalised] ?? String(normalised);
}

/** The headings the ribbon is marked at: every ten degrees of a wide enough range. */
const RIBBON_HEADINGS = (() => {
  const headings: number[] = [];
  for (let heading = -180; heading <= 540; heading += 10) {
    headings.push(heading);
  }
  return headings;
})();

export function Hud() {
  const glassId = useId();
  const ribbonId = useId();

  const attitude = useRef<SVGGElement | null>(null);
  const pointer = useRef<SVGGElement | null>(null);
  const ribbon = useRef<SVGGElement | null>(null);
  const label = useRef<SVGSVGElement | null>(null);

  // The size of the box the drawing is put in. The drawing keeps its own proportions
  // and is centred in the box, and the sky and the ground fill the box whatever its
  // shape: the column can be dragged wider or narrower, and the horizon has to reach
  // the edges of it either way.
  const box = useRef<HTMLDivElement | null>(null);
  const [width, setWidth] = useState(DESIGN_WIDTH);
  const [height, setHeight] = useState(DESIGN_HEIGHT);

  useEffect(() => {
    const element = box.current;
    if (!element) {
      return;
    }

    const observer = new ResizeObserver(() => {
      const rect = element.getBoundingClientRect();
      setWidth(Math.max(1, Math.round(rect.width)));
      setHeight(Math.max(1, Math.round(rect.height)));
    });

    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const draw = (telemetry: Telemetry) => {
      const current = telemetry.attitude;
      const roll = current ? current.roll : 0;
      const pitch = current ? current.pitch : 0;
      const yaw = current ? ((current.yaw % 360) + 360) % 360 : 0;

      // The picture of the world turns with the roll and slides with the pitch.
      attitude.current?.setAttribute(
        "transform",
        `translate(0 ${(pitch * PITCH_PIXELS).toFixed(2)}) rotate(${(-roll).toFixed(2)} ${CENTRE_X} ${CENTRE_Y})`,
      );

      // The pointer on the bank scale turns the other way: it is fixed to the aircraft.
      pointer.current?.setAttribute(
        "transform",
        `rotate(${roll.toFixed(2)} ${CENTRE_X} ${CENTRE_Y})`,
      );

      // The ribbon slides so that the heading is under the marker.
      ribbon.current?.setAttribute(
        "transform",
        `translate(${(CENTRE_X - (yaw + 180) * HEADING_PIXELS).toFixed(2)} 0)`,
      );

      label.current?.setAttribute(
        "aria-label",
        `Attitude: roll ${roll.toFixed(0)} degrees, pitch ${pitch.toFixed(0)} degrees, heading ${yaw.toFixed(0)} degrees`,
      );
    };

    draw(liveTelemetry.value);

    return liveTelemetry.subscribe(draw);
  }, []);

  // The drawing is scaled to fit the box and centred in it.
  const scale = Math.min(width / DESIGN_WIDTH, height / DESIGN_HEIGHT);
  const offsetX = (width - DESIGN_WIDTH * scale) / 2;
  const offsetY = (height - DESIGN_HEIGHT * scale) / 2;
  const placement = `translate(${offsetX.toFixed(2)} ${offsetY.toFixed(2)}) scale(${scale.toFixed(4)})`;

  return (
    <div className={styles.box} ref={box}>
      <svg
        className={styles.hud}
        viewBox={`0 0 ${width} ${height}`}
        preserveAspectRatio="none"
        role="img"
        aria-label="Attitude indicator"
        ref={label}
      >
        <defs>
          <clipPath id={glassId}>
            <rect x="0" y="0" width={width} height={height} />
          </clipPath>
          <clipPath id={ribbonId}>
            <rect
              x="0"
              y={offsetY}
              width={width}
              height={RIBBON_HEIGHT * scale}
            />
          </clipPath>
        </defs>

        <rect
          className={styles.glass}
          x="0"
          y="0"
          width={width}
          height={height}
        />

        <g clipPath={`url(#${glassId})`}>
          <g transform={placement}>
            {/* The attitude. */}
            <g ref={attitude}>
              <rect
                className={styles.sky}
                x={-400}
                y={-400}
                width={1120}
                height={CENTRE_Y + 400}
              />
              <rect
                className={styles.ground}
                x={-400}
                y={CENTRE_Y}
                width={1120}
                height={800}
              />
              <line
                className={styles.horizon}
                x1={-400}
                y1={CENTRE_Y}
                x2={720}
                y2={CENTRE_Y}
              />

              {LADDER_PITCHES.map((pitch) =>
                [pitch, -pitch].map((value) => {
                  const y = CENTRE_Y - value * PITCH_PIXELS;
                  const length = 26 + (Math.abs(value) - 10) * 6;
                  const gap = 11;
                  const label = Math.abs(value).toString();
                  return (
                    <g key={value}>
                      <line
                        className={styles.ladder}
                        x1={CENTRE_X - length}
                        y1={y}
                        x2={CENTRE_X - gap}
                        y2={y}
                      />
                      <line
                        className={styles.ladder}
                        x1={CENTRE_X + gap}
                        y1={y}
                        x2={CENTRE_X + length}
                        y2={y}
                      />
                      <text
                        className={styles.ladderLabel}
                        x={CENTRE_X - length - 4}
                        y={y + 3}
                        textAnchor="end"
                      >
                        {label}
                      </text>
                    </g>
                  );
                }),
              )}
            </g>

            {/* The bank scale. */}
            <path className={styles.bankArc} d={arc(BANK_RADIUS, -60, 60)} />
            {BANK_MARKS.map((mark) => {
              const major = mark % 30 === 0 || mark === 0;
              const outer = polar(BANK_RADIUS, mark);
              const inner = polar(BANK_RADIUS - (major ? 9 : 5), mark);
              return (
                <line
                  key={mark}
                  className={styles.bankTick}
                  x1={inner.x.toFixed(2)}
                  y1={inner.y.toFixed(2)}
                  x2={outer.x.toFixed(2)}
                  y2={outer.y.toFixed(2)}
                />
              );
            })}

            {/* The pointer, fixed to the aircraft. */}
            <g ref={pointer}>
              <path
                className={styles.bankPointer}
                d={`M ${CENTRE_X} ${CENTRE_Y - BANK_RADIUS + 4} l -5 10 l 10 0 Z`}
              />
            </g>

            {/* The aircraft, fixed in the middle of the glass. */}
            <path
              className={styles.symbol}
              d={`M ${CENTRE_X - 34} ${CENTRE_Y} L ${CENTRE_X - 14} ${CENTRE_Y} L ${CENTRE_X - 8} ${CENTRE_Y + 6} L ${CENTRE_X} ${CENTRE_Y - 6} L ${CENTRE_X + 8} ${CENTRE_Y + 6} L ${CENTRE_X + 14} ${CENTRE_Y} L ${CENTRE_X + 34} ${CENTRE_Y}`}
            />
            <circle
              className={styles.symbolDot}
              cx={CENTRE_X}
              cy={CENTRE_Y}
              r="1.6"
            />
          </g>
        </g>

        {/* The heading ribbon. */}
        <g clipPath={`url(#${ribbonId})`}>
          <g transform={placement}>
            <g ref={ribbon}>
              {RIBBON_HEADINGS.map((heading) => {
                const x = (heading + 180) * HEADING_PIXELS;
                const major = ((heading % 360) + 360) % 360 % 30 === 0;
                return (
                  <g key={heading}>
                    <line
                      className={styles.ribbonTick}
                      x1={x}
                      y1={RIBBON_HEIGHT}
                      x2={x}
                      y2={major ? RIBBON_HEIGHT - 9 : RIBBON_HEIGHT - 4}
                    />
                    {major ? (
                      <text
                        className={styles.ribbonLabel}
                        x={x}
                        y={RIBBON_HEIGHT - 11}
                        textAnchor="middle"
                      >
                        {compassLabel(heading)}
                      </text>
                    ) : null}
                  </g>
                );
              })}
            </g>
            <line
              className={styles.ribbonEdge}
              x1={-400}
              y1={RIBBON_HEIGHT}
              x2={720}
              y2={RIBBON_HEIGHT}
            />
            <path
              className={styles.ribbonMarker}
              d={`M ${CENTRE_X} ${RIBBON_HEIGHT} l -4 -6 l 8 0 Z`}
            />
          </g>
        </g>
      </svg>
    </div>
  );
}
