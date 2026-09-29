#!/usr/bin/env node
import { existsSync, readFileSync, statSync } from "node:fs";
import { extname, isAbsolute, join, resolve } from "node:path";

const requiredProfiles = {
  mp4H264: {
    extension: ".mp4",
    videoCodecs: new Set(["h264", "avc1"]),
  },
  mp4H265: {
    extension: ".mp4",
    videoCodecs: new Set(["h265", "hevc"]),
  },
  proResMov: {
    extension: ".mov",
    videoCodecs: new Set(["prores"]),
  },
};
const requiredRuntimeFactories = [
  "mp4mux",
  "qtmux",
  "vtenc_h264",
  "vtenc_h265",
  "vtenc_prores",
];

function usage() {
  return [
    "Usage: node scripts/native-export-artifact-policy.mjs --manifest native-export-evidence.json [--project-dir /path/to/project]",
    "",
    "Validates retained native export evidence for mp4H264, mp4H265, and proResMov.",
    "The manifest must list one artifact per profile with render report, output, and ffprobe JSON paths.",
    "nativeExporterReport is required for AVFoundation evidence and must be the signed exporter verification JSON.",
    "Legacy GStreamer evidence may instead provide nativeRuntimeReport from pnpm check:native-runtime with ok=true.",
    "",
    "Manifest shape:",
    "{",
    '  "nativeExporterReport": "avfoundation-exporter.json",',
    '  "artifacts": [',
    '    { "profile": "mp4H264", "report": "renders/export-h264/pipeline-report.json", "output": "renders/export-h264/output.mp4", "probe": "renders/export-h264/ffprobe.json" }',
    "  ]",
    "}",
  ].join("\n");
}

