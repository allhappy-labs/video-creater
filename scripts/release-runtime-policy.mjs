#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { basename, posix, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { evaluateSourceQuality } from "./source-quality-policy.mjs";

const deniedPathFragments = [
  "/opt/homebrew/",
  "/usr/local/",
  "/opt/local/",
  "/nix/store/",
  "/node_modules/",
  "/src-tauri/target/",
];

export function configuredReleaseExecutables({ repoRoot }) {
  const config = JSON.parse(readFileSync(resolve(repoRoot, "src-tauri/tauri.conf.json"), "utf8"));
  return new Set([
    config.productName,
    ...(config.bundle.externalBin ?? []).map((entry) => basename(entry)),
  ]);
}

export function evaluatePackagedRuntimeRecords({ records, requiredExecutables }) {
  const failures = [];
  const packagedPaths = new Set(records.map((record) => posix.normalize(record.path)));
  const directMacExecutables = records
    .map((record) => posix.normalize(record.path))
    .filter((path) => posix.dirname(path) === "Contents/MacOS");

  for (const executable of directMacExecutables) {
    if (!requiredExecutables.has(posix.basename(executable))) {
      failures.push(`${executable}: unclassified packaged executable`);
    }
  }
  for (const executable of requiredExecutables) {
    const path = `Contents/MacOS/${executable}`;
    if (!packagedPaths.has(path)) failures.push(`${path}: required executable is missing`);
  }

  for (const record of records) {
    for (const value of [...(record.dependencies ?? []), ...(record.rpaths ?? [])]) {
      const normalizedValue = value.replaceAll("\\", "/");
      const denied = deniedPathFragments.find((fragment) => normalizedValue.includes(fragment));
      if (denied) failures.push(`${record.path}: forbidden packaged path ${value}`);
    }
    for (const dependency of record.dependencies ?? []) {
      if (dependency.startsWith("/usr/lib/") || dependency.startsWith("/System/Library/")) {
        continue;
      }
      const candidates = resolveDependencyCandidates(record, dependency);
      if (candidates.length === 0 || !candidates.some((candidate) => packagedPaths.has(candidate))) {
        failures.push(`${record.path}: unresolved packaged dependency ${dependency}`);
      }
    }
  }
  return failures.sort();
}

function resolveDependencyCandidates(record, dependency) {
  const loaderDirectory = posix.dirname(posix.normalize(record.path));
  const executableDirectory = "Contents/MacOS";
  if (dependency.startsWith("@loader_path/")) {
    return [posix.normalize(posix.join(loaderDirectory, dependency.slice(13)))];
  }
  if (dependency.startsWith("@executable_path/")) {
    return [posix.normalize(posix.join(executableDirectory, dependency.slice(17)))];
  }
  if (dependency.startsWith("@rpath/")) {
    return (record.rpaths ?? []).flatMap((rpath) => {
      const expanded = expandRpath(rpath, loaderDirectory, executableDirectory);
      return expanded ? [posix.normalize(posix.join(expanded, dependency.slice(7)))] : [];
    });
  }
  if (dependency.startsWith("/")) return [];
  return [posix.normalize(posix.join(loaderDirectory, dependency))];
}

function expandRpath(rpath, loaderDirectory, executableDirectory) {
  if (rpath === "@loader_path") return loaderDirectory;
  if (rpath.startsWith("@loader_path/")) {
    return posix.normalize(posix.join(loaderDirectory, rpath.slice(13)));
  }
  if (rpath === "@executable_path") return executableDirectory;
  if (rpath.startsWith("@executable_path/")) {
    return posix.normalize(posix.join(executableDirectory, rpath.slice(17)));
  }
  return null;
}

function main() {
  const repoRoot = resolve(import.meta.dirname, "..");
  const args = process.argv.slice(2);
  if (args.includes("--source")) {
    const failures = evaluateSourceQuality(repoRoot);
    if (failures.length > 0) throw new Error(failures.join("\n"));
    process.stdout.write("release runtime source policy passed\n");
    return;
  }
  throw new Error("usage: node scripts/release-runtime-policy.mjs --source");
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
