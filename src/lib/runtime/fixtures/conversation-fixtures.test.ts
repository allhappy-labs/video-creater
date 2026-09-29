import { describe, expect, it } from "vitest";
import type {
  AppServerConversationHistory,
  CodexConversationEditCommandResult,
  CodexConversationEditProposal,
  PreparedPreviewFrameResult,
  ProjectActionWriteResult,
  ProjectAgentApplyResult,
  ProjectAgentSessionManifest,
  ProjectAgentUndoOutcome,
  VideoProject,
} from "@/lib/project";
import { generatedOutputTimelineActions } from "@/lib/generation/timeline-placement";
import { safeProjectMediaPath } from "@/lib/media/preview-source";
import { createSampleProject } from "@/lib/sample-project";
import { FixtureTransport } from "../adapters/fixture-transport";
import { classifyFixtureRisk, fixtureTurn, fixtureTurnKind, prepareFixtureProposal } from "./conversation-fixture-proposals";
import { conversationFixtureCopy, conversationFixtureOperations } from "./conversation-fixtures";
import { createFixtureProjectStore } from "./fixture-project-store";
import { generationFixtureOperations, generationFixturePolls } from "./generation-fixtures";
import { projectFixtureOperations } from "./project-fixtures";
import { taskFixtureOperations } from "./task-fixtures";

const projectDir = "/tmp/video-creater-editor-project";

function setup(options: { readonly turnDelayMs?: number } = {}) {
  const store = createFixtureProjectStore();
  const state = {
    get committed() {
      return store.current;
    },
  };
  const handlers = new Map([
    ...projectFixtureOperations(store),
    ...taskFixtureOperations(store, { seed: false }),
    ...generationFixtureOperations(store),
    ...conversationFixtureOperations({ store, ...options }),
  ]);
  async function request<Result>(operation: string, input: Record<string, unknown> = {}): Promise<Result> {
    const handler = handlers.get(operation);
    if (!handler) throw new Error(`no conversation fixture handler for ${operation}`);
    return structuredClone(await handler(structuredClone(input))) as Result;
  }
  async function open(project = createSampleProject()): Promise<VideoProject> {
    return (await request<{ project: VideoProject }>("save_split_project_to_folder", { projectDir, project })).project;
  }
  async function ask(prompt: string): Promise<CodexConversationEditCommandResult> {
    const project = state.committed ?? createSampleProject();
    return request("start_codex_conversation_edit_for_project", {
      projectDir,
      project,
      request: { prompt, focus: { mediaIds: [], timelineItemIds: [] }, createdAt: "2026-09-15T10:00:00.000Z" },
    });
  }
  async function apply(result: CodexConversationEditCommandResult, reviewApproved = false, actionIds = result.preparedProposal?.actionIds ?? []) {
    return request<ProjectAgentApplyResult>("apply_codex_conversation_proposal", { projectDir, proposal: result.proposal, actionIds, reviewApproved, sessionId: null });
  }
  return { state, request, open, ask, apply };
}

function clipCount(project: VideoProject): number {
  return project.timeline.tracks.reduce((count, track) => count + track.items.length, 0);
}

function internalIds(text: string | undefined): RegExpMatchArray | null {
  return (text ?? "").match(/media-\d|item-\d|caption-\d|track-|codex-action|agent-lab-shot|sample-generated/);
}

