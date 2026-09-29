import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { validateProviderAppE2eReport } from "./provider-app-e2e-report-policy.mjs";

function fixture(overrides = {}) {
  const root = join(tmpdir(), `provider-app-policy-${randomUUID()}`);
  mkdirSync(root, { recursive: true });
  for (const artifact of ["output.mp4", "render.log"]) {
    writeFileSync(join(root, artifact), artifact);
  }
  const report = {
    schemaVersion: 1,
    status: "passed",
    providerMode: "mock-http",
    credential: "synthetic-redacted",
    render: {
      durationSeconds: 1,
      streams: { video: true, audio: false },
      artifacts: ["output.mp4", "render.log", "report.json"],
    },
    lifecycle: {
      success: "completed",
      failure: "failed-retryable",
      cancel: "cancellation-requested",
      retry: "typed-start-request-retained",
    },
    ...overrides,
  };
  const reportPath = join(root, "report.json");
  writeFileSync(reportPath, JSON.stringify(report));
  return reportPath;
}

function liveFixture(overrides = {}) {
  const root = join(tmpdir(), `provider-app-live-policy-${randomUUID()}`);
  const projectDir = join(root, "project");
  const generatedDir = join(projectDir, "generated", "provider-e2e-generated");
  mkdirSync(generatedDir, { recursive: true });
  for (const artifact of ["provider-run.json", "final-output.mp4", "render.log"]) {
    writeFileSync(join(root, artifact), artifact);
  }
  writeFileSync(join(generatedDir, "fal-output.png"), "png");
  const report = {
    schemaVersion: 2,
    status: "passed",
    providerMode: "live",
    provider: "fal.ai",
    model: "fal-ai/flux/schnell",
    scenario: "text-to-image",
    credential: { configured: true, source: "keychain", envVar: "FAL_KEY" },
    providerRequest: {
      provider: "fal.ai",
      requestId: "request-1",
      statusUrl: "https://queue.fal.run/model/requests/request-1/status",
      responseUrl: "https://queue.fal.run/model/requests/request-1",
      cancelUrl: "https://queue.fal.run/model/requests/request-1/cancel",
      submittedAt: "2026-07-12T12:00:00Z",
    },
    providerRun: {
      jobStatus: "Completed",
      outputCount: 1,
      providerReportPath: "provider-run.json",
    },
    lifecycle: {
      observed: ["queued", "running", "completed"],
      coverage: "full-transition",
      observations: [
        { status: "queued", jobStatus: "queued", assetStatus: "queued", observedAt: "2026-07-12T12:00:00Z", source: "persisted-split-project" },
        { status: "running", jobStatus: "running", assetStatus: "running", observedAt: "2026-07-12T12:00:01Z", source: "persisted-split-project" },
        { status: "completed", jobStatus: "completed", assetStatus: "completed", observedAt: "2026-07-12T12:00:02Z", source: "persisted-split-project" },
      ],
      persistedTerminalStatus: "completed",
      recovery: {
        supported: true,
        attempted: true,
        resumeCandidate: true,
        requestIdPreserved: true,
        completed: true,
        outputReloaded: true,
        method: "persisted-provider-get-only",
      },
    },
    canonical: {
      projectDir,
      generatedMediaId: "provider-e2e-generated-fal-output",
      outputRelativePath: "generated/provider-e2e-generated/fal-output.png",
      mediaKind: "image",
      persisted: true,
      reloaded: true,
      inserted: true,
      replaced: true,
      outputCardinality: 1,
      selectedOutputIndex: 0,
      selectionMethod: "explicit-zero-based-index",
    },
    render: {
      durationSeconds: 3,
      streams: { video: true, audio: false },
      checks: {
        duration: "passed",
        streams: "passed",
        captionAlignment: "skipped",
        overlayTiming: "skipped",
        artifactPaths: "passed",
        logPath: "passed",
      },
      artifacts: ["provider-run.json", "final-output.mp4", "render.log", "report.json"],
    },
    ...overrides,
  };
  const reportPath = join(root, "report.json");
  writeFileSync(reportPath, JSON.stringify(report));
  return reportPath;
}

