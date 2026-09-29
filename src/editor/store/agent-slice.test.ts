import { beforeEach, describe, expect, it, vi } from "vitest";
import { conversationMentionTargets } from "@/lib/agent/mentions";
import { applyProjectActionsLocally } from "@/lib/agent/project-merge";
import { indicatorState } from "@/lib/jobs/task-indicator";
import type {
  CodexConversationEditCommandResult,
  CodexPreparedProposal,
  CodexProposalRiskLevel,
  ProjectAction,
  ProjectAgentApplyResult,
  ProjectAgentUndoOutcome,
  VideoProject,
} from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendOperationError, BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import type { AgentAssistantMessage } from "./agent-slice";
import { createEditorStore, type EditorStore } from "./editor-store";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

type Handler = (input: Record<string, unknown>) => unknown;

const projectDir = "/projects/demo";
const startCommand = "start_codex_conversation_edit_for_project";
const applyCommand = "apply_codex_conversation_proposal";
const undoCommand = "undo_latest_codex_conversation_edit";
const staleCopy = "The project changed after this edit was prepared, so it was not applied. Ask for the edit again.";
const report = { manifestPath: `${projectDir}/video-creater.project.json`, writtenFiles: [], removedFiles: [] };

function splitProject(): VideoProject {
  return { ...fixtureProject(), schemaVersion: 2, contentRevision: 3 };
}

function videoItemId(project: VideoProject): string {
  return fixtureItem(project, "video").id;
}

function opacityAction(project: VideoProject, opacity = 0.5): ProjectAction {
  return { type: "updateVisualClipOpacity", itemId: videoItemId(project), opacity };
}

function preparedBundle(project: VideoProject, level: CodexProposalRiskLevel): CodexPreparedProposal {
  return {
    actions: [opacityAction(project)],
    actionIds: ["codex-action-1-0a1b2c3d"],
    risk: { level, reasons: level === "review" ? [{ code: "generation", message: "Generates new media." }] : [] },
    impact: {
      summary: "Changes 1 item.",
      beforeDurationSeconds: 130,
      afterDurationSeconds: 45,
      affectedItemIds: [videoItemId(project)],
      affectedRanges: [{ startSeconds: 0, endSeconds: 5 }],
      previewTimestamp: 1,
    },
  };
}

function turnResult(project: VideoProject, level: CodexProposalRiskLevel): CodexConversationEditCommandResult {
  const bundle = preparedBundle(project, level);
  return {
    project,
    threadId: "thread-1",
    threadResponse: {},
    turnResponse: {},
    proposal: { summary: "Faded the opening clip.", edl: [], projectActions: bundle.actions, renderReview: null },
    preparedProposal: bundle,
    proposalValidationIssues: null,
  };
}

function withRevision(project: VideoProject, revision: number): VideoProject {
  return { ...project, contentRevision: revision };
}

