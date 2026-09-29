import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

// @ts-expect-error The repository does not generate declarations for scripts/*.mjs.
import {
  activationAppleScriptForPid,
  buildAcceptanceEnvironment,
  captureStorageCleanupFailureEvidence,
  createAcceptanceLayout,
  fileContainsAny,
  parseKeychainPaths,
  validateSettingsAcceptanceFailure,
  validateSettingsAcceptanceCheckpoint,
} from "./settings-packaged-acceptance.mjs";

const preRestart = {
  stage: "pre_restart",
  checks: [
    {
      id: "settingsDom",
      status: "passed",
      diagnosticCode: "settings.acceptance.settingsDom.passed",
    },
    {
      id: "modelCancel",
      status: "passed",
      diagnosticCode: "settings.acceptance.modelCancel.passed",
    },
  ],
};

const postRestart = {
  stage: "post_restart",
  checks: [
    {
      id: "settingsDom",
      status: "passed",
      diagnosticCode: "settings.acceptance.settingsDom.passed",
    },
    {
      id: "interruptedRecovery",
      status: "passed",
      diagnosticCode: "settings.acceptance.interruptedRecovery.passed",
    },
    {
      id: "modelReady",
      status: "passed",
      diagnosticCode: "settings.acceptance.modelReady.passed",
    },
    {
      id: "speechSeparation",
      status: "passed",
      diagnosticCode: "settings.acceptance.speechSeparation.passed",
    },
    {
      id: "storageCleanup",
      status: "passed",
      diagnosticCode: "settings.acceptance.storageCleanup.passed",
    },
    {
      id: "providerKeychain",
      status: "passed",
      diagnosticCode: "settings.acceptance.providerKeychain.passed",
    },
    {
      id: "agentMcpSkills",
      status: "passed",
      diagnosticCode: "settings.acceptance.agentMcpSkills.passed",
    },
  ],
};

test("packaged settings acceptance requires the exact passing checks for each launch", () => {
  assert.deepEqual(validateSettingsAcceptanceCheckpoint(preRestart, "pre_restart"), {
    passed: 2,
    stage: "pre_restart",
    total: 2,
  });
  assert.deepEqual(validateSettingsAcceptanceCheckpoint(postRestart, "post_restart"), {
    passed: 7,
    stage: "post_restart",
    total: 7,
  });
});

test("packaged settings acceptance fails closed on failed, missing, or noncanonical checks", () => {
  assert.throws(
    () =>
      validateSettingsAcceptanceCheckpoint(
        {
          ...preRestart,
          checks: preRestart.checks.map((check, index) =>
            index === 0 ? { ...check, status: "failed" } : check,
          ),
        },
        "pre_restart",
      ),
    /did not pass/,
  );
  assert.throws(
    () =>
      validateSettingsAcceptanceCheckpoint(
        { ...postRestart, checks: postRestart.checks.slice(0, -1) },
        "post_restart",
      ),
    /exact checks/,
  );
  assert.throws(
    () =>
      validateSettingsAcceptanceCheckpoint(
        {
          ...preRestart,
          checks: [
            { ...preRestart.checks[0], diagnosticCode: "settings.acceptance.fake.passed" },
            preRestart.checks[1],
          ],
        },
        "pre_restart",
      ),
    /diagnostic code/,
  );
});

test("packaged settings acceptance accepts only bounded failure diagnostics", () => {
  assert.doesNotThrow(() =>
    validateSettingsAcceptanceFailure(
      {
        stage: "pre_restart",
        phase: "settingsDom",
        diagnosticCode: "settings.acceptance.settingsDom.modelResults",
      },
      "pre_restart",
    ),
  );
  assert.doesNotThrow(() =>
    validateSettingsAcceptanceFailure(
      {
        stage: "post_restart",
        phase: "storageCleanup",
        diagnosticCode: "settings.acceptance.storageCleanup.disposableCache",
      },
      "post_restart",
    ),
  );
  assert.doesNotThrow(() =>
    validateSettingsAcceptanceFailure(
      {
        stage: "pre_restart",
        phase: "settingsDom",
        diagnosticCode: "settings.acceptance.settingsDom.failed",
      },
      "pre_restart",
    ),
  );
  assert.throws(
    () =>
      validateSettingsAcceptanceFailure(
        {
          stage: "pre_restart",
          phase: "settingsDom",
          diagnosticCode: "settings.acceptance.settingsDom.unbounded-user-content",
        },
        "pre_restart",
      ),
    /malformed failure evidence/,
  );
});

