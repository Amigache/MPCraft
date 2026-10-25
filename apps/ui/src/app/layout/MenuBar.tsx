import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";

import { ConnectionPanel } from "../../features/connection/ConnectionPanel";
import { ConnectionBar } from "./ConnectionBar";
import {
  armVehicle,
  closeLink,
  disarmVehicle,
} from "../../lib/flight/flightActions";
import { useModesStore } from "../../lib/flight/modesStore";
import { pickFolder, pickSavePath } from "../../lib/ipc/dialog";
import {
  diagnosticsBundle,
  httpServerStart,
  httpServerStop,
  setMapInjectionFolder,
  startLiveKml,
  stopLiveKml,
} from "../../lib/ipc/bridge";
import { useLinkStore } from "../../lib/link/linkStore";
import { usePlanEditorStore } from "../../lib/plan/planEditorStore";
import { usePlanStore } from "../../lib/plan/planStore";
import { TABS, useUiStore, type Theme } from "../../lib/ui/uiStore";
import { LOCALES } from "../../lib/i18n/strings";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import styles from "./MenuBar.module.css";

/**
 * The menu bar.
 *
 * Every entry either does something or is visibly disabled: a menu that leads nowhere is
 * worse than no menu at all. The state of the link and the target it is open on are on
 * the right, where they can be read without opening anything.
 *
 * The names the bar says are read through the bundles of `lib/i18n`, so that the bar, the
 * switcher and the panels of the window speak the same language, and the one that is chosen
 * is kept for the next run.
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

/**
 * The themes the interface can be drawn in.
 *
 * The dark one is the default, the light one is for the sun and the one of the contrast
 * is for the eyes that need it. The table holds what is looked up in the catalogue, not the
 * word itself: the bar draws its items as it is given them, so a word put here would never be
 * translated on the way to the screen.
 */
const THEMES: readonly { id: Theme; key: string }[] = [
  { id: "dark", key: "Dark" },
  { id: "light", key: "Light" },
  { id: "contrast", key: "High contrast" },
];

/** The version shown in the About box. */
const VERSION = "0.1.0";

