#!/usr/bin/env node

import { spawn } from "node:child_process";
import { rm } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  REQUIRED_FREE_BYTES,
  VERIFY_CACHE_LIMIT_BYTES,
  decideVerificationCacheAction,
  directorySizeBytes,
  filesystemBytes,
  validateCargoTargetPath,
} from "./cargo-cache-policy.mjs";
import { evaluateExactRustTest, parseFocusedArgs } from "./focused-development.mjs";
import { performanceEnvironment, writePerformanceReport } from "./performance-metrics.mjs";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const metricsPath = resolve(
  repoRoot,
  "output/build-metrics/native-verification-latest.json",
);

export function parseVerificationArgs(argv) {
  let lane = null;
  const filters = [];
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === "--lane") {
      lane = argv[index + 1];
      index += 1;
    } else if (argv[index] === "--") {
      continue;
    } else {
      filters.push(argv[index]);
    }
  }
  if (lane !== "fast" && lane !== "release") {
    throw new Error("--lane must be fast or release");
  }
  if (lane === "fast" && filters.length === 0) {
    throw new Error("fast verification requires at least one exact Rust test filter");
  }
  if (lane === "release" && filters.length > 0) {
    throw new Error("release verification does not accept Rust test filters");
  }
  if (lane === "fast") parseFocusedArgs(["native", ...filters]);
  return { lane, filters };
}

const manifest = ["--manifest-path", "src-tauri/Cargo.toml"];
const packagedFeatures =
  "app-runtime,custom-protocol,coreml-inspect,ges-render,gpu-render,graphics-render";

export function buildVerificationPlan({ lane, filters }) {
  const fmt = ["cargo", "fmt", ...manifest, "--all", "--", "--check"];
  if (lane === "fast") {
    return [
      fmt,
      ["cargo", "check", ...manifest, "--workspace"],
      ...filters.map((filter) => [
        "cargo", "test", ...manifest, "--lib", filter,
        "--", "--exact", "--test-threads=1",
      ]),
    ];
  }
  return [
    fmt,
    ["cargo", "clippy", ...manifest, "--workspace", "--all-targets", "--", "-D", "warnings"],
    [
      "cargo", "clippy", ...manifest, "--no-default-features", "--features",
      "mcp-server", "--bin", "video-creater-mcp-server", "--", "-D", "warnings",
    ],
    [
      "cargo", "clippy", ...manifest, "--no-default-features", "--features",
      packagedFeatures, "--bin", "video-creater", "--", "-D", "warnings",
    ],
    ["node", "scripts/run-native-rust-tests.mjs"],
  ];
}

export function verificationEnvironment(root, lane) {
  const environment = {
    CARGO_TARGET_DIR: resolve(root, "src-tauri/target/verify"),
    CARGO_INCREMENTAL: "0",
  };
  return lane === "fast"
    ? {
        ...environment,
        TAURI_CONFIG: '{"bundle":{"externalBin":[],"resources":[]}}',
      }
    : environment;
}

async function writeMetrics(report) {
  await writePerformanceReport(dirname(metricsPath), "native-verification", report);
}

async function removeVerificationTarget(targetPath) {
  const validated = await validateCargoTargetPath({
    repoRoot,
    targetPath,
    kind: "verification",
  });
  await rm(validated, { recursive: true, force: true });
}

export function runStep(command, env, cancellation) {
  const started = Date.now();
  return new Promise((resolveStep, rejectStep) => {
    const child = spawn(command[0], command.slice(1), {
      cwd: repoRoot,
      env: { ...process.env, ...env },
      stdio: ["inherit", "pipe", "pipe"],
    });
    let output = "";
    const retainOutput = (chunk, stream) => {
      stream.write(chunk);
      output = `${output}${chunk.toString()}`.slice(-1024 * 1024);
    };
    child.stdout.on("data", (chunk) => retainOutput(chunk, process.stdout));
    child.stderr.on("data", (chunk) => retainOutput(chunk, process.stderr));
    cancellation.activeChild = child;
    child.once("error", rejectStep);
    child.once("close", (exitCode, signal) => {
      cancellation.activeChild = null;
      resolveStep({
        command,
        status: cancellation.signal || signal
          ? "cancelled"
          : exitCode === 0 && (!command.includes("--exact") || evaluateExactRustTest(exitCode, output)) ? "passed" : "failed",
        elapsedMilliseconds: Date.now() - started,
        exitCode,
      });
    });
  });
}