test("packaged settings acceptance retains only bounded storage-cleanup diagnostics", () => {
  const directory = mkdtempSync(join(tmpdir(), "video-creater-acceptance-evidence-"));
  const evidenceDir = join(directory, "evidence");
  const journal = join(
    directory,
    "home",
    "Library",
    "Application Support",
    "com.olhapi.video-creater",
    "settings",
    "operations.json",
  );
  try {
    mkdirSync(join(directory, "home", "Library", "Application Support", "com.olhapi.video-creater", "settings"), {
      recursive: true,
    });
    writeFileSync(
      journal,
      JSON.stringify([
        {
          id: "storage-cleanup-v1:secret-confirmation-token",
          kind: "storageCleanup",
          state: "failed",
          phase: "failed",
          completedUnits: 0,
          totalUnits: 1,
          error: {
            code: "settings.storage.confirmationMismatch",
            message: "do not retain this message",
            detail: "or this detail",
          },
        },
        {
          id: "provider-secret",
          kind: "providerRefresh",
          state: "failed",
          error: { code: "provider.secret", message: "must not be retained" },
        },
      ]),
    );

    const evidence = captureStorageCleanupFailureEvidence(directory, evidenceDir);

    assert.equal(evidence, join(evidenceDir, "storage-cleanup-operation.json"));
    assert.deepEqual(JSON.parse(readFileSync(evidence, "utf8")), {
      schema: "video-creater.settings-acceptance.storage-cleanup-operation",
      version: 1,
      operation: {
        kind: "storageCleanup",
        state: "failed",
        phase: "failed",
        completedUnits: 0,
        totalUnits: 1,
        errorCode: "settings.storage.confirmationMismatch",
      },
    });
  } finally {
    rmSync(directory, { force: true, recursive: true });
  }
});

test("packaged settings acceptance launches with only isolated state and system tools", () => {
  const environment = buildAcceptanceEnvironment({
    appSupport: "/private/tmp/video-creater-settings-acceptance-run/app-support",
    home: "/private/tmp/video-creater-settings-acceptance-run/home",
    hostEnvironment: {
      DYLD_LIBRARY_PATH: "/opt/homebrew/lib",
      LANG: "en_US.UTF-8",
      NODE_OPTIONS: "--require /tmp/injected.js",
      USER: "acceptance-user",
    },
    keychainService: "com.olhapi.video-creater.settings-acceptance.0123456789abcdef0123456789abcdef",
    root: "/private/tmp/video-creater-settings-acceptance-run",
    stage: "pre_restart",
    tmp: "/private/tmp/video-creater-settings-acceptance-run/tmp",
    token: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  });

  assert.equal(environment.PATH, "/usr/bin:/bin:/usr/sbin:/sbin");
  assert.equal(environment.HOME, "/private/tmp/video-creater-settings-acceptance-run/home");
  assert.equal(environment.VIDEO_CREATER_SETTINGS_ACCEPTANCE_STAGE, "pre_restart");
  assert.equal(
    environment.VIDEO_CREATER_PROVIDER_KEYCHAIN_SERVICE,
    "com.olhapi.video-creater.settings-acceptance.0123456789abcdef0123456789abcdef",
  );
  assert.equal(environment.NODE_OPTIONS, undefined);
  assert.equal(environment.DYLD_LIBRARY_PATH, undefined);
});

test("packaged settings acceptance activates only the exact spawned process", () => {
  assert.equal(
    activationAppleScriptForPid(42),
    [
      'tell application "System Events"',
      "  set frontmost of (first application process whose unix id is 42) to true",
      "end tell",
    ].join("\n"),
  );
  assert.throws(() => activationAppleScriptForPid(0), /process identifier is invalid/);
  assert.throws(() => activationAppleScriptForPid(Number.NaN), /process identifier is invalid/);
});

test("packaged settings acceptance keeps the app active during timer-driven native work", () => {
  const source = readFileSync(
    new URL("./settings-packaged-acceptance.mjs", import.meta.url),
    "utf8",
  );
  assert.match(source, /const ACCEPTANCE_FOCUS_REFRESH_MS = 2_000/);
  assert.match(source, /await activateAcceptanceProcess\(child, stage\);\n\s+nextFocusRefreshAt/);
});

