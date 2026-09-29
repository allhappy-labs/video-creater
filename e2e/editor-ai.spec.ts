import { expect, test, type Locator, type Page } from "@playwright/test";
import { openSampleEditor } from "./support/editor-fixture";

// Spec acceptance flows 2–4 and the AI tab failure path, at 1440×900 and 402×874. The conversation
// fixture answers by keyword: "tighten" is a safe cut (the last clip is split and two seconds cut, so
// the timeline goes 0:08 → 0:06 with one more clip), "generate" a review bundle of three generations,
// "fail" a proposal that doesn't validate, and anything else a safe caption fix. Without it the
// fixture backend reports the agent as unavailable.

const screenshotDir = "output/editor-ai";
const validationCopy = "The edit couldn't be validated: The proposed cut ends after the end of the timeline. Try rephrasing or narrowing the request.";

const viewports = [
  { label: "desktop", width: 1440, height: 900, phone: false },
  { label: "phone", width: 402, height: 874, phone: true },
] as const;

type Viewport = (typeof viewports)[number];

async function expectNoInternalIds(panel: Locator) {
  const text = await panel.innerText();
  expect(text).not.toMatch(/media-\d+|item-\d+|caption-\d+|track-[a-z]+|codex-action|fixture-|agent-lab-shot|agent-edit-|sample-generated|generated-placeholder|app-server-turn/);
}

function timelineClips(page: Page): Locator {
  return page.getByRole("region", { name: "Timeline canvas" }).getByRole("option");
}

/** The AI surface: the desktop AI tab, or the phone AI sheet opened from the tool bar. */
async function openAi(page: Page, viewport: Viewport): Promise<Locator> {
  if (!viewport.phone) return page.getByRole("tabpanel", { name: "AI" });
  const sheet = page.getByRole("dialog", { name: "AI" });
  if (!(await sheet.isVisible())) await page.getByRole("toolbar", { name: "Editor tools" }).getByRole("button", { name: "AI" }).click();
  await expect(sheet.getByRole("textbox", { name: "Describe an edit" })).toBeVisible();
  return sheet;
}

async function closeAi(page: Page, viewport: Viewport) {
  if (!viewport.phone) return;
  await page.getByRole("button", { name: "Close AI" }).click();
  await expect(page.getByRole("dialog", { name: "AI" })).toHaveCount(0);
}

async function send(panel: Locator, prompt: string) {
  const composer = panel.getByRole("textbox", { name: "Describe an edit" });
  await composer.fill(prompt);
  await composer.press("Enter");
}

async function shot(page: Page, name: string, viewport: Viewport) {
  await page.screenshot({ path: `${screenshotDir}/${name}-${viewport.label}.png` });
}

