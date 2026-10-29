import { useCallback, useEffect, useState } from "react";

import { formatCount } from "../../lib/format";
import {
  mirrorsStatus,
  normaliseError,
  setMirrors,
  startMirror,
  stopMirror,
} from "../../lib/ipc/bridge";
import type { MirrorStatus } from "../../lib/ipc/types";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./MirrorsSheet.module.css";

/**
 * The sheet of the link copied to another program.
 *
 * Every packet the station reads from the vehicle is copied to each mirror, so a second station,
 * a recorder or a script can follow the flight without a link of its own. A row that writes also
 * puts what the other program sends back on the link. Upstream has the same list in
 * `Controls/SerialOutputPass.cs`, and the copy is made of the bytes as they arrived rather than of
 * a re-encoding of the packets.
 *
 * The rows are settings and are read without a link; starting one needs the link, and the sheet
 * says so rather than pretending a row is running when nothing is connected.
 */
export function MirrorsSheet() {
  const open = useUiStore((state) => state.mirrorsOpen);
  const hide = useUiStore((state) => state.hideMirrors);

  const t = useLocaleStore((state) => state.t);

  const [rows, setRows] = useState<readonly MirrorStatus[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const read = useCallback(async () => {
    try {
      setRows(await mirrorsStatus());
    } catch {
      // The rows are only read to be shown; a link that is not open says so when one is started.
    }
  }, []);

  useEffect(() => {
    if (!open) {
      return;
    }

    void read();

    const timer = window.setInterval(() => {
      void read();
    }, 1000);

    return () => {
      window.clearInterval(timer);
    };
  }, [open, read]);

  const edit = (index: number, change: (row: MirrorStatus) => MirrorStatus) => {
    setRows((before) =>
      before.map((row, at) => (at === index ? change(row) : row)),
    );
  };

  const save = async () => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      const saved = await setMirrors(rows.map((row) => row.spec));
      setRows(saved);
      setNotice(t("The rows are saved."));
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  const add = () => {
    setRows((before) => [
      ...before,
      {
        key: "",
        spec: {
          protocol: "tcp",
          direction: "outbound",
          port: "14550",
          extra: "127.0.0.1",
          write: false,
        },
        running: false,
        sent: 0,
        received: 0,
        error: null,
      },
    ]);
  };

  const remove = async (row: MirrorStatus, index: number) => {
    setError(null);

    if (row.running) {
      try {
        setRows(await stopMirror(row.key));
      } catch (cause) {
        setError(normaliseError(cause).message);
        return;
      }
    }

    setRows((before) => before.filter((_, at) => at !== index));
  };

  const toggle = async (row: MirrorStatus) => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      setRows(
        row.running ? await stopMirror(row.key) : await startMirror(row.key),
      );
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  if (!open) {
    return null;
  }

  return (
    <div className={styles.backdrop} role="presentation" onClick={hide}>
      <div
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-label={t("The serial output")}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>{t("The serial output")}</h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "Each row copies the link to another program. A row that writes also puts what it receives back on the link.",
            )}
          </p>

          {error !== null ? <p className={styles.error}>{error}</p> : null}
          {notice !== null ? <p className={styles.text}>{notice}</p> : null}

          <div className={styles.rows}>
            {rows.map((row, index) => (
              <div className={styles.row} key={row.key || `new-${index}`}>
                <div className={styles.rowFields}>
                  <label className={styles.field}>
                    <span className={styles.label}>{t("Protocol")}</span>
                    <select
                      className="select"
                      value={row.spec.protocol}
                      disabled={row.running}
                      aria-label={t("How the mirror reaches the other program")}
                      onChange={(event) => {
                        const protocol = event.target
                          .value as MirrorStatus["spec"]["protocol"];

                        edit(index, (current) => ({
                          ...current,
                          spec: {
                            ...current.spec,
                            protocol,
                            direction:
                              protocol === "serial"
                                ? "outbound"
                                : current.spec.direction,
                          },
                        }));
                      }}
                    >
                      <option value="serial">Serial</option>
                      <option value="tcp">TCP</option>
                      <option value="udp">UDP</option>
                    </select>
                  </label>

                  {row.spec.protocol === "serial" ? null : (
                    <label className={styles.field}>
                      <span className={styles.label}>{t("Direction")}</span>
                      <select
                        className="select"
                        value={row.spec.direction}
                        disabled={row.running}
                        aria-label={t("Which side opens the connection")}
                        onChange={(event) => {
                          const direction = event.target
                            .value as MirrorStatus["spec"]["direction"];

                          edit(index, (current) => ({
                            ...current,
                            spec: { ...current.spec, direction },
                          }));
                        }}
                      >
                        <option value="outbound">Outbound</option>
                        <option value="inbound">Inbound</option>
                      </select>
                    </label>
                  )}

                  <label className={styles.field}>
                    <span className={styles.label}>{t("Port")}</span>
                    <input
                      className="input"
                      type="text"
                      spellCheck={false}
                      value={row.spec.port}
                      disabled={row.running}
                      aria-label={t("The port of the mirror")}
                      onChange={(event) => {
                        const port = event.target.value;

                        edit(index, (current) => ({
                          ...current,
                          spec: { ...current.spec, port },
                        }));
                      }}
                    />
                  </label>

                  <label className={styles.field}>
                    <span className={styles.label}>
                      {row.spec.protocol === "serial"
                        ? t("Baud rate")
                        : t("Host")}
                    </span>
                    <input
                      className="input"
                      type="text"
                      spellCheck={false}
                      value={row.spec.extra}
                      disabled={row.running}
                      aria-label={
                        row.spec.protocol === "serial"
                          ? t("The baud rate of the port")
                          : t("The host of the mirror")
                      }
                      onChange={(event) => {
                        const extra = event.target.value;

                        edit(index, (current) => ({
                          ...current,
                          spec: { ...current.spec, extra },
                        }));
                      }}
                    />
                  </label>

                  <label className={styles.check}>
                    <input
                      type="checkbox"
                      checked={row.spec.write}
                      disabled={row.running}
                      onChange={(event) => {
                        const write = event.target.checked;

                        edit(index, (current) => ({
                          ...current,
                          spec: { ...current.spec, write },
                        }));
                      }}
                    />
                    <span className={styles.label}>{t("Write back")}</span>
                  </label>
                </div>

                <div className={styles.rowActions}>
                  <button
                    type="button"
                    className="button"
                    disabled={busy || (!row.running && row.key === "")}
                    onClick={() => {
                      void toggle(row);
                    }}
                  >
                    {row.running ? t("Stop") : t("Go")}
                  </button>
                  <button
                    type="button"
                    className="button"
                    disabled={busy}
                    onClick={() => {
                      void remove(row, index);
                    }}
                  >
                    {t("Remove")}
                  </button>

                  {row.running ? (
                    <span className={styles.count}>
                      {formatCount(row.sent)} / {formatCount(row.received)}
                    </span>
                  ) : null}
                </div>

                {row.error !== null ? (
                  <p className={styles.error}>{row.error}</p>
                ) : null}
              </div>
            ))}
          </div>

          <div className={styles.actions}>
            <button type="button" className="button" onClick={add}>
              {t("Add a row")}
            </button>
            <button
              type="button"
              className="button"
              disabled={busy}
              onClick={() => {
                void save();
              }}
            >
              {t("Save the rows")}
            </button>
          </div>
        </section>
      </div>
    </div>
  );
}
