#!/usr/bin/env node

import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { spawnSync } from "node:child_process";

import {
  evaluateFactoryProvenance,
  isSystemDependency,
  rejectDeniedDependency,
  rejectExternalPackageManagerDependency,
  requiredFactories,
  validateRuntimeManifest,
} from "./gstreamer-runtime-policy.mjs";
import {
  parseMachORpaths,
  planPackagedRuntimeRpaths,
} from "./packaged-runtime-rpaths.mjs";

const repoRoot = resolve(import.meta.dirname, "..");

export function verifyGstreamerRuntime({
  runtimeRoot,
  target = "aarch64-apple-darwin",
  probe = true,
  timeoutMs = 5_000,
}) {
  const root = resolve(runtimeRoot);
  const manifestPath = join(root, "manifest.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  if (manifest.target !== target) {
    throw new Error(`runtime target mismatch: expected ${target}, got ${manifest.target}`);
  }
  validateRuntimeManifest(manifest, { runtimeRoot: root });

  const actualFiles = listFiles(root)
    .map((path) => relative(root, path))
    .filter((path) => path !== "manifest.json")
    .sort();
  const manifestFiles = manifest.files.map((entry) => entry.path).sort();
  if (JSON.stringify(actualFiles) !== JSON.stringify(manifestFiles)) {
    throw new Error("runtime contains files outside its immutable manifest");
  }

  const dependenciesByPath = new Map();
  for (const entry of manifest.files) {
    const path = join(root, entry.path);
    if (!isMachO(entry.path)) continue;
    const dependencies = dynamicDependencies(path);
    dependenciesByPath.set(entry.path, dependencies);
    if (JSON.stringify(dependencies) !== JSON.stringify(entry.machODependencies)) {
      throw new Error(`Mach-O dependency manifest drift: ${entry.path}`);
    }
    for (const dependency of dependencies) {
      verifyRelocatedDependency(root, path, dependency);
    }
    const rpathPlan = planPackagedRuntimeRpaths({
      existingRpaths: machORpaths(path),
      requiredRpaths: [],
    });
    if (rpathPlan.deleteRpaths.length > 0 || rpathPlan.addRpaths.length > 0) {
      throw new Error(`forbidden packaged runtime rpath: ${entry.path}`);
    }
  }

  let factories = manifest.factories;
  let gesSmoke = { status: "skipped" };
  if (probe) {
    const runtimeProbe = runNativeRuntimeProbe({
      runtimeRoot: root,
      timeoutMs: Math.max(timeoutMs, 15_000),
    });
    factories = runtimeProbe.factories;
    gesSmoke = runtimeProbe.gesSmoke;
    for (const factory of factories) {
      const expected = manifest.factories.find((entry) => entry.name === factory.name);
      if (!expected || JSON.stringify(expected) !== JSON.stringify(factory)) {
        throw new Error(`factory provenance drift: ${factory.name}`);
      }
    }
  }

  return {
    schemaVersion: 1,
    status: "passed",
    target,
    runtimeRoot: root,
    manifestPath,
    gstreamerVersion: manifest.gstreamerVersion,
    gesVersion: manifest.gesVersion,
    files: manifest.files.length,
    plugins: manifest.plugins,
    factories,
    gesSmoke,
    machOFiles: [...dependenciesByPath.keys()],
  };
}

export function probeFactories({ runtimeRoot, sourceRoot, timeoutMs = 5_000 }) {
  void sourceRoot;
  return runNativeRuntimeProbe({
    runtimeRoot,
    timeoutMs: Math.max(timeoutMs, 15_000),
  }).factories;
}

export function smokeGesTimeline({
  runtimeRoot,
  sourceRoot,
  timeoutMs = 10_000,
}) {
  void sourceRoot;
  return runNativeRuntimeProbe({
    runtimeRoot,
    timeoutMs: Math.max(timeoutMs, 15_000),
  }).gesSmoke;
}

export function runProbeWithTimeoutRetry({ spawnProbe }) {
  const attempts = [];
  const maxAttempts = 2;
  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    const result = spawnProbe(attempt);
    const timedOut = result.error?.code === "ETIMEDOUT" || Boolean(result.signal);
    attempts.push(probeAttemptEvidence({ attempt, result, timedOut }));
    if (!timedOut || attempt === maxAttempts) return { attempts, result };
  }
  throw new Error("native GStreamer/GES probe retry loop exhausted without a result");
}

