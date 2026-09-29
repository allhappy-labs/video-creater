import { expect, test, type Locator, type Page } from "@playwright/test";
import { openSampleEditor } from "./support/editor-fixture";

// Sample project: Video 1 holds "Opening clip" (0–4 s); "Caption 1" ("Original caption text") runs
// 0.65–2 s. The timeline is 80 px per second, so a click 80 px into the clip seeks to 1 s.

function timelineClip(page: Page, label: string): Locator {
  return page.getByRole("region", { name: "Timeline canvas" }).getByRole("option", { name: new RegExp(`^${label},`) });
}

function properties(page: Page): Locator {
  return page.getByRole("complementary", { name: "Properties" });
}

function previewVideo(page: Page): Locator {
  return page.getByRole("region", { name: "Preview viewport" }).getByLabel("Timeline video Opening clip");
}

function previewCaption(page: Page): Locator {
  return page.getByRole("region", { name: "Preview viewport" }).getByLabel(/^Timeline preview caption /);
}

async function box(locator: Locator) {
  const bounds = await locator.boundingBox();
  if (!bounds) throw new Error("element is not laid out");
  return bounds;
}

async function selectOpeningClipAtOneSecond(page: Page) {
  const clip = timelineClip(page, "Opening clip");
  const bounds = await box(clip);
  await page.mouse.click(bounds.x + 80, bounds.y + bounds.height / 2);
  await expect(clip).toHaveAttribute("aria-selected", "true");
  await expect(page.getByTestId("playhead")).toHaveAttribute("data-seconds", "1.000");
}

test("desktop Properties docks, overlays, commits one undo step, keyframes and opens inline caption editing", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const errors = await openSampleEditor(page);
  const previewPanel = page.getByRole("region", { name: "Preview panel" });

  await selectOpeningClipAtOneSecond(page);
  const aside = properties(page);
  await expect(aside.getByRole("heading", { level: 2, name: "Opening clip" })).toBeVisible();
  // Docked at 1440 px: a third column beside the preview.
  await expect(aside).toHaveCSS("position", "static");
  expect((await box(aside)).x).toBeGreaterThanOrEqual((await box(previewPanel)).x + (await box(previewPanel)).width);

  // 1024–1279 px: over the right edge of the preview, never over the timeline.
  await page.setViewportSize({ width: 1100, height: 800 });
  await expect(aside).toHaveCSS("position", "absolute");
  const overlay = await box(aside);
  const preview = await box(previewPanel);
  expect(overlay.x).toBeLessThan(preview.x + preview.width);
  expect(overlay.y + overlay.height).toBeLessThanOrEqual((await box(page.getByRole("region", { name: "Timeline canvas" }))).y);
  await page.setViewportSize({ width: 1440, height: 900 });
  await expect(aside).toHaveCSS("position", "static");

  // Opacity 50% through the numeric field is one undo step.
  const opacity = aside.getByRole("textbox", { name: "Opacity" });
  await opacity.fill("50");
  await opacity.press("Enter");
  await expect(previewVideo(page)).toHaveCSS("opacity", "0.5");
  await aside.getByRole("heading", { level: 2 }).click();
  // The fixture runtime reports macOS, so undo is Meta+Z.
  await page.keyboard.press("Meta+z");
  await expect(previewVideo(page)).toHaveCSS("opacity", "1");
  await expect(opacity).toHaveValue("100%");
  await expect(page.getByRole("button", { name: "Undo" })).toBeDisabled();

  // ◇ adds an opacity keyframe at the playhead; the keyframe lane shows its diamond.
  await aside.getByRole("button", { name: "Add keyframe for Opacity" }).click();
  await expect(aside.getByRole("button", { name: "Remove keyframe for Opacity" })).toBeVisible();
  await page.getByRole("toolbar", { name: "Timeline tools" }).getByRole("button", { name: "Keyframes" }).click();
  const lane = page.getByRole("group", { name: "Opacity keyframe lane for Opening clip" });
  await expect(lane.getByRole("button", { name: /^Opacity keyframe at 1\.00 seconds/ })).toBeVisible();

  // Double-click the caption on the canvas: the inline editor opens on its text, and Escape cancels.
  const caption = previewCaption(page);
  await expect(caption).toHaveText("Original caption text");
  await caption.getByText("Original caption text").dblclick();
  const editor = page.getByRole("textbox", { name: "Edit caption Caption 1" });
  await expect(editor).toBeFocused();
  await expect(editor).toHaveText("Original caption text");
  await page.keyboard.type("Restored voice");
  await expect(editor).toHaveText("Restored voice");
  await page.keyboard.press("Escape");
  await expect(editor).toHaveCount(0);
  await expect(caption).toHaveText("Original caption text");

  // The Media grid asset preview step waits for the Media tab (plan 05).
  expect(errors).toEqual([]);
});

// "Caption 1" spans several words but links to transcript word 0, so Enter commits `editCaptionText`
// (a transcript repair would retime that word to the whole cue and be rejected).
test("desktop inline caption edit commits the new text on Enter", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const errors = await openSampleEditor(page);
  await selectOpeningClipAtOneSecond(page);
  const caption = previewCaption(page);
  await caption.getByText("Original caption text").dblclick();
  await expect(page.getByRole("textbox", { name: "Edit caption Caption 1" })).toBeFocused();
  await page.keyboard.type("Restored voice");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("textbox", { name: /^Edit caption / })).toHaveCount(0);
  await expect(caption).toHaveText("Restored voice");
  expect(errors).toEqual([]);
});

test.describe("phone", () => {
  test.use({ viewport: { width: 402, height: 874 }, hasTouch: true });

  test("iPhone 17 Pro adjusts a clip's scale from the Adjust sheet", async ({ page }) => {
    const errors = await openSampleEditor(page);
    const clip = timelineClip(page, "Opening clip");
    await clip.tap();
    await expect(clip).toHaveAttribute("aria-selected", "true");

    await page.getByRole("toolbar", { name: "Clip tools" }).getByRole("button", { name: "Adjust" }).tap();
    const sheet = page.getByRole("dialog", { name: "Adjust" });
    await expect(sheet).toBeVisible();
    // The sheet is non-modal and compact, so the preview stays visible above it.
    await expect(page.getByRole("region", { name: "Preview viewport" })).toBeVisible();
    expect((await box(sheet)).y).toBeGreaterThan((await box(page.getByRole("region", { name: "Preview viewport" }))).y);

    const scale = sheet.getByRole("textbox", { name: "Scale" });
    await scale.fill("150");
    await scale.press("Enter");
    await expect(scale).toHaveValue("150%");
    await expect(previewVideo(page)).toHaveCSS("transform", /matrix\(1\.5, 0, 0, 1\.5/);

    // The clip tools stay visible below the sheet: another tool swaps the sheet.
    await page.getByRole("toolbar", { name: "Clip tools" }).getByRole("button", { name: "Speed" }).tap();
    await expect(page.getByRole("dialog", { name: "Speed" })).toBeVisible();
    await expect(sheet).toHaveCount(0);
    await page.getByRole("toolbar", { name: "Clip tools" }).getByRole("button", { name: "Adjust" }).tap();
    await expect(sheet.getByRole("textbox", { name: "Scale" })).toHaveValue("150%");

    await sheet.getByRole("button", { name: "Close Adjust" }).tap();
    await expect(sheet).toHaveCount(0);
    await expect(page.getByRole("toolbar", { name: "Clip tools" })).toBeVisible();
    expect(errors).toEqual([]);
  });
});