/** A backend holding the committed project; unhandled commands are "unavailable", like a browser. */
function setup(handlers: Record<string, Handler> = {}, options: { level?: CodexProposalRiskLevel; dir?: string } = {}) {
  const initial = splitProject();
  const backend = { project: initial, beforeAgent: initial };
  const bump = (project: VideoProject) => withRevision(project, (backend.project.contentRevision ?? 0) + 1);
  const all: Record<string, Handler> = {
    [startCommand]: () => turnResult(backend.project, options.level ?? "safe"),
    [applyCommand]: (input) => {
      backend.beforeAgent = backend.project;
      const proposal = input.proposal as { projectActions: ProjectAction[] };
      backend.project = bump(applyProjectActionsLocally(backend.project, proposal.projectActions));
      const bundle = preparedBundle(initial, options.level ?? "safe");
      return {
        project: backend.project,
        report,
        historyEntryId: "agent-edit-1",
        actionIds: bundle.actionIds,
        risk: bundle.risk,
        impact: bundle.impact,
        warnings: ["The conversation session was not found, so its status was not updated."],
      } satisfies ProjectAgentApplyResult;
    },
    [undoCommand]: () => {
      const removedGeneratedAssetIds = backend.project.generatedAssets.flatMap((asset) => (backend.beforeAgent.generatedAssets.some((kept) => kept.id === asset.id) ? [] : [asset.id]));
      backend.project = bump(backend.beforeAgent);
      return {
        status: "undone",
        project: backend.project,
        report,
        entryId: "agent-edit-1",
        actionCount: 1,
        remainingAgentHistory: 0,
        warnings: [],
        removedGeneratedAssetIds,
      } satisfies ProjectAgentUndoOutcome;
    },
    apply_project_actions_to_split_project_folder: (input) => {
      backend.project = bump(applyProjectActionsLocally(backend.project, input.actions as ProjectAction[]));
      return { project: backend.project };
    },
    save_split_project_to_folder: (input) => {
      backend.project = bump(input.project as VideoProject);
      return { project: backend.project };
    },
    cancel_codex_conversation_edit_for_project: () => true,
    ...handlers,
  };
  vi.mocked(backendRequest).mockImplementation(async (command: string, input?: Record<string, unknown>) => {
    const handler = all[command];
    if (!handler) throw new BackendUnavailableError();
    return handler(input ?? {});
  });
  const store = createEditorStore({ projectDir: options.dir ?? projectDir, project: initial });
  const projectWrites: VideoProject[] = [];
  store.subscribe((state, previous) => {
    if (state.project !== previous.project) projectWrites.push(state.project);
  });
  return { backend, initial, store, projectWrites };
}

/**
 * What an agent "Generate & place" bundle records per generation (Rust `generation_record_actions`): the
 * queued asset and a queued `job-<assetId>` job whose start request names it.
 */
function generationActions(project: VideoProject, count: number): ProjectAction[] {
  const createdAt = "2026-09-15T10:00:00.000Z";
  return Array.from({ length: count }, (_, index): ProjectAction[] => {
    const assetId = `agent-shot-${(index + 1).toString()}`;
    const jobId = `job-${assetId}`;
    const startRequest = {
      workflowId: `generate-media-${assetId}`,
      workflowType: "generateMedia",
      taskQueue: "video-creater",
      input: { projectId: project.id, projectDir, assetId, jobId, mockMode: false },
      searchAttributes: {},
      activityTypes: ["runProvider"],
      idReusePolicy: "rejectDuplicate",
    };
    return [
      {
        type: "recordGeneratedAsset",
        asset: {
          id: assetId,
          kind: "video",
          status: "queued",
          name: `Lab shot ${(index + 1).toString()}`,
          placementIntent: "library",
          prompt: "A lab bench",
          model: { provider: "replicate", id: "bytedance/seedance-1-pro-fast" },
          references: { mediaIds: [], firstFrameMediaId: null, lastFrameMediaId: null },
          settings: { width: 1280, height: 720, durationSeconds: 4, fps: 24, aspectRatio: "16:9" },
          outputs: [],
          createdAt,
          parentAssetId: null,
          retryOfAssetId: null,
        },
      },
      { type: "recordJob", job: { id: jobId, kind: "generate_media", status: "queued", updatedAt: createdAt, startRequest } },
    ];
  }).flat();
}

/** A review-level turn whose bundle records `count` generations. */
function generationTurn(backend: { project: VideoProject }, count: number): CodexConversationEditCommandResult {
  const result = turnResult(backend.project, "review");
  const actions = generationActions(backend.project, count);
  const prepared = preparedBundle(backend.project, "review");
  return {
    ...result,
    proposal: result.proposal && { ...result.proposal, projectActions: actions },
    preparedProposal: { ...prepared, actions, actionIds: actions.map((_, index) => `codex-action-${(index + 1).toString()}-0a1b2c3d`) },
  };
}

/** Lets settled backend promises and the store's queued writes run. */
async function settle(): Promise<void> {
  for (let tick = 0; tick < 20; tick += 1) await Promise.resolve();
}

function calls(command: string) {
  return vi.mocked(backendRequest).mock.calls.filter(([name]) => name === command);
}

function replies(store: EditorStore): AgentAssistantMessage[] {
  return store.getState().agentMessages.filter((message): message is AgentAssistantMessage => message.role === "assistant");
}

