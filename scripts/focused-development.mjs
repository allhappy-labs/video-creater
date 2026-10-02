#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { statSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parseCargoTestSummary } from "./run-native-rust-tests.mjs";
import { REQUIRED_FREE_BYTES, filesystemBytes, validateCargoTargetPath } from "./cargo-cache-policy.mjs";
import { metricsDirectory, performanceEnvironment, repoRoot, writePerformanceReport } from "./performance-metrics.mjs";

export function parseFocusedArgs(argv) {
  const [lane, ...selectors] = argv.filter((arg) => arg !== "--");
  if (lane !== "frontend" && lane !== "native") throw new Error("lane must be frontend or native");
  if (!selectors.length) throw new Error("focused checks require at least one exact selector");
  for (const selector of selectors) {
    if (lane === "native" && !/^[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)+$/.test(selector)) throw new Error("native selector must be a fully qualified exact Rust test name");
    if (lane === "frontend" && (!/^src\/.*\.(test|spec)\.(ts|tsx)$/.test(selector) || selector.includes("..") || !statSync(resolve(repoRoot, selector), { throwIfNoEntry: false })?.isFile())) throw new Error("frontend selector must be an existing src test file");
  }
  return { lane, selectors };
}

export function evaluateExactRustTest(exitCode, output) {
  const summary = parseCargoTestSummary(output);
  return exitCode === 0 && summary.passed === 1 && summary.failed === 0 && summary.ignored === 0;
}

export async function runFocusedDevelopment({ lane, selectors }) {
  const startedAt = new Date().toISOString();
  const environment = performanceEnvironment(lane === "native" ? "default-lib-incremental" : "frontend-types-selected-units");
  let env = { ...process.env };
  if (lane === "native") {
    const targetDir = await validateCargoTargetPath({ repoRoot, targetPath: resolve(repoRoot, "src-tauri/target"), kind: "development" });
    if ((await filesystemBytes(repoRoot)).availableBytes < REQUIRED_FREE_BYTES) throw new Error("Focused native development requires at least 20 GiB free; no cache is automatically removed");
    env = { ...env, CARGO_TARGET_DIR: targetDir, CARGO_INCREMENTAL: "1", TAURI_CONFIG: '{"bundle":{"externalBin":[],"resources":[]}}' };
  }
  const commands = lane === "frontend"
    ? [["pnpm", "lint"], ["pnpm", "exec", "vitest", "run", ...selectors, "--passWithNoTests=false"]]
    : selectors.map((selector) => ["cargo", "test", "--manifest-path", "src-tauri/Cargo.toml", "--lib", selector, "--", "--exact", "--test-threads=1"]);
  const steps = [];
  for (const command of commands) {
    const start = performance.now();
    const result = spawnSync(command[0], command.slice(1), { cwd: repoRoot, env, encoding: "utf8", maxBuffer: 32 * 1024 ** 2 });
    process.stdout.write(result.stdout ?? ""); process.stderr.write(result.stderr ?? "");
    const passed = !result.error && (lane === "native" ? evaluateExactRustTest(result.status, `${result.stdout}\n${result.stderr}`) : result.status === 0);
    steps.push({ command, elapsedMilliseconds: performance.now() - start, exitCode: result.status, status: passed ? "passed" : "failed", ...(result.error ? { launchErrorCode: result.error.code ?? "unknown" } : {}), ...(lane === "native" ? { summary: parseCargoTestSummary(`${result.stdout}\n${result.stderr}`) } : {}) });
    if (!passed) break;
  }
  const report = { schemaVersion: 1, lane, startedAt, completedAt: new Date().toISOString(), environment, status: steps.every((step) => step.status === "passed") ? "passed" : "failed", steps };
  await writePerformanceReport(metricsDirectory, `focused-${lane}`, report);
  return report;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const report = await runFocusedDevelopment(parseFocusedArgs(process.argv.slice(2)));
  if (report.status !== "passed") process.exitCode = 1;
}
