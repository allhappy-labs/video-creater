#!/usr/bin/env node
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  cpSync,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  readdirSync,
  rmSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { basename, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const repoRoot = resolve(import.meta.dirname, "..");
export const HOST_FEATURES = "web-host";
export const REQUIRED_PACKAGE_PATHS = [
  "bin/video-creater-host",
  "bin/video-creater-codex",
  "bin/codex-runtime/codex-path/rg",
  "web/index.html",
  "lib/Video Creater/render-runtime/manifest.json",
  "share/video-creater-host.service",
  "share/remote-host.env.example",
  "share/remote-access.md",
  "manifest.json",
];

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
  const root = resolve(homedir(), ".local/share/com.olhapi.video-creater/render-runtime");
  if (!existsSync(root)) return null;
  return readdirSync(root, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && entry.name.startsWith("linux-"))
    .map((entry) => resolve(root, entry.name))
    .sort()
    .at(-1) ?? null;
}

export function packagePreflight(runtimeRoot = discoverRuntime()) {
  const missing = [];
  if (process.platform !== "linux" || process.arch !== "x64") missing.push("x86_64 Linux host");
  for (const tool of ["cargo", "pnpm"]) {
    if (spawnSync(tool, ["--version"], { stdio: "ignore" }).status !== 0) missing.push(tool);
  }
  if (!runtimeRoot || !existsSync(join(runtimeRoot, "manifest.json"))) {
    missing.push("reviewed render runtime manifest");
  }
  return { ready: missing.length === 0, missing, runtimeRoot };
}

function filesUnder(root, directory = root) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? filesUnder(root, path) : [relative(root, path)];
  });
}

function materializeFileSymlinks(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    const metadata = lstatSync(path);
    if (metadata.isDirectory()) {
      materializeFileSymlinks(path);
    } else if (metadata.isSymbolicLink()) {
      const resolved = realpathSync(path);
      if (!statSync(resolved).isFile()) fail(`runtime symlink is not a file: ${path}`);
      unlinkSync(path);
      copyFileSync(resolved, path);
    }
  }
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function main() {
  const args = new Set(process.argv.slice(2));
  const preflight = packagePreflight();
  if (args.has("--preflight")) {
    console.log(JSON.stringify(preflight, null, 2));
    if (!preflight.ready) process.exitCode = 1;
    return;
  }
  if (!preflight.ready || !preflight.runtimeRoot) fail(`preflight failed: ${preflight.missing.join(", ")}`);
  if (!args.has("--skip-build")) {
    run("pnpm", ["build"]);
    run("pnpm", ["build:codex-sidecar"]);
    run("cargo", [
      "build", "--release", "--manifest-path", "src-tauri/Cargo.toml",
      "--features", HOST_FEATURES, "--bin", "video-creater-host",
    ], { ...process.env, TAURI_CONFIG: '{"bundle":{"externalBin":[],"resources":[]}}' });
  }
  const targetDir = resolve(process.env.CARGO_TARGET_DIR ?? join(repoRoot, "src-tauri/target"));
  const hostBinary = join(targetDir, "release/video-creater-host");
  const codexBinary = join(repoRoot, "src-tauri/binaries/video-creater-codex-x86_64-unknown-linux-gnu");
  const codexRg = join(repoRoot, "src-tauri/resources/codex-runtime/codex-path/rg");
  if (!existsSync(hostBinary) || !existsSync(join(repoRoot, "dist/index.html"))) {
    fail("release host binary and production web assets are required");
  }
  const runtimeName = basename(preflight.runtimeRoot);
  const output = resolve(repoRoot, "output/remote-host-package", runtimeName);
  rmSync(output, { recursive: true, force: true });
  mkdirSync(join(output, "bin"), { recursive: true, mode: 0o755 });
  mkdirSync(join(output, "bin/codex-runtime/codex-path"), { recursive: true, mode: 0o755 });
  mkdirSync(join(output, "share"), { recursive: true, mode: 0o755 });
  cpSync(hostBinary, join(output, "bin/video-creater-host"));
  cpSync(codexBinary, join(output, "bin/video-creater-codex"));
  cpSync(codexRg, join(output, "bin/codex-runtime/codex-path/rg"));
  cpSync(join(repoRoot, "dist"), join(output, "web"), { recursive: true });
  cpSync(preflight.runtimeRoot, join(output, "lib/Video Creater/render-runtime"), {
    recursive: true,
    dereference: false,
  });
  materializeFileSymlinks(join(output, "lib/Video Creater/render-runtime"));
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
    runtime: runtimeName,
    files,
  }, null, 2)}\n`, { mode: 0o644 });
  const missing = REQUIRED_PACKAGE_PATHS.filter((path) => !existsSync(join(output, path)));
  if (missing.length > 0) fail(`package is incomplete: ${missing.join(", ")}`);
  console.log(JSON.stringify({ status: "passed", output, files: files.length + 1 }, null, 2));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
