import { expect, test, type Locator, type Page } from "@playwright/test";
import { openSampleEditor } from "./support/editor-fixture";

// Sample project: Video 1 holds "Opening clip" (0–4 s, media input.mp4) and "Restored Edison
// alternate" (4–8 s); captions "Original caption text" (0.65–2 s) and "Second clean split"
// (2.15–3.35 s) come from the input.mp4 transcript. The panel fixtures add an effect catalog and
// two detected silences on input.mp4 that review as 1.0–2.0 s and 2.5–3.3 s.

function canvas(page: Page): Locator {
  return page.getByRole("region", { name: "Timeline canvas" });
}

function clip(page: Page, label: string): Locator {
  return canvas(page).getByRole("option", { name: new RegExp(`^${label},`) });
}

function properties(page: Page): Locator {
  return page.getByRole("complementary", { name: "Properties" });
}

async function openTab(page: Page, name: string): Promise<Locator> {
  await page.getByRole("tablist", { name: "Editor tools" }).getByRole("tab", { name }).click();
  const panel = page.getByRole("tabpanel", { name });
  await expect(panel).toBeVisible();
  return panel;
}

async function timelineDuration(page: Page): Promise<number> {
  return Number(await page.getByRole("slider", { name: "Preview scrubber" }).getAttribute("aria-valuemax"));
}

