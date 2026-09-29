import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

import { PREPARE_SCRIPT_BY_PLATFORM, prepareScriptFor } from "./tauri-dev.mjs";
import {
  deniedLibraries,
  missingPayload,
  REQUIRED_PAYLOAD,
  RELEASE_FEATURES,
} from "./build-linux-release.mjs";

const repoRoot = resolve(import.meta.dirname, "..");
const readJson = (path: string) => JSON.parse(readFileSync(resolve(repoRoot, path), "utf8"));
const packageJson = readJson("package.json");
const baseConfig = readJson("src-tauri/tauri.conf.json");
const linuxConfig = readJson("src-tauri/tauri.linux.conf.json");

test("desktop development prepares the host platform's helpers before tauri dev", () => {
  assert.equal(prepareScriptFor("darwin"), "prepare:tauri:dev");
  assert.equal(prepareScriptFor("linux"), "prepare:tauri:dev:linux");
  assert.throws(() => prepareScriptFor("win32"), /not supported/);
  for (const script of Object.values(PREPARE_SCRIPT_BY_PLATFORM)) {
    assert.ok(packageJson.scripts[script], script);
  }
});

test("Linux development never builds Apple helpers", () => {
  const linuxPrepare = packageJson.scripts["prepare:tauri:dev:linux"];
  assert.match(linuxPrepare, /^pnpm build:linux-media-runtime --package /);
  assert.doesNotMatch(linuxPrepare, /gstreamer-runtime|avfoundation|fluidaudio|build:audio-enhancer|build:semantic-encoder\b/);
  for (const script of linuxPrepare.split(" && ").map((step: string) => step.match(/^pnpm (\S+)/)?.[1]).filter(Boolean)) {
    assert.ok(packageJson.scripts[script], `missing package script ${script}`);
  }
});

test("Linux bundle replaces Apple sidecars and runtimes with Linux equivalents", () => {
  const sidecars: string[] = linuxConfig.bundle.externalBin;
  assert.ok(!sidecars.some((path) => /avfoundation|fluidaudio/.test(path)));
  assert.ok(sidecars.includes("binaries/video-creater-speech"));
  assert.ok(sidecars.includes("binaries/video-creater-compatibility-decoder"));
  const resources = { ...baseConfig.bundle.resources, ...linuxConfig.bundle.resources };
  const merged = Object.fromEntries(Object.entries(resources).filter(([, value]) => value !== null));
  assert.equal(merged["resources/render-runtime-package/"], "render-runtime/");
  assert.equal(merged["resources/speech-runtime/"], "speech-runtime/");
  assert.ok(!("resources/render-runtime/" in merged));
  assert.ok(!("resources/compatibility-runtime/" in merged));
  assert.deepEqual(linuxConfig.bundle.targets, ["deb"]);
  assert.doesNotMatch(linuxConfig.build.beforeBuildCommand, /gstreamer-runtime|avfoundation|fluidaudio/);
  assert.ok(linuxConfig.bundle.linux.deb.depends.some((dep: string) => dep.startsWith("libges-1.0-0")));
});

test("Linux release audit requires payload and rejects GPL media libraries", () => {
  assert.doesNotMatch(RELEASE_FEATURES, /coreml|temporal/);
  assert.deepEqual(missingPayload(REQUIRED_PAYLOAD.map((path: string) => `./${path}`)), []);
  assert.deepEqual(missingPayload([]), REQUIRED_PAYLOAD);
  assert.deepEqual(deniedLibraries(["libavcodec.so.60", "libavcodec_vc.so.60", "libx264.so.164", "libc.so.6"]), [
    "libavcodec.so.60",
    "libx264.so.164",
  ]);
});
