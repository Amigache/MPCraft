/**
 * The name of a file the vehicle holds, reduced to a file name.
 *
 * The path comes from the vehicle, and a path is not a file name: it can carry a directory, a Windows
 * separator, or the two names that mean the directory above. What the sheet suggests to the save dialog is
 * the last component, and a component that names nothing is said as `file` rather than passed on. Upstream
 * reduces the same value with `Path.GetFileName` before it writes a downloaded file; here the operator
 * still chooses the place, so this is the suggestion rather than the write, and the same rule keeps the
 * vehicle from choosing where the dialog starts.
 */
export function fileNameOf(path: string): string {
  const parts = path.split(/[\\/]/).filter((part) => part.length > 0);
  const name = parts[parts.length - 1] ?? "";

  if (name === "" || name === "." || name === "..") {
    return "file";
  }

  return name;
}
