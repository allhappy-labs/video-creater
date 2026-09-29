import { expect, test, type Page } from "@playwright/test";
import { join } from "node:path";

const previewFixturePath = join(import.meta.dirname, "fixtures", "preview.webm");

const preferences = {
  schemaVersion: 2,
  projectLocation: { mode: "ask" },
  requireProviderUploadConfirmation: true,
  renderCompletionNotifications: false,
  newProjectDefaults: {
    width: 1920,
    height: 1080,
    fps: 30,
    loudnessLufs: -14,
    captions: "burn_in",
  },
  enabledGenerationModelIds: [],
  generationExecutionBackend: "inProcess",
  agentBackend: "automatic",
  claudeModel: "sonnet",
  claudeExecutablePath: "",
};

async function installFixture(page: Page) {
  await page.route(
    /\/__editor-fixture-media\/.*/,
    (route) => route.fulfill({ path: previewFixturePath, contentType: "video/webm" }),
  );
  await page.addInitScript((fixturePreferences) => {
    window.__EDITOR_FIXTURE_RUNTIME__ = {
      enabled: true,
      settingsFixtureId: "settings-ai-models-no-project",
      preferences: fixturePreferences,
      exportCapabilities: [],
    };
  }, preferences);
}

function collectUnexpectedErrors(page: Page) {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(`pageerror: ${error.message}`));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(`console.error: ${message.text()}`);
  });
  return errors;
}

async function exerciseCoreSurfaces(page: Page, viewport: "desktop" | "narrow", errors: string[], outputPath: (name: string) => string) {
  await page.goto("/");
  await expect(page.getByRole("main", { name: "Project home" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Open sample project" })).toBeVisible();

  await page.getByRole("button", { name: "Model settings", exact: true }).click();
  const settingsTabs = page.getByRole("tablist", { name: "Settings categories" });
  await expect(settingsTabs).toBeVisible();
  const selectedSettingsTab = settingsTabs.getByRole("tab", { selected: true });
  await selectedSettingsTab.focus();
  await selectedSettingsTab.press("End");
  await expect(settingsTabs.getByRole("tab").last()).toBeFocused();
  await settingsTabs.getByRole("tab").last().press("Home");
  await expect(settingsTabs.getByRole("tab").first()).toBeFocused();
  await page.screenshot({ path: outputPath(`settings-${viewport}.png`), fullPage: true });

  await page.getByRole("button", { name: "Back", exact: true }).click();
  await expect(page.getByRole("main", { name: "Project home" })).toBeVisible();
  await page.getByRole("button", { name: "Open sample project" }).click();
  await expect(page.getByRole("main", { name: "Video editor workspace" })).toBeVisible();
  expect(errors).toEqual([]);
}

test.beforeEach(async ({ page }) => {
  await installFixture(page);
});

test("desktop fixture opens Home, Settings and editor shell without browser errors", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 960 });
  const errors = collectUnexpectedErrors(page);
  await exerciseCoreSurfaces(page, "desktop", errors, testInfo.outputPath.bind(testInfo));
});

test("narrow fixture keeps Home, Settings and editor shell usable without browser errors", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const errors = collectUnexpectedErrors(page);
  await exerciseCoreSurfaces(page, "narrow", errors, testInfo.outputPath.bind(testInfo));
});
