#!/usr/bin/env node
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "..");
const dataRoot = process.env.VIDEO_CREATER_REMOTE_E2E_DATA_DIR
  ?? resolve(tmpdir(), "video-creater-remote-host-e2e");
const projectRoot = resolve(dataRoot, "projects");
const port = process.env.VIDEO_CREATER_REMOTE_E2E_PORT ?? "4788";
const pairingCode = process.env.VIDEO_CREATER_REMOTE_E2E_PAIRING_CODE ?? "135790";
const installedRuntimeRoot = resolve(
  homedir(),
  ".local/share/com.olhapi.video-creater/render-runtime",
);
const discoveredRuntime = existsSync(installedRuntimeRoot)
  ? readdirSync(installedRuntimeRoot, { withFileTypes: true })
      .filter((entry) => entry.isDirectory() && entry.name.startsWith("linux-"))
      .map((entry) => resolve(installedRuntimeRoot, entry.name))
      .sort()
      .at(-1)
  : undefined;
const renderRuntime = process.env.VIDEO_CREATER_RENDER_RUNTIME_ROOT ?? discoveredRuntime;

if (!renderRuntime) {
  throw new Error(
    "No reviewed render runtime found. Set VIDEO_CREATER_RENDER_RUNTIME_ROOT before running the remote-host browser suite.",
  );
}

rmSync(dataRoot, { recursive: true, force: true });
mkdirSync(projectRoot, { recursive: true });

const child = spawn(
  "cargo",
  [
    "run",
    "--manifest-path", "src-tauri/Cargo.toml",
    "--features", "web-host",
    "--bin", "video-creater-host",
    "--",
    "--bind", `127.0.0.1:${port}`,
    "--assets-dir", "dist",
  ],
  {
    cwd: repoRoot,
    env: {
      ...process.env,
      TAURI_CONFIG: '{"bundle":{"externalBin":[],"resources":[]}}',
      VIDEO_CREATER_HOST_DATA_DIR: dataRoot,
      VIDEO_CREATER_PROJECT_ROOTS: projectRoot,
      VIDEO_CREATER_HOST_ORIGIN: `http://127.0.0.1:${port}`,
      VIDEO_CREATER_HOST_LABEL: "Remote E2E host",
      VIDEO_CREATER_REMOTE_E2E: "1",
      VIDEO_CREATER_REMOTE_E2E_PAIRING_CODE: pairingCode,
      VIDEO_CREATER_REMOTE_AGENT_FIXTURE: "1",
      VIDEO_CREATER_RENDER_RUNTIME_ROOT: renderRuntime,
    },
    stdio: "inherit",
  },
);

const stop = (signal) => {
  if (!child.killed) child.kill(signal);
};
process.once("SIGINT", () => stop("SIGINT"));
process.once("SIGTERM", () => stop("SIGTERM"));
child.once("exit", (code, signal) => {
  process.exitCode = signal ? 1 : (code ?? 1);
});
