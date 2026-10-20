/**
 * The end-to-end specs.
 *
 * These run under Playwright, not under Vitest: the two runners cannot share a file, so
 * `vite.config.ts` keeps this folder out of the unit test run.
 *
 * Playwright is **not** a dependency yet — installing it needs a browser download that
 * does not belong in a build script — so these specs are the place to start from rather
 * than something the test run exercises today.
 */
import { expect, test } from "@playwright/test";

test("the shell renders", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator("body")).toBeVisible();
});
