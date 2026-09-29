#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  readFileSync,
  readdirSync,
  readlinkSync,
} from "node:fs";
import { join, relative, resolve } from "node:path";

const root = resolve(process.cwd(), "src-tauri/vendor/dotlottie-rs");
const sourcePath = join(root, "SOURCE.json");
const workerCargoPath = resolve(
  process.cwd(),
  "src-tauri/crates/precompose-worker/Cargo.toml",
);
const failures = [];
const expectedFeatures = [
  "dotlottie",
  "tvg",
  "tvg-cpu",
  "tvg-jpg",
  "tvg-lottie-expressions",
  "tvg-png",
  "tvg-webp",
];
const expectedDotLottieRevision = "2ce5e48f5786c3e60301d66db4cfdff56c896895";
const expectedDotLottieArchiveSha256 =
  "d057418ffc8d208e09821a676254cc969f2ed30a1ad4a47b2bddb0728589ca31";
const expectedThorvgRevision = "73045df5398690eb1c6c0946e0b2301032337776";
const expectedThorvgArchiveSha256 =
  "39eb1d49428426a17f4bba89a1be50d73ac339650db97612a2a6128c0e0b4621";
const expectedVendoredTreeSha256 =
  "4293fb1ad020b84e2a7f85b78cd93c82ed7b65c09fe40f7ead9108063cf51fce";

function requireFile(relativePath) {
  const path = join(root, relativePath);
  if (!existsSync(path)) {
    failures.push(`missing vendored provenance file: ${relativePath}`);
    return undefined;
  }
  return path;
}

function walkTree() {
  const entries = [];
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      const relativePath = relative(root, path).split("\\").join("/");
      if (relativePath === "SOURCE.json") continue;
      if (entry.isDirectory()) visit(path);
      else entries.push({ path, relativePath });
    }
  };
  visit(root);
  return entries.sort((left, right) => left.relativePath.localeCompare(right.relativePath));
}

function vendoredTreeSha256(entries) {
  const hash = createHash("sha256");
  for (const entry of entries) {
    const stat = lstatSync(entry.path);
    if (stat.isSymbolicLink()) {
      failures.push(`vendored source must not contain symlinks: ${entry.relativePath}`);
      hash.update("L\0");
      hash.update(entry.relativePath);
      hash.update("\0");
      hash.update(readlinkSync(entry.path));
      hash.update("\0");
    } else {
      hash.update("F\0");
      hash.update(entry.relativePath);
      hash.update("\0");
      hash.update(readFileSync(entry.path));
      hash.update("\0");
    }
  }
  return hash.digest("hex");
}

for (const file of [
  "LICENSE",
  "THIRD_PARTY_NOTICES.md",
  "LICENSES/Apache-2.0.txt",
  "deps/thorvg/LICENSE",
  "deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/LICENSE",
  "deps/thorvg/src/loaders/lottie/rapidjson/LICENSE",
  "deps/thorvg/src/loaders/webp/LICENSE",
]) {
  requireFile(file);
}

const entries = walkTree();
for (const entry of entries) {
  if (
    /(^|\/)(target|build|\.git)(\/|$)/.test(entry.relativePath) ||
    /\.(a|dylib|dll|exe|gz|lottie|o|rlib|rmeta|so|tar|zip)$/i.test(entry.relativePath)
  ) {
    failures.push(`build output, archive, or binary is forbidden in vendor source: ${entry.relativePath}`);
  }
}
const actualTreeSha256 = vendoredTreeSha256(entries);

