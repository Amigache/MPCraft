import { useEffect, useMemo, useState } from "react";

import { OverflowMenu, type OverflowItem } from "../../app/layout/OverflowMenu";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useCommandStore } from "../../lib/plan/commandStore";
import { pickOpenPath, pickSavePath } from "../../lib/ipc/dialog";
import type { PlanKind, PlanWarning } from "../../lib/ipc/types";
import {
  fileNameOf,
  formatOfPath,
  isWritable,
  labelOfFormat,
} from "../../lib/plan/planFiles";
import {
  addKindFor,
  isChanged,
  usePlanEditorStore,
} from "../../lib/plan/planEditorStore";
import { usePlanStore } from "../../lib/plan/planStore";
import { useUiStore } from "../../lib/ui/uiStore";
import { PlanGrid } from "./PlanGrid";
import { TerrainProfile } from "./TerrainProfile";
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

/** How a part of the interface says something in the language the window is read in. */
type Say = (source: string, values?: Record<string, string | number>) => string;

/** The kinds of file a plan is written to. */
function saveFilters(t: Say) {
  return [
    { name: t("QGC waypoints"), extensions: ["waypoints", "wpl", "qgc"] },
    { name: t("KML"), extensions: ["kml"] },
  ];
}

/**
 * The kinds of file a plan is read from.
 *
 * A drawing is on this list and not on the other one: it carries places and no commands,
 * so it is a way in, not a way out.
 */
function loadFilters(t: Say) {
  return [
    { name: t("QGC waypoints"), extensions: ["waypoints", "wpl", "qgc"] },
    { name: t("KML"), extensions: ["kml"] },
    { name: t("ESRI shapefile"), extensions: ["shp"] },
    { name: t("DXF drawing"), extensions: ["dxf"] },
  ];
}

/**
 * What a warning says.
 *
 * The reason is a code the backend gives, not a sentence: the interface says what it
 * means, and it does not look for words in a message.
 */
