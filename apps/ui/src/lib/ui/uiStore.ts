import { create } from "zustand";

import { DEFAULT_PROVIDER } from "../../features/map/providers";
import { DEFAULT_UNITS, type UnitSystem } from "../units";
import { listParameterGroups } from "../ipc/bridge";

/**
 * The state of the interface itself: which screen the window is on, which tab is open,
 * how wide the panels are and what is switched on.
 *
 * It is a store rather than React state in the shell because the menu bar, the switcher,
 * the tab strip and the map all read it, and a change in one of them must not re-render
 * the others.
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

/**
 * The screens the window can be on.
 *
 * These are the views Mission Planner's main switcher has: the flight data, the planner,
 * the configuration, and the logs. The short tasks — a calibration, a joystick mapping,
 * a file transfer — are not screens: they are sheets that lie over whichever screen is
 * open, because the operator does them and comes back to where they were.
 */
export type ScreenId = "flightdata" | "plan" | "setup" | "logs";

/** The screens, in the order the switcher shows them. */
export const SCREENS: readonly {
  readonly id: ScreenId;
  readonly label: string;
}[] = [
  { id: "flightdata", label: "Flight data" },
  { id: "plan", label: "Plan" },
  { id: "setup", label: "Setup" },
  { id: "logs", label: "Logs" },
];

/** The screen the window is on when nothing has been chosen. */
export const DEFAULT_SCREEN: ScreenId = "flightdata";

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

/** Reads a stored screen, and falls back when what was stored is not a screen. */
function readScreen(key: string): ScreenId {
  const stored = readText(key, DEFAULT_SCREEN);
  const known = SCREENS.find((screen) => screen.id === stored);
  return known === undefined ? DEFAULT_SCREEN : known.id;
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
  /** The screen the window is on. */
  readonly screen: ScreenId;
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
  /** Puts the window on a screen. */
  readonly setScreen: (screen: ScreenId) => void;
  /** Puts the window on the screen of the logs. */
  readonly showLog: () => void;
  /** Takes the window back to the screen it was on before. */
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
  /** Whether the sheet of the on-screen display is open over the window. */
  readonly osdOpen: boolean;
  /** Opens the sheet of the on-screen display. */
  readonly showOsd: () => void;
  /** Closes the sheet of the on-screen display. */
  readonly hideOsd: () => void;
  /** Whether the sheet of the corrections of a base station is open. */
  readonly ntripOpen: boolean;
  /** Opens the sheet of the corrections of a base station. */
  readonly showNtrip: () => void;
  /** Closes the sheet of the corrections of a base station. */
  readonly hideNtrip: () => void;
  /** Whether the sheet of the transponder is open over the window. */
  readonly transponderOpen: boolean;
  /** Opens the sheet of the transponder. */
  readonly showTransponder: () => void;
  /** Closes the sheet of the transponder. */
  readonly hideTransponder: () => void;
  /** Whether the sheet of the traffic of the air is open over the window. */
  readonly adsbOpen: boolean;
  /** Opens the sheet of the traffic of the air. */
  readonly showAdsb: () => void;
  /** Closes the sheet of the traffic of the air. */
  readonly hideAdsb: () => void;
  /** Whether the sheet of the video streams is open over the window. */
  readonly videoOpen: boolean;
  /** Opens the sheet of the video streams. */
  readonly showVideo: () => void;
  /** Closes the sheet of the video streams. */
  readonly hideVideo: () => void;
  /** Whether the sheet of the camera and the gimbal is open over the window. */
  readonly cameraOpen: boolean;
  /** Opens the sheet of the camera and the gimbal. */
  readonly showCamera: () => void;
  /** Closes the sheet of the camera and the gimbal. */
  readonly hideCamera: () => void;
  /** Whether the sheet of the pictures and where they were taken is open over the window. */
  readonly picturesOpen: boolean;
  /** Opens the sheet of the pictures and where they were taken. */
  readonly showPictures: () => void;
  /** Closes the sheet of the pictures and where they were taken. */
  readonly hidePictures: () => void;
  /** Whether the sheet of the plugins is open over the window. */
  readonly pluginsOpen: boolean;
  /** Opens the sheet of the plugins. */
  readonly showPlugins: () => void;
  /** Closes the sheet of the plugins. */
  readonly hidePlugins: () => void;
  /** Whether the sheet of the formation is open over the window. */
  readonly formationOpen: boolean;
  /** Opens the sheet of the formation. */
  readonly showFormation: () => void;
  /** Closes the sheet of the formation. */
  readonly hideFormation: () => void;
  /** Whether the sheet of the keys of the signed link is open over the window. */
  readonly signingOpen: boolean;
  /** Opens the sheet of the keys of the signed link. */
  readonly showSigning: () => void;
  /** Closes the sheet of the keys of the signed link. */
  readonly hideSigning: () => void;
  /** Puts the window on the screen of the parameters. */
  readonly showParameters: () => void;
  /**
   * Puts the window on the screen of the parameters, showing the parameters of one group.
   *
   * A group of nothing shows every parameter.
   */
  readonly showParameterGroup: (group: string | null) => void;
  /**
   * Puts the window on the screen of the parameters, showing the parameters whose names
   * begin with the given word.
   */
  readonly showParameterPrefix: (prefix: string | null) => void;
  /** Reads the groups the parameters belong to. */
  readonly loadParameterGroups: () => Promise<void>;
  /** Takes the window back to the screen it was on before. */
  readonly hideParameters: () => void;
}

