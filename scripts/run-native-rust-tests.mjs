#!/usr/bin/env node

import { spawn } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath, pathToFileURL } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const defaultReportPath = path.join(
  repoRoot,
  "output/settings-readiness/native-rust-tests/report.json",
);

export const REQUIRED_NATIVE_LANES = Object.freeze([
  {
    id: "metal",
    testName:
      "frame_compositor::gpu::tests::gpu_matches_canonical_cpu_blends_with_documented_tolerance",
    cargoArgs: ["--lib"],
    testArgs: [],
  },
  {
    id: "afconvert",
    testName:
      "precompose::audio_denoise::tests::system_afconvert_decodes_the_retained_mp4_speech_fixture",
    cargoArgs: ["--lib"],
    testArgs: [],
  },
  {
    id: "bundled-filmstrip",
    testName:
      "timeline_filmstrip::tests::bundled_decoder_extracts_ordered_distinct_frames_from_sample_media",
    cargoArgs: ["--lib"],
    testArgs: [],
  },
  {
    id: "keychain",
    testName:
      "provider_credentials::tests::macos_keychain_replaces_and_deletes_an_isolated_account",
    cargoArgs: ["--lib"],
    testArgs: ["--ignored"],
  },
  {
    id: "notifications",
    testName: "tests::notification_capability_invokes_native_probe_for_bundled_executable",
    cargoArgs: ["--bin", "video-creater"],
    testArgs: [],
  },
  {
    id: "media-inspection-appkit",
    cargoArgs: ["--test", "media_inspection_appkit"],
    harnessMarker: "media_inspection_appkit: passed",
  },
  {
    id: "precompose-alpha-appkit",
    cargoArgs: ["--test", "precompose_alpha_ges"],
    harnessMarker: "precompose_alpha_ges: passed",
  },
  {
    id: "nested-export-appkit",
    cargoArgs: ["--test", "project_export_nested_effect_appkit"],
    harnessMarker: "project_export_nested_effect_appkit: passed",
  },
  {
    id: "prores-export-appkit",
    cargoArgs: ["--test", "project_export_prores_appkit"],
    harnessMarker: "project_export_prores_appkit: passed",
  },
]);

export function parseCargoTestSummary(output) {
  const summary = { passed: 0, failed: 0, ignored: 0, filteredOut: 0 };
  const pattern =
    /test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; (\d+) filtered out/g;
  for (const match of output.matchAll(pattern)) {
    summary.passed += Number(match[1]);
    summary.failed += Number(match[2]);
    summary.ignored += Number(match[3]);
    summary.filteredOut += Number(match[4]);
  }
  return summary;
}

export function evaluateNativeRustTestReport(report) {
  const failures = [];
  if (report?.status !== "passed") failures.push("report status must be passed");
  for (const key of [
    "platform",
    "arch",
    "osRelease",
    "rustc",
    "xcode",
    "cargoTargetDir",
    "cargoIncremental",
  ]) {
    if (typeof report?.environment?.[key] !== "string" || !report.environment[key].trim()) {
      failures.push(`environment.${key} must be recorded`);
    }
  }
  if (report?.environment?.cargoIncremental !== "0") {
    failures.push("environment.cargoIncremental must disable incremental compilation");
  }
  if (
    typeof report?.environment?.cargoTargetDir === "string" &&
    report.environment.cargoTargetDir.trim() !== "" &&
    !report.environment.cargoTargetDir.endsWith("/src-tauri/target/verify")
  ) {
    failures.push("environment.cargoTargetDir must use the verification target");
  }
  if (report?.environment?.platform !== "darwin") {
    failures.push("native Rust verification must run on macOS");
  }
  if (report?.fullSuite?.status !== "passed" || report.fullSuite.exitCode !== 0) {
    failures.push("fullSuite did not pass");
  }
  if (!report?.fullSuite?.command?.includes("--test-threads=1")) {
    failures.push("fullSuite must execute with --test-threads=1");
  }
  if ((report?.fullSuite?.failed ?? 0) > 0) {
    failures.push(`fullSuite contains ${report.fullSuite.failed} failed test(s)`);
  }
  if (report?.fullSuite?.runtimeEnvironment?.GST_GL_WINDOW !== "dummy") {
    failures.push("fullSuite must use the headless GStreamer GL window backend");
  }
  if (report?.fullSuite?.runtimeEnvironment?.VIDEO_CREATER_HEADLESS_RUST_SUITE !== "1") {
    failures.push("fullSuite must skip custom AppKit harness execution");
  }
  if ((report?.fullSuite?.gstreamerAppKitWarnings ?? 0) > 0) {
    failures.push(
      `fullSuite emitted ${report.fullSuite.gstreamerAppKitWarnings} GStreamer AppKit warning(s)`,
    );
  }

  const lanes = new Map(
    Array.isArray(report?.requiredLanes)
      ? report.requiredLanes.map((lane) => [lane.id, lane])
      : [],
  );
  for (const required of REQUIRED_NATIVE_LANES) {
    const lane = lanes.get(required.id);
    if (!lane) {
      failures.push(`required native lane ${required.id} is missing`);
      continue;
    }
    if (lane.status !== "passed" || lane.exitCode !== 0) {
      failures.push(`required native lane ${required.id} did not pass`);
    }
    if (lane.passed !== 1 || lane.failed !== 0 || lane.ignored !== 0) {
      failures.push(
        `required native lane ${required.id} did not execute exactly one passing test`,
      );
    }
  }
  return failures;
}

