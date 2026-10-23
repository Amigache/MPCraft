import { useEffect, useId, useMemo, useRef, useState } from "react";

import type { Telemetry } from "../../lib/ipc/types";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { liveTelemetry } from "../../lib/link/liveTelemetry";
import styles from "./Hud.module.css";

/**
 * The attitude indicator.
 *
 * It is drawn in the pixels of its own box, so the text keeps its size and the
 * instrument keeps its proportions whatever the size of the column it sits in. The
 * heading runs across the top, the bank scale sits over the horizon, the airspeed and
 * the height are tapes at the sides with their value fixed in the middle, and the
 * readings sit in two rows at the bottom.
 *
 * The picture moves by writing transforms on its groups, not by re-rendering the
 * component: a telemetry tick must not cost a reconciliation of the tree. The tapes are
 * only rebuilt when the value crosses a step of their scale.
 */

const PITCH_DEGREES_IN_VIEW = 70;
const RIBBON_DEGREES_IN_VIEW = 90;
const AIRSPEED_IN_VIEW = 30;
const ALTITUDE_IN_VIEW = 60;
const AIRSPEED_STEP = 5;
const ALTITUDE_STEP = 10;
const TAPE_WIDTH = 38;
const TAPE_INSET = 4;
const BOTTOM_RESERVE = 36;

const LADDER_PITCHES = [10, 20, 30] as const;
const BANK_MARKS = [-60, -45, -30, -20, -10, 0, 10, 20, 30, 45, 60] as const;
const BANK_LABELS = [-60, -30, 30, 60] as const;
const RIBBON_HEADINGS = Array.from(
  { length: 55 },
  (_, index) => -90 + index * 10,
);

const DEFAULT_WIDTH = 280;
const DEFAULT_HEIGHT = 245;

type Layout = {
  width: number;
  height: number;
  bar: number;
  ribbon: number;
  tapeTop: number;
  tapeHeight: number;
  centreX: number;
  centreY: number;
  bankRadius: number;
  pitchPixels: number;
  ribbonPixels: number;
  airPixels: number;
  altPixels: number;
};

function layout(width: number, height: number): Layout {
  const bar = Math.min(26, Math.max(20, Math.round(height * 0.09)));
  const ribbon = bar;
  const tapeTop = bar + ribbon + 10;
  const tapeBottom = Math.max(tapeTop + 20, height - BOTTOM_RESERVE - 8);
  const tapeHeight = tapeBottom - tapeTop;
  const centreX = width / 2;
  const centreY = tapeTop + tapeHeight / 2;

  return {
    width,
    height,
    bar,
    ribbon,
    tapeTop,
    tapeHeight,
    centreX,
    centreY,
    bankRadius: Math.max(
      24,
      Math.min(width * 0.34, centreY - bar - ribbon - 30),
    ),
    pitchPixels: Math.max(2.5, height / PITCH_DEGREES_IN_VIEW),
    ribbonPixels: width / RIBBON_DEGREES_IN_VIEW,
    airPixels: tapeHeight / AIRSPEED_IN_VIEW,
    altPixels: tapeHeight / ALTITUDE_IN_VIEW,
  };
}

/** A point on a circle, with zero at the top and the angle growing clockwise. */
function polar(
  centreX: number,
  centreY: number,
  radius: number,
  angle: number,
): { x: number; y: number } {
  const radians = (angle * Math.PI) / 180;
  return {
    x: centreX + radius * Math.sin(radians),
    y: centreY - radius * Math.cos(radians),
  };
}

function arcPath(
  centreX: number,
  centreY: number,
  radius: number,
  from: number,
  to: number,
): string {
  const start = polar(centreX, centreY, radius, from);
  const end = polar(centreX, centreY, radius, to);
  return `M ${start.x.toFixed(2)} ${start.y.toFixed(2)} A ${radius.toFixed(2)} ${radius.toFixed(2)} 0 0 1 ${end.x.toFixed(2)} ${end.y.toFixed(2)}`;
}

/** The name a heading is given on the ribbon: the cardinals, or three digits. */
function compassLabel(heading: number): string {
  const normalised = ((heading % 360) + 360) % 360;
  const cardinal: Record<number, string> = {
    0: "N",
    90: "E",
    180: "S",
    270: "W",
  };
  return cardinal[normalised] ?? normalised.toString().padStart(3, "0");
}

/** Writes a reading into an element, and only when it has changed. */
function write(element: SVGTextElement | null, text: string): void {
  if (element !== null && element.textContent !== text) {
    element.textContent = text;
  }
}