export function runNativeRuntimeProbe({
  runtimeRoot,
  spawnProbe = spawnNativeProbe,
  timeoutMs = 10_000,
}) {
  const root = resolve(runtimeRoot);
  const probeDirectory = mkdtempSync(
    join(tmpdir(), "video-creater-ges-smoke-"),
  );
  const imagePath = join(probeDirectory, "frame.png");
  copyFileSync(
    join(
      repoRoot,
      "docs",
      "visual-qa",
      "browser-visual-baseline",
      "home-desktop.png",
    ),
    imagePath,
  );
  const executable = join(root, "libexec", "gstreamer-runtime-probe");
  try {
    const outcome = runProbeWithTimeoutRetry({
      spawnProbe(attempt) {
        const registryDirectory = join(probeDirectory, `attempt-${attempt}`);
        mkdirSync(registryDirectory);
        const environment = runtimeEnvironment(root, registryDirectory);
        rmSync(environment.GST_REGISTRY_1_0, { force: true });
        return spawnProbe({
          attempt,
          executable,
          args: ["--image", imagePath, ...requiredFactories],
          options: {
            encoding: "utf8",
            env: environment,
            timeout: timeoutMs,
          },
        });
      },
    });
    const { attempts, result } = outcome;
    if (result.error?.code === "ETIMEDOUT" || result.signal) {
      throw new Error(
        `native GStreamer/GES probe timed out after ${timeoutMs}ms:\n${formatProbeAttempts(attempts)}`,
      );
    }
    if (result.error || result.status !== 0) {
      throw new Error(
        `native GStreamer/GES probe failed:\n${formatProbeAttempts(attempts)}`,
      );
    }
    const factories = result.stdout
      .split(/\r?\n/)
      .filter((line) => line.startsWith("FACTORY\t"))
      .map((line) => {
        const [, name, pluginName, packageName, license] = line.split("\t");
        const factory = {
          name,
          pluginName,
          package: packageName,
          license,
        };
        const decision = evaluateFactoryProvenance(factory);
        if (decision.verdict !== "allowed") throw new Error(decision.reason);
        return factory;
      });
    if (factories.length !== requiredFactories.length) {
      throw new Error(
        `native GStreamer/GES probe returned ${factories.length} of ${requiredFactories.length} required factories`,
      );
    }
    if (!result.stdout.split(/\r?\n/).includes("GES_SMOKE\tpassed\timagefreeze+gio+nle")) {
      throw new Error("native GStreamer/GES probe omitted the real timeline smoke result");
    }
    return {
      factories,
      gesSmoke: {
        status: "passed",
        executable,
        probeAttempts: attempts,
        timeline: {
          source: "png",
          durationSeconds: 0.1,
          videoSink: "fakesink",
          requiredPlugins: ["imagefreeze", "gio", "nle"],
        },
      },
    };
  } finally {
    rmSync(probeDirectory, { recursive: true, force: true });
  }
}

function spawnNativeProbe({ executable, args, options }) {
  return spawnSync(executable, args, options);
}

function probeAttemptEvidence({ attempt, result, timedOut }) {
  return {
    attempt,
    error: result.error
      ? {
          code: result.error.code ?? null,
          message: result.error.message ?? String(result.error),
        }
      : null,
    signal: result.signal ?? null,
    status: result.status ?? null,
    stderr: result.stderr ?? "",
    stdout: result.stdout ?? "",
    timedOut,
  };
}

function formatProbeAttempts(attempts) {
  return attempts
    .map(
      ({ attempt, error, signal, status, stderr, stdout, timedOut }) =>
        `attempt ${attempt}: status=${JSON.stringify(status)} signal=${JSON.stringify(signal)} timedOut=${timedOut} error=${JSON.stringify(error)} stdout=${JSON.stringify(stdout)} stderr=${JSON.stringify(stderr)}`,
    )
    .join("\n");
}

