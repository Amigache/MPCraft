import { useVirtualizer } from "@tanstack/react-virtual";
import { useState } from "react";

import type { KnownParameter } from "../../lib/params/paramsStore";
import { ParameterRow } from "./ParameterRow";
import styles from "./ParameterSheet.module.css";

/**
 * The list of parameters.
 *
 * A vehicle has thousands of them, so the list is virtualised: only the rows that are
 * on the screen are in the document. The rest of the list is drawn when it is scrolled
 * to.
 */

/** How tall a row is, in pixels, when nothing makes it taller. */
const ROW_HEIGHT = 28;

export interface ParameterListProps {
  /** The parameters to show, already filtered. */
  readonly rows: readonly KnownParameter[];
  /** The name of the row being edited, when one is. */
  readonly editing: string | null;
  /** The name of the parameter being written, when one is. */
  readonly writing: string | null;
  /** Starts the edit of a row. */
  readonly onEdit: (name: string) => void;
  /** Abandons the edit. */
  readonly onCancel: () => void;
  /** Writes a value. */
  readonly onWrite: (name: string, value: number) => void;
}

export function ParameterList({
  rows,
  editing,
  writing,
  onEdit,
  onCancel,
  onWrite,
}: ParameterListProps) {
  // The element the list scrolls in is held in the state, so that the virtualiser is
  // told about it when it appears rather than a render later.
  const [scroller, setScroller] = useState<HTMLDivElement | null>(null);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scroller,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });

  return (
    <div
      ref={setScroller}
      className={styles.list}
      role="list"
      aria-label="The parameters the vehicle has reported"
    >
      <div className={styles.canvas} style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map((item) => {
          const parameter = rows[item.index];
          if (parameter === undefined) {
            return null;
          }

          return (
            <ParameterRow
              key={parameter.name}
              parameter={parameter}
              index={item.index}
              start={item.start}
              measure={virtualizer.measureElement}
              editing={editing === parameter.name}
              busy={writing === parameter.name}
              locked={writing !== null}
              onEdit={() => {
                onEdit(parameter.name);
              }}
              onCancel={onCancel}
              onWrite={(value) => {
                onWrite(parameter.name, value);
              }}
            />
          );
        })}
      </div>
    </div>
  );
}
