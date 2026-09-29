import { execFileSync, spawnSync } from "node:child_process";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { deflateSync } from "node:zlib";
import { describe, expect, it } from "vitest";
import { requiredAt } from "@/test-utils/required";

const repoRoot = process.cwd();

describe("browser preview/render visual regression automation", () => {
  it("exposes a runner that captures preview frames and compares them to rendered frames", () => {
    const packageJson = JSON.parse(
      readFileSync(join(repoRoot, "package.json"), "utf8"),
    ) as { scripts?: Record<string, string> };
    const scriptSource = readFileSync(
      join(repoRoot, "scripts/browser-preview-render-qa.mjs"),
      "utf8",
    );

    expect(packageJson.scripts?.["visual:qa:preview-render"]).toBe(
      "node scripts/browser-preview-render-qa.mjs",
    );
    expect(packageJson.scripts?.["visual:qa:preview-render-policy"]).toBe(
      "node scripts/preview-render-report-policy.mjs",
    );
    expect(scriptSource).toContain("--rendered-frame");
    expect(scriptSource).toContain("--rendered-video");
    expect(scriptSource).toContain("--frame-time");
    expect(scriptSource).toContain("--render-report");
    expect(scriptSource).toContain("--project-dir");
    expect(scriptSource).toContain("--project-report-id");
    expect(scriptSource).toContain("#project-folder-path");
    expect(scriptSource).toContain('name: "Open project folder"');
    expect(scriptSource).toContain("load that exact split project");
    expect(scriptSource).toContain("Preview screenshot was not written");
    expect(scriptSource).toContain("native project bridge");
    expect(scriptSource).toContain("--fail-on-mismatch");
    expect(scriptSource).toContain("attachComparisonToProjectReport");
    expect(scriptSource).toContain("renders/index.json");
    expect(scriptSource).toContain("previewComparison");
    expect(scriptSource).toContain("writeFileSync");
    expect(scriptSource).toContain("rendered-frames");
    expect(scriptSource).toContain("-frames:v");
    expect(scriptSource).toContain("--duration");
    expect(scriptSource).toContain("Preview viewport");
    expect(scriptSource).toContain("Timeline canvas");
    expect(scriptSource).toContain('name: "Preview scrubber"');
    expect(scriptSource).not.toContain("Timeline editor");
    expect(scriptSource).toContain("page.mouse.click");
    expect(scriptSource).toContain("page.locator");
    expect(scriptSource).toContain("screenshot({ path:");
    expect(scriptSource).toContain("scripts/compare-preview-render-frames.mjs");
    expect(scriptSource).toContain("previewComparison");
  });

  it("validates project-rooted preview/render report evidence before release use", () => {
    const dir = mkdtempSync(join(tmpdir(), "video-creater-preview-policy-"));
    const projectDir = join(dir, "project");
    const renderDir = join(projectDir, "renders", "render-preview-1");
    const reportPath = join(renderDir, "report.json");
    mkdirSync(join(renderDir, "preview-qa", "preview-frames"), { recursive: true });
    mkdirSync(join(renderDir, "preview-qa"), { recursive: true });
    writeRgbaPng(
      join(renderDir, "preview-qa", "preview-frames", "preview-0001.png"),
      1,
      1,
      [[20, 40, 60, 255]],
    );
    writeRgbaPng(join(renderDir, "rendered.png"), 1, 1, [[20, 40, 60, 255]]);
    writeFileSync(
      join(renderDir, "preview-qa", "preview-comparison.json"),
      `${JSON.stringify(
        {
          status: "passed",
          comparedFrames: [
            {
              timelineSeconds: 0.5,
              previewFrame:
                "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
              renderedFrame: "renders/render-preview-1/rendered.png",
              diffFrame: null,
              mismatchRatio: 0,
              passed: true,
            },
          ],
        },
        null,
        2,
      )}\n`,
    );
    writeFileSync(
      reportPath,
      `${JSON.stringify(
        {
          id: "render-preview-1",
          status: "completed",
          artifacts: [
            "renders/render-preview-1/preview-qa/preview-comparison.json",
            "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
            "renders/render-preview-1/rendered.png",
          ],
          previewComparison: {
            status: "passed",
            comparedFrames: [
              {
                timelineSeconds: 0.5,
                previewFrame:
                  "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
                renderedFrame: "renders/render-preview-1/rendered.png",
                diffFrame: null,
                mismatchRatio: 0,
                passed: true,
              },
            ],
          },
        },
        null,
        2,
      )}\n`,
    );

    const output = execFileSync(
      "node",
      [
        "scripts/preview-render-report-policy.mjs",
        "--report",
        reportPath,
        "--project-dir",
        projectDir,
        "--min-frames",
        "1",
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );
    expect(JSON.parse(output)).toMatchObject({
      status: "passed",
      comparedFrameCount: 1,
      failures: [],
    });
  });

  it("accepts release evidence from extracted final-render frames", () => {
    const dir = mkdtempSync(join(tmpdir(), "video-creater-preview-policy-extracted-rendered-"));
    const projectDir = join(dir, "project");
    const renderDir = join(projectDir, "renders", "render-preview-1");
    const reportPath = join(renderDir, "report.json");
    mkdirSync(join(renderDir, "preview-qa", "preview-frames"), { recursive: true });
    mkdirSync(join(renderDir, "preview-qa", "rendered-frames"), { recursive: true });
    writeRgbaPng(
      join(renderDir, "preview-qa", "preview-frames", "preview-0001.png"),
      1,
      1,
      [[20, 40, 60, 255]],
    );
    writeRgbaPng(
      join(renderDir, "preview-qa", "rendered-frames", "rendered-0001.png"),
      1,
      1,
      [[20, 40, 60, 255]],
    );
    writeFileSync(
      join(renderDir, "preview-qa", "preview-comparison.json"),
      `${JSON.stringify(
        {
          status: "passed",
          comparedFrames: [
            {
              timelineSeconds: 0.5,
              previewFrame:
                "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
              renderedFrame:
                "renders/render-preview-1/preview-qa/rendered-frames/rendered-0001.png",
              diffFrame: null,
              mismatchRatio: 0,
              passed: true,
            },
          ],
        },
        null,
        2,
      )}\n`,
    );
    writeFileSync(
      reportPath,
      `${JSON.stringify(
        {
          id: "render-preview-1",
          status: "completed",
          artifacts: [
            "renders/render-preview-1/preview-qa/preview-comparison.json",
            "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
            "renders/render-preview-1/preview-qa/rendered-frames/rendered-0001.png",
          ],
          previewComparison: {
            status: "passed",
            comparedFrames: [
              {
                timelineSeconds: 0.5,
                previewFrame:
                  "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
                renderedFrame:
                  "renders/render-preview-1/preview-qa/rendered-frames/rendered-0001.png",
                diffFrame: null,
                mismatchRatio: 0,
                passed: true,
              },
            ],
          },
        },
        null,
        2,
      )}\n`,
    );

    const output = execFileSync(
      "node",
      [
        "scripts/preview-render-report-policy.mjs",
        "--report",
        reportPath,
        "--project-dir",
        projectDir,
        "--min-frames",
        "1",
        "--require-extracted-rendered-frames",
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(JSON.parse(output)).toMatchObject({
      status: "passed",
      comparedFrameCount: 1,
      failures: [],
    });
  });

  it("rejects retained comparison JSON that differs from the render report evidence", () => {
    const dir = mkdtempSync(join(tmpdir(), "video-creater-preview-policy-stale-json-"));
    const projectDir = join(dir, "project");
    const renderDir = join(projectDir, "renders", "render-preview-1");
    const reportPath = join(renderDir, "report.json");
    mkdirSync(join(renderDir, "preview-qa", "preview-frames"), { recursive: true });
    mkdirSync(join(renderDir, "preview-qa", "rendered-frames"), { recursive: true });
    writeRgbaPng(
      join(renderDir, "preview-qa", "preview-frames", "preview-0001.png"),
      1,
      1,
      [[20, 40, 60, 255]],
    );
    writeRgbaPng(
      join(renderDir, "preview-qa", "rendered-frames", "rendered-0001.png"),
      1,
      1,
      [[20, 40, 60, 255]],
    );
    writeFileSync(
      join(renderDir, "preview-qa", "preview-comparison.json"),
      `${JSON.stringify(
        {
          status: "failed",
          comparedFrames: [],
        },
        null,
        2,
      )}\n`,
    );
    writeFileSync(
      reportPath,
      `${JSON.stringify(
        {
          id: "render-preview-1",
          status: "completed",
          artifacts: [
            "renders/render-preview-1/preview-qa/preview-comparison.json",
            "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
            "renders/render-preview-1/preview-qa/rendered-frames/rendered-0001.png",
          ],
          previewComparison: {
            status: "passed",
            comparedFrames: [
              {
                timelineSeconds: 0.5,
                previewFrame:
                  "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
                renderedFrame:
                  "renders/render-preview-1/preview-qa/rendered-frames/rendered-0001.png",
                diffFrame: null,
                mismatchRatio: 0,
                passed: true,
              },
            ],
          },
        },
        null,
        2,
      )}\n`,
    );

    const output = spawnSync(
      "node",
      [
        "scripts/preview-render-report-policy.mjs",
        "--report",
        reportPath,
        "--project-dir",
        projectDir,
        "--min-frames",
        "1",
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(output.status).toBe(1);
    const result = JSON.parse(output.stdout) as { status: string; failures: string[] };
    expect(result.status).toBe("failed");
    expect(
      result.failures.some((failure) => failure.includes("does not match")),
    ).toBe(true);
  });

  it("rejects preview/render report evidence without retained comparison JSON", () => {
    const dir = mkdtempSync(join(tmpdir(), "video-creater-preview-policy-missing-json-"));
    const projectDir = join(dir, "project");
    const renderDir = join(projectDir, "renders", "render-preview-1");
    const reportPath = join(renderDir, "report.json");
    mkdirSync(join(renderDir, "preview-qa", "preview-frames"), { recursive: true });
    writeRgbaPng(
      join(renderDir, "preview-qa", "preview-frames", "preview-0001.png"),
      1,
      1,
      [[20, 40, 60, 255]],
    );
    writeRgbaPng(join(renderDir, "rendered.png"), 1, 1, [[20, 40, 60, 255]]);
    writeFileSync(
      reportPath,
      `${JSON.stringify(
        {
          id: "render-preview-1",
          status: "completed",
          artifacts: [
            "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
            "renders/render-preview-1/rendered.png",
          ],
          previewComparison: {
            status: "passed",
            comparedFrames: [
              {
                timelineSeconds: 0.5,
                previewFrame:
                  "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
                renderedFrame: "renders/render-preview-1/rendered.png",
                diffFrame: null,
                mismatchRatio: 0,
                passed: true,
              },
            ],
          },
        },
        null,
        2,
      )}\n`,
    );

    expect(() =>
      execFileSync(
        "node",
        [
          "scripts/preview-render-report-policy.mjs",
          "--report",
          reportPath,
          "--project-dir",
          projectDir,
          "--min-frames",
          "1",
        ],
        { cwd: repoRoot, encoding: "utf8", stdio: "pipe" },
      ),
    ).toThrow();
  });

  it("rejects preview/render report evidence from incomplete renders", () => {
    const dir = mkdtempSync(join(tmpdir(), "video-creater-preview-policy-incomplete-"));
    const projectDir = join(dir, "project");
    const renderDir = join(projectDir, "renders", "render-preview-1");
    const reportPath = join(renderDir, "report.json");
    mkdirSync(join(renderDir, "preview-qa", "preview-frames"), { recursive: true });
    mkdirSync(join(renderDir, "preview-qa"), { recursive: true });
    writeRgbaPng(
      join(renderDir, "preview-qa", "preview-frames", "preview-0001.png"),
      1,
      1,
      [[20, 40, 60, 255]],
    );
    writeRgbaPng(join(renderDir, "rendered.png"), 1, 1, [[20, 40, 60, 255]]);
    writeFileSync(
      join(renderDir, "preview-qa", "preview-comparison.json"),
      `${JSON.stringify({ status: "passed", comparedFrames: [] }, null, 2)}\n`,
    );
    writeFileSync(
      reportPath,
      `${JSON.stringify(
        {
          id: "render-preview-1",
          status: "failed",
          artifacts: [
            "renders/render-preview-1/preview-qa/preview-comparison.json",
            "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
            "renders/render-preview-1/rendered.png",
          ],
          previewComparison: {
            status: "passed",
            comparedFrames: [
              {
                timelineSeconds: 0.5,
                previewFrame:
                  "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
                renderedFrame: "renders/render-preview-1/rendered.png",
                diffFrame: null,
                mismatchRatio: 0,
                passed: true,
              },
            ],
          },
        },
        null,
        2,
      )}\n`,
    );

    expect(() =>
      execFileSync(
        "node",
        [
          "scripts/preview-render-report-policy.mjs",
          "--report",
          reportPath,
          "--project-dir",
          projectDir,
          "--min-frames",
          "1",
        ],
        { cwd: repoRoot, encoding: "utf8", stdio: "pipe" },
      ),
    ).toThrow();
  });

  it("rejects preview/render report evidence with unlisted or missing artifacts", () => {
    const dir = mkdtempSync(join(tmpdir(), "video-creater-preview-policy-bad-"));
    const projectDir = join(dir, "project");
    const renderDir = join(projectDir, "renders", "render-preview-1");
    const reportPath = join(renderDir, "report.json");
    mkdirSync(renderDir, { recursive: true });
    writeFileSync(
      reportPath,
      `${JSON.stringify(
        {
          id: "render-preview-1",
          status: "completed",
          artifacts: [],
          previewComparison: {
            status: "failed",
            comparedFrames: [
              {
                timelineSeconds: 0.5,
                previewFrame:
                  "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
                renderedFrame: "renders/render-preview-1/rendered.png",
                diffFrame: "renders/render-preview-1/preview-qa/diffs/diff-0001.png",
                mismatchRatio: 0.42,
                passed: false,
              },
            ],
          },
        },
        null,
        2,
      )}\n`,
    );

    expect(() =>
      execFileSync(
        "node",
        [
          "scripts/preview-render-report-policy.mjs",
          "--report",
          reportPath,
          "--project-dir",
          projectDir,
        ],
        { cwd: repoRoot, encoding: "utf8", stdio: "pipe" },
      ),
    ).toThrow();
  });

  it("rejects release evidence whose rendered frames were supplied outside extracted final-video frames", () => {
    const dir = mkdtempSync(join(tmpdir(), "video-creater-preview-policy-supplied-rendered-"));
    const projectDir = join(dir, "project");
    const renderDir = join(projectDir, "renders", "render-preview-1");
    const reportPath = join(renderDir, "report.json");
    mkdirSync(join(renderDir, "preview-qa", "preview-frames"), { recursive: true });
    mkdirSync(join(renderDir, "preview-qa"), { recursive: true });
    writeRgbaPng(
      join(renderDir, "preview-qa", "preview-frames", "preview-0001.png"),
      1,
      1,
      [[20, 40, 60, 255]],
    );
    writeRgbaPng(join(renderDir, "rendered.png"), 1, 1, [[20, 40, 60, 255]]);
    writeFileSync(
      join(renderDir, "preview-qa", "preview-comparison.json"),
      `${JSON.stringify({ status: "passed", comparedFrames: [] }, null, 2)}\n`,
    );
    writeFileSync(
      reportPath,
      `${JSON.stringify(
        {
          id: "render-preview-1",
          status: "completed",
          artifacts: [
            "renders/render-preview-1/preview-qa/preview-comparison.json",
            "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
            "renders/render-preview-1/rendered.png",
          ],
          previewComparison: {
            status: "passed",
            comparedFrames: [
              {
                timelineSeconds: 0.5,
                previewFrame:
                  "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
                renderedFrame: "renders/render-preview-1/rendered.png",
                diffFrame: null,
                mismatchRatio: 0,
                passed: true,
              },
            ],
          },
        },
        null,
        2,
      )}\n`,
    );

    const result = spawnSync(
      process.execPath,
      [
        "scripts/preview-render-report-policy.mjs",
        "--report",
        reportPath,
        "--project-dir",
        projectDir,
        "--require-extracted-rendered-frames",
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("renderedFrame must come from extracted final-render frames"),
    )).toBe(true);
  });

  it("attaches executable preview/render evidence to split-project reports with project-rooted paths", () => {
    const dir = mkdtempSync(join(tmpdir(), "video-creater-browser-preview-qa-"));
    const projectDir = join(dir, "project");
    const renderDir = join(projectDir, "renders", "render-preview-1");
    const outDir = join(renderDir, "preview-qa");
    const renderedFrame = join(renderDir, "rendered.png");
    const stubPwcli = join(dir, "pwcli-stub.mjs");
    mkdirSync(renderDir, { recursive: true });

    writeRgbaPng(renderedFrame, 1, 1, [[20, 40, 60, 255]]);
    writeFileSync(
      join(renderDir, "report.json"),
      `${JSON.stringify(
        {
          id: "render-preview-1",
          status: "completed",
          outputPath: "renders/render-preview-1/output.webm",
          durationSeconds: 1,
          streams: { video: true, audio: false },
          checks: { duration: "passed" },
          artifacts: [],
          logPath: "renders/render-preview-1/render.log",
          createdAt: "2026-07-04T00:00:00Z",
        },
        null,
        2,
      )}\n`,
    );
    writeFileSync(
      stubPwcli,
      `#!/usr/bin/env node
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";
const args = process.argv.slice(2);
const runCodeIndex = args.indexOf("run-code");
if (runCodeIndex >= 0) {
  const code = args[runCodeIndex + 1] || "";
  const match = code.match(/screenshot\\(\\{ path: "([^"]+)"/);
  if (match) {
    mkdirSync(dirname(match[1]), { recursive: true });
    copyFileSync(process.env.STUB_PREVIEW_SOURCE, match[1]);
  }
}
`,
    );
    chmodSync(stubPwcli, 0o755);

    execFileSync(
      "node",
      [
        "scripts/browser-preview-render-qa.mjs",
        "--duration",
        "1",
        "--rendered-frame",
        `0.5:${renderedFrame}`,
        "--out",
        outDir,
        "--project-dir",
        projectDir,
        "--project-report-id",
        "render-preview-1",
        "--no-sample-project",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PWCLI: stubPwcli,
          STUB_PREVIEW_SOURCE: renderedFrame,
        },
      },
    );

    const report = JSON.parse(
      readFileSync(join(renderDir, "report.json"), "utf8"),
    ) as {
      artifacts: string[];
      previewComparison: {
        status: string;
        comparedFrames: Array<{
          previewFrame: string;
          renderedFrame: string;
          diffFrame: string | null;
          passed: boolean;
        }>;
      };
    };
    const index = JSON.parse(
      readFileSync(join(projectDir, "renders", "index.json"), "utf8"),
    ) as { reports: Array<{ reportId: string; artifactCount: number }> };

    expect(report.previewComparison.status).toBe("passed");
    expect(report.previewComparison.comparedFrames[0]).toMatchObject({
      previewFrame: "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
      renderedFrame: "renders/render-preview-1/rendered.png",
      diffFrame: null,
      passed: true,
    });
    expect(report.artifacts).toEqual(
      expect.arrayContaining([
        "renders/render-preview-1/preview-qa/preview-comparison.json",
        "renders/render-preview-1/preview-qa/preview-frames/preview-0001.png",
        "renders/render-preview-1/rendered.png",
      ]),
    );
    expect(report.artifacts.every((artifact) => !artifact.startsWith("/"))).toBe(true);
    expect(index.reports).toContainEqual(
      expect.objectContaining({
        reportId: "render-preview-1",
        artifactCount: report.artifacts.length,
      }),
    );
  });
});

function writeRgbaPng(
  path: string,
  width: number,
  height: number,
  pixels: Array<readonly [number, number, number, number]>,
) {
  const rowBytes = width * 4;
  const raw = Buffer.alloc((rowBytes + 1) * height);
  for (let y = 0; y < height; y += 1) {
    raw[y * (rowBytes + 1)] = 0;
    for (let x = 0; x < width; x += 1) {
      const pixel = requiredAt(pixels, y * width + x, "RGBA pixel");
      const offset = y * (rowBytes + 1) + 1 + x * 4;
      raw[offset] = pixel[0];
      raw[offset + 1] = pixel[1];
      raw[offset + 2] = pixel[2];
      raw[offset + 3] = pixel[3];
    }
  }

  writeFileSync(
    path,
    Buffer.concat([
      Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
      pngChunk("IHDR", ihdr(width, height)),
      pngChunk("IDAT", deflateSync(raw)),
      pngChunk("IEND", Buffer.alloc(0)),
    ]),
  );
}

function ihdr(width: number, height: number) {
  const data = Buffer.alloc(13);
  data.writeUInt32BE(width, 0);
  data.writeUInt32BE(height, 4);
  data[8] = 8;
  data[9] = 6;
  return data;
}

function pngChunk(type: string, data: Buffer) {
  const typeBuffer = Buffer.from(type, "ascii");
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])), 0);
  return Buffer.concat([length, typeBuffer, data, crc]);
}

function crc32(buffer: Buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = crc & 1 ? 0xedb88320 ^ (crc >>> 1) : crc >>> 1;
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}
