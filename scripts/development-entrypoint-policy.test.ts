import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

const repoRoot = resolve(import.meta.dirname, "..");
const packageJson = JSON.parse(
  readFileSync(resolve(repoRoot, "package.json"), "utf8"),
);
const tauriConfig = JSON.parse(
  readFileSync(resolve(repoRoot, "src-tauri/tauri.conf.json"), "utf8"),
);

test("desktop development is the public default and Vite remains an internal entrypoint", () => {
  assert.equal(packageJson.scripts.dev, "pnpm tauri:dev");
  assert.equal(
    packageJson.scripts["dev:web-runtime"],
    "vite --host 127.0.0.1",
  );
  assert.equal(packageJson.scripts["tauri:dev"], "node scripts/tauri-dev.mjs");
  assert.equal(
    tauriConfig.build.beforeDevCommand,
    "pnpm dev:web-runtime",
  );
});

test("development commands do not recurse", () => {
  assert.doesNotMatch(packageJson.scripts["tauri:dev"], /\bpnpm dev\b/);
  for (const script of [
    packageJson.scripts["prepare:tauri:dev"],
    packageJson.scripts["prepare:tauri:dev:linux"],
  ]) {
    assert.doesNotMatch(script, /(?:^|&& )pnpm (?:dev|tauri:dev)(?:\s|$)/);
  }
  assert.doesNotMatch(
    tauriConfig.build.beforeDevCommand,
    /\b(?:pnpm )?(?:dev|tauri:dev)\b(?!:web-runtime)/,
  );
});