for (const viewport of viewports) {
  test.describe(`AI tab (${viewport.label})`, () => {
    test.beforeEach(async ({ page }) => {
      await page.setViewportSize({ width: viewport.width, height: viewport.height });
    });

    test("auto-apply on: a safe edit applies with facts and frames, Show changes highlights it, and Undo restores it", async ({ page }) => {
      const errors = await openSampleEditor(page, { conversationFixture: true });
      const clips = await timelineClips(page).count();
      const panel = await openAi(page, viewport);
      await expect(panel.getByRole("switch", { name: "Auto-apply safe edits" })).toBeChecked();
      await expect(panel.getByRole("textbox", { name: "Describe an edit" })).toHaveValue("");
      await expect(panel.getByRole("list", { name: "Quick edits" })).toBeVisible();
      await shot(page, "empty", viewport);

      await send(panel, "Tighten the pacing");
      await expect(panel.getByRole("status").filter({ hasText: "Reviewing the timeline" })).toBeVisible();
      await shot(page, "working", viewport);

      const applied = panel.getByRole("article", { name: /^Applied to / });
      await expect(applied.getByRole("list", { name: "Facts" }).getByText("0:08 → 0:06")).toBeVisible();
      await expect(applied.getByRole("list", { name: "Facts" }).getByText("2 cuts")).toBeVisible();
      // Applied results leave focus with the composer.
      await expect(panel.getByRole("textbox", { name: "Describe an edit" })).toBeFocused();
      const frame = applied.getByRole("group", { name: "Result preview" }).getByRole("button", { name: "Play from 0:05" });
      await expect(frame).toBeVisible();
      await expect.poll(() => frame.locator("img").evaluate((image: HTMLImageElement) => image.complete && image.naturalWidth > 0)).toBe(true);
      await expect(applied.getByText("Preview frames aren't available.")).toHaveCount(0);
      await expectNoInternalIds(panel);
      await shot(page, "applied", viewport);

      await applied.getByRole("button", { name: "Show changes" }).click();
      await expect(page.getByTestId("clip-highlight").first()).toBeVisible();
      await expect(timelineClips(page).filter({ has: page.getByTestId("clip-highlight") }).first()).toHaveAccessibleDescription(/Changed by the AI edit/);
      await expect(timelineClips(page)).toHaveCount(clips + 1);
      // The frame capture's job is preview plumbing, so Background tasks still reports the edit.
      await expect(page.getByRole("button", { name: "Background tasks" })).toHaveAccessibleDescription("Agent edit complete");
      await shot(page, "show-changes", viewport);

      const again = await openAi(page, viewport);
      await again.getByRole("article", { name: /^Applied to / }).getByRole("button", { name: "Undo" }).click();
      await expect(again.getByRole("article", { name: "Undone" })).toBeVisible();
      await closeAi(page, viewport);
      await expect(timelineClips(page)).toHaveCount(clips);
      await expect(page.getByTestId("clip-highlight")).toHaveCount(0);
      expect(errors).toEqual([]);
    });

    test("review: a generation waits for Generate & place, and Dismiss leaves the project unchanged", async ({ page }) => {
      const errors = await openSampleEditor(page, { conversationFixture: true });
      const clips = await timelineClips(page).count();
      const panel = await openAi(page, viewport);
      await send(panel, "Generate three lab shots");

      const review = panel.getByRole("article", { name: "Needs your review" });
      await expect(review.getByRole("button", { name: "Generate & place" })).toBeFocused();
      await expect(review.getByRole("list", { name: "Planned changes" })).toContainText("Generate video “Lab bench wide shot” and place it on the timeline at 0:08");
      await expect(review.getByRole("button", { name: "Dismiss" })).toBeInViewport();
      await expect(panel.getByRole("list", { name: "Quick edits" })).toHaveCount(0);
      await expectNoInternalIds(panel);
      await shot(page, "review", viewport);

      await closeAi(page, viewport);
      await expect(timelineClips(page)).toHaveCount(clips);
      const reopened = await openAi(page, viewport);
      await reopened.getByRole("article", { name: "Needs your review" }).getByRole("button", { name: "Dismiss" }).click();
      await expect(reopened.getByText("Dismissed")).toBeVisible();
      await expect(reopened.getByRole("textbox", { name: "Describe an edit" })).toBeFocused();
      await closeAi(page, viewport);
      await expect(timelineClips(page)).toHaveCount(clips);
      expect(errors).toEqual([]);
    });

    test("auto-apply off: a safe edit shows a review card first, and Apply applies it", async ({ page }) => {
      const errors = await openSampleEditor(page, { conversationFixture: true });
      const clips = await timelineClips(page).count();
      const panel = await openAi(page, viewport);
      const autoApply = panel.getByRole("switch", { name: "Auto-apply safe edits" });
      await autoApply.click();
      await expect(autoApply).not.toBeChecked();

      await send(panel, "Tighten the pacing");
      const review = panel.getByRole("article", { name: "Needs your review" });
      await expect(review.getByRole("button", { name: "Apply" })).toBeFocused();
      await expect(review.getByRole("list", { name: "Facts" }).getByText("0:08 → 0:06")).toBeVisible();
      await expectNoInternalIds(panel);
      await shot(page, "review-safe", viewport);

      await review.getByRole("button", { name: "Apply" }).click();
      const applied = panel.getByRole("article", { name: /^Applied to / });
      await expect(applied.getByRole("list", { name: "Facts" }).getByText("0:08 → 0:06")).toBeVisible();
      await expect(panel.getByRole("article", { name: "Needs your review" })).toHaveCount(0);
      await closeAi(page, viewport);
      await expect(timelineClips(page)).toHaveCount(clips + 1);
      expect(errors).toEqual([]);
    });

    test("failure: a proposal that doesn't validate explains why and keeps the draft", async ({ page }) => {
      const errors = await openSampleEditor(page, { conversationFixture: true });
      const clips = await timelineClips(page).count();
      const panel = await openAi(page, viewport);
      await send(panel, "fail please");

      const failure = panel.getByRole("article", { name: "Couldn't validate the edit" });
      await expect(failure.getByText(validationCopy)).toBeVisible();
      await expect(panel.getByRole("textbox", { name: "Describe an edit" })).toHaveValue("fail please");
      await expectNoInternalIds(panel);
      await shot(page, "failure", viewport);
      await closeAi(page, viewport);
      await expect(timelineClips(page)).toHaveCount(clips);
      expect(errors).toEqual([]);
    });
  });
}

