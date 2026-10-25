import { useEffect, useMemo, useState } from "react";

import { ConfirmButton } from "../../components/ConfirmButton";
import { modeValue, useModesStore } from "../../lib/flight/modesStore";
import {
  armVehicle,
  askHome,
  changeMode,
  disarmVehicle,
  flyTo,
  jumpTo,
  makeCurrent,
  moveHome,
  rebootVehicle,
  sendCatalogueCommand,
} from "../../lib/flight/flightActions";
import { useLiveValue } from "../../lib/link/liveTelemetry";
import type { PlanKind } from "../../lib/ipc/types";
import { formatCount } from "../../lib/format";
import { usePlanEditorStore } from "../../lib/plan/planEditorStore";
import { FlightTestPanel } from "./FlightTestPanel";
import { usePlanStore } from "../../lib/plan/planStore";
import { useCommandStore } from "../../lib/plan/commandStore";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useActivityStore } from "../../lib/ui/activityStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./ActionsPanel.module.css";

/**
 * The actions tab: the flight controls.
 *
 * The layout follows upstream: the mode selector and its button, the arming buttons, the
 * quick flight modes, the waypoint selector and the plan buttons.
 *
 * Every action that hurts — arming, disarming, writing a plan, emptying a plan — asks
 * first, and none of them is the button that is pressed by accident.
 *
 * The **command of the catalogue** is the generic executor. The command is chosen from the table the
 * backend carries, which is what says what each of the seven slots means, and what is read after
 * sending is what the vehicle answered rather than what was asked for. Upstream reaches the same
 * executor through its actions dropdown, where the names of the actions are written out by hand; here
 * the choice comes from the catalogue, so a command the catalogue does not describe cannot be picked.
 */

/**
 * The modes of the quick row, in the order upstream has them.
 *
 * The label is the one the operator knows; the name is the one the backend calls it,
 * and the number comes from the backend's own list.
 */
const QUICK_MODES = [
  { label: "RTL", name: "RTL" },
  { label: "Land", name: "LAND" },
  { label: "Loiter", name: "LOITER" },
  { label: "Auto", name: "AUTO" },
  { label: "GUIDED", name: "GUIDED" },
] as const;

/** The kinds of plan the vehicle holds. */
const PLAN_KINDS: readonly { id: PlanKind; label: string }[] = [
  { id: "mission", label: "Mission" },
  { id: "fence", label: "Fence" },
  { id: "rally", label: "Rally" },
];

/** The seven parameter slots a command has, in the order the protocol puts them. */
const SLOTS = [0, 1, 2, 3, 4, 5, 6];

