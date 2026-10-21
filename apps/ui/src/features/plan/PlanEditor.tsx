import { useEffect, useMemo, useState } from "react";

import { useCommandStore } from "../../lib/plan/commandStore";
import { pickOpenPath, pickSavePath } from "../../lib/ipc/dialog";
import type { PlanWarning } from "../../lib/ipc/types";
import {
  fileNameOf,
  formatOfPath,
  labelOfFormat,
} from "../../lib/plan/planFiles";
import { isChanged, usePlanEditorStore } from "../../lib/plan/planEditorStore";
import { usePlanStore } from "../../lib/plan/planStore";
import { PlanGrid } from "./PlanGrid";
import styles from "./PlanEditor.module.css";

/**
 * The plan editor.
 *
 * Mission Planner edits a plan in a window of its own: the grid of the items above and
 * the map below, and the waypoints are dragged on the map. MPCraft is one window, so the
 * editor takes the place of the graphs — the area above the map — and the map stays
 * where it is, because the map is where the plan is edited.
 *
 * The plan that is being edited is not the plan the vehicle holds: the edits are not on
 * the vehicle until they are uploaded, and the editor says so for as long as that is
 * true.
 */

/** What the file bar is doing. */
type FileMode = "save" | "load" | null;

/** The kinds of file the dialog offers. */
const FILE_FILTERS = [
  { name: "QGC waypoints", extensions: ["waypoints", "wpl", "qgc"] },
  { name: "KML", extensions: ["kml"] },
] as const;

/**
 * What a warning says.
 *
 * The reason is a code the backend gives, not a sentence: the interface says what it
 * means, and it does not look for words in a message.
 */
function describeWarning(reason: string): string {
  switch (reason) {
    case "zeroAltitude":
      return "the altitude is zero — the vehicle would fly down to the ground.";
    default:
      return reason;
  }
}

/** The name of the kind of plan the editor is working on. */
const KIND_LABELS: Record<string, string> = {
  mission: "Mission plan",
  fence: "Fence",
  rally: "Rally points",
};

