import assert from "node:assert/strict";
import { resolve } from "node:path";
import test from "node:test";

import {
  configuredReleaseExecutables,
  evaluatePackagedRuntimeRecords,
} from "./release-runtime-policy.mjs";

const repoRoot = resolve(import.meta.dirname, "..");

test("release source has no package-manager runtime behavior", async () => {
  const { evaluateSourceQuality } = await import("./source-quality-policy.mjs");
  assert.deepEqual(evaluateSourceQuality(repoRoot), []);
});

test("packaged runtime resolves dependencies inside the app or Apple system roots", () => {
  const requiredExecutables = new Set(["Video Creater", "video-creater-mcp-server"]);
  const records = [
    {
      path: "Contents/MacOS/Video Creater",
      dependencies: ["/usr/lib/libSystem.B.dylib", "@rpath/libgstreamer-1.0.0.dylib"],
      rpaths: ["@executable_path/../Resources/render-runtime/lib"],
    },
    {
      path: "Contents/MacOS/video-creater-mcp-server",
      dependencies: ["/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation"],
      rpaths: [],
    },
    {
      path: "Contents/Resources/render-runtime/lib/libgstreamer-1.0.0.dylib",
      dependencies: ["/usr/lib/libSystem.B.dylib"],
      rpaths: ["@loader_path"],
    },
  ];
  assert.deepEqual(evaluatePackagedRuntimeRecords({ records, requiredExecutables }), []);
});

test("packaged runtime rejects external dependencies and unclassified executables", () => {
  const failures = evaluatePackagedRuntimeRecords({
    requiredExecutables: new Set(["Video Creater"]),
    records: [
      {
        path: "Contents/MacOS/Video Creater",
        dependencies: ["/opt/homebrew/lib/libgstvideo-1.0.0.dylib"],
        rpaths: ["/usr/local/lib"],
      },
      {
        path: "Contents/MacOS/debug-helper",
        dependencies: ["@rpath/libmissing.dylib"],
        rpaths: ["@executable_path/../Resources/render-runtime/lib"],
      },
    ],
  });
  assert.ok(failures.some((failure) => failure.includes("unclassified packaged executable")));
  assert.ok(failures.some((failure) => failure.includes("/opt/homebrew/")));
  assert.ok(failures.some((failure) => failure.includes("/usr/local/")));
  assert.ok(failures.some((failure) => failure.includes("unresolved packaged dependency")));
});

test("release executable inventory comes from the Tauri bundle contract", () => {
  const executables = configuredReleaseExecutables({ repoRoot });
  assert.ok(executables.has("Video Creater"));
  assert.ok(executables.has("video-creater-compatibility-decoder"));
  assert.ok(executables.has("video-creater-avfoundation-exporter"));
  assert.ok(executables.has("video-creater-mcp-server"));
  assert.ok(executables.has("video-creater-codex"));
});
