import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { poll } from "./linux-desktop-smoke-driver.mjs";
import { runTemporalUnavailableSteps, temporalUnavailableStepNames } from "./linux-desktop-smoke-temporal-unavailable.mjs";

const unavailable = {
  status: "unavailable",
  runId: null,
  message: "Temporal runtime is unavailable in this build.",
};

/**
 * Runs both steps against a fake app. `fallback` makes the fake behave like a build that silently
 * renders in the desktop process: the export leaves an artifact and a file.
 */
async function run({ featureEnabled = false, fallback = false, reasonShown = true } = {}) {
  const projectDir = mkdtempSync(join(tmpdir(), "vc-smoke-temporal-unavailable-"));
  const preferences = { generationExecutionBackend: "inProcess" };
  const patches: unknown[] = [];
  let exported = false;
  const job = { id: "export-mp4H264-test", kind: "export_media", status: fallback ? "completed" : "failed", workflow: { runId: null } };
  const driver = {
    poll: (read: () => unknown, accept: (value: unknown) => boolean, _timeoutMs: number, intervalMs?: number) => poll(read, accept, 200, Math.min(intervalMs ?? 10, 10)),
    sleep: async () => undefined,
    find: async () => "element",
    screenshot: async (name: string) => `${name}.png`,
    execute: async (script: string) => {
      if (script.includes("Temporal runtime is unavailable")) return reasonShown && !fallback;
      if (script.includes("Background tasks")) return fallback ? "Export complete" : "Export failed";
      // Settings and the export popover are already open; the project home is gone.
      return true;
    },
    click: async (xpath: string) => {
      if (xpath.includes("option[@value='temporal']")) preferences.generationExecutionBackend = "temporal";
      if (xpath.includes("Export video")) {
        exported = true;
        if (fallback) {
          mkdirSync(join(projectDir, "exports"), { recursive: true });
          writeFileSync(join(projectDir, "exports", "Sample.mp4"), "mp4");
        }
      }
    },
    invoke: async (command: string, args: { patch?: { generationExecutionBackend: string } } = {}) => {
      if (command === "get_temporal_worker_environment_report") return { featureEnabled, ready: featureEnabled, featureName: "temporal-worker" };
      if (command === "get_app_preferences") return preferences;
      if (command === "update_app_preferences") {
        patches.push(args.patch);
        Object.assign(preferences, args.patch);
        return preferences;
      }
      if (command === "load_split_project_from_folder") {
        return {
          jobs: exported ? [job] : [],
          exportArtifacts: exported && fallback ? [{ id: job.id, kind: "mp4", path: "exports/Sample.mp4" }] : [],
        };
      }
      if (command === "start_temporal_workflow") return unavailable;
      throw new Error(`unexpected command ${command}`);
    },
  };
  const results: Record<string, { detail?: unknown; error?: Error }> = {};
  const step = async (name: string, action: () => Promise<unknown>) => {
    try {
      results[name] = { detail: await action() };
    } catch (error) {
      results[name] = { error: error as Error };
    }
  };
  try {
    await runTemporalUnavailableSteps({ driver, step, projectDir, settleMs: 0 });
  } finally {
    rmSync(projectDir, { recursive: true, force: true });
  }
  return { results, patches, preferences };
}

test("a build without the Temporal feature refuses the export and restores desktop execution", async () => {
  const { results, patches, preferences } = await run();
  const [reportStep, exportStep] = temporalUnavailableStepNames;
  assert.deepEqual(results[reportStep].detail, { featureEnabled: false, ready: false, featureName: "temporal-worker" });
  assert.equal(results[exportStep].error, undefined, String(results[exportStep].error));
  const detail = results[exportStep].detail as { startResult: unknown; jobs: unknown[]; newArtifacts: unknown[]; newFiles: unknown[]; reasonShown: boolean; tasksLabel: string };
  assert.deepEqual(detail.startResult, unavailable);
  assert.deepEqual(detail.jobs, [{ id: "export-mp4H264-test", kind: "export_media", status: "failed", runId: null }]);
  assert.deepEqual([detail.newArtifacts, detail.newFiles], [[], []]);
  assert.equal(detail.reasonShown, true);
  assert.equal(detail.tasksLabel, "Export failed");
  assert.deepEqual(patches, [{ generationExecutionBackend: "inProcess" }]);
  assert.equal(preferences.generationExecutionBackend, "inProcess");
});

test("a build that can run Temporal workflows fails the feature check", async () => {
  const { results } = await run({ featureEnabled: true });
  assert.match(String(results[temporalUnavailableStepNames[0]].error), /can run Temporal workflows \(use --temporal instead\)/);
});

test("a refusal that is not explained in the editor fails the step", async () => {
  const { results, preferences } = await run({ reasonShown: false });
  assert.match(String(results[temporalUnavailableStepNames[1]].error), /the unavailable reason is not shown in the editor/);
  assert.equal(preferences.generationExecutionBackend, "inProcess");
});

test("a build that silently renders in the desktop process fails the step", async () => {
  const { results } = await run({ fallback: true });
  const error = String(results[temporalUnavailableStepNames[1]].error);
  assert.match(error, /timed out|the export produced output without a Temporal worker/);
});
