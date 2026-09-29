import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  browserVisualChannelThreshold,
  browserVisualThreshold,
  releaseBaselineFor,
} from "./browser-visual-release.mjs";

test("selects a dedicated Linux x64 baseline without replacing the original", () => {
  assert.deepEqual(releaseBaselineFor("linux", "x64"), {
    baseline: "docs/visual-qa/browser-visual-baseline-linux-x64",
    manifest: "docs/visual-qa/browser-visual-baseline-linux-x64-manifest.json",
  });
  assert.deepEqual(releaseBaselineFor("darwin", "arm64"), {
    baseline: "docs/visual-qa/browser-visual-baseline",
    manifest: "docs/visual-qa/browser-visual-baseline-manifest.json",
  });
});

test("keeps the reviewed comparison thresholds and strict mismatch gate", () => {
  assert.equal(browserVisualThreshold, 0.01);
  assert.equal(browserVisualChannelThreshold, 4);
  const source = readFileSync(
    new URL("./browser-visual-release.mjs", import.meta.url),
    "utf8",
  );
  assert.match(source, /--fail-on-mismatch/);
  assert.match(source, /--require-comparison/);
});
