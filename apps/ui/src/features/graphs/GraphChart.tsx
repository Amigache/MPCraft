import { useEffect, useRef } from "react";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";

import type { AlignedValues, ChartPlan } from "../../lib/graphs/plan";
import { formatElapsed } from "../../lib/graphs/plan";
import styles from "./GraphChart.module.css";

/**
 * The chart.
 *
 * The chart is a canvas, and a canvas is not React's business: it is built once for a
 * plan, and every new set of values is handed to it with `setData`. The React tree
 * around it is not re-rendered when the values change — the panel that owns the data
 * re-renders, and this component is handed the new values.
 *
 * The plan decides how many series there are and which axis each is drawn on, so a
 * change of plan means a new chart: the axes cannot be added to a chart that has
 * already been built.
 */

/** The colour of the axis values and labels. */
const AXIS_COLOUR = "#8b949e";

/** The colour of the grid lines. */
const GRID_COLOUR = "#2a3441";

/** The face the axis values are written in. */
const AXIS_FONT = '11px ui-monospace, "Cascadia Mono", Consolas, monospace';

/** The smallest the chart is allowed to be. */
const MINIMUM_SIZE = 1;

export interface GraphChartProps {
  /** The plan: the series, and the axes they are drawn on. */
  readonly plan: ChartPlan;
  /** The values, or null when there are none yet. */
  readonly values: AlignedValues | null;
}

/**
 * Builds the options of the chart.
 *
 * The first axis is the time axis, which is written in minutes and seconds; every scale
 * of the plan then gets an axis of its own, on the left for the first and on the right
 * for the others.
 */
function buildOptions(
  plan: ChartPlan,
  width: number,
  height: number,
): uPlot.Options {
  const axes: uPlot.Axis[] = [
    {
      stroke: AXIS_COLOUR,
      grid: { stroke: GRID_COLOUR, width: 1 },
      ticks: { stroke: GRID_COLOUR, width: 1 },
      font: AXIS_FONT,
      size: 22,
      values: (_self, splits) =>
        splits.map((value) => (value === null ? "" : formatElapsed(value))),
    },
  ];

  for (const scale of plan.scales) {
    axes.push({
      scale: scale.key,
      side: scale.side === "left" ? 3 : 1,
      label: scale.unit,
      stroke: AXIS_COLOUR,
      grid: { stroke: GRID_COLOUR, width: 1 },
      ticks: { stroke: GRID_COLOUR, width: 1 },
      font: AXIS_FONT,
      size: 42,
      space: scale.side === "left" ? 42 : 34,
      labelSize: 12,
      labelFont: AXIS_FONT,
    });
  }

  const series: uPlot.Series[] = [
    { label: "time", scale: "x" },
    ...plan.series.map((entry) => ({
      label: entry.name,
      scale: entry.scale,
      stroke: entry.colour,
      width: 1.5,
      spanGaps: false,
      points: { show: false },
    })),
  ];

  return {
    width: Math.max(MINIMUM_SIZE, width),
    height: Math.max(MINIMUM_SIZE, height),
    series,
    axes,
    // The legend of the panel is the legend: it shows the unit and the value of every
    // series, which the legend of the chart does not.
    legend: { show: false },
    // The graph is a live one: the values arrive once a second and the scales follow
    // them, so a zoom the user made would be thrown away a second later. The crosshair
    // stays, because it costs nothing and it says where the pointer is.
    cursor: {
      drag: { x: false, y: false },
      points: { show: false },
    },
    scales: { x: { time: false } },
  };
}

export function GraphChart({ plan, values }: GraphChartProps) {
  const container = useRef<HTMLDivElement | null>(null);
  const chart = useRef<uPlot | null>(null);

  // The chart itself, built once for a plan.
  useEffect(() => {
    const element = container.current;
    if (!element) {
      return;
    }

    const plot = new uPlot(
      buildOptions(plan, element.clientWidth, element.clientHeight),
      [],
      element,
    );
    chart.current = plot;

    // The chart is drawn at the size of the panel, and the panel changes size when the
    // window does, and when the splitter is dragged.
    const observer = new ResizeObserver(() => {
      const width = element.clientWidth;
      const height = element.clientHeight;

      if (width > 0 && height > 0) {
        plot.setSize({ width, height });
      }
    });
    observer.observe(element);

    return () => {
      observer.disconnect();
      plot.destroy();
      chart.current = null;
    };
  }, [plan]);

  // The values. The chart is not rebuilt for them: it is given the new ones.
  useEffect(() => {
    const plot = chart.current;
    if (!plot) {
      return;
    }

    // The values of one plan do not fit another, and a chart given the wrong number of
    // series draws rubbish. An empty set of values is drawn as an empty chart.
    if (!values || values.y.length !== plan.series.length) {
      plot.setData([[]]);
      return;
    }

    const data: uPlot.AlignedData = [values.x, ...values.y];
    plot.setData(data);
  }, [plan, values]);

  return <div className={styles.chart} ref={container} />;
}