export function MenuBar() {
  const [open, setOpen] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [about, setAbout] = useState(false);
  /** The file the track of the flight is being written to, when it is. */
  const [liveKml, setLiveKml] = useState<string | null>(null);
  /** What the last thing the operator did said. */
  const [note, setNote] = useState<string | null>(null);
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

  // The bar says its names through the bundles, so that the menus, the switcher and the panels
  // read the same words in the same language.
  const locale = useLocaleStore((state) => state.locale);
  const setLocale = useLocaleStore((state) => state.setLocale);
  const t = useLocaleStore((state) => state.t);

  const activeTab = useUiStore((state) => state.activeTab);
  const setActiveTab = useUiStore((state) => state.setActiveTab);
  const showLeftColumn = useUiStore((state) => state.showLeftColumn);
  const toggleLeftColumn = useUiStore((state) => state.toggleLeftColumn);
  const showGraphs = useUiStore((state) => state.showGraphs);
  const toggleGraphs = useUiStore((state) => state.toggleGraphs);
  const followVehicle = useUiStore((state) => state.followVehicle);
  const setFollowVehicle = useUiStore((state) => state.setFollowVehicle);
  const showMapControls = useUiStore((state) => state.showMapControls);
  const units = useUiStore((state) => state.units);
  const setUnits = useUiStore((state) => state.setUnits);
  const theme = useUiStore((state) => state.theme);
  const setTheme = useUiStore((state) => state.setTheme);
  const toggleMapControls = useUiStore((state) => state.toggleMapControls);
  const showParameters = useUiStore((state) => state.showParameters);
  const focusMap = useUiStore((state) => state.focusMap);
  const toggleFocusMap = useUiStore((state) => state.toggleFocusMap);
  const showCalibration = useUiStore((state) => state.showCalibration);
  const showJoystick = useUiStore((state) => state.showJoystick);
  const showTerrain = useUiStore((state) => state.showTerrain);
  const showPassthrough = useUiStore((state) => state.showPassthrough);
  const showFirmware = useUiStore((state) => state.showFirmware);
  const showFiles = useUiStore((state) => state.showFiles);
  const showOsd = useUiStore((state) => state.showOsd);
  const showNtrip = useUiStore((state) => state.showNtrip);
  const showTransponder = useUiStore((state) => state.showTransponder);
  const showAdsb = useUiStore((state) => state.showAdsb);
  const showVideo = useUiStore((state) => state.showVideo);
  const showCamera = useUiStore((state) => state.showCamera);
  const showPictures = useUiStore((state) => state.showPictures);
  const showPlugins = useUiStore((state) => state.showPlugins);
  const showFormation = useUiStore((state) => state.showFormation);
  const showSigning = useUiStore((state) => state.showSigning);

  // Whether the web server is running: the menu says so, and the entry stops it.
  const [serving, setServing] = useState(false);
  const showLog = useUiStore((state) => state.showLog);
  const showParameterPrefix = useUiStore((state) => state.showParameterPrefix);
  const parameterGroups = useUiStore((state) => state.parameterGroups);
  const loadParameterGroups = useUiStore((state) => state.loadParameterGroups);
  const showParameterGroup = useUiStore((state) => state.showParameterGroup);

  // The plan editor is the screen of the plan, so the menu marks it checked when the
  // window is on that screen.
  const editorOpen = useUiStore((state) => state.screen === "plan");
  const openEditor = usePlanEditorStore((state) => state.openEditor);

  // The groups of the parameters are the same for every vehicle, so they are read
  // once and then they are only looked at.
  useEffect(() => {
    void loadParameterGroups();
  }, [loadParameterGroups]);

  const close = useCallback(() => setOpen(null), []);

  // The folder of tiles the operator has brought: the map is drawn with the pictures
  // that are in it, and the folder is kept between two runs.
  const askMapFolder = useCallback(async () => {
    const picked = await pickFolder();

    if (!picked.ok) {
      return;
    }

    try {
      await setMapInjectionFolder(picked.path);
      setNote(t("The tiles are taken from {folder}.", { folder: picked.path }));
    } catch (error) {
      const cause = error as { message?: string };
      setNote(cause.message ?? t("The folder of tiles could not be kept."));
    }
  }, [setNote, t]);
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
      label: t("File"),
      items: [
        {
          id: "connect",
          label: t("Connect…"),
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
          label: t("Disconnect"),
          onSelect: () => {
            close();
            void closeLink();
          },
          disabled: !linked,
          checked: undefined,
          separator: false,
          hint: linked ? "" : t("no link"),
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
          id: "live-kml",
          label:
            liveKml === null
              ? t("Stream the track to a KML file…")
              : t("Stop writing the track"),
          onSelect: () => {
            close();

            if (liveKml !== null) {
              void stopLiveKml()
                .then(() => {
                  setLiveKml(null);
                  setNote(t("The track has been written."));
                })
                .catch((error: { message?: string }) => {
                  setNote(
                    error.message ?? t("The track could not be stopped."),
                  );
                });
              return;
            }

            void pickSavePath("track.kml", [
              { name: "KML", extensions: ["kml"] },
            ])
              .then((picked) => {
                if (!picked.ok) {
                  return;
                }

                return startLiveKml(picked.path)
                  .then(() => {
                    setLiveKml(picked.path);
                    setNote(
                      t("The track is written to {path}.", {
                        path: picked.path,
                      }),
                    );
                  })
                  .catch((error: { message?: string }) => {
                    setNote(
                      error.message ?? t("The track could not be started."),
                    );
                  });
              })
              .catch(() => {
                setNote(t("The file could not be asked for."));
              });
          },
          disabled: false,
          checked: liveKml !== null ? true : undefined,
          separator: false,
          hint: liveKml === null ? "" : liveKml,
        },
        {
          id: "passthrough",
          label: t("Serial passthrough?"),
          onSelect: () => {
            close();
            showPassthrough();
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "vehicle-files",
          label: t("The files of the vehicle…"),
          onSelect: () => {
            close();
            showFiles();
          },
          disabled: !linked,
          checked: undefined,
          separator: false,
          hint: linked ? "" : t("no link"),
        },
        {
          id: "exit",
          label: t("Exit"),
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
      label: t("Flight"),
      items: [
        {
          id: "arm",
          label: t("Arm"),
          onSelect: () => {
            close();
            void armVehicle();
          },
          disabled: !linked,
          checked: undefined,
          separator: false,
          hint: linked ? "" : t("no link"),
        },
        {
          id: "disarm",
          label: t("Disarm"),
          onSelect: () => {
            close();
            void disarmVehicle();
          },
          disabled: !linked,
          checked: undefined,
          separator: false,
          hint: linked ? "" : t("no link"),
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
          label: t("Set mode…"),
          onSelect: () => {
            close();
            setActiveTab("actions");
          },
          disabled: modes.length === 0,
          checked: undefined,
          separator: false,
          hint: modes.length === 0 ? t("no modes") : "",
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
          label: t("Download plan"),
          onSelect: () => {
            close();
            void download("mission");
          },
          disabled: !linked || busy,
          checked: undefined,
          separator: false,
          hint: linked ? "" : t("no link"),
        },
        {
          id: "upload",
          label: t("Upload plan"),
          onSelect: () => {
            close();
            void upload("mission", plan?.items ?? []);
          },
          disabled: !linked || busy || !plan || plan.items.length === 0,
          checked: undefined,
          separator: false,
          hint: plan ? "" : t("no plan"),
        },
        {
          id: "clear",
          label: t("Clear plan"),
          onSelect: () => {
            close();
            void clear("mission");
          },
          disabled: !linked || busy,
          checked: undefined,
          separator: false,
          hint: linked ? "" : t("no link"),
        },
        {
          id: "flight-sep-3",
          label: "",
          onSelect: undefined,
          disabled: false,
          checked: undefined,
          separator: true,
          hint: "",
        },
        {
          id: "joystick",
          label: t("Joystick…"),
          onSelect: () => {
            close();
            showJoystick();
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
      ],
    },
    {
      id: "view",
      label: t("View"),
      items: [
        {
          id: "focus-map",
          label: t("The map alone"),
          onSelect: () => {
            close();
            toggleFocusMap();
          },
          disabled: false,
          checked: focusMap,
          separator: false,
          hint: "",
        },
        {
          id: "logs",
          label: t("Logs"),
          onSelect: () => {
            close();
            showLog();
          },
          disabled: false,
          checked: undefined,
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
        {
          id: "theme",
          label: t("The theme"),
          onSelect: undefined,
          disabled: true,
          checked: undefined,
          separator: false,
          hint: "",
        },
        ...THEMES.map((entry): MenuItem => ({
          id: `theme-${entry.id}`,
          label: t(entry.key),
          onSelect: () => {
            close();
            setTheme(entry.id);
          },
          disabled: false,
          checked: theme === entry.id,
          separator: false,
          hint: "",
        })),
        {
          id: "view-language-sep",
          label: "",
          onSelect: undefined,
          disabled: false,
          checked: undefined,
          separator: true,
          hint: "",
        },
        {
          id: "language",
          label: t("The language"),
          onSelect: undefined,
          disabled: true,
          checked: undefined,
          separator: false,
          hint: "",
        },
        ...LOCALES.map((entry): MenuItem => ({
          id: `language-${entry.id}`,
          // A language is named in its own words, so that the two can be told apart by somebody
          // who cannot read either of them.
          label: entry.label,
          onSelect: () => {
            close();
            setLocale(entry.id);
          },
          disabled: false,
          checked: locale === entry.id,
          separator: false,
          hint: "",
        })),
        {
          id: "http",
          label: t("Serve the flight on the network"),
          onSelect: () => {
            close();

            if (serving) {
              void httpServerStop()
                .then(() => {
                  setServing(false);
                  setNote(t("The server was stopped."));
                })
                .catch((cause: { message?: string }) => {
                  setNote(
                    cause.message ?? t("The server could not be stopped."),
                  );
                });
              return;
            }

            void httpServerStart()
              .then((port) => {
                setServing(true);
                setNote(
                  t("The flight is served on the port {port}.", { port }),
                );
              })
              .catch((cause: { message?: string }) => {
                setNote(cause.message ?? t("The server could not be started."));
              });
          },
          disabled: false,
          checked: serving,
          separator: false,
          hint: "",
        },
        {
          id: "view-sep-theme",
          label: "",
          onSelect: undefined,
          disabled: false,
          checked: undefined,
          separator: true,
          hint: "",
        },
        {
          id: "units",
          label: t("Units in feet and miles"),
          onSelect: () => {
            close();
            setUnits(units === "imperial" ? "metric" : "imperial");
          },
          disabled: false,
          checked: units === "imperial",
          separator: false,
          hint: "",
        },
        {
          id: "side-panel",
          label: t("Side panel"),
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
          label: t("Graphs"),
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
          label: t("Follow the vehicle"),
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
          label: t("Map controls"),
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
          label: t(tab.label),
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
      label: t("Setup"),
      items: [
        {
          id: "modes",
          label: t("Flight modes"),
          onSelect: () => {
            close();
            showParameterPrefix("FLTMODE");
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
          id: "calibration-accelerometer",
          label: t("Accelerometer"),
          onSelect: () => {
            close();
            showCalibration();
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "calibration-compass",
          label: t("Compass"),
          onSelect: () => {
            close();
            showCalibration();
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-frame",
          label: t("Frame type"),
          onSelect: () => {
            close();
            showParameterPrefix("FRAME_");
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-gps",
          label: t("GPS"),
          onSelect: () => {
            close();
            showParameterPrefix("GPS_");
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-serial",
          label: t("Serial ports"),
          onSelect: () => {
            close();
            showParameterPrefix("SERIAL");
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-radio",
          label: t("Radio"),
          onSelect: () => {
            close();
            showParameterPrefix("RC_");
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-failsafe",
          label: t("Fail safe"),
          onSelect: () => {
            close();
            showParameterPrefix("FS_");
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-mount",
          label: t("Mount"),
          onSelect: () => {
            close();
            showParameterPrefix("MNT_");
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-dronecan",
          label: t("DroneCAN"),
          onSelect: () => {
            close();
            showParameterPrefix("CAN_");
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-map-folder",
          label: t("The folder of tiles…"),
          onSelect: () => {
            close();
            void askMapFolder();
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-terrain",
          label: t("Terrain…"),
          onSelect: () => {
            close();
            showTerrain();
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-osd",
          label: t("On-screen display…"),
          onSelect: () => {
            close();
            showOsd();
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-ntrip",
          label: t("The corrections of a base station…"),
          onSelect: () => {
            close();
            showNtrip();
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-transponder",
          label: t("The transponder…"),
          onSelect: () => {
            close();
            showTransponder();
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-adsb",
          label: t("The traffic of the air…"),
          onSelect: () => {
            close();
            showAdsb();
          },
          disabled: !linked,
          checked: undefined,
          separator: false,
          hint: linked ? "" : t("no link"),
        },
        {
          id: "setup-video",
          label: t("Video streams…"),
          onSelect: () => {
            close();
            showVideo();
          },
          disabled: !linked,
          checked: undefined,
          separator: false,
          hint: linked ? "" : t("no link"),
        },
        {
          id: "setup-camera",
          label: t("The camera and the gimbal…"),
          onSelect: () => {
            close();
            showCamera();
          },
          disabled: !linked,
          checked: undefined,
          separator: false,
          hint: linked ? "" : t("no link"),
        },
        {
          id: "setup-pictures",
          label: t("The pictures and where they were taken…"),
          onSelect: () => {
            close();
            showPictures();
          },
          // The sheet reads files, so it is open without a vehicle on the wire: a folder of
          // pictures and a log of a flight that has already happened are enough.
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-plugins",
          label: t("The plugins…"),
          onSelect: () => {
            close();
            showPlugins();
          },
          // A plugin is somebody else's code, so the sheet that lists them and says what each one
          // asks for is open without a vehicle on the wire: deciding what may run does not need a
          // link, and a plugin that has nothing to do on a tick never asks for one.
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-formation",
          label: t("The formation…"),
          onSelect: () => {
            close();
            showFormation();
          },
          // The places an operator wrote are read from the file, so the sheet opens without a
          // vehicle on the wire: a fleet is often set up before any board is on the link. What
          // needs a link is the loop, and the sheet says so rather than pretending to send orders.
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "setup-keys",
          label: t("The keys of the signed link…"),
          onSelect: () => {
            close();
            showSigning();
          },
          // The keys are read from the file, so the sheet opens without a vehicle on the wire: what
          // a person needs when a key was written wrong is to see which line they wrote wrong, and
          // that does not need a link.
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "parameters",
          label: t("Parameters…"),
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
          label: t("Mission planner"),
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
          id: "setup-sep-groups",
          label: "",
          onSelect: undefined,
          disabled: false,
          checked: undefined,
          separator: true,
          hint: "",
        },
        ...parameterGroups.map((group): MenuItem => ({
          id: `group-${group}`,
          label: group,
          onSelect: () => {
            close();
            showParameterGroup(group);
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        })),
        {
          id: "firmware",
          label: t("Firmware…"),
          onSelect: () => {
            close();
            showFirmware();
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
      ],
    },
    {
      id: "help",
      label: t("Help"),
      items: [
        {
          id: "diagnostics",
          label: t("Write a diagnostics bundle…"),
          onSelect: () => {
            close();

            void pickSavePath("mpcraft-diagnostics.zip", [
              { name: t("The bundle of the diagnostics"), extensions: ["zip"] },
            ])
              .then((picked) => {
                if (!picked.ok) {
                  return;
                }

                return diagnosticsBundle(picked.path)
                  .then(() => {
                    setNote(
                      t("The bundle was written to {path}.", {
                        path: picked.path,
                      }),
                    );
                  })
                  .catch((cause: { message?: string }) => {
                    setNote(
                      cause.message ?? "The bundle could not be written.",
                    );
                  });
              })
              .catch(() => {
                setNote(t("The file could not be asked for."));
              });
          },
          disabled: false,
          checked: undefined,
          separator: false,
          hint: "",
        },
        {
          id: "about",
          label: t("About MPCraft"),
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
    <div
      className={styles.bar}
      role="menubar"
      aria-label={t("Main menu")}
      ref={bar}
    >
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
                const index = entries.findIndex(
                  (entry) => entry === event.target,
                );
                if (event.key === "ArrowDown") {
                  event.preventDefault();
                  entries[(index + 1) % entries.length]?.focus();
                } else if (event.key === "ArrowUp") {
                  event.preventDefault();
                  entries[
                    (index - 1 + entries.length) % entries.length
                  ]?.focus();
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
                  <div
                    className={styles.separator}
                    key={item.id}
                    role="separator"
                  />
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
        <ConnectionBar />
        {note !== null ? (
          <span className={styles.note} role="status">
            {note}
          </span>
        ) : null}
        <span
          className={styles.target}
          title={t("The link the application is opened on")}
        >
          {vehicles.length > 0
            ? `${vehicles[0]?.systemId ?? "?"}/${vehicles[0]?.componentId ?? "?"}`
            : t("no vehicle")}
        </span>
        <span
          className={`${styles.state} ${styles[status] ?? ""}`}
          title={t("The state of the link")}
        >
          {t(status)}
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
            aria-label={t("Connection")}
            aria-modal="true"
            onClick={(event) => event.stopPropagation()}
          >
            <div className={styles.dialogHead}>
              <h2 className={styles.dialogTitle}>{t("Connection")}</h2>
              <button
                type="button"
                className="button"
                onClick={() => setConnecting(false)}
              >
                {t("Close")}
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
            aria-label={t("About MPCraft")}
            onClick={(event) => event.stopPropagation()}
          >
            <div className={styles.dialogHead}>
              <h2 className={styles.dialogTitle}>{t("About MPCraft")}</h2>
              <button
                type="button"
                className="button"
                onClick={() => setAbout(false)}
              >
                {t("Close")}
              </button>
            </div>
            <p className={styles.about}>
              {t(
                "MPCraft {version} — a ground station for ArduPilot and PX4, written in Rust and React.",
                { version: VERSION },
              )}
            </p>
            <p className={styles.about}>
              {t(
                "The map uses the demo tiles of MapLibre and needs no account.",
              )}
            </p>
          </div>
        </div>
      ) : null}
    </div>
  );
}
