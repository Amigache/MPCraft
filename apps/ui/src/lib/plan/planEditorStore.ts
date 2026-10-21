import { create } from "zustand";

import {
  exportPlan,
  importPlan,
  normaliseError,
  uploadMission,
  validateMission,
} from "../ipc/bridge";
import type {
  CommandError,
  MavCommand,
  PlanItem,
  PlanKind,
  PlanWarning,
} from "../ipc/types";
import { useCommandStore } from "./commandStore";
import {
  RALLY_POINT,
  VERTEX_EXCLUSION,
  VERTEX_INCLUSION,
} from "./fence";
import type { PlanFileFormat } from "./planFiles";
import {
  insertItem,
  moveItem,
  patchItem,
  removeItem,
  renumber,
  sameItems,
} from "./planItems";
import { usePlanStore } from "./planStore";
import { circlePoints } from "./circle";
import { surveyPoints } from "./survey";

/**
 * The plan that is being edited.
 *
 * The plan the vehicle holds and the plan the operator is editing are **not** the same
 * thing: the edits are not on the vehicle until they are uploaded. The edited plan is
 * therefore kept here, apart from the plan in the store, and the interface says so when
 * the two differ.
 *
 * The store owns the decisions — what an edit does, when the operator is asked to
 * confirm, what a warning means — and the components only draw what it says.
 */

/** What the editor is waiting for the operator to confirm. */
export type PendingAction =
  | {
      readonly kind: "upload";
      /** What the backend says is wrong with the plan. */
      readonly warnings: readonly PlanWarning[];
    }
  | { readonly kind: "download" }
  | {
      readonly kind: "import";
      readonly path: string;
      readonly format: PlanFileFormat;
    };

/** What the editor is adding when the operator clicks on the map. */
export type AddKind = "waypoint" | "vertex" | "exclusion" | "rally";

/** The name the specification gives the waypoint. */
const WAYPOINT_COMMAND = "MAV_CMD_NAV_WAYPOINT";

/** The frame a new item is given when there is no item before it. */
const DEFAULT_FRAME = 3;

export interface PlanEditorStore {
  /** Whether the editor is open. */
  readonly open: boolean;
  /** The items as they are being edited. */
  readonly draft: readonly PlanItem[];
  /** The items as the vehicle holds them, for the comparison that says "changed". */
  readonly baseline: readonly PlanItem[];
  /** The item the operator has selected, by its place in the plan. */
  readonly selected: number | null;
  /** Whether the next click on the map adds an item. */
  readonly adding: boolean;
  /** Says what the next click on the map adds. */
  readonly setAddKind: (kind: AddKind) => void;


  /** What the next click on the map adds. */
  readonly addKind: AddKind;
  /** How many items of the plan the map has given a marker. */
  readonly marked: number;
  /** How many items of the plan carry a place at all. */
  readonly places: number;
  /** Whether the editor is waiting for the backend. */
  readonly busy: boolean;
  /** The last failure, with its stable code. */
  readonly error: CommandError | null;
  /** The last thing that worked. */
  readonly notice: string | null;
  /** The confirmation the editor is waiting for. */
  readonly pending: PendingAction | null;
  /** The plans that were changed, so that a change can be taken back. */
  readonly history: readonly (readonly PlanItem[])[];
  /** The plans that were taken back, so that they can be done again. */
  readonly future: readonly (readonly PlanItem[])[];

  /** Opens the editor. */
  readonly openEditor: () => void;
  /** Closes the editor. */
  readonly closeEditor: () => void;
  /** Opens or closes the editor. */
  readonly toggleEditor: () => void;
  /** Selects an item, or none. */
  readonly select: (seq: number | null) => void;
  /** Arms or disarms the adding of an item by a click on the map. */
  readonly setAdding: (adding: boolean) => void;
  /** Says how many items of the plan the map has marked. */
  readonly setMarked: (marked: number, places: number) => void;
  /** Changes one item of the plan. */
  readonly change: (seq: number, patch: Partial<Omit<PlanItem, "seq">>) => void;
  /** Moves an item up or down the plan. */
  readonly move: (seq: number, delta: number) => void;
  /** Removes an item from the plan. */
  readonly remove: (seq: number) => void;

