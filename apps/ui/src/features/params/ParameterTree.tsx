import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent, ReactNode } from "react";

import {
  flattenTree,
  openPath,
  type ParamTreeNode,
} from "../../lib/params/paramTree";
import styles from "./ParameterTree.module.css";

/**
 * The tree of the parameters.
 *
 * Mission Planner's parameter window is a tree on the left and the grid on the right.
 * The tree groups the parameters by their common prefix, and clicking a branch shows the
 * parameters under it.
 *
 * The tree is walked with the keyboard: the arrow keys move from one node to the next,
 * the right key opens a branch, the left key closes it, and Enter shows the parameters
 * of the node the keyboard is on.
 *
 * The twisty of a branch is not a control of its own — the node is the control, and the
 * state of the branch is on the node — so that the tree adds no buttons to the sheet.
 */

export interface ParameterTreeProps {
  /** The tree of the parameters, as the names make it. */
  readonly tree: ParamTreeNode;
  /** The prefix of the node whose parameters are shown. */
  readonly selected: string;
  /** Shows the parameters of a node. */
  readonly onSelect: (prefix: string) => void;
}

export function ParameterTree({ tree, selected, onSelect }: ParameterTreeProps) {
  /** The branches that are open. */
  const [open, setOpen] = useState<ReadonlySet<string>>(() => new Set());
  /** The node the keyboard is on. */
  const [focused, setFocused] = useState(selected);
  /** Whether the keyboard is to be moved to the node it is on. */
  const moveFocus = useRef(false);
  /** The nodes of the tree, so that the keyboard can be moved to one of them. */
  const items = useRef(new Map<string, HTMLDivElement>());

  const visible = useMemo(() => flattenTree(tree, open), [tree, open]);

  // The branch that holds the selection is opened, so that the operator can see where
  // the selection is.
  useEffect(() => {
    const path = openPath(tree, selected);
    if (path.length === 0) {
      return;
    }

    setOpen((before) => {
      const next = new Set(before);
      for (const prefix of path) {
        next.add(prefix);
      }

      return next.size === before.size ? before : next;
    });
  }, [tree, selected]);

  // The keyboard follows the selection when the selection is made from outside.
  useEffect(() => {
    setFocused((before) => (before === selected ? before : selected));
  }, [selected]);

  // The keyboard is moved to the node it is on, but only when the operator asked for it:
  // the sheet gives the focus to the search box when it opens.
  useEffect(() => {
    if (!moveFocus.current) {
      return;
    }

    moveFocus.current = false;
    items.current.get(focused)?.focus();
  }, [focused]);

  const toggle = useCallback((prefix: string) => {
    setOpen((before) => {
      const next = new Set(before);
      if (next.has(prefix)) {
        next.delete(prefix);
      } else {
        next.add(prefix);
      }

      return next;
    });
  }, []);

  const focusOn = useCallback((prefix: string) => {
    moveFocus.current = true;
    setFocused(prefix);
  }, []);

  const onKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLDivElement>, node: ParamTreeNode) => {
      const index = visible.findIndex((entry) => entry.prefix === node.prefix);
      if (index < 0) {
        return;
      }

      const isOpen = open.has(node.prefix);

      switch (event.key) {
        case "ArrowDown": {
          event.preventDefault();
          const next = visible[index + 1];
          if (next !== undefined) {
            focusOn(next.prefix);
          }

          return;
        }

        case "ArrowUp": {
          event.preventDefault();
          const previous = visible[index - 1];
          if (previous !== undefined) {
            focusOn(previous.prefix);
          }

          return;
        }

        case "ArrowRight": {
          event.preventDefault();
          if (node.children.length > 0 && !isOpen) {
            toggle(node.prefix);
            return;
          }

          const child = node.children[0];
          if (child !== undefined) {
            focusOn(child.prefix);
          }

          return;
        }

        case "ArrowLeft": {
          event.preventDefault();
          if (node.children.length > 0 && isOpen) {
            toggle(node.prefix);
            return;
          }

          // The parent is the node above the branch this one belongs to.
          const parent = visible
            .slice(0, index)
            .reverse()
            .find((entry) => node.prefix.startsWith(`${entry.prefix}_`));

          if (parent !== undefined) {
            focusOn(parent.prefix);
          }

          return;
        }

        case "Home": {
          event.preventDefault();
          const first = visible[0];
          if (first !== undefined) {
            focusOn(first.prefix);
          }

          return;
        }

        case "End": {
          event.preventDefault();
          const last = visible[visible.length - 1];
          if (last !== undefined) {
            focusOn(last.prefix);
          }

          return;
        }

        case "Enter":
        case " ": {
          event.preventDefault();
          onSelect(node.prefix);
          return;
        }

        default:
          return;
      }
    },
    [focusOn, onSelect, open, toggle, visible],
  );

  const draw = (node: ParamTreeNode, depth: number): ReactNode => {
    const hasChildren = node.children.length > 0;
    // The root is always open: it holds every parameter.
    const isOpen = node.prefix === "" || open.has(node.prefix);
    const isSelected = node.prefix === selected;

    return (
      <div key={node.prefix} className={styles.group}>
        <div
          ref={(element) => {
            if (element === null) {
              items.current.delete(node.prefix);
            } else {
              items.current.set(node.prefix, element);
            }
          }}
          className={styles.item}
          style={{ paddingLeft: `${depth * 12 + 4}px` }}
          role="treeitem"
          aria-selected={isSelected}
          aria-expanded={hasChildren ? isOpen : undefined}
          tabIndex={node.prefix === focused ? 0 : -1}
          onClick={() => {
            onSelect(node.prefix);
            setFocused(node.prefix);
          }}
          onKeyDown={(event) => {
            onKeyDown(event, node);
          }}
        >
          {hasChildren && node.prefix !== "" ? (
            // The twisty is not a control of its own: the node is the control, and the
            // arrow keys open and close the branch. A button here would be a control
            // drawn on top of the window behind the sheet.
            <span
              className={`${styles.twisty} ${isOpen ? styles.twistyOpen : ""}`}
              aria-hidden="true"
              onClick={(event) => {
                event.stopPropagation();
                toggle(node.prefix);
              }}
            >
              <svg viewBox="0 0 12 12" focusable="false">
                <path d="M4 2 L9 6 L4 10" />
              </svg>
            </span>
          ) : (
            <span className={styles.twistySpace} aria-hidden="true" />
          )}

          <span className={styles.label}>{node.label}</span>
        </div>

        {hasChildren && isOpen ? (
          <div role="group">
            {node.children.map((child) => draw(child, depth + 1))}
          </div>
        ) : null}
      </div>
    );
  };

  return (
    <div className={styles.tree} role="tree" aria-label="The parameters by prefix">
      {draw(tree, 0)}
    </div>
  );
}
