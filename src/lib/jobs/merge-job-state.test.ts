import { describe, expect, it } from "vitest";
import { mergeJobState, projectContentEqual } from "@/lib/jobs/merge-job-state";
import type {
  GeneratedAsset,
  ProjectExportArtifact,
  ProjectJobSummary,
  ProjectRenderReport,
  VideoProject,
} from "@/lib/project";
import { fixtureGeneratedAsset, fixtureProject } from "@/test-utils/editor-fixtures";

function project(revision: number): VideoProject {
  return { ...fixtureProject(), schemaVersion: 2, contentRevision: revision };
}

function job(id: string, status: ProjectJobSummary["status"], updatedAt: string, kind = "generate_media"): ProjectJobSummary {
  return { id, kind, status, updatedAt };
}

function asset(base: VideoProject, id: string, status: GeneratedAsset["status"], outputs: GeneratedAsset["outputs"] = []): GeneratedAsset {
  return { ...fixtureGeneratedAsset(base), id, status, outputs };
}

function report(id: string, status: ProjectRenderReport["status"]): ProjectRenderReport {
  return {
    schemaVersion: 1,
    id,
    status,
    outputPath: `renders/${id}/output.webm`,
    durationSeconds: 4,
    streams: { video: true, audio: true },
    checks: {},
    artifacts: [],
    logPath: `renders/${id}/render.log`,
    createdAt: "2026-09-15T10:00:00Z",
  };
}

function artifact(id: string, jobId: string): ProjectExportArtifact {
  return {
    schemaVersion: 1,
    id,
    kind: "mp4",
    format: "mp4",
    path: `exports/${id}.mp4`,
    mimeType: "video/mp4",
    jobId,
    createdAt: "2026-09-15T10:00:00Z",
  };
}

/** The current project with a local timeline edit the loaded (polled) project does not have yet. */
function withTimelineEdit(base: VideoProject): VideoProject {
  const [track, ...rest] = base.timeline.tracks;
  if (!track) throw new Error("sample timeline has no tracks");
  return { ...base, timeline: { ...base.timeline, tracks: [{ ...track, items: track.items.slice(1) }, ...rest] } };
}

