// Evidence retention for the Linux desktop smoke run: files the checks rely on are copied out of the
// temporary project so they outlive /tmp, and exports are found through their recorded artifacts.

import { copyFileSync, existsSync, mkdirSync, readFileSync, statSync } from "node:fs";
import { dirname, isAbsolute, join, normalize, posix, relative, resolve } from "node:path";

/** Copies project-relative files into `destinationDir`, keeping their layout. */
export function retainProjectFiles(projectDir, relativePaths, destinationDir) {
  const root = resolve(projectDir);
  return relativePaths.map((path) => {
    const from = resolve(root, path);
    const inside = relative(root, from);
    if (isAbsolute(path) || inside === "" || inside.startsWith("..") || isAbsolute(inside)) {
      throw new Error(`refusing to retain a path outside the project: ${path}`);
    }
    if (!existsSync(from)) return { from: path, missing: true };
    const to = join(destinationDir, normalize(inside));
    mkdirSync(dirname(to), { recursive: true });
    copyFileSync(from, to);
    return { from: path, to, bytes: statSync(to).size };
  });
}

/** Export artifacts recorded after `before` whose kind matches, e.g. the MP4 an export just saved. */
export function newExportArtifacts(before = [], after = [], kind) {
  const known = new Set((before ?? []).map((artifact) => artifact.id));
  return (after ?? []).filter((artifact) => !known.has(artifact.id) && artifact.kind === kind);
}

/** Jobs recorded after `before` that failed, each with the app's plain-language reason. */
export function newFailedJobs(before = [], after = []) {
  const known = new Set((before ?? []).map((job) => job.id));
  return (after ?? [])
    .filter((job) => !known.has(job.id) && job.status === "failed")
    .map(({ id, kind, failureReason }) => ({ id, kind, failureReason: failureReason ?? null }));
}

/** Recorded export paths are project-relative under exports/, or absolute for a chosen folder. */
export function resolveArtifactPath(projectDir, path) {
  return isAbsolute(path) ? path : join(projectDir, path);
}

/** The render folder name for a job id, mirroring `safe_path_segment` in src-tauri/src/render_pipeline/project_export.rs. */
export function renderPathSegment(jobId) {
  const segment = Array.from(jobId.trim().toLowerCase(), (character) => (/^[a-z0-9_-]$/.test(character) ? character : "-"))
    .join("")
    .replace(/^-+|-+$/g, "");
  return segment === "" ? "render" : segment;
}

/**
 * The project-relative pipeline report of a render job. The job's `renders/<jobId>/report.json` names its
 * output inside the render folder, whose name is a sanitized (lowercased) form of the job id.
 */
export function renderPipelineReportPath(projectDir, jobId) {
  const reportPath = join(projectDir, "renders", jobId, "report.json");
  if (existsSync(reportPath)) {
    const outputPath = JSON.parse(readFileSync(reportPath, "utf8")).outputPath;
    if (typeof outputPath === "string" && !isAbsolute(outputPath)) return posix.join(posix.dirname(outputPath), "pipeline-report.json");
  }
  return `renders/${renderPathSegment(jobId)}/pipeline-report.json`;
}

/** The job's parsed pipeline report and its project-relative path, or null while it has not been written. */
export function readRenderPipelineReport(projectDir, jobId) {
  const path = renderPipelineReportPath(projectDir, jobId);
  const absolute = join(projectDir, path);
  return existsSync(absolute) ? { path, report: JSON.parse(readFileSync(absolute, "utf8")) } : null;
}

/** Fails unless the render wrote its pipeline report and the report says it succeeded. */
export function requireSucceededRenderReport(entry, jobId) {
  if (!entry) throw new Error(`render pipeline report is missing for job ${jobId}`);
  const status = entry.report?.summary?.status;
  if (status !== "succeeded") throw new Error(`render report status ${status}`);
  return entry;
}
