import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { evaluateExactRustTest, parseFocusedArgs } from "./focused-development.mjs";
import { performanceEnvironment, writePerformanceReport } from "./performance-metrics.mjs";
import { runStep } from "./run-native-verification.mjs";
import { preparationOutcome } from "./prepare-desktop-development.mjs";

test("a clean repository records a false dirty marker while failed status lookup is unverified", async () => {
  const directory = await mkdtemp(join(tmpdir(), "video-creater-clean-metrics-"));
  try {
    assert.equal(performanceEnvironment("test", directory).dirty, "unverified");
    const initialized = spawnSync("git", ["init", directory], { encoding: "utf8" });
    assert.equal(initialized.error, undefined);
    assert.equal(initialized.status, 0);
    assert.equal(performanceEnvironment("test", directory).dirty, false);
    await writeFile(join(directory, "change.txt"), "change");
    assert.equal(performanceEnvironment("test", directory).dirty, true);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("helper failure marks unattempted commands as skipped rather than claiming complete execution", () => {
  const commands = [["helper-one"], ["helper-two"], ["helper-three"]];
  assert.deepEqual(preparationOutcome(commands, [{ status: "passed" }, { status: "failed" }]), {
    status: "failed",
    skippedStages: [{ command: ["helper-three"], status: "skipped", reason: "earlier-stage-failed" }],
  });
});

test("focused checks reject missing selectors and command injection options", () => {
  assert.throws(() => parseFocusedArgs(["native"]), /exact/);
  assert.throws(() => parseFocusedArgs(["native", "--ignored"]), /selector/);
  assert.throws(() => parseFocusedArgs(["frontend", "src/App.tsx"]), /test file/);
  assert.deepEqual(parseFocusedArgs(["native", "project::tests::commit"]), { lane: "native", selectors: ["project::tests::commit"] });
});

test("exact native checks cannot accept zero, ignored, multiple, failed or missing test summaries", () => {
  assert.equal(evaluateExactRustTest(0, "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out"), true);
  for (const output of ["", "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out", "test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 10 filtered out", "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out"]) assert.equal(evaluateExactRustTest(0, output), false);
  assert.equal(evaluateExactRustTest(101, "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out"), false);
});

test("bounded native verification rejects a successful subprocess that executes no selected tests", async () => {
  const cancellation = { activeChild: null, signal: null };
  const result = await runStep([process.execPath, "-e", "console.log('test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out')", "--", "--exact"], {}, cancellation);
  assert.equal(result.exitCode, 0);
  assert.equal(result.status, "failed");
});

test("timestamped metrics retain the latest and bounded history without deleting unrelated artifacts", async () => {
  const directory = await mkdtemp(join(tmpdir(), "video-creater-metrics-"));
  try {
    await writeFile(join(directory, "keep.json"), "keep");
    for (let index = 1; index <= 4; index++) await writePerformanceReport(directory, "fixture", { startedAt: `2026-10-01T00:00:0${index}.000Z`, sample: index }, 2);
    const files = await readdir(directory);
    assert.equal(files.filter((file) => /^fixture-\d/.test(file)).length, 2);
    assert.equal(JSON.parse(await readFile(join(directory, "fixture-latest.json"), "utf8")).sample, 4);
    assert.equal(await readFile(join(directory, "keep.json"), "utf8"), "keep");
  } finally { await rm(directory, { recursive: true, force: true }); }
});
