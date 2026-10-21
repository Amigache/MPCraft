import { useEffect, useMemo, useState } from "react";

import { pickOpenPath, pickSavePath } from "../../lib/ipc/dialog";
import {
  listGraphs,
  logExportCsv,
  logExportKml,
  logSeries,
  logSummary,
  downloadLog,
  normaliseError,
  playbackOpen,
  playbackSeek,
  vehicleLogs,
} from "../../lib/ipc/bridge";
import type {
  CommandError,
  GraphPreset,
  GraphSeries,
  LogSummary,
  VehicleLog,
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

/** How far the play-back moves at a time, in seconds. */
const PLAYBACK_STEP = 0.1;

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
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  /** The logs the vehicle holds in its own memory. */
  const [logs, setLogs] = useState<readonly VehicleLog[]>([]);
  /** How long the flight lasted, in seconds. */
  const [flight, setFlight] = useState(0);
  /** Where the play-back is, in seconds. */
  const [position, setPosition] = useState(0);
  /** Whether the flight is being played. */
  const [playing, setPlaying] = useState(false);

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
        const playback = await playbackOpen(path);

        if (!cancelled) {
          setSummary(read);
          setFlight(playback.duration);
          setPosition(0);
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

  // The flight plays itself: the position moves and the window is told where it is.
  // The play-back is the backend's, so that the flight is the same one whatever the
  // interface does with it.
  useEffect(() => {
    if (!playing || flight === 0) {
      return;
    }

    const timer = window.setInterval(() => {
      setPosition((current) => {
        const next = current + PLAYBACK_STEP;

        if (next >= flight) {
          setPlaying(false);
          return flight;
        }

        void playbackSeek(next);

        return next;
      });
    }, PLAYBACK_STEP * 1000);

    return () => {
      window.clearInterval(timer);
    };
  }, [flight, playing]);

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

  const readVehicleLogs = async () => {
    setBusy(true);

    try {
      const list = await vehicleLogs();
      setLogs(list);
      setError(null);
    } catch (cause) {
      setError(normaliseError(cause));
    } finally {
      setBusy(false);
    }
  };

  const fetchLog = async (log: VehicleLog) => {
    const picked = await pickSavePath(`log-${log.id}.log`, [
      { name: "Log of the vehicle", extensions: ["log", "bin"] },
    ]);

    if (!picked.ok) {
      return;
    }

    setBusy(true);

    try {
      await downloadLog(log.id, log.size, picked.path);
      setNotice(`The log was written to ${picked.path}.`);
      setError(null);
    } catch (cause) {
      setError(normaliseError(cause));
    } finally {
      setBusy(false);
    }
  };

  const exportKml = async () => {
    if (path === null) {
      return;
    }

    const picked = await pickSavePath("flight.kml", [
      { name: "KML document", extensions: ["kml"] },
    ]);

    if (!picked.ok) {
      return;
    }

    setBusy(true);

    try {
      await logExportKml(path, picked.path);
      setNotice(`The track was written to ${picked.path}.`);
      setError(null);
    } catch (cause) {
      setError(normaliseError(cause));
    } finally {
      setBusy(false);
    }
  };

  const exportCsv = async () => {
    if (path === null || preset === "") {
      return;
    }

    const picked = await pickSavePath("samples.csv", [
      { name: "CSV document", extensions: ["csv"] },
    ]);

    if (!picked.ok) {
      return;
    }

    setBusy(true);

    try {
      await logExportCsv(path, preset, picked.path);
      setNotice(`The samples were written to ${picked.path}.`);
      setError(null);
    } catch (cause) {
      setError(normaliseError(cause));
    } finally {
      setBusy(false);
    }
  };

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

          {flight > 0 ? (
            <div className={styles.scrub}>
              <button
                type="button"
                className="button"
                disabled={busy}
                title={playing ? "Pause the flight" : "Play the flight"}
                onClick={() => {
                  setPlaying(!playing);
                }}
              >
                {playing ? "Pause" : "Play"}
              </button>

              <input
                className={styles.scrubBar}
                type="range"
                min={0}
                max={flight}
                step={PLAYBACK_STEP}
                value={Math.min(position, flight)}
                aria-label="Where the flight is"
                onChange={(event) => {
                  const value = Number(event.target.value);
                  setPosition(value);
                  void playbackSeek(value);
                }}
              />

              <span className={styles.scrubTime}>
                {position.toFixed(1)} s of {flight.toFixed(1)} s
              </span>
            </div>
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

          <div className={styles.actions}>
            <button
              type="button"
              className="button"
              disabled={busy}
              title="Ask the vehicle for the logs it holds in its own memory"
              onClick={() => {
                void readVehicleLogs();
              }}
            >
              The logs of the vehicle
            </button>
          </div>

          {logs.length > 0 ? (
            <div className={styles.messages}>
              {logs.map((log) => (
                <span key={log.id} className={styles.message}>
                  <span className={styles.messageName}>
                    {`Log ${log.id} · ${(log.size / 1024).toFixed(1)} kB`}
                  </span>
                  <button
                    type="button"
                    className="button"
                    disabled={busy}
                    onClick={() => {
                      void fetchLog(log);
                    }}
                  >
                    Download
                  </button>
                </span>
              ))}
            </div>
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

          {path !== null ? (
            <div className={styles.actions}>
              <button
                type="button"
                className="button"
                disabled={busy}
                title="Write the track of the log as a KML document"
                onClick={() => {
                  void exportKml();
                }}
              >
                Export the track (KML)
              </button>

              <button
                type="button"
                className="button"
                disabled={busy || preset === ""}
                title="Write the samples of the graph as a CSV document"
                onClick={() => {
                  void exportCsv();
                }}
              >
                Export the samples (CSV)
              </button>
            </div>
          ) : null}

          {notice !== null ? (
            <p className={styles.notice} role="status">
              {notice}
            </p>
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
