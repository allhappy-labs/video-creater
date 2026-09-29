import { expect, test, type Locator, type Page } from "@playwright/test";
import { openSampleEditor } from "./support/editor-fixture";

// The tasks fixture seeds a running Temporal transcription, a completed MP4 export with a render
// report, a failed draft render, and a DaVinci XML export whose workflow never started into the
// sample project, next to its bundled generation. Opening the editor reconciles Temporal jobs, which
// fails the stale XML export with a plain reason.

const temporalReason = "This task runs in the workflow worker and can't be cancelled from the editor.";

async function expectInsideViewport(page: Page, locator: Locator) {
  const box = await locator.boundingBox();
  const viewport = page.viewportSize();
  if (!box || !viewport) throw new Error("The overlay has no layout box");
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.y).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(viewport.width);
  expect(box.y + box.height).toBeLessThanOrEqual(viewport.height);
}

async function expectTaskRows(list: Locator) {
  await expect(list.getByRole("listitem")).toHaveCount(5);
  const transcription = list.getByRole("listitem", { name: "Transcribe input.mp4" });
  await expect(transcription.getByRole("button", { name: "Cancel" })).toHaveAttribute("aria-disabled", "true");
  await expect(transcription.getByRole("button", { name: "Cancel" })).toHaveAccessibleDescription(temporalReason);
  const failed = list.getByRole("listitem", { name: "Timeline render" });
  await expect(failed.getByText("The render stopped before it finished.")).toBeVisible();
  await expect(failed.getByRole("button", { name: "Retry" })).toBeVisible();
  const stale = list.getByRole("listitem", { name: "DaVinci XML export" });
  await expect(stale.getByText("The workflow never started.")).toBeVisible();
  await expect(stale.getByRole("button", { name: "Retry" })).toBeVisible();
}

test("desktop Background tasks popover lists tasks and opens render details", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const errors = await openSampleEditor(page, { panelFixtures: true, tasksFixture: true });

  const indicator = page.getByRole("button", { name: "Background tasks" });
  await expect(indicator).toContainText("Transcribing…");
  await indicator.click();
  const popover = page.getByRole("dialog", { name: "Background tasks" });
  await expect(popover).toBeVisible();
  await expectTaskRows(popover);
  await expectInsideViewport(page, popover);
  await page.screenshot({ path: testInfo.outputPath("tasks-popover-desktop.png") });

  const exportRow = popover.getByRole("listitem").filter({ hasText: "Video export" });
  await exportRow.getByRole("button", { name: "Show" }).click();
  await expect.poll(() => page.evaluate(() => window.__EDITOR_FIXTURE_REVEALS__?.map((reveal) => reveal.artifactPath))).toEqual(["renders/fixture-export-mp4/output.mp4"]);
  await expect(popover).toBeVisible();

  await exportRow.getByRole("button", { name: "Details" }).click();
  const details = page.getByRole("dialog", { name: "Task details" });
  await expect(details).toBeVisible();
  await expect(popover).toHaveCount(0);
  await expect(details.getByRole("region", { name: "Render report" })).toBeVisible();
  await details.getByRole("button", { name: "Compare preview and render" }).click();
  await expect(details.getByText("Preview and render differ on 1 of 2 frames.")).toBeVisible();
  await expectInsideViewport(page, details);
  await page.screenshot({ path: testInfo.outputPath("task-details-desktop.png") });

  await page.keyboard.press("Escape");
  await expect(details).toHaveCount(0);
  await expect(indicator).toBeFocused();
  expect(errors).toEqual([]);
});

test("iPhone 17 Pro Background tasks sheet fits the viewport and shows worker preflight", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 402, height: 874 });
  const errors = await openSampleEditor(page, { panelFixtures: true, tasksFixture: true });

  await page.getByRole("button", { name: "Background tasks" }).click();
  const sheet = page.getByRole("dialog", { name: "Background tasks" });
  await expect(sheet).toBeVisible();
  await expectTaskRows(sheet);
  await expectInsideViewport(page, sheet);
  await page.screenshot({ path: testInfo.outputPath("tasks-sheet-phone.png") });

  await sheet.getByRole("listitem", { name: "Transcribe input.mp4" }).getByRole("button", { name: "Details" }).click();
  const details = page.getByRole("dialog", { name: "Task details" });
  await expect(details.getByRole("region", { name: "Worker preflight" }).getByText("Setup needed")).toBeVisible();
  await expectInsideViewport(page, details);
  await page.screenshot({ path: testInfo.outputPath("task-details-phone.png") });
  expect(errors).toEqual([]);
});
