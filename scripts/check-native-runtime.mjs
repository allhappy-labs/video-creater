#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { mkdirSync, renameSync, statfsSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const defaultFactories = [
  "appsink",
  "decodebin",
  "qtdemux",
  "matroskademux",
  "pngdec",
  "h264parse",
  "h265parse",
  "vtdec",
  "vp8dec",
  "vp9dec",
  "videoconvert",
  "videoscale",
  "videorate",
  "videocrop",
  "capsfilter",
  "mp4mux",
  "qtmux",
  "aacparse",
  "atdec",
  "atenc",
  "vtenc_h264",
  "vtenc_h265",
  "vtenc_prores",
  "webmmux",
  "vp8enc",
  "vp9enc",
  "opusenc",
  "opusdec",
];
const repairPackages = [
  "gstreamer",
  "gst-plugins-base",
  "gst-plugins-good",
  "gst-plugins-bad",
  "gst-editing-services",
];
const repairCommand = `brew install ${repairPackages.join(" ")}`;

function parseArgs(argv) {
  const options = {
    factories: [],
    timeoutMs: 5_000,
    diagnosticsDir: process.cwd(),
    reportPath: null,
    minFreeGb: 10,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--") {
      continue;
    } else if (value === "--factory") {
      options.factories.push(requireValue(value, argv[++index]));
    } else if (value === "--timeout-ms") {
      options.timeoutMs = parsePositiveInteger(value, requireValue(value, argv[++index]));
    } else if (value === "--diagnostics-dir") {
      options.diagnosticsDir = requireValue(value, argv[++index]);
    } else if (value === "--report") {
      options.reportPath = requireValue(value, argv[++index]);
    } else if (value === "--min-free-gb") {
      options.minFreeGb = parsePositiveNumber(value, requireValue(value, argv[++index]));
    } else if (value === "--help" || value === "-h") {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${value}`);
    }
  }

  if (options.factories.length === 0) {
    options.factories = defaultFactories;
  }

  return options;
}

function requireValue(flag, value) {
  if (!value || value.startsWith("--")) {
    throw new Error(`Missing value for ${flag}`);
  }
  return value;
}

function parsePositiveInteger(flag, value) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${flag} must be a positive integer`);
  }
  return parsed;
}

function parsePositiveNumber(flag, value) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${flag} must be a positive number`);
  }
  return parsed;
}

function printHelp() {
console.log(`Usage: pnpm check:native-runtime -- [--factory mp4mux] [--timeout-ms 5000] [--diagnostics-dir .] [--report native-runtime.json] [--min-free-gb 10]

Checks provisioned native video runtime dependencies with bounded GStreamer
factory probes. The command prints a JSON report to stdout and exits nonzero
when a factory is missing, denied by the host runtime, times out, or the
workspace lacks enough free space for native export verification. Use --report
to retain the JSON evidence alongside release artifacts.`);
}

function writeReport(path, report) {
  const resolvedPath = resolve(path);
  mkdirSync(dirname(resolvedPath), { recursive: true });
  const temporaryPath = `${resolvedPath}.tmp`;
  writeFileSync(temporaryPath, `${JSON.stringify(report, null, 2)}\n`);
  renameSync(temporaryPath, resolvedPath);
  return resolvedPath;
}

function checkFactory(factory, timeoutMs) {
  const result = spawnSync("gst-inspect-1.0", [factory], {
    encoding: "utf8",
    timeout: timeoutMs,
    stdio: "pipe",
  });

  if (result.error?.code === "ETIMEDOUT" || result.signal) {
    return {
      name: factory,
      status: "timeout",
      ok: false,
      timedOut: true,
      exitCode: result.status,
      signal: result.signal,
      stderr: result.stderr || "",
      remediation: remediationForFactory(factory),
    };
  }

  if (result.error) {
    return {
      name: factory,
      status: "error",
      ok: false,
      timedOut: false,
      exitCode: result.status,
      signal: result.signal,
      error: result.error.message,
      stderr: result.stderr || "",
      remediation: remediationForFactory(factory),
    };
  }

  if (result.status !== 0) {
    return {
      name: factory,
      status: "missing",
      ok: false,
      timedOut: false,
      exitCode: result.status,
      signal: result.signal,
      stderr: result.stderr || "",
      remediation: remediationForFactory(factory),
    };
  }

  return {
    name: factory,
    status: "available",
    ok: true,
    timedOut: false,
    exitCode: result.status,
    signal: result.signal,
    remediation: null,
  };
}

function remediationForFactory(factory) {
  return `Install or repair the approved GStreamer runtime, then verify ${factory} with gst-inspect-1.0.`;
}

function repairPlan() {
  return {
    command: repairCommand,
    packages: repairPackages,
    nextSteps: [
      "Re-run pnpm check:native-runtime after installing or repairing the GStreamer runtime.",
      "If gst-inspect-1.0 still times out, restart the app process and inspect macOS service or plugin-loading errors from stderr.",
      "If Cargo or native export checks stay silent, check free disk space and clear stale build artifacts before retrying.",
      "Keep the JSON report with release validation artifacts when verifying MP4, H.265, ProRes, or WebM exports.",
    ],
  };
}

function checkStorage(path, minFreeGb) {
  const resolvedPath = resolve(path);
  const requiredFreeBytes = Math.ceil(minFreeGb * 1024 ** 3);
  try {
    mkdirSync(resolvedPath, { recursive: true });
    const stats = statfsSync(resolvedPath);
    const availableBytes = Number(stats.bavail) * Number(stats.bsize);
    const ok = availableBytes >= requiredFreeBytes;
    return {
      path: resolvedPath,
      status: ok ? "sufficient" : "insufficient",
      ok,
      availableBytes,
      requiredFreeBytes,
      remediation: ok
        ? null
        : "Free workspace or Cargo target disk space before running native export verification.",
    };
  } catch (error) {
    return {
      path: resolvedPath,
      status: "error",
      ok: false,
      availableBytes: null,
      requiredFreeBytes,
      error: error instanceof Error ? error.message : String(error),
      remediation:
        "Provide a writable diagnostics directory or run the runtime check from the project workspace.",
    };
  }
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const factories = options.factories.map((factory) =>
    checkFactory(factory, options.timeoutMs),
  );
  const storage = checkStorage(options.diagnosticsDir, options.minFreeGb);
  const report = {
    ok: factories.every((factory) => factory.ok) && storage.ok,
    timeoutMs: options.timeoutMs,
    command: "gst-inspect-1.0",
    storage,
    repair: repairPlan(),
    factories,
  };

  if (options.reportPath) {
    report.reportPath = resolve(options.reportPath);
    writeReport(report.reportPath, report);
  }

  console.log(JSON.stringify(report, null, 2));

  for (const factory of factories) {
    if (factory.status === "timeout") {
      console.error(`${factory.name} timed out after ${options.timeoutMs}ms`);
    } else if (factory.status === "missing") {
      console.error(`${factory.name} is not available from gst-inspect-1.0`);
    } else if (factory.status === "error") {
      console.error(`${factory.name} could not be inspected: ${factory.error}`);
    }
  }
  if (storage.status === "insufficient") {
    console.error("workspace storage below required free space");
  } else if (storage.status === "error") {
    console.error(`workspace storage could not be inspected: ${storage.error}`);
  }

  process.exit(report.ok ? 0 : 1);
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(2);
}
