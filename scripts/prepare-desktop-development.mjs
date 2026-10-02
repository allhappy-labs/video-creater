#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";
import { metricsDirectory, performanceEnvironment, repoRoot, writePerformanceReport } from "./performance-metrics.mjs";

export function helperPreparationPlan(platform) {
  const scripts = platform === "linux" ? [
    ["build:linux-media-runtime", "--package", "src-tauri/resources/render-runtime-package"],
    ["build:compatibility-decoder:linux:dev"], ["build:precompose-sidecar:dev"],
    ["build:linux-audio-enhancer:dev"], ["build:linux-semantic-encoder"],
    ["build:linux-speech-worker:dev"], ["build:codex-sidecar:dev"],
  ] : platform === "darwin" ? [
    ["build:gstreamer-runtime:dev"], ["build:compatibility-decoder:dev"],
    ["build:precompose-sidecar:dev"], ["build:avfoundation-exporter:dev"],
    ["build:audio-enhancer:dev"], ["build:semantic-encoder"],
    ["build:fluidaudio-helper:dev"], ["build:codex-sidecar:dev"],
  ] : null;
  if (!scripts) throw new Error(`desktop development is not supported on ${platform}`);
  return [...scripts.map((script) => ["pnpm", ...script]), ["node", "scripts/build-macos-release.mjs", "--prepare-mcp-sidecar", "--development"]];
}

export function preparationOutcome(commands, steps) {
  return {
    status: steps.length === commands.length && steps.every((step) => step.status === "passed") ? "passed" : "failed",
    skippedStages: commands.slice(steps.length).map((command) => ({ command, status: "skipped", reason: "earlier-stage-failed" })),
  };
}

export async function prepareDesktopDevelopment(platform) {
  const startedAt = new Date().toISOString();
  const environment = performanceEnvironment(`${platform}-desktop-helpers-development`);
  const steps = [];
  const commands = helperPreparationPlan(platform);
  for (const command of commands) {
    const start = performance.now();
    const result = spawnSync(command[0], command.slice(1), { cwd: repoRoot, stdio: "inherit" });
    steps.push({ command, elapsedMilliseconds: performance.now() - start, exitCode: result.status, status: !result.error && result.status === 0 ? "passed" : "failed", ...(result.error ? { launchErrorCode: result.error.code ?? "unknown" } : {}) });
    if (result.error || result.status !== 0) break;
  }
  const report = { schemaVersion: 1, startedAt, completedAt: new Date().toISOString(), environment, ...preparationOutcome(commands, steps), steps };
  const reportPath = await writePerformanceReport(metricsDirectory, "desktop-helper-preparation", report);
  console.log(`helper preparation metrics: ${reportPath}`);
  return report;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const args = process.argv.slice(2);
  if (args.length !== 2 || args[0] !== "--platform") throw new Error("Usage: prepare-desktop-development.mjs --platform linux|darwin");
  if ((await prepareDesktopDevelopment(args[1])).status !== "passed") process.exitCode = 1;
}
