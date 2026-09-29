import { expect, type Page } from "@playwright/test";
import { join } from "node:path";

const previewFixturePath = join(import.meta.dirname, "..", "fixtures", "preview.webm");
/** A frame of preview.webm, served for captured PNG frames (AI result frames). */
const previewFramePath = join(import.meta.dirname, "..", "fixtures", "preview-frame.png");

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

/** How long the app view and the opened editor may take to appear. */
const appStartTimeoutMs = 30_000;

interface SampleEditorOptions {
  /** Adds the editor panel fixture handlers: an effect catalog and detected silences on the sample. */
  readonly panelFixtures?: boolean;
  /** Seeds background tasks (running, completed, failed) into the sample and answers task details commands. */
  readonly tasksFixture?: boolean;
  /**
   * Answers AI conversation turns by keyword ("tighten" a safe cut, "generate" a review bundle,
   * "fail" a validation failure, anything else a safe caption fix) and applies, undoes and captures
   * result frames against the folder-backed sample.
   */
  readonly conversationFixture?: boolean;
  /**
   * Answers the export commands: MP4 and WebM available (ProRes unavailable with a reason), a render
   * that completes over three folder reloads, and XML and package exports. Seeds no tasks unless
   * `tasksFixture` is set too. Accepted reveals land on `window.__EDITOR_FIXTURE_REVEALS__`.
   */
  readonly exportFixture?: boolean;
  /**
   * Every handler the acceptance flows reach (implies the panel, conversation and export fixtures)
   * over a sample that opens untranscribed: Media → Import adds preview.webm, Captions transcribes
   * over two folder reloads, and generations complete on reloads too.
   */
  readonly acceptanceFixture?: boolean;
  /** Host platform the fixture backend reports; macOS (Meta shortcuts) by default. */
  readonly platform?: "macos" | "linux";
}

/** Collects page errors and `console.error` messages from now on; the array fills as they happen. */
export function collectConsoleErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(`pageerror: ${error.message}`));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(`console.error: ${message.text()}`);
  });
  return errors;
}

/**
 * Opens the bundled sample project through the browser fixture runtime (which reports macOS unless
 * `platform` says otherwise, so the primary shortcut modifier is Meta) and returns the page errors
 * collected from then on.
 */
export async function openSampleEditor(page: Page, options: SampleEditorOptions = {}): Promise<string[]> {
  const errors = collectConsoleErrors(page);
  await page.route(
    /\/__editor-fixture-media\/.*/,
    (route) => route.fulfill({ path: previewFixturePath, contentType: "video/webm" }),
  );
  // Registered last, so it wins for PNG requests.
  await page.route(
    /\/__editor-fixture-media\/.*\.png$/,
    (route) => route.fulfill({ path: previewFramePath, contentType: "image/png" }),
  );
  await page.addInitScript(({ fixturePreferences, panelFixtures, tasksFixture, conversationFixture, exportFixture, acceptanceFixture, platform }) => {
    window.__EDITOR_FIXTURE_RUNTIME__ = {
      enabled: true,
      settingsFixtureId: "settings-ai-models-no-project",
      preferences: fixturePreferences,
      exportCapabilities: [],
      panelFixtures,
      tasksFixture,
      conversationFixture,
      exportFixture,
      acceptanceFixture,
      platform,
    };
  }, {
    fixturePreferences: preferences,
    panelFixtures: options.panelFixtures === true,
    tasksFixture: options.tasksFixture === true,
    conversationFixture: options.conversationFixture === true,
    exportFixture: options.exportFixture === true,
    acceptanceFixture: options.acceptanceFixture === true,
    platform: options.platform ?? "macos",
  });
  await page.goto("/");
  // Right after a fresh dev server start, Vite's first module transform can hold "Loading app view"
  // well past the default 5 s.
  await expect(page.getByRole("main", { name: "Project home" })).toBeVisible({ timeout: appStartTimeoutMs });
  await page.getByRole("button", { name: "Open sample project" }).click();
  await expect(page.getByRole("main", { name: "Video editor workspace" })).toBeVisible({ timeout: appStartTimeoutMs });
  return errors;
}
