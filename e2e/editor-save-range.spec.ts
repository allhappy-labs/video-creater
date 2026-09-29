import { expect, test } from "@playwright/test";
import { collectConsoleErrors, openAcceptanceEditor, openTab, timelineClips } from "./support/editor";

// VC-020: "Save range as media" names the new media after the project and the saved range. The
// sample's first clip spans 00:00–00:04; the fixture render completes on the third folder reload.

test("desktop saves a clip's range as media named after the project and range", async ({ page }) => {
  test.slow();
  const errors = collectConsoleErrors(page);
  await openAcceptanceEditor(page, { name: "desktop", width: 1440, height: 900 });
  const name = "Edison Restoration Demo 00:00–00:04";

  const clip = timelineClips(page).first();
  await clip.click();
  await clip.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Save range as media" }).click();

  await expect(page.getByRole("region", { name: /Notification/ }).getByRole("listitem").filter({ hasText: `Saved ${name} to Media` })).toBeVisible({ timeout: 15_000 });
  const media = await openTab(page, "Media");
  await expect(media.getByText(name, { exact: true })).toBeVisible();
  expect(errors).toEqual([]);
});
