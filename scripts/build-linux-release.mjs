#!/usr/bin/env node
// Builds and audits the Linux (Ubuntu 24.04 x86_64) .deb release.
//
// Usage: node scripts/build-linux-release.mjs [--preflight] [--skip-build]
//
// The audit inspects the produced package rather than the source tree: required payload
// files, sidecar executables, and the ELF dependency closure of every shipped binary, which
// must not reference GPL media libraries or distribution FFmpeg builds.

import { createHash } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const repoRoot = resolve(import.meta.dirname, "..");
export const RELEASE_FEATURES = "app-runtime,custom-protocol,ges-render,gpu-render,graphics-render";
export const RESOURCE_ROOT = "usr/lib/Video Creater";
export const REQUIRED_PAYLOAD = [
  "usr/bin/video-creater",
  "usr/bin/video-creater-precompose-worker",
  "usr/bin/video-creater-audio-enhance",
  "usr/bin/video-creater-semantic-encoder",
  "usr/bin/video-creater-speech",
  "usr/bin/video-creater-compatibility-decoder",
  "usr/bin/video-creater-codex",
  "usr/bin/video-creater-mcp-server",
  `${RESOURCE_ROOT}/render-runtime/manifest.json`,
  `${RESOURCE_ROOT}/render-runtime/bundled-plugins/libgstlibav.so`,
  `${RESOURCE_ROOT}/speech-runtime/lib/libonnxruntime.so`,
  `${RESOURCE_ROOT}/speech-runtime/lib/libsherpa-onnx-c-api.so`,
  `${RESOURCE_ROOT}/codex-runtime/codex-path/rg`,
  `${RESOURCE_ROOT}/sample-project`,
];
// Shared-object names whose presence in a shipped binary's dependency closure means a GPL
// component or an unreviewed distribution FFmpeg build would be loaded.
export const DENIED_LIBRARY_PATTERNS = [
  /^libavcodec\.so/,
  /^libavformat\.so/,
  /^libavutil\.so/,
  /^libavfilter\.so/,
  /^libswscale\.so/,
  /^libswresample\.so/,
  /^libpostproc\.so/,
  /^libx264\.so/,
  /^libx265\.so/,
  /^libfaad\.so/,
  /^libmpeg2/,
  /^libdvdread\.so/,
  /^libdvdnav\.so/,
  /^libespeak/,
  /^libreadline\.so/,
];

export function deniedLibraries(neededNames) {
  return neededNames.filter((name) => DENIED_LIBRARY_PATTERNS.some((pattern) => pattern.test(name)));
}

export function missingPayload(files) {
  const present = new Set(files.map((file) => file.replace(/^\.\//, "").replace(/\/$/, "")));
  return REQUIRED_PAYLOAD.filter((path) => !present.has(path));
}

function fail(message) {
  console.error(`linux release: ${message}`);
  process.exit(1);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: repoRoot, stdio: "inherit", ...options });
  if (result.error) fail(`${command} could not start: ${result.error.message}`);
  if (result.status !== 0) fail(`${command} ${args.join(" ")} exited with ${result.status}`);
}

function capture(command, args) {
  return execFileSync(command, args, { cwd: repoRoot, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
}

function preflight() {
  if (process.platform !== "linux" || process.arch !== "x64") fail("Linux releases are built on x86_64 Linux");
  const missing = [];
  for (const tool of ["cargo", "rustc", "pnpm", "dpkg-deb", "readelf", "cmake", "meson", "ninja", "pkg-config", "curl"]) {
    if (spawnSync("sh", ["-c", `command -v ${tool}`]).status !== 0) missing.push(tool);
  }
  for (const module of ["gstreamer-1.0", "gst-editing-services-1.0", "gstreamer-plugins-base-1.0", "webkit2gtk-4.1"]) {
    if (spawnSync("pkg-config", ["--exists", module]).status !== 0) missing.push(`pkg-config:${module}`);
  }
  if (missing.length > 0) fail(`missing build prerequisites: ${missing.join(", ")}`);
  return { status: "ready" };
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function listElfFiles(root) {
  const files = [];
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) visit(path);
      else if (entry.isFile() && statSync(path).size > 4) {
        const header = readFileSync(path).subarray(0, 4);
        if (header.equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46]))) files.push(path);
      }
    }
  };
  visit(root);
  return files;
}

function neededLibraries(path) {
  const output = capture("readelf", ["-d", path]);
  return [...output.matchAll(/\(NEEDED\)\s+Shared library: \[([^\]]+)\]/g)].map((match) => match[1]);
}

function auditPackage(debPath) {
  const listing = capture("dpkg-deb", ["-c", debPath])
    .split("\n")
    .filter(Boolean)
    .map((line) => line.split(/\s+/).slice(5).join(" ").split(" -> ")[0]);
  const missing = missingPayload(listing);
  const extracted = mkdtempSync(join(tmpdir(), "video-creater-deb-"));
  try {
    run("dpkg-deb", ["-x", debPath, extracted]);
    const elfFindings = [];
    for (const file of listElfFiles(extracted)) {
      const denied = deniedLibraries(neededLibraries(file));
      if (denied.length > 0) elfFindings.push({ file: relative(extracted, file), denied });
    }
    const control = capture("dpkg-deb", ["-f", debPath]);
    return { missing, elfFindings, control, fileCount: listing.length };
  } finally {
    rmSync(extracted, { recursive: true, force: true });
  }
}

function main() {
  const args = new Set(process.argv.slice(2));
  const ready = preflight();
  if (args.has("--preflight")) {
    console.log(JSON.stringify(ready, null, 2));
    return;
  }
  const cargoTargetDir = resolve(repoRoot, process.env.CARGO_TARGET_DIR || "src-tauri/target");
  if (!args.has("--skip-build")) {
    run("pnpm", ["tauri", "build", "--ci", "--bundles", "deb", "--", "--no-default-features", "--features", RELEASE_FEATURES], {
      env: { ...process.env, CARGO_TARGET_DIR: cargoTargetDir },
    });
  }
  const debDir = join(cargoTargetDir, "release/bundle/deb");
  const debs = existsSync(debDir) ? readdirSync(debDir).filter((name) => name.endsWith(".deb")) : [];
  if (debs.length !== 1) fail(`expected exactly one .deb in ${debDir}, found ${debs.length}`);
  const debPath = join(debDir, debs[0]);
  const audit = auditPackage(debPath);
  const commit = capture("git", ["rev-parse", "HEAD"]).trim();
  const report = {
    status: audit.missing.length === 0 && audit.elfFindings.length === 0 ? "passed" : "failed",
    commit,
    features: RELEASE_FEATURES,
    package: { path: debPath, bytes: statSync(debPath).size, sha256: sha256(debPath) },
    control: audit.control,
    fileCount: audit.fileCount,
    missingPayload: audit.missing,
    deniedElfDependencies: audit.elfFindings,
  };
  const reportPath = join(repoRoot, "output/linux-release", commit.slice(0, 12), "report.json");
  mkdirSync(dirname(reportPath), { recursive: true });
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify({ status: report.status, reportPath, debPath }, null, 2));
  if (report.status !== "passed") process.exit(1);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
