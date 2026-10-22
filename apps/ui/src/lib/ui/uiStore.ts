import { create } from "zustand";

import { DEFAULT_PROVIDER } from "../../features/map/providers";
import { DEFAULT_UNITS, type UnitSystem } from "../units";
import { listParameterGroups } from "../ipc/bridge";

/**
 * The state of the interface itself: which tab is open, how wide the panels are and what
 * is switched on.
 *
 * It is a store rather than React state in the shell because the menu bar, the tab strip
 * and the map all read it, and a change in one of them must not re-render the others.
 */

/**
 * The theme the interface is drawn in.
 *
 * The dark one is the default, because a ground station is used in a dim room and next
 * to an aircraft. The light one is for the sun, and the one of the contrast is for the
 * eyes that need it.
 */
export type Theme = "dark" | "light" | "contrast";

/** The theme the interface is drawn in when nothing has been chosen. */
export const DEFAULT_THEME: Theme = "dark";

/** The tabs of the left column, in the order upstream has them. */
export type TabId =
  | "quick"
  | "actions"
  | "messages"
  | "preflight"
  | "gauges"
  | "status"
  | "servos"
  | "aux";

/**
 * The tabs, with the names they are given.
 *
 * The order is upstream's: the quick view, the actions, the messages, the check list
 * before the flight, the dials, the state of the vehicle and the outputs.
 */
export const TABS: readonly { id: TabId; label: string }[] = [
  { id: "quick", label: "Quick" },
  { id: "actions", label: "Actions" },
  { id: "messages", label: "Messages" },
  { id: "preflight", label: "Pre-flight" },
  { id: "gauges", label: "Gauges" },
  { id: "status", label: "Status" },
  { id: "servos", label: "Servos" },
  { id: "aux", label: "Aux" },
];

/**
 * The proportions of the layout.
 *
 * The layout is kept as proportions of the window, not as pixels: a ground station is
 * used on a laptop, on a tablet in the field and on a screen in the van, and the same
 * arrangement has to work on all three. The pixels are worked out when the window is
 * measured, and the floors are what the proportions are not allowed to go under.
 */

/** The width of the left column, as a part of the window. */
export const DEFAULT_LEFT_FRACTION = 0.22;

/** The narrowest the left column may be dragged. */
export const MIN_LEFT_WIDTH = 260;

/** The widest the left column may be dragged. */
export const MAX_LEFT_WIDTH = 520;

/** The height of the attitude indicator, as a part of the column. */
export const DEFAULT_HUD_FRACTION = 0.42;

/** The narrowest the attitude indicator may be dragged. */
export const MIN_HUD_HEIGHT = 140;

/** The tallest the attitude indicator may be dragged. */
export const MAX_HUD_HEIGHT = 420;

/** The height of the pane above the map, as a part of the area it shares. */
export const DEFAULT_TOP_FRACTION = 0.3;

/** The narrowest the graphs may be dragged. */
export const MIN_GRAPH_HEIGHT = 100;

/**
 * The narrowest the plan editor may be dragged.
 *
 * The editor holds a grid of the items of a plan and a row of buttons: less than this
 * and there is nothing to read.
 */
export const MIN_PLAN_HEIGHT = 260;

/** The tallest the graphs may be dragged. */
export const MAX_GRAPH_HEIGHT = 720;

/** The narrowest the map may be left when the graphs are above it. */
export const MIN_MAP_HEIGHT = 160;

/**
 * Reads a stored proportion, ignoring anything that is not one.
 *
 * The layout was kept in pixels before, and those pixels are read once and turned into
 * a proportion: the arrangement of an installation that was set up with them is kept,
 * and from then on it follows the window.
 */
function readFraction(key: string, fallback: number): number {
  const stored = readNumber(key, Number.NaN);

  if (Number.isFinite(stored)) {
    return clamp(stored, 0.05, 0.95);
  }

  return fallback;
}

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

