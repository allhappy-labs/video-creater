// Background processes of the Linux desktop smoke run (Xvfb, the Vite server, Temporal, tauri-driver).
// A process that fails to start rejects `failure` instead of crashing Node with an unhandled `error`
// event, so the run still stops everything it started and writes its evidence.

import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";
import { join } from "node:path";

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
        writeFileSync(join(logDir, `${command.split("/").at(-1)}.log`), logs.join(""));
      } catch {
        // A missing log must not stop the rest of the cleanup.
      }
    }
  }

  return { children, failure, start, stopAll };
}
