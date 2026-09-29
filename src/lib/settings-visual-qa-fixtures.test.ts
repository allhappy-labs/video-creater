import { describe, expect, it } from "vitest";

import { createSampleProject, sampleProjectDir } from "./sample-project";
import {
  applySettingsVisualQaProjectFixture,
  resolveSettingsVisualQaFixture,
  settingsVisualQaOperations,
  settingsVisualQaFixtureIds,
  settingsVisualQaFixtures,
} from "./settings-visual-qa-fixtures";
import { FixtureTransport } from "./runtime/adapters/fixture-transport";

const expectedFixtureIds = [
  "settings-general-no-project",
  "settings-projects-no-project",
  "settings-ai-models-no-project",
  "settings-integrations-no-project",
  "settings-storage-no-project",
  "settings-advanced-no-project",
  "project-settings-render-cleanup",
  "system-health-project-failed",
] as const;

describe("Settings visual QA fixtures", () => {
  it("publishes six App Settings categories plus project and health surfaces", () => {
    expect(settingsVisualQaFixtureIds).toEqual(expectedFixtureIds);
    expect(Object.keys(settingsVisualQaFixtures)).toEqual(expectedFixtureIds);
    expect(Object.isFrozen(settingsVisualQaFixtures)).toBe(true);
    expect(Object.fromEntries(
      Object.values(settingsVisualQaFixtures).map((fixture) => [fixture.id, {
        surface: fixture.surface,
        category: fixture.category,
        projectContext: fixture.projectContext,
      }]),
    )).toEqual({
      "settings-general-no-project": { surface: "appSettings", category: "general", projectContext: "none" },
      "settings-projects-no-project": { surface: "appSettings", category: "projects", projectContext: "none" },
      "settings-ai-models-no-project": { surface: "appSettings", category: "aiModels", projectContext: "none" },
      "settings-integrations-no-project": { surface: "appSettings", category: "integrations", projectContext: "none" },
      "settings-storage-no-project": { surface: "appSettings", category: "storage", projectContext: "none" },
      "settings-advanced-no-project": { surface: "appSettings", category: "advanced", projectContext: "none" },
      "project-settings-render-cleanup": { surface: "projectSettings", category: null, projectContext: "active" },
      "system-health-project-failed": { surface: "systemHealth", category: null, projectContext: "active" },
    });
  });

  it("bootstraps Rust-owned preferences and a non-empty generation model selector", () => {
    for (const fixture of Object.values(settingsVisualQaFixtures)) {
      expect(fixture.appPreferences.schemaVersion).toBe(2);
      expect(fixture.updateHealth).toEqual(expect.objectContaining({
        state: "unavailable",
        summary: expect.stringContaining("updater"),
      }));
    }

    const fixture = settingsVisualQaFixtures["settings-ai-models-no-project"];
    expect(fixture.generationModelCatalog.loaded).toBe(true);
    expect(fixture.generationModelCatalog.generationModels.length).toBeGreaterThan(1);
    expect(fixture.appPreferences.enabledGenerationModelIds.length).toBeGreaterThan(0);
    expect(fixture.captureState).toBe("generationModelSelectorOpen");
  });

  it("keeps project-only state out of no-project fixtures", () => {
    for (const fixture of Object.values(settingsVisualQaFixtures)) {
      if (fixture.projectContext === "none") {
        expect(fixture.mcpConfiguration.projectDir).toBeNull();
        expect(fixture.storageHealth.items).not.toContainEqual(
          expect.objectContaining({ provenance: expect.objectContaining({ scope: "projectRenderArtifacts" }) }),
        );
      }
    }

    const projectFixture = settingsVisualQaFixtures["project-settings-render-cleanup"];
    expect(projectFixture.projectRoot).toBe(sampleProjectDir);
    expect(projectFixture.projectRenderArtifactIds).toEqual(["visual-qa-render-1"]);
    expect(projectFixture.storageHealth.items).toContainEqual(
      expect.objectContaining({
        label: "Render artifacts",
        provenance: expect.objectContaining({ scope: "projectRenderArtifacts" }),
      }),
    );
  });

  it("adds a deterministic render report only to the project-settings fixture", () => {
    const sample = createSampleProject();
    const noProject = applySettingsVisualQaProjectFixture(
      sample,
      settingsVisualQaFixtures["settings-general-no-project"],
    );
    const withArtifacts = applySettingsVisualQaProjectFixture(
      sample,
      settingsVisualQaFixtures["project-settings-render-cleanup"],
    );

    expect(noProject).toBe(sample);
    expect(sample.renderReports).toHaveLength(0);
    expect(withArtifacts).not.toBe(sample);
    expect(withArtifacts.renderReports.map(({ id }) => id)).toEqual(["visual-qa-render-1"]);
  });

  it("uses a failed required health state instead of a contradictory all-ready fixture", () => {
    expect(settingsVisualQaFixtureIds).not.toContain("settings-all-ready");
    const fixture = settingsVisualQaFixtures["system-health-project-failed"];
    expect(fixture.systemHealth.overall).toBe("failed");
    expect(fixture.systemHealth.sections.project?.state).toBe("ready");
    expect(fixture.systemHealth.sections.rendering?.state).toBe("failed");
  });

  it("resolves selectors only for an explicit development marker", () => {
    const marker = { enabled: true, fixtureId: "settings-storage-no-project" } as const;
    expect(resolveSettingsVisualQaFixture({ development: false, marker })).toBeNull();
    expect(resolveSettingsVisualQaFixture({ development: true, marker }))
      .toBe(settingsVisualQaFixtures["settings-storage-no-project"]);
    expect(resolveSettingsVisualQaFixture({
      development: true,
      marker: { enabled: false, fixtureId: "settings-storage-no-project" },
    })).toBeNull();
  });

  it("provides deterministic operations without installing desktop globals", async () => {
    const fixture = settingsVisualQaFixtures["project-settings-render-cleanup"];
    const transport = new FixtureTransport(settingsVisualQaOperations(fixture));

    await expect(transport.request("get_app_preferences", { legacy: null }))
      .resolves.toEqual(fixture.appPreferences);
    await expect(transport.request("list_generation_model_catalog"))
      .resolves.toEqual(fixture.generationModelCatalog);
    await expect(transport.request("get_system_health_snapshot", {
      projectRoot: fixture.projectRoot,
    })).resolves.toEqual(fixture.systemHealth);
    await expect(transport.request("preview_storage_cleanup", {
        target: { kind: "projectRenderArtifacts", artifactIds: fixture.projectRenderArtifactIds },
        activeProjectDir: fixture.projectRoot,
    })).resolves.toEqual(fixture.renderArtifactCleanupPreview);
    await expect(transport.request("materialize_sample_project_media", {
      projectDir: fixture.projectRoot,
    })).resolves.toBeUndefined();
    const project = { id: "fixture-project" };
    await expect(transport.request("save_split_project_to_folder", {
      projectDir: fixture.projectRoot,
      project,
    })).resolves.toEqual({
      project,
      report: { manifestPath: "video-creater.project.json", writtenFiles: [], removedFiles: [] },
    });
    expect(window.__TAURI_INTERNALS__).toBeUndefined();
  });
});