let source;
if (!existsSync(sourcePath)) {
  failures.push("missing vendored provenance file: SOURCE.json");
} else {
  source = JSON.parse(readFileSync(sourcePath, "utf8"));
  if (source.sourceRevision !== expectedDotLottieRevision) {
    failures.push("dotlottie-rs revision does not match the reviewed pin");
  }
  if (source.sourceArchiveSha256 !== expectedDotLottieArchiveSha256) {
    failures.push("dotlottie-rs archive checksum does not match the reviewed pin");
  }
  if (source.thorvg?.sourceRevision !== expectedThorvgRevision) {
    failures.push("ThorVG revision does not match the reviewed pin");
  }
  if (source.thorvg?.sourceArchiveSha256 !== expectedThorvgArchiveSha256) {
    failures.push("ThorVG archive checksum does not match the reviewed pin");
  }
  if (source.permissiveOnly !== true) {
    failures.push("vendored runtime must be recorded as permissive-only");
  }
  if (source.jerryscript?.license !== "Apache-2.0" || source.jerryscript?.expressionsEnabled !== true) {
    failures.push("JerryScript expressions must be recorded as enabled under Apache-2.0");
  }
  if (JSON.stringify(source.enabledFeatures) !== JSON.stringify(expectedFeatures)) {
    failures.push("SOURCE.json enabledFeatures does not match the reviewed exact feature set");
  }
  if (source.vendoredTreeSha256 !== actualTreeSha256) {
    failures.push(
      `vendored source tree checksum mismatch: expected ${source.vendoredTreeSha256 || "<missing>"}, got ${actualTreeSha256}`,
    );
  }
  if (
    source.vendoredTreeSha256 !== expectedVendoredTreeSha256 ||
    actualTreeSha256 !== expectedVendoredTreeSha256
  ) {
    failures.push("vendored source tree does not match the hardcoded reviewed digest");
  }
}

const cargoPath = requireFile("Cargo.toml");
const buildPath = requireFile("build.rs");
if (cargoPath && buildPath) {
  const cargoSource = readFileSync(cargoPath, "utf8");
  const buildSource = readFileSync(buildPath, "utf8");
  if (/^cbindgen\s*=/m.test(cargoSource) || buildSource.includes("cbindgen::")) {
    failures.push("unused MPL-2.0 cbindgen path must remain removed");
  }
  if (/https?:\/\/|\bminreq\b|download_file|ensure_available/.test(buildSource)) {
    failures.push("vendored build script must not contain network or download paths");
  }

  const featureSection = cargoSource.split("[features]")[1]?.split("[dependencies]")[0] || "";
  const declaredFeatures = [...featureSection.matchAll(/^([a-zA-Z0-9_-]+)\s*=\s*\[/gm)]
    .map((match) => match[1])
    .filter((feature) => feature !== "default");
  const classified = new Set([
    ...(source?.enabledFeatures || []),
    ...(source?.forbiddenFeatures || []),
  ]);
  for (const feature of declaredFeatures) {
    if (!classified.has(feature)) failures.push(`unclassified vendored Cargo feature: ${feature}`);
  }
}

if (existsSync(join(root, "src/lottie_renderer/fallback_font.bin"))) {
  failures.push("unverified fallback font payload must not be vendored");
}

for (const entry of entries.filter(({ relativePath }) =>
  /^deps\/thorvg\/src\/.*\.(c|cc|cpp|h|hpp)$/.test(relativePath),
)) {
  if (/Mozilla Public License|MPL-2\.0/.test(readFileSync(entry.path, "utf8"))) {
    failures.push(`restrictive license marker remains in compiled native source: ${entry.relativePath}`);
  }
}

if (!existsSync(workerCargoPath)) {
  failures.push("missing isolated precompose worker Cargo manifest");
} else {
  const workerCargo = readFileSync(workerCargoPath, "utf8");
  const dependency = workerCargo.match(/^dotlottie-rs\s*=\s*\{([^\n]+)\}$/m)?.[1] || "";
  if (!/default-features\s*=\s*false/.test(dependency)) {
    failures.push("worker dotlottie-rs dependency must disable default features");
  }
  const actualFeatures = [...dependency.matchAll(/"([a-zA-Z0-9_-]+)"/g)]
    .map((match) => match[1])
    .filter((value) => value !== "../../vendor/dotlottie-rs")
    .sort();
  if (JSON.stringify(actualFeatures) !== JSON.stringify([...expectedFeatures].sort())) {
    failures.push("worker dotlottie-rs dependency does not use the reviewed exact feature set");
  }
}

const result = {
  status: failures.length === 0 ? "passed" : "failed",
  root,
  failures,
  vendoredTreeSha256: actualTreeSha256,
};
console.log(JSON.stringify(result, null, 2));
if (failures.length > 0) process.exitCode = 1;
