#!/usr/bin/env node
// Builds the Linux DeepFilterNet3 speech enhancement helper (Rust libDF + tract, embedded
// DeepFilterNet3 model) and stages it as the Tauri sidecar
// src-tauri/binaries/video-creater-audio-enhance-<target>.
//
// The helper is always compiled with the optimized release profile: unoptimized tract
// inference is far slower than real time. Without --release the binary is additionally copied
// next to development executables (src-tauri/target/debug).

import { copyFileSync, mkdirSync, statSync } from "node:fs";
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
if (target !== "x86_64-unknown-linux-gnu") {
  throw new Error(`Linux DeepFilterNet3 helper supports x86_64-unknown-linux-gnu, got ${target || "<missing>"}`);
}
const executable = "video-creater-audio-enhance";
const targetDir = resolve(repoRoot, process.env.CARGO_TARGET_DIR || "src-tauri/target");
const profile = "release";
const built = join(targetDir, target, profile, executable);
const bundled = resolve(repoRoot, "src-tauri/binaries", `${executable}-${target}`);
const devSibling = join(targetDir, "debug", executable);
const cargoArgs = [
  "build",
  "--locked",
  "--manifest-path",
  "src-tauri/Cargo.toml",
  "-p",
  executable,
  "--target",
  target,
  "--release",
];
if (process.env.VIDEO_CREATER_OFFLINE_BUILD === "1") cargoArgs.push("--offline");

if (dryRun) {
  console.log(JSON.stringify({ target, profile, cargoArgs, built, bundled, devSibling }, null, 2));
  process.exit(0);
}
const build = spawnSync("cargo", cargoArgs, { cwd: repoRoot, stdio: "inherit" });
if (build.status !== 0) process.exit(build.status ?? 1);
mkdirSync(dirname(bundled), { recursive: true });
copyFileSync(built, bundled);
if (!release) {
  mkdirSync(dirname(devSibling), { recursive: true });
  copyFileSync(built, devSibling);
}
console.log(`prepared ${basename(bundled)} (${statSync(bundled).size} bytes)`);
