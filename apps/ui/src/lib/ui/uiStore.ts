import { create } from "zustand";

/**
 * The state of the interface itself: which tab is open, how wide the panels are and what
 * is switched on.
 *
 * It is a store rather than React state in the shell because the menu bar, the tab strip
 * and the map all read it, and a change in one of them must not re-render the others.
 */

/** The tabs of the left column, in the order upstream has them. */
export type TabId =
  | "quick"
  | "actions"
  | "messages"
  | "gauges"
  | "status"
  | "preflight";

/** The tabs, with the names they are given. */
export const TABS: readonly { id: TabId; label: string }[] = [
  { id: "quick", label: "Quick" },
  { id: "actions", label: "Actions" },
  { id: "messages", label: "Messages" },
  { id: "gauges", label: "Gauges" },
  { id: "status", label: "Status" },
  { id: "preflight", label: "Pre-flight" },
];

/** The width of the left column when nothing has been changed. */
export const DEFAULT_LEFT_WIDTH = 320;

/** The narrowest the left column may be dragged. */
export const MIN_LEFT_WIDTH = 260;

/** The widest the left column may be dragged. */
export const MAX_LEFT_WIDTH = 520;

/** The height of the attitude indicator when nothing has been changed. */
export const DEFAULT_HUD_HEIGHT = 216;

/** The narrowest the attitude indicator may be dragged. */
export const MIN_HUD_HEIGHT = 140;

/** The tallest the attitude indicator may be dragged. */
export const MAX_HUD_HEIGHT = 420;

/** The height of the graphs when nothing has been changed. */
export const DEFAULT_GRAPH_HEIGHT = 240;

/** The narrowest the graphs may be dragged. */
export const MIN_GRAPH_HEIGHT = 100;

/**
 * The narrowest the plan editor may be dragged.
 *
 * The editor holds a grid of the items of a plan and a row of buttons: less than this
 * and there is nothing to read.
 */
export const MIN_PLAN_HEIGHT = 220;

/** The tallest the graphs may be dragged. */
export const MAX_GRAPH_HEIGHT = 720;

/** The narrowest the map may be left when the graphs are above it. */
export const MIN_MAP_HEIGHT = 160;

/** Reads a stored number, ignoring anything that is not one. */
function readNumber(key: string, fallback: number): number {
  if (typeof window === "undefined") {
    return fallback;
  }
  try {
    const raw = window.localStorage.getItem(key);
    if (raw === null) {
      return fallback;
    }
    const value = Number.parseFloat(raw);
    return Number.isFinite(value) ? value : fallback;
  } catch {
    return fallback;
  }
}

/** Stores a number, ignoring a storage that refuses. */
function writeNumber(key: string, value: number): void {
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.setItem(key, String(value));
  } catch {
    // A storage that refuses is not a reason to break the interface.
  }
}

/** Reads a stored flag. */
function readFlag(key: string, fallback: boolean): boolean {
  if (typeof window === "undefined") {
    return fallback;
  }
  try {
    const raw = window.localStorage.getItem(key);
    if (raw === null) {
      return fallback;
    }
    return raw === "1";
  } catch {
    return fallback;
  }
}

/** Stores a flag. */
function writeFlag(key: string, value: boolean): void {
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.setItem(key, value ? "1" : "0");
  } catch {
    // A storage that refuses is not a reason to break the interface.
  }
}

/** Reads a stored string, ignoring anything that is not one. */
function readString(key: string, fallback: string): string {
  if (typeof window === "undefined") {
    return fallback;
  }
  try {
    const raw = window.localStorage.getItem(key);
    return raw === null ? fallback : raw;
  } catch {
    return fallback;
  }
}

/** Stores a string, ignoring a storage that refuses. */
function writeString(key: string, value: string): void {
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // A storage that refuses is not a reason to break the interface.
  }
}

