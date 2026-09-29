import { createHash } from "node:crypto";
import {
  copyFileSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { describe, expect, it } from "vitest";
import { requiredValue } from "@/test-utils/required";
import {
  scenarioScreenshotName,
  visualQaScenarios,
// @ts-expect-error The visual harness is an executable ESM script with test-only exports.
} from "../scripts/browser-visual-qa.mjs";

const repoRoot = process.cwd();
const defaultBaselineDir = join(
  repoRoot,
  "docs/visual-qa/browser-visual-baseline",
);
const expectedScreenshots: string[] = (
  visualQaScenarios as Array<Record<string, unknown>>
).map((scenario) => String(scenarioScreenshotName(scenario)));
const expectedScreenshotCount = 7;

describe("browser visual baseline policy", () => {
  it("rejects incomplete baseline screenshot sets with a JSON report", () => {
    const packageJson = JSON.parse(
      readFileSync(join(repoRoot, "package.json"), "utf8"),
    ) as { scripts?: Record<string, string> };

    expect(packageJson.scripts?.["visual:qa:baseline-policy"]).toContain(
      "scripts/browser-visual-baseline-policy.mjs",
    );

    const baselineDir = mkdtempSync(join(tmpdir(), "video-creater-baseline-"));
    writeFileSync(join(baselineDir, "home-desktop.png"), "not-empty");

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/browser-visual-baseline-policy.mjs"),
        "--baseline",
        baselineDir,
        "--comparison",
        join(baselineDir, "missing-comparison.json"),
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
      },
    );

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("baseline policy failed");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      expectedScreenshots: number;
      missingScreenshots: string[];
      comparisonStatus: string;
    };
    expect(report.ok).toBe(false);
    expect(report.expectedScreenshots).toBe(expectedScreenshotCount);
    expect(report.missingScreenshots).toContain("settings-narrow.png");
    expect(report.comparisonStatus).toBe("missing");
  });

  it("rejects stale baseline manifests before they can be used for release gates", () => {
    const baselineDir = mkdtempSync(join(tmpdir(), "video-creater-baseline-"));
    copyCommittedBaseline(baselineDir);

    const comparisonPath = join(baselineDir, "comparison.json");
    writePassingComparison(comparisonPath, baselineDir);

    const manifestPath = join(baselineDir, "browser-visual-baseline-manifest.json");
    writeFileSync(
      manifestPath,
      JSON.stringify({
        schemaVersion: 1,
        screenshots: expectedScreenshots.filter(
          (screenshot) => screenshot !== "settings-narrow.png",
        ),
        platformThresholds: {
          "test-platform": {
            threshold: 0.01,
            channelThreshold: 4,
          },
        },
      }),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/browser-visual-baseline-policy.mjs"),
        "--baseline",
        baselineDir,
        "--comparison",
        comparisonPath,
        "--manifest",
        manifestPath,
        "--platform",
        "test-platform",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      manifestStatus: string;
      manifestMissingScreenshots: string[];
      platformThresholdStatus: string;
    };
    expect(report.ok).toBe(false);
    expect(report.manifestStatus).toBe("failed");
    expect(report.manifestMissingScreenshots).toEqual([
      "settings-narrow.png",
    ]);
    expect(report.platformThresholdStatus).toBe("passed");
  });

  it("rejects unexpected PNG files in the baseline screenshot directory", () => {
    const baselineDir = mkdtempSync(join(tmpdir(), "video-creater-baseline-"));
    copyCommittedBaseline(baselineDir);
    writeFileSync(join(baselineDir, "stale-hover-state.png"), "not-empty");

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/browser-visual-baseline-policy.mjs"),
        "--baseline",
        baselineDir,
        "--platform",
        "darwin-arm64",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      unexpectedBaselineScreenshots: string[];
      manifestStatus: string;
    };
    expect(report.ok).toBe(false);
    expect(report.unexpectedBaselineScreenshots).toEqual([
      "stale-hover-state.png",
    ]);
    expect(report.manifestStatus).toBe("passed");
  });

  it("rejects baseline comparisons that do not cover the full screenshot matrix", () => {
    const baselineDir = mkdtempSync(join(tmpdir(), "video-creater-baseline-"));
    copyCommittedBaseline(baselineDir);

    const comparisonPath = join(baselineDir, "comparison.json");
    writeFileSync(
      comparisonPath,
      JSON.stringify({
        status: "passed",
        comparedFrames: expectedScreenshots
          .filter((screenshot) => screenshot !== "settings-narrow.png")
          .map((screenshot, index) => ({
            timelineSeconds: index,
            previewFrame: join(baselineDir, screenshot),
            renderedFrame: join(baselineDir, screenshot),
            diffFrame: null,
            mismatchRatio: 0,
            passed: true,
          })),
      }),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/browser-visual-baseline-policy.mjs"),
        "--baseline",
        baselineDir,
        "--comparison",
        comparisonPath,
        "--platform",
        "darwin-arm64",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      comparisonStatus: string;
      comparisonMissingScreenshots: string[];
      manifestStatus: string;
    };
    expect(report.ok).toBe(false);
    expect(report.comparisonStatus).toBe("failed");
    expect(report.comparisonMissingScreenshots).toEqual([
      "settings-narrow.png",
    ]);
    expect(report.manifestStatus).toBe("passed");
  });

  it("can require fresh comparison evidence for release gates", () => {
    const baselineDir = mkdtempSync(join(tmpdir(), "video-creater-baseline-"));
    copyCommittedBaseline(baselineDir);

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/browser-visual-baseline-policy.mjs"),
        "--baseline",
        baselineDir,
        "--require-comparison",
        "--platform",
        "darwin-arm64",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      comparisonStatus: string;
      comparisonRequired: boolean;
      manifestStatus: string;
    };
    expect(report.ok).toBe(false);
    expect(report.comparisonStatus).toBe("required");
    expect(report.comparisonRequired).toBe(true);
    expect(report.manifestStatus).toBe("passed");
  });

  it("uses the committed release baseline manifest for a complete baseline", () => {
    const baselineDir = mkdtempSync(join(tmpdir(), "video-creater-baseline-"));
    copyCommittedBaseline(baselineDir);

    const comparisonPath = join(baselineDir, "comparison.json");
    writePassingComparison(comparisonPath, baselineDir);

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/browser-visual-baseline-policy.mjs"),
        "--baseline",
        baselineDir,
        "--comparison",
        comparisonPath,
        "--platform",
        "darwin-arm64",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
      },
    );

    expect(result.status).toBe(0);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      expectedScreenshots: number;
      manifestStatus: string;
      manifestPath: string;
      platformThresholdStatus: string;
      baselineIntegrityStatus: string;
    };
    expect(report.ok).toBe(true);
    expect(report.expectedScreenshots).toBe(expectedScreenshotCount);
    expect(report.manifestStatus).toBe("passed");
    expect(report.manifestPath).toBe(
      join(repoRoot, "docs/visual-qa/browser-visual-baseline-manifest.json"),
    );
    expect(report.platformThresholdStatus).toBe("passed");
    expect(report.baselineIntegrityStatus).toBe("passed");
  });

  it("rejects a baseline whose SHA-256 differs from schema v2 manifest metadata", () => {
    const baselineDir = mkdtempSync(join(tmpdir(), "video-creater-baseline-"));
    copyCommittedBaseline(baselineDir);
    const manifestPath = join(baselineDir, "manifest.json");
    const manifest = createIntegrityManifest(baselineDir);
    requiredValue(
      manifest.artifacts["settings-desktop.png"],
      "settings desktop baseline artifact",
    ).sha256 = "0".repeat(64);
    writeFileSync(manifestPath, JSON.stringify(manifest));

    const result = runPolicy(baselineDir, manifestPath);

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      manifestStatus: string;
      baselineIntegrityStatus: string;
      baselineIntegrityFailures: Array<{
        screenshot: string;
        fields: string[];
      }>;
    };
    expect(report.ok).toBe(false);
    expect(report.manifestStatus).toBe("passed");
    expect(report.baselineIntegrityStatus).toBe("failed");
    expect(report.baselineIntegrityFailures).toContainEqual({
      screenshot: "settings-desktop.png",
      fields: ["sha256"],
    });
  });

  it("rejects a baseline whose PNG dimensions differ from schema v2 manifest metadata", () => {
    const baselineDir = mkdtempSync(join(tmpdir(), "video-creater-baseline-"));
    copyCommittedBaseline(baselineDir);
    const manifestPath = join(baselineDir, "manifest.json");
    const manifest = createIntegrityManifest(baselineDir);
    requiredValue(
      manifest.artifacts["home-narrow.png"],
      "home narrow baseline artifact",
    ).width += 1;
    writeFileSync(manifestPath, JSON.stringify(manifest));

    const result = runPolicy(baselineDir, manifestPath);

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      baselineIntegrityStatus: string;
      baselineIntegrityFailures: Array<{
        screenshot: string;
        fields: string[];
      }>;
    };
    expect(report.baselineIntegrityStatus).toBe("failed");
    expect(report.baselineIntegrityFailures).toContainEqual({
      screenshot: "home-narrow.png",
      fields: ["width"],
    });
  });

  it("defaults to the committed release baseline screenshot directory", () => {
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/browser-visual-baseline-policy.mjs"),
        "--platform",
        "darwin-arm64",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
      },
    );

    expect(result.status).toBe(0);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      baselineDir: string;
      expectedScreenshots: number;
      missingScreenshots: string[];
      emptyScreenshots: string[];
      manifestStatus: string;
      platformThresholdStatus: string;
      baselineIntegrityStatus: string;
    };
    expect(report.ok).toBe(true);
    expect(report.baselineDir).toBe(defaultBaselineDir);
    expect(report.expectedScreenshots).toBe(expectedScreenshotCount);
    expect(report.missingScreenshots).toEqual([]);
    expect(report.emptyScreenshots).toEqual([]);
    expect(report.manifestStatus).toBe("passed");
    expect(report.platformThresholdStatus).toBe("passed");
    expect(report.baselineIntegrityStatus).toBe("passed");
  });
});