/** Reads a stored text, ignoring anything that is not one. */
function readText(key: string, fallback: string): string {
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

/** Stores a text, ignoring a storage that refuses. */
function writeText(key: string, value: string): void {
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.setItem(key, value);
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
  /**
   * The width of the left column, as a part of the window.
   *
   * The layout is kept as a proportion so that it survives a change of resolution: a
   * column that is a fifth of the window is a fifth of it on every screen.
   */
  readonly leftColumnFraction: number;
  /** The height of the attitude indicator, as a part of the column. */
  readonly hudFraction: number;
  /** The height of the pane above the map, as a part of the area it shares. */
  readonly topFraction: number;
  /**
   * Whether the map is shown alone.
   *
   * The map is what the operator looks at when the vehicle is flying, and on a small
   * screen the panels beside it take the room it needs. The focus puts the map on its
   * own, and the panels come back when it is turned off.
   */
  readonly focusMap: boolean;
  /** Whether the left column is shown at all. */
  readonly showLeftColumn: boolean;
  /** Puts the map on its own, or brings the panels back. */
  readonly toggleFocusMap: () => void;
  /** Whether the graphs are shown at all. */
  readonly showGraphs: boolean;
  /** The graph that is drawn, by its full name. */
  readonly graphPreset: string;
  /** Whether the map follows the vehicle. */
  readonly followVehicle: boolean;
  /** Whether the map's own controls are shown. */
  readonly showMapControls: boolean;

  /** The provider of the tiles the map is drawn with. */
  readonly mapProvider: string;

  /** Whether the next click on the map tells the vehicle to go there. */
  readonly mapGoing: boolean;

  /**
   * Whether the path of the plan is drawn as a smooth curve.
   *
   * The plan is a list of places and the vehicle goes from one to the next in
   * a straight line. The smooth path is what the plan looks like when the
   * vehicle is told to follow a spline, and it is what the map draws when the
   * operator asks for it.
   */
  readonly spline: boolean;
  /**
   * Whether the plan is carried to the vehicle in the file it keeps it in.
   *
   * A plan usually travels with the mission protocol, and the vehicle asks for each
   * item. It can also travel in the file the vehicle keeps it in, over the file
   * transfer protocol, which is the way upstream offers it — the `chk_usemavftp` box
   * of the plan editor.
   */
  readonly mavftp: boolean;
  /** The theme the interface is drawn in. */
  readonly theme: Theme;
  /** The system of units the operator reads the measurements in. */
  readonly units: UnitSystem;
  /** Whether the sheet of parameters is open over the window. */
  readonly parametersOpen: boolean;
  /**
   * The group of parameters that are shown, when one is chosen.
   *
   * The groups are the subjects the parameters are about, and upstream builds its
   * configuration menu out of them. The Setup menu chooses one and the sheet shows
   * the parameters of that group.
   */
  readonly parameterGroup: string | null;
  /**
   * The prefix the names of the parameters have, when one is chosen.
   *
   * Some of the screens upstream has are about the parameters whose names
   * begin with a word — the serial ports, the flight modes, the failsafe — and
   * those parameters are not in any group of the catalogue.
   */
  readonly parameterPrefix: string | null;
  /**
   * The groups the parameters belong to.
   *
   * The groups are the subjects the parameters are about, and upstream builds its
   * configuration menu out of them.
   */
  readonly parameterGroups: readonly string[];
  /** Opens a tab. */
  readonly setActiveTab: (tab: TabId) => void;
  /** Moves the edge of the left column. */
  readonly setLeftColumnFraction: (fraction: number) => void;
  /** Moves the edge of the attitude indicator. */
  readonly setHudFraction: (fraction: number) => void;
  /** Moves the edge of the pane above the map. */
  readonly setTopFraction: (fraction: number) => void;
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

  /** Chooses the provider of the tiles the map is drawn with. */
  readonly setMapProvider: (provider: string) => void;

  /** Says whether the next click on the map tells the vehicle to go there. */
  readonly setMapGoing: (going: boolean) => void;

  /** Chooses the system of units. */
  readonly setUnits: (units: UnitSystem) => void;
  /** Chooses the theme. */
  readonly setTheme: (theme: Theme) => void;
  /** Chooses whether the path of the plan is drawn as a smooth curve. */
  readonly setSpline: (spline: boolean) => void;
  /** Sets whether the plan is carried in the file the vehicle keeps it in. */
  readonly setMavftp: (mavftp: boolean) => void;
  /** Whether the sheet of the logs is open over the window. */
  readonly logOpen: boolean;
  /** Opens the sheet of the logs. */
  readonly showLog: () => void;
  /** Closes the sheet of the logs. */
  readonly hideLog: () => void;
  /** Whether the sheet of the calibrations is open over the window. */
  readonly calibrationOpen: boolean;
  /** Opens the sheet of the calibrations. */
  readonly showCalibration: () => void;
  /** Closes the sheet of the calibrations. */
  readonly hideCalibration: () => void;
  /** Whether the sheet of the joystick is open over the window. */
  readonly joystickOpen: boolean;
  /** Opens the sheet of the joystick. */
  readonly showJoystick: () => void;
  /** Closes the sheet of the joystick. */
  readonly hideJoystick: () => void;
  /** Whether the sheet of the terrain is open over the window. */
  readonly terrainOpen: boolean;
  /** Opens the sheet of the terrain. */
  readonly showTerrain: () => void;
  /** Closes the sheet of the terrain. */
  readonly hideTerrain: () => void;
  /** Whether the sheet of the serial passthrough is open over the window. */
  readonly passthroughOpen: boolean;
  /** Opens the sheet of the serial passthrough. */
  readonly showPassthrough: () => void;
  /** Closes the sheet of the serial passthrough. */
  readonly hidePassthrough: () => void;
  /** Whether the sheet of the firmware is open over the window. */
  readonly firmwareOpen: boolean;
  /** Opens the sheet of the firmware. */
  readonly showFirmware: () => void;
  /** Closes the sheet of the firmware. */
  readonly hideFirmware: () => void;
  /** Whether the sheet of the files of the vehicle is open over the window. */
  readonly filesOpen: boolean;
  /** Opens the sheet of the files of the vehicle. */
  readonly showFiles: () => void;
  /** Closes the sheet of the files of the vehicle. */
  readonly hideFiles: () => void;
  /** Opens the sheet of parameters. */
  readonly showParameters: () => void;
  /**
   * Opens the sheet of parameters, showing the parameters of one group.
   *
   * A group of nothing shows every parameter.
   */
  readonly showParameterGroup: (group: string | null) => void;
  /**
   * Opens the sheet of parameters, showing the parameters whose names begin
   * with the given word.
   */
  readonly showParameterPrefix: (prefix: string | null) => void;
  /** Reads the groups the parameters belong to. */
  readonly loadParameterGroups: () => Promise<void>;
  /** Closes the sheet of parameters. */
  readonly hideParameters: () => void;
}

const clamp = (value: number, low: number, high: number): number =>
  Math.min(high, Math.max(low, value));

export const useUiStore = create<UiStore>((set) => ({
  activeTab: "quick",
  leftColumnFraction: readFraction(
    "mpcraft.leftFraction",
    DEFAULT_LEFT_FRACTION,
  ),
  hudFraction: readFraction("mpcraft.hudFraction", DEFAULT_HUD_FRACTION),
  topFraction: readFraction("mpcraft.topFraction", DEFAULT_TOP_FRACTION),
  showLeftColumn: readFlag("mpcraft.showLeftColumn", true),
  focusMap: readFlag("mpcraft.focusMap", false),
  showGraphs: readFlag("mpcraft.showGraphs", true),
  graphPreset: readString("mpcraft.graphPreset", ""),
  followVehicle: readFlag("mpcraft.followVehicle", true),
  showMapControls: readFlag("mpcraft.showMapControls", true),
  mapProvider: readText("mpcraft.mapProvider", DEFAULT_PROVIDER),
  mapGoing: false,
  units: readText("mpcraft.units", DEFAULT_UNITS) as UnitSystem,
  theme: readText("mpcraft.theme", DEFAULT_THEME) as Theme,
  spline: readFlag("mpcraft.spline", false),
  mavftp: readFlag("mpcraft.mavftp", false),
  parametersOpen: false,
  parameterGroup: null,
  parameterPrefix: null,
  calibrationOpen: false,
  joystickOpen: false,
  terrainOpen: false,
  passthroughOpen: false,
  firmwareOpen: false,
  filesOpen: false,
  logOpen: false,
  parameterGroups: [],

  setActiveTab: (activeTab) => set({ activeTab }),

  setLeftColumnFraction: (fraction) => {
    const clamped = clamp(fraction, 0.12, 0.42);
    writeNumber("mpcraft.leftFraction", clamped);
    set({ leftColumnFraction: clamped });
  },

  setHudFraction: (fraction) => {
    const clamped = clamp(fraction, 0.2, 0.65);
    writeNumber("mpcraft.hudFraction", clamped);
    set({ hudFraction: clamped });
  },

  setTopFraction: (fraction) => {
    const clamped = clamp(fraction, 0.15, 0.75);
    writeNumber("mpcraft.topFraction", clamped);
    set({ topFraction: clamped });
  },

  toggleFocusMap: () =>
    set((state) => {
      const focusMap = !state.focusMap;
      writeFlag("mpcraft.focusMap", focusMap);
      return { focusMap };
    }),

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

  setMapProvider: (provider) => {
    writeText("mpcraft.mapProvider", provider);
    set({ mapProvider: provider });
  },

  setMapGoing: (going) => set({ mapGoing: going }),

  setUnits: (units) => {
    writeText("mpcraft.units", units);
    set({ units });
  },

  setTheme: (theme) => {
    writeString("mpcraft.theme", theme);
    set({ theme });
  },

  setSpline: (spline) => {
    writeFlag("mpcraft.spline", spline);
    set({ spline });
  },

  setMavftp: (mavftp) => {
    writeFlag("mpcraft.mavftp", mavftp);
    set({ mavftp });
  },

  // The sheet of parameters is not remembered between runs: it is a thing the operator
  // opens, not a place the window is left in.
  showParameters: () => set({ parametersOpen: true }),

  showParameterGroup: (group) =>
    set({ parametersOpen: true, parameterGroup: group, parameterPrefix: null }),

  showParameterPrefix: (prefix) =>
    set({
      parametersOpen: true,
      parameterGroup: null,
      parameterPrefix: prefix,
    }),

  showCalibration: () => set({ calibrationOpen: true }),
  showJoystick: () => set({ joystickOpen: true }),
  showTerrain: () => set({ terrainOpen: true }),
  showPassthrough: () => set({ passthroughOpen: true }),
  showFirmware: () => set({ firmwareOpen: true }),
  showFiles: () => set({ filesOpen: true }),

  showLog: () => set({ logOpen: true }),

  hideLog: () => set({ logOpen: false }),

  hideCalibration: () => set({ calibrationOpen: false }),
  hideJoystick: () => set({ joystickOpen: false }),
  hideTerrain: () => set({ terrainOpen: false }),
  hidePassthrough: () => set({ passthroughOpen: false }),
  hideFirmware: () => set({ firmwareOpen: false }),
  hideFiles: () => set({ filesOpen: false }),

  loadParameterGroups: async () => {
    try {
      const groups = await listParameterGroups();
      set({ parameterGroups: groups });
    } catch (cause) {
      // A catalogue that could not be read is an empty one, and an empty
      // configuration menu is better than one that lies about the groups.
      set({ parameterGroups: [] });
    }
  },

  hideParameters: () => set({ parametersOpen: false }),
}));
