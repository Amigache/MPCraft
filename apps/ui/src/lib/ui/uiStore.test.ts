import { describe, expect, it } from "vitest";

import {
  DEFAULT_HUD_FRACTION,
  DEFAULT_LEFT_FRACTION,
  DEFAULT_TOP_FRACTION,
  TABS,
  useUiStore,
} from "./uiStore";

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
