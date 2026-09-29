import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { basename, dirname, extname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const requiredStrictOptions = Object.freeze([
  "strict",
  "noUncheckedIndexedAccess",
  "exactOptionalPropertyTypes",
  "noImplicitOverride",
]);

const forbiddenBareCommands = Object.freeze([
  "pnpm",
  "npm",
  "node",
  "brew",
  "gst-launch-1.0",
  "ffmpeg",
  "temporal",
  "protoc",
]);

const evidenceOnlyRustFiles = new Set([
  "src-tauri/src/bin/video-creater-compatibility-evidence.rs",
  "src-tauri/src/bin/video-creater-nle-roundtrip-evidence.rs",
  "src-tauri/src/bin/video-creater-render-shader-templates.rs",
]);

const forbiddenDotlottieFeatures = Object.freeze([
  "audio",
  "c_api",
  "dev",
  "state-machines",
  "theming",
  "tracking_allocator",
  "tvg-gl",
  "tvg-log",
  "tvg-otf",
  "tvg-simd",
  "tvg-threads",
  "tvg-ttf",
  "tvg-wg",
  "wasm-bindgen-api",
  "webgl",
  "webgpu",
]);

const excludedDirectoryNames = new Set([
  ".git",
  "node_modules",
  "output",
  "target",
  "target-current-release",
  "target-final-release",
]);

function toPosix(path) {
  return path.split(sep).join("/");
}

function lineAt(source, index) {
  return source.slice(0, index).split("\n").length;
}

function walkFiles(root, options = {}) {
  if (!existsSync(root)) return [];
  const files = [];
  const visit = (directory) => {
    const entries = readdirSync(directory, { withFileTypes: true }).sort((left, right) =>
      left.name.localeCompare(right.name),
    );
    for (const entry of entries) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) {
        if (!excludedDirectoryNames.has(entry.name)) visit(path);
      } else if (entry.isFile() && (!options.extension || entry.name.endsWith(options.extension))) {
        files.push(path);
      }
    }
  };
  visit(root);
  return files;
}

function readJson(path, displayPath, failures) {
  if (!existsSync(path)) {
    failures.push(`${displayPath}: required configuration is missing`);
    return null;
  }
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    failures.push(`${displayPath}: invalid JSON: ${error.message}`);
    return null;
  }
}

function checkFrontendSource(root, failures) {
  const sourceRoot = join(root, "src");
  for (const path of walkFiles(sourceRoot)) {
    if (path.endsWith(".js") || path.endsWith(".jsx")) {
      failures.push(
        `${toPosix(relative(root, path))}: frontend source must use .ts or .tsx`,
      );
    }
  }
}

function checkTypeScriptConfig(root, failures) {
  for (const configName of ["tsconfig.json", "tsconfig.node.json"]) {
    const config = readJson(join(root, configName), configName, failures);
    if (!config) continue;
    const compilerOptions = config.compilerOptions ?? {};
    for (const option of requiredStrictOptions) {
      if (compilerOptions[option] !== true) {
        failures.push(`${configName}: ${option} must be true`);
      }
    }
  }
}

function checkClippyAttributes(root, failures) {
  const rustRoot = join(root, "src-tauri", "src");
  for (const path of walkFiles(rustRoot, { extension: ".rs" })) {
    const displayPath = toPosix(relative(root, path));
    const source = readFileSync(path, "utf8");
    const broadAllow = /#\s*!?\s*\[\s*allow\s*\(\s*clippy::[^\]]+\)\s*\]/g;
    for (const match of source.matchAll(broadAllow)) {
      failures.push(
        `${displayPath}:${lineAt(source, match.index)}: broad Clippy allow is prohibited`,
      );
    }

    const clippyExpect = /#\s*!?\s*\[\s*expect\s*\(([\s\S]*?clippy::[\s\S]*?)\)\s*\]/g;
    for (const match of source.matchAll(clippyExpect)) {
      if (!/\breason\s*=\s*"[^"\n]+"/.test(match[1])) {
        failures.push(
          `${displayPath}:${lineAt(source, match.index)}: Clippy expect requires a non-empty reason`,
        );
      }
      if (match[0].startsWith("#!")) {
        failures.push(
          `${displayPath}:${lineAt(source, match.index)}: crate-level Clippy expect is prohibited`,
        );
      }
    }
  }
}

