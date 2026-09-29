import { expect, test, type Locator, type Page } from "@playwright/test";
import {
  acceptanceViewports,
  clipCount,
  collectConsoleErrors,
  horizontalOverflow,
  isPhone,
  mod,
  openAcceptanceEditor,
  openTab,
  timelineClips,
  type EditorViewport,
} from "./support/editor";

// The spec's eight acceptance flows (plan 09 Task 4), at 1440×900 and at the iPhone 17 Pro's
// 402×874, on the acceptance fixture transport and through the UI alone. The sample opens
// untranscribed: Video 1 holds "Opening clip" (0–4 s) and "Restored Edison alternate" (4–8 s), and
// the timeline is 80 px per second. Each flow's end state is captured into output/editor-acceptance/.

const screenshotDir = "output/editor-acceptance";

function canvas(page: Page): Locator {
  return page.getByRole("region", { name: "Timeline canvas" });
}

function clip(page: Page, label: string): Locator {
  return canvas(page).getByRole("option", { name: new RegExp(`^${label},`) });
}

function properties(page: Page): Locator {
  return page.getByRole("complementary", { name: "Properties" });
}

async function send(panel: Locator, prompt: string) {
  const composer = panel.getByRole("textbox", { name: "Describe an edit" });
  await composer.fill(prompt);
  await composer.press("Enter");
}

/** Closes the open phone sheet: a modal sheet makes the tool bar and timeline inert (and hidden from roles). */
async function closeSheet(page: Page, viewport: EditorViewport) {
  if (!isPhone(viewport)) return;
  if ((await page.getByRole("dialog").count()) === 0) return;
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
}

/**
 * Selects "Opening clip" with the playhead `seconds` into it: a click on desktop, a tap on the phone
 * (where the track's sticky "Add media" button covers its right end, so stay under 2 s).
 */
async function selectOpeningClipAt(page: Page, viewport: EditorViewport, seconds: number) {
  const opening = clip(page, "Opening clip").first();
  const box = await opening.boundingBox();
  if (!box) throw new Error("Opening clip is not laid out");
  const point = { x: box.x + seconds * 80, y: box.y + box.height / 2 };
  if (isPhone(viewport)) await page.touchscreen.tap(point.x, point.y);
  else await page.mouse.click(point.x, point.y);
  await expect(opening).toHaveAttribute("aria-selected", "true");
}

/** Splits at the playhead: S on desktop, Clip tools → Split on the phone. */
async function splitAtPlayhead(page: Page, viewport: EditorViewport) {
  if (isPhone(viewport)) await page.getByRole("toolbar", { name: "Clip tools" }).getByRole("button", { name: "Split" }).click();
  else await page.keyboard.press("s");
}

/** Expects the timeline clip count, closing a phone sheet first so the timeline is exposed. */
async function expectClips(page: Page, viewport: EditorViewport, count: number) {
  await closeSheet(page, viewport);
  await expect(timelineClips(page)).toHaveCount(count);
}

async function finish(page: Page, viewport: EditorViewport, flow: string, errors: readonly string[]) {
  // A preview video remounted by the last edit (or resized when Properties closed) paints a frame late,
  // so the capture waits for loaded video and a couple of frames.
  await expect.poll(() => page.locator("video").evaluateAll((videos) => videos.every((video) => (video as HTMLVideoElement).readyState >= 2))).toBe(true);
  await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 250)))));
  await page.screenshot({ path: `${screenshotDir}/${flow}-${viewport.name}.png`, animations: "disabled" });
  expect(await horizontalOverflow(page)).toBeLessThanOrEqual(0);
  expect(errors).toEqual([]);
}

