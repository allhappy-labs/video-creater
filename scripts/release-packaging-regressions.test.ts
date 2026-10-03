import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

import * as linuxRelease from "./build-linux-release.mjs";
import * as remotePackage from "./package-remote-host.mjs";
import * as sourceEvidence from "./release-source-evidence.mjs";
import { pinnedSources } from "./build-linux-media-runtime.mjs";

const repoRoot = resolve(import.meta.dirname, "..");
function temporary(run: (root: string) => void) {
  const root = mkdtempSync(join(tmpdir(), "vc-release-regression-"));
  try { run(root); } finally { rmSync(root, { recursive: true, force: true }); }
}
function compile(directory: string, output: string, source: string, flags: string[] = []) {
  mkdirSync(directory, { recursive: true });
  const input = join(directory, `${output}.c`);
  writeFileSync(input, source);
  execFileSync("cc", [input, ...flags, "-o", join(directory, output)]);
  return join(directory, output);
}
function audit(root: string) {
  assert.equal(typeof linuxRelease.auditLinuxElfTree, "function", "release must audit resolved ELF closure");
  return linuxRelease.auditLinuxElfTree(root);
}

test("release ELF audit accepts a relocatable bundled dependency", () => temporary((root) => {
  const lib = join(root, "lib");
  compile(lib, "libvcfixture.so", "int fixture(void) { return 7; }", ["-shared", "-fPIC", "-Wl,-soname,libvcfixture.so"]);
  compile(join(root, "bin"), "fixture", "int fixture(void); int main(void) { return fixture() != 7; }", [
    `-L${lib}`, "-lvcfixture", "-Wl,-rpath,$ORIGIN/../lib",
  ]);
  assert.deepEqual(audit(root).failures, []);
}));

test("release ELF audit rejects an unresolved non-denylisted dependency", () => temporary((root) => {
  const lib = join(root, "lib");
  compile(lib, "libvcmissing.so", "int fixture(void) { return 7; }", ["-shared", "-fPIC", "-Wl,-soname,libvcmissing.so"]);
  compile(join(root, "bin"), "fixture", "int fixture(void); int main(void) { return fixture() != 7; }", [
    `-L${lib}`, "-lvcmissing", "-Wl,-rpath,$ORIGIN/../lib",
  ]);
  rmSync(join(lib, "libvcmissing.so"));
  assert.match(audit(root).failures.join("\n"), /libvcmissing\.so.*not found/);
}));

test("release ELF audit rejects a transitive denied library", () => temporary((root) => {
  const lib = join(root, "lib");
  compile(lib, "libx264.so.999", "int denied(void) { return 7; }", ["-shared", "-fPIC", "-Wl,-soname,libx264.so.999"]);
  compile(lib, "libvcfixture.so", "int denied(void); int fixture(void) { return denied(); }", [
    "-shared", "-fPIC", "-Wl,-soname,libvcfixture.so", `-L${lib}`, "-l:libx264.so.999", "-Wl,-rpath,$ORIGIN",
  ]);
  compile(join(root, "bin"), "fixture", "int fixture(void); int main(void) { return fixture() != 7; }", [
    `-L${lib}`, "-lvcfixture", `-Wl,-rpath-link,${lib}`, "-Wl,-rpath,$ORIGIN/../lib",
  ]);
  assert.match(audit(root).failures.join("\n"), /bin\/fixture.*denied.*libx264\.so\.999/);
}));

test("release ELF audit rejects a build-machine runpath", () => temporary((root) => {
  compile(join(root, "bin"), "fixture", "int main(void) { return 0; }", ["-Wl,-rpath,/home/builder/src-tauri/target/release"]);
  assert.match(audit(root).failures.join("\n"), /forbidden.*RUNPATH.*\/home\/builder/);
}));

test("release ELF audit rejects packaged symlinks escaping the artifact", () => temporary((root) => {
  const packageRoot = join(root, "package");
  mkdirSync(join(packageRoot, "bin"), { recursive: true });
  compile(join(root, "build"), "fixture", "int main(void) { return 0; }");
  symlinkSync(join(root, "build/fixture"), join(packageRoot, "bin/helper"));
  assert.match(audit(packageRoot).failures.join("\n"), /bin\/helper.*symlink.*outside/);
}));

