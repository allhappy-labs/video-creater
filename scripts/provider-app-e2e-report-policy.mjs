import { existsSync, readFileSync, statSync } from "node:fs";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const defaultReport = "output/provider-app-e2e/report.json";
const providerCredentialEnvVars = {
  "fal.ai": "FAL_KEY",
  replicate: "REPLICATE_API_TOKEN",
};

export function validateProviderAppE2eReport(reportPath = defaultReport) {
  const failures = [];
  const absoluteReport = resolve(reportPath);
  if (!existsSync(absoluteReport)) {
    return [`report is missing: ${reportPath}`];
  }
  let report;
  try {
    report = JSON.parse(readFileSync(absoluteReport, "utf8"));
  } catch (error) {
    return [`report is invalid JSON: ${error.message}`];
  }
  if (report.status !== "passed") {
    failures.push("report status must be passed");
  }
  if (report.schemaVersion === 1) {
    validateMockReport(report, failures);
  } else if (report.schemaVersion === 2) {
    validateLiveReport(report, absoluteReport, failures);
  } else if (report.schemaVersion === 3) {
    validateLiveCancellationReport(report, absoluteReport, failures);
  } else {
    failures.push("report schemaVersion must be 1 (mock), 2 (live), or 3 (live cancellation)");
  }
  if (report.schemaVersion !== 3) {
    validateRender(report, absoluteReport, failures);
  }
  validateCredentialRedaction(report, failures);
  return failures;
}

function validateLiveCancellationReport(report, absoluteReport, failures) {
  if (report.providerMode !== "live-cancellation") {
    failures.push("schemaVersion 3 providerMode must be live-cancellation");
  }
  if (!Object.hasOwn(providerCredentialEnvVars, report.provider)) {
    failures.push(`live cancellation provider must be one of ${Object.keys(providerCredentialEnvVars).join(", ")}`);
  }
  if (report.scenario !== "provider-cancellation") {
    failures.push("live cancellation scenario must be provider-cancellation");
  }
  if (typeof report.model !== "string" || report.model.trim() === "") {
    failures.push("live cancellation model must be non-empty");
  }
  const expectedEnvVar = providerCredentialEnvVars[report.provider];
  if (
    report.credential?.configured !== true ||
    !["keychain", "environment"].includes(report.credential?.source) ||
    report.credential?.envVar !== expectedEnvVar
  ) {
    failures.push("live cancellation credential metadata must prove configured Keychain/environment routing");
  }
  validateProviderRequest(report, failures);
  const lifecycle = report.lifecycle;
  if (
    lifecycle?.providerSide !== true ||
    lifecycle?.requestIdPreserved !== true ||
    !["cancellation-requested", "not-found"].includes(lifecycle?.providerCancelStatus) ||
    !["cancelled", "not-found"].includes(lifecycle?.terminalObservation) ||
    !Number.isInteger(lifecycle?.statusPolls) ||
    lifecycle.statusPolls < 1
  ) {
    failures.push("live cancellation lifecycle must prove provider-side cancellation and terminal reconciliation");
  }
  const observed = lifecycle?.observed;
  if (!Array.isArray(observed) || !observed.includes("cancellation-requested")) {
    failures.push("live cancellation lifecycle must observe cancellation-requested");
  }
  const canonical = report.canonical;
  if (
    canonical?.generatedAssetStatus !== "cancelled" ||
    canonical?.jobStatus !== "cancelled" ||
    canonical?.outputCount !== 0 ||
    canonical?.generatedMediaCount !== 0 ||
    canonical?.completedOutputAbsent !== true
  ) {
    failures.push("live cancellation canonical state must be cancelled with no completed output");
  }
  const evidenceRoot = dirname(absoluteReport);
  if (typeof canonical?.projectDir !== "string" || !isWithin(evidenceRoot, canonical.projectDir)) {
    failures.push("live cancellation canonical projectDir must stay inside the evidence root");
  }
  if (!Array.isArray(report.artifacts) || report.artifacts.length < 3) {
    failures.push("live cancellation artifacts must retain project, job, and report evidence");
  } else {
    for (const artifact of report.artifacts) {
      const candidate = resolve(evidenceRoot, artifact);
      if (!isWithin(evidenceRoot, candidate)) {
        failures.push(`artifact escapes evidence root: ${artifact}`);
      } else if (!existsSync(candidate) || !statSync(candidate).isFile()) {
        failures.push(`artifact is missing: ${artifact}`);
      }
    }
  }
}

