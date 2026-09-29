import { expect, test } from "@playwright/test";
import { openSampleEditor } from "./support/editor-fixture";

test("desktop editor shell renders the fixed layout and switches tabs", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const errors = await openSampleEditor(page);

  await expect(page.getByRole("tablist", { name: "Editor tools" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Preview viewport" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Timeline canvas" })).toBeVisible();
  await expect(page.getByRole("complementary", { name: "Properties" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Undo" })).toBeDisabled();

  await page.getByRole("tab", { name: "Captions" }).click();
  await expect(page.getByRole("tabpanel", { name: "Captions" })).toBeVisible();
  // The fixture runtime reports macOS, so the primary shortcut modifier is Meta.
  await page.keyboard.press("Meta+1");
  await expect(page.getByRole("tab", { name: "AI" })).toHaveAttribute("data-state", "active");

  await page.getByRole("button", { name: "Home" }).click();
  await expect(page.getByRole("main", { name: "Video editor workspace" })).toBeHidden();
  expect(errors).toEqual([]);
});

test("iPhone 17 Pro editor shell uses the bottom tool bar and sheets", async ({ page }) => {
  await page.setViewportSize({ width: 402, height: 874 });
  const errors = await openSampleEditor(page);

  await expect(page.getByRole("toolbar", { name: "Editor tools" })).toBeVisible();
  await expect(page.getByRole("tablist", { name: "Editor tools" })).toHaveCount(0);
  await page.getByRole("toolbar", { name: "Editor tools" }).getByRole("button", { name: "Media" }).click();
  await expect(page.getByRole("dialog", { name: "Media" })).toBeVisible();
  await page.getByRole("button", { name: "Close Media" }).click();
  await expect(page.getByRole("dialog", { name: "Media" })).toHaveCount(0);

  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow).toBeLessThanOrEqual(0);
  expect(errors).toEqual([]);
});