  /** Adds a circle of a survey to the plan. */
  readonly generateCircle: (
    centre: { lat: number; lng: number },
    radius: number,
    points: number,
    altitude: number,
    clockwise: boolean,
  ) => void;
  /** Adds a reticle of a survey to the plan. */
  readonly generateSurvey: (
    centre: { lat: number; lng: number },
    length: number,
    spacing: number,
    strips: number,
    heading: number,
    altitude: number,
  ) => void;
  /** Adds an item at a position, with the altitude of the item before it. */
  readonly addAt: (latitude: number, longitude: number) => void;
  /** Asks the backend what is wrong with the plan, and waits for the operator. */
  readonly askUpload: () => Promise<void>;
  /** Asks whether the plan on the vehicle may be read again. */
  readonly askDownload: () => Promise<void>;
  /** Asks whether the edits may be thrown away for a file. */
  readonly askImport: (path: string, format: PlanFileFormat) => Promise<void>;
  /** Carries out the action the operator has confirmed. */
  readonly confirm: () => Promise<void>;
  /** Gives up on the action the editor was waiting for. */
  readonly cancel: () => void;
  /** Writes the plan to a file. */
  readonly saveToFile: (path: string, format: PlanFileFormat) => Promise<void>;
  /** Takes the last change back. */
  readonly undo: () => void;
  /** Does again the change that was taken back. */
  readonly redo: () => void;
  /** Forgets the last failure. */
  readonly dismissError: () => void;
}

/** How many changes are remembered. */
const HISTORY_LIMIT = 50;

/**
 * Records a change, so that it can be taken back.
 *
 * The plan that was replaced goes on the stack, and the stack of things that were taken
 * back is emptied: a change made after a change that was taken back means the change that
 * was taken back is not coming back.
 */
function withHistory(
  state: { draft: readonly PlanItem[]; history: readonly (readonly PlanItem[])[] },
  draft: readonly PlanItem[],
) {
  return {
    draft,
    history: [...state.history, state.draft].slice(-HISTORY_LIMIT),
    future: [] as readonly (readonly PlanItem[])[],
  };
}

/** The kind of plan the editor is working on. */
function kindOf(): PlanKind {
  return usePlanStore.getState().kind;
}

/** Whether the edited plan differs from the plan the vehicle holds. */
export function isChanged(state: {
  readonly draft: readonly PlanItem[];
  readonly baseline: readonly PlanItem[];
}): boolean {
  return !sameItems(state.draft, state.baseline);
}

/**
 * The command of the item that is added, when the catalogue knows one.
 *
 * What is added depends on the plan: a mission takes waypoints, a fence takes the
 * vertices of its polygon, and a rally takes the points the vehicle may come back to.
 */
function commandFor(
  kind: AddKind,
  commands: readonly MavCommand[],
): MavCommand | undefined {
  const name = {
    waypoint: WAYPOINT_COMMAND,
    vertex: VERTEX_INCLUSION,
    exclusion: VERTEX_EXCLUSION,
    rally: RALLY_POINT,
  }[kind];

  return commands.find((command) => command.canonicalName === name);
}

/**
 * The kind of item that the plan of this kind takes.
 *
 * A fence is a polygon, and its points are its vertices. A rally takes rally points.
 */
export function addKindFor(kind: PlanKind): AddKind {
  switch (kind) {
    case "fence":
      return "vertex";
    case "rally":
      return "rally";
    default:
      return "waypoint";
  }
}

/** The place an item goes when it is inserted after another one. */
function insertionIndex(
  items: readonly PlanItem[],
  after: number | null,
): number {
  if (after === null) {
    return items.length;
  }
  const index = items.findIndex((item) => item.seq === after);
  return index < 0 ? items.length : index + 1;
}