function liveCancellationFixture(overrides = {}) {
  const root = join(tmpdir(), `provider-app-live-cancel-policy-${randomUUID()}`);
  const projectDir = join(root, "project");
  mkdirSync(join(projectDir, "jobs"), { recursive: true });
  writeFileSync(join(projectDir, "video-creater.project.json"), "{}");
  writeFileSync(join(projectDir, "jobs", "index.json"), "{}");
  const report = {
    schemaVersion: 3,
    status: "passed",
    providerMode: "live-cancellation",
    provider: "fal.ai",
    model: "fal-ai/flux/schnell",
    scenario: "provider-cancellation",
    credential: { configured: true, source: "environment", envVar: "FAL_KEY" },
    providerRequest: {
      provider: "fal.ai",
      requestId: "request-cancel-1",
      statusUrl: "https://queue.fal.run/model/requests/request-cancel-1/status",
      responseUrl: "https://queue.fal.run/model/requests/request-cancel-1",
      cancelUrl: "https://queue.fal.run/model/requests/request-cancel-1/cancel",
      submittedAt: "2026-07-12T12:00:00Z",
    },
    lifecycle: {
      observed: ["queued", "cancellation-requested", "not-found"],
      providerCancelStatus: "cancellation-requested",
      terminalObservation: "not-found",
      statusPolls: 1,
      requestIdPreserved: true,
      providerSide: true,
    },
    canonical: {
      projectDir,
      generatedAssetStatus: "cancelled",
      jobStatus: "cancelled",
      outputCount: 0,
      generatedMediaCount: 0,
      completedOutputAbsent: true,
    },
    artifacts: ["project/video-creater.project.json", "project/jobs/index.json", "report.json"],
    ...overrides,
  };
  const reportPath = join(root, "report.json");
  writeFileSync(reportPath, JSON.stringify(report));
  return reportPath;
}

test("accepts sanitized retained provider app evidence", () => {
  assert.deepEqual(validateProviderAppE2eReport(fixture()), []);
});

test("rejects credential material and escaped artifacts", () => {
  const failures = validateProviderAppE2eReport(
    fixture({
      credential: "test-fal-key",
      render: {
        durationSeconds: 1,
        streams: { video: true },
        artifacts: ["../secret", "render.log", "report.json"],
      },
    }),
  );
  assert(failures.some((failure) => failure.includes("escapes evidence root")));
  assert(failures.some((failure) => failure.includes("credential material")));
});

test("accepts live Keychain provider, recovery, canonical reload, and render evidence", () => {
  assert.deepEqual(validateProviderAppE2eReport(liveFixture()), []);
});

test("accepts accurately narrowed source-backed terminal-only legacy evidence", () => {
  assert.deepEqual(
    validateProviderAppE2eReport(
      liveFixture({
        lifecycle: {
          observed: ["completed"],
          coverage: "terminal-only",
          observations: [
            {
              status: "completed",
              jobStatus: "completed",
              assetStatus: "completed",
              observedAt: "2026-07-12T12:00:02Z",
              source: "persisted-split-project-terminal",
            },
          ],
          persistedTerminalStatus: "completed",
          recovery: {
            supported: true,
            attempted: true,
            resumeCandidate: true,
            requestIdPreserved: true,
            completed: true,
            outputReloaded: true,
            method: "persisted-provider-get-only",
          },
        },
      }),
    ),
    [],
  );
});

test("rejects synthesized lifecycle labels without source observations", () => {
  const failures = validateProviderAppE2eReport(
    liveFixture({
      lifecycle: {
        observed: ["queued", "running", "completed"],
        coverage: "full-transition",
        persistedTerminalStatus: "completed",
        recovery: {
          supported: true,
          attempted: true,
          resumeCandidate: true,
          requestIdPreserved: true,
          completed: true,
          outputReloaded: true,
          method: "persisted-provider-get-only",
        },
      },
    }),
  );
  assert(failures.some((failure) => failure.includes("source-backed observations")));
});

