#!/usr/bin/env node
import { createHash, randomBytes } from "node:crypto";
import {
  closeSync,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  openSync,
  readFileSync,
  readSync,
  readdirSync,
  readlinkSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { basename, join, relative, resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";

const SYSTEM_PATH = "/usr/bin:/bin:/usr/sbin:/sbin";
const ACCEPTANCE_PREFIX = "video-creater-settings-acceptance-";
const MARKER_FILE = ".video-creater-settings-acceptance";
const SETTINGS_RENDER_ARTIFACT_ID = "settings-acceptance-render-artifact";
const SETTINGS_RENDER_REPORT = Object.freeze({
  schemaVersion: 1,
  id: SETTINGS_RENDER_ARTIFACT_ID,
  status: "completed",
  outputPath: `renders/${SETTINGS_RENDER_ARTIFACT_ID}/output.mp4`,
  durationSeconds: 8,
  streams: { video: true, audio: true },
  checks: { artifactPaths: "passed", duration: "passed", streams: "passed" },
  artifacts: [`renders/${SETTINGS_RENDER_ARTIFACT_ID}/report.json`],
  previewComparisonRequest: null,
  previewComparison: null,
  logPath: `renders/${SETTINGS_RENDER_ARTIFACT_ID}/render.log`,
  createdAt: "2026-07-12T00:00:00Z",
});
const DEFAULT_TIMEOUT_MS = 30 * 60 * 1_000;
const ACCEPTANCE_FOCUS_REFRESH_MS = 2_000;
const EXPECTED_CHECKS = Object.freeze({
  pre_restart: ["settingsDom", "modelCancel"],
  post_restart: [
    "settingsDom",
    "interruptedRecovery",
    "modelReady",
    "speechSeparation",
    "storageCleanup",
    "providerKeychain",
    "agentMcpSkills",
  ],
});
const SETTINGS_DOM_FAILURE_DIAGNOSTIC_CODES = new Set([
  "settings.acceptance.settingsDom.failed",
  "settings.acceptance.settingsDom.categoryTabs",
  "settings.acceptance.settingsDom.models",
  "settings.acceptance.settingsDom.modelResults",
  "settings.acceptance.settingsDom.blockedActions",
  "settings.acceptance.settingsDom.integrations",
]);
const STORAGE_CLEANUP_FAILURE_DIAGNOSTIC_CODES = new Set([
  "settings.acceptance.storageCleanup.failed",
  "settings.acceptance.storageCleanup.renderArtifacts",
  "settings.acceptance.storageCleanup.disposableCache",
  "settings.acceptance.storageCleanup.confirmationMismatch",
  "settings.acceptance.storageCleanup.progressPersistence",
  "settings.acceptance.storageCleanup.projectSession",
  "settings.acceptance.storageCleanup.healthCommand",
  "settings.acceptance.storageCleanup.previewCommand",
  "settings.acceptance.storageCleanup.startCommand",
]);

const STORAGE_CLEANUP_OPERATION_EVIDENCE = "storage-cleanup-operation.json";

export async function runPackagedSettingsAcceptance({
  appPath,
  evidenceDir,
  source,
  timeoutMs = DEFAULT_TIMEOUT_MS,
}) {
  if (process.platform !== "darwin") {
    throw new Error("packaged Settings acceptance requires macOS");
  }
  const executable = join(appPath, "Contents/MacOS/video-creater");
  if (!existsSync(executable)) {
    throw new Error(`packaged Settings executable is missing: ${executable}`);
  }
  if (!Number.isFinite(timeoutMs) || timeoutMs < 1_000) {
    throw new Error("packaged Settings acceptance timeout is invalid");
  }

  const runId = randomBytes(8).toString("hex");
  const token = randomBytes(32).toString("hex");
  const root = `/private/tmp/${ACCEPTANCE_PREFIX}${runId}`;
  const keychainService = `com.olhapi.video-creater.settings-acceptance.${token.slice(0, 32)}`;
  const keychainPath = join(root, "acceptance.keychain-db");
  const keychainPassword = randomBytes(32).toString("base64url");
  const originalKeychains = snapshotKeychainConfiguration();
  const bundleDigestBefore = digestBundleTree(appPath);
  const existingProcesses = matchingExecutableProcesses(executable);
  if (existingProcesses.length > 0) {
    throw new Error(
      `packaged Settings acceptance requires no running instance of the release app (${existingProcesses.join(", ")})`,
    );
  }
  let keychainConfigurationTouched = false;
  let primaryError;
  let result;
  try {
    const layout = createAcceptanceLayout(root, token);
    keychainConfigurationTouched = true;
    createIsolatedKeychain({
      keychainPassword,
      keychainPath,
    });

    const launches = [];
    for (const stage of ["pre_restart", "post_restart"]) {
      const checkpointPath = join(
        layout.checkpoints,
        stage === "pre_restart" ? "pre-restart.json" : "post-restart.json",
      );
      const failurePath = join(
        layout.checkpoints,
        stage === "pre_restart"
          ? "pre-restart-failure.json"
          : "post-restart-failure.json",
      );
      const logPath = join(layout.logs, `${stage}.log`);
      const environment = buildAcceptanceEnvironment({
        appSupport: layout.appSupport,
        home: layout.home,
        keychainService,
        root,
        stage,
        tmp: layout.tmp,
        token,
      });
      const launch = await launchUntilCheckpoint({
        checkpointPath,
        evidenceDir,
        environment,
        executable,
        failurePath,
        logPath,
        stage,
        timeoutMs,
      });
      const retainedCheckpoint = join(evidenceDir, basename(checkpointPath));
      mkdirSync(evidenceDir, { recursive: true });
      copyFileSync(checkpointPath, retainedCheckpoint);
      launches.push({
        ...launch,
        checkpoint: artifactEvidence(retainedCheckpoint),
      });
    }

    const bundleDigestAfter = digestBundleTree(appPath);
    if (bundleDigestAfter !== bundleDigestBefore) {
      throw new Error("packaged Settings acceptance modified the signed app bundle");
    }
    const leakedCanaries = findCanaryLeaks([root], runId);
    if (leakedCanaries.length > 0) {
      throw new Error(
        `packaged Settings acceptance leaked provider canaries: ${leakedCanaries.join(", ")}`,
      );
    }
    const remainingProcesses = matchingExecutableProcesses(executable);
    if (remainingProcesses.length > 0) {
      throw new Error(
        `packaged Settings acceptance left release app processes running: ${remainingProcesses.join(", ")}`,
      );
    }

    result = {
      schema: "video-creater.settings-packaged-acceptance",
      version: 3,
      status: "passed",
      generatedAt: new Date().toISOString(),
      source,
      app: {
        path: appPath,
        bundleSha256: bundleDigestAfter,
        unchanged: true,
      },
      environment: {
        path: SYSTEM_PATH,
        isolatedAppSupport: true,
        isolatedHome: true,
        isolatedKeychain: true,
        isolatedTemporaryDirectory: true,
      },
      interactiveAcceptance: {
        launches: launches.length,
        maxConcurrentAppInstances: 1,
        preRestart: launches[0],
        postRestart: launches[1],
      },
      cleanup: {
        providerCanaryLeakage: "absent",
        releaseAppProcesses: 0,
      },
    };
  } catch (error) {
    primaryError = error;
    captureStorageCleanupFailureEvidence(root, evidenceDir);
  }

  let cleanupError;
  try {
    if (keychainConfigurationTouched) {
      restoreKeychainConfiguration({
        keychainPath,
        originalKeychains,
      });
    }
    const restoredKeychains = snapshotKeychainConfiguration();
    assertKeychainConfigurationRestored(originalKeychains, restoredKeychains);
    if (result) {
      result.cleanup.isolatedKeychainDeleted = !existsSync(keychainPath);
      result.cleanup.normalKeychainConfigurationUnchanged = true;
      result.cleanup.normalKeychainDefaultFingerprint = sha256Text(
        originalKeychains.defaultPath,
      );
      result.cleanup.normalKeychainSearchFingerprint = sha256Text(
        originalKeychains.searchPaths.join("\n"),
      );
    }
  } catch (error) {
    cleanupError = error;
  } finally {
    rmSync(root, { force: true, recursive: true });
  }

  if (primaryError && cleanupError) {
    throw new AggregateError(
      [primaryError, cleanupError],
      "packaged Settings acceptance and Keychain restoration both failed",
    );
  }
  if (primaryError) throw primaryError;
  if (cleanupError) throw cleanupError;

  mkdirSync(evidenceDir, { recursive: true });
  const reportPath = join(evidenceDir, "settings-packaged-acceptance.json");
  writeFileSync(reportPath, `${JSON.stringify(result, null, 2)}\n`, { mode: 0o600 });
  return {
    ...result,
    report: artifactEvidence(reportPath),
  };
}

/**
 * Preserve the smallest safe diagnostic for a failed native storage cleanup.
 * Operation IDs, messages, details, and every non-cleanup operation may carry
 * paths or other run-specific data, so none of them leave the isolated root.
 */
export function captureStorageCleanupFailureEvidence(root, evidenceDir) {
  const journalPath = [
    join(
      root,
      "home",
      "Library",
      "Application Support",
      "com.olhapi.video-creater",
      "settings",
      "operations.json",
    ),
    join(root, "app-support", "settings", "operations.json"),
  ].find((candidate) => existsSync(candidate));
  if (!journalPath) return null;

  let operations;
  try {
    operations = JSON.parse(readFileSync(journalPath, "utf8"));
  } catch {
    return null;
  }
  if (!Array.isArray(operations)) return null;
  const operation = [...operations]
    .reverse()
    .find((candidate) =>
      candidate &&
      typeof candidate === "object" &&
      candidate.kind === "storageCleanup" &&
      candidate.state === "failed",
    );
  if (!operation) return null;

  const errorCode =
    operation.error &&
    typeof operation.error === "object" &&
    typeof operation.error.code === "string"
      ? operation.error.code
      : null;
  const evidence = {
    schema: "video-creater.settings-acceptance.storage-cleanup-operation",
    version: 1,
    operation: {
      kind: "storageCleanup",
      state: "failed",
      phase: typeof operation.phase === "string" ? operation.phase : null,
      completedUnits:
        typeof operation.completedUnits === "number" ? operation.completedUnits : null,
      totalUnits: typeof operation.totalUnits === "number" ? operation.totalUnits : null,
      errorCode,
    },
  };
  mkdirSync(evidenceDir, { recursive: true });
  const evidencePath = join(evidenceDir, STORAGE_CLEANUP_OPERATION_EVIDENCE);
  writeFileSync(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
  return evidencePath;
}

export function buildAcceptanceEnvironment({
  appSupport,
  home,
  keychainService,
  root,
  stage,
  tmp,
  token,
  hostEnvironment = process.env,
}) {
  if (!EXPECTED_CHECKS[stage]) {
    throw new Error(`unsupported packaged Settings acceptance stage: ${stage}`);
  }
  if (!/^[a-f0-9]{64}$/.test(token)) {
    throw new Error("packaged Settings acceptance token is invalid");
  }
  const expectedService = `com.olhapi.video-creater.settings-acceptance.${token.slice(0, 32)}`;
  if (keychainService !== expectedService) {
    throw new Error("packaged Settings acceptance Keychain service is not isolated");
  }
  return compactObject({
    PATH: SYSTEM_PATH,
    HOME: home,
    TMPDIR: tmp,
    LANG: hostEnvironment.LANG || "en_US.UTF-8",
    LC_CTYPE: hostEnvironment.LC_CTYPE,
    USER: hostEnvironment.USER,
    LOGNAME: hostEnvironment.LOGNAME,
    __CF_USER_TEXT_ENCODING: hostEnvironment.__CF_USER_TEXT_ENCODING,
    CODEX_HOME: join(root, "codex-home"),
    VIDEO_CREATER_APP_SUPPORT_DIR: appSupport,
    VIDEO_CREATER_PROVIDER_KEYCHAIN_SERVICE: keychainService,
    VIDEO_CREATER_SETTINGS_ACCEPTANCE_ROOT: root,
    VIDEO_CREATER_SETTINGS_ACCEPTANCE_STAGE: stage,
    VIDEO_CREATER_SETTINGS_ACCEPTANCE_TOKEN: token,
  });
}

/**
 * The acceptance runner uses short browser timers while it waits for native
 * operations. macOS may pause those timers for an inactive app, so make the
 * exact spawned process frontmost rather than relying on application-name
 * lookup (which could select a different installed build).
 */
export function activationAppleScriptForPid(pid) {
  if (!Number.isSafeInteger(pid) || pid <= 0) {
    throw new Error("packaged Settings acceptance process identifier is invalid");
  }
  return [
    'tell application "System Events"',
    `  set frontmost of (first application process whose unix id is ${pid}) to true`,
    "end tell",
  ].join("\n");
}

export function validateSettingsAcceptanceCheckpoint(checkpoint, expectedStage) {
  if (!checkpoint || typeof checkpoint !== "object" || Array.isArray(checkpoint)) {
    throw new Error("packaged Settings acceptance checkpoint is not an object");
  }
  if (checkpoint.stage !== expectedStage || !EXPECTED_CHECKS[expectedStage]) {
    throw new Error("packaged Settings acceptance checkpoint stage mismatch");
  }
  if (!Array.isArray(checkpoint.checks)) {
    throw new Error("packaged Settings acceptance checkpoint checks are missing");
  }
  const expected = EXPECTED_CHECKS[expectedStage];
  const ids = checkpoint.checks.map((check) => check?.id);
  if (ids.length !== expected.length || [...ids].sort().join("\0") !== [...expected].sort().join("\0")) {
    throw new Error("packaged Settings acceptance checkpoint does not contain the exact checks");
  }
  for (const check of checkpoint.checks) {
    if (check.status !== "passed") {
      throw new Error(`packaged Settings acceptance check ${check.id} did not pass`);
    }
    if (check.diagnosticCode !== `settings.acceptance.${check.id}.passed`) {
      throw new Error(`packaged Settings acceptance check ${check.id} has a noncanonical diagnostic code`);
    }
  }
  return {
    passed: checkpoint.checks.length,
    stage: expectedStage,
    total: expected.length,
  };
}

export function validateSettingsAcceptanceFailure(failure, expectedStage) {
  if (
    !failure ||
    typeof failure !== "object" ||
    Array.isArray(failure) ||
    failure.stage !== expectedStage ||
    typeof failure.phase !== "string" ||
    !EXPECTED_CHECKS[expectedStage]?.includes(failure.phase) ||
    typeof failure.diagnosticCode !== "string"
  ) {
    throw new Error("settings acceptance app reported malformed failure evidence");
  }
  const canonicalCode = `settings.acceptance.${failure.phase}.failed`;
  const allowed = failure.phase === "settingsDom"
    ? SETTINGS_DOM_FAILURE_DIAGNOSTIC_CODES.has(failure.diagnosticCode)
    : failure.phase === "storageCleanup"
      ? STORAGE_CLEANUP_FAILURE_DIAGNOSTIC_CODES.has(failure.diagnosticCode)
    : failure.diagnosticCode === canonicalCode;
  if (!allowed) {
    throw new Error("settings acceptance app reported malformed failure evidence");
  }
  return { diagnosticCode: failure.diagnosticCode, phase: failure.phase };
}

export function parseKeychainPaths(output) {
  return [...output.matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((match) =>
    match[1].replaceAll('\\"', '"').replaceAll("\\\\", "\\"),
  );
}

export function createAcceptanceLayout(root, token) {
  mkdirSync(root, { mode: 0o700 });
  const layout = {
    appSupport: join(root, "app-support"),
    checkpoints: join(root, "checkpoints"),
    codexHome: join(root, "codex-home"),
    home: join(root, "home"),
    logs: join(root, "logs"),
    project: join(root, "projects/acceptance-project"),
    tmp: join(root, "tmp"),
  };
  for (const path of Object.values(layout)) mkdirSync(path, { mode: 0o700, recursive: true });
  const renderArtifact = join(
    layout.project,
    "renders",
    SETTINGS_RENDER_ARTIFACT_ID,
  );
  mkdirSync(renderArtifact, { mode: 0o700, recursive: true });
  writeFileSync(
    join(renderArtifact, "report.json"),
    `${JSON.stringify(SETTINGS_RENDER_REPORT, null, 2)}\n`,
    {
      mode: 0o600,
    },
  );
  const disposableCache = join(
    layout.home,
    "Library",
    "Caches",
    "com.olhapi.video-creater",
    "settings-acceptance-disposable-cache",
  );
  mkdirSync(disposableCache, { mode: 0o700, recursive: true });
  writeFileSync(
    join(disposableCache, "fixture.bin"),
    "settings acceptance disposable cache fixture\n",
    { mode: 0o600 },
  );
  writeFileSync(join(root, MARKER_FILE), token, { mode: 0o600 });
  return layout;
}

function snapshotKeychainConfiguration() {
  const searchOutput = checkedSecurity(["list-keychains", "-d", "user"]).stdout;
  const defaultOutput = checkedSecurity(["default-keychain", "-d", "user"]).stdout;
  const searchPaths = parseKeychainPaths(searchOutput);
  const defaultPath = parseKeychainPaths(defaultOutput)[0];
  if (!defaultPath) {
    throw new Error("could not determine the normal default Keychain");
  }
  return { defaultPath, searchPaths };
}

function createIsolatedKeychain({ keychainPassword, keychainPath }) {
  checkedSecurity(["create-keychain", "-p", keychainPassword, keychainPath], [keychainPassword]);
  checkedSecurity(["set-keychain-settings", "-lut", "21600", keychainPath]);
  checkedSecurity(["unlock-keychain", "-p", keychainPassword, keychainPath], [keychainPassword]);
  checkedSecurity(["list-keychains", "-d", "user", "-s", keychainPath]);
  checkedSecurity(["default-keychain", "-d", "user", "-s", keychainPath]);
}

function restoreKeychainConfiguration({ keychainPath, originalKeychains }) {
  const errors = [];
  const restoreCommands = [
    ["default-keychain", "-d", "user", "-s", originalKeychains.defaultPath],
    ["list-keychains", "-d", "user", "-s", ...originalKeychains.searchPaths],
  ];
  if (existsSync(keychainPath)) restoreCommands.push(["delete-keychain", keychainPath]);
  for (const args of restoreCommands) {
    try {
      checkedSecurity(args);
    } catch (error) {
      errors.push(error);
    }
  }
  if (errors.length > 0) {
    throw new AggregateError(errors, "could not fully restore the normal Keychain configuration");
  }
}

function assertKeychainConfigurationRestored(before, after) {
  if (
    before.defaultPath !== after.defaultPath ||
    before.searchPaths.join("\0") !== after.searchPaths.join("\0")
  ) {
    throw new Error("normal Keychain default or search list changed during packaged Settings acceptance");
  }
}

function checkedSecurity(args, secrets = []) {
  const result = spawnSync("/usr/bin/security", args, {
    encoding: "utf8",
    env: { PATH: SYSTEM_PATH, HOME: process.env.HOME },
  });
  if (result.status !== 0) {
    let detail = `${result.stderr || result.stdout || "unknown security error"}`.trim();
    for (const secret of secrets) detail = detail.replaceAll(secret, "<redacted>");
    throw new Error(`security ${args[0]} failed: ${detail}`);
  }
  return result;
}

async function launchUntilCheckpoint({
  checkpointPath,
  evidenceDir,
  environment,
  executable,
  failurePath,
  logPath,
  stage,
  timeoutMs,
}) {
  const log = openSync(logPath, "a", 0o600);
  const child = spawn(executable, [], {
    env: environment,
    stdio: ["ignore", log, log],
  });
  const startedAt = Date.now();
  let exit;
  let spawnError;
  child.once("error", (error) => {
    spawnError = error;
  });
  child.once("exit", (code, signal) => {
    exit = { code, signal };
  });
  try {
    await activateAcceptanceProcess(child, stage);
    let nextFocusRefreshAt = Date.now() + ACCEPTANCE_FOCUS_REFRESH_MS;
    while (Date.now() - startedAt < timeoutMs) {
      if (Date.now() >= nextFocusRefreshAt) {
        await activateAcceptanceProcess(child, stage);
        nextFocusRefreshAt = Date.now() + ACCEPTANCE_FOCUS_REFRESH_MS;
      }
      if (existsSync(failurePath)) {
        const failure = JSON.parse(readFileSync(failurePath, "utf8"));
        const validatedFailure = validateSettingsAcceptanceFailure(failure, stage);
        mkdirSync(evidenceDir, { recursive: true });
        const retainedFailure = join(evidenceDir, basename(failurePath));
        copyFileSync(failurePath, retainedFailure);
        throw new Error(
          `settings acceptance app reported ${validatedFailure.phase} (${validatedFailure.diagnosticCode}; ${retainedFailure})`,
        );
      }
      if (existsSync(checkpointPath)) {
        const checkpoint = JSON.parse(readFileSync(checkpointPath, "utf8"));
        const summary = validateSettingsAcceptanceCheckpoint(checkpoint, stage);
        return {
          ...summary,
          durationMs: Date.now() - startedAt,
          status: "passed",
        };
      }
      if (exit) {
        throw new Error(
          `packaged Settings acceptance app exited before ${stage} checkpoint (code=${exit.code}, signal=${exit.signal})`,
        );
      }
      if (spawnError) {
        throw new Error(`could not launch packaged Settings acceptance app: ${spawnError.message}`);
      }
      await delay(200);
    }
    throw new Error(`packaged Settings acceptance timed out waiting for ${stage} checkpoint`);
  } finally {
    await terminateChild(child);
    closeSync(log);
  }
}

async function activateAcceptanceProcess(child, stage) {
  for (let attempt = 0; attempt < 25; attempt += 1) {
    if (!child.pid) {
      throw new Error(`packaged Settings acceptance ${stage} launch has no process identifier`);
    }
    const result = spawnSync(
      "/usr/bin/osascript",
      ["-e", activationAppleScriptForPid(child.pid)],
      { encoding: "utf8", env: { PATH: SYSTEM_PATH } },
    );
    if (result.status === 0) return;
    await delay(200);
  }
  throw new Error(`could not activate packaged Settings acceptance ${stage} window`);
}

async function terminateChild(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  child.kill("SIGTERM");
  for (let attempt = 0; attempt < 50; attempt += 1) {
    if (child.exitCode !== null || child.signalCode !== null) return;
    await delay(100);
  }
  child.kill("SIGKILL");
  for (let attempt = 0; attempt < 50; attempt += 1) {
    if (child.exitCode !== null || child.signalCode !== null) return;
    await delay(100);
  }
  throw new Error(`could not terminate packaged Settings acceptance process ${child.pid}`);
}

function matchingExecutableProcesses(executable) {
  const result = spawnSync("/bin/ps", ["-axo", "pid=,command="], {
    encoding: "utf8",
    env: { PATH: SYSTEM_PATH },
  });
  if (result.status !== 0) {
    throw new Error(`could not inspect release app processes: ${result.stderr.trim()}`);
  }
  const executableName = basename(executable);
  return result.stdout
    .split("\n")
    .map((line) => /^(\s*\d+)\s+(.+)$/.exec(line))
    .filter(Boolean)
    .filter((match) => {
      const command = match[2];
      return (
        command === executable ||
        command.startsWith(`${executable} `) ||
        command === executableName ||
        command.endsWith(`/${executableName}`)
      );
    })
    .map((match) => Number.parseInt(match[1], 10));
}

function digestBundleTree(root) {
  const hash = createHash("sha256");
  const visit = (path) => {
    const stat = lstatSync(path);
    const entry = relative(root, path) || ".";
    if (stat.isSymbolicLink()) {
      hash.update(`link\0${entry}\0${stat.mode}\0${readlinkSync(path)}\0`);
      return;
    }
    if (stat.isDirectory()) {
      hash.update(`directory\0${entry}\0${stat.mode}\0`);
      for (const name of readdirSync(path).sort()) visit(join(path, name));
      return;
    }
    if (!stat.isFile()) {
      throw new Error(`signed app bundle contains an unsupported filesystem entry: ${entry}`);
    }
    hash.update(`file\0${entry}\0${stat.mode}\0${stat.size}\0`);
    hash.update(readFileSync(path));
  };
  visit(resolve(root));
  return hash.digest("hex");
}

function findCanaryLeaks(roots, runId) {
  const canaries = [
    `video-creater-acceptance-canary-${ACCEPTANCE_PREFIX}${runId}-first`,
    `video-creater-acceptance-canary-${ACCEPTANCE_PREFIX}${runId}-replacement`,
  ];
  const leaks = [];
  const visit = (path) => {
    const stat = lstatSync(path);
    if (stat.isDirectory()) {
      for (const name of readdirSync(path)) visit(join(path, name));
      return;
    }
    if (!stat.isFile()) return;
    if (fileContainsAny(path, canaries)) leaks.push(path);
  };
  for (const root of roots) if (existsSync(root)) visit(root);
  return [...new Set(leaks)].sort();
}

export function fileContainsAny(path, values) {
  const needles = values.map((value) => Buffer.from(value));
  const overlapBytes = Math.max(...needles.map((needle) => needle.length)) - 1;
  const chunk = Buffer.allocUnsafe(64 * 1024);
  let overlap = Buffer.alloc(0);
  const file = openSync(path, "r");
  try {
    while (true) {
      const bytesRead = readSync(file, chunk, 0, chunk.length, null);
      if (bytesRead === 0) return false;
      const window = Buffer.concat([overlap, chunk.subarray(0, bytesRead)]);
      if (needles.some((needle) => window.includes(needle))) return true;
      overlap = Buffer.from(window.subarray(Math.max(0, window.length - overlapBytes)));
    }
  } finally {
    closeSync(file);
  }
}

function artifactEvidence(path) {
  const bytes = readFileSync(path);
  return {
    path,
    bytes: bytes.length,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  };
}

function sha256Text(value) {
  return createHash("sha256").update(value).digest("hex");
}

function compactObject(value) {
  return Object.fromEntries(Object.entries(value).filter(([, entry]) => entry !== undefined));
}

function delay(milliseconds) {
  return new Promise((resolvePromise) => setTimeout(resolvePromise, milliseconds));
}
