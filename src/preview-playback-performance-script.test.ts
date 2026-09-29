import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const repoRoot = process.cwd();

describe("prepared preview playback performance gate", () => {
  it("uses the real preview preparation path and fails below sustained 1080p60 thresholds", () => {
    const packageJson = JSON.parse(
      readFileSync(join(repoRoot, "package.json"), "utf8"),
    ) as { scripts?: Record<string, string> };
    const scriptSource = readFileSync(
      join(repoRoot, "scripts/preview-playback-performance.mjs"),
      "utf8",
    );

    expect(packageJson.scripts?.["visual:qa:preview-performance"]).toBe(
      "node scripts/preview-playback-performance.mjs",
    );
    expect(scriptSource).toContain("testsrc2=size=1920x1080");
    expect(scriptSource).toContain("fixtureFps = 60");
    expect(scriptSource).toContain('command === "prepare_project_preview"');
    expect(scriptSource).toContain("canonical-prepared-preview-frame");
    expect(scriptSource).toContain('name: "Play preview"');
    expect(scriptSource).toContain('name: "Pause preview"');
    expect(scriptSource).toContain("Preview scrubber");
    expect(scriptSource).not.toContain("Preview transport play");
    // The Tauri stub answers the app bootstrap, and Vite serves the frames from the project root.
    expect(scriptSource).toContain('command === "get_app_preferences"');
    expect(scriptSource).toContain("relative(repoRoot, frameDir)");
    expect(scriptSource).not.toContain("symlinkSync");
    expect(scriptSource).toContain("uniquePreparedFrames");
    expect(scriptSource).toContain("loadedPreparedFrames");
    expect(scriptSource).toContain("frameLoadErrors");
    expect(scriptSource).toContain("minimumEffectivePreparedFps: 45");
    expect(scriptSource).toContain("minimumEffectiveLoadedPreparedFps: 45");
    expect(scriptSource).toContain("maximumP95RafIntervalMs: 25");
    expect(scriptSource).toContain("failures.length === 0 ? \"passed\" : \"failed\"");
    expect(scriptSource).toContain("process.exitCode = 1");
  });
});
