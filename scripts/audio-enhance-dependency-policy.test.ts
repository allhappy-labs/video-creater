import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import test from "node:test";
import { releaseNoticeResources } from "./release-notices.mjs";

// RUSTSEC-2026-0217 / GHSA-x5mv-8wgw-29hg: older tract-nnef tensor
// loaders permit integer overflow and an out-of-bounds read during model load.
// Check every locked tract crate so a split dependency graph cannot retain it.
test("the audio helper locks one patched tract release family", () => {
  const root = resolve(import.meta.dirname, "..", "src-tauri");
  const lock = readFileSync(resolve(root, "Cargo.lock"), "utf8");
  const packages = lock.split("[[package]]").flatMap((entry) => {
    const name = entry.match(/^name = "(tract-[^"]+)"$/m)?.[1];
    const version = entry.match(/^version = "([^"]+)"$/m)?.[1];
    return name && version ? [{ name, version }] : [];
  });
  assert.ok(packages.some(({ name }) => name === "tract-nnef"));
  for (const { name, version } of packages) {
    const [major, minor, patch] = version.split(".").map(Number);
    assert.ok(major === 0 && minor === 21 && patch >= 16,
      `${name} ${version} must include the RUSTSEC-2026-0217 fix`);
  }
  assert.equal(new Set(packages.map(({ version }) => version)).size, 1,
    "all tract crates must share a release version");
  const provenance = JSON.parse(readFileSync(resolve(root, "resources/audio-runtime/SOURCE.json"), "utf8"));
  assert.equal(provenance.tract.version, packages[0].version, "packaged provenance must describe the locked runtime");
  const helper = readFileSync(resolve(root, "crates/audio-enhance-worker/src/main.rs"), "utf8");
  assert.equal(provenance.deepFilterNet.revision, helper.match(/const LIBDF_REVISION: &str = "([^"]+)"/)?.[1]);
  assert.equal(provenance.model.sha256, helper.match(/const MODEL_ARCHIVE_SHA256: &str =\s*"([^"]+)"/)?.[1]);
  const model = readFileSync(resolve(root, "vendor/deepfilternet", provenance.model.path));
  assert.equal(createHash("sha256").update(model).digest("hex"), provenance.model.sha256, "embedded model must match its packaged source identity");
  const vendoredSource = JSON.parse(readFileSync(resolve(root, "vendor/deepfilternet/SOURCE.json"), "utf8"));
  assert.equal(vendoredSource.revision, provenance.deepFilterNet.revision);
});

test("release packages retain the audio runtime's source and license notices", () => {
  const repoRoot = resolve(import.meta.dirname, "..");
  const targets = releaseNoticeResources(repoRoot).map(([, target]) => target);
  for (const name of ["SOURCE.json", "libDF-LICENSE-MIT.txt", "libDF-LICENSE-APACHE.txt", "tract-LICENSE-MIT.txt", "tract-LICENSE-APACHE.txt", "deepfilternet-SOURCE.json", "tract-data-SOURCE.json", "tract-linalg-SOURCE.json"]) {
    assert.ok(targets.includes(`audio-runtime/${name}`), `missing packaged audio notice ${name}`);
  }
});

test("vendored audio sources match their recorded reviewed file inventory", () => {
  const root = resolve(import.meta.dirname, "..", "src-tauri", "vendor");
  const inventory = (directory: string, prefix = ""): string[] => readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const name = prefix + entry.name;
    assert.equal(entry.isSymbolicLink(), false, `unexpected symlink in audio vendor ${name}`);
    return entry.isDirectory() ? inventory(resolve(directory, entry.name), name + "/") : entry.isFile() && name !== "SOURCE.json" ? [name] : [];
  });
  for (const name of ["deepfilternet", "tract-data", "tract-linalg"]) {
    const provenance = JSON.parse(readFileSync(resolve(root, name, "SOURCE.json"), "utf8"));
    assert.ok(Object.keys(provenance.files).length > 10);
    assert.deepEqual(inventory(resolve(root, name)).sort(), Object.keys(provenance.files).sort(), `${name} has unreviewed or missing vendor files`);
    for (const [file, expected] of Object.entries(provenance.files)) {
      const actual = createHash("sha256").update(readFileSync(resolve(root, name, file))).digest("hex");
      assert.equal(actual, expected, `${name}/${file} differs from its reviewed provenance`);
    }
  }
});

test("the Cargo workspace owns first-party crates and excludes reviewed vendors", () => {
  const root = resolve(import.meta.dirname, "..", "src-tauri");
  const manifest = readFileSync(resolve(root, "Cargo.toml"), "utf8");
  const members = manifest.match(/\[workspace\][\s\S]*?members\s*=\s*\[([\s\S]*?)\]/)?.[1];
  assert.ok(members);
  assert.doesNotMatch(members, /"\."/, "explicit root membership defeats Cargo's nested vendor exclusions");
  const metadata = JSON.parse(execFileSync("cargo", ["metadata", "--no-deps", "--locked", "--offline", "--manifest-path", resolve(root, "Cargo.toml"), "--format-version", "1"], { encoding: "utf8", timeout: 30_000, maxBuffer: 8 * 1024 * 1024 }));
  const owned = new Set(metadata.workspace_members);
  const names = metadata.packages.filter((pkg: { id: string }) => owned.has(pkg.id)).map((pkg: { name: string }) => pkg.name).sort();
  assert.deepEqual(names, [
    "video-creater", "video-creater-audio-enhance", "video-creater-compatibility-decoder", "video-creater-compatibility-protocol",
    "video-creater-precompose-protocol", "video-creater-precompose-worker", "video-creater-provider-e2e-harness",
    "video-creater-semantic-encoder", "video-creater-speech-worker",
  ].sort());
});