describe("conversation fixture operations", () => {
  it("routes prompts by keyword, case-insensitively", () => {
    expect(fixtureTurnKind("Tighten the pacing")).toBe("tighten");
    expect(fixtureTurnKind("please GENERATE three lab shots")).toBe("generate");
    expect(fixtureTurnKind("fail please")).toBe("fail");
    expect(fixtureTurnKind("Generate something that will FAIL")).toBe("fail");
    expect(fixtureTurnKind("Organize the current project media")).toBe("caption");
  });

  it("answers a start turn with a prepared proposal chosen by keyword, with deterministic action ids", async () => {
    const { open, ask } = setup();
    const opened = await open();

    const tighten = await ask("Tighten the pacing");
    expect(tighten.preparedProposal?.risk).toEqual({ level: "safe", reasons: [] });
    expect(tighten.preparedProposal?.actions.map((action) => action.type)).toEqual(["splitItems", "rippleDeleteRanges"]);
    expect(tighten.preparedProposal?.impact).toMatchObject({
      beforeDurationSeconds: 8,
      afterDurationSeconds: 6,
      affectedItemIds: ["sample-generated-clip", "sample-generated-clip-tail"],
      affectedRanges: [{ startSeconds: 5, endSeconds: 8 }],
      previewTimestamp: 5,
      summary: "Changes 1 item, adds 1 item; the timeline goes from 8.0s to 6.0s.",
    });
    expect(tighten.preparedProposal?.actionIds).toEqual((await ask("tighten again")).preparedProposal?.actionIds);
    expect(tighten.preparedProposal?.actionIds[0]).toMatch(/^codex-action-1-[0-9a-f]{8}$/);

    const generate = await ask("Generate three lab shots");
    expect(generate.preparedProposal?.risk).toEqual({
      level: "review",
      reasons: [
        { code: "changesGeneratedAssets", message: "Starts, changes, or places generated media." },
        { code: "changesJobs", message: "Starts or changes a background job or provider request." },
      ],
    });
    expect(generate.preparedProposal?.actions.map((action) => action.type)).toEqual([
      ...["recordGeneratedAsset", "recordJob", "recordGeneratedAsset", "recordJob", "recordGeneratedAsset", "recordJob"],
      "addItems",
    ]);
    // Like Rust `generation_record_actions`: `job-<assetId>` jobs whose start requests name their asset.
    const [, firstJob] = generate.preparedProposal?.actions ?? [];
    expect(firstJob).toMatchObject({
      type: "recordJob",
      job: { id: "job-agent-lab-shot-1", kind: "generate_media", status: "queued", startRequest: { input: { assetId: "agent-lab-shot-1", jobId: "job-agent-lab-shot-1", mockMode: false } } },
    });

    const failed = await ask("fail please");
    expect(failed.proposal).not.toBeNull();
    expect(failed.preparedProposal).toBeNull();
    expect(failed.proposalValidationIssues?.[0]?.message).toBe("The proposed cut ends after the end of the timeline.");

    const caption = await ask("Make the captions friendlier");
    expect(caption.preparedProposal?.risk.level).toBe("safe");
    expect(caption.preparedProposal?.actions).toEqual([{ type: "editCaptionText", itemId: "caption-1", text: "Original caption text." }]);

    for (const result of [tighten, generate, failed, caption]) expect(internalIds(result.proposal?.summary)).toBeNull();
    // The turn persisted its thread, which bumps the revision like a native write.
    expect(caption.project.codexThreadId).toBe("fixture-thread-1");
    expect(caption.project.contentRevision).toBeGreaterThan(opened.contentRevision ?? 0);
  });

  it("classifies risk like the backend allowlist: transitions are safe, deletions and unknown actions need review", () => {
    expect(classifyFixtureRisk([{ type: "removeTransition", trackId: "track-video", transitionId: "transition-1" }, { type: "editCaptionText", itemId: "caption-1", text: "Hi" }])).toEqual({ level: "safe", reasons: [] });
    expect(classifyFixtureRisk([{ type: "removeItems", itemIds: ["item-1"] }, { type: "assignMediaFolder", mediaId: "media-1", folderId: null }])).toEqual({
      level: "review",
      reasons: [
        { code: "deletesExistingItems", message: "Removes existing clips from the timeline." },
        { code: "unclassifiedAction", message: "Includes a change that needs your review." },
      ],
    });
  });

  it("reports only the trimmed edge of a reversed clip, like the backend impact", () => {
    const project = createSampleProject();
    const clip = project.timeline.tracks.flatMap((track) => track.items).find((item) => item.kind === "video_clip" && item.source.type === "media");
    if (!clip) throw new Error("the sample project has no media clip");
    const start = clip.startSeconds;
    clip.properties = { ...clip.properties, sourceIn: 0, sourceOut: 4, speed: 1, reverse: true };
    clip.durationSeconds = 4;
    const proposal = {
      summary: "Trim the start",
      edl: [],
      renderReview: null,
      projectActions: [{ type: "trimItems" as const, trims: [{ itemId: clip.id, startSeconds: start + 1, durationSeconds: 3, sourceIn: 0, sourceOut: 3 }] }],
    };
    expect(prepareFixtureProposal(project, proposal).prepared.impact.affectedRanges).toEqual([{ startSeconds: start, endSeconds: start + 1 }]);
  });

  it("falls back to a safe fade on a project without captions", () => {
    const project = createSampleProject();
    project.timeline.tracks = project.timeline.tracks.map((track) => (track.kind === "caption" ? { ...track, items: [] } : track));
    const turn = fixtureTurn(project, "Anything at all");
    expect(turn.prepared?.risk.level).toBe("safe");
    expect(turn.prepared?.actions).toEqual([{ type: "updateVisualClipFades", itemId: "item-1", fadeInSeconds: 0.5, fadeOutSeconds: 0 }]);
  });

  it("applies a safe proposal to the committed project and records an Undo entry", async () => {
    const { state, open, ask, apply, request } = setup();
    const opened = await open();
    const turn = await ask("Tighten the pacing");
    const result = await apply(turn);
    expect(clipCount(result.project)).toBe(clipCount(opened) + 1);
    expect(result.project.timeline.tracks.find((track) => track.id === "track-video")?.items.map((item) => [item.startSeconds, item.durationSeconds])).toEqual([[0, 4], [4, 1], [5, 1]]);
    expect(result.historyEntryId).toBe(`agent-edit-r${(result.project.contentRevision ?? 0).toString()}`);
    expect(result.actionIds).toEqual(turn.preparedProposal?.actionIds);
    expect(result.impact.afterDurationSeconds).toBe(6);
    expect(state.committed).toEqual(result.project);
    const sessions = await request<ProjectAgentSessionManifest>("load_agent_sessions_from_split_project_folder", { projectDir });
    expect(sessions.sessions[0]).toMatchObject({ proposalStatus: "applied", appliedActionIds: result.actionIds });
  });

  it("refuses a review-level bundle without approval, and stale action ids, leaving the project unchanged", async () => {
    const { state, open, ask, apply } = setup();
    await open();
    const turn = await ask("Generate three lab shots");
    const before = state.committed;
    await expect(apply(turn, false)).rejects.toBe(conversationFixtureCopy.reviewRequired);
    await expect(apply(turn, true, ["codex-action-1-00000000"])).rejects.toBe(conversationFixtureCopy.staleProposal);
    expect(state.committed).toBe(before);

    const approved = await apply(turn, true);
    expect(approved.risk.level).toBe("review");
    expect(approved.project.generatedAssets.map((asset) => asset.name)).toEqual(expect.arrayContaining(["Lab bench wide shot", "Beaker pour close-up", "Microscope focus pull"]));
    // Applying the same generations twice no longer fits the project.
    await expect(apply(turn, true)).rejects.toMatch(/^This edit no longer fits the project, so it was not applied: /);
  });

  it("undoes the latest edit to the snapshot, keeping job bookkeeping, and reports conflicts", async () => {
    const { open, ask, apply, request } = setup();
    await expect(request<ProjectAgentUndoOutcome>("undo_latest_codex_conversation_edit", { projectDir })).resolves.toEqual({ status: "unavailable", message: conversationFixtureCopy.nothingToUndo });
    const opened = await open();
    const applied = await apply(await ask("Tighten the pacing"));
    // A frame capture records a job; that bookkeeping neither blocks nor is rolled back by Undo.
    await request<PreparedPreviewFrameResult>("capture_canonical_preview_frame_in_split_project_folder", { projectDir, playheadSeconds: 5, jobId: "frame-1", updatedAt: "2026-09-15T10:00:02.000Z" });
    await expect(request<ProjectAgentUndoOutcome>("undo_latest_codex_conversation_edit", { projectDir, historyEntryId: "agent-edit-r999" })).resolves.toEqual({
      status: "unavailable",
      message: conversationFixtureCopy.entryNotInHistory,
    });
    const undone = await request<ProjectAgentUndoOutcome>("undo_latest_codex_conversation_edit", { projectDir, historyEntryId: applied.historyEntryId });
    if (undone.status !== "undone") throw new Error(`expected undone, got ${undone.status}`);
    expect(undone).toMatchObject({ entryId: applied.historyEntryId, actionCount: 2, remainingAgentHistory: 0 });
    expect(undone.project.timeline).toEqual(opened.timeline);
    expect(clipCount(undone.project)).toBe(clipCount(opened));
    expect(undone.project.jobs.map((job) => job.id)).toContain("frame-1");
    expect(undone.project.contentRevision).toBeGreaterThan(applied.project.contentRevision ?? 0);

    const second = await apply(await ask("Tighten the pacing"));
    const edit = await request<ProjectActionWriteResult>("apply_project_actions_to_split_project_folder", {
      projectDir,
      actions: [{ type: "updateVisualClipOpacity", itemId: "item-1", opacity: 0.4 }],
    });
    expect(edit.project.contentRevision).toBeGreaterThan(second.project.contentRevision ?? 0);
    await expect(request<ProjectAgentUndoOutcome>("undo_latest_codex_conversation_edit", { projectDir, historyEntryId: second.historyEntryId })).resolves.toEqual({
      status: "conflict",
      entryId: second.historyEntryId,
      message: conversationFixtureCopy.projectChanged,
    });
  });

  it("undoes a Generate & place bundle whose generations ran, completed and were placed, removing them", async () => {
    const { state, open, ask, apply, request } = setup();
    const opened = await open();
    const applied = await apply(await ask("Generate three lab shots"), true);
    const job = applied.project.jobs.find((candidate) => candidate.id === "job-agent-lab-shot-1");
    // The editor starts the approved generations; the first runs on the folder's job clock.
    const run = request<VideoProject>("run_generate_media_in_process", { startRequest: job?.startRequest, updatedAt: "2026-09-15T10:00:01.000Z" });
    for (let poll = 0; poll < generationFixturePolls; poll += 1) await request<VideoProject>("load_split_project_from_folder", { projectDir });
    const completed = await run;
    expect(completed.generatedAssets.find((asset) => asset.id === "agent-lab-shot-1")?.status).toBe("completed");
    expect(completed.jobs.find((candidate) => candidate.id === "job-agent-lab-shot-1")?.status).toBe("completed");
    // The editor places the output over the placeholder.
    const placement = generatedOutputTimelineActions(completed, "agent-lab-shot-1-mock-output");
    expect(placement?.actions.map((action) => action.type)).toEqual(["removeItems", "addItems"]);
    await request<ProjectActionWriteResult>("apply_project_actions_to_split_project_folder", { projectDir, actions: placement?.actions });

    const undone = await request<ProjectAgentUndoOutcome>("undo_latest_codex_conversation_edit", { projectDir, historyEntryId: applied.historyEntryId });

    if (undone.status !== "undone") throw new Error(`expected undone, got ${JSON.stringify(undone)}`);
    expect(undone.removedGeneratedAssetIds).toEqual(["agent-lab-shot-1", "agent-lab-shot-2", "agent-lab-shot-3"]);
    expect(undone.project.timeline).toEqual(opened.timeline);
    expect(undone.project.media).toEqual(opened.media);
    expect(undone.project.generatedAssets).toEqual(opened.generatedAssets);
    expect(undone.project.jobs.map((candidate) => candidate.id)).not.toContain("job-agent-lab-shot-1");
    // A run that was still waiting finds its asset gone and records nothing.
    const late = request<VideoProject>("run_generate_media_in_process", { startRequest: job?.startRequest, updatedAt: "2026-09-15T10:00:05.000Z" });
    await expect(late).rejects.toMatch(/job was not found/);
    expect(state.committed?.generatedAssets).toEqual(opened.generatedAssets);
  });

  it("still refuses to undo a generation bundle after the user moves its placeholder", async () => {
    const { open, ask, apply, request } = setup();
    await open();
    const applied = await apply(await ask("Generate three lab shots"), true);
    const placeholder = applied.project.timeline.tracks.flatMap((track) => track.items.map((item) => ({ track, item }))).find(({ item }) => item.properties.generatedTimelinePlaceholder === true);
    if (!placeholder) throw new Error("expected a placeholder clip");
    await request<ProjectActionWriteResult>("apply_project_actions_to_split_project_folder", {
      projectDir,
      actions: [{ type: "moveItems", moves: [{ itemId: placeholder.item.id, targetTrackId: placeholder.track.id, startSeconds: placeholder.item.startSeconds + 2 }] }],
    });

    await expect(request<ProjectAgentUndoOutcome>("undo_latest_codex_conversation_edit", { projectDir, historyEntryId: applied.historyEntryId })).resolves.toEqual({
      status: "conflict",
      entryId: applied.historyEntryId,
      message: conversationFixtureCopy.projectChanged,
    });
  });

  it("captures a result frame as a project PNG the fixture media route can serve, recording its job", async () => {
    const { state, open, request } = setup();
    await open();
    const captured = await request<PreparedPreviewFrameResult>("capture_canonical_preview_frame_in_split_project_folder", {
      projectDir,
      playheadSeconds: 1.5,
      jobId: "agent-result-frame-3-1500",
      updatedAt: "2026-09-15T10:00:02.000Z",
    });
    expect(captured.previewFrame).toBe("renders/agent-result-frame-3-1500/preview-qa/preview-frames/preview-0001.png");
    expect(captured.project.jobs).toContainEqual({ id: "agent-result-frame-3-1500", kind: "captureCanonicalPreviewFrame", status: "completed", updatedAt: "2026-09-15T10:00:02.000Z" });
    expect(state.committed).toEqual(captured.project);
    const path = safeProjectMediaPath(projectDir, captured.previewFrame);
    expect(path && new FixtureTransport(new Map()).mediaUrl(path)).toBe(
      "/__editor-fixture-media/tmp/video-creater-editor-project/renders/agent-result-frame-3-1500/preview-qa/preview-frames/preview-0001.png",
    );
    await expect(request("capture_canonical_preview_frame_in_split_project_folder", { projectDir, playheadSeconds: 8, jobId: "late", updatedAt: "" })).rejects.toMatch(/before the timeline end/);
  });

  it("keeps chats and turn history in an in-memory manifest", async () => {
    const { open, ask, request } = setup();
    const project = await open();
    await expect(request<ProjectAgentSessionManifest>("load_agent_sessions_from_split_project_folder", { projectDir })).resolves.toEqual({
      schemaVersion: 1,
      projectId: project.id,
      activeSessionId: null,
      sessions: [],
      deletedSessions: [],
    });
    const act = (action: Record<string, unknown>) => request<ProjectAgentSessionManifest>("apply_agent_session_action_to_split_project_folder", { projectDir, projectId: project.id, action });
    await act({ type: "create", id: "session-a", title: " Pacing ", threadId: null, timestamp: "2026-09-15T09:00:00.000Z" });
    await act({ type: "create", id: "session-b", title: "Captions", threadId: null, timestamp: "2026-09-15T09:30:00.000Z" });
    await act({ type: "rename", sessionId: "session-a", title: "Pacing pass", timestamp: "2026-09-15T09:31:00.000Z" });
    await act({ type: "select", sessionId: "session-a" });
    await ask("Tighten the pacing");
    const deleted = await act({ type: "delete", sessionId: "session-a", timestamp: "2026-09-15T09:40:00.000Z" });
    expect(deleted.activeSessionId).toBe("session-b");
    expect(deleted.deletedSessions.map((session) => [session.title, session.turns.length])).toEqual([["Pacing pass", 1]]);
    const restored = await act({ type: "restore", sessionId: "session-a", timestamp: "2026-09-15T09:41:00.000Z" });
    expect(restored.activeSessionId).toBe("session-a");
    await expect(act({ type: "select", sessionId: "missing" })).rejects.toBe("agent session was not found");
    await expect(act({ type: "create", id: "session-b", title: "Again", threadId: null, timestamp: "" })).rejects.toMatch(/already exists/);

    const history = await request<AppServerConversationHistory>("load_app_server_conversation_history_from_split_project_folder", { projectDir });
    expect(history.entries).toHaveLength(1);
    expect(history.entries[0]).toMatchObject({ prompt: "Tighten the pacing", threadId: "fixture-thread-session-a", hasProposal: true, sessionId: "session-a" });

    await act({ type: "select", sessionId: "session-b" });
    await ask("Add clean captions");
    const both = await request<AppServerConversationHistory>("load_app_server_conversation_history_from_split_project_folder", { projectDir });
    // Each chat runs on its own Codex thread, like the backend.
    expect(both.entries.map((entry) => [entry.prompt, entry.sessionId, entry.threadId])).toEqual([
      ["Tighten the pacing", "session-a", "fixture-thread-session-a"],
      ["Add clean captions", "session-b", "fixture-thread-session-b"],
    ]);
  });

  it("stops a working turn when the conversation is cancelled", async () => {
    const { open, ask, request } = setup({ turnDelayMs: 50 });
    await open();
    const turn = ask("Tighten the pacing");
    await expect(request("cancel_codex_conversation_edit_for_project", { projectDir })).resolves.toBe(true);
    await expect(turn).rejects.toBe(conversationFixtureCopy.interrupted);
    await expect(request("cancel_codex_conversation_edit_for_project", { projectDir })).resolves.toBe(false);
  });

  it("answers a turn on a sample that isn't saved yet from the sent project", async () => {
    const { request } = setup();
    const proposalProject = createSampleProject();
    const result = await request<CodexConversationEditCommandResult>("start_codex_conversation_edit_for_project", {
      project: proposalProject,
      request: { prompt: "Tighten the pacing", focus: { mediaIds: [], timelineItemIds: [] }, createdAt: "2026-09-15T10:00:00.000Z" },
    });
    expect(result.project).toEqual(proposalProject);
    expect((result.proposal as CodexConversationEditProposal).projectActions).toHaveLength(2);
  });
});
