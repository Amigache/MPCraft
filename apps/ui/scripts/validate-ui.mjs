/**
 * Drives the running application and checks what the map and the window show.
 *
 * The window is a WebView2, so the page inside it can be reached over the Chrome DevTools
 * protocol: start the application with
 *
 *     $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222"
 *
 * and the development server running on 1420, then run this script. It connects to the vehicle
 * on UDP 14550 — `scripts/simulated-vehicle.py --port 14550` is one — and reads the state of the
 * map through the handle a development build leaves on `window.__mpcraftMap`.
 *
 * The selectors go by structure rather than by label, because the window may be read in any of
 * the languages the bundles carry.
 *
 * It is not a gate: it needs a window, a vehicle and a display, none of which a runner has.
 */

import { chromium } from "playwright";

const CDP = process.env.MPCRAFT_CDP ?? "http://127.0.0.1:9222";
const VEHICLE = process.env.MPCRAFT_UDP ?? "127.0.0.1:14550";

const results = [];
const check = (name, ok, detail) => {
  results.push({ name, ok, detail });
  console.log(
    `${ok ? "ok  " : "FAIL"} ${name}${detail === undefined ? "" : ` - ${detail}`}`,
  );
};

const browser = await chromium.connectOverCDP(CDP);
const page = browser
  .contexts()
  .flatMap((context) => context.pages())
  .find((candidate) => candidate.url().includes("localhost:1420"));

if (!page) {
  throw new Error("the window is not on the development server");
}

page.on("pageerror", (error) => console.log("page error:", error.message));

// A run starts from a window that has just been opened: the adding mode, the screen and the boxes
// of a previous run are not part of what this one measures. The link itself is the backend's, and
// it is closed below.
await page.reload({ waitUntil: "domcontentloaded" });

/** The state the map holds, read through the handle a development build leaves. */
const mapState = () =>
  page.evaluate(async () => {
    const map = window.__mpcraftMap;
    const count = async (id) => {
      const source = map.getSource(id);
      if (!source || typeof source.getData !== "function") {
        return null;
      }
      const data = await source.getData();
      if (data.type === "FeatureCollection") {
        return data.features.length;
      }
      return data.geometry.coordinates.length;
    };

    return {
      zoom: Number(map.getZoom().toFixed(3)),
      centre: map
        .getCenter()
        .toArray()
        .map((value) => Number(value.toFixed(4))),
      plan: await count("plan"),
      items: await count("plan-items"),
      circles: await count("plan-circles"),
      course: await count("course"),
      target: await count("target"),
    };
  });

await page.waitForFunction(
  () => window.__mpcraftMap && window.__mpcraftMap.loaded(),
  null,
  { timeout: 30000 },
);

const before = await mapState();
check("the map is drawn", before.zoom > 0, `zoom ${before.zoom}`);

const bar = page.locator('[role="menubar"]');

// A run that follows one that left the link open starts from a known state: the bar disables its
// controls while the link is open, so a linked window is closed before the way of connecting is
// chosen.
if (await bar.locator("select").first().isDisabled()) {
  await bar.locator("button").last().click();
  await page.waitForFunction(
    () => !document.querySelector('[role="menubar"] select')?.disabled,
    null,
    { timeout: 15000 },
  );
  await page.waitForTimeout(1500);
}

// A link that is not open draws no vehicle at all.
const idleMarkers = await page
  .locator(".maplibregl-marker:not([hidden])")
  .count();
check(
  "no vehicle is drawn before the link is open",
  idleMarkers === 0,
  `${idleMarkers} marks`,
);

// Connect to the vehicle over UDP: the first two selects of the bar are the way of connecting and
// the port, and the only input is the address the bar writes when the way is not a serial port.
await bar.locator("select").first().selectOption("udp");
await bar.locator("input").fill(VEHICLE);
await bar.locator("button").last().click();

await page.waitForFunction(
  () => /\d+ veh[ií]culos? en el enlace/.test(document.body.innerText),
  null,
  { timeout: 30000 },
);
await page.waitForTimeout(600);

const said = await page.evaluate(() => document.body.innerText);
const counted = said.match(/(\d+) veh[ií]culos? en el enlace/);
check(
  "the link says a vehicle is on it",
  counted !== null && Number(counted[1]) > 0,
  counted?.[0],
);

