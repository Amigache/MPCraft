import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useLinkStore } from "../../lib/link/linkStore";
import { useParamsStore } from "../../lib/params/paramsStore";
import { formatParameterValue } from "../../lib/params/paramValues";
import { useUiStore } from "../../lib/ui/uiStore";
import { ParameterList } from "./ParameterList";
import styles from "./ParameterSheet.module.css";

/**
 * The sheet of parameters.
 *
 * Mission Planner keeps its parameters in a window of their own. MPCraft is one window,
 * so the parameters come as a sheet over it: the window is dimmed, the sheet is laid
 * over the top of it, and closing the sheet puts the window back as it was.
 *
 * The list is what the vehicle has reported. It is empty until the vehicle volunteers
 * something, which is why there is a button that asks for the whole list.
 */

/** How often the list is read again, in milliseconds. */
const POLL_MS = 1000;

export function ParameterSheet() {
  const open = useUiStore((state) => state.parametersOpen);
  const hide = useUiStore((state) => state.hideParameters);

  const parameters = useParamsStore((state) => state.parameters);
  const reading = useParamsStore((state) => state.reading);
  const writing = useParamsStore((state) => state.writing);
  const lastWrite = useParamsStore((state) => state.lastWrite);
  const error = useParamsStore((state) => state.error);
  const refresh = useParamsStore((state) => state.refresh);
  const readAll = useParamsStore((state) => state.readAll);
  const write = useParamsStore((state) => state.write);

  const status = useLinkStore((state) => state.status);

  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<string | null>(null);

  const panel = useRef<HTMLDivElement | null>(null);
  const search = useRef<HTMLInputElement | null>(null);
  /** Whether the last pointerdown cancelled the edit. */
  const cancelled = useRef(false);

  const linked = status === "connected" || status === "reconnecting";

  const rows = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (needle.length === 0) {
      return parameters;
    }

    return parameters.filter((parameter) =>
      parameter.name.toLowerCase().includes(needle),
    );
  }, [parameters, query]);

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
        </div>

        <div className={styles.body}>
          {rows.length > 0 ? (
            <ParameterList
              rows={rows}
              editing={editing}
              writing={writing}
              onEdit={setEditing}
              onCancel={() => {
                setEditing(null);
              }}
              onWrite={handleWrite}
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
        </footer>
      </div>
    </div>
  );
}
