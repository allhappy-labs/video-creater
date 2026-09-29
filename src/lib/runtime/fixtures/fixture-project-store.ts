import { applyProjectActionsLocally } from "../../agent/project-merge";
import type { ProjectAction, ProjectWriteReport, VideoProject } from "../../project";

/**
 * The in-memory project folder every stateful DEV fixture reads and writes (tasks, export,
 * conversation, project, speech and generation handlers), with the semantics of the native split
 * folder in `src-tauri/src/project/split.rs`:
 *
 * - A content write (`write`) stores the project with the next `contentRevision`. Worker bookkeeping
 *   (`record`: job status, transcripts, render reports) keeps the revision, so a poll never makes the
 *   editor's next save stale.
 * - `save` is `replace_split_project_if_revision`: a stale `expectedRevision` rejects with Rust's
 *   message, worker-owned state in the folder wins over the saved copy, and the result is schema v2.
 *   The first save of a fresh folder runs the registered seeds (seeded tasks, detected silences).
 * - `apply` is the local action applier (`applyProjectActionsLocally`) followed by a content write.
 * - `reload` is `load_split_project_from_folder`: each reload is one tick of the job progression clock,
 *   which advances every registered background run (renders, transcriptions, generations) first.
 */

/** The native write report; fixtures write no files. */
export function fixtureWriteReport(): ProjectWriteReport {
  return { manifestPath: "video-creater.project.json", writtenFiles: [], removedFiles: [] };
}

/** `SplitProjectError::RevisionConflict` as the native command rejects it (a plain string). */
export function revisionConflictMessage(expected: number, actual: number): string {
  return `project revision conflict: expected revision ${expected.toString()}, but canonical revision is ${actual.toString()}`;
}

const noSavedProjectCopy = "This folder has no saved project yet.";

/** Transforms the first project saved into a fresh folder, like a folder that already holds analysis. */
type FixtureSeed = (project: VideoProject, projectDir: string) => VideoProject;

export interface FixtureProjectStore {
  /** The project the folder holds, or null before the first save. */
  readonly current: VideoProject | null;
  /** The folder's project; throws the native "no saved project" failure before the first save. */
  require(): VideoProject;
  /** A content write: the project with the next content revision. */
  write(project: VideoProject): VideoProject;
  /** Worker bookkeeping: the project as given, keeping the folder's content revision. */
  record(project: VideoProject): VideoProject;
  /** `save_split_project_to_folder`; `expectedRevision` is checked when given. */
  save(project: VideoProject, projectDir: string, expectedRevision?: number): VideoProject;
  /** Applies the actions with the local applier as one content write. */
  apply(actions: readonly ProjectAction[]): VideoProject;
  /** One folder reload: advances the job clock, then returns the folder's project. */
  reload(): VideoProject;
  /** Runs `step` on every reload until the returned function unregisters it. */
  onReload(step: () => void): () => void;
  /** Adds a seed for the first save; seeds run in registration order. */
  addSeed(seed: FixtureSeed): void;
}

function generatedOutputMediaIds(project: VideoProject): Set<string> {
  return new Set(project.generatedAssets.flatMap((asset) => asset.outputs.map((output) => output.mediaId)));
}

/** Rust `preserve_worker_owned_state`: the folder's worker-owned data replaces the saved copy's. */
function preserveWorkerOwnedState(canonical: VideoProject, replacement: VideoProject): VideoProject {
  const outputIds = generatedOutputMediaIds(canonical);
  const media = replacement.media.map((entry) => (outputIds.has(entry.id) ? canonical.media.find((candidate) => candidate.id === entry.id) ?? entry : entry));
  for (const entry of canonical.media) {
    if (outputIds.has(entry.id) && !media.some((candidate) => candidate.id === entry.id)) media.push(entry);
  }
  const transcripts = replacement.transcripts.map((entry) => canonical.transcripts.find((candidate) => candidate.mediaId === entry.mediaId) ?? entry);
  for (const entry of canonical.transcripts) {
    if (!transcripts.some((candidate) => candidate.mediaId === entry.mediaId)) transcripts.push(entry);
  }
  const { mediaAnalysis: _analysis, mediaSilenceRanges: _silences, exportArtifacts: _artifacts, ...rest } = replacement;
  return {
    ...rest,
    ...(canonical.mediaAnalysis === undefined ? {} : { mediaAnalysis: canonical.mediaAnalysis }),
    ...(canonical.mediaSilenceRanges === undefined ? {} : { mediaSilenceRanges: canonical.mediaSilenceRanges }),
    ...(canonical.exportArtifacts === undefined ? {} : { exportArtifacts: canonical.exportArtifacts }),
    generatedAssets: canonical.generatedAssets,
    renderReports: canonical.renderReports,
    jobs: canonical.jobs,
    media,
    transcripts,
  };
}

export function createFixtureProjectStore(): FixtureProjectStore {
  let current: VideoProject | null = null;
  const steps = new Set<() => void>();
  const seeds: FixtureSeed[] = [];

  function revision(): number {
    return current?.contentRevision ?? 0;
  }

  const store: FixtureProjectStore = {
    get current() {
      return current;
    },
    require() {
      if (!current) throw noSavedProjectCopy;
      return current;
    },
    write(project) {
      current = { ...project, contentRevision: (current ? revision() : project.contentRevision ?? 0) + 1 };
      return current;
    },
    record(project) {
      current = { ...project, ...(current?.contentRevision === undefined ? {} : { contentRevision: current.contentRevision }) };
      return current;
    },
    save(project, projectDir, expectedRevision) {
      const actual = revision();
      if (expectedRevision !== undefined && expectedRevision !== actual) throw revisionConflictMessage(expectedRevision, actual);
      const replacement = current ? preserveWorkerOwnedState(current, project) : seeds.reduce((seeded, seed) => seed(seeded, projectDir), project);
      current = { ...replacement, schemaVersion: 2, contentRevision: actual + 1 };
      return current;
    },
    apply(actions) {
      return store.write(applyProjectActionsLocally(store.require(), [...actions]));
    },
    reload() {
      store.require();
      for (const step of [...steps]) step();
      return store.require();
    },
    onReload(step) {
      steps.add(step);
      return () => {
        steps.delete(step);
      };
    },
    addSeed(seed) {
      seeds.push(seed);
    },
  };
  return store;
}
