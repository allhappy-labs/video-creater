import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdir, readdir, rename, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import { join, resolve } from "node:path";

export const repoRoot = resolve(import.meta.dirname, "..");
export const metricsDirectory = join(repoRoot, "output/build-metrics");

function commandOutput(command, args, root) {
  const result = spawnSync(command, args, { cwd: root, encoding: "utf8" });
  return !result.error && result.status === 0 ? result.stdout.trim() : null;
}

// Deliberately record an allowlist, never environment variables or process arguments.
export function performanceEnvironment(featureProfile, root = repoRoot) {
  const version = (command, args) => commandOutput(command, args, root) || "unavailable";
  const gitStatus = commandOutput("git", ["status", "--porcelain"], root);
  return {
    sha: version("git", ["rev-parse", "HEAD"]),
    dirty: gitStatus === null ? "unverified" : gitStatus !== "",
    machine: { platform: os.platform(), arch: os.arch(), osRelease: os.release(), cpuModel: os.cpus()[0]?.model, logicalCpus: os.cpus().length, totalMemoryBytes: os.totalmem() },
    versions: { node: process.version, pnpm: version("pnpm", ["--version"]), rustc: version("rustc", ["--version"]), cargo: version("cargo", ["--version"]) },
    featureProfile,
    cache: { nodeModulesPresent: existsSync(join(root, "node_modules")), cargoDevelopmentTargetPresent: existsSync(join(root, "src-tauri/target")), classification: "existing-cache-not-cleared" },
  };
}

export async function writePerformanceReport(directory, name, report, historyLimit = 12) {
  if (!/^[a-z][a-z0-9-]*$/.test(name) || !Number.isInteger(historyLimit) || historyLimit < 1) throw new Error("Invalid metrics name or history limit");
  const timestamp = new Date(report.startedAt).toISOString().replaceAll(":", "-");
  await mkdir(directory, { recursive: true });
  const serialized = `${JSON.stringify(report, null, 2)}\n`;
  const historyPath = join(directory, `${name}-${timestamp}-${process.pid}.json`);
  await writeFile(historyPath, serialized);
  const temporary = join(directory, `${name}-latest.${process.pid}.tmp`);
  await writeFile(temporary, serialized);
  await rename(temporary, join(directory, `${name}-latest.json`));
  const pattern = new RegExp(`^${name}-\\d{4}-\\d{2}-\\d{2}T[\\d.-]+Z-\\d+\\.json$`);
  const histories = (await readdir(directory)).filter((file) => pattern.test(file)).sort();
  for (const file of histories.slice(0, -historyLimit)) await rm(join(directory, file));
  return historyPath;
}
