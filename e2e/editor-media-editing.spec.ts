import { expect, test, type Locator, type Page } from "@playwright/test";
import { acceptanceViewports } from "./support/editor";
import { openSampleEditor } from "./support/editor-fixture";

// Sample project: Video 1 holds "Opening clip" (0–4 s) and "Restored Edison alternate" (4–8 s);
// Audio 1 holds "Music bed" (0–4 s), so audio detached from "Opening clip" collides with it and goes
// to a new audio track. Reverse needs render preparation, which the browser sample never runs, so the
// preview reports that it is preparing. Evidence screenshots go to output/editor-media-editing/.

const [desktop, phone] = acceptanceViewports;
const screenshotDir = "output/editor-media-editing";
const linkedAudioReason = "This clip's sound is already on a linked audio clip.";

function canvas(page: Page): Locator {
  return page.getByRole("region", { name: "Timeline canvas" });
}

function clip(page: Page, label: string): Locator {
  return canvas(page).getByRole("option", { name: new RegExp(`^${label},`) });
}

/** A clip by its stable item id, whose accessible name changes as it is edited. */
async function clipById(page: Page, label: string): Promise<Locator> {
  const itemId = await clip(page, label).first().getAttribute("data-item-id");
  expect(itemId).not.toBeNull();
  return canvas(page).locator(`[role="option"][data-item-id="${itemId ?? ""}"]`);
}

function previewIssue(page: Page): Locator {
  return page.getByRole("region", { name: "Preview viewport" }).getByRole("alert", { name: "Preview issue" });
}

/** The accessible name of the track row holding `item`. */
function trackOf(item: Locator): Promise<string | null> {
  return item.evaluate((element) => element.closest('[role="listbox"]')?.getAttribute("aria-label") ?? null);
}