export function ActionsPanel() {
  const modes = useModesStore((state) => state.modes);
  const loadModes = useModesStore((state) => state.load);

  // The catalogue of the commands, read from the backend: the meaning of a slot belongs to the
  // autopilot, so the panel does not write it out again.
  const catalogue = useCommandStore((state) => state.commands);
  const loadCatalogue = useCommandStore((state) => state.load);

  const plans = usePlanStore((state) => state.plans);
  const kind = usePlanStore((state) => state.kind);
  const setKind = usePlanStore((state) => state.setKind);
  const busy = usePlanStore((state) => state.busy);
  const planError = usePlanStore((state) => state.error);
  const download = usePlanStore((state) => state.download);
  const upload = usePlanStore((state) => state.upload);
  const clear = usePlanStore((state) => state.clear);

  // The editor is the screen of the plan: the actions tab opens it by putting the window
  // on that screen and closes it by going back to the flight data.
  const editorOpen = useUiStore((state) => state.screen === "plan");
  const openEditor = usePlanEditorStore((state) => state.openEditor);
  const closeEditor = usePlanEditorStore((state) => state.closeEditor);
  const askUpload = usePlanEditorStore((state) => state.askUpload);
  const askDownload = usePlanEditorStore((state) => state.askDownload);

  const error = useActivityStore((state) => state.error);
  const notice = useActivityStore((state) => state.notice);

  const t = useLocaleStore((state) => state.t);

  const [mode, setModeChoice] = useState<number | "">("");
  const [item, setItem] = useState<number | "">("");

  // The command of the catalogue and its seven slots. The values are kept as they were typed, so that
  // a half-written number is not turned into zero while the operator is still writing it; an empty
  // slot — which is what a slot the command does not use stays — is sent as zero.
  const [commandValue, setChosenCommand] = useState<number | "">("");
  const [values, setValues] = useState<string[]>(() =>
    Array(SLOTS.length).fill(""),
  );

  const plan = plans[kind];

  // Where the vehicle is: the home can be moved to the place it is in.
  const position = useLiveValue((telemetry) => telemetry.position ?? null);

  useEffect(() => {
    if (modes.length === 0) {
      void loadModes();
    }
  }, [modes.length, loadModes]);

  // The catalogue is read once and kept, the way the map and the plan editor read it.
  useEffect(() => {
    if (catalogue.length === 0) {
      void loadCatalogue();
    }
  }, [catalogue.length, loadCatalogue]);

  // The first mode of the list is the one the selector starts on.
  useEffect(() => {
    if (mode === "" && modes.length > 0) {
      setModeChoice(modes[0]?.value ?? "");
    }
  }, [modes, mode]);

  const items = useMemo(() => plan?.items ?? [], [plan]);

  const selected = modes.find((entry) => entry.value === mode);

  // The place of the plan that is chosen, when there is one: it is where the
  // vehicle is told to go when the operator asks it to.
  const chosen = items.find((entry) => entry.seq === item);

  // The command of the catalogue that is chosen, when there is one. The role and the unit of every
  // slot are what the catalogue says, so the panel draws them as data and decides nothing about what
  // a slot means — which is why a slot the command does not use cannot be filled in at all.
  const picked = catalogue.find((entry) => entry.value === commandValue);

  // What goes on the wire: seven numbers, with an empty slot — the one the command does not use —
  // sent as zero.
  const slots = SLOTS.map((slot) => {
    const typed = values[slot] ?? "";
    const number = Number.parseFloat(typed);
    return Number.isFinite(number) ? number : 0;
  });

  const shown = error ?? planError;

  return (
    <div className={styles.panel}>
      <section className={styles.block}>
        <div className={styles.row}>
          <label className={styles.field}>
            <span className={styles.label}>{t("Flight mode")}</span>
            <select
              className="select"
              value={mode}
              onChange={(event) =>
                setModeChoice(Number.parseInt(event.target.value, 10))
              }
            >
              {modes.length === 0 ? (
                <option value="">{t("no modes")}</option>
              ) : null}
              {modes.map((entry) => (
                <option key={entry.value} value={entry.value}>
                  {entry.name}
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            className="button button--primary"
            disabled={mode === ""}
            onClick={() => {
              if (selected) {
                void changeMode(selected.value, selected.name);
              }
            }}
          >
            {t("Set Mode")}
          </button>
        </div>

        <div className={styles.row}>
          <ConfirmButton
            label={t("Arm")}
            confirmLabel={t("Confirm arm")}
            variant="primary"
            onConfirm={() => void armVehicle()}
          />
          <ConfirmButton
            label={t("Disarm")}
            confirmLabel={t("Confirm disarm")}
            variant="danger"
            onConfirm={() => void disarmVehicle()}
          />
        </div>

        <div className={styles.quick}>
          {QUICK_MODES.map((entry) => {
            const value = modeValue(modes, entry.name);
            return (
              <button
                key={entry.name}
                type="button"
                className="button"
                disabled={value === null}
                onClick={() => {
                  if (value !== null) {
                    void changeMode(value, entry.label);
                  }
                }}
              >
                {entry.label}
              </button>
            );
          })}
        </div>
      </section>

      <section className={styles.block}>
        <div className={styles.kinds}>
          {PLAN_KINDS.map((entry) => (
            <button
              key={entry.id}
              type="button"
              className={entry.id === kind ? styles.kindActive : styles.kind}
              aria-pressed={entry.id === kind}
              onClick={() => {
                setKind(entry.id);
                setItem("");
              }}
            >
              {t(entry.label)}
            </button>
          ))}
        </div>

        <label className={styles.field}>
          <span className={styles.label}>{t("Waypoint")}</span>
          <select
            className="select"
            value={item}
            disabled={items.length === 0}
            onChange={(event) =>
              setItem(Number.parseInt(event.target.value, 10))
            }
          >
            {items.length === 0 ? (
              <option value="">{t("no plan")}</option>
            ) : null}
            {items.map((entry) => (
              <option key={entry.seq} value={entry.seq}>
                {entry.seq + 1} · {entry.command}
                {entry.current ? ` · ${t("current")}` : ""}
              </option>
            ))}
          </select>
        </label>

        <div className={styles.row}>
          <button
            type="button"
            className="button"
            disabled={item === ""}
            onClick={() => {
              if (item !== "") {
                void makeCurrent(item);
              }
            }}
          >
            {t("Set Current")}
          </button>
          <button
            type="button"
            className="button"
            disabled={item === ""}
            onClick={() => {
              if (item !== "") {
                void jumpTo(item);
              }
            }}
          >
            {t("Jump")}
          </button>
          <button
            type="button"
            className="button"
            disabled={chosen === undefined}
            title={t("Tell the vehicle to fly to this waypoint")}
            onClick={() => {
              if (chosen !== undefined) {
                void flyTo(chosen.x, chosen.y, chosen.z);
              }
            }}
          >
            {t("Fly to")}
          </button>
          <ConfirmButton
            label={t("Clear plan")}
            confirmLabel={t("Confirm clear")}
            variant="danger"
            disabled={busy}
            onConfirm={() => {
              void clear(kind);
            }}
          />
        </div>

        <div className={styles.row}>
          <button
            type="button"
            className="button"
            disabled={busy}
            onClick={() => {
              // While the plan is being edited, the editor asks before it throws the
              // edits away — the actions tab does not know about them.
              if (editorOpen) {
                askDownload();
                return;
              }
              void download(kind);
            }}
          >
            {t("Download plan")}
          </button>
          {editorOpen ? (
            <button
              type="button"
              className="button button--primary"
              disabled={busy || items.length === 0}
              onClick={() => {
                void askUpload();
              }}
            >
              {t("Upload plan")}
            </button>
          ) : (
            <ConfirmButton
              label={t("Upload plan")}
              confirmLabel={t("Confirm upload")}
              variant="primary"
              disabled={busy || items.length === 0}
              onConfirm={() => {
                void upload(kind, items);
              }}
            />
          )}
          <button
            type="button"
            className={`button ${editorOpen ? styles.editing : ""}`}
            aria-pressed={editorOpen}
            onClick={() => (editorOpen ? closeEditor() : openEditor())}
            title={t("Edit the plan in a grid above the map")}
          >
            {editorOpen ? t("Close editor") : t("Edit plan")}
          </button>
        </div>

        {editorOpen ? (
          <p className={styles.editingNote}>
            {t(
              "The plan above the map is being edited. Uploading sends the edited plan; setting the current item and jumping act on the plan the vehicle holds.",
            )}
          </p>
        ) : null}
      </section>

      <section className={styles.block}>
        <div className={styles.row}>
          <ConfirmButton
            label={t("Reboot")}
            confirmLabel={t("Confirm reboot")}
            variant="danger"
            onConfirm={() => {
              void rebootVehicle();
            }}
          />
        </div>
      </section>

      <section className={styles.block}>
        <h3 className={styles.heading}>{t("Home")}</h3>
        <div className={styles.row}>
          <ConfirmButton
            label={t("Set the home here")}
            confirmLabel={t("Confirm the home")}
            onConfirm={() => {
              if (position !== null) {
                void moveHome(position.lat, position.lng, position.alt);
              }
            }}
          />
          <button
            type="button"
            className="button"
            title={t("Ask the vehicle where its home is")}
            onClick={() => {
              void askHome();
            }}
          >
            {t("Ask the vehicle")}
          </button>
        </div>
        <p className={styles.hint}>
          {t(
            "The home is the place the vehicle measures the distance from. Moving it is what an operator does when the vehicle was armed in the wrong place.",
          )}
        </p>
      </section>

      <section className={styles.block}>
        <h3 className={styles.heading}>{t("The command of the catalogue")}</h3>

        <label className={styles.field}>
          <span className={styles.label}>{t("Command")}</span>
          <select
            className="select"
            value={commandValue}
            aria-label={t("The command to send")}
            onChange={(event) =>
              setChosenCommand(Number.parseInt(event.target.value, 10))
            }
          >
            {catalogue.length === 0 ? (
              <option value="">{t("no commands")}</option>
            ) : null}
            {catalogue.map((entry) => (
              <option key={entry.value} value={entry.value}>
                {entry.name}
              </option>
            ))}
          </select>
        </label>

        {/* The role and the unit are what the catalogue says, so they are drawn as data: the panel
           does not decide that a slot is a bank number or a percentage. */}
        <div className={styles.slots}>
          {SLOTS.map((slot) => {
            const role = picked?.roles[slot] ?? "empty";
            const unit = picked?.units[slot] ?? "none";
            const used = role !== "empty";
            const number = formatCount(slot + 1);

            return (
              <label key={slot} className={styles.slot}>
                <span className={styles.label}>
                  {t("Slot {number}", { number })}
                </span>
                <span className={styles.role}>
                  {unit === "none" ? role : `${role} · ${unit}`}
                </span>
                <input
                  className="input"
                  type="number"
                  value={values[slot] ?? ""}
                  disabled={!used}
                  aria-label={t("The value for slot {number}", { number })}
                  onChange={(event) => {
                    const typed = event.target.value;
                    setValues((current) =>
                      current.map((entry, index) =>
                        index === slot ? typed : entry,
                      ),
                    );
                  }}
                />
              </label>
            );
          })}
        </div>

        <div className={styles.row}>
          <ConfirmButton
            label={t("Send it")}
            confirmLabel={t("Confirm sending")}
            variant="primary"
            disabled={picked === undefined}
            onConfirm={() => {
              if (picked !== undefined) {
                void sendCatalogueCommand(picked, slots);
              }
            }}
          />
        </div>

        <p className={styles.hint}>
          {t(
            "The command is chosen from the catalogue, which is what says what each slot means. A slot the command does not use cannot be filled in, and is sent as zero. What is read below is what the vehicle answered, as the vehicle said it.",
          )}
        </p>
      </section>

      <FlightTestPanel />

      {shown ? (
        <p className={styles.error} role="alert">
          <span className={styles.errorCode}>{shown.code}</span> {shown.message}
        </p>
      ) : null}

      {!shown && notice ? (
        <p className={styles.notice} role="status">
          {notice}
        </p>
      ) : null}
    </div>
  );
}
