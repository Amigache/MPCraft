import { useEffect, useState } from "react";

import styles from "./PlanNumberCell.module.css";

/**
 * A value of a plan item that can be changed in place.
 *
 * The value is shown as it is, and it is written into the plan when the field is left —
 * not on every keystroke, because a half-typed coordinate is not a coordinate. A value
 * that cannot be read is refused and the old one is put back: a plan that carries
 * nonsense is a plan the vehicle will refuse to fly.
 */

export interface PlanNumberCellProps {
  /** The value the item holds. */
  readonly value: number;
  /** How the value is shown when it is not being changed. */
  readonly format: (value: number) => string;
  /** What the field is called, for the screen reader. */
  readonly label: string;
  /** Whether the value can be changed. */
  readonly disabled?: boolean;
  /** Called with the new value when the field is left. */
  readonly onCommit: (value: number) => void;
}

export function PlanNumberCell({
  value,
  format,
  label,
  disabled = false,
  onCommit,
}: PlanNumberCellProps) {
  const [text, setText] = useState(() => format(value));
  const [editing, setEditing] = useState(false);

  // The value of the item changes under the field when the plan is changed elsewhere —
  // a marker dragged on the map, for instance. The text follows, unless the operator is
  // in the middle of typing.
  useEffect(() => {
    if (!editing) {
      setText(format(value));
    }
  }, [editing, format, value]);

  const commit = (raw: string) => {
    const parsed = Number.parseFloat(raw.replace(",", "."));
    if (Number.isFinite(parsed)) {
      onCommit(parsed);
      return;
    }
    setText(format(value));
  };

  return (
    <input
      className={styles.input}
      type="text"
      inputMode="decimal"
      spellCheck={false}
      autoComplete="off"
      aria-label={label}
      title={label}
      disabled={disabled}
      value={text}
      onChange={(event) => {
        setText(event.target.value);
      }}
      onFocus={() => {
        setEditing(true);
      }}
      onBlur={(event) => {
        setEditing(false);
        commit(event.target.value);
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          event.currentTarget.blur();
        } else if (event.key === "Escape") {
          event.preventDefault();
          setText(format(value));
          event.currentTarget.blur();
        }
      }}
    />
  );
}
