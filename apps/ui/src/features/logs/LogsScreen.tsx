import { useEffect, useMemo, useState } from "react";

import { pickOpenPath, pickSavePath } from "../../lib/ipc/dialog";
import {
  listGraphs,
  logExportCsv,
  logExportKml,
  logMessageSamples,
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
  MessageSamples,
  SampleValue,
  VehicleLog,
} from "../../lib/ipc/types";
import { alignSeries, planChart } from "../../lib/graphs/plan";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useUiStore } from "../../lib/ui/uiStore";
import { GraphChart } from "../graphs/GraphChart";
import styles from "./LogsScreen.module.css";

/**
 * The screen of the logs.
 *
 * A log is what a flight is remembered by, and this is where it is read: the messages
 * the log holds, how many of each there are, and the graphs drawn over it. It is one of
 * the four screens the switcher puts the window on, and leaving it puts the window back
 * on the flight data.
 *
 * The graphs are the same ones the live view draws. The backend evaluates the
 * expressions over the telemetry of the log instead of over the telemetry of the link,
 * and the interface receives numbers with names and never sees a MAVLink field.
 */

/** How a part of the interface says something in the language the window is read in. */
type Say = (source: string, values?: Record<string, string | number>) => string;

/** The kinds of file the dialog offers. */
function logFilters(t: Say) {
  return [
    { name: t("Telemetry log"), extensions: ["tlog"] },
    { name: t("Log of PX4"), extensions: ["ulg"] },
    { name: t("All files"), extensions: ["*"] },
  ];
}

/** How far the play-back moves at a time, in seconds. */
const PLAYBACK_STEP = 0.1;

/**
 * How a value of a sample is written.
 *
 * The numbers of the log are single-precision ones, and a single-precision number
 * written out in full is a wall of digits that says nothing. The operator reads the
 * value, not its representation.
 */
function showValue(value: SampleValue, t: Say): string {
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      return "";
    }

    return Number.isInteger(value) ? String(value) : value.toFixed(4);
  }

  if (typeof value === "boolean") {
    return value ? t("yes") : t("no");
  }

  return value ?? "";
}