export function PlanEditor() {
  const draft = usePlanEditorStore((state) => state.draft);
  const baseline = usePlanEditorStore((state) => state.baseline);
  const selected = usePlanEditorStore((state) => state.selected);
  const adding = usePlanEditorStore((state) => state.adding);
  const marked = usePlanEditorStore((state) => state.marked);
  const places = usePlanEditorStore((state) => state.places);
  const busy = usePlanEditorStore((state) => state.busy);
  const error = usePlanEditorStore((state) => state.error);
  const notice = usePlanEditorStore((state) => state.notice);
  const pending = usePlanEditorStore((state) => state.pending);
  const history = usePlanEditorStore((state) => state.history);
  const future = usePlanEditorStore((state) => state.future);

  const change = usePlanEditorStore((state) => state.change);
  const move = usePlanEditorStore((state) => state.move);
  const remove = usePlanEditorStore((state) => state.remove);
  const select = usePlanEditorStore((state) => state.select);
  const setAdding = usePlanEditorStore((state) => state.setAdding);
  const askUpload = usePlanEditorStore((state) => state.askUpload);
  const askDownload = usePlanEditorStore((state) => state.askDownload);
  const askImport = usePlanEditorStore((state) => state.askImport);
  const confirm = usePlanEditorStore((state) => state.confirm);
  const cancel = usePlanEditorStore((state) => state.cancel);
  const saveToFile = usePlanEditorStore((state) => state.saveToFile);
  const closeEditor = usePlanEditorStore((state) => state.closeEditor);
  const dismissError = usePlanEditorStore((state) => state.dismissError);
  const undo = usePlanEditorStore((state) => state.undo);
  const redo = usePlanEditorStore((state) => state.redo);

  const kind = usePlanStore((state) => state.kind);

  // The keyboard, the way upstream has it: control and Z takes the last change back.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!event.ctrlKey || event.key.toLowerCase() !== "z") {
        return;
      }

      event.preventDefault();

      if (event.shiftKey) {
        redo();
      } else {
        undo();
      }
    };

    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [undo, redo]);
  const commands = useCommandStore((state) => state.commands);
  const loadCommands = useCommandStore((state) => state.load);

  const [fileMode, setFileMode] = useState<FileMode>(null);
  const [path, setPath] = useState("");
  const [dialogNote, setDialogNote] = useState<string | null>(null);

  const changed = isChanged({ draft, baseline });
  const format = useMemo(() => formatOfPath(path), [path]);

  // The catalogue of the commands is read once: the map needs it as well, to know
  // which items of a plan have a place at all.
  useEffect(() => {
    void loadCommands();
  }, [loadCommands]);

  // A fence is a polygon the vehicle will not leave, not a list of waypoints to fly:
  // there is nothing to add to it, though its points can be moved and removed.
  const canAdd = kind !== "fence";

  // The escape key stops the adding of an item. It does not close the editor: a plan
  // with unsent edits is not something to lose to a stray key.
  useEffect(() => {
    if (!adding) {
      return;
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setAdding(false);
      }
    };

    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [adding, setAdding]);

  const openFile = (mode: FileMode) => {
    setFileMode(mode);
    setDialogNote(null);
    setPath(mode === "save" ? fileNameOf("waypoints") : "");
  };

  const browse = async () => {
    const picked =
      fileMode === "save"
        ? await pickSavePath(path || fileNameOf("waypoints"), FILE_FILTERS)
        : await pickOpenPath(FILE_FILTERS);

    if (picked.ok) {
      setPath(picked.path);
      setDialogNote(null);
      return;
    }

    setDialogNote(
      picked.reason === "cancelled"
        ? "No file was chosen."
        : "This build has no file dialog — type the name of the file.",
    );
  };

  const submitFile = () => {
    if (fileMode === null || format === null) {
      return;
    }

    if (fileMode === "save") {
      void saveToFile(path, format);
    } else {
      void askImport(path, format);
    }

    setFileMode(null);
  };

  const warnings: readonly PlanWarning[] =
    pending?.kind === "upload" ? pending.warnings : [];

  return (
    <section className={styles.editor} aria-label="The plan editor">
      <div className={styles.toolbar}>
        <button
          type="button"
          className="button"
          aria-label="Take the last change back"
          disabled={busy || history.length === 0}
          title="Take the last change back (Ctrl+Z)"
          onClick={undo}
        >
          Undo
        </button>

        <button
          type="button"
          className="button"
          aria-label="Do again the change that was taken back"
          disabled={busy || future.length === 0}
          title="Do again the change that was taken back (Ctrl+Shift+Z)"
          onClick={redo}
        >
          Redo
        </button>

        <button
          type="button"
          className={`button ${adding ? styles.adding : ""}`}
          aria-label="Add a waypoint on the map"
          aria-pressed={adding}
          disabled={!canAdd || busy}
          title={
            canAdd
              ? "Click the map to add a waypoint"
              : "A fence is a polygon, not a list of waypoints"
          }
          onClick={() => {
            setAdding(!adding);
          }}
        >
          {adding ? "Click the map to add a waypoint" : "Add waypoint"}
        </button>

        <button
          type="button"
          className="button"
          disabled={busy || draft.length === 0}
          onClick={() => {
            void askUpload();
          }}
        >
          Upload plan
        </button>

        <button
          type="button"
          className="button"
          disabled={busy}
          onClick={() => {
            void askDownload();
          }}
        >
          Download plan
        </button>

        <button
          type="button"
          className="button"
          disabled={busy || draft.length === 0}
          onClick={() => {
            openFile("save");
          }}
        >
          Save to a file
        </button>

        <button
          type="button"
          className="button"
          disabled={busy}
          onClick={() => {
            openFile("load");
          }}
        >
          Load from a file
        </button>

        <button
          type="button"
          className={`button ${styles.close}`}
          onClick={closeEditor}
          title="Put the graphs back and close the editor"
        >
          Close
        </button>
      </div>

      {fileMode !== null ? (
        <div className={styles.fileBar}>
          <label className={styles.fileField}>
            <span className={styles.fileLabel}>
              {fileMode === "save" ? "Write the plan to" : "Read the plan from"}
            </span>
            <input
              className="input"
              type="text"
              spellCheck={false}
              autoComplete="off"
              value={path}
              placeholder={fileNameOf("waypoints")}
              aria-label="The name of the file"
              onChange={(event) => {
                setPath(event.target.value);
                setDialogNote(null);
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  submitFile();
                }
              }}
            />
          </label>

          <span className={styles.format}>
            {format === null
              ? "The name must end in .waypoints, .wpl or .kml"
              : labelOfFormat(format)}
          </span>

          <button type="button" className="button" onClick={() => void browse()}>
            Browse…
          </button>

          <button
            type="button"
            className="button button--primary"
            disabled={format === null || path.trim().length === 0}
            onClick={submitFile}
          >
            {fileMode === "save" ? "Save" : "Load"}
          </button>

          <button
            type="button"
            className="button"
            onClick={() => {
              setFileMode(null);
              setDialogNote(null);
            }}
          >
            Cancel
          </button>
        </div>
      ) : null}

      {pending !== null ? (
        <div className={styles.confirm} role="alertdialog" aria-label="Confirm">
          <div className={styles.confirmText}>
            {pending.kind === "upload" ? (
              warnings.length > 0 ? (
                <>
                  <strong>
                    The plan has {warnings.length} warning
                    {warnings.length === 1 ? "" : "s"}.
                  </strong>
                  <ul className={styles.warnings}>
                    {warnings.map((warning) => (
                      <li key={`${warning.seq}-${warning.reason}`}>
                        Item {warning.seq + 1}: {describeWarning(warning.reason)}
                      </li>
                    ))}
                  </ul>
                </>
              ) : (
                <span>
                  Write {draft.length} item{draft.length === 1 ? "" : "s"} to the
                  vehicle?
                </span>
              )
            ) : pending.kind === "download" ? (
              <span>
                Read the plan from the vehicle again
                {changed
                  ? " — the edits that have not been sent will be lost."
                  : "?"}
              </span>
            ) : (
              <span>
                Read the plan from {pending.path}
                {changed
                  ? " — the edits that have not been sent will be lost."
                  : "?"}
              </span>
            )}
          </div>

          <div className={styles.confirmActions}>
            <button type="button" className="button" onClick={cancel}>
              Cancel
            </button>
            <button
              type="button"
              className="button button--primary"
              onClick={() => {
                void confirm();
              }}
            >
              {pending.kind === "upload" ? "Upload" : "Go on"}
            </button>
          </div>
        </div>
      ) : null}

      {error !== null ? (
        <p className={styles.error} role="alert">
          <span className={styles.code}>{error.code}</span> {error.message}
          <button
            type="button"
            className={styles.dismiss}
            onClick={dismissError}
            aria-label="Put the failure away"
          >
            ×
          </button>
        </p>
      ) : null}

      <PlanGrid
        items={draft}
        commands={commands}
        selected={selected}
        onSelect={select}
        onChange={change}
        onMove={move}
        onRemove={remove}
      />

      <div className={styles.status}>
        <span className={styles.kind}>{KIND_LABELS[kind] ?? kind}</span>
        <span className={styles.count}>
          {draft.length} item{draft.length === 1 ? "" : "s"}
        </span>

        {changed ? (
          <span className={styles.changed} title="The edits are not on the vehicle yet">
            unsent changes
          </span>
        ) : (
          <span className={styles.synced}>the same as the vehicle</span>
        )}

        {adding ? (
          <span className={styles.hint}>
            Click the map to add a waypoint — Escape stops it.
          </span>
        ) : null}

        {marked < places ? (
          <span className={styles.hint}>
            The map marks {marked} of {places} items — zoom in for the rest.
          </span>
        ) : null}

        {dialogNote !== null ? (
          <span className={styles.hint}>{dialogNote}</span>
        ) : null}

        {notice !== null ? (
          <span className={styles.notice} role="status">
            {notice}
          </span>
        ) : null}
      </div>
    </section>
  );
}
