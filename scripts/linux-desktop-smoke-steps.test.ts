import assert from "node:assert/strict";
import { resolve } from "node:path";
import test from "node:test";

import {
  createStepRunner,
  exitCodeFor,
  parseSmokeOptions,
  skipped,
  smokeRunContext,
  summarizeSteps,
} from "./linux-desktop-smoke-steps.mjs";

test("parseSmokeOptions keeps today's defaults", () => {
  const options = parseSmokeOptions([]);
  assert.equal(options.app, resolve("src-tauri/target/debug/video-creater"));
  assert.equal(options.display, ":94");
  assert.equal(options.out, resolve("output/linux-desktop-smoke"));
  assert.equal(options.tauriDriver, "tauri-driver");
  assert.equal(options.nativeDriver, "WebKitWebDriver");
  assert.deepEqual(options.only, []);
  assert.equal(options.temporal, false);
  assert.equal(options.temporalUnavailable, false);
  assert.equal(options.nativeMenu, false);
  assert.equal(options.agentFlows, false);
});

test("parseSmokeOptions splits --only and reads boolean and value flags", () => {
  const options = parseSmokeOptions([
    "--",
    "--only",
    "a, b",
    "--dev-server",
    "--fake-audio",
    "--temporal",
    "--native-menu",
    "--agent-flows",
    "--keyring-root",
    "/k",
    "--temporal-cli",
    "/t/temporal",
    "--temporal-worker",
    "/t/worker",
    "--xdotool-root",
    "/x",
    "--gst-tools-root",
    "/g",
    "--release-report",
    "/r/report.json",
  ]);
  assert.deepEqual(options.only, ["a", "b"]);
  for (const flag of ["devServer", "fakeAudio", "temporal", "nativeMenu", "agentFlows"] as const) {
    assert.equal(options[flag], true, flag);
  }
  assert.equal(options.keyringRoot, "/k");
  assert.equal(options.temporalCli, "/t/temporal");
  assert.equal(options.temporalWorker, "/t/worker");
  assert.equal(options.xdotoolRoot, "/x");
  assert.equal(options.gstToolsRoot, "/g");
  assert.equal(options.releaseReport, "/r/report.json");
});

test("parseSmokeOptions ignores a leading -- from pnpm", () => {
  assert.deepEqual(parseSmokeOptions(["--", "--display", ":99"]).display, ":99");
});

test("parseSmokeOptions requires --xdotool-root for --native-menu", () => {
  assert.throws(() => parseSmokeOptions(["--native-menu"]), /--native-menu needs --xdotool-root/);
});

function runner(only: string[] = []) {
  const evidence = { steps: [] as Record<string, unknown>[] };
  const failures: string[] = [];
  const step = createStepRunner({ evidence, only, onFailure: async (name: string) => failures.push(name) ?? "shot.png", log: () => {} });
  return { evidence, failures, step };
}

test("a resolved step records passed with its detail and duration", async () => {
  const { evidence, step } = runner();
  const detail = await step("works", async () => ({ ok: 1 }));
  assert.deepEqual(detail, { ok: 1 });
  const [entry] = evidence.steps;
  assert.equal(entry.name, "works");
  assert.equal(entry.status, "passed");
  assert.deepEqual(entry.detail, { ok: 1 });
  assert.equal(typeof entry.ms, "number");
});

test("a thrown step records failed and asks for a failure screenshot", async () => {
  const { evidence, failures, step } = runner();
  assert.equal(await step("breaks", async () => Promise.reject(new Error("boom"))), undefined);
  const [entry] = evidence.steps;
  assert.equal(entry.status, "failed");
  assert.match(String(entry.error), /boom/);
  assert.deepEqual(failures, ["breaks"]);
});

test("a step that returns skipped(reason) records skipped with the reason", async () => {
  const { evidence, step } = runner();
  assert.equal(await step("keyring", async () => skipped("no --keyring-root")), undefined);
  assert.deepEqual(
    evidence.steps.map(({ name, status, reason }) => ({ name, status, reason })),
    [{ name: "keyring", status: "skipped", reason: "no --keyring-root" }],
  );
});

test("--only filters out other steps except the project home check", async () => {
  const { evidence, step } = runner(["export"]);
  assert.equal(await step("settings", async () => ({})), undefined);
  await step("project home renders", async () => ({}));
  await step("export the sample", async () => ({}));
  assert.deepEqual(
    evidence.steps.map((entry) => entry.name),
    ["project home renders", "export the sample"],
  );
});

test("summarizeSteps counts statuses", () => {
  assert.deepEqual(summarizeSteps([{ status: "passed" }, { status: "failed" }, { status: "skipped" }, { status: "passed" }]), {
    passed: 2,
    failed: 1,
    skipped: 1,
    total: 4,
  });
});