for (const viewport of acceptanceViewports) {
  const phone = isPhone(viewport);

  test.describe(`editor acceptance (${viewport.name})`, () => {
    test.use({ viewport: { width: viewport.width, height: viewport.height }, hasTouch: phone });

    test("1. open and import: Media → Import adds a tile, and + inserts it at the playhead", async ({ page }) => {
      const errors = collectConsoleErrors(page);
      await openAcceptanceEditor(page, viewport);
      const clips = await clipCount(page);
      expect(clips).toBeGreaterThan(0);

      const media = await openTab(page, "Media");
      const tiles = media.getByRole("button", { name: /^Add .+ at the playhead$/ });
      const before = await tiles.count();
      await media.getByRole("button", { name: "Import media" }).click();
      const add = media.getByRole("button", { name: "Add preview at the playhead" });
      await expect(add).toHaveCount(1);
      await expect(tiles).toHaveCount(before + 1);
      if (!phone) await add.hover();
      await add.click();
      await expectClips(page, viewport, clips + 1);
      await expect(clip(page, "preview")).toHaveCount(1);
      await finish(page, viewport, "1-import", errors);
    });

    test("2. agent, auto-apply on: a safe edit applies with facts, Show changes highlights it, and Undo restores it", async ({ page }) => {
      const errors = collectConsoleErrors(page);
      await openAcceptanceEditor(page, viewport);
      const clips = await clipCount(page);
      const panel = await openTab(page, "AI");
      await expect(panel.getByRole("switch", { name: "Auto-apply safe edits" })).toBeChecked();

      await send(panel, "Tighten the pacing");
      const applied = panel.getByRole("article", { name: /^Applied to / });
      await expect(applied.getByRole("list", { name: "Facts" }).getByText("0:08 → 0:06")).toBeVisible();

      await applied.getByRole("button", { name: "Show changes" }).click();
      await expect(page.getByTestId("clip-highlight").first()).toBeVisible();
      await expectClips(page, viewport, clips + 1);

      const again = await openTab(page, "AI");
      await again.getByRole("article", { name: /^Applied to / }).getByRole("button", { name: "Undo" }).click();
      await expect(again.getByRole("article", { name: "Undone" })).toBeVisible();
      await expectClips(page, viewport, clips);
      await expect(page.getByTestId("clip-highlight")).toHaveCount(0);
      await openTab(page, "AI");
      await finish(page, viewport, "2-agent-auto-apply", errors);
    });

    test("3. agent, review: Generate & place starts the generations, Undo removes them, and Dismiss changes nothing", async ({ page }) => {
      const errors = collectConsoleErrors(page);
      await openAcceptanceEditor(page, viewport);
      const clips = await clipCount(page);
      const panel = await openTab(page, "AI");

      await send(panel, "Generate three lab shots");
      const review = panel.getByRole("article", { name: "Needs your review" });
      await expect(review.getByRole("button", { name: "Generate & place" })).toBeFocused();
      await expectClips(page, viewport, clips);

      // The bundle generates three shots and places the first as a placeholder at the end.
      const reopened = await openTab(page, "AI");
      await reopened.getByRole("article", { name: "Needs your review" }).getByRole("button", { name: "Generate & place" }).click();
      const applied = reopened.getByRole("article", { name: /^Applied to / });
      await expect(applied.getByRole("list", { name: "Facts" }).getByText("3 × video · 4s")).toBeVisible();
      await expectClips(page, viewport, clips + 1);
      await expect(clip(page, "Lab bench wide shot")).toHaveCount(1);
      // Approval starts the generations: the first runs while the others wait their turn. Background
      // tasks lists each generation once (the bundle's `job-<asset>` job and its asset are one task).
      await closeSheet(page, viewport);
      const indicator = page.getByRole("button", { name: "Background tasks" });
      await expect(indicator).toHaveAccessibleDescription("Generating…");
      await indicator.click();
      const tasks = page.getByRole("dialog", { name: "Background tasks" });
      for (const name of ["Lab bench wide shot", "Beaker pour close-up", "Microscope focus pull"]) await expect(tasks.getByRole("listitem", { name })).toHaveCount(1);
      await page.keyboard.press("Escape");
      await expect(tasks).toHaveCount(0);

      // Undo removes the placeholder (or the output placed over it) and the generations with it.
      const again = await openTab(page, "AI");
      await again.getByRole("article", { name: /^Applied to / }).getByRole("button", { name: "Undo" }).click();
      await expect(again.getByRole("article", { name: "Undone" })).toBeVisible();
      await expectClips(page, viewport, clips);
      await expect(clip(page, "Lab bench wide shot")).toHaveCount(0);
      await expect(indicator).not.toHaveAccessibleDescription(/^Generat/);
      await indicator.click();
      for (const name of ["Lab bench wide shot", "Beaker pour close-up", "Microscope focus pull"]) await expect(tasks.getByRole("listitem", { name })).toHaveCount(0);
      await page.keyboard.press("Escape");
      await expect(tasks).toHaveCount(0);
      // A generation that was still running never comes back.
      await page.waitForTimeout(2_500);
      await expect(clip(page, "Lab bench wide shot")).toHaveCount(0);

      const last = await openTab(page, "AI");
      await send(last, "Generate three lab shots");
      const second = last.getByRole("article", { name: "Needs your review" });
      await second.getByRole("button", { name: "Dismiss" }).click();
      await expect(last.getByText("Dismissed")).toBeVisible();
      await expectClips(page, viewport, clips);
      await openTab(page, "AI");
      await finish(page, viewport, "3-agent-review", errors);
    });

    test("4. auto-apply off: a safe edit shows a review card first", async ({ page }) => {
      const errors = collectConsoleErrors(page);
      await openAcceptanceEditor(page, viewport);
      const clips = await clipCount(page);
      const panel = await openTab(page, "AI");
      const autoApply = panel.getByRole("switch", { name: "Auto-apply safe edits" });
      await autoApply.click();
      await expect(autoApply).not.toBeChecked();

      await send(panel, "Tighten the pacing");
      const review = panel.getByRole("article", { name: "Needs your review" });
      await expect(review.getByRole("button", { name: "Apply" })).toBeFocused();
      await expect(review.getByRole("list", { name: "Facts" }).getByText("0:08 → 0:06")).toBeVisible();
      await expect(panel.getByRole("article", { name: /^Applied to / })).toHaveCount(0);
      await expectClips(page, viewport, clips);
      await openTab(page, "AI");
      await finish(page, viewport, "4-auto-apply-off", errors);
    });

    test("5. edit a clip: opacity, keyframe, split at the playhead, then Undo and Redo", async ({ page }) => {
      const errors = collectConsoleErrors(page);
      await openAcceptanceEditor(page, viewport);
      const clips = await clipCount(page);
      await selectOpeningClipAt(page, viewport, 1.5);

      let fields: Locator;
      if (phone) {
        await page.getByRole("toolbar", { name: "Clip tools" }).getByRole("button", { name: "Adjust" }).click();
        fields = page.getByRole("dialog", { name: "Adjust" });
      } else {
        fields = properties(page);
      }
      const opacity = fields.getByRole("textbox", { name: "Opacity" });
      await opacity.fill("50");
      await opacity.press("Enter");
      await expect(opacity).toHaveValue("50%");
      await fields.getByRole("button", { name: "Add keyframe for Opacity" }).click();
      await expect(fields.getByRole("button", { name: "Remove keyframe for Opacity" })).toBeVisible();

      const undo = page.getByRole("button", { name: "Undo" });
      const redo = page.getByRole("button", { name: "Redo" });
      if (phone) {
        await fields.getByRole("button", { name: "Close Adjust" }).click();
        await expect(fields).toHaveCount(0);
        await splitAtPlayhead(page, viewport);
        await expect(timelineClips(page)).toHaveCount(clips + 1);
        await undo.click();
        await expect(timelineClips(page)).toHaveCount(clips);
        await redo.click();
      } else {
        await canvas(page).focus();
        await splitAtPlayhead(page, viewport);
        await expect(timelineClips(page)).toHaveCount(clips + 1);
        const modifier = await mod(page);
        await page.keyboard.press(`${modifier}+z`);
        await expect(timelineClips(page)).toHaveCount(clips);
        await page.keyboard.press(`Shift+${modifier}+z`);
      }
      await expect(timelineClips(page)).toHaveCount(clips + 1);
      await expect(redo).toBeDisabled();
      await expect(page.getByRole("region", { name: "Preview viewport" }).getByLabel("Timeline video Opening clip")).toHaveCSS("opacity", "0.5");
      await finish(page, viewport, "5-edit-clip", errors);
    });

    test("6. captions: generate, fix a word on the canvas, and restyle every cue", async ({ page }) => {
      test.slow();
      const errors = collectConsoleErrors(page);
      await openAcceptanceEditor(page, viewport);
      const clips = await clipCount(page);
      const captions = await openTab(page, "Captions");
      await captions.getByRole("button", { name: "Generate captions" }).click();

      const transcript = captions.getByRole("group", { name: "Transcript" });
      await expect(transcript.getByRole("button", { name: "newsreel" })).toBeVisible({ timeout: 15_000 });
      await expect(captions.getByText("2 captions · 1 word to check")).toBeVisible();

      await transcript.getByRole("button", { name: "newsreel" }).dblclick();
      const fix = captions.getByRole("textbox", { name: "Fix word “newsreel”" });
      await fix.fill("newsreels");
      await fix.press("Enter");
      await expect(transcript.getByRole("button", { name: "newsreels" })).toBeVisible();

      await captions.getByRole("radio", { name: "Styles" }).click();
      const presets = captions.getByRole("group", { name: "Caption style preset" });
      await presets.getByRole("button", { name: "Kinetic focus" }).click();
      await expect(presets.getByRole("button", { name: "Kinetic focus" })).toHaveAttribute("aria-pressed", "true");
      await expectClips(page, viewport, clips + 2);

      // Only the cue under the playhead draws, so each cue is visited in turn.
      const previewCaption = page.getByRole("region", { name: "Preview viewport" }).getByLabel(/^Timeline preview caption /);
      for (const text of ["The restored newsreels plays", "at its original speed"]) {
        const cue = timelineClips(page).filter({ hasText: text });
        await expect(cue).toHaveCount(1);
        await cue.click();
        await expect(previewCaption).toHaveText(text);
        await expect(previewCaption).toHaveAttribute("data-caption-style", "kineticFocus");
      }
      await finish(page, viewport, "6-captions", errors);
    });

    test("7. transition: + Crossfade on the selected cut, then a 1.0 s duration in Properties", async ({ page }) => {
      const errors = collectConsoleErrors(page);
      await openAcceptanceEditor(page, viewport);
      // Split "Opening clip" at 1.5 s so the cut has unused media on both sides.
      await selectOpeningClipAt(page, viewport, 1.5);
      await splitAtPlayhead(page, viewport);
      await expect(clip(page, "Opening clip split")).toHaveCount(1);
      if (phone) await page.getByRole("toolbar", { name: "Clip tools" }).getByRole("button", { name: "Back to editor tools" }).click();

      const effects = await openTab(page, "Effects");
      await effects.getByRole("group", { name: "Effects tab content" }).getByRole("button", { name: "Transitions" }).click();
      const add = effects.getByRole("button", { name: "Add Crossfade transition" });
      if (!phone) await effects.getByRole("list", { name: "Transitions" }).getByRole("listitem").first().hover();
      await add.click();
      const badge = (seconds: string) => canvas(page).getByRole("button", { name: `Crossfade transition, ${seconds}s` });
      await expect(badge("0.5")).toBeVisible();

      if (phone) {
        await page.getByRole("toolbar", { name: "Clip tools" }).getByRole("button", { name: "Duration" }).click();
        const sheet = page.getByRole("dialog", { name: "Duration" });
        const duration = sheet.getByRole("textbox", { name: "Duration" });
        await duration.fill("1.0");
        await duration.press("Enter");
        await expect(badge("1.0")).toBeVisible();
        await sheet.getByRole("button", { name: "Close Duration" }).click();
      } else {
        const duration = properties(page).getByRole("textbox", { name: "Duration" });
        await duration.fill("1.0");
        await duration.press("Enter");
        await expect(badge("1.0")).toBeVisible();
      }
      await expect(badge("0.5")).toHaveCount(0);
      await finish(page, viewport, "7-transition", errors);
    });

    test("8. export: MP4 · 1080p runs as a background task, completes, and Show in folder reveals it", async ({ page }) => {
      test.slow();
      const errors = collectConsoleErrors(page);
      await openAcceptanceEditor(page, viewport);
      await page.getByRole("button", { name: "Export", exact: true }).click();
      const dialog = page.getByRole("dialog", { name: "Export" });
      for (const name of ["MP4", "1080p"]) {
        const option = dialog.getByRole("radio", { name, exact: true });
        await option.click();
        await expect(option).toHaveAttribute("aria-checked", "true");
      }
      await dialog.getByRole("button", { name: "Export video" }).click();
      await expect(dialog).toHaveCount(0);

      const indicator = page.getByRole("button", { name: "Background tasks" });
      await expect(indicator).toHaveAccessibleDescription("Exporting…");
      const toast = page.getByRole("region", { name: /Notification/ }).getByRole("listitem").filter({ hasText: "Exported " });
      await expect(toast).toBeVisible({ timeout: 15_000 });
      await expect(indicator).toHaveAccessibleDescription("Export complete");
      await toast.getByRole("button", { name: "Show in folder" }).click();
      await expect
        .poll(() => page.evaluate(() => window.__EDITOR_FIXTURE_REVEALS__ ?? []))
        .toEqual([expect.objectContaining({ artifactPath: "exports/Edison Restoration Demo.mp4" })]);
      await indicator.click();
      const tasks = page.getByRole("dialog", { name: "Background tasks" });
      await expect(tasks.getByRole("listitem", { name: "H.264 Final export" })).toContainText("Completed");
      await finish(page, viewport, "8-export", errors);
    });
  });
}