test("remote package stages every desktop-equivalent helper and runtime", () => temporary((root) => {
  assert.equal(typeof remotePackage.stageRemoteNativePayload, "function", "package must stage the helper closure");
  for (const helper of ["precompose-worker", "compatibility-decoder", "audio-enhance", "semantic-encoder", "speech", "mcp-server", "codex"]) {
    const path = join(root, "src-tauri/binaries", `video-creater-${helper}-x86_64-unknown-linux-gnu`);
    mkdirSync(join(root, "src-tauri/binaries"), { recursive: true });
    writeFileSync(path, helper);
  }
  const speech = join(root, "src-tauri/resources/speech-runtime/lib");
  mkdirSync(speech, { recursive: true });
  writeFileSync(join(speech, "libonnxruntime.so"), "runtime");
  const output = join(root, "package");
  remotePackage.stageRemoteNativePayload({ repoRoot: root, output });
  assert.equal(readFileSync(join(output, "bin/video-creater-compatibility-decoder"), "utf8"), "compatibility-decoder");
  assert.equal(readFileSync(join(output, "bin/video-creater-precompose-worker"), "utf8"), "precompose-worker");
  assert.ok(existsSync(join(output, "bin/video-creater-speech")));
  assert.ok(existsSync(join(output, "lib/Video Creater/speech-runtime/lib/libonnxruntime.so")));
}));

test("remote package includes precompose and Codex notices and provenance", () => temporary((output) => {
  assert.equal(typeof remotePackage.stageReleaseNotices, "function", "package must stage component notices");
  remotePackage.stageReleaseNotices({ repoRoot, output });
  for (const path of [
    "precompose-runtime/LICENSE", "precompose-runtime/THIRD_PARTY_NOTICES.md", "precompose-runtime/SOURCE.json",
    "precompose-runtime/deps/thorvg/LICENSE", "precompose-runtime/deps/thorvg/src/loaders/webp/LICENSE",
    "precompose-runtime/deps/thorvg/src/loaders/lottie/rapidjson/LICENSE",
    "precompose-runtime/deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/LICENSE",
    "codex-runtime/LICENSE-APACHE-2.0.txt", "codex-runtime/provenance.json",
  ]) assert.ok(existsSync(join(output, "lib/Video Creater", path)), path);
}));

test("desktop bundle includes the reviewed precompose notices", () => {
  const config = JSON.parse(readFileSync(join(repoRoot, "src-tauri/tauri.conf.json"), "utf8"));
  const resources = Object.values(config.bundle.resources);
  for (const path of ["precompose-runtime/LICENSE", "precompose-runtime/THIRD_PARTY_NOTICES.md", "precompose-runtime/SOURCE.json", "precompose-runtime/deps/thorvg/LICENSE", "precompose-runtime/deps/thorvg/src/loaders/webp/LICENSE"]) {
    assert.ok(resources.includes(path), `bundled notice missing: ${path}`);
  }
});

test("artifact source provenance rejects stale commits and changed artifact bytes", () => temporary((root) => {
  assert.equal(typeof sourceEvidence.writeArtifactSourceEvidence, "function", "artifact must bind source and bytes");
  assert.equal(typeof sourceEvidence.verifyArtifactSourceEvidence, "function");
  const artifactPath = join(root, "app.deb");
  const source = { commit: "a".repeat(40), taskOwnedDirtyPaths: [] };
  writeFileSync(artifactPath, "release-a");
  sourceEvidence.writeArtifactSourceEvidence({ artifactPath, source });
  assert.equal(sourceEvidence.verifyArtifactSourceEvidence({ artifactPath, source }).source.commit, source.commit);
  assert.throws(() => sourceEvidence.verifyArtifactSourceEvidence({ artifactPath, source: { ...source, commit: "b".repeat(40) } }), /source commit/);
  writeFileSync(artifactPath, "replaced bytes");
  assert.throws(() => sourceEvidence.verifyArtifactSourceEvidence({ artifactPath, source }), /artifact.*(?:hash|bytes)/);
}));

test("audit-only builds reject artifacts without source evidence", () => temporary((root) => {
  assert.equal(typeof sourceEvidence.verifyArtifactSourceEvidence, "function");
  const artifactPath = join(root, "app.deb");
  writeFileSync(artifactPath, "old artifact");
  assert.throws(() => sourceEvidence.verifyArtifactSourceEvidence({ artifactPath, source: { commit: "a".repeat(40) } }), /source evidence.*missing/);
}));

test("audit-only remote builds reject changed frontend or helper payload", () => temporary((root) => {
  const artifactPath = join(root, "host");
  const web = join(root, "web");
  mkdirSync(web);
  writeFileSync(artifactPath, "host");
  writeFileSync(join(web, "index.html"), "web build a");
  const source = { commit: "a".repeat(40), taskOwnedDirtyPaths: [] };
  sourceEvidence.writeArtifactSourceEvidence({ artifactPath, source, inputPaths: [web] });
  writeFileSync(join(web, "index.html"), "web build b");
  assert.throws(() => sourceEvidence.verifyArtifactSourceEvidence({ artifactPath, source, inputPaths: [web] }), /payload.*(?:hash|inputs)/);
}));

