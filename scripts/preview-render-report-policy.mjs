#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import { isAbsolute, join, relative, resolve } from "node:path";

function usage() {
  return [
    "Usage: node scripts/preview-render-report-policy.mjs --report renders/job/report.json [--project-dir /path/to/project] [--min-frames 1] [--require-extracted-rendered-frames]",
    "",
    "Validates that a render report contains passed preview/render comparison evidence,",
    "that compared frame paths are recorded as artifacts, and that project-rooted",
    "artifacts exist when --project-dir is supplied.",
  ].join("\n");
}

function parseArgs(argv) {
  const args = argv.filter((arg) => arg !== "--");
  const options = {
    report: null,
    projectDir: null,
    minFrames: 1,
    requireExtractedRenderedFrames: false,
  };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--report") {
      options.report = requireValue(arg, args[++index]);
    } else if (arg === "--project-dir") {
      options.projectDir = requireValue(arg, args[++index]);
    } else if (arg === "--min-frames") {
      options.minFrames = parseInteger(arg, args[++index]);
    } else if (arg === "--require-extracted-rendered-frames") {
      options.requireExtractedRenderedFrames = true;
    } else if (arg === "--help" || arg === "-h") {
      console.log(usage());
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}\n\n${usage()}`);
    }
  }

  if (!options.report) {
    throw new Error(`Missing --report.\n\n${usage()}`);
  }
  if (options.minFrames < 1) {
    throw new Error("--min-frames must be at least 1.");
  }

  return options;
}

function requireValue(flag, value) {
  if (!value || value.startsWith("--")) {
    throw new Error(`Missing value for ${flag}.`);
  }
  return value;
}

function parseInteger(flag, value) {
  const parsed = Number(requireValue(flag, value));
  if (!Number.isInteger(parsed)) {
    throw new Error(`${flag} must be an integer.`);
  }
  return parsed;
}

function readJson(path, label) {
  const resolved = resolve(path);
  if (!existsSync(resolved)) {
    throw new Error(`${label} was not found at ${resolved}`);
  }
  return JSON.parse(readFileSync(resolved, "utf8"));
}

function validateReport(report, options) {
  const failures = [];
  const artifacts = Array.isArray(report.artifacts) ? report.artifacts : [];
  const artifactSet = new Set(artifacts);
  const comparison = report.previewComparison;
  const comparedFrames = Array.isArray(comparison?.comparedFrames)
    ? comparison.comparedFrames
    : [];

  if (report.status !== "completed") {
    failures.push(`render report status is ${JSON.stringify(report.status)}, expected "completed"`);
  }

  if (!comparison || typeof comparison !== "object") {
    failures.push("missing previewComparison");
  } else if (comparison.status !== "passed") {
    failures.push(`previewComparison status is ${JSON.stringify(comparison.status)}, expected "passed"`);
  }

  if (comparedFrames.length < options.minFrames) {
    failures.push(`previewComparison compared ${comparedFrames.length} frame(s), expected at least ${options.minFrames}`);
  }

  if (comparison && typeof comparison === "object") {
    const comparisonArtifact = comparison.artifactPath || inferComparisonArtifact(artifacts);
    validateArtifactPath({
      failures,
      artifactSet,
      projectDir: options.projectDir,
      field: "artifactPath",
      label: "previewComparison",
      value: comparisonArtifact,
    });
    validateRetainedComparisonJson({
      failures,
      projectDir: options.projectDir,
      comparisonArtifact,
      comparison,
    });
  }

  comparedFrames.forEach((frame, index) => {
    const label = `comparedFrames[${index}]`;
    if (!Number.isFinite(frame.timelineSeconds) || frame.timelineSeconds < 0) {
      failures.push(`${label}.timelineSeconds must be a non-negative finite number`);
    }
    if (frame.passed !== true) {
      failures.push(`${label}.passed is not true`);
    }
    if (!Number.isFinite(frame.mismatchRatio) || frame.mismatchRatio < 0) {
      failures.push(`${label}.mismatchRatio must be a non-negative finite number`);
    }
    for (const field of ["previewFrame", "renderedFrame"]) {
      validateArtifactPath({
        failures,
        artifactSet,
        artifacts,
        projectDir: options.projectDir,
        field,
        label,
        value: frame[field],
      });
    }
    if (
      options.requireExtractedRenderedFrames &&
      !isExtractedRenderedFramePath(frame.renderedFrame)
    ) {
      failures.push(
        `${label}.renderedFrame must come from extracted final-render frames under preview-qa/rendered-frames: ${frame.renderedFrame}`,
      );
    }
    if (frame.diffFrame) {
      validateArtifactPath({
        failures,
        artifactSet,
        artifacts,
        projectDir: options.projectDir,
        field: "diffFrame",
        label,
        value: frame.diffFrame,
      });
    }
  });

  if (options.projectDir) {
    const absoluteArtifacts = artifacts.filter((artifact) => isAbsoluteArtifact(artifact));
    for (const artifact of absoluteArtifacts) {
      failures.push(`artifact must be project-rooted, not absolute: ${artifact}`);
    }
  }

  return failures;
}

function isExtractedRenderedFramePath(value) {
  if (typeof value !== "string") {
    return false;
  }
  return value
    .replaceAll("\\", "/")
    .includes("/preview-qa/rendered-frames/");
}

function validateArtifactPath({
  failures,
  artifactSet,
  projectDir,
  field,
  label,
  value,
}) {
  if (typeof value !== "string" || value.trim().length === 0) {
    failures.push(`${label}.${field} must be a non-empty string`);
    return;
  }
  if (!artifactSet.has(value)) {
    failures.push(`${label}.${field} is not listed in report artifacts: ${value}`);
  }
  if (projectDir) {
    if (isAbsoluteArtifact(value)) {
      failures.push(`${label}.${field} must be project-rooted, not absolute: ${value}`);
      return;
    }
    const resolved = join(resolve(projectDir), value);
    if (!existsSync(resolved)) {
      failures.push(`${label}.${field} artifact file is missing: ${value}`);
    }
  }
}

function isAbsoluteArtifact(value) {
  return typeof value === "string" && isAbsolute(value);
}

function validateRetainedComparisonJson({
  failures,
  projectDir,
  comparisonArtifact,
  comparison,
}) {
  if (
    !projectDir ||
    typeof comparisonArtifact !== "string" ||
    isAbsoluteArtifact(comparisonArtifact)
  ) {
    return;
  }

  const resolvedProjectDir = resolve(projectDir);
  const resolvedArtifact = join(resolvedProjectDir, comparisonArtifact);
  if (!existsSync(resolvedArtifact)) {
    return;
  }

  let retainedComparison;
  try {
    retainedComparison = JSON.parse(readFileSync(resolvedArtifact, "utf8"));
  } catch (error) {
    failures.push(
      `previewComparison artifact JSON could not be parsed: ${comparisonArtifact}: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
    return;
  }

  const retainedEvidence = stableStringify(
    normalizeRetainedComparisonEvidence(retainedComparison, resolvedProjectDir),
  );
  const reportEvidence = stableStringify(
    normalizeRetainedComparisonEvidence(comparison, resolvedProjectDir),
  );
  if (retainedEvidence !== reportEvidence) {
    failures.push(
      `previewComparison retained JSON does not match report evidence: ${comparisonArtifact}`,
    );
  }
}

