import { useRef } from "react";

import styles from "./TabStrip.module.css";

/**
 * The tab strip of the left column.
 *
 * It behaves as a tab list: the arrow keys move between the tabs, the active tab is the
 * only one in the tab order, and the panel below is labelled by the tab that opened it.
 */
export interface TabStripProps<T extends string> {
  /** The tabs, in the order they are shown. */
  readonly tabs: readonly { readonly id: T; readonly label: string }[];
  /** The tab that is open. */
  readonly active: T;
  /** Opens a tab. */
  readonly onSelect: (id: T) => void;
}

export function TabStrip<T extends string>({
  tabs,
  active,
  onSelect,
}: TabStripProps<T>) {
  const list = useRef<HTMLDivElement | null>(null);

  const move = (from: number, by: number) => {
    if (tabs.length === 0) {
      return;
    }
    const next = (from + by + tabs.length) % tabs.length;
    const tab = tabs[next];
    if (!tab) {
      return;
    }
    onSelect(tab.id);
    list.current
      ?.querySelector<HTMLButtonElement>(`[data-tab="${tab.id}"]`)
      ?.focus();
  };

  return (
    <div className={styles.strip} role="tablist" ref={list}>
      {tabs.map((tab, index) => {
        const selected = tab.id === active;
        return (
          <button
            key={tab.id}
            type="button"
            role="tab"
            data-tab={tab.id}
            id={`tab-${tab.id}`}
            aria-selected={selected}
            aria-controls={`panel-${tab.id}`}
            tabIndex={selected ? 0 : -1}
            className={selected ? styles.tabActive : styles.tab}
            onClick={() => onSelect(tab.id)}
            onKeyDown={(event) => {
              if (event.key === "ArrowRight") {
                event.preventDefault();
                move(index, 1);
              } else if (event.key === "ArrowLeft") {
                event.preventDefault();
                move(index, -1);
              } else if (event.key === "Home") {
                event.preventDefault();
                move(0, 0);
              } else if (event.key === "End") {
                event.preventDefault();
                move(tabs.length - 1, 0);
              }
            }}
          >
            {tab.label}
          </button>
        );
      })}
    </div>
  );
}
