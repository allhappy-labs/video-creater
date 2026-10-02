import assert from "node:assert/strict";
import test from "node:test";
import { createServer } from "vite";
import { createPreviewWorkload } from "./preview-workloads.mjs";

test("deterministic representative workloads draw the advertised layers at a cut", async () => {
  const server = await createServer({ server: { middlewareMode: true, hmr: false, ws: false, watch: null }, appType: "custom", optimizeDeps: { noDiscovery: true, include: [] } });
  try {
    const { buildTimelinePreviewFrame } = await server.ssrLoadModule("/src/lib/timeline-preview.ts");
    for (const [scenario, layers, overlays, transitions, requiresPreparation] of [
      ["plain", 1, 0, 0, false], ["transitions", 2, 0, 1, false],
      ["nested", 2, 0, 1, false], ["captions", 1, 1, 0, false],
      ["many-media", 1, 0, 0, false], ["prepared-effects", 1, 0, 0, true],
    ]) {
      const fixture = createPreviewWorkload(scenario, 100);
      assert.deepEqual(fixture, createPreviewWorkload(scenario, 100));
      const frame = buildTimelinePreviewFrame({ ...fixture.input, playheadSeconds: 4 });
      assert.equal(frame.status, "ready", scenario);
      assert.deepEqual(frame.issues, [], scenario);
      assert.equal(frame.layers.length, layers, scenario);
      assert.equal(frame.overlayLayers.length, overlays, scenario);
      assert.equal(frame.transitions?.length ?? 0, transitions, scenario);
      assert.equal(frame.layers.some((layer) => layer.canonicalPreparationRequired), requiresPreparation, scenario);
    }
  } finally { await server.close(); }
});

test("workload selection is bounded and media dimensions reflect actual generated inputs", () => {
  assert.throws(() => createPreviewWorkload("unknown", 100), /scenario/);
  assert.throws(() => createPreviewWorkload("plain", 0), /clip count/);
  assert.throws(() => createPreviewWorkload("plain", 10001), /clip count/);
  const fixture = createPreviewWorkload("many-media", 1000);
  assert.equal(fixture.input.media.length, 1000);
  assert.equal(fixture.dimensions.videoClips, 1000);
  assert.equal(fixture.dimensions.durationSeconds, 4000);
});
