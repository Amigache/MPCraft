import { useCallback, useEffect, useState } from "react";

import {
  dronecanParameters,
  dronecanRestartNode,
  dronecanSaveParameters,
  dronecanScan,
  dronecanSlcanClose,
  dronecanSlcanOpen,
  dronecanSlcanParameters,
  dronecanSlcanRestartNode,
  dronecanSlcanSaveParameters,
  dronecanSlcanWriteParameter,
  dronecanWriteParameter,
  normaliseError,
} from "../../lib/ipc/bridge";
import type {
  DronecanNode,
  DronecanReport,
  NodeParameter,
  SlcanStatus,
} from "../../lib/ipc/types";
import { DASH, formatCount } from "../../lib/format";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useLinkStore } from "../../lib/link/linkStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./DronecanSheet.module.css";

/**
 * The sheet of the nodes of a DroneCAN bus.
 *
 * A vehicle with a CAN bus has devices on it that are not the autopilot, and the bus says what
 * they are: each node reports how long it has been up and how it is, and the station can ask for
 * what it says about itself — its name, its hardware and software versions, and the number its
 * build carries.
 *
 * What the sheet shows is what the bus said, not what was asked for. A health or mode word is the
 * name the dictionary gives the number, and a number the dictionary does not give is shown as the
 * number rather than as a word invented for it. A node that reported how it is without saying what
 * it is stays on the list as one whose information is missing, which is the case the dictionary
 * says has to be asked for again — once, because a bus that will not answer is not answered by
 * asking again.
 *
 * Upstream keeps the same nodes under the same pair (`MAVLinkInterface.cs`, lines 5271–5290), and
 * sends the ask without waiting for an acknowledgement.
 */
