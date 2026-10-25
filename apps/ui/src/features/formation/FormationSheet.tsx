import { useCallback, useEffect, useState } from "react";

import {
  formationStatus,
  normaliseError,
  setFormation,
} from "../../lib/ipc/bridge";
import type { FormationRequest } from "../../lib/ipc/types";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useLinkStore } from "../../lib/link/linkStore";
import { useUiStore } from "../../lib/ui/uiStore";
import {
  useFormationStore,
  vehicleName,
} from "../../lib/formation/formationStore";
import styles from "./FormationSheet.module.css";

/**
 * The sheet of the formation.
 *
 * Upstream has the same thing (`Swarm/FormationControl.cs`): the vehicles that are on the link,
 * the place each one is given in the formation, and the buttons that start and stop the loop. What
 * this sheet does not do is arm, take off or land a fleet — those are the actions the actions tab
 * already has, and being in a formation is not a reason to do them.
 *
 * The two things the sheet keeps apart are the two things the file keeps apart: the places an
 * operator wrote, and the decision to run them. Saving a fleet does not send orders to it; Start
 * does, and Stop stops them. A name that cannot be read as a vehicle is saved as it was written
 * and said, so that the operator can see which entry they have to fix.
 */

/** The axes of a slot, in the order the table shows them. */
const AXES = ["x", "y", "z"] as const;

type Axis = (typeof AXES)[number];

