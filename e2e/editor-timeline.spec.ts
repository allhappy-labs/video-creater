import { expect, test, type Locator, type Page } from "@playwright/test";
import { acceptanceViewports, isPhone, openAcceptanceEditor, openTab, type EditorViewport } from "./support/editor";
import { openSampleEditor } from "./support/editor-fixture";

// Sample project: Video 1 holds "Opening clip" (0–4 s) and "Restored Edison alternate" (4–8 s),
// Audio 1 holds "Music bed" (0–4 s); captions start at 0.65 s. The timeline is 80 px per second.

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

test("desktop timeline selects, splits, undoes, drags and sets a duration", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const errors = await openSampleEditor(page);
  const options = canvas(page).getByRole("option");
  const playhead = page.getByTestId("playhead");

  // A click selects the clip and moves the playhead to the clicked point (120 px = 1.5 s).
  const opening = await clipById(page, "Opening clip");
  const box = await opening.boundingBox();
  if (!box) throw new Error("Opening clip is not laid out");
  await page.mouse.click(box.x + 120, box.y + box.height / 2);
  await expect(opening).toHaveAttribute("aria-selected", "true");
  await expect(playhead).toHaveAttribute("data-seconds", "1.500");

  const count = await options.count();
  await page.keyboard.press("s");
  await expect(options).toHaveCount(count + 1);
  await expect(opening).toHaveAttribute("aria-label", "Opening clip, 00:00:00, 00:00:01.500");

  // The fixture runtime reports macOS, so undo is Meta+Z.
  await page.keyboard.press("Meta+z");
  await expect(options).toHaveCount(count);
  await expect(page.getByRole("button", { name: "Undo" })).toBeDisabled();

  const music = await clipById(page, "Music bed");
  await expect(music).toHaveAttribute("aria-label", /^Music bed, 00:00:00, /);
  const musicBox = await music.boundingBox();
  if (!musicBox) throw new Error("Music bed is not laid out");
  const from = { x: musicBox.x + 40, y: musicBox.y + musicBox.height / 2 };
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 50, from.y, { steps: 5 });
  await page.mouse.move(from.x + 100, from.y, { steps: 5 });
  await page.mouse.up();
  await expect(music).toHaveAttribute("aria-label", /^Music bed, (?!00:00:00,)\d{2}:\d{2}:\d{2}/);

  const restored = await clipById(page, "Restored Edison alternate");
  await restored.click({ button: "right" });
  await page.getByRole("menuitem", { name: /^Set duration…/ }).click();
  const dialog = page.getByRole("dialog", { name: "Set duration" });
  await dialog.getByRole("textbox", { name: "Duration" }).fill("2");
  await dialog.getByRole("button", { name: "Set duration" }).click();
  await expect(dialog).toHaveCount(0);
  await expect(restored).toHaveAttribute("aria-label", "Restored Edison alternate, 00:00:04, 00:00:02");

  expect(errors).toEqual([]);
});

test("desktop timeline splits a clip from the keyboard alone", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const errors = await openSampleEditor(page);
  const options = canvas(page).getByRole("option");
  const count = await options.count();

  // Tab from the top of the page into the Video 1 track.
  let inVideoTrack = false;
  for (let presses = 0; presses < 80 && !inVideoTrack; presses += 1) {
    await page.keyboard.press("Tab");
    inVideoTrack = await page.evaluate(() => document.activeElement?.closest('[role="listbox"]')?.getAttribute("aria-label") === "Video 1");
  }
  expect(inVideoTrack).toBe(true);
  const opening = clip(page, "Opening clip");
  await expect(opening).toBeFocused();

  await page.keyboard.press("ArrowRight");
  await expect(clip(page, "Restored Edison alternate")).toBeFocused();
  await page.keyboard.press("ArrowLeft");
  await expect(opening).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(opening).toHaveAttribute("aria-selected", "true");

  // PageDown moves the playhead to the next edit point (the first caption at 0.65 s), inside the clip.
  await page.keyboard.press("PageDown");
  await expect(page.getByTestId("playhead")).toHaveAttribute("data-seconds", "0.650");
  await page.keyboard.press("s");
  await expect(options).toHaveCount(count + 1);
  expect(errors).toEqual([]);
});