function parseArgs(argv) {
  const args = argv.filter((arg) => arg !== "--");
  const options = {
    manifest: null,
    projectDir: process.cwd(),
  };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--manifest") {
      options.manifest = requireValue(arg, args[++index]);
    } else if (arg === "--project-dir") {
      options.projectDir = requireValue(arg, args[++index]);
    } else if (arg === "--help" || arg === "-h") {
      console.log(usage());
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}\n\n${usage()}`);
    }
  }

  if (!options.manifest) {
    throw new Error(`Missing --manifest.\n\n${usage()}`);
  }
  return options;
}

function requireValue(flag, value) {
  if (!value || value.startsWith("--")) {
    throw new Error(`Missing value for ${flag}.`);
  }
  return value;
}

function readJson(path, label) {
  const resolved = resolveEvidencePath(process.cwd(), path);
  if (!existsSync(resolved)) {
    throw new Error(`${label} was not found at ${resolved}`);
  }
  return JSON.parse(readFileSync(resolved, "utf8"));
}

function resolveEvidencePath(rootDir, value) {
  if (typeof value !== "string" || value.trim().length === 0) {
    return value;
  }
  return isAbsolute(value) ? value : join(resolve(rootDir), value);
}

function validateManifest(manifest, options) {
  const failures = [];
  const projectDir = resolve(options.projectDir);
  const artifacts = Array.isArray(manifest.artifacts) ? manifest.artifacts : [];
  const byProfile = new Map();

  if (artifacts.length === 0) {
    failures.push("manifest.artifacts must list retained native export evidence");
  }

  for (const artifact of artifacts) {
    if (!artifact || typeof artifact !== "object") {
      failures.push("manifest.artifacts entries must be objects");
      continue;
    }
    const profile = artifact.profile;
    if (!requiredProfiles[profile]) {
      failures.push(`unsupported native export profile in evidence: ${JSON.stringify(profile)}`);
      continue;
    }
    if (byProfile.has(profile)) {
      failures.push(`duplicate native export evidence for profile ${profile}`);
      continue;
    }
    byProfile.set(profile, artifact);
  }

  for (const profile of Object.keys(requiredProfiles)) {
    if (!byProfile.has(profile)) {
      failures.push(`missing native export evidence for ${profile}`);
    }
  }

  const usesAvFoundation = Boolean(manifest.nativeExporterReport);
  const nativeEvidenceField = usesAvFoundation
    ? "nativeExporterReport"
    : "nativeRuntimeReport";
  let nativeEvidencePath = null;
  if (!manifest.nativeExporterReport && !manifest.nativeRuntimeReport) {
    failures.push(
      "manifest.nativeExporterReport or legacy manifest.nativeRuntimeReport is required for native export evidence",
    );
  } else {
    nativeEvidencePath = requiredProjectPath(
      projectDir,
      manifest[nativeEvidenceField],
      `manifest.${nativeEvidenceField}`,
      failures,
    );
  }
  if (nativeEvidencePath && existsSync(nativeEvidencePath)) {
    const nativeEvidence = JSON.parse(readFileSync(nativeEvidencePath, "utf8"));
    if (usesAvFoundation) validateAvFoundationExporterReport(nativeEvidence, failures);
    else validateRuntimeReport(nativeEvidence, failures);
  } else if (nativeEvidencePath) {
    failures.push(`manifest.${nativeEvidenceField} must exist`);
  }

  for (const [profile, artifact] of byProfile) {
    validateArtifact(profile, artifact, projectDir, failures, usesAvFoundation);
  }

  return failures;
}

function readJsonRelativeTo(projectDir, value, label) {
  const resolved = resolveEvidencePath(projectDir, value);
  if (!existsSync(resolved)) {
    throw new Error(`${label} was not found at ${resolved}`);
  }
  return JSON.parse(readFileSync(resolved, "utf8"));
}

function validateRuntimeReport(report, failures) {
  if (report.ok !== true) {
    failures.push("nativeRuntimeReport.ok must be true");
  }
  const factories = Array.isArray(report.factories) ? report.factories : [];
  if (factories.length === 0) {
    failures.push("nativeRuntimeReport.factories must not be empty");
  }
  const availableFactories = new Set();
  for (const factory of factories) {
    if (factory?.ok !== true || factory?.status !== "available") {
      failures.push(`native runtime factory is not available: ${factory?.name ?? "unknown"}`);
    } else if (typeof factory.name === "string") {
      availableFactories.add(factory.name);
    }
  }
  for (const factory of requiredRuntimeFactories) {
    if (!availableFactories.has(factory)) {
      failures.push(`nativeRuntimeReport is missing required factory ${factory}`);
    }
  }
}

function validateAvFoundationExporterReport(report, failures) {
  if (report?.schemaVersion !== 5 || report?.status !== "passed") {
    failures.push("nativeExporterReport must be a passing AVFoundation protocol v5 report");
  }
  if (report?.target !== "aarch64-apple-darwin") {
    failures.push("nativeExporterReport target must be aarch64-apple-darwin");
  }
  if (report?.signed !== true || report?.signatureRequired !== true) {
    failures.push("nativeExporterReport must require and verify a code signature");
  }
  if (!Array.isArray(report?.architectures) || report.architectures.join(",") !== "arm64") {
    failures.push("nativeExporterReport architecture must be exactly arm64");
  }
  if (!Array.isArray(report?.failures) || report.failures.length > 0) {
    failures.push("nativeExporterReport failures must be an empty array");
  }
  const dependencies = Array.isArray(report?.dynamicDependencies)
    ? report.dynamicDependencies
    : [];
  if (
    dependencies.length === 0 ||
    dependencies.some(
      (dependency) =>
        typeof dependency !== "string" ||
        (!dependency.startsWith("/System/Library/") && !dependency.startsWith("/usr/lib/")),
    )
  ) {
    failures.push("nativeExporterReport must contain only system dynamic dependencies");
  }
}

function validateArtifact(profile, artifact, projectDir, failures, usesAvFoundation) {
  const policy = requiredProfiles[profile];
  const outputPath = requiredProjectPath(projectDir, artifact.output, `${profile}.output`, failures);
  const reportPath = requiredProjectPath(projectDir, artifact.report, `${profile}.report`, failures);
  const probePath = requiredProjectPath(projectDir, artifact.probe, `${profile}.probe`, failures);

  if (outputPath) {
    if (extname(outputPath).toLowerCase() !== policy.extension) {
      failures.push(`${profile}.output must end with ${policy.extension}`);
    }
    if (!nonemptyFile(outputPath)) {
      failures.push(`${profile}.output must exist and be nonempty`);
    }
  }

  const report = reportPath && existsSync(reportPath) ? JSON.parse(readFileSync(reportPath, "utf8")) : null;
  if (report) {
    const relativeOutput = artifact.output;
    if (!["completed", "succeeded"].includes(report.summary?.status)) {
      failures.push(`${profile}.report summary.status must be completed or succeeded`);
    }
    if (report.summary?.outputPath !== relativeOutput && report.summary?.output_path !== relativeOutput) {
      failures.push(`${profile}.report summary output path must match manifest output`);
    }
    if (report.streams?.video !== true) {
      failures.push(`${profile}.report must prove a video stream`);
    }
    if (!Array.isArray(report.artifacts) || !report.artifacts.includes(relativeOutput)) {
      failures.push(`${profile}.report artifacts must include the output path`);
    }
    if (Array.isArray(report.errors) && report.errors.length > 0) {
      failures.push(`${profile}.report errors must be empty`);
    }
    if (usesAvFoundation && report.command?.program !== "avfoundation-native") {
      failures.push(`${profile}.report must prove the avfoundation-native backend`);
    }
  } else if (reportPath) {
    failures.push(`${profile}.report must exist`);
  }

  const probe = probePath && existsSync(probePath) ? JSON.parse(readFileSync(probePath, "utf8")) : null;
  if (probe) {
    validateProbe(profile, probe, policy, failures);
  } else if (probePath) {
    failures.push(`${profile}.probe must exist`);
  }
}

function requiredProjectPath(projectDir, value, label, failures) {
  if (typeof value !== "string" || value.trim().length === 0) {
    failures.push(`${label} must be a non-empty project-relative path`);
    return null;
  }
  if (isAbsolute(value) || value.split(/[\\/]/).includes("..")) {
    failures.push(`${label} must be a project-relative path`);
    return null;
  }
  return join(projectDir, value);
}

function nonemptyFile(path) {
  return existsSync(path) && statSync(path).isFile() && statSync(path).size > 0;
}

function validateProbe(profile, probe, policy, failures) {
  const streams = Array.isArray(probe.streams) ? probe.streams : [];
  const video = streams.find((stream) => stream.codec_type === "video");
  if (!video) {
    failures.push(`${profile}.probe must include a video stream`);
    return;
  }
  const codec = String(video.codec_name || "").toLowerCase();
  if (!policy.videoCodecs.has(codec)) {
    failures.push(`${profile}.probe video codec ${JSON.stringify(codec)} does not match profile`);
  }
  const duration = Number(video.duration ?? probe.format?.duration);
  if (!Number.isFinite(duration) || duration <= 0) {
    failures.push(`${profile}.probe must include positive duration evidence`);
  }
  const width = Number(video.width);
  const height = Number(video.height);
  if (!Number.isFinite(width) || width <= 0 || !Number.isFinite(height) || height <= 0) {
    failures.push(`${profile}.probe must include positive video dimensions`);
  }
}

try {
  const options = parseArgs(process.argv.slice(2));
  const manifest = readJson(options.manifest, "Native export evidence manifest");
  const failures = validateManifest(manifest, options);
  const result = {
    manifest: resolve(options.manifest),
    projectDir: resolve(options.projectDir),
    status: failures.length === 0 ? "passed" : "failed",
    failures,
    requiredProfiles: Object.keys(requiredProfiles),
  };
  console.log(JSON.stringify(result, null, 2));
  if (failures.length > 0) {
    process.exitCode = 1;
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 2;
}