function runtimeEnvironment(root, registryDirectory) {
  return {
    ...process.env,
    DYLD_LIBRARY_PATH: join(root, "lib"),
    GST_PLUGIN_PATH_1_0: join(root, "plugins"),
    GST_PLUGIN_SYSTEM_PATH_1_0: "",
    GST_PLUGIN_SCANNER: join(root, "libexec", "gst-plugin-scanner"),
    GST_REGISTRY_FORK: "no",
    GST_REGISTRY_1_0: join(registryDirectory, "registry.bin"),
  };
}

export function parsePluginDetails(name, output) {
  const pluginSection = output.match(/Plugin Details:\s*\n([\s\S]*?)(?:\n\n|\nElement Flags:)/)?.[1];
  if (!pluginSection) throw new Error(`gst-inspect omitted plugin details: ${name}`);
  const fields = new Map(
    pluginSection
      .split(/\r?\n/)
      .map((line) => line.match(/^\s{2}(.+?)\s{2,}(.+?)\s*$/))
      .filter(Boolean)
      .map((match) => [match[1], match[2]]),
  );
  const factory = {
    name,
    pluginName: fields.get("Name") ?? "",
    package: fields.get("Binary package") ?? "",
    license: fields.get("License") ?? "",
  };
  if (!factory.pluginName || !factory.package || !factory.license) {
    throw new Error(`gst-inspect plugin provenance is incomplete: ${name}`);
  }
  return factory;
}

function verifyRelocatedDependency(root, ownerPath, dependency) {
  rejectDeniedDependency(dependency);
  rejectExternalPackageManagerDependency(dependency);
  if (isSystemDependency(dependency)) return;
  if (!dependency.startsWith("@loader_path/")) {
    throw new Error(
      `non-system dependency must be @loader_path-relative: ${dependency} from ${basename(ownerPath)}`,
    );
  }
  const resolved = resolve(dirname(ownerPath), dependency.slice("@loader_path/".length));
  const normalizedRoot = `${resolve(root)}/`;
  if (!resolved.startsWith(normalizedRoot) || !listFiles(root).includes(resolved)) {
    throw new Error(`relocated dependency is not staged: ${dependency}`);
  }
}

function dynamicDependencies(path) {
  const result = checked("otool", ["-L", path]);
  return result
    .split(/\r?\n/)
    .slice(1)
    .map((line) => line.trim().match(/^(\S+)/)?.[1])
    .filter(Boolean)
    .filter((dependency) => basename(dependency) !== basename(path));
}

function machORpaths(path) {
  return parseMachORpaths(checked("otool", ["-l", path]));
}

function isMachO(path) {
  return (
    path.endsWith(".dylib") ||
    path === "libexec/gst-plugin-scanner" ||
    path === "libexec/gstreamer-runtime-probe"
  );
}

function checked(command, args) {
  const result = spawnSync(command, args, { encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`${command} failed: ${result.stderr || result.stdout}`);
  }
  return `${result.stdout ?? ""}${result.stderr ?? ""}`;
}

function listFiles(root) {
  return readdirSync(root, { withFileTypes: true })
    .flatMap((entry) => {
      const path = join(root, entry.name);
      if (entry.isSymbolicLink()) {
        throw new Error(`runtime may not contain symlinks: ${path}`);
      }
      return entry.isDirectory() ? listFiles(path) : [path];
    })
    .sort();
}

function parseArgs(argv) {
  const options = {
    target: "aarch64-apple-darwin",
    runtimeRoot: "src-tauri/resources/render-runtime",
    probe: true,
    timeoutMs: 5_000,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--target") options.target = requireValue(argument, argv[++index]);
    else if (argument === "--runtime") {
      options.runtimeRoot = requireValue(argument, argv[++index]);
    } else if (argument === "--timeout-ms") {
      options.timeoutMs = Number(requireValue(argument, argv[++index]));
    } else if (argument === "--skip-factory-probe") options.probe = false;
    else if (argument !== "--") throw new Error(`unknown argument: ${argument}`);
  }
  if (!Number.isInteger(options.timeoutMs) || options.timeoutMs <= 0) {
    throw new Error("--timeout-ms must be a positive integer");
  }
  return options;
}

function requireValue(flag, value) {
  if (!value || value.startsWith("--")) throw new Error(`missing value for ${flag}`);
  return value;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const result = verifyGstreamerRuntime(parseArgs(process.argv.slice(2)));
    console.log(JSON.stringify(result, null, 2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
