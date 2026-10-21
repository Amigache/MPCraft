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
const CENTRE_Y = 132;

/** How many pixels a degree of pitch is worth. */
const PITCH_PIXELS = 3;

/** How many pixels a degree of heading is worth on the ribbon. */
const HEADING_PIXELS = 3;

/** The radius of the bank scale. */
const BANK_RADIUS = 96;

/** The pitch the ladder is marked at. */
const LADDER_PITCHES = [10, 20, 30] as const;

/** The tapes of the sides: the airspeed on the left, the height on the right. */
const TAPE_TOP = 44;
const TAPE_BOTTOM = 168;
const TAPE_WIDTH = 30;
const AIR_TAPE_X = 6;
const ALT_TAPE_X = DESIGN_WIDTH - TAPE_WIDTH - 6;

/** The marks of the tape of the airspeed, in metres per second. */
const AIR_TAPE_MARKS = [0, 10, 20, 30, 40, 50] as const;

/** The marks of the tape of the height, in metres. */
const ALT_TAPE_MARKS = [0, 20, 40, 60, 80, 100] as const;

/** How many metres a second of the tape is worth. */
const AIR_TAPE_SCALE = 1.6;
const ALT_TAPE_SCALE = 1.2;

/** The bank the scale is marked at. */
const BANK_MARKS = [-60, -45, -30, -20, -10, 0, 10, 20, 30, 45, 60] as const;

/** The bank the scale is written at. */
const BANK_LABELS = [-60, -45, -30, -20, -10, 10, 20, 30, 45, 60] as const;

