import { expect, test, type Locator } from "@playwright/test";
import { openSampleEditor } from "./support/editor-fixture";

// VC-019: the Export popover saves into a chosen folder, at a chosen frame rate and Master quality,
// and never overwrites an earlier export. The export fixture's folder chooser picks
// /tmp/video-creater-exports and completes a render on the editor's third folder reload.

const screenshotDir = "output/playwright/editor-export-destination";
const projectDir = "/tmp/video-creater-editor-project";
const chosenFolder = "/tmp/video-creater-exports";

const viewports = [
  { label: "desktop", width: 1440, height: 900 },
  { label: "phone", width: 402, height: 874 },
] as const;

async function openExport(page: import("@playwright/test").Page): Promise<Locator> {
  await page.getByRole("button", { name: "Export", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Export" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("radio", { name: "MP4" })).toHaveAttribute("aria-checked", "true");
  return dialog;
}

for (const viewport of viewports) {
  test(`${viewport.label} exports Master at 25 fps to a chosen folder without overwriting`, async ({ page }) => {
    // Two renders of about three seconds each, plus their toasts.
    test.slow();
    await page.setViewportSize({ width: viewport.width, height: viewport.height });
    const errors = await openSampleEditor(page, { exportFixture: true });
    const notifications = page.getByRole("region", { name: /Notification/ });

    // 1. Save to starts at the project's exports folder; the chooser switches it.
    let dialog = await openExport(page);
    await expect(dialog.getByText(`${projectDir}/exports`)).toBeVisible();
    await expect(dialog.getByText("Saves as Edison Restoration Demo.mp4")).toBeVisible();
    await dialog.getByRole("button", { name: "Choose export folder" }).click();
    await expect(dialog.getByText(chosenFolder)).toBeVisible();

    // 2. Advanced → Frame rate 25 fps, then Master.
    await dialog.getByRole("button", { name: "Advanced" }).click();
    await dialog.getByRole("combobox", { name: "Frame rate" }).click();
    await page.getByRole("option", { name: "25 fps" }).click();
    await expect(dialog.getByText(/ · 25 fps · ≈ /)).toBeVisible();
    const master = dialog.getByRole("radio", { name: "Master" });
    await expect(master).not.toHaveAttribute("aria-disabled", "true");
    await master.click();
    await expect(master).toHaveAttribute("aria-checked", "true");
    await page.screenshot({ path: `${screenshotDir}/${viewport.label}-export-popover.png`, animations: "disabled" });

    // 3. Export video; the toast names the saved file and Show in folder reveals its absolute path.
    await dialog.getByRole("button", { name: "Export video" }).click();
    await expect(dialog).toHaveCount(0);
    const first = notifications.getByRole("listitem").filter({ hasText: "Exported Edison Restoration Demo.mp4" });
    await expect(first).toBeVisible({ timeout: 15_000 });
    await first.getByRole("button", { name: "Show in folder" }).click();
    await expect
      .poll(() => page.evaluate(() => window.__EDITOR_FIXTURE_REVEALS__ ?? []))
      .toEqual([{ projectDir, artifactPath: `${chosenFolder}/Edison Restoration Demo.mp4` }]);

    // 4. The same export again takes the next free name.
    dialog = await openExport(page);
    await dialog.getByRole("button", { name: "Choose export folder" }).click();
    await expect(dialog.getByText(chosenFolder)).toBeVisible();
    await dialog.getByRole("button", { name: "Export video" }).click();
    await expect(dialog).toHaveCount(0);
    await expect(notifications.getByRole("listitem").filter({ hasText: "Exported Edison Restoration Demo (2).mp4" })).toBeVisible({ timeout: 15_000 });
    expect(errors).toEqual([]);
  });
}
