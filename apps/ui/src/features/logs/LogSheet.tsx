import { useEffect, useMemo, useState } from "react";

import { pickOpenPath } from "../../lib/ipc/dialog";
import {
  listGraphs,
  logSeries,
  logSummary,
  normaliseError,
} from "../../lib/ipc/bridge";
import type {
  CommandError,
  GraphPreset,
  GraphSeries,
  LogSummary,
} from "../../lib/ipc/types";
import { alignSeries, planChart } from "../../lib/graphs/plan";
import { useUiStore } from "../../lib/ui/uiStore";
import { GraphChart } from "../graphs/GraphChart";
import styles from "./LogSheet.module.css";

/**
 * The sheet of the logs.
 *
 * A log is what a flight is remembered by, and this is where it is read: the messages
 * the log holds, how many of each there are, and the graphs drawn over it.
 *
 * The graphs are the same ones the live view draws. The backend evaluates the
 * expressions over the telemetry of the log instead of over the telemetry of the link,
 * and the interface receives numbers with names and never sees a MAVLink field.
 */

/** The kinds of file the dialog offers. */
const LOG_FILTERS = [
  { name: "Telemetry log", extensions: ["tlog"] },
  { name: "All files", extensions: ["*"] },
];

export function LogSheet() {
  const open = useUiStore((state) => state.logOpen);
  const hide = useUiStore((state) => state.hideLog);

  const [typed, setTyped] = useState<string>("");
  const [path, setPath] = useState<string | null>(null);
  const [summary, setSummary] = useState<LogSummary | null>(null);
  const [presets, setPresets] = useState<readonly GraphPreset[]>([]);
  const [preset, setPreset] = useState<string>("");
  const [values, setValues] = useState<GraphSeries | null>(null);
  const [error, setError] = useState<CommandError | null>(null);
  const [busy, setBusy] = useState(false);

  // The catalogue of graphs is read once, when the sheet opens.
  useEffect(() => {
    if (!open) {
      return;
    }

    let cancelled = false;

    const read = async () => {
      try {
        const catalogue = await listGraphs();
        if (cancelled) {
          return;
        }
        setPresets(catalogue);
        setPreset((current) => current || (catalogue[0]?.name ?? ""));
      } catch (cause) {
        if (!cancelled) {
          setError(normaliseError(cause));
        }
      }
    };

    void read();

    return () => {
      cancelled = true;
    };
  }, [open]);

  // The log is read when it is opened, and the graph of it when the graph is chosen.
  useEffect(() => {
    if (path === null) {
      return;
    }

    let cancelled = false;

    const read = async () => {
      setBusy(true);

      try {
        const read = await logSummary(path);
        if (!cancelled) {
          setSummary(read);
          setError(null);
        }
      } catch (cause) {
        if (!cancelled) {
          setError(normaliseError(cause));
          setSummary(null);
        }
      } finally {
        if (!cancelled) {
          setBusy(false);
        }
      }
    };

    void read();

    return () => {
      cancelled = true;
    };
  }, [path]);

  useEffect(() => {
    if (path === null || preset === "") {
      return;
    }

    let cancelled = false;

    const read = async () => {
      try {
        const series = await logSeries(path, preset);
        if (!cancelled) {
          setValues(series);
        }
      } catch (cause) {
        if (!cancelled) {
          setError(normaliseError(cause));
          setValues(null);
        }
      }
    };

    void read();

    return () => {
      cancelled = true;
    };
  }, [path, preset]);

  const plan = useMemo(
    () =>
      planChart(
        (values?.series ?? []).map((entry) => ({
          name: entry.name,
          unit: entry.unit,
        })),
      ),
    [values],
  );

  const aligned = useMemo(
    () => (values === null ? null : alignSeries(values.series)),
    [values],
  );

  if (!open) {
    return null;
  }

  const openFile = async () => {
    const picked = await pickOpenPath(LOG_FILTERS);

    if (!picked.ok) {
      return;
    }

    setTyped(picked.path);
    setSummary(null);
    setValues(null);
    setPath(picked.path);
  };

  const duration =
    summary === null || summary.end <= summary.start
      ? 0
      : (summary.end - summary.start) / 1_000_000;

  return (
    <div
      className={styles.backdrop}
      role="presentation"
      onClick={() => {
        hide();
      }}
    >
      <div
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-labelledby="log-title"
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <header className={styles.header}>
          <h2 className={styles.title} id="log-title">
            Logs
          </h2>

          <div className={styles.headerActions}>
            <button
              type="button"
              className="button"
              disabled={busy}
              onClick={() => {
                void openFile();
              }}
            >
              Open a log…
            </button>

            <button type="button" className="button" onClick={hide}>
              Close
            </button>
          </div>
        </header>

        <div className={styles.body}>
          <div className={styles.pathRow}>
            <input
              className="input"
              type="text"
              spellCheck={false}
              autoComplete="off"
              value={typed}
              placeholder="the name of the log"
              aria-label="The name of the log"
              onChange={(event) => {
                setTyped(event.target.value);
              }}
            />

            <button
              type="button"
              className="button button--primary"
              disabled={busy || typed === ""}
              onClick={() => {
                setSummary(null);
                setValues(null);
                setPath(typed);
              }}
            >
              Read
            </button>
          </div>

          {path === null ? (
            <p className={styles.note}>
              No log is open. A log is what a flight is remembered by: it is the stream of
              the frames that crossed the link, each of them with the instant it was
              seen.
            </p>
          ) : null}

          {summary !== null ? (
            <>
              <p className={styles.note}>
                {summary.records} records, {duration.toFixed(1)} seconds,{" "}
                {summary.messages.length} kinds of message.
              </p>

              <div className={styles.messages}>
                {summary.messages.map((message) => (
                  <span key={message.name} className={styles.message}>
                    <span className={styles.messageName}>{message.name}</span>
                    <span className={styles.messageCount}>{message.count}</span>
                  </span>
                ))}
              </div>
            </>
          ) : null}

          {presets.length > 0 ? (
            <label className={styles.field}>
              <span className={styles.label}>Graph</span>
              <select
                className="select"
                value={preset}
                aria-label="The graph drawn over the log"
                onChange={(event) => {
                  setPreset(event.target.value);
                }}
              >
                {presets.map((entry) => (
                  <option key={entry.name} value={entry.name}>
                    {entry.group} · {entry.label}
                  </option>
                ))}
              </select>
            </label>
          ) : null}

          {aligned !== null && plan.series.length > 0 ? (
            <div className={styles.chart}>
              <GraphChart plan={plan} values={aligned} />
            </div>
          ) : null}

          {error !== null ? (
            <p className={styles.error} role="alert">
              {error.code} — {error.message}
            </p>
          ) : null}
        </div>
      </div>
    </div>
  );
}
