import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import test from "node:test";

import { createProcessGroup } from "./linux-desktop-smoke-processes.mjs";

const repoRoot = resolve(import.meta.dirname, "..");

function withDir(run: (dir: string) => Promise<void> | void) {
  const dir = mkdtempSync(join(tmpdir(), "vc-smoke-processes-"));
  return Promise.resolve(run(dir)).finally(() => rmSync(dir, { recursive: true, force: true }));
}

test("a process that fails to start rejects the group's failure instead of crashing, and earlier processes still stop", async () => {
  await withDir(async (dir) => {
    const group = createProcessGroup({ cwd: dir });
    // A process started earlier (like Xvfb) must still be stopped after the later one fails.
    const earlier = group.start("sleep", ["30"]);
    const earlierExit = new Promise((resolveExit) => earlier.once("exit", (_code, signal) => resolveExit(signal)));
    group.start("vc-smoke-missing-binary", ["--flag"]);
    await assert.rejects(group.failure, /vc-smoke-missing-binary failed to start: spawn vc-smoke-missing-binary ENOENT/);
    group.stopAll(dir);
    assert.equal(await earlierExit, "SIGTERM");
    assert.match(readFileSync(join(dir, "vc-smoke-missing-binary.log"), "utf8"), /ENOENT/);
  });
});

test("stopAll stops running process groups and records their output", async () => {
  await withDir(async (dir) => {
    const group = createProcessGroup({ cwd: dir });
    const child = group.start("sh", ["-c", "echo started; exec sleep 30"]);
    await new Promise((resolveOutput) => child.stdout!.once("data", resolveOutput));
    const exited = new Promise((resolveExit) => child.once("exit", (_code, signal) => resolveExit(signal)));
    group.stopAll(dir);
    assert.equal(await exited, "SIGTERM");
    assert.equal(readFileSync(join(dir, "sh.log"), "utf8"), "started\n");
  });
});

test("the smoke run writes evidence.json when a background process cannot start", async () => {
  await withDir((dir) => {
    const out = join(dir, "out");
    // Only node is on PATH, so Xvfb (the first process the run starts) is missing.
    const run = spawnSync(process.execPath, ["scripts/linux-desktop-smoke.mjs", "--out", out, "--app", join(dir, "missing-app")], {
      cwd: repoRoot,
      env: { PATH: dirname(process.execPath), HOME: dir, XDG_DATA_HOME: join(dir, "data") },
      encoding: "utf8",
      timeout: 60_000,
    });
    assert.equal(run.status, 1, run.stderr);
    assert.ok(existsSync(join(out, "evidence.json")), `no evidence.json; stderr: ${run.stderr}`);
    const evidence = JSON.parse(readFileSync(join(out, "evidence.json"), "utf8"));
    assert.match(evidence.fatal, /Xvfb failed to start: spawn Xvfb ENOENT/);
    assert.match(readFileSync(join(out, "Xvfb.log"), "utf8"), /ENOENT/);
  });
});


test("retained child-process logs redact local media authorization tokens", async () => {
  await withDir(async (dir) => {
    const token = "b".repeat(64);
    const url = `http://127.0.0.1:4790/media/${token}/sample.mp4`;
    const group = createProcessGroup({ cwd: dir });
    const child = group.start(process.execPath, ["-e", `process.stdout.write(${JSON.stringify(url)}); setTimeout(() => {}, 30000);`]);
    await new Promise((resolveOutput) => child.stdout!.once("data", resolveOutput));
    const exited = new Promise((resolveExit) => child.once("exit", resolveExit));
    group.stopAll(dir);
    await exited;
    const log = readFileSync(join(dir, process.execPath.split("/").at(-1)! + ".log"), "utf8");
    assert.ok(!log.includes(token), "retained process log contains a media token");
    assert.equal(log, "http://127.0.0.1:4790/media/[redacted]/sample.mp4");
  });
});
