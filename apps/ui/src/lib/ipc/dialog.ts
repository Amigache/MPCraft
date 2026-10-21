import { invoke } from "@tauri-apps/api/core";

/**
 * The file dialog of the desktop shell.
 *
 * The shell carries the dialog plugin, which asks the operating system for a file. The
 * plugin is not always there — a browser does not have it, and a build of the shell
 * without it does not either — so the caller is told when it is missing rather than
 * being left with a promise that never settles.
 *
 * The path may also be typed in, which is why nothing here is the only way to reach a
 * file.
 */

/** One kind of file the dialog offers. */
export interface DialogFilter {
  /** The name the kind is given in the dialog. */
  readonly name: string;
  /** The extensions of the kind, without the dot. */
  readonly extensions: readonly string[];
}

/** What asking for a file came to. */
export type PickedPath =
  | { readonly ok: true; readonly path: string }
  | { readonly ok: false; readonly reason: "cancelled" | "unavailable" };

/** Asks the shell for the name of a file to write to. */
export async function pickSavePath(
  defaultPath: string,
  filters: readonly DialogFilter[],
): Promise<PickedPath> {
  try {
    const path = await invoke<string | null>("plugin:dialog|save", {
      defaultPath,
      filters,
      title: "Save the plan",
    });

    return path === null
      ? { ok: false, reason: "cancelled" }
      : { ok: true, path };
  } catch {
    return { ok: false, reason: "unavailable" };
  }
}

/** Asks the shell for the name of a file to read from. */
export async function pickOpenPath(
  filters: readonly DialogFilter[],
): Promise<PickedPath> {
  try {
    const path = await invoke<string | null>("plugin:dialog|open", {
      directory: false,
      filters,
      multiple: false,
      title: "Open the plan",
    });

    return path === null
      ? { ok: false, reason: "cancelled" }
      : { ok: true, path };
  } catch {
    return { ok: false, reason: "unavailable" };
  }
}