function parseArgs(argv) {
  let reportPath = defaultReportPath;
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--report") {
      const value = argv[index + 1];
      if (!value) throw new Error("--report requires a path");
      reportPath = path.resolve(repoRoot, value);
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${arg}`);
  }
  return { reportPath };
}

async function runCommand(
  program,
  args,
  { cwd = repoRoot, env = {}, logPath, live = true } = {},
) {
  const command = [program, ...args];
  const child = spawn(program, args, {
    cwd,
    env: { ...process.env, ...env },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => {
    const value = chunk.toString();
    stdout += value;
    if (live) process.stdout.write(value);
  });
  child.stderr.on("data", (chunk) => {
    const value = chunk.toString();
    stderr += value;
    if (live) process.stderr.write(value);
  });
  const exitCode = await new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("close", (code) => resolve(code ?? 1));
  });
  if (logPath) {
    await mkdir(path.dirname(logPath), { recursive: true });
    await writeFile(
      logPath,
      [`$ ${command.join(" ")}`, stdout, stderr].filter(Boolean).join("\n"),
    );
  }
  return { command, exitCode, stdout, stderr };
}

async function commandVersion(program, args) {
  const result = await runCommand(program, args, { live: false });
  return result.exitCode === 0 ? `${result.stdout}${result.stderr}`.trim() : "unavailable";
}

function resultRecord(result, logPath, { successMarker } = {}) {
  const summary = parseCargoTestSummary(`${result.stdout}\n${result.stderr}`);
  if (
    successMarker &&
    result.exitCode === 0 &&
    `${result.stdout}\n${result.stderr}`.includes(successMarker)
  ) {
    summary.passed = 1;
  }
  return {
    status:
      result.exitCode === 0 && summary.failed === 0 && summary.passed > 0
        ? "passed"
        : summary.ignored > 0 && summary.passed === 0
          ? "skipped"
          : "failed",
    command: result.command,
    exitCode: result.exitCode,
    ...summary,
    logPath: path.relative(repoRoot, logPath),
  };
}

export async function runNativeRustTests({ reportPath = defaultReportPath } = {}) {
  const outputDir = path.dirname(reportPath);
  await mkdir(outputDir, { recursive: true });
  const environment = {
    platform: process.platform,
    arch: process.arch,
    osRelease: os.release(),
    rustc: await commandVersion("rustc", ["--version"]),
    xcode: await commandVersion("xcodebuild", ["-version"]),
    cargoTargetDir: process.env.CARGO_TARGET_DIR ?? "",
    cargoIncremental: process.env.CARGO_INCREMENTAL ?? "",
  };
  const startedAt = new Date().toISOString();

  const fullLogPath = path.join(outputDir, "full-suite.log");
  const fullResult = await runCommand(
    "cargo",
    [
      "test",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--workspace",
      "--all-targets",
      "--",
      "--test-threads=1",
    ],
    {
      env: {
        GST_GL_WINDOW: "dummy",
        VIDEO_CREATER_HEADLESS_RUST_SUITE: "1",
      },
      logPath: fullLogPath,
    },
  );
  const fullSuite = {
    ...resultRecord(fullResult, fullLogPath),
    runtimeEnvironment: {
      GST_GL_WINDOW: "dummy",
      VIDEO_CREATER_HEADLESS_RUST_SUITE: "1",
    },
    gstreamerAppKitWarnings: (
      `${fullResult.stdout}\n${fullResult.stderr}`.match(/GStreamer-GL-WARNING/g) ?? []
    ).length,
  };

  const requiredLanes = [];
  for (const lane of REQUIRED_NATIVE_LANES) {
    const logPath = path.join(outputDir, `${lane.id}.log`);
    const cargoArgs = [
      "test",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      ...lane.cargoArgs,
    ];
    if (lane.testName) {
      cargoArgs.push(
        lane.testName,
        "--",
        ...(lane.testArgs ?? []),
        "--exact",
        "--nocapture",
        "--test-threads=1",
      );
    }
    const result = await runCommand("cargo", cargoArgs, { logPath });
    requiredLanes.push({
      id: lane.id,
      testName: lane.testName ?? lane.harnessMarker,
      ...resultRecord(result, logPath, { successMarker: lane.harnessMarker }),
    });
  }

  const report = {
    schemaVersion: 1,
    status: "passed",
    startedAt,
    completedAt: new Date().toISOString(),
    environment,
    fullSuite,
    requiredLanes,
  };
  if (evaluateNativeRustTestReport(report).length > 0) report.status = "failed";
  report.failures = evaluateNativeRustTestReport(report);
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  return report;
}

async function main() {
  const { reportPath } = parseArgs(process.argv.slice(2));
  const report = await runNativeRustTests({ reportPath });
  if (report.failures.length > 0) {
    for (const failure of report.failures) console.error(`native Rust verification: ${failure}`);
    console.error(`native Rust report: ${path.relative(repoRoot, reportPath)}`);
    process.exitCode = 1;
    return;
  }
  console.log(`native Rust verification passed: ${path.relative(repoRoot, reportPath)}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
