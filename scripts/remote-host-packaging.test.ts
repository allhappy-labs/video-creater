import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

import { HOST_FEATURES, packagePreflight, REQUIRED_PACKAGE_PATHS } from "./package-remote-host.mjs";

const repoRoot = resolve(import.meta.dirname, "..");

test("headless package is protocol-aligned and includes runtime, web, docs, and user service", () => {
  assert.equal(HOST_FEATURES, "web-host");
  assert.deepEqual(REQUIRED_PACKAGE_PATHS, [
    "bin/video-creater-host",
    "bin/video-creater-precompose-worker",
    "bin/video-creater-compatibility-decoder",
    "bin/video-creater-audio-enhance",
    "bin/video-creater-semantic-encoder",
    "bin/video-creater-speech",
    "bin/video-creater-mcp-server",
    "bin/video-creater-codex",
    "bin/codex-runtime/codex-path/rg",
    "web/index.html",
    "lib/Video Creater/render-runtime/manifest.json",
    "lib/Video Creater/speech-runtime/manifest.json",
    "share/video-creater-host.service",
    "share/remote-host.env.example",
    "share/remote-access.md",
    "manifest.json",
  ]);
  assert.equal(packagePreflight("/definitely/missing").ready, false);
  const packager = readFileSync(resolve(repoRoot, "scripts/package-remote-host.mjs"), "utf8");
  assert.match(packager, /video-creater-host"\), \["--codex-sidecar-smoke"\]/);
});

test("sample supervision is an unprivileged loopback-only user service", () => {
  const unit = readFileSync(resolve(repoRoot, "packaging/systemd/video-creater-host.service"), "utf8");
  assert.match(unit, /--bind 127\.0\.0\.1:4777/);
  assert.match(unit, /Restart=on-failure/);
  assert.match(unit, /StartLimitBurst=3/);
  assert.match(unit, /RuntimeDirectory=video-creater-host/);
  assert.match(unit, /StateDirectory=video-creater/);
  assert.match(unit, /StateDirectoryMode=0700/);
  assert.match(unit, /UMask=0077/);
  assert.match(unit, /NoNewPrivileges=true/);
  assert.match(unit, /ReadWritePaths=.*%h\/\.local\/state\/video-creater.*-%h\/\.codex/);
  assert.doesNotMatch(unit, /User=root|sudo|--allow-insecure-direct-bind/);
  const environment = readFileSync(resolve(repoRoot, "packaging/systemd/remote-host.env.example"), "utf8");
  assert.match(environment, /VIDEO_CREATER_PROJECT_ROOTS=\/home\/USER\/\.local\/state\/video-creater\/projects/);
  assert.doesNotMatch(environment, /\/Videos\//);
});

test("supervised host data and generation catalog caches stay in writable state", () => {
  const unit = readFileSync(resolve(repoRoot, "packaging/systemd/video-creater-host.service"), "utf8");
  assert.match(unit, /^ProtectHome=read-only$/m);
  const writable = unit.match(/^ReadWritePaths=([^\n]+)$/m)?.[1].split(/\s+/).filter((path) => !path.startsWith("-")) ?? [];
  for (const name of ["XDG_DATA_HOME", "XDG_CACHE_HOME"]) {
    const path = unit.match(new RegExp(`^Environment=${name}=([^\\n]+)$`, "m"))?.[1];
    assert.ok(path, `${name} must be configured for the read-only home service`);
    assert.ok(writable.some((root) => path.startsWith(`${root}/`)), `${name} must be under a writable state directory`);
  }
  const catalog = readFileSync(resolve(repoRoot, "src-tauri/src/generation/catalog.rs"), "utf8");
  assert.match(catalog, /var_os\("XDG_CACHE_HOME"\)/, "the production catalog must consume the configured cache root");
});
