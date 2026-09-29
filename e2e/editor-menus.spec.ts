import { expect, test, type Page } from "@playwright/test";
import { openSampleEditor } from "./support/editor-fixture";

// The clipboard Copy flow (which needs a granted permission) and the ⌘/ / Ctrl+/ shortcut are in
// editor-export-tasks.spec.ts.

const screenshotDir = "output/playwright/editor-menus";

async function expectInsideViewport(page: Page, name: string) {
  const box = await page.getByRole("dialog", { name }).boundingBox();
  const viewport = page.viewportSize();
  if (!box || !viewport) throw new Error(`${name} has no layout box`);
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.y).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(viewport.width);
  expect(box.y + box.height).toBeLessThanOrEqual(viewport.height);
}

async function openGearMenu(page: Page) {
  await page.getByRole("button", { name: "Editor menu" }).click();
  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  return menu;
}

for (const viewport of [
  { label: "desktop", width: 1440, height: 900 },
  { label: "phone", width: 402, height: 874 },
] as const) {
  test(`${viewport.label} gear menu opens the shortcuts sheet and agent dialogs`, async ({ page }) => {
    await page.setViewportSize({ width: viewport.width, height: viewport.height });
    const errors = await openSampleEditor(page);

    const menu = await openGearMenu(page);
    await expect(menu.getByRole("menuitem")).toHaveText([
      "Project settings",
      "App settings",
      "Keyboard shortcuts⌘/",
      "Connect external agents…",
      "Project skills…",
    ]);
    await page.screenshot({ path: `${screenshotDir}/${viewport.label}-gear-menu.png`, animations: "disabled" });

    await menu.getByRole("menuitem", { name: "Keyboard shortcuts" }).click();
    const sheet = page.getByRole("dialog", { name: "Keyboard shortcuts" });
    await expect(sheet).toBeVisible();
    await expect(sheet.getByRole("listitem").filter({ hasText: "Undo" }).first()).toContainText("⌘Z");
    await expectInsideViewport(page, "Keyboard shortcuts");
    await page.screenshot({ path: `${screenshotDir}/${viewport.label}-shortcuts-sheet.png`, animations: "disabled" });

    await sheet.getByRole("searchbox", { name: "Search shortcuts" }).fill("ripple");
    await expect(sheet.getByRole("listitem")).toHaveCount(1);
    await page.keyboard.press("Escape");
    await expect(sheet).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Editor menu" })).toBeFocused();

    await (await openGearMenu(page)).getByRole("menuitem", { name: "Connect external agents…" }).click();
    const agents = page.getByRole("dialog", { name: "Connect external agents" });
    await expect(agents).toBeVisible();
    await agents.getByRole("tab", { name: "Claude Code" }).click();
    await expect(agents.getByRole("region", { name: "Claude Code setup" })).toContainText("Claude Code: open this project folder");
    await expectInsideViewport(page, "Connect external agents");
    await page.screenshot({ path: `${screenshotDir}/${viewport.label}-connect-agents.png`, animations: "disabled" });
    await agents.getByRole("button", { name: "Close Connect external agents" }).click();
    await expect(agents).toHaveCount(0);

    await (await openGearMenu(page)).getByRole("menuitem", { name: "Project skills…" }).click();
    const skills = page.getByRole("dialog", { name: "Project skills" });
    await expect(skills.getByRole("heading", { level: 3 })).toHaveText(["Editing pipeline", "Video graphics", "Editor interface"]);
    await expectInsideViewport(page, "Project skills");
    await page.screenshot({ path: `${screenshotDir}/${viewport.label}-project-skills.png`, animations: "disabled" });
    await page.keyboard.press("Escape");
    await expect(skills).toHaveCount(0);

    // The page stays interactive after the menu and dialogs close.
    await expect(page.getByRole("button", { name: "Home" })).toBeEnabled();
    await page.getByRole("button", { name: "Export" }).click();
    await expect(page.getByRole("dialog", { name: "Export" })).toBeVisible();
    expect(errors).toEqual([]);
  });
}
