import { expect, test, type Locator, type Page } from "@playwright/test";
import { openSampleEditor } from "./support/editor-fixture";

// Spec acceptance flow 8. The export fixture reports MP4 (H.264) and WebM available and ProRes
// unavailable, and completes an in-process render on the editor's third folder reload (about three
// seconds), recording a render report. While the render waits it reports progress (reloads seen over
// reloads needed). Accepted reveals land on window.__EDITOR_FIXTURE_REVEALS__.

const screenshotDir = "output/playwright/editor-export-tasks";
const projectDir = "/tmp/video-creater-editor-project";
const proResReason = "ProRes needs the ProRes encoder, which this build doesn't include.";

const viewports = [
  { label: "desktop", width: 1440, height: 900, compact: false },
  { label: "phone", width: 402, height: 874, compact: true },
] as const;

async function expectInsideViewport(page: Page, locator: Locator) {
  const box = await locator.boundingBox();
  const viewport = page.viewportSize();
  if (!box || !viewport) throw new Error("The overlay has no layout box");
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.y).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(viewport.width);
  expect(box.y + box.height).toBeLessThanOrEqual(viewport.height);
}

async function choose(dialog: Locator, name: string) {
  const option = dialog.getByRole("radio", { name, exact: true });
  await option.click();
  await expect(option).toHaveAttribute("aria-checked", "true");
}

