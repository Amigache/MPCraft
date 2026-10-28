import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * The bridge to the shell's dialogs, read for the shape of what it sends.
 *
 * The commands take their arguments under `options`, and the message dialog answers with the label
 * of the button that was pressed. A bridge that sent the arguments flat, or asked a command the
 * plugin does not have, was a dialog that never opened and an update that never asked; these tests
 * are the shape that keeps it from happening again.
 */
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { askQuestion, pickFolder, pickOpenPath, pickSavePath } from "./dialog";

beforeEach(() => {
  invoke.mockReset();
});

describe("the dialogs of the shell", () => {
  it("puts the options under the key the save command reads", async () => {
    invoke.mockResolvedValue("C:\\plans\\field.waypoints");

    const filters = [{ name: "Plan", extensions: ["waypoints"] }];
    const picked = await pickSavePath("plan.waypoints", filters);

    expect(invoke).toHaveBeenCalledWith("plugin:dialog|save", {
      options: {
        defaultPath: "plan.waypoints",
        filters,
        title: "Save the plan",
      },
    });
    expect(picked).toEqual({
      ok: true,
      path: "C:\\plans\\field.waypoints",
    });
  });

  it("asks the open command with the options key, for a file and for a folder", async () => {
    invoke.mockResolvedValue("C:\\plans\\field.waypoints");

    const filters = [{ name: "Plan", extensions: ["waypoints"] }];

    await pickOpenPath(filters);
    expect(invoke).toHaveBeenCalledWith("plugin:dialog|open", {
      options: {
        directory: false,
        filters,
        multiple: false,
        title: "Open the plan",
      },
    });

    await pickFolder();
    expect(invoke).toHaveBeenCalledWith("plugin:dialog|open", {
      options: {
        directory: true,
        multiple: false,
        title: "Open the folder",
      },
    });
  });

  it("says a cancelled or unavailable dialog rather than a path", async () => {
    invoke.mockResolvedValue(null);
    expect(await pickSavePath("plan.waypoints", [])).toEqual({
      ok: false,
      reason: "cancelled",
    });

    invoke.mockRejectedValue(new Error("no dialog"));
    expect(await pickOpenPath([])).toEqual({
      ok: false,
      reason: "unavailable",
    });
  });

  it("asks a question through the message command and reads its own label", async () => {
    invoke.mockResolvedValue("Yes");

    expect(await askQuestion("Install it?")).toBe(true);
    expect(invoke).toHaveBeenCalledWith("plugin:dialog|message", {
      message: "Install it?",
      title: "MPCraft",
      kind: "info",
      buttons: "YesNo",
    });
  });

  it("reads No, and a shell that cannot ask, as no", async () => {
    invoke.mockResolvedValue("No");
    expect(await askQuestion("Install it?")).toBe(false);

    invoke.mockRejectedValue(new Error("no dialog"));
    expect(await askQuestion("Install it?")).toBe(false);
  });
});
