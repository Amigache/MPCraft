import { useVirtualizer } from "@tanstack/react-virtual";
import { useState } from "react";

import type { MetadataIndex } from "../../lib/params/metadataStore";
import type { KnownParameter } from "../../lib/params/paramsStore";
import { ParameterRow } from "./ParameterRow";
import styles from "./ParametersScreen.module.css";

/**
 * The list of parameters.
 *
 * A vehicle has thousands of them, so the list is virtualised: only the rows that are
 * on the screen are in the document. The rest of the list is drawn when it is scrolled
 * to.
 *
 * The favourites are the rows that carry a star: they are the ones the operator looks
 * for, and they are at the top of the list.
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
  /** The names of the favourites, in the order the operator marked them. */
  readonly favourites: readonly string[];
  /** What is known about the parameters, by name. */
  readonly metadata: MetadataIndex;
  /** Starts the edit of a row. */
  readonly onEdit: (name: string) => void;
  /** Abandons the edit. */
  readonly onCancel: () => void;
  /** Writes a value. */
  readonly onWrite: (name: string, value: number) => void;
  /** Marks a parameter as a favourite, or takes the mark away. */
  readonly onToggleFavourite: (name: string) => void;
}

export function ParameterList({
  rows,
  editing,
  writing,
  favourites,
  metadata,
  onEdit,
  onCancel,
  onWrite,
  onToggleFavourite,
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
              favourite={favourites.includes(parameter.name)}
              metadata={metadata.get(parameter.name)}
              onEdit={() => {
                onEdit(parameter.name);
              }}
              onCancel={onCancel}
              onWrite={(value) => {
                onWrite(parameter.name, value);
              }}
              onToggleFavourite={() => {
                onToggleFavourite(parameter.name);
              }}
            />
          );
        })}
      </div>
    </div>
  );
}
