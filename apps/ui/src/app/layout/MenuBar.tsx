import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";

import { ConnectionPanel } from "../../features/connection/ConnectionPanel";
import {
  armVehicle,
  closeLink,
  disarmVehicle,
} from "../../lib/flight/flightActions";
import { useModesStore } from "../../lib/flight/modesStore";
import { useLinkStore } from "../../lib/link/linkStore";
import { usePlanEditorStore } from "../../lib/plan/planEditorStore";
import { usePlanStore } from "../../lib/plan/planStore";
import { TABS, useUiStore } from "../../lib/ui/uiStore";
import styles from "./MenuBar.module.css";

/**
 * The menu bar.
 *
 * Every entry either does something or is visibly disabled: a menu that leads nowhere is
 * worse than no menu at all. The state of the link and the target it is open on are on
 * the right, where they can be read without opening anything.
 */

/** One entry of a menu. */
interface MenuItem {
  readonly id: string;
  readonly label: string;
  /** What the entry does. */
  readonly onSelect: (() => void) | undefined;
  /** Whether the entry is greyed out. */
  readonly disabled: boolean;
  /** Whether the entry carries a tick: `undefined` when it does not. */
  readonly checked: boolean | undefined;
  /** Whether the entry is a rule between two groups. */
  readonly separator: boolean;
  /** Why the entry is disabled, shown as a hint. */
  readonly hint: string;
}

/** One menu of the bar. */
interface Menu {
  readonly id: string;
  readonly label: string;
  readonly items: readonly MenuItem[];
}

/** The version shown in the About box. */
const VERSION = "0.1.0";

