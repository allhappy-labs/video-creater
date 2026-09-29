import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
// @ts-expect-error The repository does not generate declarations for scripts/*.mjs.
import { refreshManifest, pngDimensions } from "./refresh-browser-visual-baseline-manifest.mjs";

function png(width: number, height: number): Buffer {
  const buffer = Buffer.alloc(33);
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]).copy(buffer, 0);
  buffer.writeUInt32BE(13, 8);
  buffer.write("IHDR", 12, "ascii");
  buffer.writeUInt32BE(width, 16);
  buffer.writeUInt32BE(height, 20);
  return buffer;
}

test("reads PNG dimensions from the IHDR chunk", () => {
  assert.deepEqual(pngDimensions(png(1440, 960)), { width: 1440, height: 960 });
});

test("rewrites screenshots and artifacts for the requested shots and keeps metadata", () => {
  const dir = mkdtempSync(join(tmpdir(), "baseline-"));
  writeFileSync(join(dir, "home-desktop.png"), png(1440, 960));
  writeFileSync(join(dir, "settings-narrow.png"), png(390, 844));
  const manifestPath = join(dir, "manifest.json");
  writeFileSync(
    manifestPath,
    JSON.stringify({
      schemaVersion: 2,
      description: "d",
      screenshots: ["home-desktop.png", "editor-desktop.png"],
      platformThresholds: { "linux-x64": { threshold: 0.01, channelThreshold: 4 } },
      artifacts: { "editor-desktop.png": { width: 1, height: 1, bytes: 1, sha256: "x" } },
      environment: { os: "o" },
    }),
  );

  refreshManifest({
    baselineDir: dir,
    manifestPath,
    screenshots: ["home-desktop.png", "settings-narrow.png"],
  });

  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  assert.deepEqual(manifest.screenshots, ["home-desktop.png", "settings-narrow.png"]);
  assert.deepEqual(Object.keys(manifest.artifacts), ["home-desktop.png", "settings-narrow.png"]);
  assert.equal(manifest.artifacts["home-desktop.png"].width, 1440);
  assert.equal(manifest.artifacts["settings-narrow.png"].bytes, 33);
  assert.match(manifest.artifacts["home-desktop.png"].sha256, /^[0-9a-f]{64}$/);
  assert.deepEqual(manifest.platformThresholds, { "linux-x64": { threshold: 0.01, channelThreshold: 4 } });
  assert.deepEqual(manifest.environment, { os: "o" });
});

test("fails when a requested screenshot file is missing", () => {
  const dir = mkdtempSync(join(tmpdir(), "baseline-"));
  const manifestPath = join(dir, "manifest.json");
  writeFileSync(manifestPath, JSON.stringify({ schemaVersion: 2, screenshots: [], artifacts: {} }));
  assert.throws(
    () => refreshManifest({ baselineDir: dir, manifestPath, screenshots: ["missing.png"] }),
    /Missing baseline screenshot .*missing\.png/,
  );
});
