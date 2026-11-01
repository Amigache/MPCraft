/**
 * Flies a simulated vehicle into its place in a formation.
 *
 * The formation decision and the loop that dispatches it were tested against a scripted peer,
 * because a fleet could not be assembled on this machine: this is the harness that assembles one.
 * `docker/sitl` runs three ArduPilot vehicles on one link, the window speaks to one of them, the
 * other two are given places in the formation, and the loop is started from the sheet.
 *
 * What is asserted is the thing the scripted peer could not show: the vehicle the station speaks
 * to is armed, put in GUIDED, given a place twenty-five metres across and twenty above the leader,
 * and then observed to climb and to move there.
 *
 *     docker compose -f docker/sitl/compose.yaml up -d
 *     npm run dev                      # from apps/ui
 *     $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222"
 *     .\target\debug\mpcraft.exe
 *     node apps/ui/scripts/formation-live.mjs
 *
 * The leader stays on the ground and disarmed: its place is what the others are measured from, and
 * a leader that flies away would take the whole formation with it.
 *
 * What this run has shown so far, and what it has not:
 *
 * * the station sees three vehicles, lists them by their pair, speaks to the one that is chosen,
 *   arms it, puts it in GUIDED, and the sheet takes a place for it — all of that passes;
 * * the loop reports one order in every tick, which is the dispatch reaching the wire;
 * * the simulated vehicle does not act on it. The same is true of the map's own "send the vehicle
 *   here", and a guided takeoff is refused with `MAV_RESULT_FAILED`, so it is the vehicle that will
 *   not fly rather than the station that will not talk to it — most likely its estimator is not
 *   ready for guided flight. The last check fails until that is understood.
 */

import { chromium } from "playwright";

const CDP = process.env.MPCRAFT_CDP ?? "http://127.0.0.1:9222";
const TARGET = process.env.MPCRAFT_UDP ?? "127.0.0.1:14550";
const LEADER = process.env.MPCRAFT_LEADER ?? "1:1";
const MEMBER = process.env.MPCRAFT_MEMBER ?? "2:1";
const ACROSS = Number(process.env.MPCRAFT_ACROSS ?? 25);
const UP = Number(process.env.MPCRAFT_UP ?? 20);

const results = [];
const check = (name, ok, detail) => {
  results.push({ name, ok });
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

/** The snapshot the window reads, which a development build leaves on the window. */
const telemetry = () =>
  page.evaluate(() => {
    const snapshot = window.__mpcraftLive?.value;
    return {
      position: snapshot?.position ?? null,
      mode: snapshot?.mode ?? null,
      armed: snapshot?.armed ?? null,
      home: snapshot?.home ?? null,
    };
  });

/** The metres between two places. */
const metres = (a, b) => {
  const rad = Math.PI / 180;
  const north = (b.lat - a.lat) * 111_320;
  const east =
    (b.lng - a.lng) * 111_320 * Math.cos(((a.lat + b.lat) / 2) * rad);
  return Math.sqrt(north * north + east * east);
};

await page.reload({ waitUntil: "domcontentloaded" });
await page.waitForTimeout(1200);
await page.getByRole("tab", { name: "Datos del vuelo" }).click();
await page.waitForFunction(
  () => window.__mpcraftMap && window.__mpcraftMap.loaded(),
  null,
  { timeout: 30000 },
);

const bar = page.locator('[role="menubar"]');
if (await bar.locator("select").first().isDisabled()) {
  await bar.locator("button").last().click();
  await page.waitForTimeout(2500);
}

await bar.locator("select").first().selectOption("udp");
await bar.locator("input").fill(TARGET);
await bar.locator("button").last().click();

// The link is open before the vehicles answer on it, and the bar disables its controls when it is:
// waiting for the sentence without waiting for the link first is waiting for the wrong thing.
await page.waitForFunction(
  () => document.querySelector('[role="menubar"] select')?.disabled === true,
  null,
  { timeout: 60000 },
);
await page.waitForFunction(
  () => /3 veh[ií]culos en el enlace/.test(document.body.innerText),
  null,
  { timeout: 60000 },
);

const caption = await page.evaluate(() => document.body.innerText);
check(
  "the link carries the three vehicles",
  /3 veh[ií]culos en el enlace/.test(caption),
  caption.match(/\d+ veh[ií]culos en el enlace/)?.[0],
);

// The station speaks to one board at a time, and the pair is what the sheet now shows: with three
// copters the family alone is three rows that say the same word.
// A menu is opened from a bar that no sheet is lying over, and a run that follows one which left a
// sheet open starts by closing it.
const closeSheets = async () => {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const sheets = page.locator('[role="dialog"]');

    if ((await sheets.count()) === 0) {
      return;
    }

    // Escape closes a menu and some sheets; the button in the sheet closes the rest.
    await page.keyboard.press("Escape");
    await page.waitForTimeout(250);

    if ((await sheets.count()) === 0) {
      return;
    }

    const close = sheets
      .first()
      .getByRole("button", { name: /cerrar|close/i })
      .first();

    if ((await close.count()) > 0) {
      await close.click({ force: true }).catch(() => undefined);
      await page.waitForTimeout(300);
    }
  }
};

await closeSheets();
await page.locator('[role="menubar"] [data-menu="file"]').click();
await page.getByRole("menu").getByText("Conectar…", { exact: true }).click();
await page.waitForTimeout(800);

// The sheet lies over the window rather than inside the screen: the boards are listed in it, not in
// the panel of a screen.
const sheetRows = page.locator('[role="dialog"] ul li button');
const count = await sheetRows.count();
const row = sheetRows.filter({ hasText: MEMBER }).first();
check("the sheet lists the boards by their pair", count >= 3, `${count} rows`);
check(
  "the vehicle the station will speak to is listed",
  (await row.count()) === 1,
  MEMBER,
);