test("rejects full lifecycle evidence backed only by a terminal snapshot", () => {
  const reportPath = liveFixture();
  const report = JSON.parse(readFileSync(reportPath, "utf8"));
  report.lifecycle.observations[0].source = "persisted-split-project-terminal";
  report.lifecycle.observations[2].observedAt = "2026-07-12T11:59:59Z";
  writeFileSync(reportPath, JSON.stringify(report));
  const failures = validateProviderAppE2eReport(reportPath);
  assert(failures.some((failure) => failure.includes("persisted transition snapshots")));
  assert(failures.some((failure) => failure.includes("chronological timestamps")));
});

test("rejects live evidence without recovery or canonical replacement", () => {
  const failures = validateProviderAppE2eReport(
    liveFixture({
      lifecycle: {
        observed: ["queued", "completed"],
        coverage: "full-transition",
        observations: [
          { status: "queued", jobStatus: "queued", assetStatus: "queued", observedAt: "2026-07-12T12:00:00Z", source: "persisted-split-project" },
          { status: "completed", jobStatus: "completed", assetStatus: "completed", observedAt: "2026-07-12T12:00:02Z", source: "persisted-split-project" },
        ],
        persistedTerminalStatus: "completed",
        recovery: { supported: true, attempted: false },
      },
      canonical: {
        projectDir: "/tmp/escaped",
        generatedMediaId: "generated",
        outputRelativePath: "output.png",
        mediaKind: "image",
        persisted: true,
        reloaded: true,
        inserted: true,
        replaced: false,
      },
    }),
  );
  assert(failures.some((failure) => failure.includes("queued, running, completed in order")));
  assert(failures.some((failure) => failure.includes("GET-only resume")));
  assert(failures.some((failure) => failure.includes("timeline replacement")));
  assert(failures.some((failure) => failure.includes("evidence root")));
});

test("accepts live multi-image cardinality and deterministic second-output selection", () => {
  const reportPath = liveFixture();
  const report = JSON.parse(readFileSync(reportPath, "utf8"));
  report.scenario = "multi-image";
  report.providerRun.outputCount = 2;
  report.canonical.generatedMediaId = "provider-e2e-generated-fal-output-2";
  report.canonical.outputCardinality = 2;
  report.canonical.selectedOutputIndex = 1;
  writeFileSync(reportPath, JSON.stringify(report));
  assert.deepEqual(validateProviderAppE2eReport(reportPath), []);
});

test("rejects live multi-image evidence that silently selects output zero", () => {
  const failures = validateProviderAppE2eReport(
    liveFixture({
      scenario: "multi-image",
      providerRun: { jobStatus: "Completed", outputCount: 2 },
      canonical: {
        projectDir: "/tmp/escaped",
        generatedMediaId: "provider-e2e-generated-fal-output",
        outputRelativePath: "generated/provider-e2e-generated/fal-output.png",
        mediaKind: "image",
        persisted: true,
        reloaded: true,
        inserted: true,
        replaced: true,
        outputCardinality: 2,
        selectedOutputIndex: 0,
        selectionMethod: "implicit-first",
      },
    }),
  );
  assert(failures.some((failure) => failure.includes("deterministic output index 1")));
});

test("accepts sanitized live provider-side cancellation evidence", () => {
  assert.deepEqual(validateProviderAppE2eReport(liveCancellationFixture()), []);
});

test("rejects cancellation evidence with an output or non-terminal provider state", () => {
  const failures = validateProviderAppE2eReport(
    liveCancellationFixture({
      lifecycle: {
        observed: ["queued", "cancellation-requested"],
        providerCancelStatus: "already-completed",
        terminalObservation: "completed",
        statusPolls: 1,
        requestIdPreserved: true,
        providerSide: true,
      },
      canonical: {
        projectDir: "/tmp/escaped",
        generatedAssetStatus: "completed",
        jobStatus: "completed",
        outputCount: 1,
        generatedMediaCount: 1,
        completedOutputAbsent: false,
      },
    }),
  );
  assert(failures.some((failure) => failure.includes("terminal reconciliation")));
  assert(failures.some((failure) => failure.includes("no completed output")));
  assert(failures.some((failure) => failure.includes("evidence root")));
});
