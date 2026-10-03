#!/usr/bin/env node

import { copyFileSync, mkdirSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { linuxNativeBuildEnvironment } from "./linux-native-build-env.mjs";

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
if (!target || target === "universal-apple-darwin") {
  throw new Error(`unsupported precompose sidecar target: ${target || "<missing>"}`);
}
const windows = target.includes("windows");
if (release && !["aarch64-apple-darwin", "x86_64-unknown-linux-gnu"].includes(target)) {
  throw new Error(`release packaging supports aarch64-apple-darwin and x86_64-unknown-linux-gnu, got ${target}`);
}
const executable = `video-creater-precompose-worker${windows ? ".exe" : ""}`;
const targetDir = resolve(repoRoot, process.env.CARGO_TARGET_DIR || "src-tauri/target");
const profile = release ? "release" : "debug";
const built = join(targetDir, target, profile, executable);
const bundled = resolve(
  repoRoot,
  "src-tauri/binaries",
  `video-creater-precompose-worker-${target}${windows ? ".exe" : ""}`,
);
const devSibling = join(targetDir, "debug", executable);
const cargoArgs = [
  "build",
  "--locked",
  "--manifest-path",
  "src-tauri/Cargo.toml",
  "-p",
  "video-creater-precompose-worker",
  "--target",
  target,
];
if (release) cargoArgs.push("--release");
if (process.env.VIDEO_CREATER_OFFLINE_BUILD === "1") cargoArgs.push("--offline");

if (dryRun) {
  console.log(JSON.stringify({ target, profile, cargoArgs, built, bundled, devSibling }, null, 2));
  process.exit(0);
}
const provenance = spawnSync("node", ["scripts/check-precompose-vendor-provenance.mjs"], {
  cwd: repoRoot,
  stdio: "inherit",
});
if (provenance.status !== 0) process.exit(provenance.status ?? 1);
const buildEnv = target.includes("linux") ? linuxNativeBuildEnvironment(process.env) : process.env;
const build = spawnSync("cargo", cargoArgs, { cwd: repoRoot, stdio: "inherit", env: buildEnv });
if (build.status !== 0) process.exit(build.status ?? 1);
mkdirSync(dirname(bundled), { recursive: true });
copyFileSync(built, bundled);
if (!release) {
  mkdirSync(dirname(devSibling), { recursive: true });
  copyFileSync(built, devSibling);
}
if (target === "aarch64-apple-darwin") {
  const verification = spawnSync(
    "node",
    [
      "scripts/verify-precompose-sidecar.mjs",
      "--target",
      target,
      "--path",
      bundled,
    ],
    { cwd: repoRoot, stdio: "inherit" },
  );
  if (verification.status !== 0) process.exit(verification.status ?? 1);
}
console.log(`prepared ${basename(bundled)}`);
