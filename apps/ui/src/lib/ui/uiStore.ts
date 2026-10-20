import { create } from "zustand";

/**
 * The state of the interface itself: which tab is open, how wide the panels are and what
 * is switched on.
 *
 * It is a store rather than React state in the shell because the menu bar, the tab strip
 * and the map all read it, and a change in one of them must not re-render the others.
 */

/** The tabs of the left column, in the order upstream has them. */
export type TabId = "quick" | "actions" | "messages" | "gauges" | "status";

/** The tabs, with the names they are given. */
export const TABS: readonly { id: TabId; label: string }[] = [
  { id: "quick", label: "Quick" },
  { id: "actions", label: "Actions" },
  { id: "messages", label: "Messages" },
  { id: "gauges", label: "Gauges" },
  { id: "status", label: "Status" },
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

export interface UiStore {
  /** The tab that is open. */
  readonly activeTab: TabId;
  /** The width of the left column, in pixels. */
  readonly leftColumnWidth: number;
  /** The height of the attitude indicator, in pixels. */
  readonly hudHeight: number;
  /** Whether the left column is shown at all. */
  readonly showLeftColumn: boolean;
  /** Whether the map follows the vehicle. */
  readonly followVehicle: boolean;
  /** Whether the map's own controls are shown. */
  readonly showMapControls: boolean;
  /** Opens a tab. */
  readonly setActiveTab: (tab: TabId) => void;
  /** Moves the edge of the left column. */
  readonly setLeftColumnWidth: (width: number) => void;
  /** Moves the edge of the attitude indicator. */
  readonly setHudHeight: (height: number) => void;
  /** Shows or hides the left column. */
  readonly toggleLeftColumn: () => void;
  /** Follows the vehicle with the map, or stops. */
  readonly setFollowVehicle: (follow: boolean) => void;
  /** Shows or hides the map's controls. */
  readonly toggleMapControls: () => void;
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
  showLeftColumn: readFlag("mpcraft.showLeftColumn", true),
  followVehicle: readFlag("mpcraft.followVehicle", true),
  showMapControls: readFlag("mpcraft.showMapControls", true),

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

  toggleLeftColumn: () =>
    set((state) => {
      writeFlag("mpcraft.showLeftColumn", !state.showLeftColumn);
      return { showLeftColumn: !state.showLeftColumn };
    }),

  setFollowVehicle: (followVehicle) => {
    writeFlag("mpcraft.followVehicle", followVehicle);
    set({ followVehicle });
  },

  toggleMapControls: () =>
    set((state) => {
      writeFlag("mpcraft.showMapControls", !state.showMapControls);
      return { showMapControls: !state.showMapControls };
    }),
}));
