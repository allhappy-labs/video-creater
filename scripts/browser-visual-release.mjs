#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const browserVisualThreshold = 0.01;
export const browserVisualChannelThreshold = 4;

export function releaseBaselineFor(platform, arch) {
  if (platform === "linux" && arch === "x64") {
    return {
      baseline: "docs/visual-qa/browser-visual-baseline-linux-x64",
      manifest: "docs/visual-qa/browser-visual-baseline-linux-x64-manifest.json",
    };
  }
  return {
    baseline: "docs/visual-qa/browser-visual-baseline",
    manifest: "docs/visual-qa/browser-visual-baseline-manifest.json",
  };
}

function run(script, args) {
  const result = spawnSync(process.execPath, [resolve(script), ...args], {
    cwd: process.cwd(),
    stdio: "inherit",
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function main() {
  const selection = releaseBaselineFor(process.platform, process.arch);
  const comparison = "output/playwright/browser-visual-qa/browser-visual-baseline-comparison.json";
  run("scripts/browser-visual-qa.mjs", [
    "--baseline", selection.baseline,
    "--threshold", String(browserVisualThreshold),
    "--channel-threshold", String(browserVisualChannelThreshold),
    "--comparison-out", comparison,
    "--fail-on-mismatch",
  ]);
  run("scripts/browser-visual-baseline-policy.mjs", [
    "--baseline", selection.baseline,
    "--manifest", selection.manifest,
    "--comparison", comparison,
    "--platform", `${process.platform}-${process.arch}`,
    "--threshold", String(browserVisualThreshold),
    "--channel-threshold", String(browserVisualChannelThreshold),
    "--require-comparison",
  ]);
}

const isDirectRun =
  process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isDirectRun) main();
