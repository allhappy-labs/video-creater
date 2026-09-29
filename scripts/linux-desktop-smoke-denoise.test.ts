import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";

import { audioDenoiseEvidence } from "./linux-desktop-smoke-denoise.mjs";

const fingerprint = "4a9fb8b7fb0d656cf618271fa6f05d7a370d32dd3ad41a7fcd348ae2c0b1c707";
const entry = `cache/audio-denoise/v1/sha256/4a/${fingerprint}`;
const staging = `cache/audio-denoise/v1/sha256/4a/.${fingerprint}.staging-e547cda5-46e3-4579-ab8e-d17fcec9a5de`;

function write(root: string, path: string, contents: string) {
  const target = join(root, path);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, contents);
}

function projectDir() {
  return mkdtempSync(join(tmpdir(), "vc-smoke-denoise-"));
}

test("counts a committed denoise cache entry the render report lists", () => {
  const dir = projectDir();
  write(dir, `${entry}/manifest.json`, JSON.stringify({ algorithm: "deepfilternet3-onnx-tract-wet-dry-v1", noiseRmsBefore: 0.028, noiseRmsAfter: 0.012 }));
  write(dir, `${entry}/output.wav`, "wav");

  const evidence = audioDenoiseEvidence(dir, { artifacts: ["renders/r/output.webm", `${entry}/manifest.json`, `${entry}/output.wav`] });

  assert.deepEqual(evidence, [
    {
      manifest: `${entry}/manifest.json`,
      output: `${entry}/output.wav`,
      algorithm: "deepfilternet3-onnx-tract-wet-dry-v1",
      noiseRmsBefore: 0.028,
      noiseRmsAfter: 0.012,
    },
  ]);
});

test("ignores staging files, unlisted cache entries and listed entries missing from disk", () => {
  const dir = projectDir();
  write(dir, `${staging}/output.model-input.wav`, "wav");
  write(dir, `${entry}/manifest.json`, "{}");
  write(dir, `${entry}/output.wav`, "wav");

  assert.deepEqual(audioDenoiseEvidence(dir, { artifacts: [`${staging}/output.model-input.wav`, "renders/r/output.webm"] }), []);
  assert.deepEqual(audioDenoiseEvidence(dir, { artifacts: [`${entry}/manifest.json`] }), []);
  assert.deepEqual(audioDenoiseEvidence(projectDir(), { artifacts: [`${entry}/manifest.json`, `${entry}/output.wav`] }), []);
  assert.deepEqual(audioDenoiseEvidence(dir, {}), []);
});