function normalizeRetainedComparisonEvidence(value, projectDir) {
  if (Array.isArray(value)) {
    return value.map((item) => normalizeRetainedComparisonEvidence(item, projectDir));
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .filter(([key]) => key !== "artifactPath")
        .map(([key, entryValue]) => [
          key,
          normalizeRetainedComparisonEvidence(entryValue, projectDir),
        ]),
    );
  }
  if (typeof value === "string") {
    return projectRelativePath(projectDir, value);
  }
  return value;
}

function projectRelativePath(projectDir, path) {
  if (typeof path !== "string" || path.trim().length === 0 || !isAbsolute(path)) {
    return path;
  }
  const relativePath = relative(projectDir, resolve(path)).replaceAll("\\", "/");
  if (
    relativePath.length > 0 &&
    !relativePath.startsWith("../") &&
    relativePath !== ".." &&
    !isAbsolute(relativePath)
  ) {
    return relativePath;
  }
  return path;
}

function stableStringify(value) {
  return JSON.stringify(sortJson(value));
}

function sortJson(value) {
  if (Array.isArray(value)) {
    return value.map(sortJson);
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, sortJson(value[key])]),
    );
  }
  return value;
}

function inferComparisonArtifact(artifacts) {
  return artifacts.find((artifact) =>
    typeof artifact === "string" &&
    artifact.endsWith("/preview-qa/preview-comparison.json")
  );
}

try {
  const options = parseArgs(process.argv.slice(2));
  const report = readJson(options.report, "Render report");
  const failures = validateReport(report, options);
  const result = {
    report: resolve(options.report),
    projectDir: options.projectDir ? resolve(options.projectDir) : null,
    status: failures.length === 0 ? "passed" : "failed",
    failures,
    comparedFrameCount: report.previewComparison?.comparedFrames?.length ?? 0,
  };
  console.log(JSON.stringify(result, null, 2));
  if (failures.length > 0) {
    process.exitCode = 1;
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 2;
}
