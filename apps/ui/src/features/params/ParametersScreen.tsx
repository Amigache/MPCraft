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
import { useLocaleStore } from "../../lib/i18n/localeStore";
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
import styles from "./ParametersScreen.module.css";

/**
 * The screen of the parameters.
 *
 * Mission Planner keeps its parameters in a window of their own, with a tree of the
 * groups of parameters on the left and the parameters themselves on the right. MPCraft
 * is one window with four screens, and the parameters are one of them: the switcher puts
 * the window on this screen, and leaving it puts the window back on the flight data.
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

/** How a part of the interface says something in the language the window is read in. */
type Say = (source: string, values?: Record<string, string | number>) => string;

/** The kinds of file the parameter dialog offers. */
function parameterFilters(t: Say) {
  return [
    { name: t("Parameter file"), extensions: ["param", "parm"] },
    { name: t("All files"), extensions: ["*"] },
  ];
}

export function ParametersScreen() {
  // The screen is where this screen lives, so it renders exactly when the window is on
  // it. It stays mounted in the shell while another screen is shown, so that the search
  // and the chosen branch are not forgotten when the operator comes back.
  const open = useUiStore((state) => state.screen === "setup");
  const hide = useUiStore((state) => state.hideParameters);
  const group = useUiStore((state) => state.parameterGroup);
  const prefix = useUiStore((state) => state.parameterPrefix);
  const showParameterGroup = useUiStore((state) => state.showParameterGroup);

  const parameters = useParamsStore((state) => state.parameters);
  const reading = useParamsStore((state) => state.reading);
  const writing = useParamsStore((state) => state.writing);
  const lastWrite = useParamsStore((state) => state.lastWrite);
  const error = useParamsStore((state) => state.error);
  const refresh = useParamsStore((state) => state.refresh);
  const readAll = useParamsStore((state) => state.readAll);
  const write = useParamsStore((state) => state.write);
  const writeBatch = useParamsStore((state) => state.writeBatch);
  const changed = useParamsStore((state) => state.changed);

  const favourites = useFavouritesStore((state) => state.favourites);
  const favouriteError = useFavouritesStore((state) => state.error);
  const loadFavourites = useFavouritesStore((state) => state.load);
  const toggleFavourite = useFavouritesStore((state) => state.toggle);

  const status = useLinkStore((state) => state.status);

  const t = useLocaleStore((state) => state.t);

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

  // What the sheet shows is a scope: a group of the catalogue, or the parameters whose
  // names begin with a word. The two never mix, because the parameters of a prefix are
  // not in any group of the catalogue.
  const rows = useMemo(
    () =>
      orderParameters(
        matched.filter((parameter) => {
          if (!isUnder(node, parameter.name)) {
            return false;
          }

          if (prefix !== null) {
            return parameter.name.startsWith(prefix);
          }

          return (
            group === null ||
            (metadata.get(parameter.name)?.group ?? "") === group
          );
        }),
        favourites,
      ),
    [favourites, group, matched, metadata, node, prefix],
  );

  // The screen fills the window, so the room the list needs beside the tree is what the
  // window gives it.

  // The list is read again and again while the screen is open: a parameter the vehicle
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
  // two runs: they are read when the screen opens.
  useEffect(() => {
    if (!open) {
      return;
    }

    void loadFavourites();
  }, [open, loadFavourites]);

  // The search box has the focus as soon as the screen opens, so that the operator can
  // type the name of a parameter without reaching for the mouse. When the screen is left,
  // an edit that was not written is abandoned.
  useEffect(() => {
    if (open) {
      search.current?.focus();
    } else {
      setEditing(null);
    }
  }, [open]);

  // The escape key takes the window back to the flight data — unless a value is being
  // edited, in which case it belongs to the edit and the row takes it.
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

  // A click somewhere else abandons the edit. The pointerdown that did it is
  // remembered, so that the click that follows it does not close the sheet as well.
  useEffect(() => {
    if (editing === null) {
      return;
    }

    const onPointerDown = (event: PointerEvent) => {
      const target = event.target;
      const editor =
        panel.current?.querySelector("[data-editor='true']") ?? null;

      if (
        target instanceof Node &&
        editor !== null &&
        editor.contains(target)
      ) {
        return;
      }

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
    const picked = await pickSavePath("vehicle.param", parameterFilters(t));

    if (!picked.ok) {
      return;
    }

    setFileBusy(true);

    try {
      await exportParameters(picked.path);
      setFileNotice(
        t("The parameters were written to {path}.", { path: picked.path }),
      );
      setPendingLoad(null);
    } catch (cause) {
      setFileNotice(
        t("{code} — the file was not written.", {
          code: normaliseError(cause).code,
        }),
      );
    } finally {
      setFileBusy(false);
    }
  }, [t]);

  /**
   * Reads the parameters of a file.
   *
   * They are not written to the vehicle yet: the operator is asked first, because
   * writing a parameter is the vehicle's own memory changing.
   */
  const openFile = useCallback(async () => {
    const picked = await pickOpenPath(parameterFilters(t));

    if (!picked.ok) {
      return;
    }

    setFileBusy(true);

    try {
      const parameters = await importParameters(picked.path);
      setPendingLoad(parameters);
      setFileNotice(
        t(
          parameters.length === 1
            ? "{count} parameter was read from {path}."
            : "{count} parameters were read from {path}.",
          { count: parameters.length, path: picked.path },
        ),
      );
    } catch (cause) {
      setFileNotice(
        t("{code} — the file was not read.", {
          code: normaliseError(cause).code,
        }),
      );
    } finally {
      setFileBusy(false);
    }
  }, [t]);

  /**
   * Says how the parameters of the vehicle differ from the ones of a file.
   *
   * The differences are what the operator needs to know before writing anything: a
   * parameter the vehicle has and the file does not, and a parameter whose value is not
   * the one that was asked for.
   */
  const compareFile = useCallback(async () => {
    const picked = await pickOpenPath(parameterFilters(t));

    if (!picked.ok) {
      return;
    }

    setFileBusy(true);

    try {
      const differences = await compareParameters(picked.path);

      if (differences.length === 0) {
        setFileNotice(t("The vehicle and the file agree."));
      } else {
        const lines = differences.slice(0, 8).map((difference) =>
          t(
            "{name}: the vehicle has {vehicleValue}, the file has {fileValue}",
            {
              name: difference.name,
              vehicleValue: difference.current ?? "-",
              fileValue: difference.other ?? "-",
            },
          ),
        );

        setFileNotice(
          t(
            differences.length === 1
              ? "{count} parameter differs. {lines}"
              : "{count} parameters differ. {lines}",
            { count: differences.length, lines: lines.join(" · ") },
          ),
        );
      }

      setPendingLoad(null);
    } catch (cause) {
      setFileNotice(
        t("{code} — the file was not read.", {
          code: normaliseError(cause).code,
        }),
      );
    } finally {
      setFileBusy(false);
    }
  }, [t]);

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

    // The parameters of the file are written in the order the vehicle needs them: the
    // ones that turn something on are written last. The order is the autopilots'
    // knowledge, so it is the backend that applies it.
    const changes = parameters.map((parameter) => ({
      name: parameter.name,
      value: parameter.value,
    }));

    await writeBatch(changes);

    setFileNotice(
      t(
        changes.length === 1
          ? "{count} parameter was written to the vehicle."
          : "{count} parameters were written to the vehicle.",
        { count: changes.length },
      ),
    );
    setFileBusy(false);
  }, [pendingLoad, writeBatch, t]);

  /**
   * Puts back the parameters the operator changed, as they were before.
   *
   * Only the parameters that were changed are put back, and only the ones the
   * documentation knows the default of. A parameter that has no default is left alone:
   * there is no telling what it should be, and guessing would be worse than leaving it.
   */
  const restoreDefaults = useCallback(async () => {
    const changes: { name: string; value: number }[] = [];

    for (const name of changed) {
      const meta = metadata.get(name);

      if (meta === undefined || meta.default === "") {
        continue;
      }

      const value = Number.parseFloat(meta.default);

      if (!Number.isFinite(value)) {
        continue;
      }

      changes.push({ name, value });
    }

    if (changes.length === 0) {
      setFileNotice(
        t(
          "There is nothing to put back: no parameter that was changed has a default in the documentation.",
        ),
      );
      return;
    }

    setFileBusy(true);

    try {
      await writeBatch(changes);
      setFileNotice(
        t(
          changes.length === 1
            ? "{count} parameter was put back as it was."
            : "{count} parameters were put back as they were.",
          { count: changes.length },
        ),
      );
    } catch (cause) {
      setFileNotice(
        t("{code} — the parameters were not put back.", {
          code: normaliseError(cause).code,
        }),
      );
    } finally {
      setFileBusy(false);
    }
  }, [changed, metadata, writeBatch, t]);

  /** Asks the vehicle to write its parameters to its own memory. */
  const writeToFlash = useCallback(async () => {
    setFileBusy(true);

    try {
      await writeParametersToFlash();
      setFileNotice(
        t("The vehicle has written its parameters to its own memory."),
      );
    } catch (cause) {
      setFileNotice(
        t("{code} — the vehicle did not write them.", {
          code: normaliseError(cause).code,
        }),
      );
    } finally {
      setFileBusy(false);
    }
  }, [t]);

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
    <div ref={panel} className={styles.screen}>
      <header className={styles.header}>
        <h1 className={styles.title} id="parameters-title">
          {t("Parameters")}
        </h1>
        <span className={styles.count}>
          {t(
            parameters.length === 1
              ? "{count} parameter reported"
              : "{count} parameters reported",
            { count: parameters.length },
          )}
        </span>
        <div className={styles.headerActions}>
          <button
            type="button"
            className="button"
            disabled={reading || !linked}
            title={linked ? undefined : t("no link")}
            onClick={() => {
              void readAll();
            }}
          >
            {reading ? t("Reading…") : t("Read all")}
          </button>
          <button
            type="button"
            className="button"
            disabled={fileBusy || !linked}
            title={t(
              "Read the parameters of a file and write them to the vehicle",
            )}
            onClick={() => {
              void openFile();
            }}
          >
            {t("Load…")}
          </button>

          <button
            type="button"
            className="button"
            disabled={fileBusy || !linked}
            title={t("Write the parameters of the vehicle to a file")}
            onClick={() => {
              void saveFile();
            }}
          >
            {t("Save…")}
          </button>

          <button
            type="button"
            className="button"
            disabled={fileBusy || !linked}
            title={t(
              "Say how the parameters of the vehicle differ from the ones of a file",
            )}
            onClick={() => {
              void compareFile();
            }}
          >
            {t("Compare…")}
          </button>

          <button
            type="button"
            className="button"
            disabled={fileBusy || !linked || changed.length === 0}
            title={t(
              "Put back the parameters that were changed, as they were before",
            )}
            onClick={() => {
              void restoreDefaults();
            }}
          >
            {t("Restore the defaults")}
          </button>

          <button
            type="button"
            className="button"
            disabled={fileBusy || !linked}
            title={t(
              "Ask the vehicle to write its parameters to its own memory",
            )}
            onClick={() => {
              void writeToFlash();
            }}
          >
            {t("Write to flash")}
          </button>

          <button type="button" className="button" onClick={hide}>
            {t("Close")}
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
          {metadataError.message} —{" "}
          {t("the descriptions of the parameters are not shown.")}
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
          placeholder={t("Search the parameters by name")}
          aria-label={t("Search the parameters by name")}
          onChange={(event) => {
            setQuery(event.target.value);
          }}
        />
        {query.trim().length > 0 ? (
          <span className={styles.filtered}>
            {t("{number} of {count}", {
              number: rows.length,
              count: parameters.length,
            })}
          </span>
        ) : null}

        {group !== null ? (
          <span className={styles.groupChip}>
            <span className={styles.groupName}>{group}</span>
            <button
              type="button"
              className="button"
              title={t("Show every parameter again")}
              onClick={() => {
                showParameterGroup(null);
              }}
            >
              {t("All")}
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
          label={t("The width of the tree of the parameters")}
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
                  {t("The parameters could not be read. The link may be down.")}
                </p>
              ) : reading ? (
                <p className={styles.hint}>{t("Reading the parameters…")}</p>
              ) : query.trim().length > 0 ? (
                <p className={styles.hint}>
                  {t("No parameter matches “{query}”.", {
                    query: query.trim(),
                  })}
                </p>
              ) : node.prefix !== "" ? (
                <p className={styles.hint}>
                  {t("Nothing under {label} matches the search.", {
                    label: node.label,
                  })}
                </p>
              ) : (
                <>
                  <p className={styles.hint}>
                    {t("The vehicle has not reported a parameter yet.")}
                  </p>
                  <p className={styles.hint}>
                    {t("Press")} <strong>{t("Read all")}</strong>{" "}
                    {t("to ask it for the whole list.")}
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
            {t("No parameter has been written yet.")}
          </span>
        ) : lastWrite.kind === "written" ? (
          <span className={styles.written} role="status">
            {t("{name} written — the vehicle stored {value}", {
              name: lastWrite.parameter.name,
              value: formatParameterValue(
                lastWrite.parameter.value,
                lastWrite.parameter.reportedType,
              ),
            })}
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
            {t(
              pendingLoad.length === 1
                ? "{count} parameter is waiting to be written to the vehicle."
                : "{count} parameters are waiting to be written to the vehicle.",
              { count: pendingLoad.length },
            )}
            <button
              type="button"
              className="button"
              onClick={() => {
                setPendingLoad(null);
              }}
            >
              {t("No")}
            </button>
            <button
              type="button"
              className="button"
              onClick={() => {
                void confirmLoad();
              }}
            >
              {t("Yes")}
            </button>
          </p>
        ) : null}

        {favouriteError !== null ? (
          <p className={styles.favouriteError} role="alert">
            <span className={styles.code}>{favouriteError.code}</span>{" "}
            {favouriteError.message} —{" "}
            {t("the favourites have not been changed.")}
          </p>
        ) : null}
      </footer>
    </div>
  );
}
