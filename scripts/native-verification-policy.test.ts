import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

import {
  buildVerificationPlan,
  parseVerificationArgs,
  verificationEnvironment,
} from "./run-native-verification.mjs";

const repoRoot = resolve(import.meta.dirname, "..");

test("fast verification requires exact filters and excludes release-only profiles", () => {
  assert.throws(
    () => parseVerificationArgs(["--lane", "fast"]),
    /at least one exact Rust test filter/,
  );
  const input = parseVerificationArgs(["--lane", "fast", "module::exact_test"]);
  const plan = buildVerificationPlan(input);
  assert.deepEqual(plan, [
    ["cargo", "fmt", "--manifest-path", "src-tauri/Cargo.toml", "-p", "video-creater", "-p", "video-creater-precompose-protocol", "-p", "video-creater-precompose-worker", "-p", "video-creater-compatibility-protocol", "-p", "video-creater-compatibility-decoder", "-p", "video-creater-provider-e2e-harness", "-p", "video-creater-audio-enhance", "-p", "video-creater-semantic-encoder", "-p", "video-creater-speech-worker", "--", "--check"],
    ["cargo", "check", "--manifest-path", "src-tauri/Cargo.toml", "--workspace"],
    [
      "cargo", "test", "--manifest-path", "src-tauri/Cargo.toml", "--lib",
      "module::exact_test", "--", "--exact", "--test-threads=1",
    ],
  ]);
  assert.doesNotMatch(JSON.stringify(plan), /all-targets|mcp-server|custom-protocol|run-native-rust-tests/);
});

test("release verification preserves all three shipped Clippy profiles and native lanes", () => {
  const plan = buildVerificationPlan(parseVerificationArgs(["--lane", "release"]));
  assert.equal(plan.filter((command) => command[1] === "clippy").length, 3);
  assert.ok(plan.some((command) => command.includes("mcp-server")));
  const workspaceClippy = plan.find((command) => command.includes("--workspace"));
  assert.equal(workspaceClippy[workspaceClippy.indexOf("--features") + 1], "web-host");
  assert.ok(plan.some((command) =>
    command.some((argument) => argument.includes("custom-protocol"))
  ));
  assert.deepEqual(plan.at(-1), ["node", "scripts/run-native-rust-tests.mjs"]);
});

test("every child receives the canonical verification target and disabled incremental builds", () => {
  assert.deepEqual(verificationEnvironment(repoRoot, "fast", { platform: "darwin" }), {
    CARGO_TARGET_DIR: resolve(repoRoot, "src-tauri/target/verify"),
    CARGO_INCREMENTAL: "0",
    TAURI_CONFIG: '{"bundle":{"externalBin":[],"resources":[]}}',
  });
  assert.deepEqual(verificationEnvironment(repoRoot, "release", { platform: "darwin" }), {
    CARGO_TARGET_DIR: resolve(repoRoot, "src-tauri/target/verify"),
    CARGO_INCREMENTAL: "0",
  });
});

test("cancellation is forwarded and final cache enforcement cannot be skipped", () => {
  const source = readFileSync(
    resolve(repoRoot, "scripts/run-native-verification.mjs"),
    "utf8",
  );
  assert.match(source, /process\.once\("SIGINT"/);
  assert.match(source, /process\.once\("SIGTERM"/);
  assert.match(source, /activeChild\?\.kill\(signal\)/);
  assert.match(source, /finally\s*\{/);
  assert.match(source, /post_run_over_limit/);
  assert.match(source, /await writeMetrics\(report\)/);
});

test("format verification preserves reviewed vendored source bytes", () => {
  const command = buildVerificationPlan(parseVerificationArgs(["--lane", "release"]))[0];
  assert.ok(!command.includes("--all"));
  assert.ok(command.includes("video-creater-audio-enhance"));
  assert.ok(!command.includes("deep_filter"));
});

test("Linux verification supplies the same compiler defaults as release packaging", () => {
  assert.equal(verificationEnvironment(repoRoot, "release", { platform: "linux" }).CXX, "c++");
});