function validateMockReport(report, failures) {
  if (report.providerMode !== "mock-http") {
    failures.push("schemaVersion 1 providerMode must be mock-http");
  }
  for (const [field, expected] of Object.entries({
    success: "completed",
    failure: "failed-retryable",
    cancel: "cancellation-requested",
    retry: "typed-start-request-retained",
  })) {
    if (report.lifecycle?.[field] !== expected) {
      failures.push(`lifecycle.${field} must equal ${expected}`);
    }
  }
}

function validateLiveReport(report, absoluteReport, failures) {
  if (report.providerMode !== "live") {
    failures.push("schemaVersion 2 providerMode must be live");
  }
  if (!Object.hasOwn(providerCredentialEnvVars, report.provider)) {
    failures.push(`live provider must be one of ${Object.keys(providerCredentialEnvVars).join(", ")}`);
  }
  for (const field of ["model", "scenario"]) {
    if (typeof report[field] !== "string" || report[field].trim() === "") {
      failures.push(`live report ${field} must be non-empty`);
    }
  }
  const expectedEnvVar = providerCredentialEnvVars[report.provider];
  if (
    report.credential?.configured !== true ||
    !["keychain", "environment"].includes(report.credential?.source) ||
    report.credential?.envVar !== expectedEnvVar
  ) {
    failures.push("live credential metadata must prove configured Keychain/environment routing");
  }
  validateProviderRequest(report, failures);
  const observed = report.lifecycle?.observed;
  const observations = report.lifecycle?.observations;
  if (!Array.isArray(observations) || observations.length === 0) {
    failures.push("live lifecycle must retain source-backed observations");
  } else {
    const derived = observations.map((observation) => observation?.status);
    if (!Array.isArray(observed) || JSON.stringify(observed) !== JSON.stringify(derived)) {
      failures.push("live lifecycle observed states must be derived from retained observations");
    }
    for (const observation of observations) {
      if (
        !["persisted-split-project", "persisted-split-project-terminal"].includes(observation?.source) ||
        observation?.status !== observation?.jobStatus ||
        observation?.status !== observation?.assetStatus ||
        typeof observation?.observedAt !== "string" ||
        observation.observedAt.trim() === ""
      ) {
        failures.push("live lifecycle observations must match persisted job and asset states");
        break;
      }
    }
  }
  if (report.lifecycle?.coverage === "full-transition") {
    if (JSON.stringify(observed) !== JSON.stringify(["queued", "running", "completed"])) {
      failures.push("full live lifecycle coverage must retain queued, running, completed in order");
    }
    if (observations?.some((observation) => observation?.source !== "persisted-split-project")) {
      failures.push("full live lifecycle coverage must use persisted transition snapshots");
    }
  } else if (report.lifecycle?.coverage === "terminal-only") {
    if (
      JSON.stringify(observed) !== JSON.stringify(["completed"]) ||
      observations?.[0]?.source !== "persisted-split-project-terminal"
    ) {
      failures.push("terminal-only lifecycle coverage must accurately retain only persisted completion");
    }
  } else {
    failures.push("live lifecycle coverage must be full-transition or terminal-only");
  }
  const observedTimes = Array.isArray(observations)
    ? observations.map((observation) => Date.parse(observation?.observedAt))
    : [];
  if (
    observedTimes.some((value) => !Number.isFinite(value)) ||
    observedTimes.some((value, index) => index > 0 && value < observedTimes[index - 1])
  ) {
    failures.push("live lifecycle observations must have chronological timestamps");
  }
  if (report.lifecycle?.persistedTerminalStatus !== "completed") {
    failures.push("live lifecycle terminal status must be durably completed");
  }
  const recovery = report.lifecycle?.recovery;
  if (
    recovery?.supported !== true ||
    recovery?.attempted !== true ||
    recovery?.resumeCandidate !== true ||
    recovery?.requestIdPreserved !== true ||
    recovery?.completed !== true ||
    recovery?.outputReloaded !== true ||
    recovery?.method !== "persisted-provider-get-only"
  ) {
    failures.push("live recovery must prove persisted GET-only resume and durable output reload");
  }
  const canonical = report.canonical;
  if (
    canonical?.persisted !== true ||
    canonical?.reloaded !== true ||
    canonical?.inserted !== true ||
    typeof canonical?.generatedMediaId !== "string" ||
    canonical.generatedMediaId.trim() === "" ||
    typeof canonical?.outputRelativePath !== "string" ||
    canonical.outputRelativePath.trim() === ""
  ) {
    failures.push("live canonical evidence must prove persisted/reloaded output insertion");
  }
  if (canonical?.mediaKind !== "audio" && canonical?.replaced !== true) {
    failures.push("live visual output must prove canonical timeline replacement");
  }
  if (report.scenario === "multi-image") {
    if (
      !Number.isInteger(canonical?.outputCardinality) ||
      canonical.outputCardinality < 2 ||
      canonical?.selectedOutputIndex !== 1 ||
      canonical?.selectionMethod !== "explicit-zero-based-index"
    ) {
      failures.push("live multi-image evidence must prove cardinality >= 2 and deterministic output index 1 selection");
    }
    if (report.providerRun?.outputCount !== canonical?.outputCardinality) {
      failures.push("live multi-image provider and canonical output cardinality must match");
    }
  }
  const evidenceRoot = dirname(absoluteReport);
  if (
    typeof canonical?.projectDir !== "string" ||
    !isWithin(evidenceRoot, canonical.projectDir)
  ) {
    failures.push("live canonical projectDir must stay inside the evidence root");
  } else {
    const outputPath = resolve(canonical.projectDir, canonical.outputRelativePath || "");
    if (
      !isWithin(canonical.projectDir, outputPath) ||
      !existsSync(outputPath) ||
      !statSync(outputPath).isFile()
    ) {
      failures.push("live canonical generated output must be a retained project file");
    }
  }
  if (
    report.providerRun?.jobStatus !== "Completed" ||
    !Number.isInteger(report.providerRun?.outputCount) ||
    report.providerRun.outputCount < 1
  ) {
    failures.push("live provider run must retain at least one completed output");
  }
}

