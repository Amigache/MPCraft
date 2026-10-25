/**
 * The end-to-end specs.
 *
 * These run under Playwright, not under Vitest: the two runners cannot share a file, so
 * `vite.config.ts` keeps this folder out of the unit test run.
 *
 * Playwright drives the real bundle served by Vite, so what it sees is what a person sees. What
 * it cannot see is the Rust side: the IPC bridge has no backend here, so these specs check the
 * shell and its layout, not what a vehicle answers.
 */
import { expect, test } from "@playwright/test";

test("the shell renders", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator("body")).toBeVisible();
});
