#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dryRun = process.argv.includes("--dry-run");

const commands = [
  {
    command: "cargo",
    args: ["clean", "--manifest-path", "src-tauri/Cargo.toml"],
  },
  {
    command: "git",
    args: ["clean", "-ffdX", "src-tauri/native"],
  },
  {
    command: "git",
    args: ["gc", "--prune=now"],
  },
];

function displayCommand({ command, args }) {
  return [command, ...args].join(" ");
}

if (!existsSync(resolve(repoRoot, "package.json"))) {
  console.error("clean-caches must be run from inside the video-creater repo.");
  process.exit(1);
}

for (const entry of commands) {
  console.log(`${dryRun ? "Would run" : "Running"}: ${displayCommand(entry)}`);

  if (dryRun) {
    continue;
  }

  const result = spawnSync(entry.command, entry.args, {
    cwd: repoRoot,
    stdio: "inherit",
  });

  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}
