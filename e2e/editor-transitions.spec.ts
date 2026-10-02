import { expect, test, type Locator, type Page } from "@playwright/test";
import { openSampleEditor } from "./support/editor-fixture";

// Sample project: Video 1 holds "Opening clip" (0–4 s of the 4 s input.mp4) and "Restored Edison
// alternate" (4–8 s of a 4 s source). Neither clip has unused media at their 4 s cut, so each flow
// splits "Opening clip" first: both halves then have handles, allowing up to the shorter half.
// The timeline is 80 px per second.

function canvas(page: Page): Locator {
  return page.getByRole("region", { name: "Timeline canvas" });
}

function clip(page: Page, label: string): Locator {
  return canvas(page).getByRole("option", { name: new RegExp(`^${label},`) });
}

function badge(page: Page, name: string | RegExp): Locator {
  return canvas(page).getByRole("button", { name });
}

test("desktop adds a crossfade from Effects, sets its duration in Properties and undoes it", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const errors = await openSampleEditor(page);

  // Split "Opening clip" at 2 s (160 px) so the cut has unused media on both sides.
  const opening = clip(page, "Opening clip").first();
  const box = await opening.boundingBox();
  if (!box) throw new Error("Opening clip is not laid out");
  await page.mouse.click(box.x + 160, box.y + box.height / 2);
  await expect(page.getByTestId("playhead")).toHaveAttribute("data-seconds", "2.000");
  await page.keyboard.press("s");
  await expect(clip(page, "Opening clip split")).toHaveCount(1);

  await page.getByRole("tablist", { name: "Editor tools" }).getByRole("tab", { name: "Effects" }).click();
  const effects = page.getByRole("tabpanel", { name: "Effects" });
  const chip = effects.getByRole("group", { name: "Effects tab content" }).getByRole("button", { name: "Transitions" });
  await chip.click();
  await expect(chip).toHaveAttribute("aria-pressed", "true");
  const tiles = effects.getByRole("list", { name: "Transitions" });
  await expect(tiles.getByRole("listitem")).toHaveCount(4);
  await expect(effects.getByText(/\+ adds at the cut at 00:00:02 on Video 1/)).toBeVisible();
  const add = effects.getByRole("button", { name: "Add Crossfade transition" });
  await tiles.getByRole("listitem").first().hover();
  await expect(add).not.toHaveAttribute("aria-disabled");
  await page.screenshot({ path: testInfo.outputPath("transitions-effects-desktop.png") });

  await add.click();
  const crossfade = badge(page, "Crossfade transition, 0.5s");
  await expect(crossfade).toBeVisible();
  await expect(crossfade).toHaveAttribute("aria-pressed", "true");
  const properties = page.getByRole("complementary", { name: "Properties" });
  await expect(properties.getByRole("heading", { level: 2, name: "Crossfade transition" })).toBeVisible();
  await expect(properties.getByRole("tab", { name: "Transition" })).toHaveAttribute("data-state", "active");
  await expect(properties.getByText("Max 2.0s")).toBeVisible();

  const duration = properties.getByRole("textbox", { name: "Duration" });
  await duration.fill("1.0");
  await duration.press("Enter");
  await expect(badge(page, "Crossfade transition, 1.0s")).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath("transitions-badge-properties-desktop.png") });

  const undo = page.getByRole("button", { name: "Undo" });
  await undo.click();
  await expect(badge(page, "Crossfade transition, 0.5s")).toBeVisible();
  await undo.click();
  await expect(badge(page, /transition,/)).toHaveCount(0);
  await expect(clip(page, "Opening clip split")).toHaveCount(1);
  expect(errors).toEqual([]);
});