/** Writes a reading into an element of the drawing. */
function write(element: SVGTextElement | null, text: string): void {
  if (element !== null && element.textContent !== text) {
    element.textContent = text;
  }
}

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

  // The readings. They are written into the elements themselves, so that a tick of the
  // telemetry does not cost a reconciliation of the tree.
  const airspeedValue = useRef<SVGTextElement | null>(null);
  const altitudeValue = useRef<SVGTextElement | null>(null);
  const modeValue = useRef<SVGTextElement | null>(null);
  const armedValue = useRef<SVGTextElement | null>(null);
  const airValue = useRef<SVGTextElement | null>(null);
  const groundValue = useRef<SVGTextElement | null>(null);
  const batteryValue = useRef<SVGTextElement | null>(null);
  const gpsValue = useRef<SVGTextElement | null>(null);
  const climbValue = useRef<SVGTextElement | null>(null);
  const throttleValue = useRef<SVGTextElement | null>(null);
  const throttleBars = useRef<SVGGElement | null>(null);
  const clockValue = useRef<SVGTextElement | null>(null);

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

  // The clock is the time of the machine, and it does not come with the telemetry: it
  // is told once a second and not twenty times.
  useEffect(() => {
    const tick = () => {
      const now = new Date();
      const hours = now.getHours().toString().padStart(2, "0");
      const minutes = now.getMinutes().toString().padStart(2, "0");
      const seconds = now.getSeconds().toString().padStart(2, "0");

      write(clockValue.current, `${hours}:${minutes}:${seconds}`);
    };

    tick();
    const timer = window.setInterval(tick, 1000);

    return () => {
      window.clearInterval(timer);
    };
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

      // The readings of the sides. The numbers are the ones the vehicle sent, and they
      // are said in the unit the operator reads them in.
      const airspeed = telemetry.airspeed?.airspeed ?? null;
      const groundSpeed = telemetry.airspeed?.groundSpeed ?? telemetry.velocity?.groundSpeed ?? null;
      const altitude = telemetry.position?.alt ?? null;
      const climb = telemetry.airspeed?.climb ?? null;
      const throttle = telemetry.airspeed?.throttle ?? null;
      const battery = telemetry.battery ?? null;
      const gps = telemetry.gps ?? null;
      const mode = telemetry.mode ?? null;
      const armed = telemetry.armed ?? null;

      write(airspeedValue.current, airspeed === null ? "" : airspeed.toFixed(1));
      write(altitudeValue.current, altitude === null ? "" : altitude.toFixed(0));
      write(
        airValue.current,
        airspeed === null ? "" : `AS ${airspeed.toFixed(1)}`,
      );
      write(
        groundValue.current,
        groundSpeed === null ? "" : `GS ${groundSpeed.toFixed(1)}`,
      );
      write(
        batteryValue.current,
        battery === null
          ? ""
          : `Bat ${battery.voltage.toFixed(2)}v ${battery.remaining.toFixed(0)}%`,
      );
      write(
        gpsValue.current,
        gps === null
          ? "GPS: no fix"
          : `GPS: ${gps.satellites} sats`,
      );
      write(climbValue.current, climb === null ? "" : `${climb.toFixed(1)} m/s`);
      write(
        throttleValue.current,
        throttle === null ? "" : `${throttle.toFixed(0)} %`,
      );
      write(modeValue.current, mode ?? "");
      write(
        armedValue.current,
        armed === null ? "" : armed ? "ARMED" : "DISARMED",
      );

      // The throttle is a bar of cells, and the cells that are lit are the ones the
      // throttle has.
      if (throttleBars.current !== null && throttle !== null) {
        const lit = Math.round((throttle / 100) * 10);

        for (let index = 0; index < 10; index += 1) {
          const cell = throttleBars.current.children[index];

          if (cell instanceof SVGElement) {
            cell.setAttribute("opacity", index < lit ? "1" : "0.25");
          }
        }
      }
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
                  const length = 24 + Math.min(18, (Math.abs(value) - 10) * 2);
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

            {/* The bank scale. It is marked with its numbers, the way an
                instrument is: a pilot reads the bank off the scale. */}
            <path className={styles.bankArc} d={arc(BANK_RADIUS, -60, 60)} />
            {BANK_LABELS.map((mark) => {
              const at = polar(BANK_RADIUS - 14, mark);
              return (
                <text
                  key={mark}
                  className={styles.bankLabel}
                  x={at.x.toFixed(2)}
                  y={(at.y + 3).toFixed(2)}
                  textAnchor="middle"
                >
                  {mark}
                </text>
              );
            })}
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

        {/* The tapes of the sides: the airspeed on the left, the height on the
            right. They are the two numbers a pilot looks at when the aircraft is
            not doing what it was told. */}
        <g transform={placement}>
          <g className={styles.tape}>
            <rect
              className={styles.tapeBack}
              x={AIR_TAPE_X}
              y={TAPE_TOP}
              width={TAPE_WIDTH}
              height={TAPE_BOTTOM - TAPE_TOP}
            />
            {AIR_TAPE_MARKS.map((mark) => {
              const y = TAPE_BOTTOM - mark * AIR_TAPE_SCALE;

              if (y < TAPE_TOP || y > TAPE_BOTTOM) {
                return null;
              }

              return (
                <g key={mark}>
                  <line
                    className={styles.tapeTick}
                    x1={AIR_TAPE_X + TAPE_WIDTH - 6}
                    y1={y}
                    x2={AIR_TAPE_X + TAPE_WIDTH}
                    y2={y}
                  />
                  <text
                    className={styles.tapeLabel}
                    x={AIR_TAPE_X + TAPE_WIDTH - 9}
                    y={y + 3}
                    textAnchor="end"
                  >
                    {mark}
                  </text>
                </g>
              );
            })}
            <text
              ref={airspeedValue}
              className={styles.tapeValue}
              x={AIR_TAPE_X + TAPE_WIDTH - 2}
              y={TAPE_TOP + 14}
              textAnchor="end"
            />
          </g>

          <g className={styles.tape}>
            <rect
              className={styles.tapeBack}
              x={ALT_TAPE_X}
              y={TAPE_TOP}
              width={TAPE_WIDTH}
              height={TAPE_BOTTOM - TAPE_TOP}
            />
            {ALT_TAPE_MARKS.map((mark) => {
              const y = TAPE_BOTTOM - mark * ALT_TAPE_SCALE;

              if (y < TAPE_TOP || y > TAPE_BOTTOM) {
                return null;
              }

              return (
                <g key={mark}>
                  <line
                    className={styles.tapeTick}
                    x1={ALT_TAPE_X}
                    y1={y}
                    x2={ALT_TAPE_X + 6}
                    y2={y}
                  />
                  <text
                    className={styles.tapeLabel}
                    x={ALT_TAPE_X + 9}
                    y={y + 3}
                  >
                    {mark}
                  </text>
                </g>
              );
            })}
            <text
              ref={altitudeValue}
              className={styles.tapeValue}
              x={ALT_TAPE_X + 2}
              y={TAPE_TOP + 14}
            />
          </g>
        </g>

        {/* The readings. They are written over the glass, and they do not turn with
            the aircraft: they are the state of the flight and not of the picture. */}
        <g transform={placement}>
          <g className={styles.readouts}>
            <text
              ref={armedValue}
              className={styles.armed}
              x={CENTRE_X}
              y={CENTRE_Y - 22}
              textAnchor="middle"
            />

            <g ref={throttleBars} transform={`translate(${DESIGN_WIDTH - 74} 32)`}>
              {Array.from({ length: 10 }, (_, index) => (
                <rect
                  key={index}
                  className={styles.throttleCell}
                  x={index * 4}
                  y={0}
                  width={3}
                  height={10}
                />
              ))}
            </g>
            <text
              ref={throttleValue}
              className={styles.small}
              x={DESIGN_WIDTH - 30}
              y={41}
              textAnchor="end"
            />
            <text
              ref={clockValue}
              className={styles.clock}
              x={DESIGN_WIDTH - 8}
              y={58}
              textAnchor="end"
            />

            <text ref={airValue} className={styles.small} x={8} y={176} />
            <text ref={groundValue} className={styles.small} x={8} y={190} />
            <text ref={batteryValue} className={styles.small} x={8} y={204} />
            <text
              ref={modeValue}
              className={styles.mode}
              x={DESIGN_WIDTH - 8}
              y={176}
              textAnchor="end"
            />
            <text
              ref={climbValue}
              className={styles.small}
              x={DESIGN_WIDTH - 8}
              y={190}
              textAnchor="end"
            />
            <text
              ref={gpsValue}
              className={styles.small}
              x={DESIGN_WIDTH - 8}
              y={204}
              textAnchor="end"
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
