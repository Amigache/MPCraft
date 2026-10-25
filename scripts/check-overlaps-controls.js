/**
 * The controls of the overlap checker: one known-bad case and one known-good case.
 *
 * A checker that only ever prints green is not a checker. Before its green is trusted,
 * it is shown failing on something that is genuinely wrong, and passing on something
 * that is genuinely right. Both directions, or neither.
 *
 * Run it against the application with the debugger open:
 *
 *     WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222 mpcraft.exe
 *     node scripts/check-overlaps-controls.js
 *
 * What it proves, and what it does not: it proves that a clash reaches the failure path
 * and that a clipped row does not. It does not prove that the checker recognises every
 * way a layout can break — it sees boxes, so a control drawn under a picture with no
 * clipping ancestor, or an overlap that only happens after a font loads, is outside of
 * what these two controls buy.
 */

import { readFileSync } from "node:fs";

const DEBUGGER = "http://127.0.0.1:9222/json";

const checker = readFileSync(
  new URL("./check-overlaps.js", import.meta.url),
  "utf8",
).replace("export function checkOverlaps", "function checkOverlaps");

const targets = await (await fetch(DEBUGGER)).json();
const page = targets.find((target) => target.type === "page");

if (!page) {
  console.error("the application has no page to talk to");
  process.exit(1);
}

const socket = new WebSocket(page.webSocketDebuggerUrl);

let id = 0;
const pending = new Map();

socket.addEventListener("message", (event) => {
  const message = JSON.parse(event.data);
  const resolver = pending.get(message.id);

  if (resolver) {
    pending.delete(message.id);
    resolver(message);
  }
});

await new Promise((resolve) => socket.addEventListener("open", resolve));

const send = (method, params) =>
  new Promise((resolve) => {
    id += 1;
    pending.set(id, resolve);
    socket.send(JSON.stringify({ id, method, params }));
  });

const evaluate = async (expression) => {
  const answer = await send("Runtime.evaluate", {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });

  return answer.result?.result?.value ?? answer.result;
};

await send("Page.enable", {});
await send("Runtime.enable", {});

await evaluate(checker);

// The known-bad case: two controls in the same place, with nothing between them.
const bad = await evaluate(`
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

    const found = checkOverlaps();

    stage.remove();

    return {
      clashes: found.clashes.filter(
        (line) => /the known-bad control/.test(line) && /the other known-bad control/.test(line),
      ),
    };
  })()
`);

// The known-good case: a control that a clipping container has cut away is not on the
// screen, so it cannot be covered by anything.
const good = await evaluate(`
  (() => {
    const stage = document.createElement("div");

    stage.style.cssText = "position: fixed; left: 0; top: 0; width: 240px; height: 240px;";

    const scroller = document.createElement("div");

    scroller.style.cssText = "position: relative; height: 40px; overflow: auto;";

    const row = document.createElement("select");

    row.setAttribute("aria-label", "the known-good control that is cut away");
    row.style.cssText = "position: absolute; top: 60px; left: 10px; width: 120px; height: 30px;";

    scroller.append(row);

    const button = document.createElement("button");

    button.type = "button";
    button.setAttribute("aria-label", "the known-good control that is drawn");
    button.textContent = "drawn";
    button.style.cssText = "position: absolute; top: 8px; left: 10px; width: 120px; height: 30px;";

    scroller.append(button);

    stage.append(scroller);
    document.body.append(stage);

    const found = checkOverlaps();

    stage.remove();

    return {
      clashes: found.clashes.filter((line) => /cut away/.test(line)),
    };
  })()
`);

const fails = [];

if (bad.clashes.length !== 1) {
  fails.push(
    `the checker did not fail on the known-bad case (${bad.clashes.length} found)`,
  );
}

if (good.clashes.length !== 0) {
  fails.push(
    `the checker failed on the known-good case (${good.clashes.length} found)`,
  );
}

console.log(
  JSON.stringify(
    {
      knownBad: bad.clashes,
      knownGood: good.clashes,
      verdict: fails.length === 0 ? "PASS" : "FAIL",
      fails,
      limit:
        "these two controls prove the failure path and the pass path work. They do not prove the checker recognises every way a layout can break.",
    },
    null,
    2,
  ),
);

socket.close();

process.exit(fails.length === 0 ? 0 : 1);