function latestReply(store: EditorStore): AgentAssistantMessage {
  const reply = replies(store).pop();
  if (!reply) throw new Error("expected an assistant reply");
  return reply;
}

function opacityOf(project: VideoProject): unknown {
  return fixtureItem(project, "video").properties.opacity;
}

describe("agent slice", () => {
  beforeEach(() => {
    vi.mocked(backendRequest).mockReset();
    window.localStorage.clear();
  });

  describe("submit and apply", () => {
    it("auto-applies a safe proposal and replaces the project once, without a local undo step", async () => {
      const { backend, store, projectWrites } = setup();

      await expect(store.getState().submitAgentPrompt("Fade the opening clip")).resolves.toBe(true);

      expect(calls(applyCommand)).toHaveLength(1);
      expect(calls(applyCommand)[0]?.[1]).toMatchObject({ projectDir, actionIds: ["codex-action-1-0a1b2c3d"], reviewApproved: false, sessionId: null });
      expect(projectWrites).toEqual([backend.project]);
      expect(store.getState().history.past).toEqual([]);
      expect(store.getState()).toMatchObject({ lastMutationSource: "agent", agentDraft: "", agentConversation: { status: "applied" } });
      const reply = latestReply(store);
      expect(reply).toMatchObject({ status: "applied", text: "Faded the opening clip.", failure: null });
      expect(reply.card?.result).toMatchObject({
        historyEntryId: "agent-edit-1",
        revision: 4,
        warnings: ["The conversation session was not found, so its status was not updated."],
      });
      expect(reply.card?.facts.map((fact) => fact.kind)).toContain("duration");
      expect(store.getState().agentTurn).toMatchObject({ status: "completed", label: "AI edit: “Fade the opening clip”" });
    });

    it("leaves the project unchanged while a review proposal waits, then applies it on approve", async () => {
      const { backend, initial, store, projectWrites } = setup({}, { level: "review" });

      await store.getState().submitAgentPrompt("Generate three lab shots");

      const review = latestReply(store);
      expect(review.status).toBe("awaitingReview");
      expect(review.card?.facts.map((fact) => fact.label)).toContain("Replaces nothing");
      expect(calls(applyCommand)).toHaveLength(0);
      expect(store.getState().project).toBe(initial);
      expect(store.getState().agentConversation.status).toBe("awaitingReview");

      await expect(store.getState().approveAgentProposal(review.id)).resolves.toBe(true);

      expect(calls(applyCommand)[0]?.[1]).toMatchObject({ reviewApproved: true });
      expect(projectWrites).toEqual([backend.project]);
      expect(latestReply(store)).toMatchObject({ status: "applied", card: { approved: true } });
    });

    it("starts the generations a bundle records once Generate & place approves it, one after another", async () => {
      const runs: (() => void)[] = [];
      const { backend, store } = setup({
        [startCommand]: () => generationTurn(backend, 2),
        run_generate_media_in_process: () => new Promise((resolve) => runs.push(() => resolve(backend.project))),
        load_split_project_from_folder: () => backend.project,
      });
      await store.getState().submitAgentPrompt("Generate two lab shots");
      expect(latestReply(store).status).toBe("awaitingReview");
      expect(calls("run_generate_media_in_process")).toHaveLength(0);

      await expect(store.getState().approveAgentProposal(latestReply(store).id)).resolves.toBe(true);

      await vi.waitFor(() => expect(calls("run_generate_media_in_process")).toHaveLength(1));
      expect(calls("run_generate_media_in_process")[0]?.[1]).toMatchObject({ startRequest: { input: { assetId: "agent-shot-1", jobId: "job-agent-shot-1" } } });
      runs[0]?.();
      await vi.waitFor(() => expect(calls("run_generate_media_in_process")).toHaveLength(2));
      expect(calls("run_generate_media_in_process")[1]?.[1]).toMatchObject({ startRequest: { input: { assetId: "agent-shot-2" } } });
      expect(latestReply(store).status).toBe("applied");
      store.getState().stopPolling();
    });

    it("keeps the applied edit and shows the plain reason when a generation fails to start", async () => {
      const reason = "Add a Replicate API key in Settings to generate media.";
      const { backend, store } = setup({
        [startCommand]: () => generationTurn(backend, 1),
        run_generate_media_in_process: () => Promise.reject(reason),
        load_split_project_from_folder: () => backend.project,
      });
      await store.getState().submitAgentPrompt("Generate a lab shot");

      await expect(store.getState().approveAgentProposal(latestReply(store).id)).resolves.toBe(true);

      await vi.waitFor(() => expect(store.getState().lastError).toBe(reason));
      expect(latestReply(store)).toMatchObject({ status: "applied", card: { undo: { available: true } } });
      expect(store.getState().project.generatedAssets.map((asset) => asset.id)).toContain("agent-shot-1");
      expect(calls(undoCommand)).toHaveLength(0);
      store.getState().stopPolling();
    });

    it("doesn't report a turn waiting for review as a completed background task", async () => {
      const { store } = setup({}, { level: "review" });

      const pending = store.getState().submitAgentPrompt("Generate three lab shots");
      expect(store.getState().tasks.find((task) => task.kind === "agent")).toMatchObject({ status: "running" });
      await pending;

      expect(latestReply(store).status).toBe("awaitingReview");
      expect(store.getState().agentTurn).toBeNull();
      expect(store.getState().tasks.filter((task) => task.kind === "agent")).toEqual([]);
      expect(indicatorState(store.getState().tasks, Date.now())).toEqual({ visible: false });

      await store.getState().approveAgentProposal(latestReply(store).id);
      expect(store.getState().agentTurn).toMatchObject({ status: "completed", label: "AI edit: “Generate three lab shots”" });
      expect(indicatorState(store.getState().tasks, Date.now())).toMatchObject({ visible: true, label: "Agent edit complete" });
    });

    it("sends a safe proposal to review when auto-apply is off", async () => {
      const { initial, store } = setup();
      store.getState().setAutoApplySafe(false);

      await store.getState().submitAgentPrompt("Fade the opening clip");

      expect(latestReply(store).status).toBe("awaitingReview");
      expect(calls(applyCommand)).toHaveLength(0);
      expect(store.getState().project).toBe(initial);
    });

    it("rejects a review without a backend call or project change", async () => {
      const { initial, store } = setup({}, { level: "review" });
      await store.getState().submitAgentPrompt("Generate three lab shots");
      const callCount = vi.mocked(backendRequest).mock.calls.length;

      store.getState().rejectAgentProposal(latestReply(store).id);

      expect(vi.mocked(backendRequest).mock.calls).toHaveLength(callCount);
      expect(store.getState().project).toBe(initial);
      expect(latestReply(store).status).toBe("dismissed");
      expect(store.getState().agentConversation).toEqual({ status: "idle", note: "Dismissed" });
    });

    it("dismisses an unanswered review when the user sends a new message", async () => {
      const { store } = setup({}, { level: "review" });
      await store.getState().submitAgentPrompt("Generate three lab shots");
      const review = latestReply(store);

      await store.getState().submitAgentPrompt("Generate two instead");

      expect(replies(store).find((reply) => reply.id === review.id)?.status).toBe("dismissed");
      expect(latestReply(store).status).toBe("awaitingReview");
    });

    it("blocks the send and explains when a mention matches nothing", async () => {
      const { store } = setup();
      store.getState().setAgentDraft("Tighten @missing.mp4");

      await expect(store.getState().submitAgentPrompt("Tighten @missing.mp4")).resolves.toBe(false);

      expect(calls(startCommand)).toHaveLength(0);
      expect(store.getState().agentMessages).toEqual([]);
      expect(store.getState().agentDraft).toBe("Tighten @missing.mp4");
      expect(store.getState().agentComposerError).toBe("Nothing in this project is named @missing.mp4. Pick a name from the @ list.");
      store.getState().setAgentDraft("Tighten the pacing");
      expect(store.getState().agentComposerError).toBeNull();
    });

    it("sends mentions and the selection chip as focus, and a removed chip is left out once", async () => {
      const { initial, store } = setup();
      const media = conversationMentionTargets(initial).find((target) => target.kind === "media");
      if (!media) throw new Error("fixture media target");
      const itemId = videoItemId(initial);
      store.getState().selectItems([itemId]);
      expect(store.getState().agentContextChip).toEqual({
        itemIds: [itemId],
        label: conversationMentionTargets(initial).find((target) => target.id === itemId)?.name,
      });

      await store.getState().submitAgentPrompt(`Use @"${media.name}" here`);
      expect(calls(startCommand)[0]?.[1]).toMatchObject({
        projectDir,
        request: { prompt: `Use @"${media.name}" here`, focus: { mediaIds: [media.id], timelineItemIds: [itemId] } },
      });

      store.getState().removeAgentContextChip();
      expect(store.getState().agentContextChip).toBeNull();
      await store.getState().submitAgentPrompt("Fade it");
      expect(calls(startCommand)[1]?.[1]).toMatchObject({ request: { focus: { mediaIds: [], timelineItemIds: [] } } });
      expect(store.getState().agentContextChip?.itemIds).toEqual([itemId]);
    });

    it("keeps the draft and fails the turn when the edit can't be validated", async () => {
      const { initial, store } = setup({
        [startCommand]: () => ({
          ...turnResult(initial, "safe"),
          preparedProposal: null,
          proposalValidationIssues: [{ path: "projectActions[0]", message: "The clip is locked", fix: "Unlock the track." }],
        }),
      });

      await store.getState().submitAgentPrompt("Fade the locked clip");

      expect(latestReply(store)).toMatchObject({ status: "failed", card: null, failure: { kind: "validationBlocked", message: "The clip is locked", retryable: false } });
      expect(store.getState().agentDraft).toBe("Fade the locked clip");
      expect(store.getState().agentConversation).toMatchObject({ status: "failed", failure: { kind: "validationBlocked" } });
      expect(store.getState().agentTurn).toMatchObject({ status: "failed", failureReason: "The clip is locked" });
    });

    it("names a reached usage limit instead of repeating the backend's words", async () => {
      const { store } = setup({
        [startCommand]: () => {
          throw new BackendOperationError(startCommand, "the agent's usage limit is reached; resets at 18:00");
        },
      });

      await store.getState().submitAgentPrompt("Tighten the pacing");

      expect(latestReply(store)).toMatchObject({
        status: "failed",
        failure: {
          kind: "agentUnavailable",
          message: "The AI agent's usage limit is reached. Try again later.",
          retryable: false,
        },
      });
    });

    it("sends an unauthenticated agent to Agent settings", async () => {
      const { store } = setup({
        [startCommand]: () => {
          throw new BackendOperationError(startCommand, "Not logged in · Please run /login");
        },
      });

      await store.getState().submitAgentPrompt("Tighten the pacing");

      expect(latestReply(store)).toMatchObject({
        status: "failed",
        failure: {
          kind: "agentUnavailable",
          message: "The AI agent isn't signed in. Open Agent settings to fix it.",
          retryable: false,
        },
      });
    });

    it("keeps an unrecognized turn failure in the backend's own words", async () => {
      const { store } = setup({
        [startCommand]: () => {
          throw new BackendOperationError(startCommand, "the timeline is locked by another edit");
        },
      });

      await store.getState().submitAgentPrompt("Tighten the pacing");

      expect(latestReply(store)).toMatchObject({
        status: "failed",
        failure: {
          kind: "turnFailed",
          message: "the timeline is locked by another edit",
          retryable: true,
        },
      });
    });

    it("surfaces the stale re-ask copy when the project changed before apply, without a retry", async () => {
      const { initial, store, projectWrites } = setup({
        [applyCommand]: () => {
          throw new BackendOperationError(applyCommand, staleCopy);
        },
      });

      await expect(store.getState().submitAgentPrompt("Fade the opening clip")).resolves.toBe(false);

      const reply = latestReply(store);
      expect(reply).toMatchObject({ status: "failed", failure: { kind: "applyFailed", message: staleCopy, retryable: false } });
      expect(store.getState().project).toBe(initial);
      expect(projectWrites).toEqual([]);
      expect(store.getState()).toMatchObject({ lastMutationSource: "user", agentEdits: [] });
      await expect(store.getState().retryAgentTurn(reply.id)).resolves.toBe(false);
    });

    it("retries a transient apply failure with the retry event", async () => {
      let failures = 1;
      const { store } = setup();
      const apply = vi.mocked(backendRequest).getMockImplementation();
      vi.mocked(backendRequest).mockImplementation(async (command, input) => {
        if (command === applyCommand && failures-- > 0) throw new BackendOperationError(applyCommand, "disk full");
        return apply?.(command, input);
      });

      await store.getState().submitAgentPrompt("Fade the opening clip");
      const reply = latestReply(store);
      expect(reply.failure).toEqual({ kind: "applyFailed", message: "disk full", retryable: true });

      await expect(store.getState().retryAgentTurn(reply.id)).resolves.toBe(true);
      expect(latestReply(store).status).toBe("applied");
      expect(store.getState().agentConversation.status).toBe("applied");
    });
  });

  describe("undo", () => {
    it("undoes a started generation bundle quietly: removed runs report nothing and queued ones never start", async () => {
      const pending: { fail: (() => void) | null } = { fail: null };
      const { backend, initial, store } = setup({
        [startCommand]: () => generationTurn(backend, 2),
        // Undo cancels the run; the native runner then fails to write to the asset Undo removed.
        run_generate_media_in_process: () =>
          new Promise((_, reject) => {
            pending.fail = () => reject("generated asset reference is missing: agent-shot-1");
          }),
        load_split_project_from_folder: () => backend.project,
      });
      await store.getState().submitAgentPrompt("Generate two lab shots");
      await store.getState().approveAgentProposal(latestReply(store).id);
      await vi.waitFor(() => expect(calls("run_generate_media_in_process")).toHaveLength(1));

      await expect(store.getState().undoAgentEdit(latestReply(store).id)).resolves.toBe(true);

      expect(latestReply(store).status).toBe("undone");
      expect(store.getState().project.generatedAssets).toEqual(initial.generatedAssets);
      expect(store.getState().project.jobs.map((job) => job.id)).not.toContain("job-agent-shot-1");
      pending.fail?.();
      await vi.waitFor(() => expect(calls("load_split_project_from_folder").length).toBeGreaterThan(0));
      await settle();
      expect(store.getState().lastError).toBeNull();
      expect(calls("run_generate_media_in_process")).toHaveLength(1);
      expect(store.getState().project.generatedAssets).toEqual(initial.generatedAssets);
      store.getState().stopPolling();
    });

    it("routes global undo after an agent apply to the agent batch", async () => {
      const { backend, initial, store } = setup();
      await store.getState().submitAgentPrompt("Fade the opening clip");
      expect(store.getState().canUndo()).toBe(true);

      await store.getState().undo();

      expect(calls(undoCommand)[0]?.[1]).toEqual({ projectDir, historyEntryId: "agent-edit-1" });
      expect(calls("save_split_project_to_folder")).toHaveLength(0);
      expect(store.getState().project).toBe(backend.project);
      expect(opacityOf(store.getState().project)).toEqual(opacityOf(initial));
      expect(latestReply(store)).toMatchObject({ status: "undone", card: { undo: { available: false, reason: null } } });
      expect(store.getState()).toMatchObject({ lastMutationSource: "user", agentEdits: [], agentConversation: { status: "undone" } });
      expect(store.getState().canUndo()).toBe(false);
    });

    it("undoes a later local edit first, then the agent batch", async () => {
      const { initial, store } = setup();
      await store.getState().submitAgentPrompt("Fade the opening clip");
      await store.getState().applyActions([{ type: "updateVisualClipOpacity", itemId: videoItemId(initial), opacity: 0.2 }]);
      expect(store.getState().lastMutationSource).toBe("user");

      await store.getState().undo();
      expect(calls(undoCommand)).toHaveLength(0);
      expect(opacityOf(store.getState().project)).toBe(0.5);
      expect(store.getState().lastMutationSource).toBe("agent");

      await store.getState().undo();
      expect(calls(undoCommand)).toHaveLength(1);
      expect(opacityOf(store.getState().project)).toEqual(opacityOf(initial));
    });

    it("still undoes the agent batch after switching to another chat", async () => {
      const chat = (id: string) => ({ id, title: id, threadId: null, createdAt: "2026-09-15T09:00:00.000Z", updatedAt: "2026-09-15T09:00:00.000Z", turns: [], proposalStatus: "none", appliedActionIds: [] });
      const manifest = (activeSessionId: string) => ({ schemaVersion: 1, projectId: "p", activeSessionId, sessions: [chat("a"), chat("b")], deletedSessions: [] });
      const { initial, store } = setup({
        load_agent_sessions_from_split_project_folder: () => manifest("a"),
        load_app_server_conversation_history_from_split_project_folder: () => ({ schemaVersion: 1, entries: [] }),
        apply_agent_session_action_to_split_project_folder: (input) => manifest((input.action as { sessionId: string }).sessionId),
      });
      await store.getState().loadAgentSessions();
      await store.getState().submitAgentPrompt("Fade the opening clip");
      const reply = latestReply(store);
      await store.getState().selectAgentSession("b");
      expect(store.getState().agentMessages).toEqual([]);

      await store.getState().undo();

      expect(calls(undoCommand)).toHaveLength(1);
      expect(opacityOf(store.getState().project)).toEqual(opacityOf(initial));
      await store.getState().selectAgentSession("a");
      expect(replies(store).find((message) => message.id === reply.id)?.status).toBe("undone");
    });

    it("disables Undo with the conflict reason and moves global undo past the batch", async () => {
      const conflict = "A newer edit changed the project, so this edit can't be undone.";
      const { backend, store } = setup({ [undoCommand]: () => ({ status: "conflict", entryId: "agent-edit-1", message: conflict }) });
      await store.getState().submitAgentPrompt("Fade the opening clip");
      const reply = latestReply(store);

      await expect(store.getState().undoAgentEdit(reply.id)).resolves.toBe(false);

      expect(store.getState().project).toBe(backend.project);
      expect(latestReply(store)).toMatchObject({ status: "applied", card: { undo: { available: false, pending: false, reason: conflict } } });
      expect(store.getState()).toMatchObject({ lastMutationSource: "user", agentEdits: [] });
      expect(store.getState().agentConversation).toMatchObject({ status: "applied", undo: { available: false } });
      await expect(store.getState().undoAgentEdit(reply.id)).resolves.toBe(false);
      expect(calls(undoCommand)).toHaveLength(1);
    });

    it("shows an unavailable history message as the disabled reason", async () => {
      const unavailable = "There is no agent edit to undo.";
      const { store } = setup({ [undoCommand]: () => ({ status: "unavailable", message: unavailable }) });
      await store.getState().submitAgentPrompt("Fade the opening clip");

      await store.getState().undoAgentEdit(latestReply(store).id);

      expect(latestReply(store).card?.undo).toEqual({ available: false, pending: false, reason: unavailable, error: null });
    });

    it("keeps Undo available after a request error", async () => {
      const { store } = setup({
        [undoCommand]: () => {
          throw new BackendOperationError(undoCommand, "project lease busy");
        },
      });
      await store.getState().submitAgentPrompt("Fade the opening clip");

      await store.getState().undoAgentEdit(latestReply(store).id);

      expect(latestReply(store).card?.undo).toEqual({ available: true, pending: false, reason: null, error: "project lease busy" });
      expect(store.getState().lastMutationSource).toBe("agent");
    });
  });

  describe("cancel", () => {
    it("turns the interrupted start into a Stopped turn, through the background task cancel", async () => {
      const start: { reject?: (error: unknown) => void } = {};
      const { store } = setup({ [startCommand]: () => new Promise((_resolve, reject) => (start.reject = reject)) });
      const unregister = store.getState().startAgent();

      const sent = store.getState().submitAgentPrompt("Tighten the pacing");
      const turnId = store.getState().agentTurn?.id ?? "";
      expect(store.getState().agentTurn?.status).toBe("running");

      await expect(store.getState().cancelTask(turnId)).resolves.toBe(true);
      expect(calls("cancel_codex_conversation_edit_for_project")[0]?.[1]).toEqual({ projectDir });
      await vi.waitFor(() => expect(start.reject).toBeDefined());
      start.reject?.(new BackendOperationError(startCommand, "app-server turn was interrupted"));

      await expect(sent).resolves.toBe(false);
      expect(store.getState().agentConversation).toEqual({ status: "idle", note: "Stopped" });
      expect(replies(store)).toEqual([]);
      expect(store.getState().agentTurn?.status).toBe("cancelled");
      unregister();
    });

    it("ignores cancel once applying has started", async () => {
      const apply: { resolve?: () => void } = {};
      const { store } = setup();
      const handlers = vi.mocked(backendRequest).getMockImplementation();
      vi.mocked(backendRequest).mockImplementation(async (command, input) => {
        if (command === applyCommand) {
          const applied = await handlers?.(command, input);
          return new Promise((resolve) => (apply.resolve = () => resolve(applied)));
        }
        return handlers?.(command, input);
      });

      const sent = store.getState().submitAgentPrompt("Fade the opening clip");
      await vi.waitFor(() => expect(apply.resolve).toBeDefined());
      await expect(store.getState().cancelAgentTurn()).resolves.toBe(false);
      apply.resolve?.();

      await expect(sent).resolves.toBe(true);
      expect(calls("cancel_codex_conversation_edit_for_project")).toHaveLength(0);
      expect(latestReply(store).status).toBe("applied");
    });
  });

  describe("composer, handoffs, and preference", () => {
    it("persists the auto-apply preference, on by default", () => {
      const { store } = setup();
      expect(store.getState().autoApplySafe).toBe(true);

      store.getState().setAutoApplySafe(false);

      expect(window.localStorage.getItem("video-creater.editor.v2.agent.autoApplySafe")).toBe("false");
      expect(setup().store.getState().autoApplySafe).toBe(false);
    });

    it("fills the draft and chip from a handoff without sending", () => {
      const { initial, store } = setup();
      const itemId = videoItemId(initial);
      store.getState().setActiveTab("media");
      store.getState().openSheet("media");

      store.getState().requestAgent({ itemIds: [itemId], prompt: "Organize the current project media" });
      expect(store.getState()).toMatchObject({ activeTab: "ai", openSheetId: "ai", pendingAgentRequest: { itemIds: [itemId] } });

      expect(store.getState().consumePendingAgentRequest()).toBe(true);
      expect(store.getState()).toMatchObject({ pendingAgentRequest: null, agentDraft: "Organize the current project media" });
      expect(store.getState().agentContextChip?.itemIds).toEqual([itemId]);
      expect(store.getState().consumePendingAgentRequest()).toBe(false);
      expect(calls(startCommand)).toHaveLength(0);
    });

    it("sends a handoff's drafted prompt only after confirmation", async () => {
      const { store } = setup({}, { level: "review" });
      store.getState().requestAgent({ itemIds: [], prompt: "Organize the current project media", confirmSend: true });
      store.getState().consumePendingAgentRequest();
      expect(store.getState()).toMatchObject({ agentConfirmSend: true, agentDraft: "Organize the current project media" });
      expect(calls(startCommand)).toHaveLength(0);

      store.getState().cancelAgentSend();
      expect(store.getState()).toMatchObject({ agentConfirmSend: false, agentDraft: "Organize the current project media" });

      store.getState().requestAgent({ itemIds: [], prompt: "Organize the current project media", confirmSend: true });
      store.getState().consumePendingAgentRequest();
      await expect(store.getState().confirmAgentSend()).resolves.toBe(true);
      expect(calls(startCommand)[0]?.[1]).toMatchObject({ request: { prompt: "Organize the current project media" } });
      expect(store.getState()).toMatchObject({ agentConfirmSend: false, agentDraft: "" });

      // A later handoff without confirmation doesn't inherit it.
      store.getState().requestAgent({ itemIds: [], prompt: "Organize", confirmSend: true });
      store.getState().consumePendingAgentRequest();
      store.getState().requestAgent({ itemIds: [videoItemId(store.getState().project)] });
      store.getState().consumePendingAgentRequest();
      expect(store.getState().agentConfirmSend).toBe(false);
    });
  });
});
