import { useEffect, useRef } from "react";

import type { Telemetry } from "../lib/ipc/types";
import { liveTelemetry } from "../lib/link/liveTelemetry";

/**
 * A value that follows the telemetry without re-rendering.
 *
 * The text of the element is written directly when a snapshot arrives, so a number that
 * changes twenty times a second costs one text assignment rather than a reconciliation
 * of the tree. The element is given the width of its content by the surrounding layout,
 * so the number does not move the page when it changes.
 */

/** What a value may be. */
export type LiveValueSource = number | string | boolean | null | undefined;

export interface LiveValueProps {
  /** Picks the value out of a snapshot. */
  readonly select: (telemetry: Telemetry) => LiveValueSource;
  /** Renders the value. The default is the value itself. */
  readonly format?: ((value: number | string | boolean) => string) | undefined;
  /** The class of the element. */
  readonly className?: string | undefined;
  /** The title of the element, for a value that cannot be read in full. */
  readonly title?: string | undefined;
}

export function LiveValue({
  select,
  format,
  className,
  title,
}: LiveValueProps) {
  const node = useRef<HTMLSpanElement | null>(null);
  const selectRef = useRef(select);
  const formatRef = useRef(format);

  selectRef.current = select;
  formatRef.current = format;

  useEffect(() => {
    const write = (telemetry: Telemetry) => {
      const element = node.current;
      if (!element) {
        return;
      }

      const value = selectRef.current(telemetry);
      const text =
        value === null || value === undefined
          ? "—"
          : formatRef.current
            ? formatRef.current(value)
            : String(value);

      if (element.textContent !== text) {
        element.textContent = text;
      }
    };

    write(liveTelemetry.value);

    return liveTelemetry.subscribe(write);
  }, []);

  return (
    <span className={className} title={title} ref={node}>
      —
    </span>
  );
}
