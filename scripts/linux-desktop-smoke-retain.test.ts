import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  newExportArtifacts,
  newFailedJobs,
  readRenderPipelineReport,
  renderPathSegment,
  renderPipelineReportPath,
  requireSucceededRenderReport,
  resolveArtifactPath,
  retainProjectFiles,
} from "./linux-desktop-smoke-retain.mjs";

function withDirs(run: (projectDir: string, destinationDir: string) => void) {
  const root = mkdtempSync(join(tmpdir(), "vc-smoke-retain-"));
  try {
    const projectDir = join(root, "project");
    mkdirSync(projectDir);
    run(projectDir, join(root, "evidence"));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("retainProjectFiles copies project files keeping their relative layout", () => {
  withDirs((projectDir, destinationDir) => {
    mkdirSync(join(projectDir, "renders/job-1"), { recursive: true });
    writeFileSync(join(projectDir, "renders/job-1/pipeline-report.json"), "{\"ok\":true}");
    const retained = retainProjectFiles(projectDir, ["renders/job-1/pipeline-report.json"], destinationDir);
    assert.deepEqual(retained, [
      {
        from: "renders/job-1/pipeline-report.json",
        to: join(destinationDir, "renders/job-1/pipeline-report.json"),
        bytes: 11,
      },
    ]);
    assert.equal(readFileSync(join(destinationDir, "renders/job-1/pipeline-report.json"), "utf8"), "{\"ok\":true}");
  });
});

test("retainProjectFiles records missing files instead of failing", () => {
  withDirs((projectDir, destinationDir) => {
    assert.deepEqual(retainProjectFiles(projectDir, ["renders/gone/pipeline-report.json"], destinationDir), [
      { from: "renders/gone/pipeline-report.json", missing: true },
    ]);
  });
});

test("retainProjectFiles refuses paths outside the project", () => {
  withDirs((projectDir, destinationDir) => {
    for (const path of ["../secret.txt", "renders/../../secret.txt", "/etc/passwd"]) {
      assert.throws(() => retainProjectFiles(projectDir, [path], destinationDir), /refusing to retain a path outside the project/, path);
    }
  });
});

test("newExportArtifacts returns only new artifacts of the requested kind", () => {
  const before = [{ id: "export-1", kind: "mp4", path: "exports/a.mp4" }];
  const after = [
    ...before,
    { id: "export-2", kind: "webm", path: "exports/b.webm" },
    { id: "export-3", kind: "mp4", path: "exports/c.mp4", jobId: "export-3" },
  ];
  assert.deepEqual(newExportArtifacts(before, after, "mp4"), [{ id: "export-3", kind: "mp4", path: "exports/c.mp4", jobId: "export-3" }]);
  assert.deepEqual(newExportArtifacts(undefined, undefined, "mp4"), []);
});

test("resolveArtifactPath keeps absolute paths and joins project-relative ones", () => {
  assert.equal(resolveArtifactPath("/p", "/home/me/Movies/a.mp4"), "/home/me/Movies/a.mp4");
  assert.equal(resolveArtifactPath("/p", "exports/a.mp4"), "/p/exports/a.mp4");
});

test("renderPipelineReportPath follows the job's render report to its render folder", () => {
  withDirs((projectDir) => {
    mkdirSync(join(projectDir, "renders/export-mp4H264-AbC"), { recursive: true });
    writeFileSync(
      join(projectDir, "renders/export-mp4H264-AbC/report.json"),
      JSON.stringify({ outputPath: "renders/export-mp4h264-abc/output.mp4", status: "completed" }),
    );
    assert.equal(renderPipelineReportPath(projectDir, "export-mp4H264-AbC"), "renders/export-mp4h264-abc/pipeline-report.json");
    assert.equal(renderPipelineReportPath(projectDir, "render-denoise-1"), "renders/render-denoise-1/pipeline-report.json");
  });
});

test("renderPipelineReportPath falls back to the lowercased, sanitized render folder Rust writes", () => {
  withDirs((projectDir) => {
    assert.equal(renderPipelineReportPath(projectDir, "export-mp4H264-AbC"), "renders/export-mp4h264-abc/pipeline-report.json");
    assert.equal(renderPipelineReportPath(projectDir, " Render Job/2.1 "), "renders/render-job-2-1/pipeline-report.json");
  });
});

test("renderPathSegment matches safe_path_segment in render_pipeline/project_export.rs", () => {
  assert.equal(renderPathSegment("render-denoise-1"), "render-denoise-1");
  assert.equal(renderPathSegment("Export_MP4H264-XyZ"), "export_mp4h264-xyz");
  assert.equal(renderPathSegment("  --a b.c--  "), "a-b-c");
  assert.equal(renderPathSegment("../"), "render");
  assert.equal(renderPathSegment(""), "render");
});

test("readRenderPipelineReport reads the report from the sanitized render folder, or null while it is missing", () => {
  withDirs((projectDir) => {
    assert.equal(readRenderPipelineReport(projectDir, "export-mp4H264-AbC"), null);
    mkdirSync(join(projectDir, "renders/export-mp4h264-abc"), { recursive: true });
    writeFileSync(join(projectDir, "renders/export-mp4h264-abc/pipeline-report.json"), JSON.stringify({ summary: { status: "succeeded" } }));
    assert.deepEqual(readRenderPipelineReport(projectDir, "export-mp4H264-AbC"), {
      path: "renders/export-mp4h264-abc/pipeline-report.json",
      report: { summary: { status: "succeeded" } },
    });
  });
});

test("requireSucceededRenderReport fails when the report is missing or did not succeed", () => {
  assert.throws(() => requireSucceededRenderReport(null, "export-1"), /render pipeline report is missing for job export-1/);
  assert.throws(
    () => requireSucceededRenderReport({ path: "renders/export-1/pipeline-report.json", report: { summary: { status: "failed" } } }, "export-1"),
    /render report status failed/,
  );
  const succeeded = { path: "renders/export-1/pipeline-report.json", report: { summary: { status: "succeeded" } } };
  assert.equal(requireSucceededRenderReport(succeeded, "export-1"), succeeded);
});

test("newFailedJobs reports only jobs that failed after the export started, with their reason", () => {
  const before = [{ id: "old-failed", kind: "export_media", status: "failed" }];
  const after = [
    ...before,
    { id: "running", kind: "export_media", status: "running" },
    { id: "refused", kind: "export_media", status: "failed", failureReason: "Temporal runtime is unavailable in this build." },
    { id: "no-reason", kind: "transcribe_media", status: "failed" },
  ];
  assert.deepEqual(newFailedJobs(before, after), [
    { id: "refused", kind: "export_media", failureReason: "Temporal runtime is unavailable in this build." },
    { id: "no-reason", kind: "transcribe_media", failureReason: null },
  ]);
  assert.deepEqual(newFailedJobs(undefined, undefined), []);
});