export interface UiStore {
  /** The tab that is open. */
  readonly activeTab: TabId;
  /** The width of the left column, in pixels. */
  readonly leftColumnWidth: number;
  /** The height of the attitude indicator, in pixels. */
  readonly hudHeight: number;
  /** The height of the graphs, in pixels. */
  readonly graphHeight: number;
  /** Whether the left column is shown at all. */
  readonly showLeftColumn: boolean;
  /** Whether the graphs are shown at all. */
  readonly showGraphs: boolean;
  /** The graph that is drawn, by its full name. */
  readonly graphPreset: string;
  /** Whether the map follows the vehicle. */
  readonly followVehicle: boolean;
  /** Whether the map's own controls are shown. */
  readonly showMapControls: boolean;
  /** Whether the sheet of parameters is open over the window. */
  readonly parametersOpen: boolean;
  /** Opens a tab. */
  readonly setActiveTab: (tab: TabId) => void;
  /** Moves the edge of the left column. */
  readonly setLeftColumnWidth: (width: number) => void;
  /** Moves the edge of the attitude indicator. */
  readonly setHudHeight: (height: number) => void;
  /** Moves the edge of the graphs. */
  readonly setGraphHeight: (height: number) => void;
  /** Shows or hides the left column. */
  readonly toggleLeftColumn: () => void;
  /** Shows or hides the graphs. */
  readonly toggleGraphs: () => void;
  /** Chooses the graph that is drawn. */
  readonly setGraphPreset: (name: string) => void;
  /** Follows the vehicle with the map, or stops. */
  readonly setFollowVehicle: (follow: boolean) => void;
  /** Shows or hides the map's controls. */
  readonly toggleMapControls: () => void;
  /** Opens the sheet of parameters. */
  readonly showParameters: () => void;
  /** Closes the sheet of parameters. */
  readonly hideParameters: () => void;
}

const clamp = (value: number, low: number, high: number): number =>
  Math.min(high, Math.max(low, value));

export const useUiStore = create<UiStore>((set) => ({
  activeTab: "quick",
  leftColumnWidth: clamp(
    readNumber("mpcraft.leftWidth", DEFAULT_LEFT_WIDTH),
    MIN_LEFT_WIDTH,
    MAX_LEFT_WIDTH,
  ),
  hudHeight: clamp(
    readNumber("mpcraft.hudHeight", DEFAULT_HUD_HEIGHT),
    MIN_HUD_HEIGHT,
    MAX_HUD_HEIGHT,
  ),
  graphHeight: clamp(
    readNumber("mpcraft.graphHeight", DEFAULT_GRAPH_HEIGHT),
    MIN_GRAPH_HEIGHT,
    MAX_GRAPH_HEIGHT,
  ),
  showLeftColumn: readFlag("mpcraft.showLeftColumn", true),
  showGraphs: readFlag("mpcraft.showGraphs", true),
  graphPreset: readString("mpcraft.graphPreset", ""),
  followVehicle: readFlag("mpcraft.followVehicle", true),
  showMapControls: readFlag("mpcraft.showMapControls", true),
  parametersOpen: false,

  setActiveTab: (activeTab) => set({ activeTab }),

  setLeftColumnWidth: (width) => {
    const clamped = clamp(width, MIN_LEFT_WIDTH, MAX_LEFT_WIDTH);
    writeNumber("mpcraft.leftWidth", clamped);
    set({ leftColumnWidth: clamped });
  },

  setHudHeight: (height) => {
    const clamped = clamp(height, MIN_HUD_HEIGHT, MAX_HUD_HEIGHT);
    writeNumber("mpcraft.hudHeight", clamped);
    set({ hudHeight: clamped });
  },

  setGraphHeight: (height) => {
    const clamped = clamp(height, MIN_GRAPH_HEIGHT, MAX_GRAPH_HEIGHT);
    writeNumber("mpcraft.graphHeight", clamped);
    set({ graphHeight: clamped });
  },

  toggleLeftColumn: () =>
    set((state) => {
      writeFlag("mpcraft.showLeftColumn", !state.showLeftColumn);
      return { showLeftColumn: !state.showLeftColumn };
    }),

  toggleGraphs: () =>
    set((state) => {
      writeFlag("mpcraft.showGraphs", !state.showGraphs);
      return { showGraphs: !state.showGraphs };
    }),

  setGraphPreset: (graphPreset) => {
    writeString("mpcraft.graphPreset", graphPreset);
    set({ graphPreset });
  },

  setFollowVehicle: (followVehicle) => {
    writeFlag("mpcraft.followVehicle", followVehicle);
    set({ followVehicle });
  },

  toggleMapControls: () =>
    set((state) => {
      writeFlag("mpcraft.showMapControls", !state.showMapControls);
      return { showMapControls: !state.showMapControls };
    }),

  // The sheet of parameters is not remembered between runs: it is a thing the operator
  // opens, not a place the window is left in.
  showParameters: () => set({ parametersOpen: true }),

  hideParameters: () => set({ parametersOpen: false }),
}));