for (const viewport of viewports) {
  test(`${viewport.label} exports MP4 1080p High and follows it through Background tasks`, async ({ page }) => {
    // The popover, toast and task list are all waited on in turn; the render alone takes about 3 s.
    test.slow();
    await page.setViewportSize({ width: viewport.width, height: viewport.height });
    const errors = await openSampleEditor(page, { exportFixture: true });
    const indicator = page.getByRole("button", { name: "Background tasks" });
    await expect(indicator).toHaveCount(0);

    // 1. Export → MP4 · 1080p · High; the summary names H.264.
    await page.getByRole("button", { name: "Export", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Export" });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole("radio", { name: "MP4" })).not.toHaveAttribute("aria-disabled", "true");
    await choose(dialog, "WebM");
    await expect(dialog.getByText(/^VP8 · \d+ fps · ≈ /)).toBeVisible();
    await choose(dialog, "MP4");
    await choose(dialog, "720p");
    await choose(dialog, "1080p");
    await choose(dialog, "Draft");
    await choose(dialog, "High");
    await expect(dialog.getByText(/^H\.264 · \d+ fps · ≈ \d/)).toBeVisible();

    // 5. ProRes stays listed, disabled, with its reason in a tooltip.
    const proRes = dialog.getByRole("radio", { name: "ProRes" });
    await expect(proRes).toHaveAttribute("aria-disabled", "true");
    await expect(proRes).toHaveAccessibleDescription(proResReason);
    await proRes.hover();
    await expect(page.getByRole("tooltip")).toHaveText(proResReason);
    await proRes.click({ force: true });
    await expect(dialog.getByRole("radio", { name: "MP4" })).toHaveAttribute("aria-checked", "true");
    await expectInsideViewport(page, dialog);
    await page.mouse.move(0, 0);
    await expect(page.getByRole("tooltip")).toHaveCount(0);
    await page.screenshot({ path: `${screenshotDir}/${viewport.label}-export-popover.png`, animations: "disabled" });

    // 2. Export video closes the popover; the indicator shows the running export's progress.
    await dialog.getByRole("button", { name: "Export video" }).click();
    await expect(dialog).toHaveCount(0);
    await expect(indicator).toHaveAccessibleDescription(/^Exporting · [1-9]\d*%$/);
    if (!viewport.compact) {
      await expect(indicator).toContainText("Exporting");
      await indicator.click();
      const running = page.getByRole("dialog", { name: "Background tasks" });
      const progress = running.getByRole("listitem", { name: "H.264 Final export" }).getByRole("progressbar", { name: "H.264 Final export progress" });
      await expect(progress).toBeVisible();
      await expect.poll(async () => Number(await progress.getAttribute("aria-valuenow"))).toBeGreaterThan(0);
      await page.keyboard.press("Escape");
      await expect(running).toHaveCount(0);
    }

    // 3. Completion toasts the saved file, and Show in folder reveals it in the project's exports folder.
    const toast = page.getByRole("region", { name: /Notification/ }).getByRole("listitem").filter({ hasText: "Exported Edison Restoration Demo.mp4" });
    await expect(toast).toBeVisible({ timeout: 15_000 });
    await expect(indicator).toHaveAccessibleDescription("Export complete");
    await toast.getByRole("button", { name: "Show in folder" }).click();
    await expect
      .poll(() => page.evaluate(() => window.__EDITOR_FIXTURE_REVEALS__ ?? []))
      .toEqual([{ projectDir, artifactPath: "exports/Edison Restoration Demo.mp4" }]);

    // 4. Background tasks lists the completed export; Details shows its render report.
    await indicator.click();
    const tasks = page.getByRole("dialog", { name: "Background tasks" });
    await expect(tasks).toBeVisible();
    const row = tasks.getByRole("listitem", { name: "H.264 Final export" });
    await expect(tasks.getByRole("listitem")).toHaveCount(2);
    await expect(row).toContainText("Completed");
    await expect(row.getByRole("button", { name: "Cancel" })).toHaveCount(0);
    await expect(row.getByRole("button", { name: "Show" })).toBeVisible();
    await expectInsideViewport(page, tasks);
    await page.screenshot({ path: `${screenshotDir}/${viewport.label}-tasks-popover.png`, animations: "disabled" });

    await row.getByRole("button", { name: "Details" }).click();
    const details = page.getByRole("dialog", { name: "Task details" });
    await expect(details).toBeVisible();
    await expect(tasks).toHaveCount(0);
    await expect(details.getByText("Editor process")).toBeVisible();
    const report = details.getByRole("region", { name: "Render report" });
    await expect(report).toBeVisible();
    await expect(report).toContainText(/renders\/export-mp4H264-[a-z0-9]+\/output\.mp4/);
    await expectInsideViewport(page, details);
    await page.screenshot({ path: `${screenshotDir}/${viewport.label}-task-details.png`, animations: "disabled" });

    await page.keyboard.press("Escape");
    await expect(details).toHaveCount(0);
    expect(errors).toEqual([]);
  });

  test(`${viewport.label} Connect external agents copies the setup to the clipboard`, async ({ page, context, baseURL }) => {
    await context.grantPermissions(["clipboard-read", "clipboard-write"], baseURL ? { origin: baseURL } : {});
    await page.setViewportSize({ width: viewport.width, height: viewport.height });
    const errors = await openSampleEditor(page);

    await page.getByRole("button", { name: "Editor menu" }).click();
    await page.getByRole("menu").getByRole("menuitem", { name: "Connect external agents…" }).click();
    const agents = page.getByRole("dialog", { name: "Connect external agents" });
    await expect(agents).toBeVisible();
    await agents.getByRole("tab", { name: "Claude Code" }).click();
    await agents.getByRole("button", { name: "Copy" }).click();
    await expect(agents.getByRole("button", { name: "Copied" })).toBeVisible();
    await expect(agents.getByRole("status")).toHaveText("Copied Claude Code setup");

    const clipboard = await page.evaluate(() => navigator.clipboard.readText());
    expect(clipboard).toContain("Video Creater agent setup");
    expect(clipboard).toContain("Claude Code: open this project folder");
    await expect(agents.getByRole("region", { name: "Claude Code setup" })).toHaveText(clipboard);
    expect(errors).toEqual([]);
  });
}

for (const host of [
  { platform: "macos", shortcut: "Meta+/" },
  { platform: "linux", shortcut: "Control+/" },
] as const) {
  test(`desktop ${host.shortcut} opens the shortcuts sheet on ${host.platform} and Escape closes it`, async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    const errors = await openSampleEditor(page, { platform: host.platform });

    await page.keyboard.press(host.shortcut);
    const sheet = page.getByRole("dialog", { name: "Keyboard shortcuts" });
    await expect(sheet).toBeVisible();
    await expect(sheet.getByRole("listitem").filter({ hasText: "Undo" }).first()).toContainText(host.platform === "macos" ? "⌘Z" : "Ctrl+Z");
    await page.keyboard.press("Escape");
    await expect(sheet).toHaveCount(0);
    expect(errors).toEqual([]);
  });
}
