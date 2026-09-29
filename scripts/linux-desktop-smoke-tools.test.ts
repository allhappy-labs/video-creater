import assert from "node:assert/strict";
import test from "node:test";

import { extractedToolEnvironment, extractedToolPath, xScreenshotArgs } from "./linux-desktop-smoke-tools.mjs";

test("extractedToolEnvironment prepends the extracted library directory without mutating its input", () => {
  const env = { LD_LIBRARY_PATH: "/x", HOME: "/home/me" };
  const result = extractedToolEnvironment("/r", env, { DISPLAY: ":94" });
  assert.equal(result.LD_LIBRARY_PATH, "/r/usr/lib/x86_64-linux-gnu:/x");
  assert.equal(result.DISPLAY, ":94");
  assert.equal(result.HOME, "/home/me");
  assert.deepEqual(env, { LD_LIBRARY_PATH: "/x", HOME: "/home/me" });
});

test("extractedToolEnvironment sets the library directory alone when none was set", () => {
  assert.equal(extractedToolEnvironment("/r", {}, {}).LD_LIBRARY_PATH, "/r/usr/lib/x86_64-linux-gnu");
});

test("extractedToolPath points into the extracted usr/bin", () => {
  assert.equal(extractedToolPath("/r", "xdotool"), "/r/usr/bin/xdotool");
});

test("xScreenshotArgs captures one X frame as a PNG", () => {
  assert.deepEqual(xScreenshotArgs(":94", "/o/a.png"), [
    "ximagesrc",
    "display-name=:94",
    "use-damage=false",
    "num-buffers=1",
    "!",
    "videoconvert",
    "!",
    "pngenc",
    "!",
    "filesink",
    "location=/o/a.png",
  ]);
});
