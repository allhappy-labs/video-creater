#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import {
  lstat,
  readdir,
  realpath,
  statfs,
} from "node:fs/promises";
import { basename, dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const VERIFY_CACHE_LIMIT_BYTES = 24 * 1024 ** 3;
export const REQUIRED_FREE_BYTES = 20 * 1024 ** 3;

export function decideVerificationCacheAction({
  verificationBytes,
  availableBytes,
}) {
  return verificationBytes > VERIFY_CACHE_LIMIT_BYTES ||
    availableBytes < REQUIRED_FREE_BYTES
    ? "clean"
    : "retain";
}

export async function directorySizeBytes(directoryPath) {
  let entry;
  try {
    entry = await lstat(directoryPath);
  } catch (error) {
    if (error?.code === "ENOENT") return 0;
    throw error;
  }
  if (entry.isSymbolicLink()) return 0;
  if (!entry.isDirectory()) return entry.size;
  let total = 0;
  for (const child of await readdir(directoryPath, { withFileTypes: true })) {
    const childPath = resolve(directoryPath, child.name);
    if (child.isSymbolicLink()) continue;
    total += child.isDirectory()
      ? await directorySizeBytes(childPath)
      : (await lstat(childPath)).size;
  }
  return total;
}

export async function filesystemBytes(directoryPath) {
  const stats = await statfs(directoryPath, { bigint: true });
  return {
    capacityBytes: Number(stats.blocks * stats.bsize),
    availableBytes: Number(stats.bavail * stats.bsize),
  };
}

async function canonicalExistingParent(inputPath) {
  let cursor = inputPath;
  for (;;) {
    try {
      return {
        existing: await realpath(cursor),
        suffix: inputPath.slice(cursor.length),
      };
    } catch (error) {
      if (error?.code !== "ENOENT" || cursor === dirname(cursor)) throw error;
      cursor = dirname(cursor);
    }
  }
}

export async function validateCargoTargetPath({
  repoRoot,
  targetPath,
  kind,
}) {
  if (
    typeof targetPath !== "string" ||
    targetPath.trim() === "" ||
    /[$%][{(]?[A-Za-z_]/.test(targetPath)
  ) {
    throw new Error("Cargo target path must be an explicit non-empty path");
  }
  const canonicalRepo = await realpath(repoRoot);
  const expected = kind === "verification"
    ? resolve(canonicalRepo, "src-tauri/target/verify")
    : resolve(canonicalRepo, "src-tauri/target");
  const requested = resolve(targetPath);
  const canonicalParent = await canonicalExistingParent(dirname(requested));
  const canonicalRequested = resolve(
    canonicalParent.existing,
    `.${canonicalParent.suffix}`,
    basename(requested),
  );
  if (canonicalRequested !== expected) {
    throw new Error(`Refusing unexpected Cargo target path: ${requested}`);
  }

  try {
    const finalEntry = await lstat(requested);
    if (finalEntry.isSymbolicLink()) {
      throw new Error(`Refusing symbolic link Cargo target: ${requested}`);
    }
    if (!finalEntry.isDirectory()) {
      throw new Error(`Cargo target is not a directory: ${requested}`);
    }
    if (await realpath(requested) !== expected) {
      throw new Error(`Cargo target resolves outside the expected path: ${requested}`);
    }
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
    const { existing, suffix } = await canonicalExistingParent(requested);
    if (resolve(`${existing}${suffix}`) !== expected) {
      throw new Error(`Cargo target parent resolves outside the repository: ${requested}`);
    }
  }
  return expected;
}

async function cleanDevelopmentTarget() {
  const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const targetPath = await validateCargoTargetPath({
    repoRoot,
    targetPath: resolve(repoRoot, "src-tauri/target"),
    kind: "development",
  });
  const bytes = await directorySizeBytes(targetPath);
  console.log(JSON.stringify({ targetPath, bytes }));
  const result = spawnSync(
    "cargo",
    ["clean", "--manifest-path", "src-tauri/Cargo.toml"],
    { cwd: repoRoot, stdio: "inherit" },
  );
  process.exitCode = result.status ?? 1;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  if (process.argv.slice(2).join(" ") !== "--clean-development") {
    throw new Error("Usage: cargo-cache-policy.mjs --clean-development");
  }
  await cleanDevelopmentTarget();
}