export async function runNativeVerification(input) {
  const started = Date.now();
  const startedAt = new Date(started).toISOString();
  const targetDir = await validateCargoTargetPath({
    repoRoot,
    targetPath: resolve(repoRoot, "src-tauri/target/verify"),
    kind: "verification",
  });
  const developmentTarget = resolve(repoRoot, "src-tauri/target");
  const env = verificationEnvironment(repoRoot, input.lane);
  const commandPlan = buildVerificationPlan(input);
  const filesystemBefore = await filesystemBytes(repoRoot);
  const verificationBefore = await directorySizeBytes(targetDir);
  const developmentBefore = await directorySizeBytes(developmentTarget) -
    verificationBefore;
  const cleanupReasons = [];
  let cleanupRan = false;
  let status = "passed";
  const steps = [];
  const cancellation = { activeChild: null, signal: null };
  const forwardSignal = (signal) => {
    cancellation.signal = signal;
    cancellation.activeChild?.kill(signal);
  };
  const onSigint = () => forwardSignal("SIGINT");
  const onSigterm = () => forwardSignal("SIGTERM");
  process.once("SIGINT", onSigint);
  process.once("SIGTERM", onSigterm);

  try {
    if (decideVerificationCacheAction({
      verificationBytes: verificationBefore,
      availableBytes: filesystemBefore.availableBytes,
    }) === "clean") {
      if (verificationBefore > VERIFY_CACHE_LIMIT_BYTES) cleanupReasons.push("over_limit");
      if (filesystemBefore.availableBytes < REQUIRED_FREE_BYTES) {
        cleanupReasons.push("low_free_space");
      }
      await removeVerificationTarget(targetDir);
      cleanupRan = true;
    }
    const filesystemReady = await filesystemBytes(repoRoot);
    if (filesystemReady.availableBytes < REQUIRED_FREE_BYTES) {
      status = "blocked";
      throw new Error("Native verification requires at least 20 GiB free");
    }
    for (const command of commandPlan) {
      const step = await runStep(command, env, cancellation);
      steps.push(step);
      if (step.status !== "passed") {
        status = step.status;
        break;
      }
    }
  } catch (error) {
    if (status !== "blocked") status = cancellation.signal ? "cancelled" : "failed";
    console.error(error instanceof Error ? error.message : String(error));
  } finally {
    process.removeListener("SIGINT", onSigint);
    process.removeListener("SIGTERM", onSigterm);
    let verificationAfter = await directorySizeBytes(targetDir);
    if (verificationAfter > VERIFY_CACHE_LIMIT_BYTES) {
      cleanupReasons.push("post_run_over_limit");
      await removeVerificationTarget(targetDir);
      cleanupRan = true;
      verificationAfter = 0;
    }
    const filesystemAfter = await filesystemBytes(repoRoot);
    const report = {
      schemaVersion: 1,
      lane: input.lane,
      command: ["node", "scripts/run-native-verification.mjs", "--lane", input.lane],
      environment: performanceEnvironment(input.lane === "release" ? "bounded-release" : "bounded-default-focused"),
      status,
      startedAt,
      completedAt: new Date().toISOString(),
      elapsedMilliseconds: Date.now() - started,
      cache: {
        classification: verificationBefore === 0 ? "cold" : "warm",
        cleanupRan,
        cleanupTarget: cleanupRan ? targetDir : null,
        cleanupReasons,
      },
      filesystem: {
        capacityBytesBefore: filesystemBefore.capacityBytes,
        availableBytesBefore: filesystemBefore.availableBytes,
        capacityBytesAfter: filesystemAfter.capacityBytes,
        availableBytesAfter: filesystemAfter.availableBytes,
      },
      targets: {
        developmentBytesBefore: developmentBefore,
        developmentBytesAfter:
          await directorySizeBytes(developmentTarget) - verificationAfter,
        verificationBytesBefore: verificationBefore,
        verificationBytesAfter: verificationAfter,
      },
      cargo: {
        targetDir,
        incremental: "0",
        featureProfiles: input.lane === "release"
          ? ["default-all-targets", "mcp-only", "packaged-custom-protocol"]
          : ["default-focused"],
      },
      steps,
    };
    await writeMetrics(report);
  }
  return status === "passed";
}

async function main() {
  const input = parseVerificationArgs(process.argv.slice(2));
  if (!await runNativeVerification(input)) process.exitCode = 1;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
