import { expect, test, type Page } from "@playwright/test";
import {
  acceptanceViewports,
  collectConsoleErrors,
  horizontalOverflow,
  isPhone,
  openAcceptanceEditor,
  openTab,
  timelineClips,
  type EditorViewport,
} from "./support/editor";

const canvas = (page: Page) => page.getByRole("region", { name: "Timeline canvas" });

async function closeSheet(page: Page, viewport: EditorViewport) {
  if (!isPhone(viewport) || (await page.getByRole("dialog").count()) === 0) return;
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
}

for (const viewport of acceptanceViewports) {
  test.describe(`continuous editing session (${viewport.name})`, () => {
    test.use({ viewport: { width: viewport.width, height: viewport.height }, hasTouch: isPhone(viewport) });

    test("AI edit, import, split, transition, captions, export in one project", async ({ page }) => {
      test.slow();
      const errors = collectConsoleErrors(page);
      await openAcceptanceEditor(page, viewport);
      const originalCount = await timelineClips(page).count();

      const ai = await openTab(page, "AI");
      const composer = ai.getByRole("textbox", { name: "Describe an edit" });
      await composer.fill("Tighten the pacing");
      await composer.press("Enter");
      await expect(ai.getByRole("article", { name: /^Applied to / })).toBeVisible();
      await closeSheet(page, viewport);
      await expect(timelineClips(page)).toHaveCount(originalCount + 1);

      const media = await openTab(page, "Media");
      await media.getByRole("button", { name: "Import media" }).click();
      await media.getByRole("button", { name: "Add preview at the playhead" }).click();
      await closeSheet(page, viewport);
      await expect(timelineClips(page)).toHaveCount(originalCount + 2);

      const opening = canvas(page).getByRole("option", { name: /^Opening clip,/ }).first();
      const box = await opening.boundingBox();
      if (!box) throw new Error("Opening clip did not lay out");
      const point = { x: box.x + 120, y: box.y + box.height / 2 };
      if (isPhone(viewport)) await page.touchscreen.tap(point.x, point.y);
      else await page.mouse.click(point.x, point.y);
      await expect(opening).toHaveAttribute("aria-selected", "true");
      if (isPhone(viewport)) await page.getByRole("toolbar", { name: "Clip tools" }).getByRole("button", { name: "Split" }).click();
      else await page.keyboard.press("s");
      await expect(timelineClips(page)).toHaveCount(originalCount + 3);

      if (isPhone(viewport)) await page.getByRole("toolbar", { name: "Clip tools" }).getByRole("button", { name: "Back to editor tools" }).click();
      const effects = await openTab(page, "Effects");
      await effects.getByRole("group", { name: "Effects tab content" }).getByRole("button", { name: "Transitions" }).click();
      await effects.getByRole("button", { name: "Add Crossfade transition" }).click();
      await closeSheet(page, viewport);
      await expect(canvas(page).getByRole("button", { name: "Crossfade transition, 0.5s" })).toBeVisible();
      if (isPhone(viewport)) {
        await page.getByRole("toolbar", { name: "Clip tools" }).getByRole("button", { name: "Duration" }).click();
        const durationSheet = page.getByRole("dialog", { name: "Duration" });
        const duration = durationSheet.getByRole("textbox", { name: "Duration" });
        await duration.fill("1.0");
        await duration.press("Enter");
        await durationSheet.getByRole("button", { name: "Close Duration" }).click();
      } else {
        const duration = page.getByRole("complementary", { name: "Properties" }).getByRole("textbox", { name: "Duration" });
        await duration.fill("1.0");
        await duration.press("Enter");
      }
      await expect(canvas(page).getByRole("button", { name: "Crossfade transition, 1.0s" })).toBeVisible();

      if (isPhone(viewport)) {
        const back = page.getByRole("toolbar", { name: "Clip tools" }).getByRole("button", { name: "Back to editor tools" });
        await expect(back.getByText("Tools")).toBeVisible();
        await page.screenshot({ path: "output/editor-acceptance/continuous-phone-clip-tools.png", animations: "disabled" });
        await back.click();
      }
      const captions = await openTab(page, "Captions");
      await captions.getByRole("button", { name: "Generate captions" }).click();
      await expect(captions.getByText(/\d+ captions? · 1 word to check/)).toBeVisible({ timeout: 15_000 });
      const transcript = captions.getByRole("group", { name: "Transcript" });
      await transcript.getByRole("button", { name: "newsreel" }).dblclick();
      const correctedWord = captions.getByRole("textbox", { name: "Fix word “newsreel”" });
      await correctedWord.fill("newsreels");
      await correctedWord.press("Enter");
      await expect(transcript.getByRole("button", { name: "newsreels" })).toBeVisible();
      await captions.getByRole("radio", { name: "Styles" }).click();
      await captions.getByRole("group", { name: "Caption style preset" }).getByRole("button", { name: "Kinetic focus" }).click();
      await closeSheet(page, viewport);
      expect(await timelineClips(page).count()).toBeGreaterThan(originalCount + 3);
      const renderedCaption = timelineClips(page).filter({ hasText: "newsreels" });
      await expect(renderedCaption).toHaveCount(1);
      await renderedCaption.click();
      await expect(page.getByRole("region", { name: "Preview viewport" }).getByLabel(/^Timeline preview caption /)).toHaveAttribute("data-caption-style", "kineticFocus");

      await page.getByRole("button", { name: "Export", exact: true }).click();
      const dialog = page.getByRole("dialog", { name: "Export" });
      await dialog.getByRole("radio", { name: "MP4", exact: true }).click();
      await dialog.getByRole("radio", { name: "1080p", exact: true }).click();
      await dialog.getByRole("button", { name: "Export video" }).click();
      await expect(page.getByRole("button", { name: "Background tasks" })).toHaveAccessibleDescription("Export complete", { timeout: 15_000 });
      await expect(page.getByRole("region", { name: /Notification/ }).getByText(/Exported /)).toBeVisible();

      expect(await horizontalOverflow(page)).toBeLessThanOrEqual(0);
      expect(errors).toEqual([]);
      await page.screenshot({ path: `output/editor-acceptance/continuous-${viewport.name}.png`, animations: "disabled" });
    });
  });
}
