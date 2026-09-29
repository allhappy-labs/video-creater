// Test-only tools extracted from Ubuntu .deb packages with `dpkg-deb -x` (xdotool, gnome-keyring,
// gstreamer1.0-tools). They run from their extraction root without being installed or linked into the
// app.

import { readdirSync, readlinkSync } from "node:fs";
import { join, resolve } from "node:path";

/** The environment for a tool extracted under `root`: its libraries first, plus `extra`. */
export function extractedToolEnvironment(root, env, extra = {}) {
  const libraries = join(root, "usr/lib/x86_64-linux-gnu");
  return {
    ...env,
    LD_LIBRARY_PATH: env.LD_LIBRARY_PATH ? `${libraries}:${env.LD_LIBRARY_PATH}` : libraries,
    ...extra,
  };
}

export function extractedToolPath(root, name) {
  return join(root, "usr/bin", name);
}

/** gst-launch-1.0 arguments that save one frame of an X display as a PNG. */
export function xScreenshotArgs(display, path) {
  return ["ximagesrc", `display-name=${display}`, "use-damage=false", "num-buffers=1", "!", "videoconvert", "!", "pngenc", "!", "filesink", `location=${path}`];
}

/**
 * Stops daemonized processes started from an extraction root (gnome-keyring-daemon detaches from the
 * smoke run and would outlive its private session bus). Returns the stopped pids.
 */
export function stopExtractedToolProcesses(root) {
  const prefix = `${resolve(root)}/`;
  const stopped = [];
  for (const pid of readdirSync("/proc").filter((entry) => /^\d+$/.test(entry))) {
    try {
      if (!readlinkSync(`/proc/${pid}/exe`).startsWith(prefix)) continue;
      process.kill(Number(pid), "SIGTERM");
      stopped.push(Number(pid));
    } catch {
      // The process exited or belongs to another user.
    }
  }
  return stopped;
}
