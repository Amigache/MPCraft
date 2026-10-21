import { beforeEach, describe, expect, it, vi } from "vitest";

import type { MavCommand, PlanItem } from "../ipc/types";

/**
 * The backend is stubbed: the store is about what the interface does with a plan, not
 * about how the plan travels.
 */
vi.mock("../ipc/bridge", () => ({
  exportPlan: vi.fn(async () => undefined),
  importPlan: vi.fn(async () => ({ kind: "mission", items: [] })),
  listMavCommands: vi.fn(async () => []),
  normaliseError: (cause: unknown) => ({
    code: "internal",
    message: String(cause),
  }),
  uploadMission: vi.fn(async () => undefined),
  validateMission: vi.fn(async () => []),
}));

const { usePlanEditorStore, isChanged } = await import("./planEditorStore");
const { usePlanStore } = await import("./planStore");
const { useCommandStore } = await import("./commandStore");
const { uploadMission, validateMission } = await import("../ipc/bridge");

/** The waypoint of the catalogue, for the test. */
const WAYPOINT: MavCommand = {
  name: "Waypoint",
  value: 16,
  roles: ["empty", "empty", "empty", "other", "latitude", "longitude", "altitude"],
  units: ["", "", "", "", "°", "°", "m"],
};

/** One item of a plan, for the test. */
function item(seq: number, overrides: Partial<PlanItem> = {}): PlanItem {
  return {
    seq,
    command: "MAV_CMD_NAV_WAYPOINT",
    commandValue: 16,
    frame: 3,
    params: [0, 0, 0, 0],
    x: -35.36 + seq * 0.001,
    y: 149.16,
    z: 100,
    current: false,
    autocontinue: true,
    ...overrides,
  };
}

/** The plan the vehicle holds, for the test. */
const VEHICLE_PLAN = {
  kind: "mission" as const,
  items: [item(0, { z: 100 }), item(1, { z: 80 }), item(2, { z: 60 })],
};

beforeEach(() => {
  vi.clearAllMocks();

  useCommandStore.setState({ commands: [WAYPOINT], loaded: true, error: null });
  usePlanStore.setState({
    plans: { mission: VEHICLE_PLAN, fence: null, rally: null },
    kind: "mission",
    busy: false,
    error: null,
  });
  usePlanEditorStore.setState({
    open: false,
    draft: [],
    baseline: [],
    selected: null,
    adding: false,
    marked: 0,
    places: 0,
    busy: false,
    error: null,
    notice: null,
    pending: null,
  });
});

describe("the plan that is being edited", () => {
  it("starts as the plan the vehicle holds", () => {
    usePlanEditorStore.getState().openEditor();

    const state = usePlanEditorStore.getState();
    expect(state.open).toBe(true);
    expect(state.draft).toHaveLength(3);
    expect(isChanged(state)).toBe(false);
  });

  it("is not the plan the vehicle holds once it is changed", () => {
    usePlanEditorStore.getState().openEditor();
    usePlanEditorStore.getState().change(1, { z: 250 });

    const state = usePlanEditorStore.getState();
    expect(state.draft[1]?.z).toBe(250);
    expect(isChanged(state)).toBe(true);
  });

  it("keeps the edits when the editor is closed and opened again", () => {
    usePlanEditorStore.getState().openEditor();
    usePlanEditorStore.getState().change(1, { z: 250 });
    usePlanEditorStore.getState().closeEditor();
    usePlanEditorStore.getState().openEditor();

    expect(usePlanEditorStore.getState().draft[1]?.z).toBe(250);
  });

  it("is the same as the vehicle again after the plan is uploaded", async () => {
    usePlanEditorStore.getState().openEditor();
    usePlanEditorStore.getState().change(1, { z: 250 });

    await usePlanEditorStore.getState().askUpload();
    expect(usePlanEditorStore.getState().pending?.kind).toBe("upload");

    await usePlanEditorStore.getState().confirm();

    const state = usePlanEditorStore.getState();
    expect(isChanged(state)).toBe(false);
    expect(uploadMission).toHaveBeenCalledTimes(1);
  });
});

describe("the order of the plan", () => {
  it("moves an item and keeps the selection on it", () => {
    usePlanEditorStore.getState().openEditor();
    usePlanEditorStore.getState().select(0);
    usePlanEditorStore.getState().move(0, 1);

    const state = usePlanEditorStore.getState();
    expect(state.draft.map((entry) => entry.seq)).toEqual([0, 1, 2]);
    expect(state.selected).toBe(1);
  });

  it("removes an item and closes the gap", () => {
    usePlanEditorStore.getState().openEditor();
    usePlanEditorStore.getState().remove(1);

    const state = usePlanEditorStore.getState();
    expect(state.draft).toHaveLength(2);
    expect(state.draft.map((entry) => entry.seq)).toEqual([0, 1]);
  });

  it("adds an item with the altitude of the item before it", () => {
    usePlanEditorStore.getState().openEditor();
    usePlanEditorStore.getState().addAt(-35.4, 149.2);

    const state = usePlanEditorStore.getState();
    expect(state.draft).toHaveLength(4);
    expect(state.draft[3]?.z).toBe(60);
    expect(state.draft[3]?.x).toBe(-35.4);
    expect(state.draft[3]?.y).toBe(149.2);
    expect(state.draft[3]?.commandValue).toBe(16);
  });

  it("adds the item after the one that is selected", () => {
    usePlanEditorStore.getState().openEditor();
    usePlanEditorStore.getState().select(0);
    usePlanEditorStore.getState().addAt(-35.4, 149.2);

    const state = usePlanEditorStore.getState();
    expect(state.draft).toHaveLength(4);
    expect(state.draft[1]?.x).toBe(-35.4);
    expect(state.draft[1]?.z).toBe(100);
  });
});

describe("what the operator is asked to confirm", () => {
  it("asks before the plan is uploaded", async () => {
    usePlanEditorStore.getState().openEditor();
    await usePlanEditorStore.getState().askUpload();

    const pending = usePlanEditorStore.getState().pending;
    expect(pending?.kind).toBe("upload");
    expect(uploadMission).not.toHaveBeenCalled();
  });

  it("shows what the backend says is wrong with the plan", async () => {
    vi.mocked(validateMission).mockResolvedValueOnce([
      { seq: 2, reason: "zeroAltitude" },
    ]);

    usePlanEditorStore.getState().openEditor();
    await usePlanEditorStore.getState().askUpload();

    const pending = usePlanEditorStore.getState().pending;
    expect(pending?.kind).toBe("upload");
    if (pending?.kind === "upload") {
      expect(pending.warnings).toEqual([{ seq: 2, reason: "zeroAltitude" }]);
    }
  });

  it("asks before the plan on the vehicle is read again", async () => {
    usePlanEditorStore.getState().openEditor();
    usePlanEditorStore.getState().change(1, { z: 250 });
    usePlanEditorStore.getState().askDownload();

    expect(usePlanEditorStore.getState().pending?.kind).toBe("download");
  });

  it("gives up on the action when the operator refuses", () => {
    usePlanEditorStore.getState().openEditor();
    usePlanEditorStore.getState().askDownload();
    usePlanEditorStore.getState().cancel();

    expect(usePlanEditorStore.getState().pending).toBeNull();
  });
});
