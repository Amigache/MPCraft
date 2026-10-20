/**
 * What the window shows when the interface cannot be drawn.
 *
 * A ground station that fails silently is worse than one that says why. React reports
 * the errors it cannot handle through the options of the root, and they end up here,
 * where the operator can read them.
 */

/** Shows a failure in the window. */
export function reportFailure(error: unknown): void {
  const message =
    error instanceof Error
      ? `${error.name}: ${error.message}${error.stack ? `\n${error.stack}` : ""}`
      : typeof error === "string"
        ? error
        : JSON.stringify(error);

  const root = document.getElementById("root");
  if (!root) {
    return;
  }

  const panel = document.createElement("div");
  panel.setAttribute("role", "alert");
  panel.style.cssText = [
    "margin: 24px",
    "padding: 16px",
    "border: 1px solid #6d2b28",
    "border-radius: 6px",
    "background: rgba(248, 81, 73, 0.14)",
    "color: #f85149",
    "font-family: ui-monospace, Consolas, monospace",
    "font-size: 12px",
    "line-height: 1.5",
    "white-space: pre-wrap",
  ].join(";");

  panel.textContent = `MPCraft could not draw the interface.\n\n${message}`;

  // The console of the web view is the only place a developer can read the stack.
  console.error(
    `MPCraft could not draw the interface. ${String(
      error instanceof Error ? (error.stack ?? error.message) : error,
    ).replace(/\s*\n\s*/g, " | ")}`,
  );

  root.replaceChildren(panel);
}
