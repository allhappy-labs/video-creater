#!/usr/bin/env node

import { copyFileSync, mkdirSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const repoRoot = resolve(import.meta.dirname, "..");
const packagePath = resolve(repoRoot, "src-tauri/native/avfoundation-export");
const release = process.argv.includes("--release");
const dryRun = process.argv.includes("--dry-run");
const rustc = spawnSync("rustc", ["-vV"], { cwd: repoRoot, encoding: "utf8" });
if (rustc.status !== 0) {
  process.stderr.write(rustc.stderr);
  process.exit(rustc.status ?? 1);
}
const host = rustc.stdout.match(/^host:\s*(.+)$/m)?.[1];
const target = process.env.TAURI_ENV_TARGET_TRIPLE || host;
if (!target?.endsWith("-apple-darwin") || target === "universal-apple-darwin") {
  throw new Error(`AVFoundation exporter supports explicit macOS targets only, got ${target || "<missing>"}`);
}
if (release && target !== "aarch64-apple-darwin") {
  throw new Error(`initial signed release packaging supports only aarch64-apple-darwin, got ${target}`);
}
const architecture = target.startsWith("aarch64-") ? "arm64" : "x86_64";
const executable = "video-creater-avfoundation-exporter";
const configuration = release ? "release" : "debug";
const buildArgs = [
  "build",
  "--package-path",
  packagePath,
  "--configuration",
  configuration,
  "--arch",
  architecture,
];
const binPathArgs = [
  "build",
  "--show-bin-path",
  "--package-path",
  packagePath,
  "--configuration",
  configuration,
  "--arch",
  architecture,
];
const bundled = resolve(
  repoRoot,
  "src-tauri/binaries",
  `${executable}-${target}`,
);
const devSibling = resolve(repoRoot, "src-tauri/target/debug", executable);

if (dryRun) {
  console.log(JSON.stringify({ target, architecture, configuration, buildArgs, bundled, devSibling }, null, 2));
  process.exit(0);
}
const build = spawnSync("swift", buildArgs, { cwd: repoRoot, stdio: "inherit" });
if (build.status !== 0) process.exit(build.status ?? 1);
const binPath = spawnSync("swift", binPathArgs, { cwd: repoRoot, encoding: "utf8" });
if (binPath.status !== 0) {
  process.stderr.write(binPath.stderr);
  process.exit(binPath.status ?? 1);
}
const built = join(binPath.stdout.trim(), executable);
mkdirSync(dirname(bundled), { recursive: true });
copyFileSync(built, bundled);
if (!release) {
  mkdirSync(dirname(devSibling), { recursive: true });
  copyFileSync(built, devSibling);
}
const verification = spawnSync(
  "node",
  [
    "scripts/verify-avfoundation-exporter.mjs",
    "--target",
    target,
    "--path",
    bundled,
  ],
  { cwd: repoRoot, stdio: "inherit" },
);
if (verification.status !== 0) process.exit(verification.status ?? 1);
console.log(`prepared ${basename(bundled)}`);
