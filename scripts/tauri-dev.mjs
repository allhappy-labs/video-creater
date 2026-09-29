#!/usr/bin/env node
// Desktop development entrypoint. Prepares the host platform's native helpers, then starts
// `tauri dev` (which merges src-tauri/tauri.<platform>.conf.json automatically).

import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";

export const PREPARE_SCRIPT_BY_PLATFORM = {
  darwin: "prepare:tauri:dev",
  linux: "prepare:tauri:dev:linux",
};

export function prepareScriptFor(platform) {
  const script = PREPARE_SCRIPT_BY_PLATFORM[platform];
  if (!script) throw new Error(`desktop development is not supported on ${platform}`);
  return script;
}

function run(command, args) {
  const result = spawnSync(command, args, { stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  run("pnpm", [prepareScriptFor(process.platform)]);
  run("pnpm", ["exec", "tauri", "dev", ...process.argv.slice(2)]);
}
