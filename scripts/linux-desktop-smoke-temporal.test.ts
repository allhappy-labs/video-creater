import assert from "node:assert/strict";
import test from "node:test";

import { poll } from "./linux-desktop-smoke-driver.mjs";
import { runTemporalSteps, temporalStepNames } from "./linux-desktop-smoke-temporal.mjs";

/** Runs only the "select Temporal execution" step against a fake driver whose saved backend follows `savedBackends`. */
async function selectTemporalExecution(savedBackends: string[]) {
  const clicks: string[] = [];
  let reads = 0;
  const driver = {
    poll: (read: () => unknown, accept: (value: unknown) => boolean, _timeoutMs: number, intervalMs?: number) => poll(read, accept, 200, Math.min(intervalMs ?? 10, 10)),
    sleep: async () => undefined,
    // Settings are already open on the Advanced tab.
    execute: async () => true,
    click: async (xpath: string) => void clicks.push(xpath),
    screenshot: async (name: string) => `${name}.png`,
    invoke: async (command: string) => {
      assert.equal(command, "get_app_preferences");
      const backend = savedBackends[Math.min(reads, savedBackends.length - 1)];
      reads += 1;
      return { generationExecutionBackend: backend };
    },
  };
  let result: unknown;
  const step = async (name: string, action: () => Promise<unknown>) => {
    if (name === temporalStepNames[1]) result = await action();
  };
  await runTemporalSteps({ driver, step, projectPath: "/tmp/unused", outDir: "/tmp/unused" });
  return { clicks, reads, result };
}

test("selecting Temporal execution waits until the preference is saved", async () => {
  const { clicks, reads, result } = await selectTemporalExecution(["inProcess", "inProcess", "temporal"]);
  assert.ok(clicks.some((xpath) => xpath.includes("option[@value='temporal']")), clicks.join("\n"));
  assert.equal(reads, 3);
  assert.deepEqual(result, { generationExecutionBackend: "temporal", screenshot: "09-advanced-temporal.png" });
});

test("selecting Temporal execution fails when the saved preference does not change", async () => {
  await assert.rejects(selectTemporalExecution(["inProcess"]), /generation execution backend is still "inProcess", not "temporal"/);
});