export const usePlanEditorStore = create<PlanEditorStore>((set, get) => ({
  open: false,
  draft: [],
  baseline: [],
  selected: null,
  adding: false,
  addKind: "waypoint",
  marked: 0,
  places: 0,
  busy: false,
  error: null,
  notice: null,
  pending: null,
  history: [],
  future: [],

  openEditor: () => {
    void useCommandStore.getState().load();

    // Edits that have not been sent are kept: throwing them away because the editor was
    // closed would be a ground station that loses work.
    if (isChanged(get())) {
      set({
        open: true,
        adding: false,
        pending: null,
        error: null,
        notice: null,
      });
      return;
    }

    const plan = usePlanStore.getState().plans[kindOf()];
    if (plan === null || plan === undefined) {
      set({
        open: true,
        adding: false,
        pending: null,
        error: null,
        notice: null,
      });
      void readFromVehicle();
      return;
    }

    set({
      open: true,
      adding: false,
      pending: null,
      error: null,
      notice: null,
      draft: renumber(plan.items),
      baseline: plan.items,
      selected: null,
    });
  },

  closeEditor: () =>
    set({ open: false, adding: false, pending: null, error: null }),

  toggleEditor: () => {
    if (get().open) {
      get().closeEditor();
    } else {
      get().openEditor();
    }
  },

  select: (selected) => set({ selected }),

  setAdding: (adding) => set({ adding }),
  setAddKind: (kind) => set({ addKind: kind }),

  setMarked: (marked, places) =>
    set((state) =>
      state.marked === marked && state.places === places
        ? {}
        : { marked, places },
    ),

  change: (seq, patch) =>
    set((state) => withHistory(state, patchItem(state.draft, seq, patch))),

  move: (seq, delta) =>
    set((state) => {
      const index = state.draft.findIndex((item) => item.seq === seq);
      const target = index + delta;

      if (index < 0 || target < 0 || target >= state.draft.length) {
        return {};
      }

      const draft = moveItem(state.draft, seq, delta);
      let selected = state.selected;

      if (selected !== null) {
        if (selected === index) {
          selected = target;
        } else if (index < selected && selected <= target) {
          selected -= 1;
        } else if (target <= selected && selected < index) {
          selected += 1;
        }
      }

      return { ...withHistory(state, draft), selected };
    }),

  remove: (seq) =>
    set((state) => {
      const index = state.draft.findIndex((item) => item.seq === seq);
      if (index < 0) {
        return {};
      }

      const draft = removeItem(state.draft, seq);
      let selected = state.selected;

      if (selected !== null) {
        if (selected === index) {
          selected = null;
        } else if (selected > index) {
          selected -= 1;
        }
      }

      return { ...withHistory(state, draft), selected };
    }),

  generateSurvey: (centre, length, spacing, strips, heading, altitude) =>
    set((state) => {
      const generated = surveyPoints({
        centre,
        length,
        spacing,
        strips,
        heading,
        altitude,
      });

      if (generated.length === 0) {
        return { notice: "The survey needs at least one strip and a spacing." };
      }

      // The reticle is added to the plan and nothing is thrown away: the operator can
      // take it back with the same key that takes back any other change.
      const draft = [...state.draft, ...generated];

      return {
        ...withHistory(state, renumber(draft)),
        selected: state.draft.length,
        notice: `${generated.length} points were added to the plan.`,
      };
    }),

  generateCircle: (centre, radius, points, altitude, clockwise) =>
    set((state) => {
      const generated = circlePoints({
        centre,
        radius,
        points,
        altitude,
        clockwise,
      });

      if (generated.length === 0) {
        return { notice: "The circle needs at least three points and a radius." };
      }

      // The circle is added to the plan and nothing is thrown away: the operator can
      // take it back with the same key that takes back any other change.
      const draft = [...state.draft, ...generated];

      return {
        ...withHistory(state, renumber(draft)),
        selected: state.draft.length,
        notice: `${generated.length} points were added to the plan.`,
      };
    }),

  addAt: (latitude, longitude) => {
    const state = get();
    const command = commandFor(
      state.addKind,
      useCommandStore.getState().commands,
    );
    if (command === undefined) {
      set({
        error: {
          code: "unknownCommand",
          message: "The catalogue of commands has not been read yet.",
        },
      });
      return;
    }

    const at = insertionIndex(state.draft, state.selected);
    const before = state.draft[at - 1] ?? null;

    // The new item takes the altitude of the item before it: a waypoint that jumps to
    // another height is a waypoint the operator did not mean.
    const item: PlanItem = {
      seq: 0,
      command: command.canonicalName,
      commandValue: command.value,
      frame: before?.frame ?? DEFAULT_FRAME,
      params: [0, 0, 0, 0],
      x: latitude,
      y: longitude,
      z: before?.z ?? 0,
      current: false,
      autocontinue: true,
    };

    const draft = insertItem(state.draft, state.selected, item);

    set({
      draft,
      selected: at,
      notice: null,
    });
  },

  askUpload: async () => {
    const draft = get().draft;
    if (draft.length === 0) {
      set({
        error: {
          code: "emptyPlan",
          message: "There is nothing in the plan to upload.",
        },
      });
      return;
    }

    set({ busy: true, error: null });
    try {
      const warnings = await validateMission(draft, kindOf());
      set({ busy: false, pending: { kind: "upload", warnings } });
    } catch (cause) {
      set({ busy: false, error: normaliseError(cause) });
    }
  },

  askDownload: async () => {
    set({ pending: { kind: "download" } });
  },

  askImport: async (path, format) => {
    set({ pending: { kind: "import", path, format } });
  },

  confirm: async () => {
    const pending = get().pending;
    if (pending === null) {
      return;
    }

    set({ pending: null, busy: true, error: null });

    if (pending.kind === "upload") {
      const items = get().draft;
      try {
        await uploadMission(items, kindOf());
        set({
          busy: false,
          baseline: items,
          notice: `The plan was uploaded: ${items.length} item${
            items.length === 1 ? "" : "s"
          }.`,
        });
      } catch (cause) {
        set({ busy: false, error: normaliseError(cause) });
      }
      return;
    }

    if (pending.kind === "download") {
      await readFromVehicle();
      return;
    }

    try {
      const plan = await importPlan(pending.format, pending.path);
      set({
        busy: false,
        draft: renumber(plan.items),
        selected: null,
        notice: `The plan was read from ${pending.path}.`,
      });
    } catch (cause) {
      set({ busy: false, error: normaliseError(cause) });
    }
  },

  cancel: () => set({ pending: null }),

  saveToFile: async (path, format) => {
    set({ busy: true, error: null });
    try {
      await exportPlan(get().draft, kindOf(), format, path);
      set({ busy: false, notice: `The plan was written to ${path}.` });
    } catch (cause) {
      set({ busy: false, error: normaliseError(cause) });
    }
  },

  undo: () =>
    set((state) => {
      const previous = state.history[state.history.length - 1];
      if (previous === undefined) {
        return {};
      }

      return {
        draft: previous,
        history: state.history.slice(0, -1),
        future: [state.draft, ...state.future].slice(0, HISTORY_LIMIT),
        selected: null,
      };
    }),

  redo: () =>
    set((state) => {
      const next = state.future[0];
      if (next === undefined) {
        return {};
      }

      return {
        draft: next,
        history: [...state.history, state.draft].slice(-HISTORY_LIMIT),
        future: state.future.slice(1),
        selected: null,
      };
    }),

  dismissError: () => set({ error: null }),
}));