export function Hud() {
  const id = useId().replace(/:/g, "");

  const t = useLocaleStore((state) => state.t);

  const box = useRef<HTMLDivElement | null>(null);
  const label = useRef<SVGSVGElement | null>(null);
  const attitude = useRef<SVGGElement | null>(null);
  const pointer = useRef<SVGGElement | null>(null);
  const ribbon = useRef<SVGGElement | null>(null);
  const airTape = useRef<SVGGElement | null>(null);
  const altTape = useRef<SVGGElement | null>(null);
  const airValue = useRef<SVGTextElement | null>(null);
  const altValue = useRef<SVGTextElement | null>(null);
  const throttleBars = useRef<SVGGElement | null>(null);
  const throttleValue = useRef<SVGTextElement | null>(null);
  const armedValue = useRef<SVGTextElement | null>(null);
  const clockValue = useRef<SVGTextElement | null>(null);
  const groundValue = useRef<SVGTextElement | null>(null);
  const modeValue = useRef<SVGTextElement | null>(null);
  const climbValue = useRef<SVGTextElement | null>(null);
  const batteryValue = useRef<SVGTextElement | null>(null);
  const gpsValue = useRef<SVGTextElement | null>(null);

  // The steps the tapes are built around. They are kept in refs for the telemetry
  // callback, and in state so that the marks are rebuilt when a step is crossed.
  const airBaseRef = useRef(0);
  const altBaseRef = useRef(0);
  const [airBase, setAirBase] = useState(0);
  const [altBase, setAltBase] = useState(0);

  const [size, setSize] = useState({
    width: DEFAULT_WIDTH,
    height: DEFAULT_HEIGHT,
  });

  useEffect(() => {
    const element = box.current;
    if (element === null) {
      return;
    }

    const observer = new ResizeObserver(() => {
      setSize({
        width: Math.max(1, element.clientWidth),
        height: Math.max(1, element.clientHeight),
      });
    });

    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const L = useMemo(() => layout(size.width, size.height), [size]);

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
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    const draw = (telemetry: Telemetry) => {
      const current = telemetry.attitude;
      const roll = current ? current.roll : 0;
      const pitch = current ? current.pitch : 0;
      const yaw = current ? ((current.yaw % 360) + 360) % 360 : 0;

      // The world turns with the roll and slides with the pitch.
      attitude.current?.setAttribute(
        "transform",
        `translate(0 ${(pitch * L.pitchPixels).toFixed(2)}) rotate(${(-roll).toFixed(2)} ${L.centreX} ${L.centreY})`,
      );

      // The bank pointer is fixed to the aircraft, so it turns against the picture.
      pointer.current?.setAttribute(
        "transform",
        `rotate(${roll.toFixed(2)} ${L.centreX} ${L.centreY})`,
      );

      ribbon.current?.setAttribute(
        "transform",
        `translate(${(L.centreX - yaw * L.ribbonPixels).toFixed(2)} 0)`,
      );

      const tapeCentre = L.tapeTop + L.tapeHeight / 2;
      const airspeed = telemetry.airspeed?.airspeed ?? 0;
      const altitude = telemetry.position?.alt ?? 0;

      const airBucket = Math.round(airspeed / AIRSPEED_STEP) * AIRSPEED_STEP;
      const altBucket = Math.round(altitude / ALTITUDE_STEP) * ALTITUDE_STEP;
      if (airBucket !== airBaseRef.current) {
        airBaseRef.current = airBucket;
        setAirBase(airBucket);
      }
      if (altBucket !== altBaseRef.current) {
        altBaseRef.current = altBucket;
        setAltBase(altBucket);
      }

      airTape.current?.setAttribute(
        "transform",
        `translate(0 ${(tapeCentre + (airspeed - airBaseRef.current) * L.airPixels).toFixed(2)})`,
      );
      altTape.current?.setAttribute(
        "transform",
        `translate(0 ${(tapeCentre + (altitude - altBaseRef.current) * L.altPixels).toFixed(2)})`,
      );

      label.current?.setAttribute(
        "aria-label",
        t(
          "Attitude: roll {roll} degrees, pitch {pitch} degrees, heading {heading} degrees",
          {
            roll: roll.toFixed(0),
            pitch: pitch.toFixed(0),
            heading: yaw.toFixed(0),
          },
        ),
      );

      const airReading = telemetry.airspeed?.airspeed ?? null;
      const groundSpeed =
        telemetry.airspeed?.groundSpeed ??
        telemetry.velocity?.groundSpeed ??
        null;
      const climb = telemetry.airspeed?.climb ?? null;
      const throttle = telemetry.airspeed?.throttle ?? null;
      const battery = telemetry.battery ?? null;
      const gps = telemetry.gps ?? null;
      const mode = telemetry.mode ?? null;
      const armed = telemetry.armed ?? null;

      write(airValue.current, airReading === null ? "" : airReading.toFixed(1));
      write(
        altValue.current,
        telemetry.position?.alt === undefined ||
          telemetry.position?.alt === null
          ? ""
          : telemetry.position.alt.toFixed(0),
      );
      write(
        groundValue.current,
        groundSpeed === null ? "" : `GS ${groundSpeed.toFixed(1)}`,
      );
      write(
        climbValue.current,
        climb === null ? "" : `${climb >= 0 ? "+" : ""}${climb.toFixed(1)} m/s`,
      );
      write(
        batteryValue.current,
        battery === null
          ? ""
          : `BAT ${battery.voltage.toFixed(2)} V ${battery.remaining.toFixed(0)}%`,
      );
      write(
        gpsValue.current,
        gps === null ? "GPS --" : `GPS ${gps.satellites}`,
      );
      write(
        throttleValue.current,
        throttle === null ? "" : `${throttle.toFixed(0)}%`,
      );
      write(modeValue.current, mode ?? "");

      if (armedValue.current !== null) {
        armedValue.current.textContent =
          armed === null ? "" : armed ? t("ARMED") : t("DISARMED");
        armedValue.current.style.fill = armed
          ? "var(--colour-accent)"
          : "var(--colour-danger)";
      }

      if (throttleBars.current !== null && throttle !== null) {
        const lit = Math.round((throttle / 100) * 10);
        for (let index = 0; index < 10; index += 1) {
          const cell = throttleBars.current.children[index];
          if (cell instanceof SVGElement) {
            cell.setAttribute("opacity", index < lit ? "1" : "0.2");
          }
        }
      }
    };

    draw(liveTelemetry.value);
    return liveTelemetry.subscribe(draw);
  }, [L, t]);

  const { width, height } = L;
  const id_ = (name: string) => `${id}-${name}`;
  const tapeCentre = L.tapeTop + L.tapeHeight / 2;
  const airRight = TAPE_INSET + TAPE_WIDTH;
  const altLeft = width - TAPE_INSET - TAPE_WIDTH;
  const inner = width * 0.12;
  const outer = width * 0.24;

  return (
    <div className={styles.box} ref={box}>
      <svg
        className={styles.hud}
        viewBox={`0 0 ${width} ${height}`}
        role="img"
        aria-label={t("Attitude indicator")}
        ref={label}
      >
        <defs>
          <clipPath id={id_("glass")}>
            <rect x={0} y={0} width={width} height={height} />
          </clipPath>
          <clipPath id={id_("ribbon")}>
            <rect x={0} y={0} width={width} height={L.ribbon} />
          </clipPath>
          <clipPath id={id_("air")}>
            <rect
              x={TAPE_INSET}
              y={L.tapeTop}
              width={TAPE_WIDTH}
              height={L.tapeHeight}
            />
          </clipPath>
          <clipPath id={id_("alt")}>
            <rect
              x={altLeft}
              y={L.tapeTop}
              width={TAPE_WIDTH}
              height={L.tapeHeight}
            />
          </clipPath>
        </defs>

        <rect
          className={styles.glass}
          x={0}
          y={0}
          width={width}
          height={height}
        />

        <g clipPath={`url(#${id_("glass")})`}>
          {/* The picture of the world: the sky, the ground, the horizon and the ladder. */}
          <g ref={attitude}>
            <rect
              className={styles.sky}
              x={-width}
              y={L.centreY - height * 3}
              width={width * 3}
              height={height * 3}
            />
            <rect
              className={styles.ground}
              x={-width}
              y={L.centreY}
              width={width * 3}
              height={height * 3}
            />
            <line
              className={styles.horizon}
              x1={-width}
              y1={L.centreY}
              x2={width * 2}
              y2={L.centreY}
            />
            {LADDER_PITCHES.map((pitch) =>
              [pitch, -pitch].map((value) => {
                const y = L.centreY - value * L.pitchPixels;
                const className =
                  value > 0 ? styles.ladder : styles.ladderNegative;
                return (
                  <g key={value}>
                    <line
                      className={className}
                      x1={L.centreX - outer}
                      y1={y}
                      x2={L.centreX - inner}
                      y2={y}
                    />
                    <line
                      className={className}
                      x1={L.centreX + inner}
                      y1={y}
                      x2={L.centreX + outer}
                      y2={y}
                    />
                    {value > 0 ? (
                      <>
                        <text
                          className={styles.ladderLabel}
                          x={L.centreX - outer - 4}
                          y={y + 3.5}
                          textAnchor="end"
                        >
                          {value}
                        </text>
                        <text
                          className={styles.ladderLabel}
                          x={L.centreX + outer + 4}
                          y={y + 3.5}
                        >
                          {value}
                        </text>
                      </>
                    ) : null}
                  </g>
                );
              }),
            )}
          </g>

          {/* The bank scale is fixed; only its pointer turns with the aircraft. */}
          <path
            className={styles.bankArc}
            d={arcPath(L.centreX, L.centreY, L.bankRadius, -60, 60)}
          />
          {BANK_MARKS.map((mark) => {
            const major = mark % 30 === 0;
            const outerPoint = polar(L.centreX, L.centreY, L.bankRadius, mark);
            const innerPoint = polar(
              L.centreX,
              L.centreY,
              L.bankRadius - (major ? 8 : 4),
              mark,
            );
            return (
              <line
                key={mark}
                className={styles.bankTick}
                x1={innerPoint.x.toFixed(2)}
                y1={innerPoint.y.toFixed(2)}
                x2={outerPoint.x.toFixed(2)}
                y2={outerPoint.y.toFixed(2)}
              />
            );
          })}
          {BANK_LABELS.map((mark) => {
            const at = polar(L.centreX, L.centreY, L.bankRadius - 13, mark);
            return (
              <text
                key={mark}
                className={styles.bankLabel}
                x={at.x.toFixed(2)}
                y={(at.y + 3).toFixed(2)}
                textAnchor="middle"
              >
                {Math.abs(mark)}
              </text>
            );
          })}
          <g ref={pointer}>
            <path
              className={styles.bankPointer}
              d={`M ${L.centreX} ${(L.centreY - L.bankRadius + 2).toFixed(2)} l -4 -7 l 8 0 Z`}
            />
          </g>

          {/* The aircraft, fixed in the middle of the glass. */}
          <path
            className={styles.symbolHalo}
            d={`M ${L.centreX - 40} ${L.centreY} L ${L.centreX - 14} ${L.centreY} L ${L.centreX - 7} ${L.centreY + 7} L ${L.centreX} ${L.centreY} L ${L.centreX + 7} ${L.centreY + 7} L ${L.centreX + 14} ${L.centreY} L ${L.centreX + 40} ${L.centreY}`}
          />
          <path
            className={styles.symbol}
            d={`M ${L.centreX - 40} ${L.centreY} L ${L.centreX - 14} ${L.centreY} L ${L.centreX - 7} ${L.centreY + 7} L ${L.centreX} ${L.centreY} L ${L.centreX + 7} ${L.centreY + 7} L ${L.centreX + 14} ${L.centreY} L ${L.centreX + 40} ${L.centreY}`}
          />
        </g>

        {/* The heading ribbon, under the bar of the readings. */}
        <g transform={`translate(0 ${L.bar})`}>
          <g clipPath={`url(#${id_("ribbon")})`}>
            <rect
              className={styles.ribbonBack}
              x={0}
              y={0}
              width={width}
              height={L.ribbon}
            />
            <g ref={ribbon}>
              {RIBBON_HEADINGS.map((heading) => {
                const x = heading * L.ribbonPixels;
                const major = ((heading % 30) + 30) % 30 === 0;
                return (
                  <g key={heading}>
                    <line
                      className={styles.ribbonTick}
                      x1={x.toFixed(2)}
                      y1={L.ribbon}
                      x2={x.toFixed(2)}
                      y2={L.ribbon - (major ? 9 : 5)}
                    />
                    {major ? (
                      <text
                        className={styles.ribbonLabel}
                        x={x.toFixed(2)}
                        y={L.ribbon - 11}
                        textAnchor="middle"
                      >
                        {compassLabel(heading)}
                      </text>
                    ) : null}
                  </g>
                );
              })}
            </g>
          </g>
          <line
            className={styles.ribbonEdge}
            x1={0}
            y1={L.ribbon}
            x2={width}
            y2={L.ribbon}
          />
          <path
            className={styles.ribbonMarker}
            d={`M ${L.centreX} ${L.ribbon - 1} l -5 -6 l 10 0 Z`}
          />
        </g>

        {/* The airspeed tape on the left: its marks scroll, its value stays in the middle. */}
        <rect
          className={styles.tapeBack}
          x={TAPE_INSET}
          y={L.tapeTop}
          width={TAPE_WIDTH}
          height={L.tapeHeight}
        />
        <g clipPath={`url(#${id_("air")})`}>
          <g ref={airTape}>
            {Array.from({ length: 41 }, (_, index) => airBase - 20 + index)
              .filter((mark) => mark >= 0)
              .map((mark) => {
                const y = -(mark - airBase) * L.airPixels;
                const major = mark % 5 === 0;
                return (
                  <g key={mark}>
                    <line
                      className={styles.tapeTick}
                      x1={airRight - (major ? 10 : 5)}
                      y1={y.toFixed(2)}
                      x2={airRight}
                      y2={y.toFixed(2)}
                    />
                    {major ? (
                      <text
                        className={styles.tapeLabel}
                        x={airRight - 12}
                        y={(y + 3.5).toFixed(2)}
                        textAnchor="end"
                      >
                        {mark}
                      </text>
                    ) : null}
                  </g>
                );
              })}
          </g>
        </g>
        <rect
          className={styles.tapeBox}
          x={TAPE_INSET - 2}
          y={tapeCentre - 11}
          width={TAPE_WIDTH + 4}
          height={22}
        />
        <text
          ref={airValue}
          className={styles.tapeValue}
          x={TAPE_INSET - 2 + (TAPE_WIDTH + 4) / 2}
          y={tapeCentre + 4.5}
          textAnchor="middle"
        />

        {/* The height tape on the right. */}
        <rect
          className={styles.tapeBack}
          x={altLeft}
          y={L.tapeTop}
          width={TAPE_WIDTH}
          height={L.tapeHeight}
        />
        <g clipPath={`url(#${id_("alt")})`}>
          <g ref={altTape}>
            {Array.from(
              { length: 17 },
              (_, index) => altBase - 80 + index * 10,
            ).map((mark) => {
              const y = -(mark - altBase) * L.altPixels;
              const major = mark % 20 === 0;
              return (
                <g key={mark}>
                  <line
                    className={styles.tapeTick}
                    x1={altLeft}
                    y1={y.toFixed(2)}
                    x2={altLeft + (major ? 10 : 5)}
                    y2={y.toFixed(2)}
                  />
                  {major ? (
                    <text
                      className={styles.tapeLabel}
                      x={altLeft + 12}
                      y={(y + 3.5).toFixed(2)}
                    >
                      {mark}
                    </text>
                  ) : null}
                </g>
              );
            })}
          </g>
        </g>
        <rect
          className={styles.tapeBox}
          x={width - TAPE_INSET + 2 - (TAPE_WIDTH + 4)}
          y={tapeCentre - 11}
          width={TAPE_WIDTH + 4}
          height={22}
        />
        <text
          ref={altValue}
          className={styles.tapeValue}
          x={width - TAPE_INSET + 2 - (TAPE_WIDTH + 4) / 2}
          y={tapeCentre + 4.5}
          textAnchor="middle"
        />

        {/* The readings: the top row under the ribbon, and the two rows at the bottom. */}
        <g className={styles.readouts}>
          <rect
            className={styles.readBand}
            x={0}
            y={0}
            width={width}
            height={L.bar}
          />
          <rect
            className={styles.readBand}
            x={0}
            y={height - BOTTOM_RESERVE}
            width={width}
            height={BOTTOM_RESERVE}
          />
          <g ref={throttleBars} transform={`translate(6 ${L.bar - 15})`}>
            {Array.from({ length: 10 }, (_, index) => (
              <rect
                key={index}
                className={styles.throttleCell}
                x={index * 4}
                y={0}
                width={3}
                height={9}
              />
            ))}
          </g>
          <text
            ref={throttleValue}
            className={styles.small}
            x={50}
            y={L.bar - 6}
          />
          <text
            ref={armedValue}
            className={styles.armed}
            x={L.centreX}
            y={L.bar - 6}
            textAnchor="middle"
          />
          <text
            ref={clockValue}
            className={styles.small}
            x={width - 6}
            y={L.bar - 6}
            textAnchor="end"
          />

          <text
            ref={groundValue}
            className={styles.small}
            x={6}
            y={height - 20}
          />
          <text
            ref={modeValue}
            className={styles.mode}
            x={L.centreX}
            y={height - 20}
            textAnchor="middle"
          />
          <text
            ref={climbValue}
            className={styles.small}
            x={width - 6}
            y={height - 20}
            textAnchor="end"
          />
          <text
            ref={batteryValue}
            className={styles.small}
            x={6}
            y={height - 6}
          />
          <text
            ref={gpsValue}
            className={styles.small}
            x={width - 6}
            y={height - 6}
            textAnchor="end"
          />
        </g>
      </svg>
    </div>
  );
}