export function MenuBar() {
  const [open, setOpen] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [about, setAbout] = useState(false);
  const bar = useRef<HTMLDivElement | null>(null);

  const status = useLinkStore((state) => state.status);
  const vehicles = useLinkStore((state) => state.vehicles);
  const modes = useModesStore((state) => state.modes);
  const plans = usePlanStore((state) => state.plans);
  const setKind = usePlanStore((state) => state.setKind);
  const busy = usePlanStore((state) => state.busy);
  const download = usePlanStore((state) => state.download);
  const upload = usePlanStore((state) => state.upload);
  const clear = usePlanStore((state) => state.clear);

  const activeTab = useUiStore((state) => state.activeTab);
  const setActiveTab = useUiStore((state) => state.setActiveTab);
  const showLeftColumn = useUiStore((state) => state.showLeftColumn);
  const toggleLeftColumn = useUiStore((state) => state.toggleLeftColumn);
  const showGraphs = useUiStore((state) => state.showGraphs);
  const toggleGraphs = useUiStore((state) => state.toggleGraphs);
  const followVehicle = useUiStore((state) => state.followVehicle);
  const setFollowVehicle = useUiStore((state) => state.setFollowVehicle);
  const showMapControls = useUiStore((state) => state.showMapControls);
  const toggleMapControls = useUiStore((state) => state.toggleMapControls);
  const showParameters = useUiStore((state) => state.showParameters);

  const editorOpen = usePlanEditorStore((state) => state.open);
  const openEditor = usePlanEditorStore((state) => state.openEditor);

  const close = useCallback(() => setOpen(null), []);
  // A click anywhere else closes the menu.
  useEffect(() => {
    if (open === null) {
      return;
    }
    const onPointerDown = (event: PointerEvent) => {
      if (bar.current && !bar.current.contains(event.target as Node)) {
        close();
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [open, close]);

  // The escape key closes the menu and the boxes.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      if (about) {
        setAbout(false);
        return;
      }
      if (connecting) {
        setConnecting(false);
        return;
      }
      close();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [about, connecting, close]);

  const linked = status === "connected" || status === "reconnecting";
  const plan = plans.mission;

  const menus: readonly Menu[] = [
    {
      id: "file",
      label: "File",
      items: [
        {
          id: "connect",
          label: "Connect…",
          onSelect: () => {
            close();
            setConnecting(true);
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "disconnect",
          label: "Disconnect",
          onSelect: () => {
            close();
            void closeLink();
          },
          disabled: !linked,
          checked: undefined,
          separator: false,
          hint: linked ? "" : "no link",
        },
        {
          id: "file-sep",
          label: "",
          onSelect: undefined,
          disabled: false,
          checked: undefined,
          separator: true,
          hint: "",
        },
        {
          id: "exit",
          label: "Exit",
          onSelect: () => {
            close();
            void getCurrentWindow()
              .close()
              .catch(() => undefined);
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
      ],
    },
    {
      id: "flight",
      label: "Flight",
      items: [
        {
          id: "arm",
          label: "Arm",
          onSelect: () => {
            close();
            void armVehicle();
          },
          disabled: !linked,
          checked: undefined,
          separator: false,
          hint: linked ? "" : "no link",
        },
        {
          id: "disarm",
          label: "Disarm",
          onSelect: () => {
            close();
            void disarmVehicle();
          },
          disabled: !linked,
          checked: undefined,
          separator: false,
          hint: linked ? "" : "no link",
        },
        {
          id: "flight-sep-1",
          label: "",
          onSelect: undefined,
          disabled: false,
          checked: undefined,
          separator: true,
          hint: "",
        },
        {
          id: "set-mode",
          label: "Set mode…",
          onSelect: () => {
            close();
            setActiveTab("actions");
          },
          disabled: modes.length === 0,
          checked: undefined,
          separator: false,
          hint: modes.length === 0 ? "no modes" : "",
        },
        {
          id: "flight-sep-2",
          label: "",
          onSelect: undefined,
          disabled: false,
          checked: undefined,
          separator: true,
          hint: "",
        },
        {
          id: "download",
          label: "Download plan",
          onSelect: () => {
            close();
            void download("mission");
          },
          disabled: !linked || busy,
          checked: undefined,
          separator: false,
          hint: linked ? "" : "no link",
        },
        {
          id: "upload",
          label: "Upload plan",
          onSelect: () => {
            close();
            void upload("mission", plan?.items ?? []);
          },
          disabled: !linked || busy || !plan || plan.items.length === 0,
          checked: undefined,
          separator: false,
          hint: plan ? "" : "no plan",
        },
        {
          id: "clear",
          label: "Clear plan",
          onSelect: () => {
            close();
            void clear("mission");
          },
          disabled: !linked || busy,
          checked: undefined,
          separator: false,
          hint: linked ? "" : "no link",
        },
      ],
    },
    {
      id: "view",
      label: "View",
      items: [
        {
          id: "side-panel",
          label: "Side panel",
          onSelect: () => {
            close();
            toggleLeftColumn();
          },
          disabled: false,
          checked: showLeftColumn,
          separator: false,
          hint: "",
        },
        {
          id: "graphs",
          label: "Graphs",
          onSelect: () => {
            close();
            toggleGraphs();
          },
          disabled: false,
          // The graphs are put away while the plan is being edited, so the tick says
          // what is on the screen.
          checked: showGraphs && !editorOpen,
          separator: false,
          hint: "",
        },
        {
          id: "follow",
          label: "Follow the vehicle",
          onSelect: () => {
            close();
            setFollowVehicle(!followVehicle);
          },
          disabled: false,
          checked: followVehicle,
          separator: false,
          hint: "",
        },
        {
          id: "map-controls",
          label: "Map controls",
          onSelect: () => {
            close();
            toggleMapControls();
          },
          disabled: false,
          checked: showMapControls,
          separator: false,
          hint: "",
        },
        {
          id: "view-sep",
          label: "",
          onSelect: undefined,
          disabled: false,
          checked: undefined,
          separator: true,
          hint: "",
        },
        ...TABS.map((tab) => ({
          id: `tab-${tab.id}`,
          label: tab.label,
          onSelect: () => {
            close();
            setActiveTab(tab.id);
          },
          disabled: false,
          checked: activeTab === tab.id,
          separator: false,
          hint: "",
        })),
      ],
    },
    {
      id: "setup",
      label: "Setup",
      items: [
        {
          id: "modes",
          label: "Flight modes",
          onSelect: () => {
            close();
            setActiveTab("actions");
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-sep",
          label: "",
          onSelect: undefined,
          disabled: false,
          checked: undefined,
          separator: true,
          hint: "",
        },
        {
          id: "parameters",
          label: "Parameters…",
          onSelect: () => {
            close();
            showParameters();
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "mission-planner",
          label: "Mission planner",
          onSelect: () => {
            close();
            setActiveTab("actions");
            setKind("mission");
            openEditor();
          },
          disabled: false,
          checked: editorOpen,
          separator: false,
          hint: "",
        },
        {
          id: "firmware",
          label: "Firmware…",
          onSelect: undefined,
          disabled: true,
          checked: undefined,
          separator: false,
          hint: "not in this build",
        },
      ],
    },
    {
      id: "help",
      label: "Help",
      items: [
        {
          id: "about",
          label: "About MPCraft",
          onSelect: () => {
            close();
            setAbout(true);
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
      ],
    },
  ];

  const move = (target: EventTarget | null, by: number) => {
    const buttons = Array.from(
      bar.current?.querySelectorAll<HTMLButtonElement>("[data-menu]") ?? [],
    );
    if (buttons.length === 0) {
      return;
    }
    const current = buttons.findIndex((button) => button === target);
    const next = buttons[(current + by + buttons.length) % buttons.length];
    next?.focus();
  };

  return (
    <div className={styles.bar} role="menubar" aria-label="Main menu" ref={bar}>
      {menus.map((menu) => (
        <div className={styles.menu} key={menu.id}>
          <button
            type="button"
            role="menuitem"
            data-menu={menu.id}
            aria-haspopup="menu"
            aria-expanded={open === menu.id}
            className={open === menu.id ? styles.menuOpen : styles.menuButton}
            onClick={() => setOpen(open === menu.id ? null : menu.id)}
            onKeyDown={(event) => {
              if (event.key === "ArrowDown" || event.key === "ArrowRight") {
                event.preventDefault();
                setOpen(menu.id);
                move(event.target, 1);
              } else if (event.key === "ArrowLeft") {
                event.preventDefault();
                move(event.target, -1);
              } else if (event.key === "Escape") {
                event.preventDefault();
                close();
              }
            }}
          >
            {menu.label}
          </button>

          {open === menu.id ? (
            <div
              className={styles.dropdown}
              role="menu"
              aria-label={menu.label}
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
                }
              }}
            >
              {menu.items.map((item) =>
                item.separator ? (
                  <div className={styles.separator} key={item.id} role="separator" />
                ) : (
                  <button
                    type="button"
                    role={
                      item.checked === undefined
                        ? "menuitem"
                        : "menuitemcheckbox"
                    }
                    aria-checked={item.checked}
                    aria-disabled={item.disabled}
                    key={item.id}
                    className={styles.item}
                    disabled={item.disabled}
                    title={item.hint || undefined}
                    onClick={() => {
                      if (item.onSelect) {
                        item.onSelect();
                      }
                    }}
                  >
                    <span className={styles.check}>
                      {item.checked ? "✓" : ""}
                    </span>
                    <span className={styles.itemLabel}>{item.label}</span>
                    {item.hint ? (
                      <span className={styles.hint}>{item.hint}</span>
                    ) : null}
                  </button>
                ),
              )}
            </div>
          ) : null}
        </div>
      ))}

      <div className={styles.right}>
        <span className={styles.target} title="The link the application is opened on">
          {vehicles.length > 0
            ? `${vehicles[0]?.systemId ?? "?"}/${vehicles[0]?.componentId ?? "?"}`
            : "no vehicle"}
        </span>
        <span
          className={`${styles.state} ${styles[status] ?? ""}`}
          title="The state of the link"
        >
          {status}
        </span>
      </div>

      {connecting ? (
        <div
          className={styles.overlay}
          onClick={() => setConnecting(false)}
          role="presentation"
        >
          <div
            className={styles.dialog}
            role="dialog"
            aria-modal="true"
            aria-label="Connection"
            onClick={(event) => event.stopPropagation()}
          >
            <div className={styles.dialogHead}>
              <h2 className={styles.dialogTitle}>Connection</h2>
              <button
                type="button"
                className="button"
                onClick={() => setConnecting(false)}
              >
                Close
              </button>
            </div>
            <ConnectionPanel />
          </div>
        </div>
      ) : null}

      {about ? (
        <div
          className={styles.overlay}
          onClick={() => setAbout(false)}
          role="presentation"
        >
          <div
            className={styles.dialog}
            role="dialog"
            aria-modal="true"
            aria-label="About MPCraft"
            onClick={(event) => event.stopPropagation()}
          >
            <div className={styles.dialogHead}>
              <h2 className={styles.dialogTitle}>About MPCraft</h2>
              <button
                type="button"
                className="button"
                onClick={() => setAbout(false)}
              >
                Close
              </button>
            </div>
            <p className={styles.about}>
              MPCraft {VERSION} — a ground station for ArduPilot and PX4, written in Rust
              and React.
            </p>
            <p className={styles.about}>
              The map uses the demo tiles of MapLibre and needs no account.
            </p>
          </div>
        </div>
      ) : null}
    </div>
  );
}
