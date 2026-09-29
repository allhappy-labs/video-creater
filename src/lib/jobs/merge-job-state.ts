import { generationJobAssetId, mergeProjectJobs } from "@/lib/jobs/activity-records";
import type {
  GeneratedAsset,
  ProjectExportArtifact,
  ProjectJobSummary,
  ProjectRenderReport,
  VideoProject,
} from "@/lib/project";

export interface JobStateMerge {
  readonly project: VideoProject;
  /** True when the loaded project replaced `current` because its content changed elsewhere. */
  readonly externalChange: boolean;
}

const terminalStatuses: ReadonlySet<string> = new Set(["completed", "failed", "cancelled"]);

function revision(project: VideoProject): number {
  return project.contentRevision ?? 0;
}

function statusRank(status: string): number {
  return terminalStatuses.has(status) ? 1 : 0;
}

function jobUpdatedAtMs(jobs: readonly ProjectJobSummary[], id: string): number {
  const job = jobs.find((candidate) => candidate.id === id);
  const timestamp = job ? Date.parse(job.updatedAt) : Number.NaN;
  return Number.isFinite(timestamp) ? timestamp : Number.NEGATIVE_INFINITY;
}

/**
 * Generated assets carry no `updatedAt`, so precedence is: a terminal status beats a non-terminal
 * one; otherwise the side whose job (same id) has the newer `updatedAt` wins, and loaded wins ties.
 * The winner keeps the loser's outputs when it has none, so outputs never disappear.
 */
function mergeGeneratedAsset(
  current: GeneratedAsset,
  loaded: GeneratedAsset,
  currentJobs: readonly ProjectJobSummary[],
  loadedJobs: readonly ProjectJobSummary[],
): GeneratedAsset {
  const rankDifference = statusRank(loaded.status) - statusRank(current.status);
  const loadedWins =
    rankDifference !== 0
      ? rankDifference > 0
      : jobUpdatedAtMs(loadedJobs, loaded.id) >= jobUpdatedAtMs(currentJobs, current.id);
  const [winner, loser] = loadedWins ? [loaded, current] : [current, loaded];
  return winner.outputs.length === 0 && loser.outputs.length > 0 ? { ...winner, outputs: loser.outputs } : winner;
}

/** Unions by id in `current` order, then loaded-only entries; `pick` resolves shared ids. */
function unionById<T extends { id: string }>(current: readonly T[], loaded: readonly T[], pick: (current: T, loaded: T) => T): T[] {
  const loadedById = new Map(loaded.map((entry) => [entry.id, entry] as const));
  const currentIds = new Set(current.map((entry) => entry.id));
  const merged = current.map((entry) => {
    const incoming = loadedById.get(entry.id);
    return incoming ? pick(entry, incoming) : entry;
  });
  return [...merged, ...loaded.filter((entry) => !currentIds.has(entry.id))];
}

/**
 * Generated assets only one side has that a write between the two revisions removed: current-only
 * assets when `loaded` is newer, loaded-only assets when it is stale. The folder's revisions are linear,
 * so a newer folder lacks them only because they were removed (an agent Undo removes the generations
 * its batch recorded), and a stale load still has them only because it predates that removal.
 */
function removedGeneratedAssetIds(current: VideoProject, loaded: VideoProject): Set<string> {
  if (revision(loaded) === revision(current)) return new Set();
  const [newer, older] = revision(loaded) > revision(current) ? [loaded, current] : [current, loaded];
  const kept = new Set(newer.generatedAssets.map((asset) => asset.id));
  return new Set(older.generatedAssets.flatMap((asset) => (kept.has(asset.id) ? [] : [asset.id])));
}

/** A report keeps its terminal status; otherwise the loaded copy wins. */
function pickRenderReport(current: ProjectRenderReport, loaded: ProjectRenderReport): ProjectRenderReport {
  return statusRank(current.status) > statusRank(loaded.status) ? current : loaded;
}

function pickLoaded<T>(_current: T, loaded: T): T {
  return loaded;
}

function generatedOutputMediaIds(...projects: readonly VideoProject[]): Set<string> {
  return new Set(projects.flatMap((project) => project.generatedAssets.flatMap((asset) => asset.outputs.map((output) => output.mediaId))));
}

/**
 * The content compared by the conflict rule: the project without worker-owned bookkeeping. This
 * mirrors the backend's `preserve_worker_owned_state` (split.rs): jobs, generated assets, render
 * reports, export artifacts, media analysis, silence ranges, transcripts and generated output media.
 * `contentRevision`, `updatedAt` and `codexThreadId` also change on every backend write, so they are
 * ignored too.
 */
function contentOf(project: VideoProject, outputMediaIds: ReadonlySet<string>): Record<string, unknown> {
  const {
    contentRevision: _contentRevision,
    updatedAt: _updatedAt,
    codexThreadId: _codexThreadId,
    jobs: _jobs,
    generatedAssets: _generatedAssets,
    renderReports: _renderReports,
    exportArtifacts: _exportArtifacts,
    mediaAnalysis: _mediaAnalysis,
    mediaSilenceRanges: _mediaSilenceRanges,
    transcripts: _transcripts,
    media,
    ...content
  } = project;
  return { ...content, media: media.filter((entry) => !outputMediaIds.has(entry.id)) };
}