await row.click();
await page.waitForTimeout(1200);

const marked = await page.evaluate(() =>
  [...document.querySelectorAll('[role="dialog"] ul li')].map((item) => ({
    text: item.textContent?.trim(),
    chosen:
      item.querySelector("button")?.getAttribute("aria-pressed") === "true",
  })),
);
check(
  "the station speaks to the member",
  marked.some((entry) => entry.chosen && entry.text?.includes(MEMBER)),
  JSON.stringify(marked.find((entry) => entry.chosen)?.text),
);

// The sheet closes, and the commands go to the board that was chosen.
await page
  .locator('[role="dialog"]')
  .getByRole("button", { name: /cerrar|close/i })
  .first()
  .click();
await page.waitForTimeout(500);

await page.getByRole("tab", { name: "Acciones" }).click();
await page.waitForTimeout(400);

await page
  .getByRole("button", { name: /^Armar$/ })
  .first()
  .click();
await page.waitForTimeout(300);
await page
  .getByRole("button", { name: /Confirmar/i })
  .first()
  .click();
await page.waitForTimeout(3000);

await page
  .getByRole("button", { name: /^GUIDED$/ })
  .first()
  .click();
await page.waitForTimeout(1500);

const before = await telemetry();
check("the member is armed", before.armed === true, `armed ${before.armed}`);
check(
  "the member is in GUIDED",
  before.mode === "GUIDED",
  `mode ${before.mode}`,
);

// A formation is flown by vehicles that are already in the air: ArduPilot does not take a copter
// off the ground because a position target arrived, so the height is asked for first.
const height = page.getByLabel(
  /altura a la que despega|height to take off to/i,
);
if ((await height.count()) > 0) {
  await height.fill(String(UP + 5));
}

await page
  .getByRole("button", { name: /^(Despegar|Take off)$/ })
  .first()
  .click();
await page.waitForTimeout(300);
await page
  .getByRole("button", { name: /Confirmar/i })
  .first()
  .click();

let airborne = null;
for (let waited = 0; waited < 60; waited += 3) {
  await page.waitForTimeout(3000);
  const now = await telemetry();

  if (now.position !== null && now.position.alt - before.position.alt > 10) {
    airborne = now.position.alt - before.position.alt;
    break;
  }
}

check(
  "the member takes off when it is told to",
  airborne !== null,
  airborne === null
    ? "it never left the ground"
    : `climbed ${airborne.toFixed(1)} m`,
);

// The formation sheet: who leads, where the member sits, and the loop started.
await closeSheets();
await page.locator('[role="menubar"] [data-menu="setup"]').click();
await page
  .getByRole("menu")
  .getByText(/formación/i)
  .first()
  .click();
await page.waitForTimeout(1000);

const sheet = page.locator('[role="dialog"]');
check("the formation sheet opens", (await sheet.count()) === 1);

await sheet.locator("select").first().selectOption(LEADER);
await sheet.locator("select").nth(1).selectOption(MEMBER);

// The places are kept between two runs, so the member may already have one: adding it again is
// what the sheet refuses, and what this run would trip over.
const alreadyPlaced = await sheet
  .locator("tbody tr", { hasText: MEMBER })
  .count();

if (alreadyPlaced === 0) {
  await sheet.getByRole("button", { name: /^(Añadir|Add)$/ }).click();
  await page.waitForTimeout(400);
}

const fields = sheet.locator("input[type=number]");
check(
  "the member has a place with three numbers",
  (await fields.count()) === 3,
);

for (const [index, value] of [ACROSS, 0, UP].entries()) {
  await fields.nth(index).fill(String(value));
}
await page.waitForTimeout(400);

await sheet.getByRole("button", { name: /^(Empezar|Start)$/ }).click();
await page.waitForTimeout(1500);

const notice = await sheet.innerText();
check(
  "the loop says it is running",
  /se está|being run|ejecut/i.test(notice),
  notice.split("\n").slice(-2).join(" "),
);

// What the vehicle does about it: the place is measured from the leader, so the member that started
// two hundred metres away must end up beside it, at the height the place asks for. The leader's
// home is the one the compose file gives it.
const leader = { lat: -35.363261, lng: 149.16523, alt: 584 };
let reached = null;
let last = null;

for (let waited = 0; waited < 150; waited += 3) {
  await page.waitForTimeout(3000);
  const now = await telemetry();

  if (now.position === null) {
    continue;
  }

  const apart = metres(leader, now.position);
  const high = now.position.alt - leader.alt;
  last = { apart, high };

  if (Math.abs(apart - ACROSS) < 12 && Math.abs(high - UP) < 8) {
    reached = { apart, high, waited };
    break;
  }
}

check(
  "the member flies to its place",
  reached !== null,
  reached === null
    ? `it ended ${last === null ? "nowhere" : `${last.apart.toFixed(1)} m from the leader at ${last.high.toFixed(1)} m`}, and the place is ${ACROSS} m away at ${UP} m`
    : `${reached.apart.toFixed(1)} m from the leader, ${reached.high.toFixed(1)} m high, after ${reached.waited + 3}s`,
);

// Stopping the loop is part of the feature: nothing more is sent to the fleet.
await sheet.getByRole("button", { name: /^(Parar|Stop)$/ }).click();
await page.waitForTimeout(1500);
const stopped = await sheet.innerText();
check(
  "the loop stops when it is told to",
  /parado|stopped|detenid/i.test(stopped),
  stopped.split("\n").slice(-2).join(" "),
);

await browser.close();

const failures = results.filter((result) => !result.ok);
console.log(
  `\n${results.length - failures.length}/${results.length} checks passed`,
);
process.exit(failures.length === 0 ? 0 : 1);
