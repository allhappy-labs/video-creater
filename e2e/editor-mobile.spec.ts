import { expect, test, type CDPSession, type Locator, type Page } from "@playwright/test";
import { openSampleEditor } from "./support/editor-fixture";

const screenshotDir = "output/editor-mobile";

test.use({ hasTouch: true });

async function horizontalOverflow(page: Page): Promise<number> {
  return page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
}

async function center(locator: Locator): Promise<{ x: number; y: number }> {
  const box = await locator.boundingBox();
  if (!box) throw new Error("element has no box");
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

/** Real touch input through the DevTools protocol, so the page sees `pointerType: "touch"`. */
async function touch(session: CDPSession, type: "touchStart" | "touchMove" | "touchEnd", point: { x: number; y: number }) {
  await session.send("Input.dispatchTouchEvent", { type, touchPoints: type === "touchEnd" ? [] : [{ x: point.x, y: point.y }] });
}

async function expectTopBarFits(page: Page, width: number) {
  const header = page.locator("header");
  const controls = ["Home", "Undo", "Redo", width < 380 ? "More" : "Editor menu", "Export"];
  for (const name of controls) {
    const box = await header.getByRole("button", { name, exact: true }).boundingBox();
    expect(box, name).not.toBeNull();
    expect(box?.x ?? -1, name).toBeGreaterThanOrEqual(0);
    expect((box?.x ?? 0) + (box?.width ?? 0), name).toBeLessThanOrEqual(width);
  }
  await expect(header.getByRole("button", { name: "Background tasks" })).toBeVisible();
  // The project name is the part that gives way: it truncates rather than pushing controls out.
  const name = header.getByTestId("project-name");
  expect(await name.evaluate((element) => element.scrollWidth > element.clientWidth)).toBe(true);
  expect(await header.evaluate((element) => element.scrollWidth - element.clientWidth)).toBeLessThanOrEqual(0);
}

test("iPhone 17 Pro top bar fits at 402 px with the tasks pill, and a sheet swipes closed", async ({ page }) => {
  await page.setViewportSize({ width: 402, height: 874 });
  const errors = await openSampleEditor(page, { tasksFixture: true });

  await expectTopBarFits(page, 402);
  await expect(page.getByRole("navigation", { name: "Bottom tool bar" })).toBeVisible();
  expect(await horizontalOverflow(page)).toBeLessThanOrEqual(0);
  await page.screenshot({ path: `${screenshotDir}/phone-402.png` });

  await page.getByRole("toolbar", { name: "Editor tools" }).getByRole("button", { name: "Media" }).click();
  const sheet = page.getByRole("dialog", { name: "Media" });
  await expect(sheet).toBeVisible();
  await page.screenshot({ path: `${screenshotDir}/phone-402-media-sheet.png` });

  const session = await page.context().newCDPSession(page);
  const handle = sheet.getByRole("button", { name: "Dismiss Media sheet" });
  // A short drag springs back.
  let start = await center(handle);
  await touch(session, "touchStart", start);
  for (const dy of [15, 30, 45]) await touch(session, "touchMove", { x: start.x, y: start.y + dy });
  await touch(session, "touchEnd", start);
  await expect(sheet).toBeVisible();
  await expect.poll(() => sheet.evaluate((element) => getComputedStyle(element).transform)).toBe("none");

  // Dragging past 80 px closes it.
  start = await center(handle);
  await touch(session, "touchStart", start);
  for (const dy of [30, 60, 90, 120]) await touch(session, "touchMove", { x: start.x, y: start.y + dy });
  await touch(session, "touchEnd", start);
  await expect(page.getByRole("dialog", { name: "Media" })).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("a touch long press on a clip opens its context menu", async ({ page }) => {
  await page.setViewportSize({ width: 402, height: 874 });
  const errors = await openSampleEditor(page);
  const clip = page.getByRole("option", { name: /^Opening clip/ });
  await expect(clip).toBeVisible();
  const box = await clip.boundingBox();
  if (!box) throw new Error("clip has no box");
  const point = { x: box.x + Math.min(40, box.width / 2), y: box.y + box.height / 2 };

  const session = await page.context().newCDPSession(page);
  // A press that slides away is a scroll, not a long press.
  await touch(session, "touchStart", point);
  await touch(session, "touchMove", { x: point.x - 30, y: point.y });
  await page.waitForTimeout(700);
  await touch(session, "touchEnd", { x: point.x - 30, y: point.y });
  await expect(page.getByRole("menu")).toHaveCount(0);

  await touch(session, "touchStart", point);
  await expect(page.getByRole("menu", { name: "Clip actions" })).toBeVisible();
  await touch(session, "touchEnd", point);
  await expect(page.getByRole("menuitem", { name: /Split at playhead/ })).toBeVisible();
  await page.screenshot({ path: `${screenshotDir}/phone-402-clip-menu.png` });
  expect(await horizontalOverflow(page)).toBeLessThanOrEqual(0);
  expect(errors).toEqual([]);
});

test("375 px folds the gear menu into More without overflow", async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 });
  const errors = await openSampleEditor(page, { tasksFixture: true });

  await expectTopBarFits(page, 375);
  await expect(page.getByRole("button", { name: "Editor menu" })).toHaveCount(0);
  expect(await horizontalOverflow(page)).toBeLessThanOrEqual(0);
  await page.getByRole("button", { name: "More", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: "App settings" })).toBeVisible();
  await page.screenshot({ path: `${screenshotDir}/phone-375-more.png` });
  await page.keyboard.press("Escape");
  expect(errors).toEqual([]);
});

test("iPad 820×1180 keeps the mobile layout with usable preview and timeline heights and 55% sheets", async ({ page }) => {
  await page.setViewportSize({ width: 820, height: 1180 });
  const errors = await openSampleEditor(page);

  await expect(page.getByRole("toolbar", { name: "Editor tools" })).toBeVisible();
  const preview = await page.getByRole("region", { name: "Preview viewport" }).boundingBox();
  const timeline = await page.getByRole("region", { name: "Timeline", exact: true }).boundingBox();
  expect(preview?.height ?? 0).toBeGreaterThanOrEqual(360);
  expect(timeline?.height ?? 0).toBeGreaterThanOrEqual(360);
  expect(await horizontalOverflow(page)).toBeLessThanOrEqual(0);
  await page.screenshot({ path: `${screenshotDir}/ipad-820.png` });

  await page.getByRole("toolbar", { name: "Editor tools" }).getByRole("button", { name: "AI" }).click();
  const sheet = page.getByRole("dialog", { name: "AI" });
  await expect(sheet).toBeVisible();
  const sheetBox = await sheet.boundingBox();
  expect(Math.round(sheetBox?.height ?? 0)).toBe(Math.round(1180 * 0.55));
  await page.screenshot({ path: `${screenshotDir}/ipad-820-ai-sheet.png` });
  expect(errors).toEqual([]);
});