function describeWarning(reason: string, t: Say): string {
  switch (reason) {
    case "zeroAltitude":
      return t(
        "the altitude is zero — the vehicle would fly down to the ground.",
      );
    case "tooFewFenceVertices":
      return t(
        "the fence has fewer than three vertices — a polygon needs three.",
      );
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
  const kind = usePlanStore((state) => state.kind);
  const setKind = usePlanStore((state) => state.setKind);
  const adding = usePlanEditorStore((state) => state.adding);
  const addKind = usePlanEditorStore((state) => state.addKind);
  const setAddKind = usePlanEditorStore((state) => state.setAddKind);
  const marked = usePlanEditorStore((state) => state.marked);
  const places = usePlanEditorStore((state) => state.places);
  const busy = usePlanEditorStore((state) => state.busy);
  const error = usePlanEditorStore((state) => state.error);
  const notice = usePlanEditorStore((state) => state.notice);
  const pending = usePlanEditorStore((state) => state.pending);
  const history = usePlanEditorStore((state) => state.history);
  const future = usePlanEditorStore((state) => state.future);

  const t = useLocaleStore((state) => state.t);

  const change = usePlanEditorStore((state) => state.change);
  const move = usePlanEditorStore((state) => state.move);
  const remove = usePlanEditorStore((state) => state.remove);
  const select = usePlanEditorStore((state) => state.select);
  const setAdding = usePlanEditorStore((state) => state.setAdding);
  const generateCircle = usePlanEditorStore((state) => state.generateCircle);
  const generateSurvey = usePlanEditorStore((state) => state.generateSurvey);

  const spline = useUiStore((state) => state.spline);
  const setSpline = useUiStore((state) => state.setSpline);
  const mavftp = useUiStore((state) => state.mavftp);
  const setMavftp = useUiStore((state) => state.setMavftp);
  const askUpload = usePlanEditorStore((state) => state.askUpload);
  const askDownload = usePlanEditorStore((state) => state.askDownload);
  const uploadPartial = usePlanEditorStore((state) => state.uploadPartial);
  const askImport = usePlanEditorStore((state) => state.askImport);
  const confirm = usePlanEditorStore((state) => state.confirm);
  const cancel = usePlanEditorStore((state) => state.cancel);
  const saveToFile = usePlanEditorStore((state) => state.saveToFile);
  const closeEditor = usePlanEditorStore((state) => state.closeEditor);
  const dismissError = usePlanEditorStore((state) => state.dismissError);
  const undo = usePlanEditorStore((state) => state.undo);
  const redo = usePlanEditorStore((state) => state.redo);

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
  /** Whether the profile of the terrain is shown. */

  /** Whether the reticle of a survey is being drawn. */
  const [survey, setSurvey] = useState(false);
  /** How long the strips of the survey are, in metres. */
  const [stripLength, setStripLength] = useState(200);
  /** How far apart the strips of the survey are, in metres. */
  const [stripSpacing, setStripSpacing] = useState(50);
  /** How many strips the survey has. */
  const [strips, setStrips] = useState(5);
  /** The angle of the survey, in degrees from the north. */
  const [heading, setHeading] = useState(0);
  /** Whether the circle of a survey is being drawn. */
  const [circle, setCircle] = useState(false);
  /** The radius of the circle, in metres. */
  const [radius, setRadius] = useState(500);
  /** How many points the circle has. */
  const [circlePoints, setCirclePoints] = useState(12);
  /** Whether the circle is turned the other way. */
  const [clockwise, setClockwise] = useState(false);
  const [profile, setProfile] = useState(false);
  const [path, setPath] = useState("");
  const [dialogNote, setDialogNote] = useState<string | null>(null);

  const changed = isChanged({ draft, baseline });
  const format = useMemo(() => formatOfPath(path), [path]);

  // The catalogue of the commands is read once: the map needs it as well, to know
  // which items of a plan have a place at all.
  useEffect(() => {
    void loadCommands();
  }, [loadCommands]);

  // What the plan of this kind takes: a mission takes waypoints, a fence takes the
  // vertices of its polygon, and a rally takes the points the vehicle may come back
  // to.
  const planAddKind = addKindFor(kind);

  /** The name of the button, and what it says when it is armed. */
  const addLabel = {
    waypoint: "waypoint",
    vertex: "vertex",
    exclusion: "excluded vertex",
    rally: "rally point",
  }[planAddKind];

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
        ? await pickSavePath(path || fileNameOf("waypoints"), saveFilters(t))
        : await pickOpenPath(loadFilters(t));

    if (picked.ok) {
      setPath(picked.path);
      setDialogNote(null);
      return;
    }

    setDialogNote(
      picked.reason === "cancelled"
        ? t("No file was chosen.")
        : t("This build has no file dialog — type the name of the file."),
    );
  };

  const submitFile = () => {
    if (fileMode === null || format === null) {
      return;
    }

    // A drawing is a way in only: writing a plan to one would be a file the drawing
    // program cannot open and this one cannot read back.
    if (fileMode === "save") {
      if (!isWritable(format)) {
        setDialogNote(
          t(
            "A drawing can be read, not written — name a .waypoints or .kml file.",
          ),
        );
        return;
      }

      void saveToFile(path, format);
    } else {
      void askImport(path, format);
    }

    setFileMode(null);
  };

  const warnings: readonly PlanWarning[] =
    pending?.kind === "upload" ? pending.warnings : [];

  /** The kinds of plan the vehicle holds, in the order upstream has them. */
  const kinds: readonly PlanKind[] = ["mission", "fence", "rally"];

  // The tools that are not used every minute go into the menu of the bar: a bar that
  // wraps is read twice, and on a narrow window the last button ends up alone on a row
  // of its own.
  const moreItems: readonly OverflowItem[] = [
    ...(kind === "mission"
      ? [
          {
            id: "survey",
            label: t("Add a survey"),
            hint: t("Add the reticle of a survey to the plan"),
            disabled: busy || draft.length === 0,
            onSelect: () => {
              setSurvey(true);
            },
          },
          {
            id: "circle",
            label: t("Add a circle"),
            hint: t("Add a circle of points to the plan"),
            disabled: busy || draft.length === 0,
            onSelect: () => {
              setCircle(true);
            },
          },
        ]
      : []),
    {
      id: "profile",
      label: t("Profile of the ground"),
      hint: t("Show the shape of the ground the plan flies over"),
      disabled: busy || draft.length < 2,
      onSelect: () => {
        setProfile(true);
      },
    },
    {
      id: "partial",
      label: t("Write the plan from the item that is selected"),
      hint: t("The items before it are left on the vehicle as they are"),
      disabled: busy || selected === null || draft.length === 0,
      onSelect: () => {
        if (selected !== null) {
          void uploadPartial(selected);
        }
      },
    },
    {
      id: "save",
      label: t("Save to a file"),
      disabled: busy || draft.length === 0,
      onSelect: () => {
        openFile("save");
      },
    },
    {
      id: "load",
      label: t("Load from a file"),
      disabled: busy,
      onSelect: () => {
        openFile("load");
      },
    },
  ];

  return (
    <section className={styles.editor} aria-label={t("The plan editor")}>
      <div
        className={styles.kinds}
        role="tablist"
        aria-label={t("The kind of plan")}
      >
        {kinds.map((entry) => (
          <button
            key={entry}
            type="button"
            role="tab"
            aria-selected={kind === entry}
            className={`${styles.kind} ${kind === entry ? styles.kindOn : ""}`}
            onClick={() => {
              setKind(entry);
            }}
          >
            {t(KIND_LABELS[entry] ?? entry)}
          </button>
        ))}
      </div>

      <div className={styles.toolbar}>
        <div className={styles.group}>
          <button
            type="button"
            className="button"
            aria-label={t("Take the last change back")}
            disabled={busy || history.length === 0}
            title={t("Take the last change back (Ctrl+Z)")}
            onClick={undo}
          >
            {t("Undo")}
          </button>

          <button
            type="button"
            className="button"
            aria-label={t("Do again the change that was taken back")}
            disabled={busy || future.length === 0}
            title={t("Do again the change that was taken back (Ctrl+Shift+Z)")}
            onClick={redo}
          >
            {t("Redo")}
          </button>
        </div>

        <div className={styles.group}>
          <button
            type="button"
            className={`button ${adding && addKind === planAddKind ? styles.adding : ""}`}
            aria-label={t("Add a {name} on the map", { name: t(addLabel) })}
            aria-pressed={adding && addKind === planAddKind}
            disabled={busy}
            title={t("Click the map to add a {name}", { name: t(addLabel) })}
            onClick={() => {
              setAddKind(planAddKind);
              setAdding(!(adding && addKind === planAddKind));
            }}
          >
            {adding && addKind === planAddKind
              ? t("Adding a {name}", { name: t(addLabel) })
              : t("Add a {name}", { name: t(addLabel) })}
          </button>

          {kind === "fence" ? (
            <button
              type="button"
              className={`button ${adding && addKind === "exclusion" ? styles.adding : ""}`}
              aria-label={t("Add an excluded vertex on the map")}
              aria-pressed={adding && addKind === "exclusion"}
              disabled={busy}
              title={t(
                "Click the map to add a point that is excluded from the polygon",
              )}
              onClick={() => {
                setAddKind("exclusion");
                setAdding(!(adding && addKind === "exclusion"));
              }}
            >
              {t("Add exclusion")}
            </button>
          ) : null}
        </div>

        <div className={styles.group}>
          <button
            type="button"
            className="button"
            disabled={busy || draft.length === 0}
            onClick={() => {
              void askUpload();
            }}
          >
            {t("Upload plan")}
          </button>

          <button
            type="button"
            className="button"
            disabled={busy}
            onClick={() => {
              void askDownload();
            }}
          >
            {t("Download plan")}
          </button>
        </div>

        <div className={styles.group}>
          <label className={styles.spline}>
            <input
              type="checkbox"
              checked={spline}
              aria-label={t("Draw the path of the plan as a smooth curve")}
              onChange={(event) => {
                setSpline(event.target.checked);
              }}
            />
            <span>{t("Spline")}</span>
          </label>

          <label className={styles.spline}>
            <input
              type="checkbox"
              checked={mavftp}
              aria-label={t(
                "Carry the plan in the file the vehicle keeps it in",
              )}
              onChange={(event) => {
                setMavftp(event.target.checked);
              }}
            />
            <span>MAVFTP</span>
          </label>
        </div>

        <div className={`${styles.group} ${styles.end}`}>
          <OverflowMenu items={moreItems} />

          <button
            type="button"
            className="button"
            onClick={closeEditor}
            title={t("Put the graphs back and close the editor")}
          >
            {t("Close")}
          </button>
        </div>
      </div>

      {fileMode !== null ? (
        <div className={styles.fileBar}>
          <label className={styles.fileField}>
            <span className={styles.fileLabel}>
              {fileMode === "save"
                ? t("Write the plan to")
                : t("Read the plan from")}
            </span>
            <input
              className="input"
              type="text"
              spellCheck={false}
              autoComplete="off"
              value={path}
              placeholder={fileNameOf("waypoints")}
              aria-label={t("The name of the file")}
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
              ? t("The name must end in .waypoints, .kml, .shp or .dxf")
              : labelOfFormat(format, t)}
          </span>

          <button
            type="button"
            className="button"
            onClick={() => void browse()}
          >
            {t("Browse…")}
          </button>

          <button
            type="button"
            className="button button--primary"
            disabled={format === null || path.trim().length === 0}
            onClick={submitFile}
          >
            {fileMode === "save" ? t("Write the plan") : t("Read the plan")}
          </button>

          <button
            type="button"
            className="button"
            onClick={() => {
              setFileMode(null);
              setDialogNote(null);
            }}
          >
            {t("Cancel")}
          </button>
        </div>
      ) : null}

      {pending !== null ? (
        <div
          className={styles.confirm}
          role="alertdialog"
          aria-label={t("Confirm")}
        >
          <div className={styles.confirmText}>
            {pending.kind === "upload" ? (
              warnings.length > 0 ? (
                <>
                  <strong>
                    {t(
                      warnings.length === 1
                        ? "The plan has {count} warning."
                        : "The plan has {count} warnings.",
                      { count: warnings.length },
                    )}
                  </strong>
                  <ul className={styles.warnings}>
                    {warnings.map((warning) => (
                      <li key={`${warning.seq}-${warning.reason}`}>
                        {warning.seq === null
                          ? describeWarning(warning.reason, t)
                          : t("Item {number}: {warning}", {
                              number: warning.seq + 1,
                              warning: describeWarning(warning.reason, t),
                            })}
                      </li>
                    ))}
                  </ul>
                </>
              ) : (
                <span>
                  {t(
                    draft.length === 1
                      ? "Write {count} item to the vehicle?"
                      : "Write {count} items to the vehicle?",
                    { count: draft.length },
                  )}
                </span>
              )
            ) : pending.kind === "download" ? (
              <span>
                {changed
                  ? t(
                      "Read the plan from the vehicle again? The edits that have not been sent will be lost.",
                    )
                  : t("Read the plan from the vehicle again?")}
              </span>
            ) : (
              <span>
                {changed
                  ? t(
                      "Read the plan from {path}? The edits that have not been sent will be lost.",
                      { path: pending.path },
                    )
                  : t("Read the plan from {path}?", { path: pending.path })}
              </span>
            )}
          </div>

          <div className={styles.confirmActions}>
            <button type="button" className="button" onClick={cancel}>
              {t("Cancel")}
            </button>
            <button
              type="button"
              className="button button--primary"
              onClick={() => {
                void confirm();
              }}
            >
              {pending.kind === "upload" ? t("Upload") : t("Go on")}
            </button>
          </div>
        </div>
      ) : null}

      {survey ? (
        <div className={styles.fileBar}>
          <label className={styles.fileField}>
            <span className={styles.fileLabel}>{t("Strip m")}</span>
            <input
              className="input"
              type="number"
              min={1}
              max={100000}
              value={stripLength}
              aria-label={t("How long the strips are")}
              onChange={(event) => setStripLength(Number(event.target.value))}
            />
          </label>

          <label className={styles.fileField}>
            <span className={styles.fileLabel}>{t("Apart m")}</span>
            <input
              className="input"
              type="number"
              min={1}
              max={10000}
              value={stripSpacing}
              aria-label={t("How far apart the strips are")}
              onChange={(event) => setStripSpacing(Number(event.target.value))}
            />
          </label>

          <label className={styles.fileField}>
            <span className={styles.fileLabel}>{t("Strips")}</span>
            <input
              className="input"
              type="number"
              min={1}
              max={200}
              value={strips}
              aria-label={t("How many strips the survey has")}
              onChange={(event) => setStrips(Number(event.target.value))}
            />
          </label>

          <label className={styles.fileField}>
            <span className={styles.fileLabel}>{t("Angle")}</span>
            <input
              className="input"
              type="number"
              min={-180}
              max={180}
              value={heading}
              aria-label={t("The angle of the survey")}
              onChange={(event) => setHeading(Number(event.target.value))}
            />
          </label>

          <button
            type="button"
            className="button button--primary"
            disabled={busy}
            onClick={() => {
              const centre =
                draft.find((item) => item.seq === selected) ?? draft[0];

              if (centre === undefined) {
                return;
              }

              generateSurvey(
                { lat: centre.x, lng: centre.y },
                stripLength,
                stripSpacing,
                strips,
                heading,
                centre.z,
              );

              setSurvey(false);
            }}
          >
            {t("Add the reticle")}
          </button>

          <button
            type="button"
            className="button"
            onClick={() => {
              setSurvey(false);
            }}
          >
            {t("Cancel")}
          </button>
        </div>
      ) : null}

      {circle ? (
        <>
          <div className={styles.fileBar}>
            <label className={styles.fileField}>
              <span className={styles.fileLabel}>{t("Radius m")}</span>
              <input
                className="input"
                type="number"
                min={1}
                max={100000}
                value={radius}
                aria-label={t("The radius of the circle")}
                onChange={(event) => setRadius(Number(event.target.value))}
              />
            </label>

            <label className={styles.fileField}>
              <span className={styles.fileLabel}>{t("Points")}</span>
              <input
                className="input"
                type="number"
                min={3}
                max={200}
                value={circlePoints}
                aria-label={t("How many points the circle has")}
                onChange={(event) =>
                  setCirclePoints(Number(event.target.value))
                }
              />
            </label>

            <label className={styles.fileField}>
              <span className={styles.fileLabel}>{t("Direction")}</span>
              <select
                className="select"
                value={clockwise ? "clockwise" : "anticlockwise"}
                aria-label={t("The direction of the circle")}
                onChange={(event) =>
                  setClockwise(event.target.value === "clockwise")
                }
              >
                <option value="anticlockwise">{t("Anticlockwise")}</option>
                <option value="clockwise">{t("Clockwise")}</option>
              </select>
            </label>

            <button
              type="button"
              className="button button--primary"
              disabled={busy}
              onClick={() => {
                const centre =
                  draft.find((item) => item.seq === selected) ?? draft[0];

                if (centre === undefined) {
                  return;
                }

                generateCircle(
                  { lat: centre.x, lng: centre.y },
                  radius,
                  circlePoints,
                  centre.z,
                  clockwise,
                );

                setCircle(false);
              }}
            >
              {t("Add the circle")}
            </button>

            <button
              type="button"
              className="button"
              onClick={() => {
                setCircle(false);
              }}
            >
              {t("Cancel")}
            </button>
          </div>

          <p className={styles.hint}>
            {t(
              "The circle is centred on the waypoint that is selected, and it is added to the plan without throwing anything away.",
            )}
          </p>
        </>
      ) : null}

      {error !== null ? (
        <p className={styles.error} role="alert">
          <span className={styles.code}>{error.code}</span> {error.message}
          <button
            type="button"
            className={styles.dismiss}
            onClick={dismissError}
            aria-label={t("Put the failure away")}
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

      {profile ? (
        <TerrainProfile
          kind={kind}
          items={draft}
          onClose={() => {
            setProfile(false);
          }}
        />
      ) : null}

      <div className={styles.status}>
        <span className={styles.kind}>
          {KIND_LABELS[kind] ? t(KIND_LABELS[kind]) : kind}
        </span>
        <span className={styles.count}>
          {t(draft.length === 1 ? "{count} item" : "{count} items", {
            count: draft.length,
          })}
        </span>

        {changed ? (
          <span
            className={styles.changed}
            title={t("The edits are not on the vehicle yet")}
          >
            {t("unsent changes")}
          </span>
        ) : (
          <span className={styles.synced}>{t("the same as the vehicle")}</span>
        )}

        {adding ? (
          <span className={styles.hint}>
            {t("Click the map to add a waypoint — Escape stops it.")}
          </span>
        ) : null}

        {marked < places ? (
          <span className={styles.hint}>
            {t(
              "The map marks {marked} of {places} items — zoom in for the rest.",
              {
                marked,
                places,
              },
            )}
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
