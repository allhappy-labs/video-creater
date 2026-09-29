import { applyProjectActionsLocally } from "@/lib/agent/project-merge";
import {
  applyProjectActionsToSplitProjectFolder,
  saveSplitProjectToFolder,
  type ProjectAction,
  type VideoProject,
} from "@/lib/project";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import type { EditorSliceCreator } from "./editor-store";

const maxUndoSnapshots = 100;

const nonUndoableActionTypes: ReadonlySet<ProjectAction["type"]> = new Set<ProjectAction["type"]>([
  "recordJob",
  "updateJobStatus",
  "updateJobProviderRequest",
  "recordJobFailure",
  "attachRenderReport",
  "recordExportArtifact",
  "recordGeneratedAsset",
  "updateGeneratedAssetStatus",
]);

export type SaveStatus = "saved" | "saving" | "unsaved" | "failed";

/** Who made the newest undoable change: a local edit (snapshot history) or an agent batch. */
type MutationSource = "user" | "agent";

/**
 * An applied agent batch. Rust keeps its snapshot, so it sits between local snapshots: `depth` is
 * how many local snapshots lie beneath it, and it is the newest change while `past` has that length.
 */
interface AgentEditMarker {
  /** The assistant message whose card owns the batch's Undo. */
  readonly messageId: string;
  readonly depth: number;
}

interface ProjectHistory {
  readonly past: readonly VideoProject[];
  readonly future: readonly VideoProject[];
}

interface ApplyActionsOptions {
  /** Force-disable history recording (e.g. agent batches recorded elsewhere). */
  readonly recordHistory?: boolean;
}

interface ExternalStateOptions {
  /** The project changed elsewhere (see `mergeJobState`): redo history and stale selection are dropped. */
  readonly externalChange: boolean;
  /** The project `merged` was computed from. When a write has replaced it since, the merge is stale and skipped. */
  readonly base?: VideoProject;
}

export interface ProjectSlice {
  readonly projectDir: string;
  readonly project: VideoProject;
  readonly saveStatus: SaveStatus;
  readonly lastError: string | null;
  readonly history: ProjectHistory;
  /** "agent" while the newest undoable change is an agent batch; global Undo then undoes that batch. */
  readonly lastMutationSource: MutationSource;
  /** Applied agent batches that Undo can still reach, oldest first. */
  readonly agentEdits: readonly AgentEditMarker[];
  /** Commits an edit; an undoable one clears the Show changes highlights, which describe the timeline before it. */
  applyActions(actions: readonly ProjectAction[], options?: ApplyActionsOptions): Promise<VideoProject | null>;
  /** Global Undo: the newest agent batch through its card (`undoAgentEdit`), else the last local snapshot. */
  undo(): Promise<void>;
  redo(): Promise<void>;
  /** True when `undo` has something to undo, including an agent batch. */
  canUndo(): boolean;
  canRedo(): boolean;
  replaceProject(project: VideoProject): void;
  /**
   * Commits polled worker state (jobs, generated assets, reports) without an undo step. It waits for
   * pending writes so it never lands under an edit in flight, and resolves false when `base` is stale.
   */
  mergeExternalState(merged: VideoProject, options: ExternalStateOptions): Promise<boolean>;
  /**
   * Commits a project the backend already persisted for an agent batch, without a local snapshot:
   * the batch becomes the newest undoable change. Resolves false when a newer write already landed.
   */
  commitAgentApply(project: VideoProject, messageId: string): Promise<boolean>;
  /** Removes an agent batch from Undo: `project` is its undone state, or null when it can't be undone. */
  commitAgentUndo(messageId: string, project: VideoProject | null): Promise<void>;
  /** Shows user-facing copy (e.g. a blocked timeline command); cleared by the next edit. */
  setLastError(message: string | null): void;
}

class LocalOnlyProject extends Error {}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function usesSplitFolder(project: VideoProject, projectDir: string): boolean {
  return project.schemaVersion >= 2 && projectDir.trim().length > 0;
}

function shouldRecord(actions: readonly ProjectAction[], options?: ApplyActionsOptions): boolean {
  if (options?.recordHistory === false) return false;
  return actions.some((action) => !nonUndoableActionTypes.has(action.type));
}

function mutationSource(history: ProjectHistory, agentEdits: readonly AgentEditMarker[]): MutationSource {
  const newest = agentEdits[agentEdits.length - 1];
  return newest !== undefined && newest.depth === history.past.length ? "agent" : "user";
}

function undoState(history: ProjectHistory, agentEdits: readonly AgentEditMarker[]) {
  return { history, agentEdits, lastMutationSource: mutationSource(history, agentEdits) };
}

/** Records a local snapshot; markers shift down with snapshots dropped past the cap. */
function pushPast(state: Pick<ProjectSlice, "history" | "agentEdits">, snapshot: VideoProject) {
  const past = [...state.history.past, structuredClone(snapshot)];
  const dropped = Math.max(0, past.length - maxUndoSnapshots);
  const agentEdits = dropped === 0 ? state.agentEdits : state.agentEdits.map((edit) => ({ ...edit, depth: Math.max(0, edit.depth - dropped) }));
  return undoState({ past: past.slice(dropped), future: [] }, agentEdits);
}

function revisionOf(project: VideoProject): number {
  return project.contentRevision ?? 0;
}

