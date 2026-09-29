import assert from "node:assert/strict";
import test from "node:test";

import {
  REQUIRED_NATIVE_LANES,
  evaluateNativeRustTestReport,
  parseCargoTestSummary,
} from "./run-native-rust-tests.mjs";

function passingReport() {
  return {
    schemaVersion: 1,
    status: "passed",
    environment: {
      platform: "darwin",
      arch: "arm64",
      osRelease: "25.0.0",
      rustc: "rustc 1.94.0",
      xcode: "Xcode 27.0",
      cargoTargetDir: "/repo/src-tauri/target/verify",
      cargoIncremental: "0",
    },
    fullSuite: {
      status: "passed",
      command: ["cargo", "test", "--", "--test-threads=1"],
      runtimeEnvironment: {
        GST_GL_WINDOW: "dummy",
        VIDEO_CREATER_HEADLESS_RUST_SUITE: "1",
      },
      exitCode: 0,
      passed: 100,
      failed: 0,
      ignored: 1,
      gstreamerAppKitWarnings: 0,
      logPath: "output/native/full-suite.log",
    },
    requiredLanes: REQUIRED_NATIVE_LANES.map((lane) => ({
      id: lane.id,
      testName: lane.testName,
      status: "passed",
      command: ["cargo", "test"],
      exitCode: 0,
      passed: 1,
      failed: 0,
      ignored: 0,
      logPath: `output/native/${lane.id}.log`,
    })),
  };
}

test("requires complete serial Cargo coverage and native environment metadata", () => {
  assert.deepEqual(evaluateNativeRustTestReport(passingReport()), []);

  const missingMetadata = passingReport();
  missingMetadata.environment.xcode = "";
  missingMetadata.environment.cargoTargetDir = "";
  missingMetadata.fullSuite.command = ["cargo", "test", "--parallel"];
  missingMetadata.fullSuite.runtimeEnvironment.VIDEO_CREATER_HEADLESS_RUST_SUITE = "";
  assert.deepEqual(evaluateNativeRustTestReport(missingMetadata), [
    "environment.xcode must be recorded",
    "environment.cargoTargetDir must be recorded",
    "fullSuite must execute with --test-threads=1",
    "fullSuite must skip custom AppKit harness execution",
  ]);
});

test("requires Metal afconvert bundled filmstrip Keychain and notification lanes", () => {
  assert.deepEqual(
    REQUIRED_NATIVE_LANES.map((lane) => lane.id),
    [
      "metal",
      "afconvert",
      "bundled-filmstrip",
      "keychain",
      "notifications",
      "media-inspection-appkit",
      "precompose-alpha-appkit",
      "nested-export-appkit",
      "prores-export-appkit",
    ],
  );

  const report = passingReport();
  report.requiredLanes = report.requiredLanes.filter((lane) => lane.id !== "keychain");
  assert.deepEqual(evaluateNativeRustTestReport(report), [
    "required native lane keychain is missing",
  ]);
});

test("rejects failed full suites and skipped required native lanes", () => {
  const report = passingReport();
  report.status = "failed";
  report.fullSuite.status = "failed";
  report.fullSuite.exitCode = 101;
  report.fullSuite.failed = 1;
  const notification = report.requiredLanes.find((lane) => lane.id === "notifications");
  assert.ok(notification);
  notification.status = "skipped";
  notification.passed = 0;
  notification.ignored = 1;

  assert.deepEqual(evaluateNativeRustTestReport(report), [
    "report status must be passed",
    "fullSuite did not pass",
    "fullSuite contains 1 failed test(s)",
    "required native lane notifications did not pass",
    "required native lane notifications did not execute exactly one passing test",
  ]);
});

test("requires a warning-free headless full suite while retaining AppKit lanes", () => {
  const report = passingReport();
  report.fullSuite.runtimeEnvironment.GST_GL_WINDOW = "cocoa";
  report.fullSuite.gstreamerAppKitWarnings = 2;

  assert.deepEqual(evaluateNativeRustTestReport(report), [
    "fullSuite must use the headless GStreamer GL window backend",
    "fullSuite emitted 2 GStreamer AppKit warning(s)",
  ]);
});

test("parses and aggregates Cargo summaries across test binaries", () => {
  assert.deepEqual(
    parseCargoTestSummary(`
test result: ok. 12 passed; 0 failed; 1 ignored; 0 measured; 3 filtered out
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
`),
    { passed: 16, failed: 0, ignored: 1, filteredOut: 3 },
  );
});
