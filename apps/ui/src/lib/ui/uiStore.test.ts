import { describe, expect, it } from "vitest";

import {
  DEFAULT_HUD_HEIGHT,
  DEFAULT_LEFT_WIDTH,
  MAX_HUD_HEIGHT,
  MAX_LEFT_WIDTH,
  MIN_HUD_HEIGHT,
  MIN_LEFT_WIDTH,
  TABS,
  useUiStore,
} from "./uiStore";

describe("the state of the interface", () => {
  it("keeps the panels within their limits", () => {
    const store = useUiStore.getState();

    store.setLeftColumnWidth(10);
    expect(useUiStore.getState().leftColumnWidth).toBe(MIN_LEFT_WIDTH);

    store.setLeftColumnWidth(10_000);
    expect(useUiStore.getState().leftColumnWidth).toBe(MAX_LEFT_WIDTH);

    store.setLeftColumnWidth(DEFAULT_LEFT_WIDTH);
    expect(useUiStore.getState().leftColumnWidth).toBe(DEFAULT_LEFT_WIDTH);
  });

  it("keeps the attitude indicator within its limits", () => {
    const store = useUiStore.getState();

    store.setHudHeight(10);
    expect(useUiStore.getState().hudHeight).toBe(MIN_HUD_HEIGHT);

    store.setHudHeight(10_000);
    expect(useUiStore.getState().hudHeight).toBe(MAX_HUD_HEIGHT);

    store.setHudHeight(DEFAULT_HUD_HEIGHT);
    expect(useUiStore.getState().hudHeight).toBe(DEFAULT_HUD_HEIGHT);
  });

  it("names the tabs the way upstream names them", () => {
    expect(TABS.map((tab) => tab.label)).toEqual([
      "Quick",
      "Actions",
      "Messages",
      "Gauges",
      "Status",
    ]);
  });

  it("opens a tab", () => {
    useUiStore.getState().setActiveTab("gauges");
    expect(useUiStore.getState().activeTab).toBe("gauges");
    useUiStore.getState().setActiveTab("quick");
  });
});
