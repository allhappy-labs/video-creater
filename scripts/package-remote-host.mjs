#!/usr/bin/env node
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  cpSync,
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { auditLinuxElfTree } from "./linux-package-audit.mjs";
import { collectReleaseSourceEvidence, assertReleaseSourceStable, writeArtifactSourceEvidence, verifyArtifactSourceEvidence } from "./release-source-evidence.mjs";
import { stageReleaseNotices, missingReleaseNotices } from "./release-notices.mjs";
import { stageRemoteRenderRuntime, validateLinuxRuntimePayload } from "./linux-runtime-package.mjs";
import { verifyLinuxPackageMedia } from "./verify-linux-package-media.mjs";
import { linuxNativeBuildEnvironment } from "./linux-native-build-env.mjs";
export { linuxNativeBuildEnvironment } from "./linux-native-build-env.mjs";
export { stageReleaseNotices } from "./release-notices.mjs";
export { stageRemoteRenderRuntime } from "./linux-runtime-package.mjs";
export { verifyLinuxPackageMedia } from "./verify-linux-package-media.mjs";

const repoRoot = resolve(import.meta.dirname, "..");
export const HOST_FEATURES = "web-host";
export const REMOTE_NATIVE_HELPERS = [
  "video-creater-precompose-worker", "video-creater-compatibility-decoder", "video-creater-audio-enhance",
  "video-creater-semantic-encoder", "video-creater-speech", "video-creater-mcp-server", "video-creater-codex",
];
export const REQUIRED_PACKAGE_PATHS = [
  "bin/video-creater-host",
  ...REMOTE_NATIVE_HELPERS.map((name) => `bin/${name}`),
  "bin/codex-runtime/codex-path/rg",
  "web/index.html",
  "lib/Video Creater/render-runtime/manifest.json",
  "lib/Video Creater/speech-runtime/manifest.json",
  "share/video-creater-host.service",
  "share/remote-host.env.example",
  "share/remote-access.md",
  "manifest.json",
];

export function stageRemoteNativePayload({ repoRoot, output }) {
  mkdirSync(join(output, "bin"), { recursive: true });
  for (const name of REMOTE_NATIVE_HELPERS) {
    const destination = join(output, "bin", name);
    copyFileSync(join(repoRoot, "src-tauri/binaries", `${name}-x86_64-unknown-linux-gnu`), destination);
    chmodSync(destination, 0o755);
  }
  cpSync(join(repoRoot, "src-tauri/resources/speech-runtime"), join(output, "lib/Video Creater/speech-runtime"), { recursive: true });
}

function fail(message) {
  throw new Error(`remote host package: ${message}`);
}

function run(command, args, env = process.env) {
  const result = spawnSync(command, args, { cwd: repoRoot, env, stdio: "inherit" });
  if (result.error || result.status !== 0) {
    fail(`${command} ${args.join(" ")} failed${result.error ? `: ${result.error.message}` : ""}`);
  }
}

function discoverRuntime() {
  if (process.env.VIDEO_CREATER_RENDER_RUNTIME_ROOT) {
    return resolve(process.env.VIDEO_CREATER_RENDER_RUNTIME_ROOT);
  }
  // An installed materialized cache does not contain redistributable notices/bundled files.
  const root = join(repoRoot, "src-tauri/resources/render-runtime");
  return existsSync(root) ? root : null;
}

export function packagePreflight(runtimeRoot = discoverRuntime()) {
  const missing = [];
  if (process.platform !== "linux" || process.arch !== "x64") missing.push("x86_64 Linux host");
  for (const tool of ["cargo", "pnpm", "readelf", "ldd", "cc", "bwrap"]) {
    if (spawnSync(tool, ["--version"], { stdio: "ignore" }).status !== 0) missing.push(tool);
  }
  if (!runtimeRoot || !existsSync(join(runtimeRoot, "manifest.json"))) {
    missing.push("reviewed render runtime manifest");
  } else {
    try { validateLinuxRuntimePayload(runtimeRoot); }
    catch (error) { missing.push(`redistributable render runtime: ${error.message}`); }
  }
  return { ready: missing.length === 0, missing, runtimeRoot };
}