test.describe("desktop editor panels", () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
  });

  test("Media filters the library and + inserts a clip on a video track", async ({ page }) => {
    const errors = await openSampleEditor(page, { panelFixtures: true });
    const media = await openTab(page, "Media");
    const library = media.getByRole("list", { name: "Media library" });

    // The sample has no still images, so the Images chip empties the grid; the video tile inserts instead.
    await media.getByRole("group", { name: "Media type" }).getByRole("button", { name: "Images" }).click();
    await expect(library.getByRole("button", { name: /^Preview / })).toHaveCount(0);
    await media.getByRole("group", { name: "Media type" }).getByRole("button", { name: "Video" }).click();
    await expect(library.getByRole("button", { name: /^Preview / })).toHaveCount(2);

    await expect(clip(page, "input\\.mp4")).toHaveCount(0);
    await library.getByRole("button", { name: "Add input.mp4 at the playhead" }).click();
    const inserted = clip(page, "input\\.mp4");
    await expect(inserted).toHaveCount(1);
    const track = await inserted.evaluate((option) => option.closest('[role="listbox"]')?.getAttribute("aria-label") ?? "");
    expect(track).toMatch(/^Video \d+$/);
    expect(errors).toEqual([]);
  });

  test("Text adds a text item and opens its Properties Text tab", async ({ page }) => {
    const errors = await openSampleEditor(page, { panelFixtures: true });
    const text = await openTab(page, "Text");
    await text.getByRole("button", { name: "Add text" }).click();

    const aside = properties(page);
    await expect(aside).toBeVisible();
    await expect(aside.getByRole("tab", { name: "Text" })).toHaveAttribute("data-state", "active");
    await expect(canvas(page).getByRole("option", { selected: true })).toHaveCount(1);
    expect(errors).toEqual([]);
  });

  test("Captions fixes a transcript word, updates its caption, and Undo reverts both", async ({ page }) => {
    const errors = await openSampleEditor(page, { panelFixtures: true });
    const captions = await openTab(page, "Captions");
    const transcript = captions.getByRole("group", { name: "Transcript" });
    const firstCaption = canvas(page).getByRole("listbox", { name: "Captions" }).getByRole("option").first();
    await expect(firstCaption).toHaveText("Original caption text");

    await transcript.getByRole("button", { name: "caption", exact: true }).dblclick();
    const editor = captions.getByRole("textbox", { name: "Fix word “caption”" });
    await editor.fill("captions");
    await editor.press("Enter");

    await expect(transcript.getByRole("button", { name: "captions", exact: true })).toBeVisible();
    await expect(firstCaption).toHaveText("Original captions text");

    await page.getByRole("button", { name: "Undo" }).click();
    await expect(firstCaption).toHaveText("Original caption text");
    await expect(transcript.getByRole("button", { name: "caption", exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Undo" })).toBeDisabled();
    expect(errors).toEqual([]);
  });

  test("Effects applies an effect to the selected clip and Properties lists it", async ({ page }) => {
    const errors = await openSampleEditor(page, { panelFixtures: true });
    const effects = await openTab(page, "Effects");
    await expect(effects.getByRole("button", { name: "Apply Film Grain" })).toBeDisabled();

    await clip(page, "Opening clip").click();
    await expect(clip(page, "Opening clip")).toHaveAttribute("aria-selected", "true");
    await effects.getByRole("button", { name: "Apply Film Grain" }).click();

    const section = properties(page).getByRole("region", { name: "Effects" });
    await expect(section.getByText("Film Grain")).toBeVisible();
    expect(errors).toEqual([]);
  });

  test("Audio reviews detected pauses and cuts only the checked ranges", async ({ page }) => {
    const errors = await openSampleEditor(page, { panelFixtures: true });
    const audio = await openTab(page, "Audio");
    const card = audio.getByRole("listitem", { name: "Remove silences" });
    await expect(card).toContainText("2 pauses · saves 1.8s");
    const before = await timelineDuration(page);

    await card.getByRole("button", { name: "Review pauses" }).click();
    const dialog = page.getByRole("dialog", { name: "Review pauses" });
    await expect(dialog.getByRole("checkbox")).toHaveCount(2);
    // Pause 1 (1.0–2.0 s) stays; only pause 2 (2.5–3.3 s, 0.8 s) is cut.
    await dialog.getByRole("checkbox", { name: "Include pause 1" }).uncheck();
    await expect(dialog).toContainText("1 pause selected · saves 0.8s");
    await dialog.getByRole("button", { name: "Apply 1 cut" }).click();
    await expect(dialog).toHaveCount(0);

    await expect.poll(() => timelineDuration(page)).toBeCloseTo(before - 0.8, 3);
    await expect(clip(page, "Restored Edison alternate")).toHaveAttribute("aria-label", /^Restored Edison alternate, 00:00:03\.200,/);
    expect(errors).toEqual([]);
  });

  test("Generate shows the cost and explains why an empty prompt cannot generate", async ({ page }) => {
    const errors = await openSampleEditor(page, { panelFixtures: true });
    const media = await openTab(page, "Media");
    await media.getByRole("button", { name: "Generate", exact: true }).click();

    const view = media.getByRole("region", { name: "Generate" });
    await expect(view).toBeVisible();
    await expect(view.getByRole("radio", { name: "Video" })).toHaveAttribute("aria-checked", "true");
    // The settings fixture catalog carries no pricing, so the estimate reads "varies"; the network notice names the provider.
    await expect(view.getByText("Est. varies", { exact: true })).toBeVisible();
    await expect(view.getByText("Uses OpenAI · network")).toBeVisible();
    const generate = view.getByRole("button", { name: "Generate", exact: true });
    await expect(generate).toBeDisabled();
    await expect(generate).toHaveAccessibleDescription("Prompt required");

    // Typing a prompt clears the reason; nothing starts until Generate is pressed.
    await view.getByRole("textbox", { name: "Prompt" }).fill("Brass phonograph on a lab bench");
    await expect(generate).toBeEnabled();
    await expect(view.getByText("Prompt required")).toHaveCount(0);
    expect(errors).toEqual([]);
  });
});

test("iPhone 17 Pro Media sheet inserts a tile onto the timeline", async ({ page }) => {
  await page.setViewportSize({ width: 402, height: 874 });
  const errors = await openSampleEditor(page, { panelFixtures: true });

  await page.getByRole("toolbar", { name: "Editor tools" }).getByRole("button", { name: "Media" }).click();
  const sheet = page.getByRole("dialog", { name: "Media" });
  await expect(sheet).toBeVisible();
  await sheet.getByRole("button", { name: "Add input.mp4 at the playhead" }).click();
  await sheet.getByRole("button", { name: "Close Media" }).click();
  await expect(sheet).toHaveCount(0);

  await expect(clip(page, "input\\.mp4")).toHaveCount(1);
  expect(errors).toEqual([]);
});