export function LogsScreen() {
  // The screen is where this screen lives, so it renders exactly when the window is on
  // it. It stays mounted in the shell while another screen is shown, so that the log that
  // was opened and the graph that was chosen are not forgotten when the operator comes
  // back to it.
  const open = useUiStore((state) => state.screen === "logs");
  const hide = useUiStore((state) => state.hideLog);

  const t = useLocaleStore((state) => state.t);

  const [typed, setTyped] = useState<string>("");
  const [path, setPath] = useState<string | null>(null);
  const [summary, setSummary] = useState<LogSummary | null>(null);
  const [presets, setPresets] = useState<readonly GraphPreset[]>([]);
  const [preset, setPreset] = useState<string>("");
  const [values, setValues] = useState<GraphSeries | null>(null);
  const [error, setError] = useState<CommandError | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  /**
   * How many times the log has been asked for.
   *
   * Reading the same log again is reading it again: the effect that reads it
   * only runs when the path changes, and the path does not change when the
   * operator presses the button again.
   */
  const [revision, setRevision] = useState(0);
  /** The samples of the message that is chosen. */
  const [samples, setSamples] = useState<MessageSamples | null>(null);
  /** The message the samples are of. */
  const [chosen, setChosen] = useState<string>("");

  /** The logs the vehicle holds in its own memory. */
  const [logs, setLogs] = useState<readonly VehicleLog[]>([]);
  /** How long the flight lasted, in seconds. */
  const [flight, setFlight] = useState(0);
  /** Where the play-back is, in seconds. */
  const [position, setPosition] = useState(0);
  /** Whether the flight is being played. */
  const [playing, setPlaying] = useState(false);

  // The catalogue of graphs is read once, when the screen opens.
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

  // The escape key takes the window back to the flight data.
  useEffect(() => {
    if (!open) {
      return;
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        hide();
      }
    };

    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open, hide]);

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

      // The play-back is only for the logs that carry the frames of the link: a log
      // that carries its samples in another way is read, but not played back.
      try {
        const playback = await playbackOpen(path);

        if (!cancelled) {
          setFlight(playback.duration);
          setPosition(0);
        }
      } catch {
        if (!cancelled) {
          setFlight(0);
        }
      }
    };

    void read();

    return () => {
      cancelled = true;
    };
  }, [path, revision]);

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
    const picked = await pickOpenPath(logFilters(t));

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
      { name: t("Log of the vehicle"), extensions: ["log", "bin"] },
    ]);

    if (!picked.ok) {
      return;
    }

    setBusy(true);

    try {
      await downloadLog(log.id, log.size, picked.path);
      setNotice(t("The log was written to {path}.", { path: picked.path }));
      setError(null);
    } catch (cause) {
      setError(normaliseError(cause));
    } finally {
      setBusy(false);
    }
  };

  const readSamples = async (name: string) => {
    if (path === null || name === "") {
      return;
    }

    try {
      const read = await logMessageSamples(path, name);
      setSamples(read);
      setError(null);
    } catch (cause) {
      setError(normaliseError(cause));
      setSamples(null);
    }
  };

  const exportKml = async () => {
    if (path === null) {
      return;
    }

    const picked = await pickSavePath("flight.kml", [
      { name: t("KML document"), extensions: ["kml"] },
    ]);

    if (!picked.ok) {
      return;
    }

    setBusy(true);

    try {
      await logExportKml(path, picked.path);
      setNotice(t("The track was written to {path}.", { path: picked.path }));
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
      { name: t("CSV document"), extensions: ["csv"] },
    ]);

    if (!picked.ok) {
      return;
    }

    setBusy(true);

    try {
      await logExportCsv(path, preset, picked.path);
      setNotice(
        t("The samples were written to {path}.", { path: picked.path }),
      );
      setError(null);
    } catch (cause) {
      setError(normaliseError(cause));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={styles.screen}>
      <header className={styles.header}>
        <h1 className={styles.title} id="log-title">
          {t("Logs")}
        </h1>

        <div className={styles.headerActions}>
          <button
            type="button"
            className="button"
            disabled={busy}
            onClick={() => {
              void openFile();
            }}
          >
            {t("Open a log…")}
          </button>

          <button type="button" className="button" onClick={hide}>
            {t("Close")}
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
            placeholder={t("the name of the log")}
            aria-label={t("The name of the log")}
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
              setRevision((count) => count + 1);
            }}
          >
            {t("Read the log")}
          </button>
        </div>

        {path === null ? (
          <p className={styles.note}>
            {t(
              "No log is open. A log is what a flight is remembered by: it is the stream of the frames that crossed the link, each of them with the instant it was seen.",
            )}
          </p>
        ) : null}

        {flight > 0 ? (
          <div className={styles.scrub}>
            <button
              type="button"
              className="button"
              disabled={busy}
              title={playing ? t("Pause the flight") : t("Play the flight")}
              onClick={() => {
                setPlaying(!playing);
              }}
            >
              {playing ? t("Pause") : t("Play")}
            </button>

            <input
              className={styles.scrubBar}
              type="range"
              min={0}
              max={flight}
              step={PLAYBACK_STEP}
              value={Math.min(position, flight)}
              aria-label={t("Where the flight is")}
              onChange={(event) => {
                const value = Number(event.target.value);
                setPosition(value);
                void playbackSeek(value);
              }}
            />

            <span className={styles.scrubTime}>
              {t("{elapsed} s of {total} s", {
                elapsed: position.toFixed(1),
                total: flight.toFixed(1),
              })}
            </span>
          </div>
        ) : null}

        {summary !== null ? (
          <>
            <p className={styles.note}>
              {[
                t(
                  summary.records === 1
                    ? "{records} record"
                    : "{records} records",
                  { records: summary.records },
                ),
                t(duration === 1 ? "{seconds} second" : "{seconds} seconds", {
                  seconds: duration.toFixed(1),
                }),
                t(
                  summary.messages.length === 1
                    ? "{kinds} kind of message"
                    : "{kinds} kinds of message",
                  { kinds: summary.messages.length },
                ),
              ].join(", ")}
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
            title={t("Ask the vehicle for the logs it holds in its own memory")}
            onClick={() => {
              void readVehicleLogs();
            }}
          >
            {t("The logs of the vehicle")}
          </button>
        </div>

        {logs.length > 0 ? (
          <div className={styles.messages}>
            {logs.map((log) => (
              <span key={log.id} className={styles.message}>
                <span className={styles.messageName}>
                  {t("Log {number} · {size} kB", {
                    number: log.id,
                    size: (log.size / 1024).toFixed(1),
                  })}
                </span>
                <button
                  type="button"
                  className="button"
                  disabled={busy}
                  onClick={() => {
                    void fetchLog(log);
                  }}
                >
                  {t("Download")}
                </button>
              </span>
            ))}
          </div>
        ) : null}

        {summary !== null && summary.messages.length > 0 ? (
          <>
            <label className={styles.field}>
              <span className={styles.label}>{t("The samples of")}</span>
              <select
                className="select"
                value={chosen}
                aria-label={t("The message the samples are of")}
                onChange={(event) => {
                  setChosen(event.target.value);
                  void readSamples(event.target.value);
                }}
              >
                <option value="">{t("Choose a message")}</option>
                {summary.messages.map((message) => (
                  <option key={message.name} value={message.name}>
                    {`${message.name} (${message.count})`}
                  </option>
                ))}
              </select>
            </label>

            {samples !== null ? (
              <div className={styles.grid}>
                <table className={styles.table}>
                  <thead>
                    <tr>
                      <th scope="col">{t("Seconds")}</th>
                      {samples.fields.map((field) => (
                        <th key={field} scope="col">
                          {field}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {samples.rows.map((row, index) => (
                      // The rows are the samples of the log, and they have no identity
                      // of their own: the position in the list is the identity.
                      // eslint-disable-next-line react/no-array-index-key
                      <tr
                        key={index}
                        className={styles.row}
                        onClick={() => {
                          const at = samples.instants[index];

                          if (at === undefined) {
                            return;
                          }

                          setPosition(at);
                          void playbackSeek(at);
                        }}
                      >
                        <td className={styles.instant}>
                          {(samples.instants[index] ?? 0).toFixed(2)}
                        </td>
                        {row.map((value, column) => (
                          <td key={column}>{showValue(value, t)}</td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>

                <p className={styles.note}>
                  {t(
                    samples.rows.length === 1
                      ? "{shown} of {total} sample. Click a row and the flight is moved to that moment."
                      : "{shown} of {total} samples. Click a row and the flight is moved to that moment.",
                    { shown: samples.rows.length, total: samples.total },
                  )}
                </p>
              </div>
            ) : null}
          </>
        ) : null}

        {presets.length > 0 ? (
          <label className={styles.field}>
            <span className={styles.label}>{t("Graph")}</span>
            <select
              className="select"
              value={preset}
              aria-label={t("The graph drawn over the log")}
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
              title={t("Write the track of the log as a KML document")}
              onClick={() => {
                void exportKml();
              }}
            >
              {t("Write the track (KML)")}
            </button>

            <button
              type="button"
              className="button"
              disabled={busy || preset === ""}
              title={t("Write the samples of the graph as a CSV document")}
              onClick={() => {
                void exportCsv();
              }}
            >
              {t("Write the samples (CSV)")}
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
  );
}
