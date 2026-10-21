import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * The backend is stubbed: the store is about what the interface does with the
 * favourites, not about how they travel.
 */
vi.mock("../ipc/bridge", () => ({
  listFavouriteParameters: vi.fn(async () => []),
  normaliseError: (cause: unknown) => ({
    code: "internal",
    message: String(cause),
  }),
  setFavouriteParameter: vi.fn(async () => undefined),
}));

const { markFavourite, orderParameters, useFavouritesStore } = await import(
  "./favouritesStore"
);
const { listFavouriteParameters, setFavouriteParameter } = await import(
  "../ipc/bridge"
);

/** One parameter, for the test. */
function parameter(name: string): { readonly name: string } {
  return { name };
}

beforeEach(() => {
  vi.clearAllMocks();
  useFavouritesStore.setState({ favourites: [], loading: false, error: null });
});

describe("the order of the list", () => {
  it("puts the favourites first, in the order they were marked", () => {
    const ordered = orderParameters(
      [parameter("RTL_ALT"), parameter("ARMING_CHECK"), parameter("RTL_SPEED")],
      ["RTL_SPEED", "ARMING_CHECK"],
    );

    expect(ordered.map((entry) => entry.name)).toEqual([
      "RTL_SPEED",
      "ARMING_CHECK",
      "RTL_ALT",
    ]);
  });

  it("puts the rest by name", () => {
    const ordered = orderParameters(
      [parameter("RTL_ALT"), parameter("ARMING_CHECK"), parameter("FENCE_ENABLE")],
      [],
    );

    expect(ordered.map((entry) => entry.name)).toEqual([
      "ARMING_CHECK",
      "FENCE_ENABLE",
      "RTL_ALT",
    ]);
  });

  it("leaves a parameter that is not a favourite where it was", () => {
    const parameters = [parameter("RTL_ALT"), parameter("ARMING_CHECK")];

    expect(orderParameters(parameters, [])).not.toBe(parameters);
  });
});

describe("the mark of a favourite", () => {
  it("is put on the parameter", () => {
    expect(markFavourite([], "RTL_ALT", true)).toEqual(["RTL_ALT"]);
  });

  it("is taken off the parameter", () => {
    expect(markFavourite(["RTL_ALT", "FENCE_ENABLE"], "RTL_ALT", false)).toEqual([
      "FENCE_ENABLE",
    ]);
  });

  it("goes to the end when a parameter is marked again", () => {
    expect(markFavourite(["RTL_ALT", "FENCE_ENABLE"], "RTL_ALT", true)).toEqual([
      "FENCE_ENABLE",
      "RTL_ALT",
    ]);
  });
});

describe("the favourites of the operator", () => {
  it("are read when the sheet opens", async () => {
    vi.mocked(listFavouriteParameters).mockResolvedValueOnce([
      "RTL_ALT",
      "FENCE_ENABLE",
    ]);

    await useFavouritesStore.getState().load();

    expect(useFavouritesStore.getState().favourites).toEqual([
      "RTL_ALT",
      "FENCE_ENABLE",
    ]);
    expect(useFavouritesStore.getState().error).toBeNull();
  });

  it("are marked on the backend when a star is turned on", async () => {
    await useFavouritesStore.getState().toggle("RTL_ALT");

    expect(setFavouriteParameter).toHaveBeenCalledWith("RTL_ALT", true);
    expect(useFavouritesStore.getState().favourites).toEqual(["RTL_ALT"]);
  });

  it("are taken off the backend when a star is turned off", async () => {
    useFavouritesStore.setState({ favourites: ["RTL_ALT"] });

    await useFavouritesStore.getState().toggle("RTL_ALT");

    expect(setFavouriteParameter).toHaveBeenCalledWith("RTL_ALT", false);
    expect(useFavouritesStore.getState().favourites).toEqual([]);
  });

  it("keep the mark they had when the backend refuses", async () => {
    vi.mocked(setFavouriteParameter).mockRejectedValueOnce(
      new Error("the settings file could not be written"),
    );

    await useFavouritesStore.getState().toggle("RTL_ALT");

    const state = useFavouritesStore.getState();
    expect(state.favourites).toEqual([]);
    expect(state.error?.code).toBe("internal");
  });

  it("keep the order they were marked in", async () => {
    useFavouritesStore.setState({ favourites: ["RTL_ALT"] });

    await useFavouritesStore.getState().toggle("FENCE_ENABLE");

    expect(useFavouritesStore.getState().favourites).toEqual([
      "RTL_ALT",
      "FENCE_ENABLE",
    ]);
  });
});
