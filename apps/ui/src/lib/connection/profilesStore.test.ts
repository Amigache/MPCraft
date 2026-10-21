import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * The backend is stubbed: the store is about what the interface does with the links the
 * operator has named, not about how they are written.
 */
vi.mock("../ipc/bridge", () => ({
  listConnectionProfiles: vi.fn(async () => []),
  setConnectionProfiles: vi.fn(async () => undefined),
}));

const { useProfilesStore } = await import("./profilesStore");
const { listConnectionProfiles, setConnectionProfiles } = await import("../ipc/bridge");

beforeEach(() => {
  vi.clearAllMocks();

  useProfilesStore.setState({
    profiles: [],
    error: null,
    busy: false,
  });
});

describe("the links the operator has named", () => {
  it("are read from the backend", async () => {
    vi.mocked(listConnectionProfiles).mockResolvedValueOnce([
      { name: "SITL", target: "udp:127.0.0.1:14550", autoConnect: true },
    ]);

    await useProfilesStore.getState().load();

    const profiles = useProfilesStore.getState().profiles;

    expect(profiles).toHaveLength(1);
    expect(profiles[0]?.name).toBe("SITL");
    expect(profiles[0]?.autoConnect).toBe(true);
  });

  it("are saved under their name", async () => {
    await useProfilesStore.getState().save({
      name: "SITL",
      target: "udp:127.0.0.1:14550",
      autoConnect: false,
    });

    expect(setConnectionProfiles).toHaveBeenCalledWith([
      { name: "SITL", target: "udp:127.0.0.1:14550", autoConnect: false },
    ]);
    expect(useProfilesStore.getState().profiles).toHaveLength(1);
  });

  it("replace the one with the same name", async () => {
    useProfilesStore.setState({
      profiles: [
        { name: "SITL", target: "udp:127.0.0.1:14550", autoConnect: false },
      ],
    });

    await useProfilesStore.getState().save({
      name: "SITL",
      target: "serial:COM3:57600",
      autoConnect: false,
    });

    const profiles = useProfilesStore.getState().profiles;

    expect(profiles).toHaveLength(1);
    expect(profiles[0]?.target).toBe("serial:COM3:57600");
  });

  it("are forgotten by name", async () => {
    useProfilesStore.setState({
      profiles: [
        { name: "SITL", target: "udp:127.0.0.1:14550", autoConnect: false },
        { name: "Drone", target: "serial:COM3:57600", autoConnect: false },
      ],
    });

    await useProfilesStore.getState().remove("SITL");

    const profiles = useProfilesStore.getState().profiles;

    expect(profiles).toHaveLength(1);
    expect(profiles[0]?.name).toBe("Drone");
  });

  it("say whether they are opened at the launch", async () => {
    useProfilesStore.setState({
      profiles: [
        { name: "SITL", target: "udp:127.0.0.1:14550", autoConnect: false },
      ],
    });

    await useProfilesStore.getState().setAutoConnect("SITL", true);

    expect(useProfilesStore.getState().profiles[0]?.autoConnect).toBe(true);
  });

  it("say when the backend refuses", async () => {
    vi.mocked(setConnectionProfiles).mockRejectedValueOnce({
      code: "settings",
      message: "the settings file could not be written",
    });

    await useProfilesStore.getState().save({
      name: "SITL",
      target: "udp:127.0.0.1:14550",
      autoConnect: false,
    });

    const state = useProfilesStore.getState();

    expect(state.error?.code).toBe("settings");
    expect(state.busy).toBe(false);
  });
});
