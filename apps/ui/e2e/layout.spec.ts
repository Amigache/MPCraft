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
import { createServer } from "node:http";

import { expect, test } from "@playwright/test";

/** The rule the format gives for a chunk, written here rather than taken from the writer. */
function chunkCheck(bytes) {
  let crc = 0xffffffff;

  for (const byte of bytes) {
    let value = crc ^ byte;

    for (let bit = 0; bit < 8; bit += 1) {
      value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
    }

    crc = value;
  }

  return ~crc >>> 0;
}

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
  { name: "dronecan", entry: "Los nodos del bus…", title: "Los nodos del bus" },
  { name: "firmware", entry: "El firmware…", title: "El firmware" },
  { name: "formation", entry: "La formación…", title: "La formación" },
  {
    name: "keys",
    entry: "Las claves del enlace firmado…",
    title: "Las claves del enlace firmado",
  },
  {
    name: "video",
    entry: "Los canales de vídeo…",
    title: "Los canales de vídeo del vehículo",
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

/**
 * The picture, drawn from an address the operator typed.
 *
 * A real camera cannot be asserted here, so the picture is served by a stream of one frame from a
 * fixture written by `scripts/write-test-picture.js` — an independent implementation of the format,
 * not a mirror of the reader. What this asserts is that the picture is drawn at the size the fixture
 * declares, that a click says a place **in the picture**, and that a click in the band the box leaves
 * over is refused rather than read as the nearest edge, which is where upstream clamps.
 *
 * The place is read against numbers written down before the mapping was called: the fixture declares
 * 1280 × 720, and for a box of 640 × 480 those proportions cannot be held without a band, so the
 * middle of the box is the middle of the picture — which is `0, 0` — while a point inside the band is
 * not a place at all.
 */
for (const [width, height] of SIZES) {
  test(`the picture drawn on the video sheet at ${width}x${height} in Spanish`, async ({
    page,
  }) => {
    const fixture = readFileSync(
      new URL("fixtures/picture-1280x720.png", import.meta.url),
    );

    // The declaration read from the fixture's own header, not taken from the name of the file.
    const declared = {
      width: fixture.readUInt32BE(16),
      height: fixture.readUInt32BE(20),
    };

    expect(declared, "the fixture declares the size its name says").toEqual({
      width: 1280,
      height: 720,
    });

    // The checks the file carries, read with the rule the format gives and compared with what an
    // independent implementation (Python's `zlib.crc32`) gives for the same bytes. This caught a
    // writer that started its run at zero without the final inversion: every chunk it wrote disagreed
    // with a PNG written by another tool, and a writer checked only against itself proves nothing.
    const chunks = [];
    let position = 8;

    while (position < fixture.length) {
      const length = fixture.readUInt32BE(position);
      const name = fixture
        .subarray(position + 4, position + 8)
        .toString("ascii");
      const body = fixture.subarray(position + 8, position + 8 + length);
      const stored = fixture.readUInt32BE(position + 8 + length);

      chunks.push({
        name,
        stored,
        recomputed: chunkCheck(
          Buffer.concat([Buffer.from(name, "ascii"), body]),
        ),
      });

      position += 12 + length;
    }

    expect(
      chunks.map(
        (chunk) =>
          `${chunk.name} ${chunk.recomputed.toString(16).padStart(8, "0")}`,
      ),
    ).toEqual(["IHDR 401f4a01", "IDAT 5f2ed914", "IEND ae426082"]);

    for (const chunk of chunks) {
      expect(chunk.stored, `the check the file carries for ${chunk.name}`).toBe(
        chunk.recomputed,
      );
    }

    const server = createServer((request, response) => {
      response.writeHead(200, {
        "Content-Type": "multipart/x-mixed-replace; boundary=frame",
      });

      response.write(
        `--frame\r\nContent-Type: image/png\r\nContent-Length: ${fixture.length}\r\n\r\n`,
      );
      response.write(fixture);
      response.end();
    });

    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
    const port = (server.address() as { port: number }).port;

    try {
      await page.setViewportSize({ width, height });
      await openInSpanish(page, "setup");

      await page.locator('button[data-menu="setup"]').click();
      await page
        .getByRole("menuitem", { name: "Los canales de vídeo…" })
        .click();

      const sheet = page.getByRole("dialog", {
        name: "Los canales de vídeo del vehículo",
      });

      await expect(sheet).toBeVisible();

      // The address is typed by hand, which is the only way the drawing can be reached without a
      // camera that has described a stream.
      await page
        .getByLabel("La dirección de la imagen")
        .fill(`http://127.0.0.1:${port}/stream`);
      await page.getByRole("button", { name: "Dibujar la imagen" }).click();

      // `exact` because the button that starts the drawing says "Dibujar la imagen", which contains
      // the name of the area.
      const area = page.getByRole("button", { name: "la imagen", exact: true });
      const picture = area.locator("img");

      await expect(picture).toBeVisible();

      const size = await picture.evaluate((element) => ({
        width: element.naturalWidth,
        height: element.naturalHeight,
      }));

      expect(
        size,
        "the picture that arrived is the size the fixture declares",
      ).toEqual({ width: 1280, height: 720 });

      // The box is read as the component reads it. It has no border, so the box the element reports is
      // the box a click is measured from — which is what makes the numbers below exact rather than a
      // pixel apart.
      const box = await area.evaluate((element) => {
        const style = getComputedStyle(element);

        return {
          width: element.clientWidth,
          height: element.clientHeight,
          borderX: Number.parseFloat(style.borderLeftWidth) || 0,
          borderY: Number.parseFloat(style.borderTopWidth) || 0,
        };
      });

      expect(
        { borderX: box.borderX, borderY: box.borderY },
        "the edge is drawn inside the box, not on it",
      ).toEqual({ borderX: 0, borderY: 0 });

      // The proportions are 16:9 and the box is not, so there is a band on one axis — which is what
      // makes the two readings differ, and what the clicks below are read against.
      const drawn = {
        width: Math.min(box.width, (box.height * 1280) / 720),
        height: Math.min(box.height, (box.width * 720) / 1280),
      };

      expect(
        drawn.width < box.width || drawn.height < box.height,
        "the box cannot hold these proportions without a band",
      ).toBe(true);

      // The middle of the picture is the middle of the box, because the letterboxing is symmetric, so a
      // click there says `0, 0` whatever the shape of the box is.
      await area.click({ position: { x: box.width / 2, y: box.height / 2 } });

      await expect(sheet.locator("dd").nth(2)).toHaveText("0.00, 0.00");
      await expect(sheet.locator("dd").nth(3)).toHaveText(
        "El lugar se muestra como lo dijo el clic, pero no hay cámara detrás de una dirección escrita a mano, así que no se mandó nada.",
      );

      // A point inside the band: not part of the picture, so it says no place. Upstream clamps the
      // same point to the nearest edge and sends it, which for these numbers would say the top of the
      // picture — and that is the disagreement the last assertion records rather than papers over.
      const band =
        drawn.height < box.height
          ? { x: box.width / 2, y: (box.height - drawn.height) / 4 }
          : { x: (box.width - drawn.width) / 4, y: box.height / 2 };

      await area.click({ position: { x: band.x, y: band.y } });

      await expect(sheet.locator("dd").nth(3)).toHaveText(
        "El clic cayó fuera de la imagen, así que no dice ningún lugar y no se mandó nada.",
      );

      // The place the last click said is still the one the centre said, which is what a refusal means:
      // nothing was read, so nothing was changed.
      await expect(sheet.locator("dd").nth(2)).toHaveText("0.00, 0.00");

      // Read against the box, the way upstream does (`GimbalVideoControl.getMousePosition`, lines
      // 686-691): the same point is clamped to the nearest edge, so it says an edge of the picture and
      // is sent. Here it says no place at all, which is the difference recorded in the matrix.
      const upstream = (value, drawnOnAxis, boxOnAxis) => {
        let read = value;

        if (drawnOnAxis < boxOnAxis) {
          read -= (boxOnAxis - drawnOnAxis) / 2;
          read *= boxOnAxis / drawnOnAxis;
          read = Math.max(0, Math.min(drawnOnAxis, read));
        }

        return (2 * read) / drawnOnAxis - 1;
      };

      const upstreamForBand =
        drawn.height < box.height
          ? upstream(band.y, drawn.height, box.height)
          : upstream(band.x, drawn.width, box.width);

      expect(
        upstreamForBand,
        "the same point read against the box says an edge, which is what upstream sends",
      ).toBeCloseTo(-1, 6);
    } finally {
      server.close();
    }
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