/**
 * Reads the plan from the vehicle and puts it in the editor.
 *
 * The plan the vehicle holds is also put in the store, so that the map and the rest of
 * the window show the same plan the editor does.
 */
async function readFromVehicle(): Promise<void> {
  const kind = kindOf();

  usePlanEditorStore.setState({ busy: true, error: null });

  const plan = await usePlanStore.getState().download(kind);
  if (plan === null) {
    usePlanEditorStore.setState({
      busy: false,
      error: usePlanStore.getState().error,
    });
    return;
  }

  usePlanEditorStore.setState({
    busy: false,
    draft: renumber(plan.items),
    baseline: plan.items,
    selected: null,
    notice: `The plan was read from the vehicle: ${plan.items.length} item${
      plan.items.length === 1 ? "" : "s"
    }.`,
  });
}

/**
 * The editor follows the kind of plan the window is working on.
 *
 * The actions tab chooses the kind — mission, fence or rally — and the editor edits the
 * plan of that kind. When the kind changes, the plan being edited is another plan, so it
 * is replaced.
 */
usePlanStore.subscribe((state, previous) => {
  if (state.kind === previous.kind) {
    return;
  }

  const editor = usePlanEditorStore.getState();

  if (!editor.open) {
    return;
  }

  // A plan with edits that have not been sent is work, and the work is not lost
  // because the operator looked at another plan.
  if (isChanged(editor)) {
    usePlanEditorStore.setState({
      notice:
        "The plan being edited has changes that have not been sent. They are still here."
    });
    return;
  }

  const plan = state.plans[state.kind];

  if (plan === null || plan === undefined) {
    usePlanEditorStore.setState({
      draft: [],
      baseline: [],
      selected: null,
      adding: false,
      pending: null,
    });
    void readFromVehicle();
    return;
  }

  usePlanEditorStore.setState({
    draft: renumber(plan.items),
    baseline: plan.items,
    selected: null,
    adding: false,
    pending: null,
  });
});
