import { expect, test, type Page } from "@playwright/test";
import { openSampleEditor } from "./support/editor-fixture";

// Without the export fixture there is no native exporter, so every video format reads as unavailable
// with its reason. The export flow itself, at desktop and phone sizes, is in editor-export-tasks.spec.ts.

async function expectInsideViewport(page: Page, name: string) {
  const box = await page.getByRole("dialog", { name }).boundingBox();
  const viewport = page.viewportSize();
  if (!box || !viewport) throw new Error(`${name} has no layout box`);
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.y).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(viewport.width);
  expect(box.y + box.height).toBeLessThanOrEqual(viewport.height);
}

test("desktop Export button and Meta+E open the Export popover", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const errors = await openSampleEditor(page, { panelFixtures: true });

  await page.getByRole("button", { name: "Export" }).click();
  const dialog = page.getByRole("dialog", { name: "Export" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("radio", { name: "WebM" })).toHaveAttribute("aria-disabled", "true");
  await expect(dialog.getByRole("radio", { name: "ProRes" })).toHaveAttribute("aria-disabled", "true");
  await expect(dialog.getByRole("button", { name: "Export video" })).toHaveAttribute("aria-disabled", "true");
  await expectInsideViewport(page, "Export");

  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await page.keyboard.press("Meta+e");
  await expect(dialog).toBeVisible();
  expect(errors).toEqual([]);
});
