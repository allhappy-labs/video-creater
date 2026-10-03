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
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { auditLinuxElfTree } from "./linux-package-audit.mjs";
import { collectReleaseSourceEvidence, assertReleaseSourceStable, writeArtifactSourceEvidence, verifyArtifactSourceEvidence } from "./release-source-evidence.mjs";
import { missingReleaseNotices } from "./release-notices.mjs";
import { verifyLinuxPackageMedia } from "./verify-linux-package-media.mjs";
import { linuxNativeBuildEnvironment } from "./linux-native-build-env.mjs";
export { linuxNativeBuildEnvironment } from "./linux-native-build-env.mjs";
export { auditLinuxElfTree, DENIED_LIBRARY_PATTERNS, deniedLibraries } from "./linux-package-audit.mjs";

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
  for (const tool of ["cargo", "rustc", "pnpm", "dpkg-deb", "readelf", "ldd", "cc", "bwrap", "cmake", "meson", "ninja", "pkg-config", "curl"]) {
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

export function auditPackage(debPath, { evidenceDirectory } = {}) {
  const listing = capture("dpkg-deb", ["-c", debPath])
    .split("\n")
    .filter(Boolean)
    .map((line) => line.split(/\s+/).slice(5).join(" ").split(" -> ")[0]);
  const missing = missingPayload(listing);
  const extracted = mkdtempSync(join(tmpdir(), "video-creater-deb-"));
  try {
    run("dpkg-deb", ["-x", debPath, extracted]);
    const elfAudit = auditLinuxElfTree(extracted);
    const missingNotices = missingReleaseNotices({ repoRoot, resourceRoot: join(extracted, RESOURCE_ROOT) });
    let mediaSmoke = { status: "blocked", detail: "payload/ELF audit failed before media acceptance" };
    if (missing.length === 0 && elfAudit.failures.length === 0) {
      try { mediaSmoke = verifyLinuxPackageMedia({ packageRoot: extracted, evidenceDirectory }); }
      catch (error) { mediaSmoke = { status: "failed", detail: error.message }; }
    }
    const control = capture("dpkg-deb", ["-f", debPath]);
    return { missing, missingNotices, elfAudit, mediaSmoke, control, fileCount: listing.length };
  } finally {
    rmSync(extracted, { recursive: true, force: true });
  }
}

function main() {
  const args = new Set(process.argv.slice(2));
  for (const arg of args) if (!["--preflight", "--skip-build"].includes(arg)) fail(`unknown argument: ${arg}`);
  const ready = preflight();
  if (args.has("--preflight")) {
    console.log(JSON.stringify(ready, null, 2));
    return;
  }
  const source = collectReleaseSourceEvidence({ repoRoot });
  const cargoTargetDir = resolve(repoRoot, process.env.CARGO_TARGET_DIR || "src-tauri/target");
  if (!args.has("--skip-build")) {
    run("pnpm", ["tauri", "build", "--ci", "--bundles", "deb", "--", "--locked", "--no-default-features", "--features", RELEASE_FEATURES], {
      env: { ...linuxNativeBuildEnvironment(process.env), CARGO_TARGET_DIR: cargoTargetDir },
    });
  }
  const debDir = join(cargoTargetDir, "release/bundle/deb");
  const debs = existsSync(debDir) ? readdirSync(debDir).filter((name) => name.endsWith(".deb")) : [];
  if (debs.length !== 1) fail(`expected exactly one .deb in ${debDir}, found ${debs.length}`);
  const debPath = join(debDir, debs[0]);
  assertReleaseSourceStable(source, collectReleaseSourceEvidence({ repoRoot }));
  const buildEvidence = args.has("--skip-build")
    ? verifyArtifactSourceEvidence({ artifactPath: debPath, source })
    : writeArtifactSourceEvidence({ artifactPath: debPath, source });
  const audit = auditPackage(debPath, { evidenceDirectory: join(repoRoot, "output/linux-release", source.shortCommit, "media-smoke") });
  assertReleaseSourceStable(source, collectReleaseSourceEvidence({ repoRoot }));
  const commit = source.commit;
  const report = {
    status: audit.missing.length === 0 && audit.missingNotices.length === 0 && audit.elfAudit.failures.length === 0 && audit.mediaSmoke.status === "passed" ? "passed" : "failed",
    commit,
    source,
    buildEvidence,
    features: RELEASE_FEATURES,
    package: { path: debPath, bytes: statSync(debPath).size, sha256: sha256(debPath) },
    control: audit.control,
    fileCount: audit.fileCount,
    missingPayload: audit.missing,
    missingNotices: audit.missingNotices,
    elfAudit: audit.elfAudit,
    mediaSmoke: audit.mediaSmoke,
  };
  const reportPath = join(repoRoot, "output/linux-release", commit.slice(0, 12), "report.json");
  mkdirSync(dirname(reportPath), { recursive: true });
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify({ status: report.status, reportPath, debPath }, null, 2));
  if (report.status !== "passed") process.exit(1);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
