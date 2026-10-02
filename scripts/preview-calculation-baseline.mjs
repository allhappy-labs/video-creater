#!/usr/bin/env node
import { performance } from "node:perf_hooks";
import { createServer } from "vite";
import { performanceEnvironment, metricsDirectory, writePerformanceReport } from "./performance-metrics.mjs";
import { createPreviewWorkload, PREVIEW_SCENARIOS } from "./preview-workloads.mjs";

function parseArgs(argv) {
  const options = { sizes: [100, 1000, 5000], scenarios: [...PREVIEW_SCENARIOS], samples: 100, warmups: 20, maximumWorkloadMilliseconds: 30000 };
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index]; const value = argv[index + 1];
    if (!value) throw new Error(`Missing value for ${flag}`);
    if (flag === "--sizes") options.sizes = value.split(",").map(Number);
    else if (flag === "--scenarios") options.scenarios = value.split(",");
    else if (flag === "--samples") options.samples = Number(value);
    else if (flag === "--max-workload-ms") options.maximumWorkloadMilliseconds = Number(value);
    else throw new Error(`Unknown argument: ${flag}`);
  }
  if (!Number.isInteger(options.samples) || options.samples < 100 || options.samples > 10000) throw new Error("--samples must be between 100 and 10000");
  if (!Number.isInteger(options.maximumWorkloadMilliseconds) || options.maximumWorkloadMilliseconds < 1000 || options.maximumWorkloadMilliseconds > 600000) throw new Error("--max-workload-ms must be between 1000 and 600000");
  for (const scenario of options.scenarios) for (const size of options.sizes) createPreviewWorkload(scenario, size);
  return options;
}

const options = parseArgs(process.argv.slice(2));
const startedAt = new Date().toISOString();
const environment = performanceEnvironment("node-preview-calculation-only");
const server = await createServer({ server: { middlewareMode: true, hmr: false, ws: false, watch: null }, appType: "custom", optimizeDeps: { noDiscovery: true, include: [] } });
const workloads = [];
try {
  const { buildTimelinePreviewFrame } = await server.ssrLoadModule("/src/lib/timeline-preview.ts");
  for (const scenario of options.scenarios) for (const size of options.sizes) {
    global.gc?.();
    const heapBeforeFixture = process.memoryUsage().heapUsed;
    let fixture = createPreviewWorkload(scenario, size);
    global.gc?.();
    const heapWithFixture = process.memoryUsage().heapUsed;
    const serializedBytes = Buffer.byteLength(JSON.stringify(fixture));
    const durations = []; const warmupDurations = []; let peakHeapBytes = heapWithFixture;
    const memoryCycles = [];
    const workloadStarted = performance.now();
    for (let iteration = 0; iteration < options.samples + options.warmups; iteration++) {
      // Alternate cut centers with clip interiors; transition and nested paths are exercised.
      const index = 1 + iteration % (size - 1);
      const playheadSeconds = index * 4 + (iteration % 2 === 0 ? 0 : 0.75);
      const start = performance.now();
      const frame = buildTimelinePreviewFrame({ ...fixture.input, playheadSeconds });
      const elapsed = performance.now() - start;
      if (frame.status !== "ready" || frame.issues.length) throw new Error(`Invalid ${scenario}/${size} frame: ${JSON.stringify(frame.issues)}`);
      if (iteration >= options.warmups) durations.push(elapsed);
      else warmupDurations.push(elapsed);
      peakHeapBytes = Math.max(peakHeapBytes, process.memoryUsage().heapUsed);
      if ((iteration + 1) % 20 === 0) { global.gc?.(); memoryCycles.push({ calls: iteration + 1, heapUsedBytes: process.memoryUsage().heapUsed }); }
      if (performance.now() - workloadStarted > options.maximumWorkloadMilliseconds) break;
    }
    durations.sort((left, right) => left - right);
    const dimensions = fixture.dimensions;
    fixture = null;
    global.gc?.();
    workloads.push({ scenario, dimensions, status: durations.length === options.samples ? "measured" : "partial-time-budget", samples: durations.length, requestedSamples: options.samples, warmups: warmupDurations.length, firstCalculationMilliseconds: warmupDurations[0] ?? null, warmupMaximumMilliseconds: warmupDurations.length ? Math.max(...warmupDurations) : null, phase: "frame-calculation", p50Milliseconds: durations[Math.floor(durations.length * 0.5)] ?? null, p95Milliseconds: durations[Math.floor(durations.length * 0.95)] ?? null, elapsedMilliseconds: performance.now() - workloadStarted, memory: { gcAvailable: Boolean(global.gc), heapBeforeFixture, heapWithFixture, peakHeapBytes, heapAfterRelease: process.memoryUsage().heapUsed, serializedBytes, cycles: memoryCycles } });
    console.log(`${scenario}/${size}: ${durations.length}/${options.samples} samples, p50 ${workloads.at(-1).p50Milliseconds?.toFixed(3) ?? "unmeasured"} ms, p95 ${workloads.at(-1).p95Milliseconds?.toFixed(3) ?? "unmeasured"} ms`);
  }
  const report = { schemaVersion: 1, startedAt, completedAt: new Date().toISOString(), status: workloads.every((workload) => workload.status === "measured") ? "measured" : "partial", environment, maximumWorkloadMilliseconds: options.maximumWorkloadMilliseconds, limitations: ["Node calculation and heap only; excludes React, paint, decoder, native rendering, network and editor history", "Prepared effects represent preparation-required inputs, not decoded prepared playback", "Retained heap samples are observations, not a browser stability or performance gate", "Time budget is checked between frame calculations; an individual pathological calculation can exceed it", "Partial workloads must not be accepted as 100-observation baselines"], workloads };
  const path = await writePerformanceReport(metricsDirectory, "preview-calculation", report);
  console.log(`preview calculation metrics: ${path}`);
} finally { await server.close(); }
