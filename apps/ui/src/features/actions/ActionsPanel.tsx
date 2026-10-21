import { useEffect, useMemo, useState } from "react";

import { ConfirmButton } from "../../components/ConfirmButton";
import { modeValue, useModesStore } from "../../lib/flight/modesStore";
import {
  armVehicle,
  changeMode,
  disarmVehicle,
  jumpTo,
  makeCurrent,
} from "../../lib/flight/flightActions";
import type { PlanKind } from "../../lib/ipc/types";
import { usePlanEditorStore } from "../../lib/plan/planEditorStore";
import { usePlanStore } from "../../lib/plan/planStore";
import { useActivityStore } from "../../lib/ui/activityStore";
import styles from "./ActionsPanel.module.css";

/**
 * The actions tab: the flight controls.
 *
 * The layout follows upstream: the mode selector and its button, the arming buttons, the
 * quick flight modes, the waypoint selector and the plan buttons.
 *
 * Every action that hurts — arming, disarming, writing a plan, emptying a plan — asks
 * first, and none of them is the button that is pressed by accident.
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

export function ActionsPanel() {
  const modes = useModesStore((state) => state.modes);
  const loadModes = useModesStore((state) => state.load);

  const plans = usePlanStore((state) => state.plans);
  const kind = usePlanStore((state) => state.kind);
  const setKind = usePlanStore((state) => state.setKind);
  const busy = usePlanStore((state) => state.busy);
  const planError = usePlanStore((state) => state.error);
  const download = usePlanStore((state) => state.download);
  const upload = usePlanStore((state) => state.upload);
  const clear = usePlanStore((state) => state.clear);

  const editorOpen = usePlanEditorStore((state) => state.open);
  const toggleEditor = usePlanEditorStore((state) => state.toggleEditor);
  const askUpload = usePlanEditorStore((state) => state.askUpload);
  const askDownload = usePlanEditorStore((state) => state.askDownload);

  const error = useActivityStore((state) => state.error);
  const notice = useActivityStore((state) => state.notice);

  const [mode, setModeChoice] = useState<number | "">("");
  const [item, setItem] = useState<number | "">("");

  const plan = plans[kind];

  useEffect(() => {
    if (modes.length === 0) {
      void loadModes();
    }
  }, [modes.length, loadModes]);

  // The first mode of the list is the one the selector starts on.
  useEffect(() => {
    if (mode === "" && modes.length > 0) {
      setModeChoice(modes[0]?.value ?? "");
    }
  }, [modes, mode]);

  const items = useMemo(() => plan?.items ?? [], [plan]);

  const selected = modes.find((entry) => entry.value === mode);

  const shown = error ?? planError;

  return (
    <div className={styles.panel}>
      <section className={styles.block}>
        <div className={styles.row}>
          <label className={styles.field}>
            <span className={styles.label}>Flight mode</span>
            <select
              className="select"
              value={mode}
              onChange={(event) =>
                setModeChoice(Number.parseInt(event.target.value, 10))
              }
            >
              {modes.length === 0 ? <option value="">no modes</option> : null}
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
            Set Mode
          </button>
        </div>

        <div className={styles.row}>
          <ConfirmButton
            label="Arm"
            confirmLabel="Confirm arm"
            variant="primary"
            onConfirm={() => void armVehicle()}
          />
          <ConfirmButton
            label="Disarm"
            confirmLabel="Confirm disarm"
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
              {entry.label}
            </button>
          ))}
        </div>

        <label className={styles.field}>
          <span className={styles.label}>Waypoint</span>
          <select
            className="select"
            value={item}
            disabled={items.length === 0}
            onChange={(event) =>
              setItem(Number.parseInt(event.target.value, 10))
            }
          >
            {items.length === 0 ? <option value="">no plan</option> : null}
            {items.map((entry) => (
              <option key={entry.seq} value={entry.seq}>
                {entry.seq + 1} · {entry.command}
                {entry.current ? " · current" : ""}
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
            Set Current
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
            Jump
          </button>
          <ConfirmButton
            label="Clear plan"
            confirmLabel="Confirm clear"
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
            Download plan
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
              Upload plan
            </button>
          ) : (
            <ConfirmButton
              label="Upload plan"
              confirmLabel="Confirm upload"
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
            onClick={toggleEditor}
            title="Edit the plan in a grid above the map"
          >
            {editorOpen ? "Close editor" : "Edit plan"}
          </button>
        </div>

        {editorOpen ? (
          <p className={styles.editingNote}>
            The plan above the map is being edited. Uploading sends the edited plan;
            setting the current item and jumping act on the plan the vehicle holds.
          </p>
        ) : null}
      </section>

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
