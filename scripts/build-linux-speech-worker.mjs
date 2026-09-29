#!/usr/bin/env node
// Builds the Linux on-device speech helper (`video-creater-speech`) and stages its runtime.
//
// 1. Compiles sherpa-onnx v1.13.8 from a SHA-256 pinned source archive with TTS disabled, so the
//    GPL-3.0 espeak-ng / piper-phonemize components are never built or linked. sherpa-onnx pins
//    its own CMake dependencies (including the MIT ONNX Runtime binary) by SHA-256.
// 2. Builds `src-tauri/crates/speech-worker` with the `native` feature against that build.
// 3. Stages:
//    - src-tauri/binaries/video-creater-speech-<target>            (Tauri externalBin)
//    - src-tauri/resources/speech-runtime/lib/*.so                  (Tauri resource `speech-runtime/`)
//    - src-tauri/resources/speech-runtime/licenses/*                (third-party notices)
//    - src-tauri/resources/speech-runtime/manifest.json             (files, sizes, SHA-256)
//
// Usage: node scripts/build-linux-speech-worker.mjs [--development|--release] [--dry-run]

import { createHash } from "node:crypto";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const repoRoot = resolve(import.meta.dirname, "..");
const release = process.argv.includes("--release") && !process.argv.includes("--development");
const dryRun = process.argv.includes("--dry-run");
const target = targetTriple();
if (target !== "x86_64-unknown-linux-gnu") {
  throw new Error(`Linux speech worker packaging supports only x86_64-unknown-linux-gnu, got ${target}`);
}

const sherpaVersion = "1.13.8";
const sherpaArchiveUrl = `https://github.com/k2-fsa/sherpa-onnx/archive/refs/tags/v${sherpaVersion}.tar.gz`;
const sherpaArchiveSha256 = "b0374cc56dbc186d442ae73d5de743bb092470b640c4c50ce7b029044c0c4fa8";
const sherpaCMakeOptions = [
  "-DCMAKE_BUILD_TYPE=Release",
  "-DBUILD_SHARED_LIBS=ON",
  "-DSHERPA_ONNX_ENABLE_TTS=OFF",
  "-DSHERPA_ONNX_ENABLE_SPEAKER_DIARIZATION=ON",
  "-DSHERPA_ONNX_ENABLE_C_API=ON",
  "-DSHERPA_ONNX_ENABLE_PORTAUDIO=OFF",
  "-DSHERPA_ONNX_ENABLE_WEBSOCKET=OFF",
  "-DSHERPA_ONNX_ENABLE_BINARY=OFF",
  "-DSHERPA_ONNX_BUILD_C_API_EXAMPLES=OFF",
  "-DSHERPA_ONNX_ENABLE_PYTHON=OFF",
  "-DSHERPA_ONNX_ENABLE_TESTS=OFF",
  "-DSHERPA_ONNX_ENABLE_JNI=OFF",
  "-DSHERPA_ONNX_ENABLE_GPU=OFF",
  "-DSHERPA_ONNX_USE_PRE_INSTALLED_ONNXRUNTIME_IF_AVAILABLE=OFF",
  "-DCMAKE_INSTALL_RPATH=$ORIGIN",
  "-DCMAKE_BUILD_WITH_INSTALL_RPATH=ON",
];
// Denied anywhere in the staged runtime: GPL TTS front-ends and GPL media libraries.
const deniedLibraryPattern = /(espeak|piper|ucd|libav|ffmpeg|x264|x265|fdk|faad)/i;

const executable = "video-creater-speech";
const cargoTargetDir = resolve(repoRoot, process.env.CARGO_TARGET_DIR || "src-tauri/target");
const nativeRoot = join(cargoTargetDir, "speech-runtime", `sherpa-onnx-v${sherpaVersion}-no-tts`);
const sourceDir = join(nativeRoot, "src");
const buildDir = join(nativeRoot, "build");
const installDir = join(nativeRoot, "install");
const installLibDir = join(installDir, "lib");
const buildMarker = join(installLibDir, "video-creater-sherpa-onnx-build.json");
const profile = release ? "release" : "debug";
const built = join(cargoTargetDir, profile, executable);
const bundled = resolve(repoRoot, "src-tauri/binaries", `${executable}-${target}`);
const runtime = resolve(repoRoot, "src-tauri/resources/speech-runtime");
const jobs = process.env.CARGO_BUILD_JOBS || process.env.VIDEO_CREATER_NATIVE_JOBS || "3";
const cargoArgs = [
  "build",
  "--locked",
  "--manifest-path",
  "src-tauri/crates/speech-worker/Cargo.toml",
  "--features",
  "native",
  "--bin",
  executable,
];
if (release) cargoArgs.push("--release");
if (process.env.VIDEO_CREATER_OFFLINE_BUILD === "1") cargoArgs.push("--offline");