const clamp = (value: number, low: number, high: number): number =>
  Math.min(high, Math.max(low, value));

export const useUiStore = create<UiStore>((set, get) => ({
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
  screen: readScreen("mpcraft.screen"),
  parameterGroup: null,
  parameterPrefix: null,
  calibrationOpen: false,
  joystickOpen: false,
  terrainOpen: false,
  passthroughOpen: false,
  firmwareOpen: false,
  filesOpen: false,
  osdOpen: false,
  ntripOpen: false,
  transponderOpen: false,
  adsbOpen: false,
  videoOpen: false,
  cameraOpen: false,
  picturesOpen: false,
  pluginsOpen: false,
  formationOpen: false,
  signingOpen: false,
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

  setScreen: (screen) => {
    // The screen is remembered: a ground station left open on the plan editor is opened
    // again on it, the way upstream's window comes back to the tab it was on.
    writeText("mpcraft.screen", screen);
    set({ screen });
  },

  showParameters: () => get().setScreen("setup"),

  showParameterGroup: (group) => {
    set({ parameterGroup: group, parameterPrefix: null });
    get().setScreen("setup");
  },

  showParameterPrefix: (prefix) => {
    set({ parameterGroup: null, parameterPrefix: prefix });
    get().setScreen("setup");
  },

  showCalibration: () => set({ calibrationOpen: true }),
  showJoystick: () => set({ joystickOpen: true }),
  showTerrain: () => set({ terrainOpen: true }),
  showPassthrough: () => set({ passthroughOpen: true }),
  showFirmware: () => set({ firmwareOpen: true }),
  showFiles: () => set({ filesOpen: true }),
  showOsd: () => set({ osdOpen: true }),
  showNtrip: () => set({ ntripOpen: true }),
  showTransponder: () => set({ transponderOpen: true }),
  showAdsb: () => set({ adsbOpen: true }),
  showVideo: () => set({ videoOpen: true }),
  showCamera: () => set({ cameraOpen: true }),
  showPictures: () => set({ picturesOpen: true }),
  showPlugins: () => set({ pluginsOpen: true }),
  showFormation: () => set({ formationOpen: true }),
  showSigning: () => set({ signingOpen: true }),

  showLog: () => get().setScreen("logs"),

  hideLog: () => get().setScreen(DEFAULT_SCREEN),

  hideCalibration: () => set({ calibrationOpen: false }),
  hideJoystick: () => set({ joystickOpen: false }),
  hideTerrain: () => set({ terrainOpen: false }),
  hidePassthrough: () => set({ passthroughOpen: false }),
  hideFirmware: () => set({ firmwareOpen: false }),
  hideFiles: () => set({ filesOpen: false }),
  hideOsd: () => set({ osdOpen: false }),
  hideNtrip: () => set({ ntripOpen: false }),
  hideTransponder: () => set({ transponderOpen: false }),
  hideAdsb: () => set({ adsbOpen: false }),
  hideVideo: () => set({ videoOpen: false }),
  hideCamera: () => set({ cameraOpen: false }),
  hidePictures: () => set({ picturesOpen: false }),
  hidePlugins: () => set({ pluginsOpen: false }),
  hideFormation: () => set({ formationOpen: false }),
  hideSigning: () => set({ signingOpen: false }),

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

  hideParameters: () => get().setScreen(DEFAULT_SCREEN),
}));