test("desktop retimes Music bed, detaches and reverses Opening clip's audio, each one undo step", async ({ page }) => {
  await page.setViewportSize({ width: desktop.width, height: desktop.height });
  const errors = await openSampleEditor(page);
  const undo = page.getByRole("button", { name: "Undo" });
  const rows = canvas(page).getByRole("listbox");
  const properties = page.getByRole("complementary", { name: "Properties" });

  // 1. Music bed's Speed tab: 2× halves the clip to 2 s.
  const music = await clipById(page, "Music bed");
  await expect(music).toHaveAttribute("aria-label", "Music bed, 00:00:00, 00:00:04");
  await music.click();
  await expect(music).toHaveAttribute("aria-selected", "true");
  await expect(properties.getByRole("heading", { level: 2, name: "Music bed" })).toBeVisible();
  await properties.getByRole("tab", { name: "Speed" }).click();
  await expect(properties.getByRole("tab", { name: "Speed" })).toHaveAttribute("data-state", "active");
  await page.screenshot({ path: `${screenshotDir}/audio-speed-tab-${desktop.name}.png`, animations: "disabled" });
  await properties.getByRole("group", { name: "Speed presets" }).getByRole("button", { name: "2×" }).click();
  await expect(music).toHaveAttribute("aria-label", "Music bed, 00:00:00, 00:00:02");

  // 2. One undo restores 4 s, and nothing is left to undo.
  await properties.getByRole("heading", { level: 2 }).click();
  // The fixture runtime reports macOS, so undo is Meta+Z.
  await page.keyboard.press("Meta+z");
  await expect(music).toHaveAttribute("aria-label", "Music bed, 00:00:00, 00:00:04");
  await expect(undo).toBeDisabled();

  // 3. Detach audio from the context menu: Music bed occupies Audio 1, so a new audio track holds it.
  // The new track sits right under the video band, so it reads "Audio 1" and Music bed's "Audio 2".
  const rowCount = await rows.count();
  const opening = await clipById(page, "Opening clip");
  await opening.click({ button: "right" });
  const detach = page.getByRole("menuitem", { name: "Detach audio" });
  await expect(detach).not.toHaveAttribute("aria-disabled", "true");
  await page.screenshot({ path: `${screenshotDir}/context-menu-${desktop.name}.png`, animations: "disabled" });
  await detach.click();
  const detached = clip(page, "Opening clip audio");
  await expect(detached).toHaveCount(1);
  await expect(detached).toHaveAttribute("aria-label", "Opening clip audio, 00:00:00, 00:00:04");
  await expect(rows).toHaveCount(rowCount + 1);
  expect(await trackOf(detached)).toBe("Audio 1");
  expect(await trackOf(music)).toBe("Audio 2");

  // 4. The clip's sound is now on a linked audio clip, so Detach audio is disabled with the reason.
  await opening.click({ button: "right" });
  await expect(detach).toHaveAttribute("aria-disabled", "true");
  await detach.hover();
  await expect(page.getByRole("tooltip")).toHaveText(linkedAudioReason);
  await page.screenshot({ path: `${screenshotDir}/context-menu-detached-${desktop.name}.png`, animations: "disabled" });
  // The first Escape dismisses the tooltip, the second the menu.
  await page.keyboard.press("Escape");
  await expect(page.getByRole("tooltip")).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu")).toHaveCount(0);

  // 5. One undo removes the audio clip and its new track.
  await page.keyboard.press("Meta+z");
  await expect(detached).toHaveCount(0);
  await expect(rows).toHaveCount(rowCount);
  expect(await trackOf(music)).toBe("Audio 1");
  await expect(undo).toBeDisabled();

  // 6. Reverse: with the audio detached again, the Speed tab's Reverse switch reverses Opening clip
  // and its linked audio clip in one step; the menu then offers Play forward.
  await opening.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Detach audio" }).click();
  await expect(detached).toHaveCount(1);
  await opening.click();
  await expect(properties.getByRole("heading", { level: 2, name: "Opening clip" })).toBeVisible();
  await properties.getByRole("tab", { name: "Speed" }).click();
  const reverse = properties.getByRole("switch", { name: "Reverse" });
  await expect(reverse).not.toBeChecked();
  await reverse.click();
  await expect(reverse).toBeChecked();
  await expect(opening).toHaveAccessibleDescription("Plays in reverse");
  await expect(opening.getByTestId("clip-reversed-mark")).toBeVisible();
  await expect(opening).toHaveAttribute("aria-label", "Opening clip, 00:00:00, 00:00:04");
  await expect(detached).toHaveAccessibleDescription("Plays in reverse");
  // The browser sample never prepares, so the preview waits for the reversed frames.
  await expect(previewIssue(page).getByRole("paragraph").filter({ hasText: "Preparing canonical Lottie, LUT, reversed-clip, or richer-blend preview frames." })).toBeVisible();
  await page.screenshot({ path: `${screenshotDir}/reverse-speed-tab-${desktop.name}.png`, animations: "disabled" });
  await opening.click({ button: "right" });
  await expect(page.getByRole("menuitem", { name: "Play forward" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Reverse", exact: true })).toHaveCount(0);
  await page.screenshot({ path: `${screenshotDir}/context-menu-reversed-${desktop.name}.png`, animations: "disabled" });
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu")).toHaveCount(0);

  // One undo plays both clips forward again and keeps the detached audio.
  await properties.getByRole("heading", { level: 2 }).click();
  await page.keyboard.press("Meta+z");
  await expect(opening).not.toHaveAccessibleDescription("Plays in reverse");
  await expect(detached).not.toHaveAccessibleDescription("Plays in reverse");
  await expect(opening.getByTestId("clip-reversed-mark")).toHaveCount(0);
  await expect(detached).toHaveCount(1);

  expect(errors).toEqual([]);
});

test.describe("phone", () => {
  test.use({ viewport: { width: phone.width, height: phone.height }, hasTouch: true });

  test("iPhone 17 Pro detaches audio from the clip tools and reverses Music bed from the Speed sheet", async ({ page }) => {
    const errors = await openSampleEditor(page);
    const clipTools = page.getByRole("toolbar", { name: "Clip tools" });

    const opening = await clipById(page, "Opening clip");
    await opening.tap();
    await expect(opening).toHaveAttribute("aria-selected", "true");
    const detach = clipTools.getByRole("button", { name: "Detach audio" });
    await expect(detach).toBeVisible();
    await detach.tap();
    await expect(clip(page, "Opening clip audio")).toHaveCount(1);

    const music = await clipById(page, "Music bed");
    await music.tap();
    await expect(music).toHaveAttribute("aria-selected", "true");
    await expect(clipTools.getByRole("button", { name: "Detach audio" })).toHaveCount(0);
    await clipTools.getByRole("button", { name: "Speed" }).tap();
    const sheet = page.getByRole("dialog", { name: "Speed" });
    await expect(sheet).toBeVisible();
    await expect(sheet.getByRole("group", { name: "Speed presets" })).toBeVisible();
    const reverse = sheet.getByRole("switch", { name: "Reverse" });
    await expect(reverse).not.toBeChecked();
    await page.screenshot({ path: `${screenshotDir}/audio-speed-tab-${phone.name}.png`, animations: "disabled" });
    // Reversed audio waits for preparation, which the browser sample never runs.
    await reverse.tap();
    await expect(reverse).toBeChecked();
    await expect(music).toHaveAccessibleDescription("Plays in reverse");
    await expect(previewIssue(page).getByRole("paragraph").filter({ hasText: "Reversed audio is preparing." })).toBeVisible();
    await page.screenshot({ path: `${screenshotDir}/audio-reversed-${phone.name}.png`, animations: "disabled" });
    await sheet.getByRole("button", { name: "Close Speed" }).tap();
    await expect(sheet).toHaveCount(0);

    // The clip menu at phone size, opened with a secondary click on the detached video clip.
    await opening.click({ button: "right" });
    await expect(page.getByRole("menuitem", { name: "Detach audio" })).toHaveAttribute("aria-disabled", "true");
    await page.screenshot({ path: `${screenshotDir}/context-menu-${phone.name}.png`, animations: "disabled" });
    await page.keyboard.press("Escape");
    expect(errors).toEqual([]);
  });
});
