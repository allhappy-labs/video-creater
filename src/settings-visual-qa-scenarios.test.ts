import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import {
  captureSettingsVisualQaScenario,
  scenarioScreenshotName,
  settingsVisualQaScenarios,
// @ts-expect-error The visual harness is an executable ESM script with test-only exports.
} from "../scripts/browser-visual-qa.mjs";
import {
  type SettingsVisualQaFixtureId,
  settingsVisualQaFixtureIds,
  settingsVisualQaFixtures,
} from "./lib/settings-visual-qa-fixtures";

describe("Settings browser visual QA scenario policy", () => {
  it("captures six App Settings categories plus Project Settings and System Health", () => {
    expect(settingsVisualQaScenarios).toHaveLength(16);
    for (const fixtureId of settingsVisualQaFixtureIds) {
      const scenarios = settingsVisualQaScenarios.filter(
        (scenario: { fixtureId: string }) => scenario.fixtureId === fixtureId,
      );
      expect(scenarios.map((scenario: { viewport: string }) => scenario.viewport))
        .toEqual(["desktop", "narrow"]);
      expect(scenarios.map((scenario: { width: number }) => scenario.width))
        .toEqual([1440, 390]);
    }
  });

  it("keeps scenario surface and project context aligned with each fixture", () => {
    const filenames = settingsVisualQaScenarios.map(scenarioScreenshotName);
    expect(new Set(filenames).size).toBe(16);

    for (const scenario of settingsVisualQaScenarios) {
      const fixture = settingsVisualQaFixtures[
        scenario.fixtureId as SettingsVisualQaFixtureId
      ];
      expect(scenario.surface).toBe("settings-state");
      expect(scenario.fixtureSurface).toBe(fixture.surface);
      expect(scenario.category).toBe(fixture.category);
      expect(scenario.projectContext).toBe(fixture.projectContext);
      expect(scenario.expectedMarker).toBe(fixture.expectedMarker);
      expect(scenario.captureState).toBe(fixture.captureState);
      expect(scenario.captureContext).toEqual(fixture.captureContext);
      expect(scenarioScreenshotName(scenario)).toBe(
        `${scenario.fixtureId}-${scenario.viewport}.png`,
      );
    }
  });

  it("bootstraps the fixture runtime before React mounts", () => {
    const source = readFileSync(join(process.cwd(), "src/main.tsx"), "utf8");
    const browserSource = readFileSync(
      join(process.cwd(), "scripts/browser-visual-qa.mjs"),
      "utf8",
    );
    const bootstrap = source.indexOf("bootstrapRuntime");
    const render = source.indexOf("ReactDOM.createRoot");
    expect(bootstrap).toBeGreaterThanOrEqual(0);
    expect(render).toBeGreaterThan(bootstrap);
    expect(source).toContain("await bootstrapRuntime()");
    expect(browserSource).toContain("bootstrapSettingsFixtureId");
    expect(browserSource).toContain('scenario.surface === "settings-state"');
    expect(browserSource).toContain("?? bootstrapFixtureId");
    expect(browserSource).toContain("window.__EDITOR_FIXTURE_RUNTIME__ = {");
    expect(browserSource).toContain("settingsFixtureId: fixtureId");
    expect(browserSource).not.toContain("__TAURI_INTERNALS__");
  });

  it("opens the populated generation selector and project render-cleanup preview", () => {
    const models = captureSettingsVisualQaScenario(
      settingsVisualQaScenarios.find(
        (scenario: { id: string }) => scenario.id === "settings-ai-models-no-project-narrow",
      ),
      "/tmp/settings-ai-models-no-project-narrow.png",
    );
    expect(models).toContain('name: "Enabled generation models"');
    expect(models).toContain('name: "Generation model choices"');
    expect(models).toContain("GPT Image 1");

    const project = captureSettingsVisualQaScenario(
      settingsVisualQaScenarios.find(
        (scenario: { id: string }) => scenario.id === "project-settings-render-cleanup-desktop",
      ),
      "/tmp/project-settings-render-cleanup-desktop.png",
    );
    expect(project).toContain('command: "openProjectSettings"');
    expect(project).toContain('name: "Review cleanup"');
    expect(project).toContain('name: "Confirm render artifact cleanup"');
    expect(project).toContain('name: "Delete reviewed render files"');
  });

  it("opens System Health through the deterministic native-menu event", () => {
    const generated = captureSettingsVisualQaScenario(
      settingsVisualQaScenarios.find(
        (scenario: { id: string }) => scenario.id === "system-health-project-failed-desktop",
      ),
      "/tmp/system-health-project-failed-desktop.png",
    );
    expect(generated).toContain('command: "openSystemHealth"');
    expect(generated).toContain('name: "System Health"');
    expect(generated).toContain("GStreamer composition probe failed locally");
  });

  it("proves the active narrow tab is visible within a scrollable afforded rail", () => {
    const generated = captureSettingsVisualQaScenario(
      settingsVisualQaScenarios.find(
        (scenario: { id: string }) => scenario.id === "settings-advanced-no-project-narrow",
      ),
      "/tmp/settings-advanced-no-project-narrow.png",
    );
    expect(generated).toContain("settingsTabRailBounds");
    expect(generated).toContain("activeSettingsTabBounds");
    expect(generated).toContain("scrollWidth <= settingsTabRailMetrics.clientWidth");
    expect(generated).toContain('affordance !== "horizontal-scroll"');
  });

  it("captures and cleans up in one stable try/finally lifecycle", () => {
    const source = readFileSync(
      join(process.cwd(), "scripts/browser-visual-qa.mjs"),
      "utf8",
    );
    const generated = captureSettingsVisualQaScenario(
      settingsVisualQaScenarios[0],
      "/tmp/settings-general-no-project-desktop.png",
    );
    expect(source).toContain("settingsVisualQaSessionKey");
    expect(source).toContain("page.addInitScript");
    expect(generated).toContain("try {");
    expect(generated).toContain("finally {");
    expect(generated).toContain("window.sessionStorage.removeItem(settingsKey)");
    expect(generated).toContain("await page.evaluate(() => document.fonts.ready)");
    expect(generated).toContain("stablePaintSamples");
    expect(generated).toContain("animations: \"disabled\"");
    expect(source).not.toContain("await page.waitForTimeout(150)");
  });
});
