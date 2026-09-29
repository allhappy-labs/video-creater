#!/usr/bin/env node

import {
  existsSync,
  readFileSync,
  readdirSync,
  renameSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { spawnSync } from "node:child_process";

import {
  isSystemDependency,
  rejectExternalPackageManagerDependency,
  sha256File,
} from "./gstreamer-runtime-policy.mjs";
import { verifyGstreamerRuntime } from "./verify-gstreamer-runtime.mjs";
import {
  isReviewedRuntimeRpath,
  parseMachORpaths,
} from "./packaged-runtime-rpaths.mjs";

const repoRoot = resolve(import.meta.dirname, "..");

export const executableRuntimeRpath =
  "@executable_path/../Resources/render-runtime/lib";
export const compatibilityRuntimeRpath =
  "@executable_path/../Resources/compatibility-runtime/lib";

export function planExecutableRelocation({
  dependencies,
  rpaths,
  runtimeLibraryBasenames,
  runtimeRpath = executableRuntimeRpath,
}) {
  const dependencyChanges = [];
  for (const dependency of dependencies) {
    if (isSystemDependency(dependency)) continue;
    const name = basename(dependency);
    if (runtimeLibraryBasenames.has(name)) {
      const replacement = `@rpath/${name}`;
      if (dependency !== replacement) {
        dependencyChanges.push({ from: dependency, to: replacement });
      }
      continue;
    }
    rejectExternalPackageManagerDependency(dependency);
    if (dependency.startsWith("/")) {
      throw new Error(
        `application dependency is not system or curated runtime code: ${dependency}`,
      );
    }
  }
  dependencyChanges.sort((left, right) => left.from.localeCompare(right.from));
  const deleteRpaths = [...new Set(
    rpaths.filter((rpath) => isForbiddenPackageRpath(rpath)),
  )].sort();

  return {
    dependencyChanges,
    deleteRpaths,
    addRpaths: !runtimeRpath || rpaths.includes(runtimeRpath)
      ? []
      : [runtimeRpath],
  };
}

export function selectBundleExecutableCandidates({
  repoRoot,
  targetRoot,
  targetTriple,
  profile,
  cargoBinNames,
  externalBins,
  exists = existsSync,
}) {
  const result = [];
  for (const name of cargoBinNames) {
    const candidates = [
      targetTriple
        ? join(targetRoot, targetTriple, profile, name)
        : null,
      join(targetRoot, profile, name),
    ].filter(Boolean);
    const sourcePath = candidates.find((path) => exists(path));
    if (sourcePath) {
      result.push({
        sourcePath,
        packagedName: name,
        runtimeOwner: "render",
      });
    }
  }
  for (const configuredPath of externalBins) {
    const packagedName = basename(configuredPath);
    const sourcePath = join(
      repoRoot,
      "src-tauri",
      `${configuredPath}-${targetTriple}`,
    );
    if (!exists(sourcePath)) {
      throw new Error(`configured external binary is missing: ${sourcePath}`);
    }
    result.push({
      sourcePath,
      packagedName,
      runtimeOwner: packagedName === "video-creater-compatibility-decoder"
        ? "compatibility"
        : "system",
    });
  }
  return result;
}

export function assertNoForbiddenPackageDependencies({
  path,
  dependencies,
}) {
  const forbidden = dependencies.find((dependency) =>
    /(?:^|\/)(?:opt\/homebrew|usr\/local)(?:\/|$)/i.test(dependency),
  );
  if (forbidden) {
    throw new Error(`forbidden packaged dependency in ${path}: ${forbidden}`);
  }
}

export function assertNoForbiddenPackageRpaths({ path, rpaths }) {
  const forbidden = rpaths.find((rpath) => isForbiddenPackageRpath(rpath));
  if (forbidden) {
    throw new Error(`forbidden packaged rpath in ${path}: ${forbidden}`);
  }
}

export function assertNoPostSignMutation(plan) {
  const mutations = [
    ...plan.dependencyChanges.map(
      ({ from, to }) => `dependency ${from} -> ${to}`,
    ),
    ...(plan.deleteRpaths ?? []).map((rpath) => `delete rpath ${rpath}`),
    ...plan.addRpaths.map((rpath) => `rpath ${rpath}`),
  ];
  if (mutations.length > 0) {
    throw new Error(
      `post-sign mutation required; release is invalid: ${mutations.join(", ")}`,
    );
  }
}

export function nestedMachOPaths(manifest) {
  return manifest.files
    .filter(
      (entry) =>
        entry.path.endsWith(".dylib") ||
        entry.path === "libexec/gst-plugin-scanner" ||
        entry.path === "libexec/gstreamer-runtime-probe",
    )
    .map((entry) => entry.path);
}

export function assertNestedDependenciesRelocated(manifest) {
  for (const entry of manifest.files) {
    for (const dependency of entry.machODependencies ?? []) {
      if (isSystemDependency(dependency)) continue;
      if (!dependency.startsWith("@loader_path/")) {
        throw new Error(
          `nested runtime dependency is not loader-relative: ${entry.path} -> ${dependency}`,
        );
      }
    }
  }
}

export function parseCodeSignatureDetails(output) {
  const authorities = [...output.matchAll(/^Authority=(.+)$/gm)].map(
    (match) => match[1].trim(),
  );
  const signatureLine = output.match(/^Signature=(.+)$/m)?.[1]?.trim() ?? "";
  return {
    adhoc:
      signatureLine.toLowerCase() === "adhoc" ||
      /\bflags=.*\badhoc\b/i.test(output),
    authority:
      authorities.find((authority) =>
        authority.startsWith("Developer ID Application:"),
      ) ?? authorities[0] ?? null,
    teamId: output.match(/^TeamIdentifier=(.+)$/m)?.[1]?.trim() ?? null,
  };
}

export function assertMatchingDeveloperIdSignature({
  path,
  signature,
  expectedAuthority,
  expectedTeamId,
}) {
  if (signature.adhoc) {
    throw new Error(`ad-hoc signature is forbidden in a release: ${path}`);
  }
  if (signature.teamId !== expectedTeamId) {
    throw new Error(
      `TeamIdentifier mismatch for ${path}: got ${
        signature.teamId ?? "missing"
      }, expected ${expectedTeamId}`,
    );
  }
  if (
    !signature.authority?.startsWith("Developer ID Application:") ||
    signature.authority !== expectedAuthority
  ) {
    throw new Error(
      `Developer ID authority mismatch for ${path}: got ${
        signature.authority ?? "missing"
      }, expected ${expectedAuthority}`,
    );
  }
}

export function requireReleaseSigningConfiguration(environment) {
  const identity = environment.APPLE_SIGNING_IDENTITY?.trim();
  if (!identity) {
    throw new Error(
      "APPLE_SIGNING_IDENTITY is required before release runtime mutation",
    );
  }
  if (!identity.startsWith("Developer ID Application:")) {
    throw new Error(
      "APPLE_SIGNING_IDENTITY must be a Developer ID Application identity",
    );
  }
  const teamId = environment.APPLE_TEAM_ID?.trim();
  if (!teamId) {
    throw new Error("APPLE_TEAM_ID is required before release runtime mutation");
  }
  if (!/^[A-Z0-9]{10}$/.test(teamId)) {
    throw new Error("APPLE_TEAM_ID must be a 10-character identifier");
  }
  return { identity, teamId };
}

export function refreshRuntimeManifestIntegrity(manifest, runtimeRoot) {
  for (const entry of manifest.files) {
    const path = join(runtimeRoot, entry.path);
    entry.bytes = statSync(path).size;
    entry.sha256 = sha256File(path);
  }
  return manifest;
}

export function signNestedRuntime({
  manifest,
  runtimeRoot,
  identity,
  teamId,
  command = runRequired,
}) {
  const signed = [];
  for (const relativePath of nestedMachOPaths(manifest)) {
    const path = join(runtimeRoot, relativePath);
    command("codesign", [
      "--force",
      "--sign",
      identity,
      "--timestamp",
      "--options",
      "runtime",
      path,
    ]);
    const details = command("codesign", ["-dvvv", path]);
    const signature = parseCodeSignatureDetails(
      `${details.stdout}\n${details.stderr}`,
    );
    assertMatchingDeveloperIdSignature({
      path: relativePath,
      signature,
      expectedAuthority: identity,
      expectedTeamId: teamId,
    });
    signed.push({ path: relativePath, ...signature });
  }
  return signed;
}

export function signExecutable({
  path,
  identity,
  teamId,
  command = runRequired,
}) {
  command("codesign", [
    "--force",
    "--sign",
    identity,
    "--timestamp",
    "--options",
    "runtime",
    path,
  ]);
  const details = command("codesign", ["-dvvv", path]);
  const signature = parseCodeSignatureDetails(
    `${details.stdout}\n${details.stderr}`,
  );
  assertMatchingDeveloperIdSignature({
    path,
    signature,
    expectedAuthority: identity,
    expectedTeamId: teamId,
  });
  return signature;
}

export function inspectExecutableRelocation({
  binaryPath,
  manifest,
  runtimeRpath = executableRuntimeRpath,
  command = runRequired,
}) {
  const runtimeLibraryBasenames = new Set(
    (manifest?.files ?? [])
      .filter(
        (entry) =>
          entry.path.startsWith("lib/") && entry.path.endsWith(".dylib"),
      )
      .map((entry) => basename(entry.path)),
  );
  return planExecutableRelocation({
    dependencies: machODependencies(binaryPath, command),
    rpaths: machORpaths(binaryPath, command),
    runtimeLibraryBasenames,
    runtimeRpath,
  });
}

export function relocateExecutable({
  binaryPath,
  manifest,
  runtimeRpath = executableRuntimeRpath,
  checkOnly = false,
  command = runRequired,
}) {
  if (manifest?.files) assertNestedDependenciesRelocated(manifest);
  const plan = inspectExecutableRelocation({
    binaryPath,
    manifest,
    runtimeRpath,
    command,
  });
  if (checkOnly) {
    assertNoPostSignMutation(plan);
    return plan;
  }

  for (const change of plan.dependencyChanges) {
    command("install_name_tool", [
      "-change",
      change.from,
      change.to,
      binaryPath,
    ]);
  }
  for (const rpath of plan.deleteRpaths) {
    command("install_name_tool", ["-delete_rpath", rpath, binaryPath]);
  }
  for (const rpath of plan.addRpaths) {
    command("install_name_tool", ["-add_rpath", rpath, binaryPath]);
  }

  const completed = inspectExecutableRelocation({
    binaryPath,
    manifest,
    runtimeRpath,
    command,
  });
  assertNoPostSignMutation(completed);
  return plan;
}

export function auditPackagedMachO({
  appPath,
  expectedAuthority,
  expectedTeamId,
  command = runRequired,
}) {
  const records = [];
  for (const path of walkFiles(appPath)) {
    const fileType = command("file", [path]).stdout;
    if (!fileType.includes("Mach-O")) continue;
    const packagedPath = relative(appPath, path);
    const dependencies = machODependencies(path, command);
    assertNoForbiddenPackageDependencies({
      path: packagedPath,
      dependencies,
    });
    const rpaths = machORpaths(path, command);
    assertNoForbiddenPackageRpaths({
      path: packagedPath,
      rpaths,
    });
    command("codesign", ["--verify", "--strict", path]);
    const signatureDetails = command("codesign", ["-dvvv", path]);
    const signature = parseCodeSignatureDetails(
      `${signatureDetails.stdout}\n${signatureDetails.stderr}`,
    );
    assertMatchingDeveloperIdSignature({
      path: packagedPath,
      signature,
      expectedAuthority,
      expectedTeamId,
    });
    records.push({
      path: packagedPath,
      sha256: sha256File(path),
      dependencies,
      rpaths,
      signature,
    });
  }
  if (records.length === 0) {
    throw new Error(`packaged app contains no Mach-O files: ${appPath}`);
  }
  return records;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const signing =
    !options.checkOnly && !options.development
      ? requireReleaseSigningConfiguration(process.env)
      : null;
  const runtimeRoot = resolve(repoRoot, options.runtime);
  const manifestPath = join(runtimeRoot, "manifest.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const compatibilityRoot = resolve(repoRoot, options.compatibilityRuntime);
  const compatibilityManifest = JSON.parse(
    readFileSync(join(compatibilityRoot, "manifest.json"), "utf8"),
  );
  verifyGstreamerRuntime({
    runtimeRoot,
    target: options.target,
    probe: false,
  });
  const executables = options.binary
    ? [
        {
          sourcePath: resolveBinaryPath(options),
          packagedName: basename(options.binary),
          runtimeOwner: options.runtimeOwner,
        },
      ]
    : discoverConfiguredExecutables(options);
  const executableResults = executables.map((executable) => {
    const runtime = executable.runtimeOwner === "render"
      ? { manifest, rpath: executableRuntimeRpath }
      : executable.runtimeOwner === "compatibility"
        ? {
            manifest: compatibilityManifest,
            rpath: compatibilityRuntimeRpath,
          }
        : { manifest: null, rpath: null };
    const plan = relocateExecutable({
      binaryPath: executable.sourcePath,
      manifest: runtime.manifest,
      runtimeRpath: runtime.rpath,
      checkOnly: options.checkOnly,
    });
    const signature = signing
      ? signExecutable({
          path: executable.sourcePath,
          identity: signing.identity,
          teamId: signing.teamId,
        })
      : null;
    return { ...executable, plan, signature };
  });
  let signedRuntime = [];
  if (signing) {
    signedRuntime = signNestedRuntime({
      manifest,
      runtimeRoot,
      identity: signing.identity,
      teamId: signing.teamId,
    });
    refreshRuntimeManifestIntegrity(manifest, runtimeRoot);
    writeManifestAtomically(manifestPath, manifest);
    verifyGstreamerRuntime({
      runtimeRoot,
      target: options.target,
      probe: false,
    });
  }
  console.log(
    JSON.stringify(
      {
        schemaVersion: 1,
        status: "passed",
        mode: options.checkOnly ? "check" : "rewrite",
        executables: executableResults,
        runtimeRoot,
        nestedMachOFiles: nestedMachOPaths(manifest),
        signedRuntime,
      },
      null,
      2,
    ),
  );
}

function discoverConfiguredExecutables(options) {
  const targetRoot = resolve(
    repoRoot,
    process.env.CARGO_TARGET_DIR?.trim() || "src-tauri/target",
  );
  const profile = options.development ? "debug" : "release";
  const cargoMetadata = JSON.parse(
    runRequired("cargo", [
      "metadata",
      "--no-deps",
      "--format-version",
      "1",
      "--manifest-path",
      "src-tauri/Cargo.toml",
    ]).stdout,
  );
  const packageMetadata = cargoMetadata.packages.find(
    (entry) => entry.name === "video-creater",
  );
  if (!packageMetadata) {
    throw new Error("Cargo metadata omitted the video-creater package");
  }
  const cargoBinNames = packageMetadata.targets
    .filter((target) => target.kind.includes("bin"))
    .map((target) => target.name);
  const tauriConfig = JSON.parse(
    readFileSync(join(repoRoot, "src-tauri", "tauri.conf.json"), "utf8"),
  );
  return selectBundleExecutableCandidates({
    repoRoot,
    targetRoot,
    targetTriple: options.target,
    profile,
    cargoBinNames,
    externalBins: tauriConfig.bundle.externalBin ?? [],
  });
}

function resolveBinaryPath(options) {
  if (options.binary) {
    const binaryPath = resolve(repoRoot, options.binary);
    requireFile(binaryPath, "application binary");
    return binaryPath;
  }
  const targetRoot = resolve(
    repoRoot,
    process.env.CARGO_TARGET_DIR?.trim() || "src-tauri/target",
  );
  const profile = process.env.TAURI_ENV_DEBUG === "true" ? "debug" : "release";
  const candidates = [
    process.env.TAURI_ENV_TARGET_TRIPLE?.trim()
      ? join(
          targetRoot,
          process.env.TAURI_ENV_TARGET_TRIPLE.trim(),
          profile,
          "video-creater",
        )
      : null,
    join(targetRoot, profile, "video-creater"),
  ].filter(Boolean);
  const selected = candidates.find((candidate) => existsSync(candidate));
  if (!selected) {
    throw new Error(
      `application binary is missing before bundling: ${candidates.join(", ")}`,
    );
  }
  return selected;
}

function machODependencies(path, command) {
  return command("otool", ["-L", path]).stdout
    .split(/\r?\n/)
    .slice(1)
    .map((line) => line.trim().match(/^(\S+)/)?.[1])
    .filter(Boolean);
}

function machORpaths(path, command) {
  const result = command("otool", ["-l", path]);
  return parseMachORpaths(`${result.stdout ?? ""}${result.stderr ?? ""}`);
}

function isForbiddenPackageRpath(rpath) {
  return !isReviewedRuntimeRpath(rpath) &&
    rpath !== "/usr/lib" &&
    !rpath.startsWith("/System/Library/");
}

function runRequired(command, args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
  });
  if (result.error || result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed: ${
        result.error?.message || result.stderr || result.stdout
      }`,
    );
  }
  return {
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
  };
}

function parseArgs(argv) {
  const options = {
    binary: null,
    checkOnly: false,
    development: process.env.TAURI_ENV_DEBUG === "true",
    compatibilityRuntime: "src-tauri/resources/compatibility-runtime",
    runtime: "src-tauri/resources/render-runtime",
    runtimeOwner: "render",
    target:
      process.env.TAURI_ENV_TARGET_TRIPLE?.trim() ||
      "aarch64-apple-darwin",
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--binary") {
      options.binary = requireValue(argument, argv[++index]);
    } else if (argument === "--runtime") {
      options.runtime = requireValue(argument, argv[++index]);
    } else if (argument === "--compatibility-runtime") {
      options.compatibilityRuntime = requireValue(argument, argv[++index]);
    } else if (argument === "--runtime-owner") {
      options.runtimeOwner = requireValue(argument, argv[++index]);
      if (!["render", "compatibility", "system"].includes(options.runtimeOwner)) {
        throw new Error(
          "--runtime-owner must be render, compatibility, or system",
        );
      }
    } else if (argument === "--target") {
      options.target = requireValue(argument, argv[++index]);
    } else if (argument === "--check") {
      options.checkOnly = true;
    } else if (argument === "--development") {
      options.development = true;
    } else if (argument !== "--") {
      throw new Error(`unknown argument: ${argument}`);
    }
  }
  return options;
}

function requireValue(flag, value) {
  if (!value || value.startsWith("--")) {
    throw new Error(`missing value for ${flag}`);
  }
  return value;
}

function requireFile(path, label) {
  if (!existsSync(path)) throw new Error(`${label} is missing: ${path}`);
}

function writeManifestAtomically(path, manifest) {
  const temporaryPath = `${path}.tmp-${process.pid}`;
  writeFileSync(temporaryPath, `${JSON.stringify(manifest, null, 2)}\n`);
  renameSync(temporaryPath, path);
}

function walkFiles(root) {
  return readdirSync(root, { withFileTypes: true })
    .flatMap((entry) => {
      const path = join(root, entry.name);
      return entry.isDirectory() ? walkFiles(path) : [path];
    })
    .sort();
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