function checkReleaseCommands(root, failures) {
  const rustRoot = join(root, "src-tauri", "src");
  const commandPattern = new RegExp(
    `(?:Command|CommandSpec)::new\\s*\\(\\s*["'](${forbiddenBareCommands
      .map((command) => command.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
      .join("|")})["']\\s*\\)`,
    "g",
  );
  const recoveryPattern = /["'`](brew install\s+[^"'`\n]+|pnpm visual:qa[^"'`\n]*)["'`]/g;
  const developerRecoveryPattern =
    /["'`]((?:Install[^"'`\n]*GStreamer[^"'`\n]*)|(?:[^"'`\n]*add a reviewed policy entry[^"'`\n]*))["'`]/g;

  for (const path of walkFiles(rustRoot, { extension: ".rs" })) {
    const displayPath = toPosix(relative(root, path));
    if (evidenceOnlyRustFiles.has(displayPath)) continue;
    const source = readFileSync(path, "utf8");
    for (const match of source.matchAll(commandPattern)) {
      failures.push(
        `${displayPath}:${lineAt(source, match.index)}: release runtime resolves forbidden bare command "${match[1]}"`,
      );
    }
    for (const match of source.matchAll(recoveryPattern)) {
      failures.push(
        `${displayPath}:${lineAt(source, match.index)}: release-facing recovery exposes developer command "${match[1]}"`,
      );
    }
    for (const match of source.matchAll(developerRecoveryPattern)) {
      failures.push(
        `${displayPath}:${lineAt(source, match.index)}: release-facing recovery exposes developer instruction "${match[1]}"`,
      );
    }
  }

  const frontendRoot = join(root, "src");
  for (const path of walkFiles(frontendRoot)) {
    if (!path.endsWith(".ts") && !path.endsWith(".tsx")) continue;
    const displayPath = toPosix(relative(root, path));
    if (displayPath.includes(".test.")) continue;
    const source = readFileSync(path, "utf8");
    for (const match of source.matchAll(recoveryPattern)) {
      failures.push(
        `${displayPath}:${lineAt(source, match.index)}: release-facing frontend exposes developer command "${match[1]}"`,
      );
    }
    for (const match of source.matchAll(developerRecoveryPattern)) {
      failures.push(
        `${displayPath}:${lineAt(source, match.index)}: release-facing frontend exposes developer instruction "${match[1]}"`,
      );
    }
  }
}

function checkDotlottieModuleTraversal(root, failures) {
  const sourceRoot = join(root, "src-tauri", "vendor", "dotlottie-rs", "src");
  for (const path of walkFiles(sourceRoot, { extension: ".rs" })) {
    const source = readFileSync(path, "utf8");
    const displayPath = toPosix(relative(root, path));
    const fileName = basename(path);
    const moduleRoot = ["lib.rs", "main.rs", "mod.rs"].includes(fileName)
      ? dirname(path)
      : join(dirname(path), fileName.slice(0, -extname(fileName).length));
    const moduleDeclaration = /^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;/gm;
    for (const match of source.matchAll(moduleDeclaration)) {
      const moduleName = match[1];
      const flatPath = join(moduleRoot, `${moduleName}.rs`);
      const nestedPath = join(moduleRoot, moduleName, "mod.rs");
      if (!existsSync(flatPath) && !existsSync(nestedPath)) {
        failures.push(
          `${displayPath}:${lineAt(source, match.index)}: module ${moduleName} has no traversable source file`,
        );
      }
    }
  }
}

function checkDotlottieFeaturePolicy(root, failures) {
  const crateRoot = join(root, "src-tauri", "vendor", "dotlottie-rs");
  const manifestPath = join(crateRoot, "Cargo.toml");
  const libraryPath = join(crateRoot, "src", "lib.rs");
  if (!existsSync(manifestPath) || !existsSync(libraryPath)) return;

  const manifest = readFileSync(manifestPath, "utf8");
  const library = readFileSync(libraryPath, "utf8");
  const buildScriptPath = join(crateRoot, "build.rs");
  const buildScript = existsSync(buildScriptPath) ? readFileSync(buildScriptPath, "utf8") : "";
  const compileErrorMarker =
    'compile_error!("this vendored dotLottie runtime supports only the reviewed native CPU feature set")';
  const markerIndex = library.indexOf(compileErrorMarker);
  const guardStart = markerIndex === -1 ? -1 : library.lastIndexOf("#[cfg(any(", markerIndex);
  const guard = guardStart === -1 ? "" : library.slice(guardStart, markerIndex);

  for (const feature of forbiddenDotlottieFeatures) {
    const escapedFeature = feature.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    if (!new RegExp(`^${escapedFeature}\\s*=`, "m").test(manifest)) {
      failures.push(`src-tauri/vendor/dotlottie-rs/Cargo.toml: forbidden feature ${feature} must remain explicitly declared`);
    }
    if (!guard.includes(`feature = "${feature}"`)) {
      failures.push(
        `src-tauri/vendor/dotlottie-rs/src/lib.rs: dotLottie feature ${feature} is missing from the fail-closed compile guard`,
      );
    }
    if (!buildScript.includes(`feature = "${feature}"`)) {
      failures.push(
        `src-tauri/vendor/dotlottie-rs/build.rs: dotLottie feature ${feature} is missing from the pre-compilation build guard`,
      );
    }
  }

  const workerManifestPath = join(root, "src-tauri", "crates", "precompose-worker", "Cargo.toml");
  if (!existsSync(workerManifestPath)) return;
  const workerManifest = readFileSync(workerManifestPath, "utf8");
  const dependencyLine = workerManifest
    .split("\n")
    .find((line) => /^dotlottie-rs\s*=/.test(line.trim()));
  if (!dependencyLine) return;
  for (const feature of forbiddenDotlottieFeatures) {
    if (dependencyLine.includes(`"${feature}"`)) {
      failures.push(
        `src-tauri/crates/precompose-worker/Cargo.toml: precompose-worker enables forbidden dotLottie feature ${feature}`,
      );
    }
  }
}

export function evaluateSourceQuality(root) {
  const resolvedRoot = resolve(root);
  const failures = [];
  checkFrontendSource(resolvedRoot, failures);
  checkTypeScriptConfig(resolvedRoot, failures);
  checkClippyAttributes(resolvedRoot, failures);
  checkReleaseCommands(resolvedRoot, failures);
  checkDotlottieModuleTraversal(resolvedRoot, failures);
  checkDotlottieFeaturePolicy(resolvedRoot, failures);
  return failures.sort();
}

function parseRoot(argv) {
  const rootIndex = argv.indexOf("--root");
  if (rootIndex === -1) return resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const root = argv[rootIndex + 1];
  if (!root) throw new Error("--root requires a directory path");
  const resolvedRoot = resolve(root);
  if (!existsSync(resolvedRoot) || !statSync(resolvedRoot).isDirectory()) {
    throw new Error(`policy root is not a directory: ${root}`);
  }
  return resolvedRoot;
}

function main() {
  try {
    const root = parseRoot(process.argv.slice(2));
    const failures = evaluateSourceQuality(root);
    if (failures.length > 0) {
      process.stderr.write(`${failures.join("\n")}\n`);
      process.exitCode = 1;
      return;
    }
    process.stdout.write("source quality policy passed\n");
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 2;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main();
}