function runtimeFixture(root: string) {
  const paths = ["bundled-plugins/libgstlibav.so", "lib/libavcodec_vc.so.60", "lib/libavfilter_vc.so.9", "lib/libavformat_vc.so.60", "lib/libavutil_vc.so.58", "licenses/NOTICE.txt", "licenses/FFmpeg-LICENSE.md", "licenses/FFmpeg-COPYING.LGPLv2.1.txt", "licenses/gst-libav-COPYING.txt"];
  const bundledFiles = paths.map((path) => {
    mkdirSync(resolve(root, path, ".."), { recursive: true });
    writeFileSync(join(root, path), path);
    return { path, bytes: Buffer.byteLength(path), sha256: createHash("sha256").update(path).digest("hex") };
  });
  writeFileSync(join(root, "manifest.json"), JSON.stringify({
    schemaVersion: 1, target: "x86_64-unknown-linux-gnu", platform: "linux",
    linux: { bundledPluginDirectory: "bundled-plugins", bundledPluginFiles: ["libgstlibav.so"] },
    ffmpeg: { version: pinnedSources.ffmpeg.version, sha256: pinnedSources.ffmpeg.sha256 },
    gstLibav: { version: pinnedSources.gstLibav.version, sha256: pinnedSources.gstLibav.sha256 }, bundledFiles,
  }));
}

test("remote package stages only redistributable runtime files and license notices", () => temporary((root) => {
  assert.equal(typeof remotePackage.stageRemoteRenderRuntime, "function", "remote package needs the redistributable runtime recipe");
  const runtimeRoot = join(root, "runtime");
  runtimeFixture(runtimeRoot);
  mkdirSync(join(runtimeRoot, "plugins"));
  symlinkSync("/usr/lib/x86_64-linux-gnu/libc.so.6", join(runtimeRoot, "plugins/system-plugin.so"));
  const output = join(root, "package");
  remotePackage.stageRemoteRenderRuntime({ runtimeRoot, output });
  const staged = join(output, "lib/Video Creater/render-runtime");
  assert.ok(existsSync(join(staged, "licenses/NOTICE.txt")));
  assert.ok(existsSync(join(staged, "bundled-plugins/libgstlibav.so")));
  assert.equal(existsSync(join(staged, "plugins")), false, "distribution plugins are selected on the target, never redistributed");
}));

test("remote package rejects a tampered runtime payload despite a reviewed source pin", () => temporary((root) => {
  assert.equal(typeof remotePackage.stageRemoteRenderRuntime, "function");
  const runtimeRoot = join(root, "runtime");
  runtimeFixture(runtimeRoot);
  writeFileSync(join(runtimeRoot, "lib/libavcodec_vc.so.60"), "tampered");
  assert.throws(() => remotePackage.stageRemoteRenderRuntime({ runtimeRoot, output: join(root, "package") }), /runtime.*(?:hash|bytes)/);
}));

test("remote preflight rejects an installed runtime that lacks redistributable notices and plugins", () => temporary((root) => {
  writeFileSync(join(root, "manifest.json"), "{}");
  assert.equal(remotePackage.packagePreflight(root).ready, false);
}));

test("packaged media acceptance rejects a package missing render workers", () => temporary((root) => {
  assert.equal(typeof remotePackage.verifyLinuxPackageMedia, "function", "release packaging must execute packaged media helpers");
  assert.throws(() => remotePackage.verifyLinuxPackageMedia({ packageRoot: root }), /packaged.*helper.*missing/);
}));

test("packaged media acceptance requires the audio inference helper", () => temporary((root) => {
  mkdirSync(join(root, "bin"));
  for (const name of ["video-creater-compatibility-decoder", "video-creater-precompose-worker"]) writeFileSync(join(root, "bin", name), "");
  assert.throws(() => remotePackage.verifyLinuxPackageMedia({ packageRoot: root }), /helper.*missing.*audio-enhance/);
}));

test("Linux package entrypoints supply a usable ThorVG compiler and builtin headers", () => {
  assert.equal(typeof remotePackage.linuxNativeBuildEnvironment, "function");
  assert.equal(typeof linuxRelease.linuxNativeBuildEnvironment, "function");
  const capture = () => ({ status: 0, stdout: "/usr/lib/gcc/x86_64-linux-gnu/13/include\n" });
  assert.deepEqual(remotePackage.linuxNativeBuildEnvironment({ PATH: "/usr/bin" }, { capture }), {
    PATH: "/usr/bin", CXX: "c++", BINDGEN_EXTRA_CLANG_ARGS: "-isystem /usr/lib/gcc/x86_64-linux-gnu/13/include",
  });
  assert.deepEqual(linuxRelease.linuxNativeBuildEnvironment({ CXX: "custom-c++", BINDGEN_EXTRA_CLANG_ARGS: "-I/custom" }, { capture }), {
    CXX: "custom-c++", BINDGEN_EXTRA_CLANG_ARGS: "-I/custom",
  });
});
