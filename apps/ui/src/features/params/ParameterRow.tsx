import { useEffect, useRef, useState } from "react";

import type { ParameterMetadata } from "../../lib/ipc/types";
import { isOutOfRange } from "../../lib/params/metadataStore";
import type { KnownParameter } from "../../lib/params/paramsStore";
import {
  formatParameterValue,
  parseParameterValue,
} from "../../lib/params/paramValues";
import { isIntegralType, paramTypeName } from "../../lib/params/paramTypes";
import styles from "./ParametersScreen.module.css";

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
 *
 * The star at the front of the row marks the parameter as one of the favourites. The
 * favourites are the ones the operator looks for, and they are kept between two runs.
 */

export interface ParameterRowProps {
  /** The parameter the row draws. */
  readonly parameter: KnownParameter;
  /** What is known about the parameter, when the documentation knows it. */
  readonly metadata: ParameterMetadata | undefined;
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
  /** Whether the parameter is one of the favourites. */
  readonly favourite: boolean;
  /** Starts the edit. */
  readonly onEdit: () => void;
  /** Abandons the edit. */
  readonly onCancel: () => void;
  /** Writes the value. */
  readonly onWrite: (value: number) => void;
  /** Marks the parameter as a favourite, or takes the mark away. */
  readonly onToggleFavourite: () => void;
}

/**
 * What the row says about the parameter when the pointer rests on it.
 *
 * The description, the value it had before anybody changed it, the range it takes and
 * the group it belongs to are the autopilots' knowledge, and they are what an operator
 * reads when they do not know what a parameter does.
 */
function describe(
  metadata: ParameterMetadata | undefined,
  parameter: KnownParameter,
): string {
  if (metadata === undefined) {
    return parameter.name;
  }

  const parts: string[] = [];

  if (metadata.description !== "") {
    parts.push(metadata.description);
  }

  if (metadata.default !== "") {
    parts.push(`Default: ${metadata.default}`);
  }

  if (metadata.minimum !== "" || metadata.maximum !== "") {
    parts.push(`Range: ${metadata.minimum} to ${metadata.maximum}`);
  }

  if (metadata.group !== "") {
    parts.push(`Group: ${metadata.group}`);
  }

  if (metadata.rebootRequired) {
    parts.push("A change to it needs the vehicle to be restarted");
  }

  return parts.length > 0 ? parts.join(" · ") : parameter.name;
}

/**
 * The choices a parameter can be given, with the value it has now among them.
 *
 * A parameter that is a choice is drawn as a list of what it can be, and the value it
 * has is the one that is chosen. When the vehicle holds a value the documentation does
 * not know — a value a newer autopilot has — the value is kept as a choice of its own,
 * so that the row says what the vehicle has instead of pretending it does not.
 */
function choicesFor(
  metadata: ParameterMetadata | undefined,
  value: number | null,
): readonly { code: number; text: string }[] {
  const choices = metadata?.values ?? [];

  if (value === null || choices.length === 0) {
    return choices;
  }

  if (choices.some((choice) => choice.code === value)) {
    return choices;
  }

  return [...choices, { code: value, text: String(value) }];
}

export function ParameterRow({
  parameter,
  metadata,
  index,
  start,
  measure,
  editing,
  busy,
  locked,
  favourite,
  onEdit,
  onCancel,
  onWrite,
  onToggleFavourite,
}: ParameterRowProps) {
  const [draft, setDraft] = useState("");
  const [problem, setProblem] = useState<string | null>(null);
  const [wasEditing, setWasEditing] = useState(editing);
  const input = useRef<HTMLInputElement | null>(null);

  const typeName = paramTypeName(parameter.reportedType);
  const shown = formatParameterValue(parameter.value, parameter.reportedType);
  const integral = isIntegralType(parameter.reportedType);
  const outOfRange = isOutOfRange(metadata, parameter.value);

  const choices = choicesFor(metadata, parameter.value);
  const flags = metadata?.bitmask ?? [];

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
      <button
        type="button"
        className={`${styles.star} ${favourite ? styles.starOn : ""}`}
        aria-pressed={favourite}
        aria-label={
          favourite
            ? `Take ${parameter.name} out of the favourites`
            : `Make ${parameter.name} a favourite`
        }
        title={
          favourite
            ? "Not a favourite any more"
            : "A favourite — kept between two runs"
        }
        onClick={onToggleFavourite}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true" focusable="false">
          <path d="M8 1.4 L9.9 5.5 L14.4 6.1 L11.1 9.2 L12 13.6 L8 11.5 L4 13.6 L4.9 9.2 L1.6 6.1 L6.1 5.5 Z" />
        </svg>
      </button>

      <span className={styles.name} title={describe(metadata, parameter)}>
        {parameter.name}
      </span>

      {editing ? (
        <span className={styles.editor}>
          {flags.length > 0 ? (
            <>
              <span className={styles.flags}>
                {flags.map((flag) => (
                  <label key={flag.mask} className={styles.flag}>
                    <input
                      type="checkbox"
                      checked={((parameter.value ?? 0) & flag.mask) !== 0}
                      disabled={busy || locked}
                      aria-label={`${flag.text} of ${parameter.name}`}
                      onChange={(event) => {
                        const value = parameter.value ?? 0;

                        onWrite(
                          event.target.checked
                            ? value | flag.mask
                            : value & ~flag.mask,
                        );
                      }}
                    />
                    <span className={styles.flagText}>{flag.text}</span>
                  </label>
                ))}
              </span>

              <button
                type="button"
                className={styles.abort}
                disabled={busy}
                onClick={onCancel}
              >
                Done
              </button>
            </>
          ) : (
            <>
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
                    // The sheet listens for the escape key as well; this one belongs to
                    // the edit, not to the sheet.
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
            </>
          )}
        </span>
      ) : (
        <span className={styles.valueCell}>
          {choices.length > 0 ? (
            <select
              className="select"
              value={String(parameter.value ?? "")}
              disabled={locked}
              title={
                outOfRange
                  ? "The value is outside the range the documentation gives"
                  : "Choose one of the values it can be given"
              }
              aria-label={`${parameter.name}, ${shown}. Change the value`}
              onChange={(event) => {
                onWrite(Number(event.target.value));
              }}
            >
              {choices.map((choice) => (
                <option key={choice.code} value={choice.code}>
                  {choice.text}
                </option>
              ))}
            </select>
          ) : (
            <button
              type="button"
              className={`${styles.value} ${outOfRange ? styles.outOfRange : ""}`}
              disabled={locked}
              title={
                outOfRange
                  ? "The value is outside the range the documentation gives"
                  : "Click to change the value"
              }
              aria-label={`${parameter.name}, ${shown}. Change the value`}
              onClick={onEdit}
            >
              {shown}
            </button>
          )}
          {metadata !== undefined && metadata.unit !== "" ? (
            <span className={styles.unit}>{metadata.unit}</span>
          ) : null}
        </span>
      )}

      <span className={styles.badgeCell}>
        <span className={styles.badge} title={typeName}>
          {typeName}
        </span>
        {metadata !== undefined && metadata.rebootRequired ? (
          <span
            className={styles.badge}
            title="A change to this parameter needs the vehicle to be restarted to take effect"
          >
            Reboot
          </span>
        ) : null}
      </span>

      {problem ? (
        <span className={styles.problem} role="alert">
          {parameter.name}: {problem}
        </span>
      ) : null}
    </div>
  );
}
