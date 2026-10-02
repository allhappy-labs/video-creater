#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { performanceEnvironment, metricsDirectory, repoRoot, writePerformanceReport } from "./performance-metrics.mjs";

const lanes = {
  frontend: [["pnpm", "lint"], ["pnpm", "build"], ["pnpm", "build:bundle"]],
  "focused-frontend": [["node", "scripts/focused-development.mjs", "frontend", "src/lib/runtime/adapters/remote-transport.test.ts", "src/lib/runtime/backend-client.test.ts", "src/lib/timeline-preview.test.ts"]],
  "helpers-linux": [["node", "scripts/prepare-desktop-development.mjs", "--platform", "linux"]],
  "helpers-macos": [["node", "scripts/prepare-desktop-development.mjs", "--platform", "darwin"]],
};
let lane = "frontend"; let repeats = 2; let cacheLabel = "warm-existing";
for (let index = 0; index < process.argv.slice(2).length; index += 2) {
  const args = process.argv.slice(2); const value = args[index + 1];
  if (args[index] === "--lane") lane = value;
  else if (args[index] === "--repeats") repeats = Number(value);
  else if (args[index] === "--cache") cacheLabel = value;
  else throw new Error(`Unknown argument: ${args[index]}`);
}
if (!lanes[lane] || !Number.isInteger(repeats) || repeats < 1 || repeats > 10 || !["warm-existing", "cold-user-isolated"].includes(cacheLabel)) throw new Error("Invalid lane, repeats (1–10) or cache label (warm-existing|cold-user-isolated)");
if (lane === "helpers-macos" && process.platform !== "darwin" || lane === "helpers-linux" && process.platform !== "linux") throw new Error("Helper measurement must run on its supported platform");
const startedAt = new Date().toISOString();
const environment = performanceEnvironment(lane); environment.cache.classification = cacheLabel;
const steps = [];
for (let repeat = 1; repeat <= repeats; repeat++) {
  for (const command of lanes[lane]) {
    const start = performance.now();
    const result = spawnSync(command[0], command.slice(1), { cwd: repoRoot, stdio: "inherit" });
    steps.push({ command, repeat, elapsedMilliseconds: performance.now() - start, exitCode: result.status, status: !result.error && result.status === 0 ? "passed" : "failed", ...(result.error ? { launchErrorCode: result.error.code ?? "unknown" } : {}) });
    if (result.error || result.status !== 0) break;
  }
  if (steps.at(-1)?.status !== "passed") break;
}
const report = { schemaVersion: 1, lane, startedAt, completedAt: new Date().toISOString(), environment, status: steps.every((step) => step.status === "passed") ? "passed" : "failed", steps };
console.log(`iteration metrics: ${await writePerformanceReport(metricsDirectory, `iteration-${lane}`, report)}`);
if (report.status !== "passed") process.exitCode = 1;
