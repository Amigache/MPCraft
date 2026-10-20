import { useEffect, useRef, useState } from "react";

import styles from "./ConfirmButton.module.css";

/**
 * A button that asks before it does.
 *
 * Arming, disarming, writing a plan and emptying a plan are the actions that hurt when
 * they are pressed by accident. The first press arms the button and the second carries
 * the action out; nothing happens by accident, and the destructive action is never the
 * button that is pressed first.
 */

export interface ConfirmButtonProps {
  /** What the button says before it is armed. */
  readonly label: string;
  /** What the armed button says. */
  readonly confirmLabel?: string | undefined;
  /** The tone of the button. */
  readonly variant?: "default" | "primary" | "danger";
  /** Whether the button is disabled. */
  readonly disabled?: boolean;
  /** Whether the button takes the whole width of its row. */
  readonly wide?: boolean;
  /** What the action does. */
  readonly onConfirm: () => void;
}

/** How long the button stays armed, in milliseconds. */
const ARM_TIMEOUT = 6000;

export function ConfirmButton({
  label,
  confirmLabel,
  variant = "default",
  disabled = false,
  wide = false,
  onConfirm,
}: ConfirmButtonProps) {
  const [armed, setArmed] = useState(false);
  const timer = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (timer.current !== null) {
        window.clearTimeout(timer.current);
      }
    },
    [],
  );

  const disarm = () => {
    if (timer.current !== null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
    setArmed(false);
  };

  if (!armed) {
    return (
      <button
        type="button"
        className={`button${variant === "primary" ? " button--primary" : ""}${
          variant === "danger" ? " button--danger" : ""
        }${wide ? " button--wide" : ""}`}
        disabled={disabled}
        onClick={() => {
          setArmed(true);
          timer.current = window.setTimeout(disarm, ARM_TIMEOUT);
        }}
      >
        {label}
      </button>
    );
  }

  return (
    <span className={styles.group}>
      <button
        type="button"
        className={`button${
          variant === "primary" ? " button--primary" : " button--danger"
        }`}
        onClick={() => {
          disarm();
          onConfirm();
        }}
      >
        {confirmLabel ?? `Confirm ${label}`}
      </button>
      <button type="button" className="button" onClick={disarm}>
        Cancel
      </button>
    </span>
  );
}