/** Structural equality where key order is irrelevant and `undefined` properties count as absent. */
function deepEqual(left: unknown, right: unknown): boolean {
  if (Object.is(left, right)) return true;
  if (typeof left !== "object" || typeof right !== "object" || left === null || right === null) return false;
  if (Array.isArray(left) || Array.isArray(right)) {
    return Array.isArray(left) && Array.isArray(right) && left.length === right.length && left.every((entry, index) => deepEqual(entry, right[index]));
  }
  const leftRecord = left as Record<string, unknown>;
  const rightRecord = right as Record<string, unknown>;
  const keys = new Set([...Object.keys(leftRecord), ...Object.keys(rightRecord)]);
  for (const key of keys) {
    if (!deepEqual(leftRecord[key], rightRecord[key])) return false;
  }
  return true;
}

/** True when the projects differ only in worker-owned bookkeeping (see `contentOf`). */
export function projectContentEqual(left: VideoProject, right: VideoProject): boolean {
  const outputMediaIds = generatedOutputMediaIds(left, right);
  return deepEqual(contentOf(left, outputMediaIds), contentOf(right, outputMediaIds));
}

/** Loaded output media replace or join `media`, as the backend does when it preserves worker state. */
function mergeOutputMedia(current: VideoProject, loaded: VideoProject, outputMediaIds: ReadonlySet<string>): VideoProject["media"] {
  const incoming = new Map(loaded.media.filter((entry) => outputMediaIds.has(entry.id)).map((entry) => [entry.id, entry] as const));
  const media = current.media.map((entry) => incoming.get(entry.id) ?? entry);
  const present = new Set(media.map((entry) => entry.id));
  return [...media, ...[...incoming.values()].filter((entry) => !present.has(entry.id))];
}

/** Loaded transcripts replace the current transcript for the same media; others are kept. */
function mergeTranscripts(current: VideoProject, loaded: VideoProject): VideoProject["transcripts"] {
  const incoming = new Map(loaded.transcripts.map((entry) => [entry.mediaId, entry] as const));
  const transcripts = current.transcripts.map((entry) => incoming.get(entry.mediaId) ?? entry);
  const present = new Set(transcripts.map((entry) => entry.mediaId));
  return [...transcripts, ...loaded.transcripts.filter((entry) => !present.has(entry.mediaId))];
}

/**
 * Merges a polled project into the editor's project without clobbering in-flight edits.
 *
 * - A different project id, or a newer loaded revision whose content (see `projectContentEqual`)
 *   differs, means the project was edited elsewhere: the loaded project wins entirely.
 * - Otherwise `current` is kept, with `jobs` via `mergeProjectJobs`, generated assets by status
 *   precedence, and render reports and export artifacts unioned by id. A generated asset the newer
 *   side no longer has stays removed, with its generation jobs and output media, so a poll never
 *   brings back generations an agent Undo removed.
 * - Worker-owned data without its own ordering (transcripts, media analysis, silence ranges,
 *   generated output media, Codex thread) is taken from `loaded` only when it is not stale
 *   (loaded revision >= current revision), and `contentRevision` becomes the larger revision.
 */
export function mergeJobState(current: VideoProject, loaded: VideoProject): JobStateMerge {
  if (current.id !== loaded.id || (revision(loaded) > revision(current) && !projectContentEqual(current, loaded))) {
    return { project: loaded, externalChange: true };
  }

  const removed = removedGeneratedAssetIds(current, loaded);
  const removedOutputMediaIds = new Set(
    [...current.generatedAssets, ...loaded.generatedAssets].flatMap((asset) => (removed.has(asset.id) ? asset.outputs.map((output) => output.mediaId) : [])),
  );
  const generatedAssets = unionById(current.generatedAssets, loaded.generatedAssets, (currentAsset, loadedAsset) =>
    mergeGeneratedAsset(currentAsset, loadedAsset, current.jobs, loaded.jobs),
  ).filter((asset) => !removed.has(asset.id));
  const renderReports = unionById(current.renderReports, loaded.renderReports, pickRenderReport);
  const exportArtifacts: ProjectExportArtifact[] = unionById(current.exportArtifacts ?? [], loaded.exportArtifacts ?? [], pickLoaded);
  const merged: VideoProject = {
    ...current,
    jobs: mergeProjectJobs(current.jobs, loaded.jobs).filter((job) => job.kind !== "generate_media" || !removed.has(generationJobAssetId(job))),
    generatedAssets,
    renderReports,
    ...(current.exportArtifacts === undefined && exportArtifacts.length === 0 ? {} : { exportArtifacts }),
  };
  if (revision(loaded) < revision(current)) return { project: merged, externalChange: false };

  const keptMedia = merged.media.filter((entry) => !removedOutputMediaIds.has(entry.id));
  const mediaAnalysis = loaded.mediaAnalysis ?? current.mediaAnalysis;
  const mediaSilenceRanges = loaded.mediaSilenceRanges ?? current.mediaSilenceRanges;
  return {
    project: {
      ...merged,
      ...(loaded.contentRevision === undefined ? {} : { contentRevision: loaded.contentRevision }),
      updatedAt: Date.parse(loaded.updatedAt) > Date.parse(current.updatedAt) ? loaded.updatedAt : current.updatedAt,
      codexThreadId: loaded.codexThreadId ?? current.codexThreadId,
      media: mergeOutputMedia({ ...current, media: keptMedia }, loaded, generatedOutputMediaIds(merged)),
      transcripts: mergeTranscripts(current, loaded),
      ...(mediaAnalysis === undefined ? {} : { mediaAnalysis }),
      ...(mediaSilenceRanges === undefined ? {} : { mediaSilenceRanges }),
    },
    externalChange: false,
  };
}
