import { useEffect, useMemo, useState } from "react";

import { formatNumber } from "../../lib/format";
import { graphSeries, listGraphs } from "../../lib/ipc/bridge";
import type { CommandError, GraphPreset, GraphSeries } from "../../lib/ipc/types";
import { alignSeries, latestValues, planChart } from "../../lib/graphs/plan";
import { useLinkStore } from "../../lib/link/linkStore";
import { useUiStore } from "../../lib/ui/uiStore";
import { GraphChart } from "./GraphChart";
import styles from "./GraphPanel.module.css";

/**
 * The graphs.
 *
 * The catalogue of graphs is the one Mission Planner has, ported, and the backend
 * evaluates the expressions: the panel receives numbers with names and never sees a
 * telemetry field.
 *
 * The values are asked for once a second rather than on every tick. The backend keeps
 * five minutes of history, and asking for it twenty times a second would be absurd; a
 * graph of a flight is read at the speed of a flight.
 */

/** How often the values are asked for, in milliseconds. */
const REFRESH_MS = 1000;

/** How far back the graph looks, in seconds. */
const WINDOW_SECONDS = 120;

/** What is shown while the catalogue is on its way. */
type CatalogueState = "loading" | "ready" | "failed";

export function GraphPanel() {
  const [presets, setPresets] = useState<readonly GraphPreset[]>([]);
  const [values, setValues] = useState<GraphSeries | null>(null);
  const [error, setError] = useState<CommandError | null>(null);
  const [state, setState] = useState<CatalogueState>("loading");

  const graphPreset = useUiStore((store) => store.graphPreset);
  const setGraphPreset = useUiStore((store) => store.setGraphPreset);
  const toggleGraphs = useUiStore((store) => store.toggleGraphs);

  // Where the play-back is. A log that is being played back is what the graph draws,
  // and the line that says where the play-head is is the moment the grid of the
  // samples and the map are showing too.
  const playback = useLinkStore((store) => store.playback);
  const playbackPosition = useLinkStore((store) => store.playbackPosition);
  const playHead = playback ? playbackPosition : null;

  // The catalogue. It is read once, when the panel opens.
  useEffect(() => {
    let cancelled = false;

    const read = async () => {
      try {
        const catalogue = await listGraphs();
        if (cancelled) {
          return;
        }
        setPresets(catalogue);
        setState("ready");
      } catch (cause) {
        if (!cancelled) {
          setError(cause as CommandError);
          setState("failed");
        }
      }
    };

    void read();

    return () => {
      cancelled = true;
    };
  }, []);

  // The chosen graph. When the one that was chosen is not in the catalogue — a
  // catalogue that has changed since it was chosen — the first one is taken.
  useEffect(() => {
    if (presets.length === 0) {
      return;
    }
    if (presets.some((entry) => entry.name === graphPreset)) {
      return;
    }
    setGraphPreset(presets[0]?.name ?? "");
  }, [graphPreset, presets, setGraphPreset]);

  const preset = useMemo(
    () => presets.find((entry) => entry.name === graphPreset) ?? presets[0] ?? null,
    [graphPreset, presets],
  );

  // The values of the chosen graph, asked for once a second.
  useEffect(() => {
    if (!preset) {
      return;
    }

    let cancelled = false;

    const refresh = async () => {
      try {
        // Only the recent past is asked for: the backend keeps five minutes, and
        // fetching all of it every second would send megabytes for nothing.
        const series = await graphSeries(preset.name, WINDOW_SECONDS);
        if (cancelled) {
          return;
        }
        setValues(series);
        setError(null);
      } catch (cause) {
        if (!cancelled) {
          setError(cause as CommandError);
        }
      }
    };

    // The values of the graph that was chosen before belong to another graph, and are
    // not drawn on this one.
    setValues(null);
    void refresh();

    const timer = window.setInterval(() => {
      void refresh();
    }, REFRESH_MS);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [preset]);

  const plan = useMemo(() => planChart(preset?.series ?? []), [preset]);

  // The values that belong to the graph that is on the screen.
  const current = useMemo(
    () =>
      values && values.name === preset?.name ? values.series : ([] as const),
    [preset, values],
  );

  const aligned = useMemo(
    () =>
      values && values.name === preset?.name ? alignSeries(values.series) : null,
    [preset, values],
  );

  const latest = useMemo(() => latestValues(current), [current]);

  const groups = useMemo(() => {
    const byGroup = new Map<string, GraphPreset[]>();

    for (const entry of presets) {
      const list = byGroup.get(entry.group);
      if (list) {
        list.push(entry);
      } else {
        byGroup.set(entry.group, [entry]);
      }
    }

    return [...byGroup.entries()];
  }, [presets]);

  const empty = aligned === null || aligned.x.length === 0;

  return (
    <div className={styles.panel}>
      <div className={styles.head}>
        <select
          className={`select ${styles.preset}`}
          value={preset?.name ?? ""}
          aria-label="The graph"
          disabled={presets.length === 0}
          onChange={(event) => setGraphPreset(event.target.value)}
        >
          {groups.map(([group, entries]) => (
            <optgroup key={group} label={group}>
              {entries.map((entry) => (
                <option key={entry.name} value={entry.name}>
                  {entry.label}
                  {entry.upstream ? "" : " — MPCraft"}
                </option>
              ))}
            </optgroup>
          ))}
        </select>

        <span className={styles.description} title={preset?.description ?? ""}>
          {preset?.description ?? ""}
        </span>

        <button
          type="button"
          className="button"
          onClick={toggleGraphs}
          title="Hide the graphs and give the map the whole area"
        >
          Hide graphs
        </button>
      </div>

      <div className={styles.plot}>
        <GraphChart plan={plan} values={aligned} playHead={playHead} />

        {empty ? (
          <p className={styles.empty}>
            {state === "failed"
              ? "The catalogue of graphs could not be read."
              : "Waiting for the vehicle to report."}
          </p>
        ) : null}
      </div>

      <ul className={styles.legend}>
        {plan.series.map((entry, index) => (
          <li className={styles.entry} key={entry.name}>
            <span
              className={styles.swatch}
              style={{ background: entry.colour }}
              aria-hidden="true"
            />
            <span className={styles.name}>{entry.name}</span>
            <span className={styles.value}>{formatNumber(latest[index] ?? null, 2)}</span>
            {entry.unit ? <span className={styles.unit}>{entry.unit}</span> : null}
          </li>
        ))}
      </ul>

      {error ? (
        <p className={styles.error} role="alert">
          <span className={styles.code}>{error.code}</span> {error.message}
        </p>
      ) : null}
    </div>
  );
}
