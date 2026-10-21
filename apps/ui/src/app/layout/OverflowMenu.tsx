import { useEffect, useId, useRef, useState } from "react";

import styles from "./OverflowMenu.module.css";

/**
 * A menu of the tools that do not fit on the bar.
 *
 * A bar of tools that wraps is a bar of tools that is read twice, and on a narrow
 * window the last button ends up alone on a row of its own. The tools that are not
 * used every minute go into this menu instead, and the menu says what is in it.
 */

/** How wide the menu is taken to be, to know which side it opens on. */
const MENU_WIDTH = 200;

export interface OverflowItem {
  /** What tells the item apart from the others. */
  readonly id: string;
  /** What the item says. */
  readonly label: string;
  /** What the item says when the mouse is over it. */
  readonly hint?: string;
  /** Whether the item can be used now. */
  readonly disabled?: boolean;
  /** What the item does. */
  readonly onSelect: () => void;
}

export interface OverflowMenuProps {
  /** What the button says. */
  readonly label?: string;
  /** The items of the menu. */
  readonly items: readonly OverflowItem[];
}

export function OverflowMenu({ label = "More", items }: OverflowMenuProps) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [align, setAlign] = useState<"start" | "end">("start");
  const root = useRef<HTMLDivElement | null>(null);
  const button = useRef<HTMLButtonElement | null>(null);

  // The menu opens towards the side that has room: a menu that hangs out of the
  // window is a menu whose last item cannot be reached.
  const toggle = () => {
    if (open) {
      setOpen(false);
      return;
    }

    const box = button.current?.getBoundingClientRect();

    setAlign(
      box !== undefined && box.left + MENU_WIDTH > window.innerWidth - 8
        ? "end"
        : "start",
    );
    setOpen(true);
  };

  // The menu closes when the click lands outside of it, or when the window loses the
  // focus: a menu that stays open behind another window is a menu nobody can close.
  useEffect(() => {
    if (!open) {
      return;
    }

    const onPointerDown = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };

    const onBlur = () => {
      setOpen(false);
    };

    document.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("blur", onBlur);

    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("blur", onBlur);
    };
  }, [open]);

  return (
    <div className={styles.root} ref={root}>
      <button
        type="button"
        ref={button}
        className="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={id}
        onClick={toggle}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" && !open) {
            event.preventDefault();
            toggle();
          }
        }}
      >
        {label} ▾
      </button>

      {open ? (
        <div
          className={`${styles.menu} ${align === "end" ? styles.menuEnd : ""}`}
          role="menu"
          id={id}
          aria-label={label}
          onKeyDown={(event) => {
            const entries = Array.from(
              event.currentTarget.querySelectorAll<HTMLButtonElement>(
                "[role='menuitem']",
              ),
            );
            const index = entries.findIndex((entry) => entry === event.target);

            if (event.key === "ArrowDown") {
              event.preventDefault();
              entries[(index + 1) % entries.length]?.focus();
            } else if (event.key === "ArrowUp") {
              event.preventDefault();
              entries[(index - 1 + entries.length) % entries.length]?.focus();
            } else if (event.key === "Home") {
              event.preventDefault();
              entries[0]?.focus();
            } else if (event.key === "End") {
              event.preventDefault();
              entries[entries.length - 1]?.focus();
            } else if (event.key === "Escape") {
              event.preventDefault();
              setOpen(false);
              button.current?.focus();
            }
          }}
        >
          {items.map((item) => (
            <button
              key={item.id}
              type="button"
              role="menuitem"
              className={styles.item}
              disabled={item.disabled}
              title={item.hint}
              onClick={() => {
                setOpen(false);
                item.onSelect();
              }}
            >
              {item.label}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
