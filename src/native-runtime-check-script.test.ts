import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { describe, expect, it } from "vitest";

const repoRoot = process.cwd();

describe("native runtime verification script", () => {
  it("checks the reviewed AudioToolbox AAC decoder by default", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-runtime-check-"));
    const fakeGstInspect = join(tempDir, "gst-inspect-1.0");
    const inspectedFactories = join(tempDir, "factories.txt");
    writeFileSync(
      fakeGstInspect,
      `#!/bin/sh\necho "$1" >> "${inspectedFactories}"\nexit 0\n`,
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/check-native-runtime.mjs"),
        "--diagnostics-dir",
        tempDir,
        "--min-free-gb",
        "0.001",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
        },
      },
    );

    expect(result.status).toBe(0);
    expect(readFileSync(inspectedFactories, "utf8").split(/\r?\n/)).toContain("atdec");
    expect(JSON.parse(result.stdout)).toMatchObject({
      ok: true,
      factories: expect.arrayContaining([
        expect.objectContaining({ name: "atdec", status: "available", ok: true }),
      ]),
    });
  });

  it("reports GStreamer factory timeouts without hanging", () => {
    const packageJson = JSON.parse(
      readFileSync(join(repoRoot, "package.json"), "utf8"),
    ) as { scripts?: Record<string, string> };

    expect(packageJson.scripts?.["check:native-runtime"]).toBe(
      "node scripts/check-native-runtime.mjs",
    );
    expect(packageJson.scripts?.["check:native-export-artifacts"]).toBe(
      "node scripts/native-export-artifact-policy.mjs",
    );
    expect(packageJson.scripts?.["evidence:native-export"]).toBe(
      "cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-native-export-evidence --",
    );

    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-runtime-check-"));
    const fakeGstInspect = join(tempDir, "gst-inspect-1.0");
    writeFileSync(
      fakeGstInspect,
      "#!/bin/sh\nsleep 5\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/check-native-runtime.mjs"),
        "--factory",
        "fakesrc",
        "--timeout-ms",
        "50",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
        },
        timeout: 1_000,
      },
    );

    expect(result.status).toBe(1);
    expect(result.error).toBeUndefined();
    expect(result.stderr).toContain("fakesrc timed out after 50ms");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      factories: Array<{ name: string; status: string; timedOut: boolean }>;
    };
    expect(report.ok).toBe(false);
    expect(report.factories).toEqual([
      expect.objectContaining({
        name: "fakesrc",
        status: "timeout",
        timedOut: true,
      }),
    ]);
  });

  it("prints a structured repair plan when GStreamer factories are unavailable", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-runtime-check-"));
    const fakeGstInspect = join(tempDir, "gst-inspect-1.0");
    writeFileSync(
      fakeGstInspect,
      "#!/bin/sh\necho missing factory >&2\nexit 1\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/check-native-runtime.mjs"),
        "--factory",
        "vtenc_h264",
        "--timeout-ms",
        "5000",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      repair: {
        command: string;
        packages: string[];
        nextSteps: string[];
      };
      factories: Array<{
        name: string;
        status: string;
        remediation: string;
      }>;
    };
    expect(report.ok).toBe(false);
    expect(report.repair.command).toBe(
      "brew install gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-editing-services",
    );
    expect(report.repair.packages).toContain("gst-editing-services");
    expect(report.repair.nextSteps).toContain(
      "Re-run pnpm check:native-runtime after installing or repairing the GStreamer runtime.",
    );
    expect(report.factories[0]).toMatchObject({
      name: "vtenc_h264",
      status: "missing",
      remediation:
        "Install or repair the approved GStreamer runtime, then verify vtenc_h264 with gst-inspect-1.0.",
    });
  });

  it("reports insufficient workspace storage for native runtime verification", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-runtime-check-"));
    const fakeGstInspect = join(tempDir, "gst-inspect-1.0");
    writeFileSync(
      fakeGstInspect,
      "#!/bin/sh\nexit 0\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/check-native-runtime.mjs"),
        "--factory",
        "fakesrc",
        "--diagnostics-dir",
        tempDir,
        "--min-free-gb",
        "1000000",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
        },
      },
    );

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("workspace storage below required free space");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      storage: {
        ok: boolean;
        status: string;
        path: string;
        requiredFreeBytes: number;
        availableBytes: number;
        remediation: string;
      };
      repair: {
        nextSteps: string[];
      };
    };
    expect(report.ok).toBe(false);
    expect(report.storage).toMatchObject({
      ok: false,
      status: "insufficient",
      path: tempDir,
      requiredFreeBytes: 1_000_000 * 1024 ** 3,
      remediation:
        "Free workspace or Cargo target disk space before running native export verification.",
    });
    expect(report.storage.availableBytes).toBeGreaterThan(0);
    expect(report.repair.nextSteps).toContain(
      "If Cargo or native export checks stay silent, check free disk space and clear stale build artifacts before retrying.",
    );
  });

  it("creates a requested diagnostics directory before checking its storage", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-runtime-check-"));
    const fakeGstInspect = join(tempDir, "gst-inspect-1.0");
    const diagnosticsDir = join(tempDir, "new", "release-evidence");
    const reportPath = join(diagnosticsDir, "native-runtime.json");
    writeFileSync(
      fakeGstInspect,
      "#!/bin/sh\nexit 0\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/check-native-runtime.mjs"),
        "--factory",
        "fakesrc",
        "--diagnostics-dir",
        diagnosticsDir,
        "--report",
        reportPath,
        "--min-free-gb",
        "0.001",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
        },
      },
    );

    expect(result.status).toBe(0);
    expect(existsSync(diagnosticsDir)).toBe(true);
    const report = JSON.parse(result.stdout) as { reportPath?: string };
    expect(report).toMatchObject({
      ok: true,
      storage: { ok: true, status: "sufficient", path: diagnosticsDir },
    });
    expect(report.reportPath).toBe(reportPath);
    expect(JSON.parse(readFileSync(reportPath, "utf8"))).toMatchObject(report);
  });

  it("validates retained native export artifact evidence for required profiles", () => {
    const projectDir = mkdtempSync(join(tmpdir(), "video-creater-native-export-"));
    writeFileSync(
      join(projectDir, "native-runtime.json"),
      `${JSON.stringify(
        {
          ok: true,
          factories: [
            { name: "mp4mux", ok: true, status: "available" },
            { name: "qtmux", ok: true, status: "available" },
            { name: "vtenc_h264", ok: true, status: "available" },
            { name: "vtenc_h265", ok: true, status: "available" },
            { name: "vtenc_prores", ok: true, status: "available" },
          ],
        },
        null,
        2,
      )}\n`,
    );
    const artifacts = [
      profileFixture("mp4H264", "export-h264", "output.mp4", "h264"),
      profileFixture("mp4H265", "export-h265", "output.mp4", "hevc"),
      profileFixture("proResMov", "export-prores", "output.mov", "prores"),
    ];
    for (const artifact of artifacts) {
      writeNativeExportFixture(projectDir, artifact);
    }
    writeFileSync(
      join(projectDir, "native-export-evidence.json"),
      `${JSON.stringify(
        {
          nativeRuntimeReport: "native-runtime.json",
          artifacts: artifacts.map(({ profile, jobId, outputName }) => ({
            profile,
            report: `renders/${jobId}/report.json`,
            output: `renders/${jobId}/${outputName}`,
            probe: `renders/${jobId}/ffprobe.json`,
          })),
        },
        null,
        2,
      )}\n`,
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/native-export-artifact-policy.mjs"),
        "--manifest",
        join(projectDir, "native-export-evidence.json"),
        "--project-dir",
        projectDir,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(0);
    expect(JSON.parse(result.stdout)).toMatchObject({
      status: "passed",
      requiredProfiles: ["mp4H264", "mp4H265", "proResMov"],
      failures: [],
    });
  });

  it("validates signed system-only AVFoundation export evidence", () => {
    const projectDir = mkdtempSync(join(tmpdir(), "video-creater-avfoundation-export-"));
    writeFileSync(
      join(projectDir, "avfoundation-exporter.json"),
      `${JSON.stringify(
        {
          schemaVersion: 5,
          status: "passed",
          target: "aarch64-apple-darwin",
          architectures: ["arm64"],
          signed: true,
          signatureRequired: true,
          dynamicDependencies: [
            "/usr/lib/libSystem.B.dylib (compatibility version 1.0.0)",
            "/System/Library/Frameworks/AVFoundation.framework/Versions/A/AVFoundation (compatibility version 1.0.0)",
          ],
          failures: [],
        },
        null,
        2,
      )}\n`,
    );
    const artifacts = [
      profileFixture("mp4H264", "export-h264", "output.mp4", "h264"),
      profileFixture("mp4H265", "export-h265", "output.mp4", "hevc"),
      profileFixture("proResMov", "export-prores", "output.mov", "prores"),
    ];
    for (const artifact of artifacts) {
      writeNativeExportFixture(projectDir, artifact, "avfoundation-native");
    }
    writeFileSync(
      join(projectDir, "native-export-evidence.json"),
      `${JSON.stringify(
        {
          nativeExporterReport: "avfoundation-exporter.json",
          artifacts: artifacts.map(({ profile, jobId, outputName }) => ({
            profile,
            report: `renders/${jobId}/report.json`,
            output: `renders/${jobId}/${outputName}`,
            probe: `renders/${jobId}/ffprobe.json`,
          })),
        },
        null,
        2,
      )}\n`,
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/native-export-artifact-policy.mjs"),
        "--manifest",
        join(projectDir, "native-export-evidence.json"),
        "--project-dir",
        projectDir,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(0);
    expect(JSON.parse(result.stdout)).toMatchObject({ status: "passed", failures: [] });
  });

  it("rejects native export evidence when required runtime factories are missing", () => {
    const projectDir = mkdtempSync(join(tmpdir(), "video-creater-native-export-factory-gap-"));
    writeFileSync(
      join(projectDir, "native-runtime.json"),
      `${JSON.stringify(
        {
          ok: true,
          factories: [
            { name: "mp4mux", ok: true, status: "available" },
          ],
        },
        null,
        2,
      )}\n`,
    );
    const artifacts = [
      profileFixture("mp4H264", "export-h264", "output.mp4", "h264"),
      profileFixture("mp4H265", "export-h265", "output.mp4", "hevc"),
      profileFixture("proResMov", "export-prores", "output.mov", "prores"),
    ];
    for (const artifact of artifacts) {
      writeNativeExportFixture(projectDir, artifact);
    }
    writeFileSync(
      join(projectDir, "native-export-evidence.json"),
      `${JSON.stringify(
        {
          nativeRuntimeReport: "native-runtime.json",
          artifacts: artifacts.map(({ profile, jobId, outputName }) => ({
            profile,
            report: `renders/${jobId}/report.json`,
            output: `renders/${jobId}/${outputName}`,
            probe: `renders/${jobId}/ffprobe.json`,
          })),
        },
        null,
        2,
      )}\n`,
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/native-export-artifact-policy.mjs"),
        "--manifest",
        join(projectDir, "native-export-evidence.json"),
        "--project-dir",
        projectDir,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toEqual(
      expect.arrayContaining([
        "nativeRuntimeReport is missing required factory qtmux",
        "nativeRuntimeReport is missing required factory vtenc_h264",
        "nativeRuntimeReport is missing required factory vtenc_h265",
        "nativeRuntimeReport is missing required factory vtenc_prores",
      ]),
    );
  });

  it("rejects missing native export artifact evidence", () => {
    const projectDir = mkdtempSync(join(tmpdir(), "video-creater-native-export-bad-"));
    mkdirSync(join(projectDir, "renders", "export-h264"), { recursive: true });
    writeFileSync(join(projectDir, "renders", "export-h264", "output.mp4"), "not empty");
    writeFileSync(
      join(projectDir, "renders", "export-h264", "report.json"),
      `${JSON.stringify(
        {
          summary: { status: "completed", outputPath: "renders/export-h264/output.mp4" },
          streams: { video: true, audio: true },
          artifacts: ["renders/export-h264/output.mp4"],
          errors: [],
        },
        null,
        2,
      )}\n`,
    );
    writeFileSync(
      join(projectDir, "renders", "export-h264", "ffprobe.json"),
      `${JSON.stringify(
        {
          streams: [
            {
              codec_type: "video",
              codec_name: "vp9",
              width: 1920,
              height: 1080,
              duration: "3.0",
            },
          ],
        },
        null,
        2,
      )}\n`,
    );
    writeFileSync(
      join(projectDir, "native-export-evidence.json"),
      `${JSON.stringify(
        {
          artifacts: [
            {
              profile: "mp4H264",
              report: "renders/export-h264/report.json",
              output: "renders/export-h264/output.mp4",
              probe: "renders/export-h264/ffprobe.json",
            },
          ],
        },
        null,
        2,
      )}\n`,
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/native-export-artifact-policy.mjs"),
        "--manifest",
        join(projectDir, "native-export-evidence.json"),
        "--project-dir",
        projectDir,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { status: string; failures: string[] };
    expect(report.status).toBe("failed");
    expect(report.failures).toEqual(
      expect.arrayContaining([
        "manifest.nativeExporterReport or legacy manifest.nativeRuntimeReport is required for native export evidence",
        "missing native export evidence for mp4H265",
        "missing native export evidence for proResMov",
        'mp4H264.probe video codec "vp9" does not match profile',
      ]),
    );
  });

  it("rejects native export evidence with missing retained report or probe files", () => {
    const projectDir = mkdtempSync(join(tmpdir(), "video-creater-native-export-missing-files-"));
    writeFileSync(
      join(projectDir, "native-runtime.json"),
      `${JSON.stringify(
        {
          ok: true,
          factories: [
            { name: "mp4mux", ok: true, status: "available" },
          ],
        },
        null,
        2,
      )}\n`,
    );
    const artifacts = [
      profileFixture("mp4H264", "export-h264", "output.mp4", "h264"),
      profileFixture("mp4H265", "export-h265", "output.mp4", "hevc"),
      profileFixture("proResMov", "export-prores", "output.mov", "prores"),
    ];
    for (const artifact of artifacts) {
      const renderDir = join(projectDir, "renders", artifact.jobId);
      mkdirSync(renderDir, { recursive: true });
      writeFileSync(
        join(projectDir, "renders", artifact.jobId, artifact.outputName),
        "native export fixture",
      );
    }
    writeFileSync(
      join(projectDir, "native-export-evidence.json"),
      `${JSON.stringify(
        {
          nativeRuntimeReport: "native-runtime.json",
          artifacts: artifacts.map(({ profile, jobId, outputName }) => ({
            profile,
            report: `renders/${jobId}/report.json`,
            output: `renders/${jobId}/${outputName}`,
            probe: `renders/${jobId}/ffprobe.json`,
          })),
        },
        null,
        2,
      )}\n`,
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/native-export-artifact-policy.mjs"),
        "--manifest",
        join(projectDir, "native-export-evidence.json"),
        "--project-dir",
        projectDir,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toEqual(
      expect.arrayContaining([
        "mp4H264.report must exist",
        "mp4H264.probe must exist",
        "mp4H265.report must exist",
        "mp4H265.probe must exist",
        "proResMov.report must exist",
        "proResMov.probe must exist",
      ]),
    );
  });

  it("rejects native runtime reports that are absolute or escape the project", () => {
    const projectDir = mkdtempSync(join(tmpdir(), "video-creater-native-export-runtime-path-"));
    const outsideDir = mkdtempSync(join(tmpdir(), "video-creater-native-runtime-outside-"));
    writeFileSync(
      join(outsideDir, "native-runtime.json"),
      `${JSON.stringify(
        {
          ok: true,
          factories: [{ name: "mp4mux", ok: true, status: "available" }],
        },
        null,
        2,
      )}\n`,
    );
    const artifacts = [
      profileFixture("mp4H264", "export-h264", "output.mp4", "h264"),
      profileFixture("mp4H265", "export-h265", "output.mp4", "hevc"),
      profileFixture("proResMov", "export-prores", "output.mov", "prores"),
    ];
    for (const artifact of artifacts) {
      writeNativeExportFixture(projectDir, artifact);
    }
    writeFileSync(
      join(projectDir, "native-export-evidence.json"),
      `${JSON.stringify(
        {
          nativeRuntimeReport: join(outsideDir, "native-runtime.json"),
          artifacts: artifacts.map(({ profile, jobId, outputName }) => ({
            profile,
            report: `renders/${jobId}/report.json`,
            output: `renders/${jobId}/${outputName}`,
            probe: `renders/${jobId}/ffprobe.json`,
          })),
        },
        null,
        2,
      )}\n`,
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/native-export-artifact-policy.mjs"),
        "--manifest",
        join(projectDir, "native-export-evidence.json"),
        "--project-dir",
        projectDir,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toContain(
      "manifest.nativeRuntimeReport must be a project-relative path",
    );
  });
});

function profileFixture(
  profile: "mp4H264" | "mp4H265" | "proResMov",
  jobId: string,
  outputName: string,
  codecName: string,
) {
  return { profile, jobId, outputName, codecName };
}

function writeNativeExportFixture(
  projectDir: string,
  artifact: ReturnType<typeof profileFixture>,
  backend?: string,
) {
  const renderDir = join(projectDir, "renders", artifact.jobId);
  const output = `renders/${artifact.jobId}/${artifact.outputName}`;
  mkdirSync(renderDir, { recursive: true });
  writeFileSync(join(projectDir, output), "native export fixture");
  writeFileSync(
    join(renderDir, "report.json"),
    `${JSON.stringify(
      {
        summary: { status: "completed", outputPath: output },
        ...(backend ? { command: { program: backend, args: [] } } : {}),
        streams: { video: true, audio: true },
        artifacts: [output],
        errors: [],
      },
      null,
      2,
    )}\n`,
  );
  writeFileSync(
    join(renderDir, "ffprobe.json"),
    `${JSON.stringify(
      {
        streams: [
          {
            codec_type: "video",
            codec_name: artifact.codecName,
            width: 1920,
            height: 1080,
            duration: "3.0",
          },
        ],
        format: { duration: "3.0" },
      },
      null,
      2,
    )}\n`,
  );
}