function filesUnder(root, directory = root) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? filesUnder(root, path) : [relative(root, path)];
  });
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function main() {
  const args = new Set(process.argv.slice(2));
  for (const arg of args) if (!["--preflight", "--skip-build"].includes(arg)) fail(`unknown argument: ${arg}`);
  const preflight = packagePreflight();
  if (args.has("--preflight")) {
    console.log(JSON.stringify(preflight, null, 2));
    if (!preflight.ready) process.exitCode = 1;
    return;
  }
  if (!preflight.ready || !preflight.runtimeRoot) fail(`preflight failed: ${preflight.missing.join(", ")}`);
  const source = collectReleaseSourceEvidence({ repoRoot });
  if (!args.has("--skip-build")) {
    run("pnpm", ["build"]);
    for (const script of ["build:compatibility-decoder:linux", "build:precompose-sidecar", "build:linux-audio-enhancer", "build:linux-semantic-encoder", "build:linux-speech-worker"]) {
      run("pnpm", [script]);
    }
    run("pnpm", ["build:codex-sidecar"]);
    run("node", ["scripts/build-macos-release.mjs", "--prepare-mcp-sidecar"]);
    run("cargo", [
      "build", "--locked", "--release", "--manifest-path", "src-tauri/Cargo.toml",
      "--features", HOST_FEATURES, "--bin", "video-creater-host",
    ], { ...linuxNativeBuildEnvironment(process.env), TAURI_CONFIG: '{"bundle":{"externalBin":[],"resources":[]}}' });
  }
  const targetDir = resolve(process.env.CARGO_TARGET_DIR ?? join(repoRoot, "src-tauri/target"));
  const hostBinary = join(targetDir, "release/video-creater-host");
  const codexRg = join(repoRoot, "src-tauri/resources/codex-runtime/codex-path/rg");
  if (!existsSync(hostBinary) || !existsSync(join(repoRoot, "dist/index.html"))) {
    fail("release host binary and production web assets are required");
  }
  assertReleaseSourceStable(source, collectReleaseSourceEvidence({ repoRoot }));
  const inputPaths = [
    join(repoRoot, "dist"), join(repoRoot, "src-tauri/resources/speech-runtime"), preflight.runtimeRoot, codexRg,
    ...REMOTE_NATIVE_HELPERS.map((name) => join(repoRoot, "src-tauri/binaries", `${name}-x86_64-unknown-linux-gnu`)),
  ];
  // --skip-build never labels arbitrary old host bytes with the current checkout's commit.
  if (args.has("--skip-build")) verifyArtifactSourceEvidence({ artifactPath: hostBinary, source, inputPaths });
  else writeArtifactSourceEvidence({ artifactPath: hostBinary, source, inputPaths });
  const runtimeName = basename(preflight.runtimeRoot);
  const output = resolve(repoRoot, "output/remote-host-package", runtimeName);
  rmSync(output, { recursive: true, force: true });
  mkdirSync(join(output, "bin"), { recursive: true, mode: 0o755 });
  mkdirSync(join(output, "bin/codex-runtime/codex-path"), { recursive: true, mode: 0o755 });
  mkdirSync(join(output, "share"), { recursive: true, mode: 0o755 });
  cpSync(hostBinary, join(output, "bin/video-creater-host"));
  stageRemoteNativePayload({ repoRoot, output });
  stageReleaseNotices({ repoRoot, output });
  cpSync(codexRg, join(output, "bin/codex-runtime/codex-path/rg"));
  cpSync(join(repoRoot, "dist"), join(output, "web"), { recursive: true });
  stageRemoteRenderRuntime({ runtimeRoot: preflight.runtimeRoot, output });
  const elfAudit = auditLinuxElfTree(output);
  if (elfAudit.failures.length > 0) fail(`ELF runtime audit failed:\n${elfAudit.failures.join("\n")}`);
  const missingNotices = missingReleaseNotices({ repoRoot, resourceRoot: join(output, "lib/Video Creater") });
  if (missingNotices.length > 0) fail(`component notices are missing: ${missingNotices.join(", ")}`);
  const mediaSmoke = verifyLinuxPackageMedia({ packageRoot: output, evidenceDirectory: join(repoRoot, "output/remote-host-package-evidence", source.shortCommit) });
  cpSync(join(repoRoot, "packaging/systemd/video-creater-host.service"), join(output, "share/video-creater-host.service"));
  cpSync(join(repoRoot, "packaging/systemd/remote-host.env.example"), join(output, "share/remote-host.env.example"));
  cpSync(join(repoRoot, "docs/development/remote-access.md"), join(output, "share/remote-access.md"));
  run(join(output, "bin/video-creater-host"), ["--codex-sidecar-smoke"]);
  const files = filesUnder(output)
    .sort()
    .map((path) => ({ path, bytes: statSync(join(output, path)).size, sha256: sha256(join(output, path)) }));
  writeFileSync(join(output, "manifest.json"), `${JSON.stringify({
    schemaVersion: 1,
    protocolVersion: 1,
    version: JSON.parse(readFileSync(join(repoRoot, "package.json"), "utf8")).version,
    source,
    runtime: runtimeName,
    elfAudit,
    mediaSmoke,
    files,
  }, null, 2)}\n`, { mode: 0o644 });
  const missing = REQUIRED_PACKAGE_PATHS.filter((path) => !existsSync(join(output, path)));
  if (missing.length > 0) fail(`package is incomplete: ${missing.join(", ")}`);
  assertReleaseSourceStable(source, collectReleaseSourceEvidence({ repoRoot }));
  console.log(JSON.stringify({ status: "passed", output, files: files.length + 1 }, null, 2));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
