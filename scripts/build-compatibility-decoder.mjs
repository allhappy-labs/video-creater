#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  chmodSync,
  copyFileSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

import {
  reconcilePackagedRuntimeRpaths,
} from "./packaged-runtime-rpaths.mjs";

const repoRoot = resolve(import.meta.dirname, "..");
const release = process.argv.includes("--release");
const dryRun = process.argv.includes("--dry-run");
const target = targetTriple();
if (target !== "aarch64-apple-darwin") {
  throw new Error(`compatibility decoder packaging supports only aarch64-apple-darwin, got ${target}`);
}

const executable = "video-creater-compatibility-decoder";
const targetDir = resolve(repoRoot, process.env.CARGO_TARGET_DIR || "src-tauri/target");
const profile = release ? "release" : "debug";
const built = join(targetDir, target, profile, executable);
const bundled = resolve(repoRoot, "src-tauri/binaries", `${executable}-${target}`);
const runtime = resolve(repoRoot, "src-tauri/resources/compatibility-runtime");
const pluginSource = resolvePluginSource();
const cargoArgs = [
  "build",
  "--locked",
  "--manifest-path",
  "src-tauri/crates/compatibility-worker/Cargo.toml",
  "--bin",
  executable,
  "--target",
  target,
];
if (release) cargoArgs.push("--release");
if (process.env.VIDEO_CREATER_OFFLINE_BUILD === "1") cargoArgs.push("--offline");

const requiredPlugins = [
  "app",
  "applemedia",
  "audioconvert",
  "audiorate",
  "audioresample",
  "coreelements",
  "isomp4",
  "matroska",
  "opengl",
  "opus",
  "playback",
  "typefindfunctions",
  "videoconvertscale",
  "videorate",
  "vorbis",
  "vpx",
];

if (dryRun) {
  console.log(
    JSON.stringify(
      { target, profile, cargoArgs, built, bundled, runtime, pluginSource, requiredPlugins },
      null,
      2,
    ),
  );
  process.exit(0);
}

const build = spawnSync("cargo", cargoArgs, { cwd: repoRoot, stdio: "inherit" });
if (build.status !== 0) process.exit(build.status ?? 1);

rmSync(runtime, { recursive: true, force: true });
mkdirSync(dirname(bundled), { recursive: true });
mkdirSync(join(runtime, "lib"), { recursive: true });
mkdirSync(join(runtime, "plugins"), { recursive: true });
mkdirSync(join(runtime, "licenses"), { recursive: true });
copyFileSync(built, bundled);
chmodSync(bundled, 0o755);

const stagedMachO = [{ source: realpathSync(built), target: bundled, kind: "worker" }];
for (const plugin of requiredPlugins) {
  const source = join(pluginSource, `libgst${plugin}.dylib`);
  requireRegularFile(source, `required GStreamer plugin ${plugin}`);
  const targetPath = join(runtime, "plugins", basename(source));
  copyFileSync(source, targetPath);
  chmodSync(targetPath, 0o755);
  stagedMachO.push({ source: realpathSync(source), target: targetPath, kind: "plugin" });
}

const queued = [...stagedMachO];
const stagedByBasename = new Map(stagedMachO.map((entry) => [basename(entry.target), entry]));
for (let index = 0; index < queued.length; index += 1) {
  const entry = queued[index];
  for (const dependency of dynamicDependencies(entry.source)) {
    if (isSystemDependency(dependency) || dependency.startsWith("@")) continue;
    rejectDeniedDependency(dependency);
    if (!dependency.startsWith("/")) {
      throw new Error(`non-absolute dependency cannot be safely staged: ${dependency}`);
    }
    requireRegularFile(dependency, `dynamic dependency of ${basename(entry.source)}`);
    const source = realpathSync(dependency);
    const fileName = basename(dependency);
    const existing = stagedByBasename.get(fileName);
    if (existing) {
      if (sha256(existing.source) !== sha256(source)) {
        throw new Error(`dynamic library basename collision for ${fileName}`);
      }
      continue;
    }
    const targetPath = join(runtime, "lib", fileName);
    copyFileSync(source, targetPath);
    chmodSync(targetPath, 0o755);
    const dependencyEntry = { source, target: targetPath, kind: "library" };
    stagedByBasename.set(fileName, dependencyEntry);
    stagedMachO.push(dependencyEntry);
    queued.push(dependencyEntry);
  }
}

for (const entry of stagedMachO) relocateMachO(entry, stagedMachO);
for (const entry of [...stagedMachO].reverse()) signMachO(entry.target);
stageLicenses(runtime);

