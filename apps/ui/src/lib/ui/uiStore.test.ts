import { describe, expect, it, vi } from "vitest";

import {
  DEFAULT_HUD_FRACTION,
  DEFAULT_LEFT_FRACTION,
  DEFAULT_TOP_FRACTION,
  SCREENS,
  TABS,
  useUiStore,
} from "./uiStore";

/**
 * The store as it is when a screen is already stored.
 *
 * The screen is remembered between two runs, and what is remembered can be something
 * that is not a screen: the store has to fall back rather than show a screen that does
 * not exist. A storage is read through `window`, so the test gives one, and the store is
 * built again because it takes its first value when the module is imported.
 */
async function storeWithStoredScreen(stored: string | null) {
  const values = new Map<string, string>();
  if (stored !== null) {
    values.set("mpcraft.screen", stored);
  }

  (globalThis as { window?: unknown }).window = {
    localStorage: {
      getItem: (key: string) =>
        values.has(key) ? (values.get(key) as string) : null,
      setItem: (key: string, value: string) => {
        values.set(key, value);
      },
    },
  };

  vi.resetModules();
  const module = await import("./uiStore");

  return { store: module.useUiStore, values };
}

describe("the state of the interface", () => {
  it("keeps the proportions of the layout within their limits", () => {
    const store = useUiStore.getState();

    store.setLeftColumnFraction(0.01);
    expect(useUiStore.getState().leftColumnFraction).toBeCloseTo(0.12, 6);

    store.setLeftColumnFraction(0.99);
    expect(useUiStore.getState().leftColumnFraction).toBeCloseTo(0.42, 6);

    store.setLeftColumnFraction(DEFAULT_LEFT_FRACTION);
    expect(useUiStore.getState().leftColumnFraction).toBeCloseTo(
      DEFAULT_LEFT_FRACTION,
      6,
    );
  });

  it("keeps the attitude indicator within its limits", () => {
    const store = useUiStore.getState();

    store.setHudFraction(0.01);
    expect(useUiStore.getState().hudFraction).toBeCloseTo(0.2, 6);

    store.setHudFraction(0.99);
    expect(useUiStore.getState().hudFraction).toBeCloseTo(0.65, 6);

    store.setHudFraction(DEFAULT_HUD_FRACTION);
    expect(useUiStore.getState().hudFraction).toBeCloseTo(
      DEFAULT_HUD_FRACTION,
      6,
    );
  });

  it("names the tabs the way upstream names them", () => {
    expect(TABS.map((tab) => tab.label)).toEqual([
      "Quick",
      "Actions",
      "Messages",
      "Pre-flight",
      "Gauges",
      "Status",
      "Servos",
      "Aux",
    ]);
  });

  it("keeps the pane above the map within its limits", () => {
    const store = useUiStore.getState();

    store.setTopFraction(0.01);
    expect(useUiStore.getState().topFraction).toBeCloseTo(0.15, 6);

    store.setTopFraction(0.99);
    expect(useUiStore.getState().topFraction).toBeCloseTo(0.75, 6);

    store.setTopFraction(DEFAULT_TOP_FRACTION);
    expect(useUiStore.getState().topFraction).toBeCloseTo(
      DEFAULT_TOP_FRACTION,
      6,
    );
  });

  it("puts the graphs away and brings them back", () => {
    const before = useUiStore.getState().showGraphs;

    useUiStore.getState().toggleGraphs();
    expect(useUiStore.getState().showGraphs).toBe(!before);

    useUiStore.getState().toggleGraphs();
    expect(useUiStore.getState().showGraphs).toBe(before);
  });

  it("keeps the graph that is chosen", () => {
    useUiStore.getState().setGraphPreset("Speed/Ground Speed");
    expect(useUiStore.getState().graphPreset).toBe("Speed/Ground Speed");
  });

  it("opens a tab", () => {
    useUiStore.getState().setActiveTab("gauges");
    expect(useUiStore.getState().activeTab).toBe("gauges");
    useUiStore.getState().setActiveTab("quick");
  });
});

describe("the screens", () => {
  it("names the screens the way upstream names its views", async () => {
    const { store } = await storeWithStoredScreen(null);

    expect(store.getState().screen).toBe("flightdata");
    expect(SCREENS.map((screen) => screen.label)).toEqual([
      "Flight data",
      "Plan",
      "Setup",
      "Logs",
    ]);
  });

  it("comes back to the screen it was on, and remembers the one it chooses", async () => {
    const { store, values } = await storeWithStoredScreen("plan");

    expect(store.getState().screen).toBe("plan");

    store.getState().setScreen("logs");
    expect(store.getState().screen).toBe("logs");
    expect(values.get("mpcraft.screen")).toBe("logs");
  });

  it("falls back to the flight data when what was stored is not a screen", async () => {
    const { store } = await storeWithStoredScreen("cockpit");

    expect(store.getState().screen).toBe("flightdata");
  });

  it("puts the window on the screen of the parameters when a group is chosen", async () => {
    const { store } = await storeWithStoredScreen(null);

    store.getState().showParameterGroup("Frame");
    expect(store.getState().screen).toBe("setup");
    expect(store.getState().parameterGroup).toBe("Frame");
    expect(store.getState().parameterPrefix).toBe(null);

    store.getState().hideParameters();
    expect(store.getState().screen).toBe("flightdata");
  });

  it("puts the window on the screen of the parameters when a prefix is chosen", async () => {
    const { store } = await storeWithStoredScreen(null);

    store.getState().showParameterPrefix("SERIAL");
    expect(store.getState().screen).toBe("setup");
    expect(store.getState().parameterPrefix).toBe("SERIAL");
    expect(store.getState().parameterGroup).toBe(null);
  });

  it("puts the window on the screen of the logs and back to the flight data", async () => {
    const { store } = await storeWithStoredScreen(null);

    store.getState().showLog();
    expect(store.getState().screen).toBe("logs");

    store.getState().hideLog();
    expect(store.getState().screen).toBe("flightdata");
  });

  it("keeps a sheet as a sheet: a short task does not change the screen", async () => {
    const { store } = await storeWithStoredScreen(null);

    store.getState().showCalibration();
    expect(store.getState().screen).toBe("flightdata");
    expect(store.getState().calibrationOpen).toBe(true);
  });
});
