import { useCallback, useEffect, useState } from "react";

import { dronecanScan, normaliseError } from "../../lib/ipc/bridge";
import type { DronecanNode, DronecanReport } from "../../lib/ipc/types";
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
                </li>
              ))}
            </ul>
          )}
        </section>

        {error !== null ? (
          <p className={styles.error} role="alert">
            {error}
          </p>
        ) : null}
      </div>
    </div>
  );
}