test("desktop previews each transition kind at its midpoint", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const errors = await openSampleEditor(page);

  const opening = clip(page, "Opening clip").first();
  const box = await opening.boundingBox();
  if (!box) throw new Error("Opening clip is not laid out");
  await page.mouse.click(box.x + 160, box.y + box.height / 2);
  await expect(page.getByTestId("playhead")).toHaveAttribute("data-seconds", "2.000");
  await page.keyboard.press("s");
  await expect(clip(page, "Opening clip split")).toHaveCount(1);

  await page.getByRole("tablist", { name: "Editor tools" }).getByRole("tab", { name: "Effects" }).click();
  const effects = page.getByRole("tabpanel", { name: "Effects" });
  await effects.getByRole("group", { name: "Effects tab content" }).getByRole("button", { name: "Transitions" }).click();
  await effects.getByRole("list", { name: "Transitions" }).getByRole("listitem").first().hover();
  await effects.getByRole("button", { name: "Add Crossfade transition" }).click();
  const properties = page.getByRole("complementary", { name: "Properties" });
  const duration = properties.getByRole("textbox", { name: "Duration" });
  await duration.fill("1.0");
  await duration.press("Enter");
  await expect(badge(page, "Crossfade transition, 1.0s")).toBeVisible();

  // The playhead sits on the cut, the midpoint of the 1.0 s window: both clips draw, the outgoing
  // clip beneath the incoming one.
  const preview = page.getByRole("region", { name: "Preview viewport" });
  const layers = preview.getByTestId("preview-layer");
  await expect(layers).toHaveCount(2);
  await expect(layers.nth(0)).toHaveAttribute("data-transition-role", "outgoing");
  await expect(layers.nth(1)).toHaveAttribute("data-transition-role", "incoming");
  await expect(layers.nth(1)).toHaveAttribute("data-transition-progress", "0.5");
  const opacities = () => layers.locator("video").evaluateAll((videos) => videos.map((video) => Number(getComputedStyle(video).opacity)));
  await expect.poll(opacities).toEqual([1, 0.5]);
  await page.screenshot({ path: testInfo.outputPath("transition-midpoint-crossfade-desktop.png") });

  const type = properties.getByRole("radiogroup", { name: "Transition type" });
  for (const [label, color] of [["Dip to black", "black"], ["Dip to white", "white"]] as const) {
    await type.getByRole("radio", { name: label }).click();
    await expect(preview.getByTestId("preview-transition-solid")).toHaveAttribute("data-color", color);
    await expect.poll(opacities).toEqual([0, 0]);
    await page.screenshot({ path: testInfo.outputPath(`transition-midpoint-${color === "black" ? "dip-to-black" : "dip-to-white"}-desktop.png`) });
  }

  await type.getByRole("radio", { name: "Wipe" }).click();
  await expect(preview.getByTestId("preview-transition-solid")).toHaveCount(0);
  await expect(layers.nth(1)).toHaveCSS("clip-path", "inset(0px 50% 0px 0px)");
  await expect.poll(opacities).toEqual([1, 1]);
  await page.screenshot({ path: testInfo.outputPath("transition-midpoint-wipe-desktop.png") });
  expect(errors).toEqual([]);
});

test.describe("phone", () => {
  test.use({ viewport: { width: 402, height: 874 }, hasTouch: true });

  test("iPhone 17 Pro adds a crossfade with +, edits it from the clip tools and undoes it", async ({ page }, testInfo) => {
    const errors = await openSampleEditor(page);

    // Tap 1.5 s into "Opening clip" and split there from the clip tools.
    const opening = clip(page, "Opening clip").first();
    const box = await opening.boundingBox();
    if (!box) throw new Error("Opening clip is not laid out");
    await page.touchscreen.tap(box.x + 120, box.y + box.height / 2);
    const clipTools = page.getByRole("toolbar", { name: "Clip tools" });
    await clipTools.getByRole("button", { name: "Split" }).tap();
    await expect(clip(page, "Opening clip split")).toHaveCount(1);
    await clipTools.getByRole("button", { name: "Back to editor tools" }).tap();

    await page.getByRole("toolbar", { name: "Editor tools" }).getByRole("button", { name: "Effects" }).tap();
    const sheet = page.getByRole("dialog", { name: "Effects" });
    const chip = sheet.getByRole("group", { name: "Effects tab content" }).getByRole("button", { name: "Transitions" });
    await chip.tap();
    await expect(chip).toHaveAttribute("aria-pressed", "true");
    await expect(sheet.getByRole("list", { name: "Transitions" }).getByRole("listitem")).toHaveCount(4);
    // Touch shows every tile's + without hover.
    const add = sheet.getByRole("button", { name: "Add Crossfade transition" });
    await expect(add).toHaveCSS("opacity", "1");
    await page.screenshot({ path: testInfo.outputPath("transitions-effects-phone.png") });

    await add.tap();
    await expect(sheet).toHaveCount(0);
    const crossfade = badge(page, "Crossfade transition, 0.5s");
    await expect(crossfade).toBeVisible();
    await expect(crossfade).toHaveAttribute("aria-pressed", "true");
    await expect(clipTools.getByRole("button")).toHaveText(["Tools", "Type", "Duration", "Delete"]);
    const fits = await clipTools.evaluate((toolbar) => toolbar.scrollWidth <= toolbar.clientWidth && toolbar.getBoundingClientRect().right <= 402);
    expect(fits).toBe(true);
    await page.screenshot({ path: testInfo.outputPath("transitions-badge-tools-phone.png") });

    // Tapping the badge after clearing the selection selects it again.
    await clipTools.getByRole("button", { name: "Back to editor tools" }).tap();
    await expect(crossfade).toHaveAttribute("aria-pressed", "false");
    await crossfade.tap();
    await expect(crossfade).toHaveAttribute("aria-pressed", "true");

    await clipTools.getByRole("button", { name: "Duration" }).tap();
    const durationSheet = page.getByRole("dialog", { name: "Duration" });
    const duration = durationSheet.getByRole("textbox", { name: "Duration" });
    await duration.fill("1.0");
    await duration.press("Enter");
    await expect(badge(page, "Crossfade transition, 1.0s")).toHaveCount(1);
    await expect(durationSheet.getByText(/^Max \d\.\d+s$/)).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath("transitions-duration-phone.png") });
    await durationSheet.getByRole("button", { name: "Close Duration" }).tap();

    const undo = page.getByRole("button", { name: "Undo" });
    await undo.tap();
    await expect(badge(page, "Crossfade transition, 0.5s")).toHaveCount(1);
    await undo.tap();
    await expect(badge(page, /transition,/)).toHaveCount(0);
    expect(errors).toEqual([]);
  });
});