test("exitCodeFor fails on failed steps or a fatal error, not on skipped steps", () => {
  assert.equal(exitCodeFor({ steps: [{ status: "passed" }, { status: "skipped" }] }), 0);
  assert.equal(exitCodeFor({ steps: [{ status: "failed" }] }), 1);
  assert.equal(exitCodeFor({ steps: [], fatal: "Error: no driver" }), 1);
});

test("smokeRunContext labels the app kind and copies only app overrides", () => {
  const options = parseSmokeOptions(["--app", "/repo/src-tauri/target/debug/video-creater", "--only", "export"]);
  const context = smokeRunContext({
    options,
    env: { VIDEO_CREATER_APP_SUPPORT_DIR: "/tmp/support", HOME: "/home/me", OPENAI_API_KEY: "secret", DBUS_SESSION_BUS_ADDRESS: "unix:path=/x" },
    commit: "abc123",
  });
  assert.equal(context.appKind, "debug");
  assert.equal(context.commit, "abc123");
  assert.deepEqual(context.only, ["export"]);
  assert.deepEqual(context.overrides, { VIDEO_CREATER_APP_SUPPORT_DIR: "/tmp/support" });
  assert.equal(context.sessionBus, true);
  assert.ok(!JSON.stringify(context).includes("secret"));
  assert.ok(!JSON.stringify(context).includes("/home/me"));

  const release = smokeRunContext({ options: parseSmokeOptions(["--app", "/target/release/video-creater"]), env: {}, commit: "c" });
  assert.equal(release.appKind, "debug");
  assert.equal(release.sessionBus, false);
});

test("smokeRunContext reads the package from a release report", () => {
  const context = smokeRunContext({
    options: parseSmokeOptions(["--app", "/tmp/vc-deb-root-06/usr/bin/video-creater"]),
    env: {},
    commit: "head",
    releaseReport: { status: "passed", commit: "built", package: { path: "/p/app.deb", sha256: "f00", bytes: 5 } },
  });
  assert.equal(context.appKind, "packaged");
  assert.equal(context.commit, "head");
  assert.deepEqual(context.package, { commit: "built", path: "/p/app.deb", sha256: "f00", status: "passed" });
});

test("parseSmokeOptions defaults the agent flows to the Claude backend", () => {
  const options = parseSmokeOptions([]);
  assert.equal(options.agentBackend, "claude");
  assert.equal(options.claudeModel, "sonnet");
});

test("parseSmokeOptions reads the agent backend and the Claude model", () => {
  const options = parseSmokeOptions(["--agent-flows", "--agent-backend", "claude", "--claude-model", "haiku"]);
  assert.equal(options.agentBackend, "claude");
  assert.equal(options.claudeModel, "haiku");
});

test("parseSmokeOptions refuses an unknown agent backend or Claude model", () => {
  assert.throws(() => parseSmokeOptions(["--agent-backend", "gemini"]), /--agent-backend must be one of codex, claude/);
  assert.throws(() => parseSmokeOptions(["--claude-model", "turbo"]), /--claude-model must be one of sonnet, haiku, opus/);
});

test("parseSmokeOptions reads the export destination and render responsiveness flag", () => {
  assert.equal(parseSmokeOptions([]).exportTasks, false);
  assert.equal(parseSmokeOptions(["--export-tasks"]).exportTasks, true);
});


test("step evidence and diagnostics redact local media tokens without changing accepted detail", async () => {
  const token = "a".repeat(64);
  const url = `http://127.0.0.1:4788/media/${token}/sample.mp4`;
  const detail = { videos: [{ src: url, duration: 8 }], output: "exports/sample.mp4" };
  const evidence = { steps: [] as Record<string, unknown>[] };
  const logs: string[] = [];
  const step = createStepRunner({ evidence, log: (line: string) => logs.push(line) });
  assert.equal(await step("playback", async () => detail), detail);
  await step("failed playback", async () => { throw new Error(`Playback failed: ${url}`); });
  assert.ok(!JSON.stringify(evidence).includes(token), "persisted evidence contains a media token");
  assert.ok(!logs.join("\n").includes(token), "logged diagnostics contain a media token");
  assert.ok(JSON.stringify(evidence).includes("/media/[redacted]/sample.mp4"));
  assert.equal(detail.videos[0].src, url, "validation must continue using the real source URL");
  assert.ok(JSON.stringify(evidence).includes("exports/sample.mp4"));
});

test("--temporal-unavailable is a flag that cannot be combined with --temporal", () => {
  assert.equal(parseSmokeOptions(["--temporal-unavailable"]).temporalUnavailable, true);
  assert.throws(() => parseSmokeOptions(["--temporal", "--temporal-unavailable"]), /test opposite builds/);
});
