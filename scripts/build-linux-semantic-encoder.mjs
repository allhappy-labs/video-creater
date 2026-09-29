#!/usr/bin/env node
// Builds the Linux SigLIP 2 semantic encoder helper and stages it as the Tauri sidecar
// src-tauri/binaries/video-creater-semantic-encoder-<target>.
//
// ONNX Runtime 1.28.0 is linked statically: ort-sys `download-binaries` fetches the pinned
// x86_64-unknown-linux-gnu archive listed in its dist.tsv and verifies its SHA-256 before
// linking, so no libonnxruntime.so is shipped next to the helper. Offline builds can point
// ORT_LIB_LOCATION at a reviewed ONNX Runtime build instead.

import { copyFileSync, mkdirSync, statSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const repoRoot = resolve(import.meta.dirname, "..");
const release = !process.argv.includes("--development");
const dryRun = process.argv.includes("--dry-run");
const rustc = spawnSync("rustc", ["-vV"], { cwd: repoRoot, encoding: "utf8" });
if (rustc.status !== 0) {
  process.stderr.write(rustc.stderr);
  process.exit(rustc.status ?? 1);
}
const host = rustc.stdout.match(/^host:\s*(.+)$/m)?.[1];
const target = process.env.TAURI_ENV_TARGET_TRIPLE || host;
if (target !== "x86_64-unknown-linux-gnu") {
  throw new Error(`Linux SigLIP 2 ONNX helper supports x86_64-unknown-linux-gnu, got ${target || "<missing>"}`);
}
const executable = "video-creater-semantic-encoder";
const targetDir = resolve(repoRoot, process.env.CARGO_TARGET_DIR || "src-tauri/target");
const profile = release ? "release" : "debug";
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
];
if (release) cargoArgs.push("--release");
if (process.env.VIDEO_CREATER_OFFLINE_BUILD === "1") cargoArgs.push("--offline");

if (dryRun) {
  console.log(JSON.stringify({ target, profile, cargoArgs, built, bundled, devSibling }, null, 2));
  process.exit(0);
}
const build = spawnSync("cargo", cargoArgs, { cwd: repoRoot, stdio: "inherit" });
if (build.status !== 0) process.exit(build.status ?? 1);
const linked = spawnSync("ldd", [built], { cwd: repoRoot, encoding: "utf8" });
if (linked.status === 0 && /libonnxruntime/.test(linked.stdout)) {
  throw new Error("semantic encoder must link ONNX Runtime statically; found a libonnxruntime dependency");
}
mkdirSync(dirname(bundled), { recursive: true });
copyFileSync(built, bundled);
mkdirSync(dirname(devSibling), { recursive: true });
copyFileSync(built, devSibling);
console.log(`prepared ${basename(bundled)} (${statSync(bundled).size} bytes)`);
