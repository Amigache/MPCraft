import { useCallback, useRef } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";

import styles from "./Splitter.module.css";

/**
 * The edge between two panels.
 *
 * It is dragged with the pointer and moved with the arrow keys, so the interface can be
 * arranged without a mouse.
 */
export interface SplitterProps {
  /** Which way the edge runs. */
  readonly orientation: "vertical" | "horizontal";
  /** What the edge is called, for the screen reader. */
  readonly label: string;
  /** The size of the panel the edge controls. */
  readonly value: number;
  /** The smallest the panel may be. */
  readonly min: number;
  /** The largest the panel may be. */
  readonly max: number;
  /** How far a key press moves the edge. */
  readonly step?: number;
  /** Called with the new size. */
  readonly onChange: (value: number) => void;
}

export function Splitter({
  orientation,
  label,
  value,
  min,
  max,
  step = 16,
  onChange,
}: SplitterProps) {
  const origin = useRef<{ x: number; y: number } | null>(null);

  const onPointerDown = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      event.preventDefault();
      origin.current = { x: event.clientX, y: event.clientY };

      const move = (moveEvent: PointerEvent) => {
        const start = origin.current;
        if (!start) {
          return;
        }
        const delta =
          orientation === "vertical"
            ? moveEvent.clientX - start.x
            : moveEvent.clientY - start.y;
        onChange(Math.min(max, Math.max(min, value + delta)));
      };

      const up = () => {
        origin.current = null;
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
      };

      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
    },
    [max, min, onChange, orientation, value],
  );

  return (
    <div
      className={
        orientation === "vertical" ? styles.vertical : styles.horizontal
      }
      role="separator"
      aria-orientation={orientation}
      aria-label={label}
      aria-valuenow={Math.round(value)}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onKeyDown={(event) => {
        const forward =
          orientation === "vertical" ? "ArrowRight" : "ArrowDown";
        const backward =
          orientation === "vertical" ? "ArrowLeft" : "ArrowUp";

        if (event.key === forward) {
          event.preventDefault();
          onChange(Math.min(max, value + step));
        } else if (event.key === backward) {
          event.preventDefault();
          onChange(Math.max(min, value - step));
        } else if (event.key === "Home") {
          event.preventDefault();
          onChange(min);
        } else if (event.key === "End") {
          event.preventDefault();
          onChange(max);
        }
      }}
    />
  );
}
