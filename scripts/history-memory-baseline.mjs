#!/usr/bin/env node
import { createStore } from "zustand/vanilla";
import { createServer } from "vite";
import { createPreviewWorkload } from "./preview-workloads.mjs";
import { metricsDirectory, performanceEnvironment, writePerformanceReport } from "./performance-metrics.mjs";

const startedAt = new Date().toISOString();
const environment = performanceEnvironment("node-local-project-history-no-backend");
const server = await createServer({ server: { middlewareMode: true, hmr: false, ws: false, watch: null }, appType: "custom", optimizeDeps: { noDiscovery: true, include: [] } });
const workloads = [];
try {
  const { createProjectSlice } = await server.ssrLoadModule("/src/editor/store/project-slice.ts");
  for (const scenario of ["plain", "captions", "many-media"]) for (const size of [100, 1000, 5000]) {
    global.gc?.(); const heapBefore = process.memoryUsage().heapUsed;
    let fixture = createPreviewWorkload(scenario, size);
    let project = {
      schemaVersion: 2, id: `history-${scenario}-${size}`, name: "History memory fixture",
      createdAt: "2026-10-01T00:00:00Z", updatedAt: "2026-10-01T00:00:00Z",
      media: fixture.input.media, timeline: fixture.input.timeline,
      timelines: [{ id: "main", name: "Main", timeline: fixture.input.timeline }], activeTimelineId: "main",
      mediaFolders: [], generatedAssets: [], renderReports: [], jobs: [], transcripts: fixture.transcripts,
      renderSettings: { width: 1920, height: 1080, fps: 30, loudnessLufs: -14, captions: "burn_in" }, codexThreadId: null,
    };
    const dimensions = fixture.dimensions;
    const sourceSerializedBytes = Buffer.byteLength(JSON.stringify(project));
    // Empty projectDir guarantees the real slice's local path: no native/remote writes.
    // Other editor slices, React and media caches are intentionally excluded.
    let store = createStore((set, get) => ({ ...createProjectSlice({ projectDir: "", project })(set, get), clearHighlights() {}, pruneSelection() {} }));
    const spans = []; const samples = [];
    function sample(phase, edits) {
      global.gc?.();
      const history = store.getState().history;
      let bytes = 0;
      let logicalBytes = 0;
      for (const snapshot of [...history.past, ...history.future]) {
        const serialized = JSON.stringify(snapshot);
        bytes += Buffer.byteLength(serialized);
        logicalBytes += serialized.length * 2;
      }
      global.gc?.();
      samples.push({ phase, edits, past: history.past.length, future: history.future.length, heapUsedBytes: process.memoryUsage().heapUsed, historySerializedBytes: bytes, historyLogicalUtf16Bytes: logicalBytes });
    }
    sample("initial", 0);
    for (let edit = 1; edit <= 125; edit++) {
      const start = performance.now();
      const changed = await store.getState().applyActions([{ type: "updateVisualClipOpacity", itemId: "clip-0", opacity: edit % 2 ? 0.8 : 0.9 }]);
      if (!changed || store.getState().saveStatus !== "unsaved") throw new Error("Local history edit did not succeed");
      spans.push(performance.now() - start);
      if ([25, 50, 100, 125].includes(edit)) sample("edits", edit);
    }
    const retainedSnapshots = store.getState().history.past.length;
    if (retainedSnapshots < 1 || retainedSnapshots > 100) throw new Error("History retention policy was not exercised");
    const undoOperations = Math.min(50, retainedSnapshots);
    for (let index = 0; index < undoOperations; index++) await store.getState().undo();
    sample(`undo-${undoOperations}`, 125);
    if (store.getState().history.past.length !== retainedSnapshots - undoOperations || store.getState().history.future.length !== undoOperations) throw new Error("Undo did not move the expected history snapshots");
    const redoOperations = Math.min(25, undoOperations);
    for (let index = 0; index < redoOperations; index++) await store.getState().redo();
    sample(`redo-${redoOperations}`, 125);
    if (store.getState().history.past.length !== retainedSnapshots - undoOperations + redoOperations || store.getState().history.future.length !== undoOperations - redoOperations) throw new Error("Redo did not move the expected history snapshots");
    store = null; project = null; fixture = null; global.gc?.();
    spans.sort((left, right) => left - right);
    workloads.push({ scenario, dimensions, edits: 125, retainedSnapshots, undoOperations, redoOperations, sourceSerializedBytes, p50EditMilliseconds: spans[Math.floor(spans.length * 0.5)], p95EditMilliseconds: spans[Math.floor(spans.length * 0.95)], memory: { gcAvailable: Boolean(global.gc), heapBefore, heapAfterRelease: process.memoryUsage().heapUsed, samples } });
    console.log(`${scenario}/${size}: retained history ${(samples.find((entry) => entry.edits === 125).historySerializedBytes / 1024 ** 2).toFixed(1)} MiB serialized`);
  }
  const report = { schemaVersion: 1, startedAt, completedAt: new Date().toISOString(), status: "measured", environment, limitations: ["Actual Zustand project-history slice in Node, local edits only; excludes browser/React/selection/jobs/media caches and persisted undo", "Serialized byte totals are representation sizes, not heap byte estimates", "Collection samples are Node heap observations, not packaged/editor-memory acceptance"], workloads };
  console.log(`history memory metrics: ${await writePerformanceReport(metricsDirectory, "history-memory", report)}`);
} finally { await server.close(); }
