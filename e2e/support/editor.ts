import { expect, type Locator, type Page } from "@playwright/test";
import { openSampleEditor } from "./editor-fixture";

export { collectConsoleErrors } from "./editor-fixture";

/** An acceptance viewport; widths under 1024 px get the mobile layout (bottom tool bar and sheets). */
export interface EditorViewport {
  readonly name: string;
  readonly width: number;
  readonly height: number;
}

export const acceptanceViewports = [
  { name: "desktop", width: 1440, height: 900 },
  { name: "iphone-17-pro", width: 402, height: 874 },
] as const satisfies readonly EditorViewport[];

const mobileLayoutMaxWidth = 1023;

export function isPhone(viewport: EditorViewport): boolean {
  return viewport.width <= mobileLayoutMaxWidth;
}

/**
 * Sizes the page, then opens the sample through the acceptance fixture runtime: every fixture domain
 * over one folder-backed sample that starts untranscribed, with fixture media routed. Call
 * `collectConsoleErrors(page)` first to see errors from before the app loads.
 */
export async function openAcceptanceEditor(page: Page, viewport: EditorViewport, options: { readonly platform?: "macos" | "linux" } = {}): Promise<void> {
  await page.setViewportSize({ width: viewport.width, height: viewport.height });
  await openSampleEditor(page, { acceptanceFixture: true, ...options });
}

/** The clips (including caption cues) on the timeline canvas. */
export function timelineClips(page: Page): Locator {
  return page.getByRole("region", { name: "Timeline canvas" }).getByRole("option");
}

export function clipCount(page: Page): Promise<number> {
  return timelineClips(page).count();
}

/** The primary shortcut modifier for the host platform the fixture reports: Meta on macOS, else Control. */
export async function mod(page: Page): Promise<"Meta" | "Control"> {
  const platform = await page.evaluate(() => window.__EDITOR_FIXTURE_RUNTIME__?.platform ?? "macos");
  return platform === "macos" ? "Meta" : "Control";
}

/**
 * Opens a left tab and returns its panel: the tab on desktop, or the bottom tool bar button on the
 * phone, whose sheet is the panel there.
 */
export async function openTab(page: Page, name: string): Promise<Locator> {
  const viewport = page.viewportSize();
  if (viewport && viewport.width > mobileLayoutMaxWidth) {
    await page.getByRole("tablist", { name: "Editor tools" }).getByRole("tab", { name, exact: true }).click();
    const panel = page.getByRole("tabpanel", { name, exact: true });
    await expect(panel).toBeVisible();
    return panel;
  }
  const sheet = page.getByRole("dialog", { name, exact: true });
  if (await sheet.isVisible()) return sheet;
  // A modal sheet makes the tool bar inert, so another open sheet is closed first.
  if ((await page.getByRole("dialog").count()) > 0) {
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog")).toHaveCount(0);
  }
  await page.getByRole("toolbar", { name: "Editor tools" }).getByRole("button", { name, exact: true }).click();
  await expect(sheet).toBeVisible();
  return sheet;
}

/** The document's horizontal overflow in CSS pixels; 0 when nothing scrolls sideways. */
export function horizontalOverflow(page: Page): Promise<number> {
  return page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
}
