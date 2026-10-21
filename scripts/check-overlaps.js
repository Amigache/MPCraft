/**
 * The overlap checker.
 *
 * Two buttons drawn on top of each other, or a label that does not fit its box, are the
 * two ways an interface quietly stops being usable. They are also the two things that are
 * hard to see in a screenshot and easy to measure.
 *
 * The function below is to be run **inside the page**. It returns what it finds, and
 * nothing is left behind.
 *
 * It is used from the browser console, from a driver that can evaluate JavaScript, and
 * from the WebView2 debugger of the running application:
 *
 *     const ws = ...;   // the debugger url of the application
 *
 * `scripts/check-overlaps.js` holds the same function, so that it can be injected into
 * the application without copying it by hand.
 */

/** The controls whose boxes matter: the things a person clicks or types into. */
const INTERACTIVE = [
  "button",
  "input",
  "select",
  "textarea",
  "a[href]",
  '[role="button"]',
  '[role="tab"]',
  '[role="checkbox"]',
  '[role="switch"]',
];

/** The elements whose text must not be cut off. */
const WITH_TEXT = ["button", "label", "span", "td", "th", "div", "p", "legend", "a", "li"];

/**
 * Finds the overlaps and the clipped text.
 *
 * @param {{width: number, height: number}} viewport the size the window is looked at
 * @returns {{clashes: string[], clipped: string[], overflowing: string[]}}
 */
export function checkOverlaps() {
  const visible = (element) => {
    const style = getComputedStyle(element);
    if (style.display === "none" || style.visibility === "hidden") {
      return false;
    }
    if (style.opacity === "0") {
      return false;
    }
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  };

  const describe = (element) => {
    const name =
      element.getAttribute("aria-label") ||
      element.textContent?.trim().slice(0, 40) ||
      element.getAttribute("placeholder") ||
      "";
    const classes = String(element.className || "").split(" ")[0];
    return `${element.tagName.toLowerCase()}${classes ? "." + classes : ""} "${name}"`;
  };

  // 1. Two controls on top of each other.
  //
  // A menu is a popup: it lies over what is under it, and that is what a popup is for.
  // Its items are left out, so that what is left is a layout fault and not a menu.
  const controls = Array.from(document.querySelectorAll(INTERACTIVE.join(",")))
    .filter(visible)
    .filter((element) => !element.closest('[role="menu"]'))
    // A sheet lies over the window, and the window under it is inert: its
    // controls cannot be reached, so they are not controls that can be
    // covered by another one.
    .filter((element) => !element.closest("[inert], [aria-hidden=\"true\"]"));

  const clashes = [];

  for (let i = 0; i < controls.length; i += 1) {
    for (let j = i + 1; j < controls.length; j += 1) {
      const a = controls[i].getBoundingClientRect();
      const b = controls[j].getBoundingClientRect();

      // A control inside another one is not a clash: it is a label in a field.
      if (controls[i].contains(controls[j]) || controls[j].contains(controls[i])) {
        continue;
      }

      const overlap =
        a.left < b.right - 1 &&
        b.left < a.right - 1 &&
        a.top < b.bottom - 1 &&
        b.top < a.bottom - 1;

      if (overlap) {
        clashes.push(`${describe(controls[i])}  ×  ${describe(controls[j])}`);
      }
    }
  }

  // 2. Text that does not fit.
  const clipped = Array.from(document.querySelectorAll(WITH_TEXT.join(",")))
    .filter(visible)
    .filter((element) => {
      if (element.children.length > 0) {
        return false;
      }
      const text = element.textContent?.trim();
      if (!text) {
        return false;
      }
      return (
        element.scrollWidth > element.clientWidth + 1 ||
        element.scrollHeight > element.clientHeight + 1
      );
    })
    .map(describe);

  // 3. An element that sticks out of the window.
  //
  // The inside of an SVG is left out: a drawing is drawn larger than its box and clipped
  // by the viewport, which is how a horizon is drawn, and it is not an overflow.
  const overflowing = Array.from(document.querySelectorAll("body *"))
    .filter(visible)
    .filter((element) => !element.closest("svg"))
    .filter((element) => {
      const rect = element.getBoundingClientRect();
      return (
        rect.right > window.innerWidth + 1 ||
        rect.bottom > window.innerHeight + 1 ||
        rect.left < -1
      );
    })
    .map(describe);

  return { clashes, clipped, overflowing };
}