type IntegrityManifest = {
  schemaVersion: number;
  screenshots: string[];
  artifacts: Record<
    string,
    { width: number; height: number; bytes: number; sha256: string }
  >;
  platformThresholds: Record<
    string,
    { threshold: number; channelThreshold: number }
  >;
};

function copyCommittedBaseline(targetDir: string) {
  for (const screenshot of expectedScreenshots) {
    copyFileSync(
      join(defaultBaselineDir, screenshot),
      join(targetDir, screenshot),
    );
  }
}

function createIntegrityManifest(baselineDir: string): IntegrityManifest {
  return {
    schemaVersion: 2,
    screenshots: [...expectedScreenshots],
    artifacts: Object.fromEntries(
      expectedScreenshots.map((screenshot) => {
        const png = readFileSync(join(baselineDir, screenshot));
        return [
          screenshot,
          {
            width: png.readUInt32BE(16),
            height: png.readUInt32BE(20),
            bytes: png.length,
            sha256: createHash("sha256").update(png).digest("hex"),
          },
        ];
      }),
    ),
    platformThresholds: {
      "darwin-arm64": { threshold: 0.01, channelThreshold: 4 },
      "test-platform": { threshold: 0.01, channelThreshold: 4 },
    },
  };
}

function runPolicy(baselineDir: string, manifestPath: string) {
  return spawnSync(
    process.execPath,
    [
      join(repoRoot, "scripts/browser-visual-baseline-policy.mjs"),
      "--baseline",
      baselineDir,
      "--manifest",
      manifestPath,
      "--platform",
      "test-platform",
    ],
    {
      cwd: repoRoot,
      encoding: "utf8",
    },
  );
}

function writePassingComparison(comparisonPath: string, screenshotDir: string) {
  writeFileSync(
    comparisonPath,
    JSON.stringify({
      status: "passed",
      comparedFrames: expectedScreenshots.map((screenshot, index) => ({
        timelineSeconds: index,
        previewFrame: join(screenshotDir, screenshot),
        renderedFrame: join(screenshotDir, screenshot),
        diffFrame: null,
        mismatchRatio: 0,
        passed: true,
      })),
    }),
  );
}
