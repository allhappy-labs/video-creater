import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const repoRoot = process.cwd();
const binaryPath = join(
  repoRoot,
  "src-tauri/src/bin/video-creater-settings-e2e.rs",
);

describe("native Settings readiness evidence", () => {
  it("exposes the exact local-only evidence command", () => {
    const packageJson = JSON.parse(
      readFileSync(join(repoRoot, "package.json"), "utf8"),
    ) as { scripts?: Record<string, string> };

    expect(packageJson.scripts?.["e2e:settings"]).toBe(
      "cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-settings-e2e --",
    );
  });

  it("keeps all required check and report identifiers in the native binary", () => {
    expect(existsSync(binaryPath)).toBe(true);
    const source = readFileSync(binaryPath, "utf8");

    expect(source).toContain(
      "output/settings-readiness/integration/report.json",
    );
    for (const checkId of [
      "settings-health-snapshot",
      "operation-journal-recovery",
      "transcription-model-lifecycle",
      "speech-readiness-separation",
      "render-system-rollup",
      "agent-component-independence",
      "skill-scoped-repair",
      "storage-cleanup-safety",
      "provider-secret-redaction",
    ]) {
      expect(source).toContain(checkId);
    }
  });

  it("seeds the packaged acceptance project with one exact render cleanup target", () => {
    const source = readFileSync(
      join(repoRoot, "scripts/settings-packaged-acceptance.mjs"),
      "utf8",
    );
    expect(source).toContain(
      'const SETTINGS_RENDER_ARTIFACT_ID = "settings-acceptance-render-artifact"',
    );
    expect(source).toContain("layout.project,");
    expect(source).toContain("SETTINGS_RENDER_ARTIFACT_ID,");
    expect(source).toMatch(
      /writeFileSync\(\s*join\(renderArtifact, "report\.json"\)/,
    );
    expect(source).toContain("JSON.stringify(SETTINGS_RENDER_REPORT");
    expect(source).not.toContain('writeFileSync(join(renderArtifact, "pipeline-report.json")');
  });

  it("registers the packaged project through native save and acceptance-gated reload commands before cleanup preview", () => {
    const source = readFileSync(
      join(repoRoot, "src/lib/settings-acceptance-runner.ts"),
      "utf8",
    );
    expect(source).toContain('bridge.invoke("save_split_project_to_folder"');
    expect(source).toContain('bridge.invoke("load_settings_acceptance_project"');
    expect(source.indexOf('bridge.invoke("load_settings_acceptance_project"')).toBeLessThan(
      source.indexOf('bridge.invoke("preview_storage_cleanup"'),
    );
  });

  it("records the final product contract in the packaged-app checkpoint", () => {
    const source = readFileSync(
      join(repoRoot, "src/lib/settings-acceptance-runner.ts"),
      "utf8",
    );

    expect(source).toContain("createFinalSettingsAcceptanceReport");
    expect(source).toContain("productContract");
    expect(source).toContain('selectionControl: "multiselect-combobox"');
    expect(source).toContain("optionalProviderCountedAsRequired");
  });

  it("uses explicit production DOM markers and an acceptance-only fail-fast exit", () => {
    const runner = readFileSync(
      join(repoRoot, "src/lib/settings-acceptance-runner.ts"),
      "utf8",
    );
    const shell = readFileSync(
      join(repoRoot, "src/components/settings/settings-shell.tsx"),
      "utf8",
    );
    const providers = readFileSync(
      join(repoRoot, "src/components/settings/providers-settings.tsx"),
      "utf8",
    );
    const models = readFileSync(
      join(repoRoot, "src/components/settings/generation-model-multiselect.tsx"),
      "utf8",
    );
    const main = readFileSync(join(repoRoot, "src-tauri/src/main.rs"), "utf8");

    expect(shell).toContain("data-settings-acceptance-category-rail");
    expect(shell).toContain("data-settings-category-id={category.id}");
    expect(providers).toContain('data-settings-credential-boundary="keychain-only"');
    expect(models).toContain("data-settings-blocked-action");
    expect(runner).toContain('bridge.invoke("abort_settings_acceptance_run"');
    expect(main).toContain("fn abort_settings_acceptance_run(");
    expect(main).toContain("std::process::exit(86)");
  });
});
