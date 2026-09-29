#!/usr/bin/env node

import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const repoRoot = resolve(import.meta.dirname, "..");
const packagePath = resolve(repoRoot, "src-tauri/native/audio-enhance");
const release = process.argv.includes("--release");
const rustc = spawnSync("rustc", ["-vV"], { cwd: repoRoot, encoding: "utf8" });
if (rustc.status !== 0) process.exit(rustc.status ?? 1);
const target = process.env.TAURI_ENV_TARGET_TRIPLE || rustc.stdout.match(/^host:\s*(.+)$/m)?.[1];
if (target !== "aarch64-apple-darwin") {
  throw new Error(`DeepFilterNet3 Core ML helper supports aarch64-apple-darwin, got ${target}`);
}
const configuration = release ? "release" : "debug";
const args = ["build", "--package-path", packagePath, "--configuration", configuration, "--arch", "arm64"];
const build = spawnSync("swift", args, { cwd: repoRoot, stdio: "inherit" });
if (build.status !== 0) process.exit(build.status ?? 1);
const binPath = spawnSync("swift", [...args.slice(0, 1), "--show-bin-path", ...args.slice(1)], {
  cwd: repoRoot,
  encoding: "utf8",
});
if (binPath.status !== 0) process.exit(binPath.status ?? 1);
const executable = "video-creater-audio-enhance";
const built = join(binPath.stdout.trim(), executable);
const bundled = resolve(repoRoot, "src-tauri/binaries", `${executable}-${target}`);
mkdirSync(dirname(bundled), { recursive: true });
copyFileSync(built, bundled);
if (!release) {
  const devSibling = resolve(repoRoot, "src-tauri/target/debug", executable);
  mkdirSync(dirname(devSibling), { recursive: true });
  copyFileSync(built, devSibling);
}
console.log(`prepared ${bundled}`);