export function createProjectSlice(init: {
  readonly projectDir: string;
  readonly project: VideoProject;
}): EditorSliceCreator<ProjectSlice> {
  return (set, get) => {
    let writeQueue: Promise<unknown> = Promise.resolve();

    function enqueue<T>(work: () => Promise<T>): Promise<T> {
      const next = writeQueue.then(work, work);
      writeQueue = next.catch(() => undefined);
      return next;
    }

    async function persistSnapshot(
      snapshot: VideoProject,
      expectedRevision: number,
    ): Promise<{ project: VideoProject; saveStatus: SaveStatus }> {
      const { projectDir } = get();
      if (!usesSplitFolder(snapshot, projectDir)) return { project: snapshot, saveStatus: "unsaved" };
      try {
        const result = await saveSplitProjectToFolder({ projectDir, project: snapshot, expectedRevision });
        return { project: result.project, saveStatus: "saved" };
      } catch (error) {
        if (isBackendUnavailableError(error)) return { project: snapshot, saveStatus: "unsaved" };
        throw error;
      }
    }

    return {
      projectDir: init.projectDir,
      project: init.project,
      saveStatus: "saved",
      lastError: null,
      history: { past: [], future: [] },
      lastMutationSource: "user",
      agentEdits: [],

      applyActions(actions, options) {
        return enqueue(async () => {
          const base = get().project;
          const { projectDir } = get();
          const record = shouldRecord(actions, options);
          set({ lastError: null });
          try {
            if (!usesSplitFolder(base, projectDir)) throw new LocalOnlyProject();
            set({ saveStatus: "saving" });
            const result = await applyProjectActionsToSplitProjectFolder({ projectDir, actions: [...actions] });
            set((state) => ({ project: result.project, saveStatus: "saved", ...(record ? pushPast(state, base) : {}) }));
            if (record) get().clearHighlights();
            return result.project;
          } catch (error) {
            if (error instanceof LocalOnlyProject || isBackendUnavailableError(error)) {
              const next = applyProjectActionsLocally(base, [...actions]);
              if (next === base) {
                set({ saveStatus: "failed", lastError: "The edit could not be applied." });
                return null;
              }
              set((state) => ({ project: next, saveStatus: "unsaved", ...(record ? pushPast(state, base) : {}) }));
              if (record) get().clearHighlights();
              return next;
            }
            set({ saveStatus: "failed", lastError: errorMessage(error) });
            return null;
          }
        });
      },

      async undo() {
        const { lastMutationSource, agentEdits } = get();
        const newest = agentEdits[agentEdits.length - 1];
        if (lastMutationSource === "agent" && newest) {
          await get().undoAgentEdit(newest.messageId);
          return;
        }
        return enqueue(async () => {
          const { history, project, agentEdits: edits } = get();
          const previous = history.past[history.past.length - 1];
          if (!previous) return;
          try {
            const restored = await persistSnapshot(previous, project.contentRevision ?? 0);
            const nextHistory = { past: history.past.slice(0, -1), future: [structuredClone(project), ...history.future] };
            set({
              project: restored.project,
              saveStatus: restored.saveStatus,
              // A marker above the remaining snapshots was reverted by this restore, so it is unreachable.
              ...undoState(nextHistory, edits.filter((edit) => edit.depth <= nextHistory.past.length)),
              lastError: null,
            });
            get().clearHighlights();
          } catch (error) {
            set({ saveStatus: "failed", lastError: errorMessage(error) });
          }
        });
      },

      redo() {
        return enqueue(async () => {
          const { history, project } = get();
          const next = history.future[0];
          if (!next) return;
          try {
            const restored = await persistSnapshot(next, project.contentRevision ?? 0);
            set((state) => ({
              project: restored.project,
              saveStatus: restored.saveStatus,
              ...undoState({ past: [...history.past, structuredClone(project)], future: history.future.slice(1) }, state.agentEdits),
              lastError: null,
            }));
            get().clearHighlights();
          } catch (error) {
            set({ saveStatus: "failed", lastError: errorMessage(error) });
          }
        });
      },

      canUndo: () => get().history.past.length > 0 || get().lastMutationSource === "agent",
      canRedo: () => get().history.future.length > 0,
      replaceProject: (project) => {
        set({ project });
        // The store subscription prunes on identity changes; prune here too so replacing with
        // the same (mutated) project object still drops stale selection.
        get().pruneSelection(project);
      },
      mergeExternalState(merged, { externalChange, base }) {
        return enqueue(async () => {
          if (base && get().project !== base) return false;
          set((state) => ({
            project: merged,
            history: externalChange ? { past: state.history.past, future: [] } : state.history,
          }));
          get().pruneSelection(merged);
          return true;
        });
      },
      commitAgentApply(project, messageId) {
        return enqueue(async () => {
          // The backend applies against the folder, so a higher revision already holds this batch.
          if (revisionOf(project) < revisionOf(get().project)) return false;
          set((state) => ({
            project,
            saveStatus: "saved",
            lastError: null,
            // Redo snapshots predate the batch; replaying them would silently revert it.
            ...undoState(
              { past: state.history.past, future: [] },
              [...state.agentEdits.filter((edit) => edit.messageId !== messageId), { messageId, depth: state.history.past.length }],
            ),
          }));
          get().pruneSelection(project);
          get().clearHighlights();
          return true;
        });
      },
      commitAgentUndo(messageId, project) {
        return enqueue(async () => {
          const restored = project !== null && revisionOf(project) >= revisionOf(get().project) ? project : null;
          set((state) => ({
            ...(restored ? { project: restored, saveStatus: "saved" as const, lastError: null } : {}),
            ...undoState(
              restored ? { past: state.history.past, future: [] } : state.history,
              state.agentEdits.filter((edit) => edit.messageId !== messageId),
            ),
          }));
          if (restored) {
            get().pruneSelection(restored);
            get().clearHighlights();
          }
        });
      },
      setLastError: (message) => set({ lastError: message }),
    };
  };
}
