/**
 * The interface at the two sizes it has to serve, in the language it is read in.
 *
 * A control that does not fit its box, or a control drawn on top of another one, is the kind of
 * fault a screenshot only shows when somebody happens to look at the right size. So the sizes are
 * walked here, in Spanish, because a word that is longer than its English is what usually breaks
 * a row.
 *
 * What the checker sees is boxes, so it cannot see a font that loads later or a control hidden
 * under a picture with no clipping ancestor. The known-bad case below is what makes its green
 * mean something: without it, a run that finds nothing and a run that cannot find anything look
 * the same.
 */

import { readFileSync } from "node:fs";

import { expect, test } from "@playwright/test";

const CHECKER = readFileSync(
  new URL("../../../scripts/check-overlaps.js", import.meta.url),
  "utf8",
).replace("export function checkOverlaps", "function checkOverlaps");

const SIZES = [
  [1920, 1080],
  [1024, 640],
];

const SCREENS = ["flightdata", "plan", "setup", "logs"];

/** The sheets that lie over a screen, named by what the Setup menu says in Spanish. */
const SHEETS = [
  { name: "firmware", entry: "El firmware…", title: "El firmware" },
  { name: "formation", entry: "La formación…", title: "La formación" },
  {
    name: "keys",
    entry: "Las claves del enlace firmado…",
    title: "Las claves del enlace firmado",
  },
];

/** The language is chosen before the page reads it, so the first paint is already in it. */
async function openInSpanish(page, screen) {
  await page.addInitScript(
    ([locale, which]) => {
      window.localStorage.setItem("mpcraft.locale", locale);
      window.localStorage.setItem("mpcraft.screen", which);
    },
    ["es", screen],
  );

  await page.goto("/");

  await expect(page.locator('[role="tab"]')).not.toHaveCount(0);
}

function found(page) {
  return page.evaluate(`${CHECKER}\ncheckOverlaps()`);
}

for (const [width, height] of SIZES) {
  for (const screen of SCREENS) {
    test(`the ${screen} screen at ${width}x${height} in Spanish`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height });
      await openInSpanish(page, screen);

      const result = await found(page);

      expect(
        {
          size: `${width}x${height}`,
          screen,
          ...result,
        },
        "nothing is drawn on top of something else and nothing is cut off",
      ).toEqual({
        size: `${width}x${height}`,
        screen,
        clashes: [],
        clipped: [],
        overflowing: [],
      });
    });
  }
}

for (const [width, height] of SIZES) {
  for (const sheet of SHEETS) {
    test(`the ${sheet.name} sheet at ${width}x${height} in Spanish`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height });
      await openInSpanish(page, "setup");

      // The sheet is opened the way an operator opens it, through the Setup menu, so that what is
      // measured is the sheet as it stands over the window rather than a box put there for the test.
      await page.locator('button[data-menu="setup"]').click();
      await page.getByRole("menuitem", { name: sheet.entry }).click();

      await expect(
        page.getByRole("dialog", { name: sheet.title }),
      ).toBeVisible();

      const result = await found(page);

      expect(
        {
          size: `${width}x${height}`,
          screen: "setup",
          ...result,
        },
        "nothing is drawn on top of something else and nothing is cut off",
      ).toEqual({
        size: `${width}x${height}`,
        screen: "setup",
        clashes: [],
        clipped: [],
        overflowing: [],
      });
    });
  }
}

for (const [width, height] of SIZES) {
  test(`the connection sheet at ${width}x${height} in Spanish`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height });
    await openInSpanish(page, "flightdata");

    // The sheet is opened the way an operator opens it, through the File menu, so that what is
    // measured is the sheet as it stands over the window rather than a box put there for the test.
    await page.locator('button[data-menu="file"]').click();
    await page.getByRole("menuitem", { name: "Conectar…" }).click();

    await expect(page.getByRole("dialog", { name: "Conexión" })).toBeVisible();

    const result = await found(page);

    expect(
      {
        size: `${width}x${height}`,
        screen: "flightdata",
        ...result,
      },
      "nothing is drawn on top of something else and nothing is cut off",
    ).toEqual({
      size: `${width}x${height}`,
      screen: "flightdata",
      clashes: [],
      clipped: [],
      overflowing: [],
    });
  });
}

for (const [width, height] of SIZES) {
  test(`the outputs tab at ${width}x${height} in Spanish`, async ({ page }) => {
    await page.setViewportSize({ width, height });
    await openInSpanish(page, "flightdata");

    // The tab is opened the way an operator opens it, so what is measured is the panel as it
    // stands in the column rather than a box put there for the test.
    await page.getByRole("tab", { name: "Servos" }).click();

    const result = await found(page);

    expect(
      {
        size: `${width}x${height}`,
        screen: "flightdata",
        tab: "servos",
        ...result,
      },
      "nothing is drawn on top of something else and nothing is cut off",
    ).toEqual({
      size: `${width}x${height}`,
      screen: "flightdata",
      tab: "servos",
      clashes: [],
      clipped: [],
      overflowing: [],
    });
  });
}

for (const [width, height] of SIZES) {
  test(`the actions tab at ${width}x${height} in Spanish`, async ({ page }) => {
    await page.setViewportSize({ width, height });
    await openInSpanish(page, "flightdata");

    // The tab is opened the way an operator opens it, so what is measured is the panel as it stands
    // in the column rather than a box put there for the test.
    await page.getByRole("tab", { name: "Acciones" }).click();

    const result = await found(page);

    expect(
      {
        size: `${width}x${height}`,
        screen: "flightdata",
        tab: "actions",
        ...result,
      },
      "nothing is drawn on top of something else and nothing is cut off",
    ).toEqual({
      size: `${width}x${height}`,
      screen: "flightdata",
      tab: "actions",
      clashes: [],
      clipped: [],
      overflowing: [],
    });
  });
}

for (const [width, height] of SIZES) {
  test(`the messages tab at ${width}x${height} in Spanish`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height });
    await openInSpanish(page, "flightdata");

    // The tab is opened the way an operator opens it, so what is measured is the panel as it
    // stands in the column rather than a box put there for the test.
    await page.getByRole("tab", { name: "Mensajes" }).click();

    const result = await found(page);

    expect(
      {
        size: `${width}x${height}`,
        screen: "flightdata",
        tab: "messages",
        ...result,
      },
      "nothing is drawn on top of something else and nothing is cut off",
    ).toEqual({
      size: `${width}x${height}`,
      screen: "flightdata",
      tab: "messages",
      clashes: [],
      clipped: [],
      overflowing: [],
    });
  });
}

test("the checker still fails on something that is genuinely wrong", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1024, height: 640 });
  await openInSpanish(page, "flightdata");

  const clash = await page.evaluate(
    `${CHECKER}
      (() => {
        const stage = document.createElement("div");

        stage.style.cssText = "position: fixed; left: 0; top: 0; width: 200px; height: 200px;";

        for (const [top, name] of [[40, "the known-bad control"], [60, "the other known-bad control"]]) {
          const button = document.createElement("button");

          button.type = "button";
          button.setAttribute("aria-label", name);
          button.textContent = name;
          button.style.cssText = "position: absolute; top: " + top + "px; left: 20px; width: 120px; height: 30px;";

          stage.append(button);
        }

        document.body.append(stage);

        const result = checkOverlaps();

        stage.remove();

        return result.clashes.filter((line) => /the known-bad control/.test(line) && /the other known-bad control/.test(line));
      })()`,
  );

  expect(
    clash,
    "the checker found nothing on a pair that is on top of each other",
  ).toHaveLength(1);
});