/**
 * Imports preview.webm and adds it with `+` at the playhead, which puts it on a new track: six rows.
 * Every row must then lie inside the scroller without scrolling, and on desktop above the overview bar.
 */
async function expectRowsClearAfterImport(page: Page, viewport: EditorViewport) {
  const media = await openTab(page, "Media");
  await media.getByRole("button", { name: "Import media" }).click();
  await media.getByRole("button", { name: "Add preview at the playhead" }).click();
  if (isPhone(viewport) && (await page.getByRole("dialog").count()) > 0) await page.keyboard.press("Escape");
  await expect(clip(page, "preview")).toHaveCount(1);
  const rows = canvas(page).getByRole("listbox");
  await expect(rows).toHaveCount(6);

  const scroller = page.getByTestId("timeline-scroller");
  await expect.poll(() => scroller.evaluate((element) => element.scrollTop)).toBe(0);
  const scrollerBox = await scroller.boundingBox();
  const lastBox = await rows.last().boundingBox();
  if (!scrollerBox || !lastBox) throw new Error("timeline rows are not laid out");
  expect(lastBox.y + lastBox.height).toBeLessThanOrEqual(scrollerBox.y + scrollerBox.height + 0.5);
  if (!isPhone(viewport)) {
    const overviewBox = await page.getByRole("scrollbar", { name: "Timeline overview" }).boundingBox();
    if (!overviewBox) throw new Error("overview bar is not laid out");
    expect(lastBox.y + lastBox.height).toBeLessThanOrEqual(overviewBox.y);
  }
  await page.screenshot({ path: `output/editor-timeline/rows-after-import-${viewport.name}.png` });
}

for (const viewport of acceptanceViewports) {
  test.describe(`rows after import (${viewport.name})`, () => {
    test.use({ viewport: { width: viewport.width, height: viewport.height }, hasTouch: isPhone(viewport) });

    test("every track row stays fully visible above the overview bar", async ({ page }) => {
      await openAcceptanceEditor(page, viewport);
      await expectRowsClearAfterImport(page, viewport);
    });
  });
}

test.describe("phone", () => {
  test.use({ viewport: { width: 402, height: 874 }, hasTouch: true });

  test("iPhone 17 Pro timeline taps to select, splits from clip tools and scrolls the playhead", async ({ page }) => {
    const errors = await openSampleEditor(page);
    const options = canvas(page).getByRole("option");
    const playhead = page.getByTestId("playhead");
    const count = await options.count();

    const opening = await clipById(page, "Opening clip");
    await opening.tap();
    await expect(opening).toHaveAttribute("aria-selected", "true");
    const clipTools = page.getByRole("toolbar", { name: "Clip tools" });
    await expect(clipTools).toBeVisible();
    await expect(page.getByRole("toolbar", { name: "Editor tools" })).toHaveCount(0);
    // The tap moved the playhead into the clip, under the fixed centre line.
    await expect(playhead).not.toHaveAttribute("data-seconds", "0.000");

    await clipTools.getByRole("button", { name: "Split" }).tap();
    await expect(options).toHaveCount(count + 1);

    const before = Number(await playhead.getAttribute("data-seconds"));
    const scroller = page.getByTestId("timeline-scroller");
    const scrollerBox = await scroller.boundingBox();
    if (!scrollerBox) throw new Error("timeline scroller is not laid out");
    await page.mouse.move(scrollerBox.x + scrollerBox.width / 2, scrollerBox.y + scrollerBox.height / 2);
    await page.mouse.wheel(160, 0);
    await expect.poll(async () => Number(await playhead.getAttribute("data-seconds"))).toBeGreaterThan(before + 1);
    const playheadBox = await playhead.boundingBox();
    expect(Math.abs((playheadBox?.x ?? 0) + (playheadBox?.width ?? 0) / 2 - (scrollerBox.x + scrollerBox.width / 2))).toBeLessThanOrEqual(2);

    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    expect(overflow).toBeLessThanOrEqual(0);
    expect(errors).toEqual([]);
  });
});