await page.waitForTimeout(2500);

// A plan that has not arrived on its own is asked for, which is what an operator does on the plan
// screen when the vehicle holds one and the window shows none.
if (((await mapState()).plan ?? 0) < 2) {
  const downloadPlan = page
    .getByRole("button", { name: /bajar el plan/i })
    .first();
  if (await downloadPlan.count()) {
    await downloadPlan.click();

    // Reading a plan over the plan the operator may have edited is asked first.
    const confirm = page.locator('[role="alertdialog"] button.button--primary');
    if (await confirm.count()) {
      await confirm.click();
    }

    await page.waitForTimeout(4000);
  }
}

// A vehicle may hold no plan at all — ArduPilot starts with none — and then there is nothing to
// arrive and no item to fly to. What is not allowed is a plan that is in the vehicle and not on the
// screen, which is why the expectation is said rather than assumed.
const expectPlan = process.env.MPCRAFT_EXPECT_PLAN !== "0";

const connected = await mapState();
check(
  expectPlan ? "the plan arrived from the vehicle" : "the vehicle holds no plan yet",
  expectPlan ? (connected.plan ?? 0) >= 2 : (connected.plan ?? 0) === 0,
  `${connected.plan} points`,
);
check(
  "the heading line is drawn",
  (connected.course ?? 0) === 2,
  `${connected.course} points`,
);
check(
  "the line to the current item is drawn",
  expectPlan ? (connected.target ?? 0) === 2 : true,
  expectPlan ? `${connected.target} points` : "no plan, so no leg to draw",
);

const footer = await page.locator("footer").innerText();
check(
  "the window reports no fault on a link with a vehicle",
  !footer.includes("no_vehicle") && !footer.includes("no vehicle"),
  footer.replace(/\s+/g, " ").slice(0, 80),
);

const marks = await page.locator(".maplibregl-marker:not([hidden])").count();
check("the vehicle is drawn on the map", marks > 0, `${marks} marks`);

// The plan is edited on the plan screen: a press adds an item, and it must not move the view.
await page.getByRole("tab").nth(1).click();
await page.waitForTimeout(800);
await page.getByRole("button", { name: /a[nñ]adir punto/i }).click();

const canvas = page.locator(".maplibregl-canvas");
const box = await canvas.boundingBox();

// The presses are far apart and away from the middle, where the vehicle and the items of the plan
// already are: a press on a mark selects that mark, which is the map working, not the test.
for (const [index, offset] of [
  [0, { x: -150, y: -90 }],
  [1, { x: 150, y: 110 }],
]) {
  // The view is read before each press, because a fit that was already running when the last
  // press was measured is not the press moving the map.
  await page.waitForTimeout(600);
  const from = await mapState();

  // The press goes to the window the way an operator's does, rather than to the canvas element:
  // the map puts its own layers over the canvas, and a press is a press of the map.
  await page.mouse.click(
    Math.round(box.x + box.width / 2 + offset.x),
    Math.round(box.y + box.height / 2 + offset.y),
  );
  await page.waitForTimeout(900);

  const after = await mapState();
  check(
    `a press that adds an item (${index + 1}) does not move the view`,
    Math.abs(after.zoom - from.zoom) < 0.001 &&
      Math.abs(after.centre[0] - from.centre[0]) < 0.0005 &&
      Math.abs(after.centre[1] - from.centre[1]) < 0.0005,
    `zoom ${from.zoom} -> ${after.zoom}, centre ${from.centre} -> ${after.centre}`,
  );
  check(
    `the plan grows by an item (${index + 1})`,
    (after.items ?? 0) > (from.items ?? 0),
    `${from.items} -> ${after.items}`,
  );
}

const edited = await mapState();
check(
  "the line of the plan is drawn between the items",
  (edited.plan ?? 0) >= 2,
  `${edited.plan} points`,
);

const shot = `${process.env.TEMP ?? "."}/mpcraft-map-after-editing.png`;
await page.screenshot({ path: shot });
console.log(`\nthe map after the edits: ${shot}`);

await browser.close();

const failures = results.filter((result) => !result.ok);
console.log(
  `\n${results.length - failures.length}/${results.length} checks passed`,
);
process.exit(failures.length === 0 ? 0 : 1);
