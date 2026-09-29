#!/usr/bin/env node

import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { spawnSync } from "node:child_process";

import {
  isSystemDependency,
  rejectDeniedDependency,
  renderThirdPartyNotices,
  resolveReviewedRuntimeComponent,
  reviewedRuntimeFormulaNames,
  requiredLibraries,
  requiredPlugins,
  sha256File,
  validateCuratedSelection,
} from "./gstreamer-runtime-policy.mjs";
import {
  probeFactories,
  verifyGstreamerRuntime,
} from "./verify-gstreamer-runtime.mjs";
import {
  reconcilePackagedRuntimeRpaths,
} from "./packaged-runtime-rpaths.mjs";

const repoRoot = resolve(import.meta.dirname, "..");
let options;
let runtimeRoot;
let sourceRoot;
let temporaryRoot;
let formulae;
let staged;
let stagedByBasename;

function main() {
  options = parseArgs(process.argv.slice(2));
  runtimeRoot = resolve(repoRoot, options.runtime);
  sourceRoot = realpathSync(
    resolve(
      options.sourceRoot ??
        process.env.VIDEO_CREATER_GSTREAMER_ROOT ??
        defaultSourceRoot(),
    ),
  );

  validateCuratedSelection({
    libraries: requiredLibraries,
    plugins: requiredPlugins,
  });
  if (options.target !== "aarch64-apple-darwin") {
    throw new Error(
      `curated GStreamer runtime supports only aarch64-apple-darwin, got ${options.target}`,
    );
  }

  if (options.dryRun) {
    console.log(
      JSON.stringify(
        {
          schemaVersion: 1,
          target: options.target,
          development: options.development,
          sourceRoot,
          runtimeRoot,
          libraries: requiredLibraries,
          plugins: requiredPlugins,
        },
        null,
        2,
      ),
    );
    return;
  }

  temporaryRoot = `${runtimeRoot}.candidate-${process.pid}`;
  rmSync(temporaryRoot, { recursive: true, force: true });
  for (const directory of ["lib", "plugins", "libexec", "licenses"]) {
    mkdirSync(join(temporaryRoot, directory), { recursive: true });
  }

  formulae = readFormulae(reviewedRuntimeFormulaNames);
  staged = [];
  stagedByBasename = new Map();

  for (const library of requiredLibraries) {
    stageMachO(
      join(sourceRoot, "lib", library),
      join(temporaryRoot, "lib", library),
      "library",
    );
  }
  for (const plugin of requiredPlugins) {
    stageMachO(
      join(sourceRoot, "lib", "gstreamer-1.0", `libgst${plugin}.dylib`),
      join(temporaryRoot, "plugins", `libgst${plugin}.dylib`),
      "plugin",
    );
  }
  stageMachO(
    join(sourceRoot, "libexec", "gstreamer-1.0", "gst-plugin-scanner"),
    join(temporaryRoot, "libexec", "gst-plugin-scanner"),
    "scanner",
  );
  const probeBuildDirectory = mkdtempSync(
    join(tmpdir(), "video-creater-gstreamer-probe-build-"),
  );
  try {
    const probe = compileRuntimeProbe(probeBuildDirectory);
    stageMachO(
      probe,
      join(temporaryRoot, "libexec", "gstreamer-runtime-probe"),
      "probe",
      { owner: "video-creater", installedVersion: appVersion() },
    );
  } finally {
    rmSync(probeBuildDirectory, { recursive: true, force: true });
  }

  for (let index = 0; index < staged.length; index += 1) {
    const owner = staged[index];
    for (const dependency of sourceDependencies(owner.source)) {
      if (isSystemDependency(dependency)) continue;
      rejectDeniedDependency(dependency);
      const source = resolveDependencySource(owner.source, dependency);
      const name = basename(dependency);
      const existing = stagedByBasename.get(name);
      if (existing) {
        if (sha256File(existing.source) !== sha256File(source)) {
          throw new Error(`dynamic library basename collision: ${name}`);
        }
        continue;
      }
      stageMachO(source, join(temporaryRoot, "lib", name), "library");
    }
  }

  for (const entry of staged) relocateMachO(entry);
  for (const entry of [...staged].reverse()) {
    runRequired("codesign", ["--force", "--sign", "-", entry.target]);
  }
  stageLicenses();

  const factories = probeFactories({
    runtimeRoot: temporaryRoot,
    sourceRoot,
    timeoutMs: options.timeoutMs,
  });
  const manifest = {
    schemaVersion: 1,
    target: options.target,
    gstreamerVersion: packageVersion("gstreamer-1.0"),
    gesVersion: packageVersion("gst-editing-services-1.0"),
    libraries: [...requiredLibraries],
    plugins: [...requiredPlugins],
    factories,
    files: listStagedFiles(),
  };
  writeFileSync(
    join(temporaryRoot, "manifest.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );

  const verification = verifyGstreamerRuntime({
    runtimeRoot: temporaryRoot,
    target: options.target,
    timeoutMs: options.timeoutMs,
  });
  publishVerifiedRuntime(temporaryRoot, runtimeRoot);
  console.log(JSON.stringify(verification, null, 2));
}

export function publishVerifiedRuntime(
  candidateRoot,
  publishedRoot,
  fileSystem = { renameSync },
) {
  const candidate = resolve(candidateRoot);
  const published = resolve(publishedRoot);
  const backup = `${published}.backup-${process.pid}`;
  const hadPublishedRuntime = existsSync(published);
  rmSync(backup, { recursive: true, force: true });
  mkdirSync(dirname(published), { recursive: true });
  if (hadPublishedRuntime) fileSystem.renameSync(published, backup);
  try {
    fileSystem.renameSync(candidate, published);
  } catch (publishError) {
    if (hadPublishedRuntime) {
      try {
        fileSystem.renameSync(backup, published);
      } catch (restoreError) {
        throw new AggregateError(
          [publishError, restoreError],
          "runtime publication failed and the previous runtime could not be restored",
        );
      }
    }
    throw publishError;
  }
  rmSync(backup, { recursive: true, force: true });
}

function stageMachO(sourcePath, targetPath, kind, ownerOverride = null) {
  requireRegularFile(sourcePath, `required ${kind}`);
  const source = realpathSync(sourcePath);
  const existing = stagedByBasename.get(basename(targetPath));
  if (existing) {
    if (sha256File(existing.source) !== sha256File(source)) {
      throw new Error(`staged Mach-O basename collision: ${basename(targetPath)}`);
    }
    return existing;
  }
  mkdirSync(dirname(targetPath), { recursive: true });
  copyFileSync(source, targetPath);
  chmodSync(targetPath, 0o755);
  const entry = {
    source: targetPath,
    originalSource: source,
    target: targetPath,
    kind,
    provenance: provenanceForSource(source, targetPath, ownerOverride),
  };
  staged.push(entry);
  stagedByBasename.set(basename(targetPath), entry);
  return entry;
}

function compileRuntimeProbe(buildDirectory) {
  const pkgConfigPath = join(sourceRoot, "lib", "pkgconfig");
  const pkgConfig = spawnSync(
    "pkg-config",
    ["--cflags", "--libs", "gstreamer-1.0", "gst-editing-services-1.0"],
    {
      encoding: "utf8",
      env: { ...process.env, PKG_CONFIG_PATH: pkgConfigPath },
    },
  );
  if (pkgConfig.status !== 0) {
    throw new Error(
      `runtime probe pkg-config failed: ${pkgConfig.stderr || pkgConfig.stdout}`,
    );
  }
  const output = join(buildDirectory, "gstreamer-runtime-probe");
  const source = join(repoRoot, "scripts", "gstreamer-runtime-probe.c");
  const flags = pkgConfig.stdout.trim().split(/\s+/).filter(Boolean);
  const compile = spawnSync("cc", [source, "-o", output, ...flags], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  if (compile.status !== 0) {
    throw new Error(
      `runtime probe compilation failed: ${compile.stderr || compile.stdout}`,
    );
  }
  return output;
}

function relocateMachO(entry) {
  for (const dependency of sourceDependencies(entry.target)) {
    if (isSystemDependency(dependency)) continue;
    rejectDeniedDependency(dependency);
    const dependencyEntry = stagedByBasename.get(basename(dependency));
    if (!dependencyEntry) {
      throw new Error(`dynamic dependency was not staged: ${dependency}`);
    }
    let loaderRelative = relative(dirname(entry.target), dependencyEntry.target);
    if (!loaderRelative.startsWith(".")) loaderRelative = `./${loaderRelative}`;
    const replacement = `@loader_path/${loaderRelative.replace(/^\.\//, "")}`;
    runRequired("install_name_tool", [
      "-change",
      dependency,
      replacement,
      entry.target,
    ]);
  }
  if (entry.target.endsWith(".dylib")) {
    runRequired("install_name_tool", [
      "-id",
      `@rpath/${basename(entry.target)}`,
      entry.target,
    ]);
  }
  reconcilePackagedRuntimeRpaths({
    path: entry.target,
    requiredRpaths: [],
    run: runRequired,
  });
}

function stageLicenses() {
  const licenseRoot = join(temporaryRoot, "licenses");
  const gstreamerLicense = join(sourceRoot, "LICENSE");
  requireRegularFile(gstreamerLicense, "GStreamer license");
  copyFileSync(gstreamerLicense, join(licenseRoot, "GStreamer-LICENSE.txt"));
  writeFileSync(
    join(licenseRoot, "THIRD_PARTY_NOTICES.md"),
    renderThirdPartyNotices(
      staged.map((entry) => ({
        component: entry.provenance.component,
        file: entry.provenance.file,
        sourceUrl: entry.provenance.sourceUrl,
        license: entry.provenance.license,
      })),
    ),
  );
}

function listStagedFiles() {
  return walk(temporaryRoot)
    .filter((path) => basename(path) !== "manifest.json")
    .map((path) => {
      const entry = staged.find((candidate) => candidate.target === path);
      const provenance = entry?.provenance ?? noticeProvenance(path);
      return {
        path: relative(temporaryRoot, path),
        bytes: statSync(path).size,
        sha256: sha256File(path),
        license: provenance.license,
        sourceComponent: provenance.component,
        sourceFile: provenance.file,
        sourceUrl: provenance.sourceUrl,
        machODependencies: entry ? relocatedDependencies(path) : [],
      };
    });
}

function sourceDependencies(path) {
  return machODependenciesWithoutInstallId(path, otoolDependencies(path));
}

function relocatedDependencies(path) {
  return machODependenciesWithoutInstallId(path, otoolDependencies(path));
}

export function machODependenciesWithoutInstallId(path, dependencies) {
  return path.endsWith(".dylib") ? dependencies.slice(1) : [...dependencies];
}

function otoolDependencies(path) {
  return runRequired("otool", ["-L", path]).stdout
    .split(/\r?\n/)
    .slice(1)
    .map((line) => line.trim().match(/^(\S+)/)?.[1])
    .filter(Boolean);
}

function resolveDependencySource(ownerSource, dependency) {
  if (dependency.startsWith("/")) {
    requireRegularFile(dependency, `dynamic dependency of ${basename(ownerSource)}`);
    return realpathSync(dependency);
  }
  const name = basename(dependency);
  const candidates = [
    join(dirname(ownerSource), name),
    join(sourceRoot, "lib", name),
  ];
  const selected = candidates.find((candidate) => {
    try {
      return statSync(candidate).isFile();
    } catch {
      return false;
    }
  });
  if (!selected) {
    throw new Error(`could not resolve dynamic dependency ${dependency} from ${ownerSource}`);
  }
  return realpathSync(selected);
}

function provenanceForSource(path, targetPath, ownerOverride = null) {
  const ownership = ownerOverride ?? homebrewOwnership(path);
  if (!ownership) {
    throw new Error(`runtime dependency has no reviewed source package: ${path}`);
  }
  return resolveReviewedRuntimeComponent({
    owner: ownership.owner,
    file: relative(temporaryRoot, targetPath),
    installedVersion: ownership.installedVersion,
    formula: formulae.get(ownership.owner),
  });
}

function homebrewOwnership(path) {
  const match = path.match(/\/Cellar\/([^/]+)\/([^/]+)\//);
  return match ? { owner: match[1], installedVersion: match[2] } : null;
}

function noticeProvenance(path) {
  if (path.endsWith("Homebrew-sbom.spdx.json")) {
    throw new Error("Homebrew bottle SBOM must not be published as source metadata");
  }
  if (path.endsWith("GStreamer-LICENSE.txt")) {
    const ownership = homebrewOwnership(sourceRoot);
    return resolveReviewedRuntimeComponent({
      owner: "gstreamer",
      file: relative(temporaryRoot, path),
      installedVersion: ownership?.installedVersion,
      formula: formulae.get("gstreamer"),
    });
  }
  if (path.endsWith("THIRD_PARTY_NOTICES.md")) {
    return resolveReviewedRuntimeComponent({
      owner: "video-creater",
      file: relative(temporaryRoot, path),
      installedVersion: appVersion(),
    });
  }
  throw new Error(`runtime notice has no reviewed provenance: ${path}`);
}

function packageVersion(name) {
  const pkgConfigPath = join(sourceRoot, "lib", "pkgconfig");
  const result = spawnSync("pkg-config", ["--modversion", name], {
    encoding: "utf8",
    env: {
      ...process.env,
      PKG_CONFIG_PATH: pkgConfigPath,
    },
  });
  if (result.status !== 0) {
    throw new Error(`pkg-config ${name} failed: ${result.stderr || result.stdout}`);
  }
  return result.stdout.trim();
}

function readFormulae(names) {
  const result = spawnSync("brew", ["info", "--json=v2", ...names], {
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(`brew formula metadata failed: ${result.stderr || result.stdout}`);
  }
  const payload = JSON.parse(result.stdout);
  const entries = new Map(
    (payload.formulae ?? []).map((formula) => [formula.name, formula]),
  );
  for (const name of names) {
    if (!entries.has(name)) {
      throw new Error(`reviewed Homebrew formula metadata is unavailable: ${name}`);
    }
  }
  return entries;
}

function appVersion() {
  return JSON.parse(readFileSync(join(repoRoot, "package.json"), "utf8")).version;
}

function defaultSourceRoot() {
  const result = spawnSync("brew", ["--prefix", "gstreamer"], { encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(
      "GStreamer source root is unavailable; install the build dependency or set VIDEO_CREATER_GSTREAMER_ROOT.",
    );
  }
  return result.stdout.trim();
}

function walk(root) {
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const path = join(root, entry.name);
    if (entry.isSymbolicLink()) {
      throw new Error(`runtime may not contain symlinks: ${path}`);
    }
    return entry.isDirectory() ? walk(path) : [path];
  }).sort();
}

function runRequired(command, args) {
  const result = spawnSync(command, args, { cwd: repoRoot, encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`${command} failed: ${result.stderr || result.stdout}`);
  }
  return { ...result, stdout: result.stdout ?? "", stderr: result.stderr ?? "" };
}

function requireRegularFile(path, label) {
  let stats;
  try {
    stats = statSync(path);
  } catch {
    throw new Error(`${label} is missing: ${path}`);
  }
  if (!stats.isFile()) throw new Error(`${label} is not a regular file: ${path}`);
}

function parseArgs(argv) {
  const parsed = {
    development: false,
    dryRun: false,
    sourceRoot: null,
    runtime: "src-tauri/resources/render-runtime",
    target: process.env.TAURI_ENV_TARGET_TRIPLE || "aarch64-apple-darwin",
    timeoutMs: 30_000,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--development") parsed.development = true;
    else if (argument === "--dry-run") parsed.dryRun = true;
    else if (argument === "--source-root") {
      parsed.sourceRoot = requireValue(argument, argv[++index]);
    } else if (argument === "--runtime") {
      parsed.runtime = requireValue(argument, argv[++index]);
    } else if (argument === "--target") {
      parsed.target = requireValue(argument, argv[++index]);
    } else if (argument === "--timeout-ms") {
      parsed.timeoutMs = Number(requireValue(argument, argv[++index]));
    } else if (argument !== "--") throw new Error(`unknown argument: ${argument}`);
  }
  if (!Number.isInteger(parsed.timeoutMs) || parsed.timeoutMs <= 0) {
    throw new Error("--timeout-ms must be a positive integer");
  }
  return parsed;
}

function requireValue(flag, value) {
  if (!value || value.startsWith("--")) throw new Error(`missing value for ${flag}`);
  return value;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