test("desktop missing-agent state", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const errors = await openSampleEditor(page);
  const panel = page.getByRole("tabpanel", { name: "AI" });
  await panel.getByRole("list", { name: "Quick edits" }).getByRole("button", { name: "Tighten the pacing" }).click();
  const composer = panel.getByRole("textbox", { name: "Describe an edit" });
  await expect(composer).toHaveValue(/^Tighten the pacing/);
  await expect(composer).toBeFocused();
  await composer.press("Enter");
  const missing = panel.getByRole("region", { name: "AI agent unavailable" });
  await expect(missing.getByRole("button", { name: "Open Agent settings" })).toBeVisible();
  await page.screenshot({ path: `${screenshotDir}/missing-agent-desktop.png` });
  expect(errors).toEqual([]);
});

test("desktop Organize with AI asks before sending, and Ask AI only fills the context chip", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const errors = await openSampleEditor(page, { conversationFixture: true });
  const tabs = page.getByRole("tablist", { name: "Editor tools" });
  await tabs.getByRole("tab", { name: "Media" }).click();
  await page.getByRole("tabpanel", { name: "Media" }).getByRole("button", { name: "More media actions" }).click();
  await page.getByRole("menuitem", { name: "Organize with AI" }).click();

  const panel = page.getByRole("tabpanel", { name: "AI" });
  const composer = panel.getByRole("textbox", { name: "Describe an edit" });
  const confirmation = panel.getByRole("group", { name: "Send this request?" });
  await expect(composer).toHaveValue(/^Organize the current project media/);
  await expect(confirmation.getByRole("button", { name: "Send" })).toBeFocused();
  await page.screenshot({ path: `${screenshotDir}/organize-confirm-desktop.png` });
  await confirmation.getByRole("button", { name: "Send" }).click();
  await expect(panel.getByRole("article", { name: /^Applied to / })).toBeVisible();
  await expect(confirmation).toHaveCount(0);

  await page.getByRole("region", { name: "Timeline canvas" }).getByRole("option", { name: /^Opening clip,/ }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "Ask AI about this clip" }).click();
  await expect(composer).toBeFocused();
  await expect(composer).toHaveValue("");
  await expect(panel.getByRole("button", { name: "Remove Opening clip from the request" })).toBeVisible();
  await expect(confirmation).toHaveCount(0);
  await expect(panel.getByRole("article", { name: "Needs your review" })).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("phone Ask AI opens the AI sheet with the clip as context", async ({ page }) => {
  await page.setViewportSize({ width: 402, height: 874 });
  const errors = await openSampleEditor(page, { conversationFixture: true });
  await page.getByRole("region", { name: "Timeline canvas" }).getByRole("option", { name: /^Opening clip,/ }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "Ask AI about this clip" }).click();
  const sheet = page.getByRole("dialog", { name: "AI" });
  const composer = sheet.getByRole("textbox", { name: "Describe an edit" });
  await expect(composer).toBeFocused();
  await expect(sheet.getByRole("button", { name: "Remove Opening clip from the request" })).toBeVisible();
  await expect(sheet.getByRole("group", { name: "Send this request?" })).toHaveCount(0);
  await page.screenshot({ path: `${screenshotDir}/ask-phone.png` });
  expect(errors).toEqual([]);
});
