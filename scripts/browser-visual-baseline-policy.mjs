#!/usr/bin/env node
import { createHash } from "node:crypto";
import { existsSync, statSync, readFileSync, readdirSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  scenarioScreenshotName,
  visualQaScenarios,
} from "./browser-visual-qa.mjs";

const defaultManifestPath = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../docs/visual-qa/browser-visual-baseline-manifest.json",
);
const defaultBaselineDir = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../docs/visual-qa/browser-visual-baseline",
);

const expectedScreenshots = visualQaScenarios.map(scenarioScreenshotName);

function parseArgs(argv) {
  const options = {
    baselineDir: defaultBaselineDir,
    comparisonPath: null,
    manifestPath: defaultManifestPath,
    platform: `${process.platform}-${process.arch}`,
    threshold: 0.01,
    channelThreshold: 4,
    requireComparison: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--") {
      continue;
    } else if (value === "--baseline") {
      options.baselineDir = requireValue(value, argv[++index]);
    } else if (value === "--comparison") {
      options.comparisonPath = requireValue(value, argv[++index]);
    } else if (value === "--require-comparison") {
      options.requireComparison = true;
    } else if (value === "--manifest") {
      options.manifestPath = requireValue(value, argv[++index]);
    } else if (value === "--platform") {
      options.platform = requireValue(value, argv[++index]);
    } else if (value === "--threshold") {
      options.threshold = parseNumberArg(value, requireValue(value, argv[++index]));
    } else if (value === "--channel-threshold") {
      options.channelThreshold = parseNumberArg(value, requireValue(value, argv[++index]));
    } else if (value === "--help" || value === "-h") {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${value}`);
    }
  }

  return options;
}

function requireValue(flag, value) {
  if (!value || value.startsWith("--")) {
    throw new Error(`Missing value for ${flag}`);
  }
  return value;
}

function parseNumberArg(flag, value) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed < 0) {
    throw new Error(`${flag} must be a non-negative number`);
  }
  return parsed;
}

function printHelp() {
  console.log(`Usage: pnpm visual:qa:baseline-policy -- [--baseline docs/visual-qa/browser-visual-baseline]
       [--comparison browser-visual-baseline-comparison.json]
       [--require-comparison]
       [--manifest docs/visual-qa/browser-visual-baseline-manifest.json]
       [--platform ${process.platform}-${process.arch}]
       [--threshold 0.01] [--channel-threshold 4]

Validates the curated browser visual baseline set for release/CI gating. The
policy expects the current 7-shot browser visual QA matrix, nonempty PNG files,
an optional passing comparison JSON when supplied, and schema v2 manifest
metadata that pins each curated shot's dimensions, byte length, SHA-256 digest,
and per-platform thresholds. Use
--require-comparison for release jobs that must prove a fresh browser run was
compared against the curated baseline. By default, the policy uses
docs/visual-qa/browser-visual-baseline and
docs/visual-qa/browser-visual-baseline-manifest.json.`);
}

function inspectBaseline(baselineDir) {
  const resolvedBaselineDir = resolve(baselineDir);
  const missingScreenshots = [];
  const emptyScreenshots = [];
  const expectedScreenshotSet = new Set(expectedScreenshots);
  const unexpectedBaselineScreenshots = existsSync(resolvedBaselineDir)
    ? readdirSync(resolvedBaselineDir)
        .filter((entry) => entry.endsWith(".png"))
        .filter((entry) => !expectedScreenshotSet.has(entry))
        .sort()
    : [];

  for (const screenshot of expectedScreenshots) {
    const screenshotPath = join(resolvedBaselineDir, screenshot);
    if (!existsSync(screenshotPath)) {
      missingScreenshots.push(screenshot);
      continue;
    }
    if (statSync(screenshotPath).size <= 0) {
      emptyScreenshots.push(screenshot);
    }
  }

  return {
    baselineDir: resolvedBaselineDir,
    expectedScreenshots: expectedScreenshots.length,
    missingScreenshots,
    emptyScreenshots,
    unexpectedBaselineScreenshots,
  };
}

function inspectComparison(comparisonPath, threshold, channelThreshold, requireComparison) {
  if (!comparisonPath) {
    return {
      comparisonStatus: requireComparison ? "required" : "not_provided",
      comparisonPath: null,
      comparisonRequired: requireComparison,
      mismatches: null,
      comparisonMissingScreenshots: [],
      comparisonUnexpectedScreenshots: [],
      comparisonDuplicateScreenshots: [],
      comparisonFailedFrames: 0,
      threshold,
      channelThreshold,
    };
  }

  const resolvedComparisonPath = resolve(comparisonPath);
  if (!existsSync(resolvedComparisonPath)) {
    return {
      comparisonStatus: "missing",
      comparisonPath: resolvedComparisonPath,
      comparisonRequired: requireComparison,
      mismatches: null,
      comparisonMissingScreenshots: expectedScreenshots,
      comparisonUnexpectedScreenshots: [],
      comparisonDuplicateScreenshots: [],
      comparisonFailedFrames: 0,
      threshold,
      channelThreshold,
    };
  }

  const comparison = JSON.parse(readFileSync(resolvedComparisonPath, "utf8"));
  const comparedFrames = Array.isArray(comparison.comparedFrames)
    ? comparison.comparedFrames
    : [];
  const comparedScreenshots = comparedFrames
    .map((frame) => {
      if (typeof frame.renderedFrame === "string") {
        return basename(frame.renderedFrame);
      }
      if (typeof frame.previewFrame === "string") {
        return basename(frame.previewFrame);
      }
      return null;
    })
    .filter(Boolean);
  const comparedScreenshotSet = new Set(comparedScreenshots);
  const expectedScreenshotSet = new Set(expectedScreenshots);
  const comparisonMissingScreenshots = comparedFrames.length > 0
    ? expectedScreenshots.filter((screenshot) => !comparedScreenshotSet.has(screenshot))
    : [];
  const comparisonUnexpectedScreenshots = comparedScreenshots.filter(
    (screenshot) => !expectedScreenshotSet.has(screenshot),
  );
  const comparisonDuplicateScreenshots = comparedScreenshots.filter(
    (screenshot, index) => comparedScreenshots.indexOf(screenshot) !== index,
  );
  const comparisonFailedFrames = comparedFrames.filter(
    (frame) => frame?.passed === false,
  ).length;
  const mismatches = Array.isArray(comparison.mismatches)
    ? comparison.mismatches.length
    : Number(comparison.mismatchCount ?? comparisonFailedFrames);
  const comparisonFailed =
    comparison.ok === false ||
    comparison.status === "failed" ||
    mismatches > 0 ||
    comparisonMissingScreenshots.length > 0 ||
    comparisonUnexpectedScreenshots.length > 0 ||
    comparisonDuplicateScreenshots.length > 0 ||
    comparisonFailedFrames > 0;

  return {
    comparisonStatus: comparisonFailed ? "failed" : "passed",
    comparisonPath: resolvedComparisonPath,
    comparisonRequired: requireComparison,
    mismatches,
    comparisonMissingScreenshots,
    comparisonUnexpectedScreenshots,
    comparisonDuplicateScreenshots,
    comparisonFailedFrames,
    threshold,
    channelThreshold,
  };
}

function inspectManifest(
  manifestPath,
  baselineDir,
  threshold,
  channelThreshold,
  platform,
) {
  if (!manifestPath) {
    return {
      manifestStatus: "not_provided",
      manifestPath: null,
      manifestMissingScreenshots: [],
      manifestUnexpectedScreenshots: [],
      manifestDuplicateScreenshots: [],
      manifestMissingArtifacts: [],
      manifestUnexpectedArtifacts: [],
      manifestInvalidArtifactMetadata: [],
      baselineIntegrityStatus: "not_provided",
      baselineIntegrityFailures: [],
      platform,
      platformThresholdStatus: "not_provided",
    };
  }

  const resolvedManifestPath = resolve(manifestPath);
  if (!existsSync(resolvedManifestPath)) {
    return {
      manifestStatus: "missing",
      manifestPath: resolvedManifestPath,
      manifestMissingScreenshots: expectedScreenshots,
      manifestUnexpectedScreenshots: [],
      manifestDuplicateScreenshots: [],
      manifestMissingArtifacts: expectedScreenshots,
      manifestUnexpectedArtifacts: [],
      manifestInvalidArtifactMetadata: [],
      baselineIntegrityStatus: "not_provided",
      baselineIntegrityFailures: [],
      platform,
      platformThresholdStatus: "not_provided",
    };
  }

  const manifest = JSON.parse(readFileSync(resolvedManifestPath, "utf8"));
  const declaredScreenshots = Array.isArray(manifest.screenshots)
    ? manifest.screenshots
    : [];
  const declaredScreenshotSet = new Set(declaredScreenshots);
  const missingScreenshots = expectedScreenshots.filter(
    (screenshot) => !declaredScreenshotSet.has(screenshot),
  );
  const expectedScreenshotSet = new Set(expectedScreenshots);
  const unexpectedScreenshots = declaredScreenshots.filter(
    (screenshot) => !expectedScreenshotSet.has(screenshot),
  );
  const duplicateScreenshots = declaredScreenshots.filter(
    (screenshot, index) => declaredScreenshots.indexOf(screenshot) !== index,
  );
  const declaredArtifacts = isPlainObject(manifest.artifacts)
    ? manifest.artifacts
    : {};
  const declaredArtifactScreenshots = Object.keys(declaredArtifacts);
  const declaredArtifactSet = new Set(declaredArtifactScreenshots);
  const missingArtifacts = expectedScreenshots.filter(
    (screenshot) => !declaredArtifactSet.has(screenshot),
  );
  const unexpectedArtifacts = declaredArtifactScreenshots.filter(
    (screenshot) => !expectedScreenshotSet.has(screenshot),
  );
  const invalidArtifactMetadata = expectedScreenshots.filter(
    (screenshot) =>
      declaredArtifactSet.has(screenshot) &&
      !isValidArtifactMetadata(declaredArtifacts[screenshot]),
  );
  const manifestStatus =
    manifest.schemaVersion === 2 &&
    missingScreenshots.length === 0 &&
    unexpectedScreenshots.length === 0 &&
    duplicateScreenshots.length === 0 &&
    missingArtifacts.length === 0 &&
    unexpectedArtifacts.length === 0 &&
    invalidArtifactMetadata.length === 0
      ? "passed"
      : "failed";

  const baselineIntegrityFailures = [];
  if (manifestStatus === "passed") {
    for (const screenshot of expectedScreenshots) {
      const screenshotPath = join(resolve(baselineDir), screenshot);
      if (!existsSync(screenshotPath) || statSync(screenshotPath).size <= 0) {
        continue;
      }
      const expectedArtifact = declaredArtifacts[screenshot];
      const actualArtifact = inspectPngArtifact(screenshotPath);
      const fields = [];
      if (!actualArtifact) {
        fields.push("png");
      } else {
        for (const field of ["width", "height", "bytes", "sha256"]) {
          if (actualArtifact[field] !== expectedArtifact[field]) {
            fields.push(field);
          }
        }
      }
      if (fields.length > 0) {
        baselineIntegrityFailures.push({ screenshot, fields });
      }
    }
  }
  const baselineIntegrityStatus =
    manifestStatus === "passed" && baselineIntegrityFailures.length === 0
      ? "passed"
      : "failed";

  const platformThreshold = manifest.platformThresholds?.[platform];
  let platformThresholdStatus = "missing";
  if (platformThreshold) {
    platformThresholdStatus =
      platformThreshold.threshold === threshold &&
      platformThreshold.channelThreshold === channelThreshold
        ? "passed"
        : "failed";
  }

  return {
    manifestStatus,
    manifestPath: resolvedManifestPath,
    manifestMissingScreenshots: missingScreenshots,
    manifestUnexpectedScreenshots: unexpectedScreenshots,
    manifestDuplicateScreenshots: duplicateScreenshots,
    manifestMissingArtifacts: missingArtifacts,
    manifestUnexpectedArtifacts: unexpectedArtifacts,
    manifestInvalidArtifactMetadata: invalidArtifactMetadata,
    baselineIntegrityStatus,
    baselineIntegrityFailures,
    platform,
    platformThresholdStatus,
  };
}

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function isValidArtifactMetadata(value) {
  return (
    isPlainObject(value) &&
    Number.isInteger(value.width) &&
    value.width > 0 &&
    Number.isInteger(value.height) &&
    value.height > 0 &&
    Number.isInteger(value.bytes) &&
    value.bytes > 0 &&
    typeof value.sha256 === "string" &&
    /^[a-f0-9]{64}$/.test(value.sha256)
  );
}

function inspectPngArtifact(screenshotPath) {
  const png = readFileSync(screenshotPath);
  const expectedSignature = "89504e470d0a1a0a";
  if (
    png.length < 24 ||
    png.subarray(0, 8).toString("hex") !== expectedSignature ||
    png.subarray(12, 16).toString("ascii") !== "IHDR"
  ) {
    return null;
  }
  return {
    width: png.readUInt32BE(16),
    height: png.readUInt32BE(20),
    bytes: png.length,
    sha256: createHash("sha256").update(png).digest("hex"),
  };
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const baseline = inspectBaseline(options.baselineDir);
  const comparison = inspectComparison(
    options.comparisonPath,
    options.threshold,
    options.channelThreshold,
    options.requireComparison,
  );
  const manifest = inspectManifest(
    options.manifestPath,
    options.baselineDir,
    options.threshold,
    options.channelThreshold,
    options.platform,
  );
  const ok =
    baseline.missingScreenshots.length === 0 &&
    baseline.emptyScreenshots.length === 0 &&
    baseline.unexpectedBaselineScreenshots.length === 0 &&
    !["missing", "failed", "required"].includes(comparison.comparisonStatus) &&
    !["missing", "failed"].includes(manifest.manifestStatus) &&
    manifest.baselineIntegrityStatus === "passed" &&
    !["missing", "failed"].includes(manifest.platformThresholdStatus);
  const report = {
    ok,
    ...baseline,
    ...comparison,
    ...manifest,
  };

  console.log(JSON.stringify(report, null, 2));
  if (!ok) {
    console.error("baseline policy failed");
    process.exit(1);
  }
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(2);
}
