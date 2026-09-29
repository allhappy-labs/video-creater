#!/usr/bin/env node
// Builds the GStreamer compatibility decoder for Linux and stages it as the Tauri sidecar
// src-tauri/binaries/video-creater-compatibility-decoder-<target>. On Linux the worker loads
// the same license-filtered plugin set as the render runtime, so no separate runtime is staged.
//
// Usage: node scripts/build-linux-compatibility-decoder.mjs [--release] [--dry-run]

import { chmodSync, copyFileSync, mkdirSync, statSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const repoRoot = resolve(import.meta.dirname, "..");
const release = process.argv.includes("--release");
const dryRun = process.argv.includes("--dry-run");
const rustc = spawnSync("rustc", ["-vV"], { cwd: repoRoot, encoding: "utf8" });
if (rustc.status !== 0) {
  process.stderr.write(rustc.stderr);
  process.exit(rustc.status ?? 1);
}
const host = rustc.stdout.match(/^host:\s*(.+)$/m)?.[1];
const target = process.env.TAURI_ENV_TARGET_TRIPLE || host;
if (!target?.endsWith("-unknown-linux-gnu")) {
  throw new Error(`Linux compatibility decoder requires a *-unknown-linux-gnu target, got ${target || "<missing>"}`);
}
const executable = "video-creater-compatibility-decoder";
const targetDir = resolve(repoRoot, process.env.CARGO_TARGET_DIR || "src-tauri/target");
const profile = release ? "release" : "debug";
const built = join(targetDir, target, profile, executable);
const bundled = resolve(repoRoot, "src-tauri/binaries", `${executable}-${target}`);
const devSibling = join(targetDir, "debug", executable);
const cargoArgs = ["build", "--locked", "--manifest-path", "src-tauri/Cargo.toml", "-p", executable, "--target", target];
if (release) cargoArgs.push("--release");
if (process.env.VIDEO_CREATER_OFFLINE_BUILD === "1") cargoArgs.push("--offline");

if (dryRun) {
  console.log(JSON.stringify({ target, profile, cargoArgs, built, bundled }, null, 2));
  process.exit(0);
}
const build = spawnSync("cargo", cargoArgs, { cwd: repoRoot, stdio: "inherit" });
if (build.status !== 0) process.exit(build.status ?? 1);
for (const destination of release ? [bundled] : [bundled, devSibling]) {
  mkdirSync(dirname(destination), { recursive: true });
  copyFileSync(built, destination);
  chmodSync(destination, 0o755);
}
console.log(`prepared ${basename(bundled)} (${statSync(bundled).size} bytes)`);
