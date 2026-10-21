import { useEffect, useRef, useState } from "react";

import type { KnownParameter } from "../../lib/params/paramsStore";
import {
  formatParameterValue,
  parseParameterValue,
} from "../../lib/params/paramValues";
import { isIntegralType, paramTypeName } from "../../lib/params/paramTypes";
import styles from "./ParameterSheet.module.css";

/**
 * One parameter.
 *
 * The row shows what the store says, and nothing of its own: the value it draws is the
 * value the vehicle has, whether it was put there by a write or by the vehicle changing
 * its mind.
 *
 * Clicking the value turns it into an input, filled with the value as it is now, so
 * that a mistake is one keystroke away from being undone. Enter writes it, Escape
 * undoes it, and a click somewhere else does the same.
 */

export interface ParameterRowProps {
  /** The parameter the row draws. */
  readonly parameter: KnownParameter;
  /** Where the row is in the list, for the virtualiser. */
  readonly index: number;
  /** How far down the list the row is, in pixels. */
  readonly start: number;
  /** The virtualiser's ruler, which measures the row. */
  readonly measure: (node: Element | null) => void;
  /** Whether the value of this row is being edited. */
  readonly editing: boolean;
  /** Whether a write of this row is in flight. */
  readonly busy: boolean;
  /** Whether any write is in flight, which stops a second one. */
  readonly locked: boolean;
  /** Starts the edit. */
  readonly onEdit: () => void;
  /** Abandons the edit. */
  readonly onCancel: () => void;
  /** Writes the value. */
  readonly onWrite: (value: number) => void;
}

export function ParameterRow({
  parameter,
  index,
  start,
  measure,
  editing,
  busy,
  locked,
  onEdit,
  onCancel,
  onWrite,
}: ParameterRowProps) {
  const [draft, setDraft] = useState("");
  const [problem, setProblem] = useState<string | null>(null);
  const [wasEditing, setWasEditing] = useState(editing);
  const input = useRef<HTMLInputElement | null>(null);

  const typeName = paramTypeName(parameter.reportedType);
  const shown = formatParameterValue(parameter.value, parameter.reportedType);
  const integral = isIntegralType(parameter.reportedType);

  // The edit starts with the value the vehicle has now.
  //
  // The draft is filled in as the row begins to be edited, and not again: the list is
  // read again and again, and a value that arrives while the operator is typing must
  // not wipe out what they wrote.
  //
  // It is filled in while the row is drawn rather than in an effect, so that the input
  // holds the value before it is focused. An input whose value is set after it has been
  // focused puts the caret at the end, and the operator's first keystroke would be
  // added to the value instead of replacing it.
  if (editing !== wasEditing) {
    setWasEditing(editing);
    setProblem(null);

    if (editing) {
      setDraft(
        parameter.value === null
          ? ""
          : formatParameterValue(parameter.value, parameter.reportedType),
      );
    }
  }

  useEffect(() => {
    if (!editing) {
      return;
    }

    const element = input.current;
    element?.focus();
    element?.select();
  }, [editing]);

  const submit = () => {
    if (busy) {
      return;
    }

    const parsed = parseParameterValue(draft, parameter.reportedType);
    if (!parsed.ok) {
      setProblem(parsed.reason);
      return;
    }

    setProblem(null);
    onWrite(parsed.value);
  };

  return (
    <div
      ref={measure}
      data-index={index}
      className={styles.row}
      style={{ transform: `translateY(${start}px)` }}
      role="listitem"
    >
      <span className={styles.name} title={parameter.name}>
        {parameter.name}
      </span>

      {editing ? (
        <span className={styles.editor}>
          <input
            ref={input}
            className={styles.input}
            data-editor="true"
            value={draft}
            spellCheck={false}
            autoComplete="off"
            inputMode={integral ? "numeric" : "decimal"}
            aria-label={`The value of ${parameter.name}`}
            aria-invalid={problem !== null}
            disabled={busy}
            onChange={(event) => {
              setDraft(event.target.value);
              setProblem(null);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                submit();
              } else if (event.key === "Escape") {
                // The sheet listens for the escape key as well; this one belongs to the
                // edit, not to the sheet.
                event.preventDefault();
                event.stopPropagation();
                onCancel();
              }
            }}
          />
          <button
            type="button"
            className={styles.write}
            disabled={busy || locked}
            onClick={submit}
          >
            {busy ? "Writing…" : "Write"}
          </button>
          <button
            type="button"
            className={styles.abort}
            disabled={busy}
            onClick={onCancel}
          >
            Cancel
          </button>
        </span>
      ) : (
        <button
          type="button"
          className={styles.value}
          disabled={locked}
          title="Click to change the value"
          aria-label={`${parameter.name}, ${shown}. Change the value`}
          onClick={onEdit}
        >
          {shown}
        </button>
      )}

      <span className={styles.badgeCell}>
        <span className={styles.badge} title={typeName}>
          {typeName}
        </span>
      </span>

      {problem ? (
        <span className={styles.problem} role="alert">
          {parameter.name}: {problem}
        </span>
      ) : null}
    </div>
  );
}
