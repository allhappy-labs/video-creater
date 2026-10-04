// Background processes of the Linux desktop smoke run (Xvfb, the Vite server, Temporal, tauri-driver).
// A process that fails to start rejects `failure` instead of crashing Node with an unhandled `error`
// event, so the run still stops everything it started and writes its evidence.

import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { redactSmokeMediaTokens } from "./linux-desktop-smoke-steps.mjs";

export function createProcessGroup({ cwd, env = process.env }) {
  const children = [];
  let rejectFailure;
  /** Rejects with the first start error. Race it against the run. */
  const failure = new Promise((_resolve, reject) => {
    rejectFailure = reject;
  });
  // Nobody awaits `failure` once the run has finished; keep a late rejection from being unhandled.
  failure.catch(() => {});

  function start(command, args, extraEnv = {}) {
    const child = spawn(command, args, {
      cwd,
      env: { ...env, ...extraEnv },
      stdio: ["ignore", "pipe", "pipe"],
      // Own process group so wrappers such as pnpm are stopped with their children.
      detached: true,
    });
    const entry = { command, child, logs: [], error: null };
    child.stdout?.on("data", (chunk) => entry.logs.push(chunk.toString()));
    child.stderr?.on("data", (chunk) => entry.logs.push(chunk.toString()));
    child.on("error", (error) => {
      entry.logs.push(`${error.message}\n`);
      if (entry.error) return;
      entry.error = error;
      rejectFailure(new Error(`${command} failed to start: ${error.message}`));
    });
    children.push(entry);
    return child;
  }

  /** Stops every started process group, newest first, and writes each one's log into `logDir`. Never throws. */
  function stopAll(logDir) {
    for (const { command, child, logs } of children.toReversed()) {
      if (child.pid !== undefined) {
        try {
          process.kill(-child.pid, "SIGTERM");
        } catch {
          try {
            child.kill("SIGTERM");
          } catch {
            // Already gone.
          }
        }
      }
      try {
        writeFileSync(join(logDir, `${command.split("/").at(-1)}.log`), redactSmokeMediaTokens(logs.join("")));
      } catch {
        // A missing log must not stop the rest of the cleanup.
      }
    }
  }

  /** The last `lines` output lines of every process that wrote any, oldest process first. */
  function logTails(lines = 20) {
    return children
      .map(({ command, logs }) => ({
        command: command.split("/").at(-1),
        tail: redactSmokeMediaTokens(logs.join("")).split("\n").filter((line) => line.trim() !== "").slice(-lines),
      }))
      .filter((entry) => entry.tail.length > 0);
  }

  return { children, failure, logTails, start, stopAll };
}

const fatalLine = /panicked at|Failed to setup app|error while loading shared libraries|Segmentation fault|core dumped/;

/**
 * The first process output that explains a run which stopped before its steps: a panic (with the
 * message on the line after it), a failed app setup, an unloadable library or a crash.
 */
export function fatalCause(tails) {
  for (const { command, tail } of tails) {
    const index = tail.findIndex((line) => fatalLine.test(line));
    if (index < 0) continue;
    const lines = /panicked at/.test(tail[index]) && tail[index + 1] ? [tail[index], tail[index + 1]] : [tail[index]];
    return `${command}: ${lines.map((line) => line.trim()).join(" ")}`;
  }
  return null;
}