describe("mergeJobState", () => {
  it("merges a loaded job status update without touching a concurrent timeline edit", () => {
    const base = project(4);
    const current = withTimelineEdit({ ...base, jobs: [...base.jobs, job("gen-1", "running", "2026-09-15T10:00:00Z")] });
    const loaded = { ...base, jobs: [...base.jobs, job("gen-1", "completed", "2026-09-15T10:00:05Z")] };

    const { project: merged, externalChange } = mergeJobState(current, loaded);

    expect(externalChange).toBe(false);
    expect(merged.timeline).toBe(current.timeline);
    expect(merged.jobs.find((entry) => entry.id === "gen-1")?.status).toBe("completed");
  });

  it("adds a newly recorded job and keeps an older local job update", () => {
    const base = project(4);
    const current = { ...base, jobs: [job("a", "failed", "2026-09-15T10:00:09Z")] };
    const loaded = { ...base, jobs: [job("a", "running", "2026-09-15T10:00:01Z"), job("b", "queued", "2026-09-15T10:00:02Z")] };

    const merged = mergeJobState(current, loaded).project;

    expect(merged.jobs.map((entry) => [entry.id, entry.status])).toEqual([
      ["a", "failed"],
      ["b", "queued"],
    ]);
  });

  it("adds the outputs and output media of a generated asset that completed", () => {
    const base = project(4);
    const output = { mediaId: "gen-1-output", relativePath: "generated/gen-1.mp4", width: 640, height: 360, durationSeconds: 4, fps: 24 };
    const current = {
      ...base,
      generatedAssets: [...base.generatedAssets, asset(base, "gen-1", "running")],
      jobs: [...base.jobs, job("gen-1", "running", "2026-09-15T10:00:00Z")],
    };
    const loaded = {
      ...base,
      contentRevision: 5,
      media: [...base.media, { id: "gen-1-output", relativePath: "generated/gen-1.mp4", kind: "generated" as const, durationSeconds: 4, width: 640, height: 360, fps: 24 }],
      generatedAssets: [...base.generatedAssets, asset(base, "gen-1", "completed", [output])],
      jobs: [...base.jobs, job("gen-1", "completed", "2026-09-15T10:00:05Z")],
    };

    const { project: merged, externalChange } = mergeJobState(current, loaded);

    expect(externalChange).toBe(false);
    const mergedAsset = merged.generatedAssets.find((entry) => entry.id === "gen-1");
    expect(mergedAsset?.status).toBe("completed");
    expect(mergedAsset?.outputs).toEqual([output]);
    expect(merged.media.map((entry) => entry.id)).toContain("gen-1-output");
    expect(merged.contentRevision).toBe(5);
  });

  it("keeps a terminal generated asset over a non-terminal one, whichever side has it", () => {
    const base = project(4);
    const cancelled = { ...base, generatedAssets: [asset(base, "gen-1", "cancelled")], jobs: [job("gen-1", "cancelled", "2026-09-15T10:00:00Z")] };
    const running = { ...base, generatedAssets: [asset(base, "gen-1", "running")], jobs: [job("gen-1", "running", "2026-09-15T10:00:09Z")] };

    expect(mergeJobState(cancelled, running).project.generatedAssets[0]?.status).toBe("cancelled");
    expect(mergeJobState(running, cancelled).project.generatedAssets[0]?.status).toBe("cancelled");
  });

  it("picks the generated asset whose job is newer when both are terminal or both are active", () => {
    const base = project(4);
    const queued = { ...base, generatedAssets: [asset(base, "gen-1", "queued")], jobs: [job("gen-1", "queued", "2026-09-15T10:00:00Z")] };
    const running = { ...base, generatedAssets: [asset(base, "gen-1", "running")], jobs: [job("gen-1", "running", "2026-09-15T10:00:03Z")] };
    const failed = { ...base, generatedAssets: [asset(base, "gen-1", "failed")], jobs: [job("gen-1", "failed", "2026-09-15T10:00:01Z")] };
    const completed = { ...base, generatedAssets: [asset(base, "gen-1", "completed")], jobs: [job("gen-1", "completed", "2026-09-15T10:00:07Z")] };

    expect(mergeJobState(running, queued).project.generatedAssets[0]?.status).toBe("running");
    expect(mergeJobState(queued, running).project.generatedAssets[0]?.status).toBe("running");
    expect(mergeJobState(completed, failed).project.generatedAssets[0]?.status).toBe("completed");
    expect(mergeJobState(failed, completed).project.generatedAssets[0]?.status).toBe("completed");
  });

  it("never drops outputs when the winning generated asset has none", () => {
    const base = project(4);
    const output = { mediaId: "out", relativePath: "generated/out.mp4", width: 1, height: 1, durationSeconds: 1, fps: 24 };
    const current = { ...base, generatedAssets: [asset(base, "gen-1", "completed", [output])], jobs: [job("gen-1", "completed", "2026-09-15T10:00:00Z")] };
    const loaded = { ...base, generatedAssets: [asset(base, "gen-1", "failed")], jobs: [job("gen-1", "failed", "2026-09-15T10:00:05Z")] };

    const merged = mergeJobState(current, loaded).project.generatedAssets[0];

    expect(merged?.status).toBe("failed");
    expect(merged?.outputs).toEqual([output]);
  });

  it("unions duplicate render reports and export artifacts by id", () => {
    const base = project(4);
    const current = { ...base, renderReports: [report("render-1", "running"), report("render-2", "completed")], exportArtifacts: [artifact("export-1", "render-1")] };
    const loaded = {
      ...base,
      renderReports: [report("render-1", "completed"), report("render-2", "completed"), report("render-3", "failed")],
      exportArtifacts: [artifact("export-1", "render-1"), artifact("export-2", "render-3")],
    };

    const merged = mergeJobState(current, loaded).project;

    expect(merged.renderReports.map((entry) => [entry.id, entry.status])).toEqual([
      ["render-1", "completed"],
      ["render-2", "completed"],
      ["render-3", "failed"],
    ]);
    expect(merged.exportArtifacts?.map((entry) => entry.id)).toEqual(["export-1", "export-2"]);
  });

  it("keeps a terminal render report over a stale running copy", () => {
    const base = project(4);
    const current = { ...base, renderReports: [report("render-1", "failed")] };
    const loaded = { ...base, renderReports: [report("render-1", "running")] };

    expect(mergeJobState(current, loaded).project.renderReports[0]?.status).toBe("failed");
  });

  it("lets a newer loaded revision with different content win entirely", () => {
    const base = project(4);
    const current = { ...base, jobs: [...base.jobs, job("gen-1", "running", "2026-09-15T10:00:00Z")] };
    const loaded = withTimelineEdit({ ...base, contentRevision: 6, name: "Renamed by the agent" });

    const result = mergeJobState(current, loaded);

    expect(result.externalChange).toBe(true);
    expect(result.project).toBe(loaded);
  });

  it("treats a loaded project with another id as an external change", () => {
    const current = project(4);
    const loaded = { ...project(1), id: "another-project" };

    expect(mergeJobState(current, loaded)).toEqual({ project: loaded, externalChange: true });
  });

  it("keeps current content when a stale loaded revision differs, merging only ordered bookkeeping", () => {
    const base = project(4);
    const current = withTimelineEdit({ ...base, contentRevision: 7 });
    const loaded = {
      ...base,
      contentRevision: 6,
      transcripts: [],
      jobs: [...base.jobs, job("render-1", "completed", "2026-09-15T10:00:00Z", "render_draft")],
    };

    const { project: merged, externalChange } = mergeJobState(current, loaded);

    expect(externalChange).toBe(false);
    expect(merged.timeline).toBe(current.timeline);
    expect(merged.transcripts).toBe(current.transcripts);
    expect(merged.contentRevision).toBe(7);
    expect(merged.jobs.map((entry) => entry.id)).toContain("render-1");
  });

  it("never brings back a generation a newer write removed, with its generation job and output media", () => {
    const base = project(4);
    const output = { mediaId: "agent-shot-1-output", relativePath: "generated/agent-shot-1/output.mp4", width: 640, height: 360, durationSeconds: 4, fps: 24 };
    const outputMedia = { id: "agent-shot-1-output", relativePath: output.relativePath, kind: "generated" as const, durationSeconds: 4, width: 640, height: 360, fps: 24 };
    const agentJob = { ...job("job-agent-shot-1", "running", "2026-09-15T10:00:01Z"), startRequest: { workflowId: "wf", workflowType: "generateMedia", taskQueue: "q", input: { assetId: "agent-shot-1" }, searchAttributes: {}, activityTypes: [], idReusePolicy: "rejectDuplicate" } };
    const withGeneration = {
      ...base,
      media: [...base.media, outputMedia],
      generatedAssets: [...base.generatedAssets, asset(base, "agent-shot-1", "completed", [output])],
      jobs: [...base.jobs, agentJob, job("render-1", "completed", "2026-09-15T10:00:00Z", "render_draft")],
    };
    // An agent Undo removed the generation at revision 5; the job bookkeeping of other work stays.
    const undone = { ...base, contentRevision: 5, jobs: [...base.jobs, job("render-1", "completed", "2026-09-15T10:00:00Z", "render_draft")] };

    // A poll of the undone folder while the editor still shows the generation.
    const newer = mergeJobState(withGeneration, undone).project;
    expect(newer.generatedAssets.map((entry) => entry.id)).not.toContain("agent-shot-1");
    expect(newer.jobs.map((entry) => entry.id)).toEqual([...base.jobs.map((entry) => entry.id), "render-1"]);
    expect(newer.media.map((entry) => entry.id)).not.toContain("agent-shot-1-output");

    // A poll that loaded the folder before the Undo, merged after the editor took the undone project.
    const stale = mergeJobState(undone, { ...withGeneration, contentRevision: 4 }).project;
    expect(stale.generatedAssets.map((entry) => entry.id)).not.toContain("agent-shot-1");
    expect(stale.jobs.map((entry) => entry.id)).not.toContain("job-agent-shot-1");
    expect(stale.media.map((entry) => entry.id)).not.toContain("agent-shot-1-output");
  });

  it("takes worker-owned transcripts, analysis and thread from a newer equal-content revision", () => {
    const base = project(4);
    const transcript = { id: "transcript-voiceover", mediaId: "media-voiceover", repairs: [], segments: [], words: [] };
    const loaded = {
      ...base,
      contentRevision: 5,
      updatedAt: "2026-09-15T11:00:00Z",
      codexThreadId: "thread-1",
      transcripts: [...base.transcripts, transcript],
      mediaAnalysis: [{ mediaId: "media-1", sourceIn: 0, sourceOut: 1 }],
      mediaSilenceRanges: [],
    };

    const { project: merged, externalChange } = mergeJobState(base, loaded);

    expect(externalChange).toBe(false);
    expect(merged.transcripts.map((entry) => entry.id)).toEqual(["transcript-media-1", "transcript-voiceover"]);
    expect(merged.mediaAnalysis).toEqual(loaded.mediaAnalysis);
    expect(merged.mediaSilenceRanges).toEqual([]);
    expect(merged.codexThreadId).toBe("thread-1");
    expect(merged.updatedAt).toBe("2026-09-15T11:00:00Z");
    expect(merged.contentRevision).toBe(5);
  });
});