test("Keychain snapshots preserve quoted paths containing spaces", () => {
  assert.deepEqual(
    parseKeychainPaths(
      '    "/Users/test/Library/Keychains/login.keychain-db"\n    "/private/tmp/Acceptance Keychain.keychain-db"\n',
    ),
    [
      "/Users/test/Library/Keychains/login.keychain-db",
      "/private/tmp/Acceptance Keychain.keychain-db",
    ],
  );
});

test("provider canary scanning covers chunk boundaries in large artifacts", () => {
  const directory = mkdtempSync(join(tmpdir(), "video-creater-acceptance-scan-"));
  const artifact = join(directory, "journal.bin");
  try {
    writeFileSync(
      artifact,
      Buffer.concat([
        Buffer.alloc(64 * 1024 - 4, "x"),
        Buffer.from("secret-canary"),
        Buffer.alloc(64 * 1024, "y"),
      ]),
    );
    assert.equal(fileContainsAny(artifact, ["secret-canary"]), true);
    assert.equal(fileContainsAny(artifact, ["different-canary"]), false);
  } finally {
    rmSync(directory, { force: true, recursive: true });
  }
});

test("packaged acceptance seeds a native render report sidecar", () => {
  const directory = mkdtempSync(join(tmpdir(), "video-creater-acceptance-layout-"));
  try {
    assert.equal(typeof createAcceptanceLayout, "function");
    const layout = createAcceptanceLayout(join(directory, "run"), "acceptance-token");
    const artifactDir = join(
      layout.project,
      "renders",
      "settings-acceptance-render-artifact",
    );
    assert.deepEqual(
      JSON.parse(readFileSync(join(artifactDir, "report.json"), "utf8")),
      {
        schemaVersion: 1,
        id: "settings-acceptance-render-artifact",
        status: "completed",
        outputPath: "renders/settings-acceptance-render-artifact/output.mp4",
        durationSeconds: 8,
        streams: { video: true, audio: true },
        checks: { artifactPaths: "passed", duration: "passed", streams: "passed" },
        artifacts: ["renders/settings-acceptance-render-artifact/report.json"],
        previewComparisonRequest: null,
        previewComparison: null,
        logPath: "renders/settings-acceptance-render-artifact/render.log",
        createdAt: "2026-07-12T00:00:00Z",
      },
    );
    assert.throws(
      () => readFileSync(join(artifactDir, "pipeline-report.json"), "utf8"),
      { code: "ENOENT" },
    );
    assert.equal(
      readFileSync(
        join(
          layout.home,
          "Library",
          "Caches",
          "com.olhapi.video-creater",
          "settings-acceptance-disposable-cache",
          "fixture.bin",
        ),
        "utf8",
      ),
      "settings acceptance disposable cache fixture\n",
    );
  } finally {
    rmSync(directory, { force: true, recursive: true });
  }
});

test("the macOS release cannot report success before packaged Settings acceptance passes", () => {
  const source = readFileSync(new URL("./build-macos-release.mjs", import.meta.url), "utf8");
  const acceptanceCall = source.indexOf("await runPackagedSettingsAcceptance({");
  const reportWrite = source.indexOf("writeFileSync(reportPath");
  const passedOutput = source.indexOf('status: "passed", reportPath');

  assert.match(source, /from "\.\/settings-packaged-acceptance\.mjs"/);
  assert.ok(acceptanceCall > 0, "release must run packaged Settings acceptance");
  assert.ok(reportWrite > acceptanceCall, "release report must follow packaged acceptance");
  assert.ok(passedOutput > acceptanceCall, "passed output must follow packaged acceptance");
  assert.match(source, /packagedSettingsAcceptance/);
});

test("the packaged harness fails fast when the app records a runner failure", () => {
  const source = readFileSync(
    new URL("./settings-packaged-acceptance.mjs", import.meta.url),
    "utf8",
  );
  assert.match(source, /failurePath/);
  assert.match(source, /settings acceptance app reported/);
  assert.match(source, /readFileSync\(failurePath, "utf8"\)/);
  assert.match(source, /failure\.diagnosticCode/);
  assert.doesNotMatch(source, /failure\.message/);
});