export function DronecanSheet() {
  const open = useUiStore((state) => state.dronecanOpen);
  const hide = useUiStore((state) => state.hideDronecan);

  const t = useLocaleStore((state) => state.t);

  const status = useLinkStore((state) => state.status);
  const linked = status === "connected" || status === "reconnecting";

  const [report, setReport] = useState<DronecanReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  /** The node whose parameters are open, and which way they are reached by. */
  const [openNode, setOpenNode] = useState<{
    systemId: number;
    componentId: number;
    via: "link" | "adapter";
  } | null>(null);

  /** The adapter the operator opened, when one is. */
  const [adapter, setAdapter] = useState<SlcanStatus | null>(null);
  const [port, setPort] = useState("COM3");
  const [rate, setRate] = useState(500000);
  const [adapterNode, setAdapterNode] = useState(42);
  const [adapterBusy, setAdapterBusy] = useState(false);
  const [parameters, setParameters] = useState<readonly NodeParameter[]>([]);
  const [bus, setBus] = useState(1);
  const [busyParameters, setBusyParameters] = useState(false);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [note, setNote] = useState<string | null>(null);

  const loadParameters = useCallback(
    async (
      which: { systemId: number; componentId: number; via: "link" | "adapter" },
      on: number,
    ) => {
      setBusyParameters(true);
      setError(null);
      setNote(null);

      try {
        setParameters(
          which.via === "adapter"
            ? await dronecanSlcanParameters(which.componentId)
            : await dronecanParameters(
                { systemId: which.systemId, componentId: which.componentId },
                on,
                which.componentId,
              ),
        );
      } catch (cause) {
        setError(normaliseError(cause).message);
      } finally {
        setBusyParameters(false);
      }
    },
    [],
  );

  const openAdapter = async () => {
    setAdapterBusy(true);
    setError(null);
    setNote(null);

    try {
      const opened = await dronecanSlcanOpen(port, rate);

      setAdapter(opened);
      setNote(
        t("The adapter on {port} is open at {rate} bits per second.", {
          port: opened.port,
          rate: formatCount(opened.rate),
        }),
      );
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setAdapterBusy(false);
    }
  };

  const closeAdapter = async () => {
    setAdapterBusy(true);

    try {
      await dronecanSlcanClose();
      setAdapter(null);
      setNote(t("The adapter is closed."));
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setAdapterBusy(false);
    }
  };

  const read = useCallback(async () => {
    if (!linked) {
      return;
    }

    try {
      setReport(await dronecanScan());
    } catch (cause) {
      setError(normaliseError(cause).message);
    }
  }, [linked]);

  useEffect(() => {
    if (!open) {
      return;
    }

    void read();

    const timer = window.setInterval(() => {
      void read();
    }, 2000);

    return () => {
      window.clearInterval(timer);
    };
  }, [open, read]);

  const writeParameter = async (name: string) => {
    if (openNode === null) {
      return;
    }

    setBusyParameters(true);
    setError(null);
    setNote(null);

    try {
      const written = drafts[name] ?? "";
      const stored =
        openNode.via === "adapter"
          ? await dronecanSlcanWriteParameter(
              openNode.componentId,
              name,
              written,
            )
          : await dronecanWriteParameter(
              {
                systemId: openNode.systemId,
                componentId: openNode.componentId,
              },
              bus,
              openNode.componentId,
              name,
              written,
            );

      setNote(
        t("The node stored {name} = {value}.", {
          name: stored.name,
          value: stored.value,
        }),
      );
      setDrafts((current) => ({ ...current, [name]: "" }));

      await loadParameters(openNode, bus);
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusyParameters(false);
    }
  };

  const saveParameters = async () => {
    if (openNode === null) {
      return;
    }

    setBusyParameters(true);
    setError(null);
    setNote(null);

    try {
      if (openNode.via === "adapter") {
        await dronecanSlcanSaveParameters(openNode.componentId);
      } else {
        await dronecanSaveParameters(
          { systemId: openNode.systemId, componentId: openNode.componentId },
          bus,
          openNode.componentId,
        );
      }
      setNote(t("The node was asked to keep what it holds."));
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusyParameters(false);
    }
  };

  const restartNode = async () => {
    if (openNode === null) {
      return;
    }

    setBusyParameters(true);
    setError(null);
    setNote(null);

    try {
      if (openNode.via === "adapter") {
        await dronecanSlcanRestartNode(openNode.componentId);
      } else {
        await dronecanRestartNode(
          { systemId: openNode.systemId, componentId: openNode.componentId },
          bus,
          openNode.componentId,
        );
      }
      setNote(t("The node was asked to restart."));
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusyParameters(false);
    }
  };

  if (!open) {
    return null;
  }

  const nodes: readonly DronecanNode[] = report?.nodes ?? [];

  return (
    <div className={styles.backdrop} role="presentation" onClick={hide}>
      <div
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-label={t("The nodes of the bus")}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>{t("The nodes of the bus")}</h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "Each node says how long it has been up and how it is. The station asks once when the link opens, and once more for a node that reported how it is without saying what it is; a number the dictionary does not give is shown as the number.",
            )}
          </p>

          <dl className={styles.facts}>
            <dt>{t("How many times this link has asked")}</dt>
            <dd>{formatCount(report?.asked)}</dd>
            <dt>{t("What this reading did")}</dt>
            <dd>
              {report?.sentNow
                ? t("It put the ask on the wire")
                : t("It read the bus and sent nothing")}
            </dd>
          </dl>

          <div className={styles.row}>
            <button
              type="button"
              className="button button--primary"
              disabled={busy || !linked}
              onClick={() => {
                setBusy(true);
                setError(null);
                void read().finally(() => {
                  setBusy(false);
                });
              }}
            >
              {t("Read the bus")}
            </button>
          </div>

          {!linked ? (
            <p className={styles.text}>{t("There is no link to a vehicle.")}</p>
          ) : null}
        </section>

        <section className={styles.block}>
          {nodes.length === 0 ? (
            <p className={styles.text}>{t("The bus has reported nothing.")}</p>
          ) : (
            <ul className={styles.list}>
              {nodes.map((node) => (
                <li
                  key={`${node.vehicle.systemId}:${node.vehicle.componentId}`}
                  className={styles.item}
                >
                  <div className={styles.headline}>
                    <span className={styles.name}>
                      {node.name ?? t("the node has not said what it is")}
                    </span>
                    <span className={styles.pair}>
                      {node.vehicle.systemId}:{node.vehicle.componentId}
                    </span>
                  </div>

                  <dl className={styles.facts}>
                    <dt>{t("How it is")}</dt>
                    <dd>
                      {node.healthName ?? formatCount(node.health)}
                      {node.infoMissing ? (
                        <span className={styles.missing}>
                          {t("(no information behind it)")}
                        </span>
                      ) : null}
                    </dd>
                    <dt>{t("What it is doing")}</dt>
                    <dd>{node.modeName ?? formatCount(node.mode)}</dd>
                    <dt>{t("How long it has been up")}</dt>
                    <dd>{formatCount(node.uptimeSec)}</dd>
                    <dt>{t("What the vendor says")}</dt>
                    <dd>{formatCount(node.vendorSpecificStatusCode)}</dd>
                    <dt>{t("The hardware version")}</dt>
                    <dd>
                      {node.hwVersionMajor === null
                        ? DASH
                        : `${node.hwVersionMajor}.${node.hwVersionMinor}`}
                    </dd>
                    <dt>{t("The software version")}</dt>
                    <dd>
                      {node.swVersionMajor === null
                        ? DASH
                        : `${node.swVersionMajor}.${node.swVersionMinor}`}
                    </dd>
                    <dt>{t("The number that identifies this board")}</dt>
                    <dd>{node.hwUniqueId ?? DASH}</dd>
                    <dt>{t("The commit the build came from")}</dt>
                    <dd>{node.swVcsCommit ?? DASH}</dd>
                  </dl>

                  <div className={styles.row}>
                    <button
                      type="button"
                      className="button"
                      disabled={busyParameters}
                      onClick={() => {
                        setOpenNode({
                          systemId: node.vehicle.systemId,
                          componentId: node.vehicle.componentId,
                          via: "link",
                        });
                        setParameters([]);
                        setDrafts({});
                        void loadParameters(
                          {
                            systemId: node.vehicle.systemId,
                            componentId: node.vehicle.componentId,
                            via: "link",
                          },
                          bus,
                        );
                      }}
                    >
                      {t("Its parameters")}
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "A CAN adapter on a serial port is the other way to the bus: no vehicle has to forward it. Open the port, then read a node by its number.",
            )}
          </p>

          <div className={styles.row}>
            <label className="field">
              <span>{t("The port the adapter is on")}</span>
              <input
                type="text"
                value={port}
                onChange={(event) => {
                  setPort(event.target.value);
                }}
              />
            </label>
            <label className="field">
              <span>{t("The rate of the bus")}</span>
              <select
                className="select"
                value={rate}
                onChange={(event) => {
                  setRate(Number(event.target.value));
                }}
              >
                {[
                  10000, 20000, 50000, 100000, 125000, 250000, 500000, 800000,
                  1000000,
                ].map((value) => (
                  <option key={value} value={value}>
                    {formatCount(value)}
                  </option>
                ))}
              </select>
            </label>
            <button
              type="button"
              className="button"
              disabled={adapterBusy || adapter !== null}
              onClick={() => {
                void openAdapter();
              }}
            >
              {t("Open the adapter")}
            </button>
            <button
              type="button"
              className="button"
              disabled={adapterBusy || adapter === null}
              onClick={() => {
                void closeAdapter();
              }}
            >
              {t("Close the adapter")}
            </button>
          </div>

          {adapter !== null ? (
            <div className={styles.row}>
              <label className="field">
                <span>{t("The node to read")}</span>
                <input
                  type="number"
                  min={1}
                  max={127}
                  value={adapterNode}
                  onChange={(event) => {
                    setAdapterNode(Number(event.target.value) || 1);
                  }}
                />
              </label>
              <button
                type="button"
                className="button"
                disabled={busyParameters}
                onClick={() => {
                  setOpenNode({
                    systemId: 0,
                    componentId: adapterNode,
                    via: "adapter",
                  });
                  setParameters([]);
                  setDrafts({});
                  void loadParameters(
                    { systemId: 0, componentId: adapterNode, via: "adapter" },
                    bus,
                  );
                }}
              >
                {t("Read its parameters")}
              </button>
            </div>
          ) : null}
        </section>
        {openNode !== null ? (
          <section className={styles.block}>
            <div className={styles.headline}>
              <span className={styles.name}>
                {t("The parameters of the node")} {openNode.componentId}
              </span>
              <button
                type="button"
                className="button"
                onClick={() => {
                  setOpenNode(null);
                  setParameters([]);
                }}
              >
                {t("Close the parameters")}
              </button>
            </div>

            <div className={styles.row}>
              <label className="field">
                <span>{t("The bus it is on")}</span>
                <input
                  type="number"
                  min={1}
                  max={3}
                  value={bus}
                  onChange={(event) => {
                    setBus(Number(event.target.value) || 1);
                  }}
                />
              </label>
              <button
                type="button"
                className="button"
                disabled={busyParameters}
                onClick={() => {
                  void loadParameters(openNode, bus);
                }}
              >
                {t("Read them again")}
              </button>
            </div>

            {busyParameters && parameters.length === 0 ? (
              <p className={styles.text}>{t("The node is being asked.")}</p>
            ) : null}

            {parameters.length === 0 && !busyParameters ? (
              <p className={styles.text}>
                {t("The node named no parameters, or it did not answer.")}
              </p>
            ) : null}

            {parameters.map((parameter) => (
              <div key={parameter.name} className={styles.parameter}>
                <div className={styles.headline}>
                  <span className={styles.name}>{parameter.name}</span>
                  <span className={styles.pair}>{parameter.value}</span>
                </div>

                <dl className={styles.facts}>
                  <dt>{t("Before anybody changed it")}</dt>
                  <dd>{parameter.defaultValue || DASH}</dd>
                  <dt>{t("What it accepts")}</dt>
                  <dd>
                    {parameter.minValue || DASH} … {parameter.maxValue || DASH}
                  </dd>
                </dl>

                <div className={styles.row}>
                  <input
                    className="input"
                    aria-label={t("The new value of {name}", {
                      name: parameter.name,
                    })}
                    value={drafts[parameter.name] ?? ""}
                    placeholder={parameter.value}
                    onChange={(event) => {
                      setDrafts((current) => ({
                        ...current,
                        [parameter.name]: event.target.value,
                      }));
                    }}
                  />
                  <button
                    type="button"
                    className="button"
                    disabled={
                      busyParameters ||
                      (drafts[parameter.name] ?? "").trim() === ""
                    }
                    onClick={() => {
                      void writeParameter(parameter.name);
                    }}
                  >
                    {t("Write it")}
                  </button>
                </div>
              </div>
            ))}

            <div className={styles.row}>
              <button
                type="button"
                className="button"
                disabled={busyParameters}
                onClick={() => {
                  void saveParameters();
                }}
              >
                {t("Keep them across a power cycle")}
              </button>
              <button
                type="button"
                className="button"
                disabled={busyParameters}
                onClick={() => {
                  void restartNode();
                }}
              >
                {t("Restart the node")}
              </button>
            </div>

            {note !== null ? <p className={styles.note}>{note}</p> : null}
          </section>
        ) : null}

        {error !== null ? (
          <p className={styles.error} role="alert">
            {error}
          </p>
        ) : null}
      </div>
    </div>
  );
}
