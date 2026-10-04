// Option parsing, the step runner and run bookkeeping for the Linux desktop smoke run. Pure: no
// process, file or WebDriver side effects, so it is unit tested with node --test.

import { resolve } from "node:path";

const booleanFlags = {
  "--dev-server": "devServer",
  "--fake-audio": "fakeAudio",
  "--temporal": "temporal",
  "--temporal-unavailable": "temporalUnavailable",
  "--native-menu": "nativeMenu",
  "--agent-flows": "agentFlows",
  "--export-tasks": "exportTasks",
};

const valueFlags = {
  "--app": "app",
  "--tauri-driver": "tauriDriver",
  "--native-driver": "nativeDriver",
  "--out": "out",
  "--display": "display",
  "--only": "only",
  "--keyring-root": "keyringRoot",
  "--temporal-cli": "temporalCli",
  "--temporal-worker": "temporalWorker",
  "--xdotool-root": "xdotoolRoot",
  "--gst-tools-root": "gstToolsRoot",
  "--release-report": "releaseReport",
  "--agent-backend": "agentBackend",
  "--claude-model": "claudeModel",
};

/** Which conversation-turn backend `--agent-flows` pins before it sends a prompt. */
const agentBackends = ["codex", "claude"];
/** The Claude model aliases the app's preference accepts. */
const claudeModels = ["sonnet", "haiku", "opus"];

export function parseSmokeOptions(argv) {
  const options = {
    app: "src-tauri/target/debug/video-creater",
    tauriDriver: "tauri-driver",
    nativeDriver: "WebKitWebDriver",
    out: "output/linux-desktop-smoke",
    display: ":94",
    only: "",
    temporalCli: "temporal",
    temporalWorker: "src-tauri/target/debug/video-creater-temporal-worker",
    agentBackend: "claude",
    claudeModel: "sonnet",
    ...Object.fromEntries(Object.values(booleanFlags).map((key) => [key, false])),
  };
  const args = argv[0] === "--" ? argv.slice(1) : argv;
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--") continue;
    if (booleanFlags[arg]) options[booleanFlags[arg]] = true;
    else if (valueFlags[arg]) {
      if (index + 1 >= args.length) throw new Error(`${arg} needs a value`);
      options[valueFlags[arg]] = args[index + 1];
      index += 1;
    } else throw new Error(`unknown option ${arg}`);
  }
  options.app = resolve(options.app);
  options.out = resolve(options.out);
  options.only = options.only
    .split(",")
    .map((pattern) => pattern.trim())
    .filter(Boolean);
  if (options.temporal && options.temporalUnavailable) throw new Error("--temporal and --temporal-unavailable test opposite builds; pass one");
  if (options.nativeMenu && !options.xdotoolRoot) throw new Error("--native-menu needs --xdotool-root");
  if (!agentBackends.includes(options.agentBackend)) throw new Error(`--agent-backend must be one of ${agentBackends.join(", ")}`);
  if (!claudeModels.includes(options.claudeModel)) throw new Error(`--claude-model must be one of ${claudeModels.join(", ")}`);
  return options;
}

/** Retained diagnostics must never persist the per-launch loopback media authorization. */
export function redactSmokeMediaTokens(value) {
  if (typeof value === "string") {
    return value.replace(/(https?:\/\/(?:127\.0\.0\.1|localhost|\[::1\]):\d+\/media\/)[^/\s"'?]+/g, "$1[redacted]");
  }
  if (Array.isArray(value)) return value.map(redactSmokeMediaTokens);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([key, entry]) => [key, redactSmokeMediaTokens(entry)]));
  }
  return value;
}

const skippedMarker = Symbol("skipped step");

/** A step action returns this when its prerequisite is absent, so the run records it as skipped. */
export function skipped(reason) {
  return { [skippedMarker]: true, reason };
}

/**
 * Returns `step(name, action)`. Steps outside `only` are not recorded, except the first home check
 * that every run needs. A thrown error records `failed` and calls `onFailure(name)`, which may return
 * a screenshot path.
 */
export function createStepRunner({ evidence, only = [], onFailure = async () => undefined, log = console.log }) {
  return async function step(name, action) {
    if (only.length > 0 && name !== "project home renders" && !only.some((pattern) => name.includes(pattern))) {
      return undefined;
    }
    const startedAt = Date.now();
    try {
      const detail = await action();
      if (detail && detail[skippedMarker]) {
        evidence.steps.push({ name, status: "skipped", reason: detail.reason });
        log(`skipped: ${name}: ${detail.reason}`);
        return undefined;
      }
      evidence.steps.push({ name, status: "passed", ms: Date.now() - startedAt, detail: redactSmokeMediaTokens(detail) });
      log(`passed: ${name}`);
      return detail;
    } catch (error) {
      let screenshot;
      try {
        screenshot = await onFailure(name);
      } catch {
        screenshot = undefined;
      }
      const diagnosis = redactSmokeMediaTokens(String(error));
      evidence.steps.push({ name, status: "failed", ms: Date.now() - startedAt, error: diagnosis, screenshot });
      log(`failed: ${name}: ${diagnosis}`);
      return undefined;
    }
  };
}

export function summarizeSteps(steps) {
  const count = (status) => steps.filter((entry) => entry.status === status).length;
  return { passed: count("passed"), failed: count("failed"), skipped: count("skipped"), total: steps.length };
}

export function exitCodeFor(evidence) {
  return evidence.fatal || evidence.steps.some((entry) => entry.status === "failed") ? 1 : 0;
}

/** What the run exercised: app kind, source commit, package identity and app-specific overrides. */
export function smokeRunContext({ options, env, commit, releaseReport }) {
  const appKind = /\/target\/(?:debug|release)\//.test(options.app) ? "debug" : "packaged";
  const overrides = Object.fromEntries(Object.entries(env).filter(([key]) => key.startsWith("VIDEO_CREATER_")));
  const context = {
    appKind,
    app: options.app,
    commit,
    only: options.only,
    overrides,
    sessionBus: Boolean(env.DBUS_SESSION_BUS_ADDRESS),
  };
  if (releaseReport) {
    context.package = {
      commit: releaseReport.commit,
      path: releaseReport.package?.path,
      sha256: releaseReport.package?.sha256,
      status: releaseReport.status,
    };
  }
  return context;
}
