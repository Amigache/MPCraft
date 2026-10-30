/**
 * A battery of checks over the running interface, driven against a live vehicle.
 *
 * The window is a WebView2 and can be reached over the Chrome DevTools protocol:
 *
 *     $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222"
 *
 * Start `docker/sitl` (three vehicles on `udp:127.0.0.1:14550`) and the development server, then
 * run `node scripts/qa-ui.mjs`.
 *
 * What it checks is that the window answers: every screen and tab draws, every menu opens, the
 * vehicle is told to arm and to change mode and says it did, a plan goes up and comes back, and
 * nothing along the way throws into the console. It is not a substitute for a person looking at
 * the result — it says whether the parts respond, not whether they are pretty.
 *
 * The selectors go by role and by structure wherever they can, and by the words the window says
 * where they cannot: the run is done in Spanish, which is the language this install is read in.
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

// A fault the window throws is a fault, and it is collected rather than printed and forgotten.
const faults = [];
page.on("pageerror", (error) => faults.push(`page: ${error.message}`));
page.on("console", (message) => {
  if (message.type() === "error") {
    faults.push(`console: ${message.text()}`);
  }
});

const text = () => page.evaluate(() => document.body.innerText);
const footer = () => page.locator("footer").innerText();
const armed = async () => (await footer()).match(/ARMADO\s+(\S+)/)?.[1];
const mode = async () => (await footer()).match(/MODO\s+(\S+)/)?.[1];

const openMenu = async (id) => {
  await page.locator(`[role="menubar"] [data-menu="${id}"]`).click();
  await page.waitForTimeout(250);
};

const menuItem = (label) =>
  page.getByRole("menu").getByText(label, { exact: true });

const pressMenu = async (id, label) => {
  await openMenu(id);
  await menuItem(label).click();
  await page.waitForTimeout(600);
};

// A window that has just been opened: the state of a previous run is not part of this one.
//
// The screen is chosen before the map is waited for. The map lives on the flight-data screen, and
// a window that opens on another screen has it hidden: a map that is not drawn has not loaded, and
// the line that says how many vehicles the link carries is on that screen too.
await page.reload({ waitUntil: "domcontentloaded" });
await page.waitForTimeout(1200);
await page.getByRole("tab", { name: "Datos del vuelo" }).click();
await page.waitForFunction(
  () => window.__mpcraftMap && window.__mpcraftMap.loaded(),
  null,
  { timeout: 30000 },
);

// ---------------------------------------------------------------- connection

// The link belongs to the backend, not to the page: a page that was reloaded comes back with the
// bar of a window that is still linked, and the button that opens a link would close it.

const bar = page.locator('[role="menubar"]');

if (await bar.locator("select").first().isDisabled()) {
  await bar.locator("button").last().click();
  await page.waitForFunction(
    () => !document.querySelector('[role="menubar"] select')?.disabled,
    null,
    {
      timeout: 15000,
    },
  );
  await page.waitForTimeout(1500);
}

await bar.locator("select").first().selectOption("udp");
await bar.locator("input").fill(VEHICLE);
await bar.locator("button").last().click();

await page.waitForFunction(
  () => /\d+ veh[ií]culos? en el enlace/.test(document.body.innerText),
  null,
  { timeout: 30000 },
);
await page.waitForTimeout(6000);

const said = await text();
const counted = said.match(/(\d+) veh[ií]culos? en el enlace/);
check(
  "the link carries the vehicles",
  counted !== null && Number(counted[1]) >= 3,
  counted?.[0],
);

const barText = await footer();
check(
  "the link reports no fault",
  !/no_vehicle|rejected_by_vehicle|link_timeout|transport/i.test(barText),
  barText.replace(/\s+/g, " ").slice(0, 90),
);

check("a mode is reported", (await mode()) !== undefined, await mode());
check(
  "the armed state is reported",
  (await armed()) !== undefined,
  await armed(),
);

// ---------------------------------------------------------------- screens

for (const name of [
  "Datos del vuelo",
  "Plan",
  "Configuración",
  "Registros",
].entries()) {
  await page.getByRole("tab", { name }).click();
  await page.waitForTimeout(900);

  const panel = await page.evaluate(() => {
    const main = document.querySelector("main");
    return { id: main?.id, length: main?.innerText.trim().length ?? 0 };
  });

  check(
    `the screen ${name} draws`,
    panel.id !== undefined && panel.length > 40,
    `${panel.id}, ${panel.length} chars`,
  );
}

// ---------------------------------------------------------------- menus

for (const id of ["file", "flight", "view", "setup", "help"]) {
  await openMenu(id);
  const items = await page.evaluate(
    () =>
      document.querySelectorAll('[role="menuitem"], [role="menuitemcheckbox"]')
        .length,
  );
  check(`the menu ${id} opens`, items > 0, `${items} entries`);
  await page.keyboard.press("Escape");
  await page.waitForTimeout(200);
}

// ---------------------------------------------------------------- flight data

await page.getByRole("tab", { name: "Datos del vuelo" }).click();
await page.waitForTimeout(500);

for (const name of [
  "Vista rápida",
  "Acciones",
  "Mensajes",
  "Antes del vuelo",
  "Diales",
  "Estado",
  "Servos",
  "Auxiliares",
]) {
  await page.getByRole("tab", { name }).click();
  await page.waitForTimeout(500);
  const filled = await page.evaluate(() => {
    const aside = document.querySelector("main aside");
    return (aside?.innerText.trim().length ?? 0) > 20;
  });
  check(`the tab ${name} draws`, filled);
}

// The words the vehicle said while the link was held: ArduPilot speaks when it arms and when its
// estimator comes up, and the console has to be the place they are read.
await page.getByRole("tab", { name: "Mensajes" }).click();
await page.waitForTimeout(400);
const console_ = await page.locator("main aside").innerText();
check(
  "the messages console shows what the vehicle said",
  console_.trim().length > 10,
  console_.replace(/\s+/g, " ").slice(0, 70),
);

// ---------------------------------------------------------------- commands

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
await page.waitForTimeout(3500);
check(
  "the vehicle arms when it is told to",
  (await armed()) === "sí",
  `armed ${await armed()}`,
);

await page
  .getByRole("button", { name: /^GUIDED$/ })
  .first()
  .click();
await page.waitForTimeout(1500);
check(
  "the vehicle takes a mode",
  (await mode()) === "GUIDED",
  `mode ${await mode()}`,
);

await page
  .getByRole("button", { name: /^Desarmar$/ })
  .first()
  .click();
await page.waitForTimeout(300);
await page
  .getByRole("button", { name: /Confirmar/i })
  .first()
  .click();
await page.waitForTimeout(3500);
check(
  "the vehicle disarms when it is told to",
  (await armed()) === "no",
  `armed ${await armed()}`,
);

// ---------------------------------------------------------------- plan

await page.getByRole("tab", { name: "Plan" }).click();
// The editor frames the plan once when it opens, and that is an animation: a measurement taken
// while it runs is a measurement of the animation, not of the press that follows.
await page.waitForTimeout(2000);
await page
  .getByRole("button", { name: /a[nñ]adir punto/i })
  .first()
  .click();

const canvas = page.locator(".maplibregl-canvas");
const box = await canvas.boundingBox();

const mapState = () =>
  page.evaluate(async () => ({
    zoom: window.__mpcraftMap.getZoom(),
    items: (await window.__mpcraftMap.getSource("plan-items").getData())
      .features.length,
  }));

// The presses are far from the middle, where the vehicle and its home already are: a press on a
// mark selects that mark, which is the map working rather than the plan growing.
for (const offset of [
  { x: -300, y: -150 },
  { x: 300, y: 150 },
]) {
  const from = await mapState();
  await page.mouse.click(
    Math.round(box.x + box.width / 2 + offset.x),
    Math.round(box.y + box.height / 2 + offset.y),
  );
  await page.waitForTimeout(900);
  const to = await mapState();

  check(
    "a press that adds an item leaves the view alone",
    Math.abs(to.zoom - from.zoom) < 0.001,
    `${from.zoom} -> ${to.zoom}`,
  );
  check(
    "the press adds an item to the plan",
    to.items > from.items,
    `${from.items} -> ${to.items}`,
  );
}

const drawn = await page.evaluate(
  async () =>
    (await window.__mpcraftMap.getSource("plan").getData()).geometry.coordinates
      .length,
);
check("the plan is drawn between the items", drawn >= 2, `${drawn} points`);

// The plan goes to the vehicle and comes back: the pair that matters is the one that survives the
// trip, not the one on the screen.
await page
  .getByRole("button", { name: /subir el plan/i })
  .first()
  .click();
const uploadConfirm = page.locator(
  '[role="alertdialog"] button.button--primary',
);
if (await uploadConfirm.count()) {
  await uploadConfirm.click();
}
await page.waitForTimeout(6000);

await page
  .getByRole("button", { name: /bajar el plan/i })
  .first()
  .click();
if (await uploadConfirm.count()) {
  await uploadConfirm.click();
}
await page.waitForTimeout(6000);

const returned = await page.evaluate(
  async () =>
    (await window.__mpcraftMap.getSource("plan").getData()).geometry.coordinates
      .length,
);
check(
  "the plan comes back from the vehicle",
  returned === drawn,
  `${drawn} up, ${returned} back`,
);

// ---------------------------------------------------------------- parameters

await page.getByRole("tab", { name: "Configuración" }).click();
await page.waitForTimeout(1200);

// The parameters are a sheet over the configuration screen, and the vehicle says none of them
// until it is asked for the whole list.
const readAll = page
  .getByRole("button", { name: /leerlos todos|read them all/i })
  .first();
check(
  "the parameters sheet offers to read the list",
  (await readAll.count()) === 1,
);

if (await readAll.count()) {
  await readAll.click();
  await page
    .waitForFunction(
      () =>
        /([5-9]\d\d|\d{4,})\s+(par[aá]metros|parameters)/i.test(
          document.body.innerText,
        ),
      null,
      { timeout: 60000 },
    )
    .catch(() => undefined);
}

const params = await page.evaluate(() => {
  const main = document.querySelector("main");
  const count = (main?.innerText.match(
    /(\d[\d.,]*)\s+(par[aá]metros|parameters)/i,
  ) ?? [])[1];
  return { count, length: main?.innerText.trim().length ?? 0 };
});
check(
  "the parameters arrive from the vehicle",
  params.count !== undefined && params.count !== "0",
  params.count,
);

// ---------------------------------------------------------------- map menu

await page.getByRole("tab", { name: "Datos del vuelo" }).click();
await page.waitForTimeout(800);

await page.mouse.click(
  Math.round(box.x + box.width / 2),
  Math.round(box.y + box.height / 2),
  { button: "right" },
);
await page.waitForTimeout(500);

const entries = await page.evaluate(() => {
  const menu = document.querySelector('[role="menu"]');
  return menu ? menu.querySelectorAll("button").length : 0;
});
check("the map opens its menu", entries >= 3, `${entries} entries`);

await page.keyboard.press("Escape");
await page.waitForTimeout(300);
const closed = await page.evaluate(
  () => document.querySelectorAll('[role="menu"]').length === 0,
);
check("the map menu closes on Escape", closed);

// ---------------------------------------------------------------- theme and language

await pressMenu("view", "Claro");
const light = await page.evaluate(() =>
  document.documentElement.getAttribute("data-theme"),
);
check("the theme changes", light === "light", `data-theme=${light}`);

await pressMenu("view", "Oscuro");
const dark = await page.evaluate(() =>
  document.documentElement.getAttribute("data-theme"),
);
check("the theme goes back", dark === "dark", `data-theme=${dark}`);

await pressMenu("view", "English");
await page.waitForTimeout(700);
const english = await text();
check(
  "the language changes",
  /Flight Data|File|Setup/.test(english),
  english.split("\n")[0]?.slice(0, 40),
);

await pressMenu("view", "Español");
await page.waitForTimeout(700);
const spanish = await text();
check(
  "the language goes back",
  /Datos del vuelo|Archivo|Configuración/.test(spanish),
  spanish.split("\n")[0]?.slice(0, 40),
);

// ---------------------------------------------------------------- faults

check(
  "the window threw nothing",
  faults.length === 0,
  faults.slice(0, 2).join(" | "),
);

await browser.close();

const failures = results.filter((result) => !result.ok);
console.log(
  `\n${results.length - failures.length}/${results.length} checks passed`,
);
process.exit(failures.length === 0 ? 0 : 1);
