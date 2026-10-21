/**
 * The plan of a graph.
 *
 * Two decisions have to be made before a chart can be drawn, and both of them are made
 * here rather than in the component, so that they can be tested without a canvas:
 *
 *  - **which axis each series goes on.** A series in metres and a series in degrees
 *    cannot share an axis — the chart would be a lie — so every distinct unit gets its
 *    own scale, and every scale its own axis.
 *  - **how the values of the series are laid on one time axis.** The backend skips the
 *    samples a vehicle has not reported, so two series of the same graph do not have the
 *    same instants. The chart needs one axis of time, so the values are laid on the
 *    union of the instants, and a value that is missing becomes a gap rather than a
 *    zero.
 *
 * Nothing here derives a value: the numbers are the ones the backend sent, and the only
 * thing added is a gap where there was nothing to draw.
 */

import type { SeriesInfo, SeriesPoints } from "../ipc/types";

/**
 * The colours the series are drawn in.
 *
 * The first is the accent of the interface, so that the first series of a graph is the
 * one the eye goes to. The rest are picked to stay apart from each other on a dark
 * ground.
 */
export const SERIES_COLOURS: readonly string[] = [
  "#00c896",
  "#58a6ff",
  "#d29922",
  "#f85149",
  "#bc8cff",
  "#79c0ff",
];

/**
 * How close two instants have to be to be the same one.
 *
 * The instants come from the same clock in the same history, so they are the same
 * number; the tolerance is here for the sake of the rounding a float can introduce.
 */
const SAME_INSTANT = 1e-6;

/** One axis of the chart, with the unit it measures. */
export interface ChartScale {
  /** The key of the scale, which is also the key of the axis. */
  readonly key: string;
  /** The unit the values on this scale are in. */
  readonly unit: string;
  /** Which side of the chart the axis stands on. */
  readonly side: "left" | "right";
}

/** One series of the chart. */
export interface ChartSeries {
  /** The name of the series, as the legend shows it. */
  readonly name: string;
  /** The unit the values are in. */
  readonly unit: string;
  /** The scale the series is drawn on. */
  readonly scale: string;
  /** The colour the series is drawn in. */
  readonly colour: string;
}

/** The plan of a chart: the axes, and the series drawn on them. */
export interface ChartPlan {
  /** The scales, in the order they are declared. */
  readonly scales: readonly ChartScale[];
  /** The series, in the order they are drawn. */
  readonly series: readonly ChartSeries[];
}

/**
 * Works out the axes of the chart from the units of the series.
 *
 * The first unit met goes on the left-hand axis; every other unit gets an axis of its
 * own on the right. A series with no unit shares the axis of the other unitless
 * series, because a count and a count are the same kind of thing.
 */
export function planChart(series: readonly SeriesInfo[]): ChartPlan {
  const scales: ChartScale[] = [];
  const byUnit = new Map<string, string>();

  const planned = series.map((entry, index) => {
    let key = byUnit.get(entry.unit);

    if (key === undefined) {
      key = scales.length === 0 ? "y" : `y${scales.length + 1}`;
      byUnit.set(entry.unit, key);
      scales.push({
        key,
        unit: entry.unit,
        side: scales.length === 0 ? "left" : "right",
      });
    }

    return {
      name: entry.name,
      unit: entry.unit,
      scale: key,
      colour: SERIES_COLOURS[index % SERIES_COLOURS.length] ?? "#00c896",
    };
  });

  return { scales, series: planned };
}

/** The values of a graph, laid on one axis of time. */
export interface AlignedValues {
  /** The instants, in seconds after the start of the history. */
  readonly x: number[];
  /** The values of each series, with a gap where the vehicle reported nothing. */
  readonly y: (number | null)[][];
}

/**
 * Lays the values of the series on one axis of time.
 *
 * The instants of every series are gathered into one list, in order, and each series is
 * then laid on it. An instant a series has no value for becomes a gap, so that the line
 * breaks where the data breaks instead of dropping to nothing.
 */
export function alignSeries(series: readonly SeriesPoints[]): AlignedValues {
  const gathered: number[] = [];
  for (const entry of series) {
    for (const at of entry.x) {
      gathered.push(at);
    }
  }
  gathered.sort((left, right) => left - right);

  const x: number[] = [];
  for (const at of gathered) {
    const last = x[x.length - 1];
    if (last === undefined || at - last > SAME_INSTANT) {
      x.push(at);
    }
  }

  const y = series.map((entry) => {
    const values: (number | null)[] = new Array<number | null>(x.length).fill(null);
    let index = 0;

    for (let position = 0; position < entry.x.length; position += 1) {
      const at = entry.x[position];
      const value = entry.y[position];

      if (at === undefined || value === undefined) {
        continue;
      }

      while (index < x.length) {
        const candidate = x[index];
        if (candidate === undefined || candidate >= at - SAME_INSTANT) {
          break;
        }
        index += 1;
      }

      const target = x[index];
      if (target !== undefined && Math.abs(target - at) <= SAME_INSTANT) {
        values[index] = value;
      }
    }

    return values;
  });

  return { x, y };
}

/**
 * The value of each series at its most recent instant.
 *
 * A series the vehicle has stopped reporting has no current value, and the legend says
 * so rather than showing the last thing it said as if it were live.
 */
export function latestValues(series: readonly SeriesPoints[]): (number | null)[] {
  return series.map((entry) => {
    const last = entry.y[entry.y.length - 1];
    return last === undefined ? null : last;
  });
}

/**
 * An instant of the history, as it is written on the time axis.
 *
 * The instants are seconds after the start of the history, and an operator reads them
 * as minutes and seconds.
 */
export function formatElapsed(seconds: number): string {
  const total = Number.isFinite(seconds) ? Math.max(0, Math.floor(seconds)) : 0;
  const minutes = Math.floor(total / 60);
  const rest = total % 60;

  return `${minutes}:${rest.toString().padStart(2, "0")}`;
}