if (dryRun) {
  console.log(
    JSON.stringify(
      { target, profile, sherpaVersion, sherpaArchiveSha256, sherpaCMakeOptions, cargoArgs, built, bundled, runtime, installLibDir },
      null,
      2,
    ),
  );
  process.exit(0);
}

ensureSherpaOnnx();

runRequired("cargo", cargoArgs, {
  env: { ...process.env, SHERPA_ONNX_LIB_DIR: installLibDir, CARGO_BUILD_JOBS: jobs },
  stdio: "inherit",
});

rmSync(runtime, { recursive: true, force: true });
mkdirSync(join(runtime, "lib"), { recursive: true });
mkdirSync(join(runtime, "licenses"), { recursive: true });
mkdirSync(dirname(bundled), { recursive: true });
copyFileSync(built, bundled);
chmodSync(bundled, 0o755);

const stagedLibraries = stageLibraryClosure(bundled);
stageLicenses();

const runpath = dynamicSection(bundled).runpath;
for (const required of ["$ORIGIN", "$ORIGIN/../resources/speech-runtime/lib", "$ORIGIN/../lib/Video Creater/speech-runtime/lib"]) {
  if (!runpath.includes(required)) throw new Error(`${basename(bundled)} RUNPATH is missing ${required}: ${runpath.join(":")}`);
}
verifyResolution(bundled);