const files = listFiles(runtime).map((path) => ({
  path: path.slice(runtime.length + 1),
  bytes: statSync(path).size,
  sha256: sha256(path),
}));
const manifest = {
  schemaVersion: 1,
  target,
  protocol: "video-creater.compatibility",
  protocolVersion: 1,
  worker: basename(bundled),
  runtimeRoot: "compatibility-runtime",
  plugins: requiredPlugins,
  deniedDependencyPatterns: ["libav", "ffmpeg", "x264", "x265", "fdk", "faac", "openh264"],
  licenseComponents: {
    "LGPL-2.1.txt": ["GStreamer", "GLib", "GNU libintl runtime"],
    "libvpx-BSD-3-Clause.txt": ["libvpx"],
    "opus-BSD-3-Clause.txt": ["Opus"],
    "PCRE2-BSD-3-Clause.txt": ["PCRE2"],
    "ORC-BSD.txt": ["ORC"],
    "libogg-BSD-3-Clause.txt": ["libogg"],
    "libvorbis-BSD-3-Clause.txt": ["libvorbis"],
    "libX11-MIT.txt": ["libX11", "libX11-xcb"],
    "libXau-MIT.txt": ["libXau"],
    "libXdmcp-MIT.txt": ["libXdmcp"],
    "libxcb-MIT.txt": ["libxcb"],
  },
  files,
};
writeFileSync(join(runtime, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);

const verification = spawnSync(
  "node",
  [
    "scripts/verify-compatibility-runtime.mjs",
    "--target",
    target,
    "--path",
    bundled,
    "--runtime",
    runtime,
    ...(process.env.APPLE_SIGNING_IDENTITY?.trim().startsWith("Developer ID Application:")
      ? ["--require-developer-id"]
      : []),
  ],
  { cwd: repoRoot, stdio: "inherit" },
);
if (verification.status !== 0) process.exit(verification.status ?? 1);
console.log(`prepared ${basename(bundled)} with ${files.length} compatibility runtime files`);

function targetTriple() {
  if (process.env.TAURI_ENV_TARGET_TRIPLE) return process.env.TAURI_ENV_TARGET_TRIPLE;
  const rustc = spawnSync("rustc", ["-vV"], { cwd: repoRoot, encoding: "utf8" });
  if (rustc.status !== 0) throw new Error(rustc.stderr || "rustc -vV failed");
  return rustc.stdout.match(/^host:\s*(.+)$/m)?.[1] ?? "";
}

function resolvePluginSource() {
  const candidates = [
    process.env.VIDEO_CREATER_GSTREAMER_PLUGIN_DIR,
    process.env.GST_PLUGIN_SYSTEM_PATH_1_0?.split(":")[0],
    "/opt/homebrew/lib/gstreamer-1.0",
    "/usr/local/lib/gstreamer-1.0",
  ].filter(Boolean);
  const selected = candidates.find((candidate) => {
    try {
      return lstatSync(join(candidate, "libgstcoreelements.dylib")).isFile();
    } catch {
      return false;
    }
  });
  if (!selected) throw new Error("approved GStreamer plugin source directory is unavailable");
  return realpathSync(selected);
}

function dynamicDependencies(path) {
  const result = run("otool", ["-L", path]);
  return result.stdout
    .split(/\r?\n/)
    .slice(1)
    .map((line) => line.trim().match(/^(\S+)/)?.[1])
    .filter(Boolean);
}

function relocateMachO(entry, allEntries) {
  const dependencies = dynamicDependencies(entry.target);
  for (const dependency of dependencies) {
    if (isSystemDependency(dependency)) continue;
    rejectDeniedDependency(dependency);
    const replacement = `@rpath/${basename(dependency)}`;
    if (dependency !== replacement) runRequired("install_name_tool", ["-change", dependency, replacement, entry.target]);
  }
  if (entry.kind !== "worker") {
    runRequired("install_name_tool", ["-id", `@rpath/${basename(entry.target)}`, entry.target]);
  }
  const rpaths = entry.kind === "worker"
    ? [
        "@executable_path/../Resources/compatibility-runtime/lib",
        "@executable_path/../../resources/compatibility-runtime/lib",
        "@executable_path/../resources/compatibility-runtime/lib",
      ]
    : [entry.kind === "plugin" ? "@loader_path/../lib" : "@loader_path"];
  reconcilePackagedRuntimeRpaths({
    path: entry.target,
    requiredRpaths: rpaths,
    run: runRequired,
  });

  const available = new Set(allEntries.map((item) => basename(item.target)));
  for (const dependency of dynamicDependencies(entry.target)) {
    if (isSystemDependency(dependency)) continue;
    if (!dependency.startsWith("@rpath/") || !available.has(basename(dependency))) {
      throw new Error(`relocated dependency is not staged: ${dependency} from ${basename(entry.target)}`);
    }
  }
}

function stageLicenses(runtimeRoot) {
  const licenseDir = join(runtimeRoot, "licenses");
  const sources = [
    [join(dirname(dirname(pluginSource)), "LICENSE"), "LGPL-2.1.txt"],
    [findCellarFile("libvpx", "LICENSE"), "libvpx-BSD-3-Clause.txt"],
    [findCellarFile("opus", "COPYING"), "opus-BSD-3-Clause.txt"],
    [findCellarFile("pcre2", "COPYING"), "PCRE2-BSD-3-Clause.txt"],
    [findCellarFile("orc", "COPYING"), "ORC-BSD.txt"],
    [findCellarFile("libogg", "COPYING"), "libogg-BSD-3-Clause.txt"],
    [findCellarFile("libvorbis", "COPYING"), "libvorbis-BSD-3-Clause.txt"],
    [findCellarFile("libx11", "COPYING"), "libX11-MIT.txt"],
    [findCellarFile("libxau", "COPYING"), "libXau-MIT.txt"],
    [findCellarFile("libxdmcp", "COPYING"), "libXdmcp-MIT.txt"],
    [findCellarFile("libxcb", "COPYING"), "libxcb-MIT.txt"],
  ];
  for (const [source, name] of sources) {
    requireRegularFile(source, `license notice ${name}`);
    copyFileSync(source, join(licenseDir, name));
  }
  writeFileSync(
    join(licenseDir, "THIRD_PARTY_NOTICES.md"),
    [
      "# Compatibility Runtime Third-Party Notices",
      "",
      "This dynamically linked runtime contains a curated subset of GStreamer and its dependencies.",
      "Users may replace the bundled LGPL libraries with ABI-compatible builds for debugging or modification.",
      "",
      "- GStreamer: LGPL-2.1-or-later. Source: https://gitlab.freedesktop.org/gstreamer/gstreamer.",
      "- GLib and the dynamically linked GNU libintl runtime library: LGPL-2.1-or-later.",
      "- libvpx: BSD-3-Clause.",
      "- Opus: BSD-3-Clause.",
      "- PCRE2: BSD-3-Clause.",
      "- ORC: BSD-2-Clause and BSD-3-Clause.",
      "- libogg and libvorbis: BSD-3-Clause.",
      "- libX11, libXau, libXdmcp, and libxcb: MIT-style licenses; exact notices are bundled.",
      "",
      "Excluded by policy: gst-libav, FFmpeg/libav, x264, x265, FDK-AAC, FAAC, and OpenH264.",
      "",
    ].join("\n"),
  );
}

function findCellarFile(formula, file) {
  const root = `/opt/homebrew/Cellar/${formula}`;
  const versions = readdirSync(root).sort().reverse();
  const selected = versions.map((version) => join(root, version, file)).find((path) => {
    try { return lstatSync(path).isFile(); } catch { return false; }
  });
  if (!selected) throw new Error(`could not locate ${formula} ${file}`);
  return selected;
}

function listFiles(root) {
  return readdirSync(root, { withFileTypes: true })
    .flatMap((entry) => {
      const path = join(root, entry.name);
      if (entry.isSymbolicLink()) throw new Error(`runtime may not contain symlinks: ${path}`);
      return entry.isDirectory() ? listFiles(path) : [path];
    })
    .sort();
}

function isSystemDependency(path) {
  return path.startsWith("/System/Library/") || path.startsWith("/usr/lib/");
}

function rejectDeniedDependency(path) {
  if (/(?:libav|ffmpeg|x264|x265|fdk|faac|openh264|gstlibav)/i.test(path)) {
    throw new Error(`denied codec dependency: ${path}`);
  }
}

function requireRegularFile(path, label) {
  let stat;
  try { stat = statSync(path); } catch { throw new Error(`${label} is missing: ${path}`); }
  if (!stat.isFile()) throw new Error(`${label} must resolve to a regular file: ${path}`);
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function signMachO(path) {
  const identity = process.env.APPLE_SIGNING_IDENTITY?.trim();
  if (identity?.startsWith("Developer ID Application:")) {
    runRequired("codesign", [
      "--force",
      "--sign",
      identity,
      "--timestamp",
      "--options",
      "runtime",
      path,
    ]);
    return;
  }
  runRequired("codesign", ["--force", "--sign", "-", path]);
}

function run(command, args) {
  const result = spawnSync(command, args, { cwd: repoRoot, encoding: "utf8" });
  return { ...result, stdout: result.stdout ?? "", stderr: result.stderr ?? "" };
}

function runRequired(command, args) {
  const result = run(command, args);
  if (result.status !== 0) throw new Error(`${command} failed: ${result.stderr || result.stdout}`);
  return result;
}