describe("projectContentEqual", () => {
  it("ignores worker-owned bookkeeping and revision fields", () => {
    const base = project(4);
    const output = { mediaId: "gen-1-output", relativePath: "generated/gen-1.mp4", width: 1, height: 1, durationSeconds: 1, fps: 24 };
    const other: VideoProject = {
      ...base,
      contentRevision: 9,
      updatedAt: "2026-09-15T12:00:00Z",
      codexThreadId: "thread-1",
      jobs: [job("gen-1", "completed", "2026-09-15T12:00:00Z")],
      generatedAssets: [asset(base, "gen-1", "completed", [output])],
      renderReports: [report("render-1", "completed")],
      exportArtifacts: [artifact("export-1", "render-1")],
      mediaAnalysis: [{ mediaId: "media-1", sourceIn: 0, sourceOut: 1 }],
      mediaSilenceRanges: [],
      transcripts: [],
      media: [...base.media, { id: "gen-1-output", relativePath: "generated/gen-1.mp4", kind: "generated", durationSeconds: 1, width: 1, height: 1, fps: 24 }],
    };

    expect(projectContentEqual(base, other)).toBe(true);
  });

  it("ignores key order and undefined properties", () => {
    const base = project(4);
    const reordered = Object.fromEntries(Object.entries(base).reverse()) as unknown as VideoProject;

    const withUndefined = { ...reordered, activeTimelineId: undefined } as unknown as VideoProject;

    expect(projectContentEqual(base, withUndefined)).toBe(true);
  });

  it("detects timeline, media, settings and name changes", () => {
    const base = project(4);

    expect(projectContentEqual(base, withTimelineEdit(base))).toBe(false);
    expect(projectContentEqual(base, { ...base, name: "Renamed" })).toBe(false);
    expect(projectContentEqual(base, { ...base, renderSettings: { ...base.renderSettings, fps: 30 } })).toBe(false);
    expect(projectContentEqual(base, { ...base, media: base.media.slice(1) })).toBe(false);
  });
});