function validateProviderRequest(report, failures) {
  const request = report.providerRequest;
  if (!request || request.provider !== report.provider) {
    failures.push("live providerRequest must match report.provider");
    return;
  }
  for (const field of ["requestId", "statusUrl", "responseUrl", "cancelUrl", "submittedAt"]) {
    if (typeof request[field] !== "string" || request[field].trim() === "") {
      failures.push(`live providerRequest.${field} must be non-empty`);
    }
  }
  for (const field of ["statusUrl", "responseUrl", "cancelUrl"]) {
    try {
      const url = new URL(request[field]);
      if (url.protocol !== "https:") {
        failures.push(`live providerRequest.${field} must use HTTPS`);
      }
    } catch {
      failures.push(`live providerRequest.${field} must be a valid URL`);
    }
  }
}

function validateRender(report, absoluteReport, failures) {
  if (!(Number.isFinite(report.render?.durationSeconds) && report.render.durationSeconds > 0)) {
    failures.push("render durationSeconds must be positive");
  }
  if (report.render?.streams?.video !== true) {
    failures.push("render must contain a video stream");
  }
  const artifacts = report.render?.artifacts;
  if (!Array.isArray(artifacts) || artifacts.length < 3) {
    failures.push("render artifacts must retain output, log, and report");
  } else {
    const root = dirname(absoluteReport);
    for (const artifact of artifacts) {
      const candidate = resolve(root, artifact);
      if (!isWithin(root, candidate)) {
        failures.push(`artifact escapes evidence root: ${artifact}`);
      } else if (!existsSync(candidate) || !statSync(candidate).isFile()) {
        failures.push(`artifact is missing: ${artifact}`);
      }
    }
  }
  if (report.schemaVersion === 2) {
    for (const check of ["duration", "streams", "artifactPaths", "logPath"]) {
      if (report.render?.checks?.[check] !== "passed") {
        failures.push(`live render check ${check} must pass`);
      }
    }
    for (const check of ["captionAlignment", "overlayTiming"]) {
      if (!["passed", "skipped"].includes(report.render?.checks?.[check])) {
        failures.push(`live render check ${check} must pass or be explicitly skipped`);
      }
    }
  }
}

function validateCredentialRedaction(report, failures) {
  const serialized = JSON.stringify(report).toLowerCase();
  for (const forbidden of [
    "test-fal-key",
    "\"authorization\"",
    "bearer ",
    "\"secret\"",
    "\"password\"",
  ]) {
    if (serialized.includes(forbidden)) {
      failures.push(`report contains forbidden credential material: ${forbidden}`);
    }
  }
}

function isWithin(rootPath, candidatePath) {
  const root = resolve(rootPath);
  const candidate = resolve(candidatePath);
  return candidate === root || candidate.startsWith(`${root}${sep}`);
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  const reportPath = process.argv.slice(2).find((argument) => argument !== "--") || defaultReport;
  const failures = validateProviderAppE2eReport(reportPath);
  if (failures.length > 0) {
    console.error(failures.join("\n"));
    process.exit(1);
  }
  console.log(JSON.stringify({ status: "passed", reportPath, artifactRoot: dirname(reportPath) }));
}
