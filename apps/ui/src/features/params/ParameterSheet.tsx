import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { Splitter } from "../../app/layout/Splitter";
import { pickOpenPath, pickSavePath } from "../../lib/ipc/dialog";
import {
  compareParameters,
  exportParameters,
  importParameters,
  normaliseError,
  writeParametersToFlash,
} from "../../lib/ipc/bridge";
import type { Parameter } from "../../lib/ipc/types";
import { useLinkStore } from "../../lib/link/linkStore";
import {
  orderParameters,
  useFavouritesStore,
} from "../../lib/params/favouritesStore";
import { useMetadata } from "../../lib/params/metadataStore";
import { buildParamTree, findNode, isUnder } from "../../lib/params/paramTree";
import { useParamsStore } from "../../lib/params/paramsStore";
import { formatParameterValue } from "../../lib/params/paramValues";
import { useUiStore } from "../../lib/ui/uiStore";
import { ParameterList } from "./ParameterList";
import { ParameterTree } from "./ParameterTree";
import styles from "./ParameterSheet.module.css";

/**
 * The sheet of parameters.
 *
 * Mission Planner keeps its parameters in a window of their own, with a tree of the
 * groups of parameters on the left and the parameters themselves on the right. MPCraft
 * is one window, so the parameters come as a sheet over it: the window is dimmed, the
 * sheet is laid over the top of it, and closing the sheet puts the window back as it
 * was.
 *
 * The list is what the vehicle has reported. It is empty until the vehicle volunteers
 * something, which is why there is a button that asks for the whole list.
 *
 * The tree is the way a parameter of a vehicle with hundreds of them is found: the
 * branches are the common prefixes of the names, and clicking one shows the parameters
 * under it. The search narrows the tree and the list at the same time.
 *
 * The favourites are the parameters the operator marked, and they are kept between two
 * runs. They are at the top of the list, in the order they were marked, because they
 * are the ones the operator is looking for.
 */

/** How often the list is read again, in milliseconds. */
const POLL_MS = 1000;

/** The width of the tree of the parameters, in pixels, when nothing has been changed. */
const DEFAULT_TREE_WIDTH = 208;

/** The narrowest the tree may be dragged. */
const MIN_TREE_WIDTH = 140;

/** The widest the tree may be dragged. */
const MAX_TREE_WIDTH = 480;

/** The width of the sheet, in pixels, when nothing has been changed. */
const DEFAULT_SHEET_WIDTH = 760;

/**
 * The room the list needs beside the tree, in pixels.
 *
 * A row is a star, a name, a value and a type: the name needs room to be read, and the
 * value and the type need room of their own.
 */
const LIST_WIDTH = 440;

/** The kinds of file the parameter dialog offers. */
const PARAMETER_FILTERS = [
  { name: "Parameter file", extensions: ["param", "parm"] },
  { name: "All files", extensions: ["*"] },
];

/** The widest the sheet may grow. */
const MAX_SHEET_WIDTH = 1080;

