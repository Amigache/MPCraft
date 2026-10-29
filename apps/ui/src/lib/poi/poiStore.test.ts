import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/bridge", () => ({
  pois: vi.fn(),
  addPoi: vi.fn(),
  removePoi: vi.fn(),
  clearPois: vi.fn(),
}));

import { addPoi, clearPois, pois, removePoi } from "../ipc/bridge";
import { usePoiStore } from "./poiStore";

const gate = { name: "Gate", latitude: -35, longitude: 149 };
const shed = { name: "Shed", latitude: -35.1, longitude: 149.1 };

beforeEach(() => {
  usePoiStore.setState({ points: [] });
  vi.clearAllMocks();
});

describe("the places of interest", () => {
  it("are read from the settings when the window starts", async () => {
    vi.mocked(pois).mockResolvedValue([gate, shed]);

    await usePoiStore.getState().load();

    expect(usePoiStore.getState().points).toEqual([gate, shed]);
  });

  it("are kept as the backend stored them, not as they were typed", async () => {
    vi.mocked(addPoi).mockResolvedValue([gate]);

    await usePoiStore.getState().add("Gate", -35, 149);

    expect(addPoi).toHaveBeenCalledWith("Gate", -35, 149);
    expect(usePoiStore.getState().points).toEqual([gate]);
  });

  it("are taken off the map by the answer of the backend", async () => {
    usePoiStore.setState({ points: [gate, shed] });
    vi.mocked(removePoi).mockResolvedValue([shed]);

    await usePoiStore.getState().remove(gate);

    expect(usePoiStore.getState().points).toEqual([shed]);
  });

  it("are all taken off the map at once", async () => {
    usePoiStore.setState({ points: [gate, shed] });
    vi.mocked(clearPois).mockResolvedValue([]);

    await usePoiStore.getState().clear();

    expect(usePoiStore.getState().points).toEqual([]);
  });
});