const files = listFiles(runtime).map((path) => ({
  path: path.slice(runtime.length + 1),
  bytes: statSync(path).size,
  sha256: sha256(path),
}));
const manifest = {
  schemaVersion: 1,
  target,
  helper: basename(bundled),
  helperBytes: statSync(bundled).size,
  helperSha256: sha256(bundled),
  runtimeRoot: "speech-runtime",
  sherpaOnnx: { version: sherpaVersion, sourceSha256: sherpaArchiveSha256, cmakeOptions: sherpaCMakeOptions, ttsEnabled: false },
  libraries: stagedLibraries,
  deniedLibraryPattern: deniedLibraryPattern.source,
  files,
};
writeFileSync(join(runtime, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`prepared ${basename(bundled)} with ${stagedLibraries.length} speech runtime libraries (${profile})`);

function ensureSherpaOnnx() {
  if (existsSync(buildMarker)) {
    const marker = JSON.parse(readFileSync(buildMarker, "utf8"));
    if (marker.sourceSha256 === sherpaArchiveSha256 && JSON.stringify(marker.cmakeOptions) === JSON.stringify(sherpaCMakeOptions)) {
      return;
    }
  }
  rmSync(nativeRoot, { recursive: true, force: true });
  mkdirSync(nativeRoot, { recursive: true });
  const archive = join(nativeRoot, `sherpa-onnx-v${sherpaVersion}.tar.gz`);
  runRequired("curl", ["--fail", "--location", "--silent", "--show-error", "--output", archive, sherpaArchiveUrl]);
  const actual = sha256(archive);
  if (actual !== sherpaArchiveSha256) {
    throw new Error(`sherpa-onnx source archive SHA-256 mismatch: expected ${sherpaArchiveSha256}, got ${actual}`);
  }
  mkdirSync(sourceDir, { recursive: true });
  runRequired("tar", ["-xzf", archive, "-C", sourceDir, "--strip-components=1"]);
  const generator = spawnSync("ninja", ["--version"], { encoding: "utf8" }).status === 0 ? ["-G", "Ninja"] : [];
  runRequired("cmake", ["-S", sourceDir, "-B", buildDir, ...generator, `-DCMAKE_INSTALL_PREFIX=${installDir}`, ...sherpaCMakeOptions], {
    stdio: "inherit",
  });
  runRequired("cmake", ["--build", buildDir, "--parallel", jobs], { stdio: "inherit" });
  runRequired("cmake", ["--install", buildDir], { stdio: "inherit" });

  for (const name of readdirSync(installLibDir)) {
    if (deniedLibraryPattern.test(name)) throw new Error(`denied library in sherpa-onnx install: ${name}`);
  }
  for (const required of ["libsherpa-onnx-c-api.so", "libonnxruntime.so"]) {
    if (!existsSync(join(installLibDir, required))) throw new Error(`sherpa-onnx install is missing ${required}`);
  }
  writeFileSync(
    buildMarker,
    `${JSON.stringify({ sherpaOnnxVersion: sherpaVersion, sourceSha256: sherpaArchiveSha256, cmakeOptions: sherpaCMakeOptions, ttsEnabled: false }, null, 2)}\n`,
  );
}

// Copies every NEEDED library that resolves inside the sherpa-onnx install (transitively).
// System libraries (glibc, libstdc++, GStreamer, GLib) are expected from the host distribution.
function stageLibraryClosure(binary) {
  const available = new Map(
    readdirSync(installLibDir)
      .filter((name) => name.includes(".so"))
      .map((name) => [name, join(installLibDir, name)]),
  );
  const staged = [];
  const queue = [binary];
  const seen = new Set();
  while (queue.length > 0) {
    const current = queue.shift();
    for (const needed of dynamicSection(current).needed) {
      if (deniedLibraryPattern.test(needed)) throw new Error(`denied dynamic dependency ${needed} of ${basename(current)}`);
      if (seen.has(needed) || !available.has(needed)) continue;
      seen.add(needed);
      const source = realpathSync(available.get(needed));
      const destination = join(runtime, "lib", needed);
      copyFileSync(source, destination);
      chmodSync(destination, 0o755);
      staged.push({ name: needed, bytes: statSync(destination).size, sha256: sha256(destination) });
      queue.push(destination);
    }
  }
  for (const entry of staged) {
    const bytes = readFileSync(join(runtime, "lib", entry.name));
    for (const marker of ["espeak-ng", "espeak_", "piper_phonemize", "piper-phonemize"]) {
      if (bytes.includes(Buffer.from(marker))) throw new Error(`${entry.name} contains GPL TTS component marker ${marker}`);
    }
  }
  for (const required of ["libsherpa-onnx-c-api.so", "libonnxruntime.so"]) {
    if (!staged.some((entry) => entry.name.startsWith(required))) {
      throw new Error(`${basename(binary)} does not link ${required}; staged: ${staged.map((entry) => entry.name).join(", ")}`);
    }
  }
  return staged;
}

function stageLicenses() {
  const licenseDir = join(runtime, "licenses");
  const deps = join(buildDir, "_deps");
  const sources = [
    [join(sourceDir, "LICENSE"), "sherpa-onnx-Apache-2.0.txt"],
    [findFirst([join(deps, "onnxruntime-src", "LICENSE"), join(deps, "onnxruntime-src", "LICENSE.txt")]), "onnxruntime-MIT.txt"],
    [findFirst([join(deps, "onnxruntime-src", "ThirdPartyNotices.txt")]), "onnxruntime-ThirdPartyNotices.txt"],
    [findFirst([join(deps, "kaldi_native_fbank-src", "LICENSE")]), "kaldi-native-fbank-Apache-2.0.txt"],
    [findFirst([join(deps, "kaldi_decoder-src", "LICENSE")]), "kaldi-decoder-Apache-2.0.txt"],
    [findFirst([join(deps, "openfst-src", "COPYING")]), "openfst-Apache-2.0.txt"],
    [findFirst([join(deps, "simple-sentencepiece-src", "LICENSE")]), "simple-sentencepiece-Apache-2.0.txt"],
    [findFirst([join(deps, "eigen-src", "COPYING.MPL2")]), "eigen-MPL-2.0.txt"],
    [findFirst([join(deps, "json-src", "LICENSE.MIT")]), "nlohmann-json-MIT.txt"],
    [findFirst([join(deps, "hclust_cpp-src", "LICENSE")]), "hclust-cpp-fastcluster-BSD-2-Clause.txt"],
    [findFirst([join(deps, "kaldifst-src", "LICENSE")]), "kaldifst-Apache-2.0.txt"],
    [findFirst([join(deps, "kissfft-src", "COPYING")]), "kissfft-BSD-3-Clause.txt"],
  ];
  for (const [source, name] of sources) {
    if (!source || !existsSync(source)) throw new Error(`license notice for ${name} is missing (looked under ${deps})`);
    copyFileSync(source, join(licenseDir, name));
  }
  writeFileSync(
    join(licenseDir, "THIRD_PARTY_NOTICES.md"),
    [
      "# Speech Runtime Third-Party Notices",
      "",
      "`video-creater-speech` dynamically links the libraries in `speech-runtime/lib`.",
      "",
      `- sherpa-onnx ${sherpaVersion} (Apache-2.0), built from source with SHERPA_ONNX_ENABLE_TTS=OFF;`,
      "  espeak-ng and piper-phonemize (GPL-3.0) are not built or linked.",
      "- ONNX Runtime (MIT), prebuilt archive pinned by SHA-256 in sherpa-onnx's CMake files.",
      "- kaldi-native-fbank, kaldi-decoder, kaldifst, OpenFst, simple-sentencepiece: Apache-2.0 (statically included).",
      "- KISS FFT: BSD-3-Clause (statically included).",
      "- Eigen: MPL-2.0 (header-only). nlohmann/json: MIT. fastcluster (hclust-cpp): BSD-2-Clause.",
      "- System GStreamer and GLib (LGPL-2.1-or-later) are used from the host distribution.",
      "",
      "Models are downloaded on demand by the app and are not part of this runtime:",
      "Parakeet TDT 0.6B v3 (CC-BY-4.0), Silero VAD v6.0 (MIT), pyannote segmentation 3.0 (MIT),",
      "WeSpeaker VoxCeleb ResNet34-LM (CC-BY-4.0).",
      "",
    ].join("\n"),
  );
}

function verifyResolution(binary) {
  const env = { ...process.env };
  delete env.LD_LIBRARY_PATH;
  const result = runRequired("ldd", [binary], { env });
  const lines = result.stdout.split(/\r?\n/);
  const missing = lines.filter((line) => line.includes("not found"));
  if (missing.length > 0) throw new Error(`unresolved libraries for ${basename(binary)}:\n${missing.join("\n")}`);
  const runtimeLib = realpathSync(join(runtime, "lib"));
  for (const library of ["libsherpa-onnx-c-api.so", "libonnxruntime.so"]) {
    const line = lines.find((entry) => entry.trim().startsWith(library));
    const resolved = line?.match(/=>\s*(\S+)/)?.[1];
    if (!resolved || !realpathSync(resolved).startsWith(runtimeLib)) {
      throw new Error(`${library} did not resolve from the staged runtime: ${line ?? "absent"}`);
    }
  }
  for (const line of lines) {
    if (deniedLibraryPattern.test(line)) throw new Error(`denied library resolved for ${basename(binary)}: ${line.trim()}`);
  }
}

function dynamicSection(path) {
  const output = runRequired("readelf", ["--dynamic", "--wide", path]).stdout;
  const needed = [...output.matchAll(/\(NEEDED\)\s+Shared library: \[([^\]]+)\]/g)].map((match) => match[1]);
  const runpath = output.match(/\((?:RUNPATH|RPATH)\)\s+Library (?:runpath|rpath): \[([^\]]*)\]/)?.[1]?.split(":") ?? [];
  return { needed, runpath };
}

function findFirst(candidates) {
  return candidates.find((candidate) => existsSync(candidate));
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

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function targetTriple() {
  if (process.env.TAURI_ENV_TARGET_TRIPLE) return process.env.TAURI_ENV_TARGET_TRIPLE;
  const rustc = spawnSync("rustc", ["-vV"], { cwd: repoRoot, encoding: "utf8" });
  if (rustc.status !== 0) throw new Error(rustc.stderr || "rustc -vV failed");
  return rustc.stdout.match(/^host:\s*(.+)$/m)?.[1] ?? "";
}

function runRequired(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    ...options,
  });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed (${result.status}): ${result.stderr || result.stdout || result.error}`);
  }
  return { stdout: result.stdout ?? "", stderr: result.stderr ?? "" };
}