export function ParameterSheet() {
  const open = useUiStore((state) => state.parametersOpen);
  const hide = useUiStore((state) => state.hideParameters);
  const group = useUiStore((state) => state.parameterGroup);
  const showParameterGroup = useUiStore((state) => state.showParameterGroup);

  const parameters = useParamsStore((state) => state.parameters);
  const reading = useParamsStore((state) => state.reading);
  const writing = useParamsStore((state) => state.writing);
  const lastWrite = useParamsStore((state) => state.lastWrite);
  const error = useParamsStore((state) => state.error);
  const refresh = useParamsStore((state) => state.refresh);
  const readAll = useParamsStore((state) => state.readAll);
  const write = useParamsStore((state) => state.write);

  const favourites = useFavouritesStore((state) => state.favourites);
  const favouriteError = useFavouritesStore((state) => state.error);
  const loadFavourites = useFavouritesStore((state) => state.load);
  const toggleFavourite = useFavouritesStore((state) => state.toggle);

  const status = useLinkStore((state) => state.status);

  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  /** The prefix of the branch of the tree whose parameters are shown. */
  const [selected, setSelected] = useState("");
  /** The width of the tree, in pixels. */
  const [treeWidth, setTreeWidth] = useState(DEFAULT_TREE_WIDTH);

  /** The parameters a file holds, waiting to be written to the vehicle. */
  const [pendingLoad, setPendingLoad] = useState<readonly Parameter[] | null>(
    null,
  );
  /** What the last file operation said. */
  const [fileNotice, setFileNotice] = useState<string | null>(null);
  /** Whether a file operation is in flight. */
  const [fileBusy, setFileBusy] = useState(false);

  const panel = useRef<HTMLDivElement | null>(null);
  const search = useRef<HTMLInputElement | null>(null);
  /** Whether the last pointerdown cancelled the edit. */
  const cancelled = useRef(false);

  const linked = status === "connected" || status === "reconnecting";

  // The search narrows the tree and the list at the same time: a branch that has
  // nothing under it after the search is not drawn at all.
  const matched = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (needle.length === 0) {
      return parameters;
    }

    return parameters.filter((parameter) =>
      parameter.name.toLowerCase().includes(needle),
    );
  }, [parameters, query]);

  // What is known about the parameters: the description, the unit, the range and the
  // value a parameter had before anybody changed it.
  const { index: metadata, error: metadataError } = useMetadata(
    parameters.map((parameter) => parameter.name),
  );

  const tree = useMemo(
    () => buildParamTree(matched.map((parameter) => parameter.name)),
    [matched],
  );

  // The branch that is chosen. When the search has taken it away, the whole list is
  // shown instead, and the tree says so.
  const node = useMemo(
    () => findNode(tree, selected) ?? tree,
    [selected, tree],
  );

  const rows = useMemo(
    () =>
      orderParameters(
        matched.filter((parameter) =>
          isUnder(node, parameter.name) &&
          (group === null ||
            (metadata.get(parameter.name)?.group ?? "") === group),
        ),
        favourites,
      ),
    [favourites, group, matched, metadata, node],
  );

  // The sheet grows when the tree is dragged wider, so that the list keeps the room it
  // needs.
  const sheetWidth = Math.min(
    MAX_SHEET_WIDTH,
    Math.max(DEFAULT_SHEET_WIDTH, treeWidth + LIST_WIDTH),
  );

  // The list is read again and again while the sheet is open: a parameter the vehicle
  // changes on its own has to show up in the list without the operator asking for it.
  useEffect(() => {
    if (!open) {
      return;
    }

    void refresh();

    const timer = window.setInterval(() => {
      // A write that is in flight carries the value the vehicle stored; reading the
      // list in the middle of it would put the old value back for a moment.
      if (useParamsStore.getState().writing !== null) {
        return;
      }

      void refresh();
    }, POLL_MS);

    return () => {
      window.clearInterval(timer);
    };
  }, [open, refresh]);

  // The favourites are the ones the operator marked before, and they are kept between
  // two runs: they are read when the sheet opens.
  useEffect(() => {
    if (!open) {
      return;
    }

    void loadFavourites();
  }, [open, loadFavourites]);

  // The search box has the focus as soon as the sheet opens, so that the operator can
  // type the name of a parameter without reaching for the mouse.
  useEffect(() => {
    if (open) {
      search.current?.focus();
    } else {
      setEditing(null);
    }
  }, [open]);

  // The escape key closes the sheet — unless a value is being edited, in which case it
  // belongs to the edit and the row takes it.
  useEffect(() => {
    if (!open) {
      return;
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && editing === null) {
        hide();
      }
    };

    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open, editing, hide]);

  // The sheet keeps the keyboard to itself: the tab goes round the sheet and never
  // reaches the window underneath it.
  useEffect(() => {
    if (!open) {
      return;
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Tab") {
        return;
      }

      const root = panel.current;
      if (root === null) {
        return;
      }

      const focusable = root.querySelectorAll<HTMLElement>(
        "button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex='-1'])",
      );

      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (first === undefined || last === undefined) {
        return;
      }

      const active = document.activeElement;
      const inside = active !== null && root.contains(active);

      if (event.shiftKey && (!inside || active === first)) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (!inside || active === last)) {
        event.preventDefault();
        first.focus();
      }
    };

    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  // A click somewhere else abandons the edit. The pointerdown that did it is
  // remembered, so that the click that follows it does not close the sheet as well.
  useEffect(() => {
    if (editing === null) {
      return;
    }

    const onPointerDown = (event: PointerEvent) => {
      // Every pointerdown starts by forgetting the last one: the flag says whether
      // *this* pointerdown cancelled the edit.
      cancelled.current = false;

      const target = event.target;
      const editor =
        panel.current?.querySelector("[data-editor='true']") ?? null;

      if (target instanceof Node && editor !== null && editor.contains(target)) {
        return;
      }

      cancelled.current = true;
      setEditing(null);
    };

    document.addEventListener("pointerdown", onPointerDown, true);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown, true);
    };
  }, [editing]);

  /**
   * Writes the parameters of the vehicle to a file.
   *
   * The format is upstream's: a line per parameter, the name and the value separated by
   * a comma.
   */
  const saveFile = useCallback(async () => {
    const picked = await pickSavePath("vehicle.param", PARAMETER_FILTERS);

    if (!picked.ok) {
      return;
    }

    setFileBusy(true);

    try {
      await exportParameters(picked.path);
      setFileNotice(`The parameters were written to ${picked.path}.`);
      setPendingLoad(null);
    } catch (cause) {
      setFileNotice(`${normaliseError(cause).code} — the file was not written.`);
    } finally {
      setFileBusy(false);
    }
  }, []);

  /**
   * Reads the parameters of a file.
   *
   * They are not written to the vehicle yet: the operator is asked first, because
   * writing a parameter is the vehicle's own memory changing.
   */
  const openFile = useCallback(async () => {
    const picked = await pickOpenPath(PARAMETER_FILTERS);

    if (!picked.ok) {
      return;
    }

    setFileBusy(true);

    try {
      const parameters = await importParameters(picked.path);
      setPendingLoad(parameters);
      setFileNotice(
        `${parameters.length} parameters were read from ${picked.path}.`,
      );
    } catch (cause) {
      setFileNotice(`${normaliseError(cause).code} — the file was not read.`);
    } finally {
      setFileBusy(false);
    }
  }, []);

  /**
   * Says how the parameters of the vehicle differ from the ones of a file.
   *
   * The differences are what the operator needs to know before writing anything: a
   * parameter the vehicle has and the file does not, and a parameter whose value is not
   * the one that was asked for.
   */
  const compareFile = useCallback(async () => {
    const picked = await pickOpenPath(PARAMETER_FILTERS);

    if (!picked.ok) {
      return;
    }

    setFileBusy(true);

    try {
      const differences = await compareParameters(picked.path);

      if (differences.length === 0) {
        setFileNotice("The vehicle and the file agree.");
      } else {
        const lines = differences
          .slice(0, 8)
          .map(
            (difference) =>
              `${difference.name}: the vehicle has ${difference.current ?? "-"}, the file has ${difference.other ?? "-"}`,
          );

        setFileNotice(
          `${differences.length} parameters differ. ${lines.join(" · ")}`,
        );
      }

      setPendingLoad(null);
    } catch (cause) {
      setFileNotice(`${normaliseError(cause).code} — the file was not read.`);
    } finally {
      setFileBusy(false);
    }
  }, []);

  /**
   * Writes the parameters of a file to the vehicle.
   *
   * They are written one by one, because the vehicle confirms each of them: a file of
   * two hundred parameters is two hundred writes, and a vehicle that refuses one of them
   * says which.
   */
  const confirmLoad = useCallback(async () => {
    const parameters = pendingLoad;
    setPendingLoad(null);

    if (parameters === null) {
      return;
    }

    setFileBusy(true);

    let written = 0;

    for (const parameter of parameters) {
      await write(parameter.name, parameter.value);
      written += 1;
    }

    setFileNotice(`${written} parameters were written to the vehicle.`);
    setFileBusy(false);
  }, [pendingLoad, write]);

  /** Asks the vehicle to write its parameters to its own memory. */
  const writeToFlash = useCallback(async () => {
    setFileBusy(true);

    try {
      await writeParametersToFlash();
      setFileNotice("The vehicle has written its parameters to its own memory.");
    } catch (cause) {
      setFileNotice(`${normaliseError(cause).code} — the vehicle did not write them.`);
    } finally {
      setFileBusy(false);
    }
  }, []);

  const handleWrite = useCallback(
    (name: string, value: number) => {
      void write(name, value).then(() => {
        // The row goes back to showing the value, whether the write was accepted or
        // refused: the value on the row is the one the vehicle holds.
        setEditing(null);
      });
    },
    [write],
  );

  if (!open) {
    return null;
  }

  return (
    <div
      className={styles.backdrop}
      role="presentation"
      onClick={() => {
        if (cancelled.current) {
          cancelled.current = false;
          return;
        }

        hide();
      }}
    >
      <div
        ref={panel}
        className={styles.panel}
        style={{ width: `min(${sheetWidth}px, 100%)` }}
        role="dialog"
        aria-modal="true"
        aria-labelledby="parameters-title"
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <header className={styles.header}>
          <h2 className={styles.title} id="parameters-title">
            Parameters
          </h2>
          <span className={styles.count}>{parameters.length} reported</span>
          <div className={styles.headerActions}>
            <button
              type="button"
              className="button"
              disabled={reading || !linked}
              title={linked ? undefined : "no link"}
              onClick={() => {
                void readAll();
              }}
            >
              {reading ? "Reading…" : "Read all"}
            </button>
            <button
              type="button"
              className="button"
              disabled={fileBusy || !linked}
              title="Read the parameters of a file and write them to the vehicle"
              onClick={() => {
                void openFile();
              }}
            >
              Load…
            </button>

            <button
              type="button"
              className="button"
              disabled={fileBusy || !linked}
              title="Write the parameters of the vehicle to a file"
              onClick={() => {
                void saveFile();
              }}
            >
              Save…
            </button>

            <button
              type="button"
              className="button"
              disabled={fileBusy || !linked}
              title="Say how the parameters of the vehicle differ from the ones of a file"
              onClick={() => {
                void compareFile();
              }}
            >
              Compare…
            </button>

            <button
              type="button"
              className="button"
              disabled={fileBusy || !linked}
              title="Ask the vehicle to write its parameters to its own memory"
              onClick={() => {
                void writeToFlash();
              }}
            >
              Write to flash
            </button>

            <button type="button" className="button" onClick={hide}>
              Close
            </button>
          </div>
        </header>

        {error !== null ? (
          <p className={styles.errorBar} role="alert">
            <span className={styles.code}>{error.code}</span> {error.message}
          </p>
        ) : null}

        {metadataError !== null ? (
          <p className={styles.errorBar} role="alert">
            <span className={styles.code}>{metadataError.code}</span>{" "}
            {metadataError.message} — the descriptions of the parameters are not
            shown.
          </p>
        ) : null}

        <div className={styles.searchRow}>
          <input
            ref={search}
            className={`input ${styles.search}`}
            type="search"
            value={query}
            spellCheck={false}
            autoComplete="off"
            placeholder="Search the parameters by name"
            aria-label="Search the parameters by name"
            onChange={(event) => {
              setQuery(event.target.value);
            }}
          />
          {query.trim().length > 0 ? (
            <span className={styles.filtered}>
              {rows.length} of {parameters.length}
            </span>
          ) : null}

          {group !== null ? (
            <span className={styles.groupChip}>
              <span className={styles.groupName}>{group}</span>
              <button
                type="button"
                className="button"
                title="Show every parameter again"
                onClick={() => {
                  showParameterGroup(null);
                }}
              >
                All
              </button>
            </span>
          ) : null}
        </div>

        <div className={styles.body}>
          <div className={styles.treePane} style={{ width: `${treeWidth}px` }}>
            <ParameterTree
              tree={tree}
              selected={node.prefix}
              onSelect={(prefix) => {
                setSelected(prefix);
              }}
            />
          </div>

          <Splitter
            orientation="vertical"
            label="The width of the tree of the parameters"
            value={treeWidth}
            min={MIN_TREE_WIDTH}
            max={MAX_TREE_WIDTH}
            onChange={setTreeWidth}
          />

          <div className={styles.listPane}>
            {rows.length > 0 ? (
              <ParameterList
                rows={rows}
                editing={editing}
                writing={writing}
                favourites={favourites}
                metadata={metadata}
                onEdit={setEditing}
                onCancel={() => {
                  setEditing(null);
                }}
                onWrite={handleWrite}
                onToggleFavourite={(name) => {
                  void toggleFavourite(name);
                }}
              />
            ) : (
              <div className={styles.empty}>
                {error !== null ? (
                  <p className={styles.hint}>
                    The parameters could not be read. The link may be down.
                  </p>
                ) : reading ? (
                  <p className={styles.hint}>Reading the parameters…</p>
                ) : query.trim().length > 0 ? (
                  <p className={styles.hint}>
                    No parameter matches “{query.trim()}”.
                  </p>
                ) : node.prefix !== "" ? (
                  <p className={styles.hint}>
                    Nothing under {node.label} matches the search.
                  </p>
                ) : (
                  <>
                    <p className={styles.hint}>
                      The vehicle has not reported a parameter yet.
                    </p>
                    <p className={styles.hint}>
                      Press <strong>Read all</strong> to ask it for the whole list.
                    </p>
                  </>
                )}
              </div>
            )}
          </div>
        </div>

        <footer className={styles.footer}>
          {lastWrite === null ? (
            <span className={styles.idle}>
              No parameter has been written yet.
            </span>
          ) : lastWrite.kind === "written" ? (
            <span className={styles.written} role="status">
              {lastWrite.parameter.name} written — the vehicle stored{" "}
              {formatParameterValue(
                lastWrite.parameter.value,
                lastWrite.parameter.reportedType,
              )}
            </span>
          ) : (
            <span className={styles.failed} role="alert">
              <span className={styles.code}>{lastWrite.code}</span>{" "}
              {lastWrite.message}
            </span>
          )}

          {fileNotice !== null ? (
            <p className={styles.favouriteError} role="status">
              {fileNotice}
            </p>
          ) : null}

          {pendingLoad !== null ? (
            <p className={styles.favouriteError} role="alert">
              {pendingLoad.length} parameters are waiting to be written to the
              vehicle.
              <button
                type="button"
                className="button"
                onClick={() => {
                  setPendingLoad(null);
                }}
              >
                No
              </button>
              <button
                type="button"
                className="button"
                onClick={() => {
                  void confirmLoad();
                }}
              >
                Yes
              </button>
            </p>
          ) : null}

          {favouriteError !== null ? (
            <p className={styles.favouriteError} role="alert">
              <span className={styles.code}>{favouriteError.code}</span>{" "}
              {favouriteError.message} — the favourites have not been changed.
            </p>
          ) : null}
        </footer>
      </div>
    </div>
  );
}