export function FormationSheet() {
  const open = useUiStore((state) => state.formationOpen);
  const hide = useUiStore((state) => state.hideFormation);
  const t = useLocaleStore((state) => state.t);
  const tag = useLocaleStore((state) => state.tag);

  const status = useLinkStore((state) => state.status);
  const vehicles = useLinkStore((state) => state.vehicles);
  const linked = status === "connected" || status === "reconnecting";

  const leader = useFormationStore((state) => state.leader);
  const enabled = useFormationStore((state) => state.enabled);
  const slots = useFormationStore((state) => state.slots);
  const running = useFormationStore((state) => state.running);
  const told = useFormationStore((state) => state.told);
  const refused = useFormationStore((state) => state.refused);
  const error = useFormationStore((state) => state.error);

  const [chosen, setChosen] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const read = useCallback(async () => {
    try {
      const state = await formationStatus();
      useFormationStore.getState().apply(state);
    } catch {
      // The state is only read to be shown.
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

  if (!open) {
    return null;
  }

  /** What an axis means, said as the word the table shows. */
  const axisName = (axis: Axis) => {
    if (axis === "x") {
      return t("Across");
    }

    if (axis === "y") {
      return t("Ahead");
    }

    return t("Above");
  };

  /** Every name the sheet knows: what the link has seen, and what the file kept. */
  const names = [
    ...new Set([
      ...vehicles.map((vehicle) =>
        vehicleName(vehicle.systemId, vehicle.componentId),
      ),
      leader,
      ...slots.map((slot) => slot.vehicle),
    ]),
  ];

  const request = (decision: boolean): FormationRequest => ({
    leader: leader === "" ? null : leader,
    enabled: decision,
    slots,
  });

  const send = async (req: FormationRequest, message: string) => {
    setBusy(true);
    setNotice(null);

    try {
      const state = await setFormation(req);
      useFormationStore.getState().apply(state);
      setNotice(message);
    } catch (cause) {
      setNotice(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={styles.backdrop} role="presentation" onClick={hide}>
      <div
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-label={t("The formation")}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>{t("The formation")}</h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "One vehicle leads and the others sit in the places that were written for them. The loop runs ten times a second and tells each member where its place is.",
            )}
          </p>

          <label className={styles.field}>
            <span className={styles.label}>{t("Who leads")}</span>
            <select
              className="input"
              value={leader}
              aria-label={t("The vehicle the formation follows")}
              onChange={(event) => {
                useFormationStore.getState().setLeader(event.target.value);
              }}
            >
              <option value="">{t("nobody yet")}</option>
              {names.map((name) => (
                <option value={name} key={name}>
                  {name}
                </option>
              ))}
            </select>
          </label>

          <div className={styles.row}>
            <label className={styles.field}>
              <span className={styles.label}>
                {t("Put a vehicle in the formation")}
              </span>
              <select
                className="input"
                value={chosen}
                aria-label={t("The vehicle to put in the formation")}
                onChange={(event) => {
                  setChosen(event.target.value);
                }}
              >
                <option value="">{t("choose one")}</option>
                {names.map((name) => (
                  <option value={name} key={name}>
                    {name}
                  </option>
                ))}
              </select>
            </label>

            <button
              type="button"
              className="button"
              disabled={
                busy ||
                chosen === "" ||
                slots.some((slot) => slot.vehicle === chosen)
              }
              onClick={() => {
                if (chosen !== "") {
                  useFormationStore.getState().addSlot(chosen);
                }
              }}
            >
              {t("Add")}
            </button>
          </div>
        </section>

        <section className={styles.block}>
          <table className={styles.table}>
            <thead>
              <tr>
                <th>{t("The vehicle")}</th>
                {AXES.map((axis) => (
                  <th key={axis}>{axisName(axis)}</th>
                ))}
                <th>{t("Take out")}</th>
              </tr>
            </thead>
            <tbody>
              {slots.length === 0 ? (
                <tr>
                  <td colSpan={5}>{t("Nothing has been written yet.")}</td>
                </tr>
              ) : (
                slots.map((slot) => (
                  <tr key={slot.vehicle}>
                    <td>{slot.vehicle}</td>
                    {AXES.map((axis) => (
                      <td key={axis}>
                        <input
                          className="input"
                          type="number"
                          step={0.1}
                          value={slot[axis]}
                          aria-label={`${slot.vehicle} ${axisName(axis)}`}
                          onChange={(event) => {
                            const metres = Number(event.target.value);

                            if (Number.isFinite(metres)) {
                              useFormationStore
                                .getState()
                                .setOffset(slot.vehicle, axis, metres);
                            }
                          }}
                        />
                      </td>
                    ))}
                    <td>
                      <button
                        type="button"
                        className="button"
                        disabled={busy}
                        onClick={() => {
                          useFormationStore.getState().removeSlot(slot.vehicle);
                        }}
                      >
                        {t("Take out")}
                      </button>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>

          <p className={styles.text}>
            {t("The numbers are metres from the leader.")}
          </p>
        </section>

        <section className={styles.block}>
          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={busy}
              onClick={() => {
                void send(request(enabled), t("The places were saved."));
              }}
            >
              {t("Save")}
            </button>

            <button
              type="button"
              className="button button--primary"
              disabled={busy || running || leader === "" || slots.length === 0}
              onClick={() => {
                void send(request(true), t("The loop is being run."));
              }}
            >
              {t("Start")}
            </button>

            <button
              type="button"
              className="button"
              disabled={busy || !running}
              onClick={() => {
                void send(request(false), t("The loop has been stopped."));
              }}
            >
              {t("Stop")}
            </button>
          </div>

          {!linked ? (
            <p className={styles.text}>
              {t(
                "There is no link to a vehicle. The places can be written, but nothing is sent until a board answers.",
              )}
            </p>
          ) : null}
        </section>

        <section className={styles.block}>
          <dl className={styles.facts}>
            <dt>{t("What it is doing")}</dt>
            <dd>
              {running ? t("Telling each member its place") : t("Not running")}
            </dd>
            <dt>{t("How many were told on the last tick")}</dt>
            <dd>{new Intl.NumberFormat(tag).format(told)}</dd>
            <dt>{t("The names that could not be read as vehicles")}</dt>
            <dd>{refused.length === 0 ? t("none") : refused.join(", ")}</dd>
            {error !== null ? (
              <>
                <dt>{t("The last failure")}</dt>
                <dd>{error}</dd>
              </>
            ) : null}
          </dl>
        </section>

        {notice !== null ? (
          <p className={styles.notice} role="status">
            {notice}
          </p>
        ) : null}
      </div>
    </div>
  );
}
