import type {
  CodexConversationEditCommandResult,
  CodexConversationEditProposal,
  CodexConversationEditRequest,
  PreparedPreviewFrameResult,
  ProjectAgentApplyResult,
  ProjectAgentSessionAction,
  ProjectAgentUndoOutcome,
  ProjectJobSummary,
  ProjectRenderReport,
  VideoProject,
} from "../../project";
import { buildSampleRenderReport } from "../../render";
import type { FixtureOperationHandler } from "../adapters/fixture-transport";
import { canonicalJson, FixtureProposalError, fixtureTurn, prepareFixtureProposal } from "./conversation-fixture-proposals";
import {
  applySessionAction,
  conversationHistory,
  createFixtureChats,
  markSessionTurn,
  recordTurn,
  sessionManifest,
  turnThreadId,
} from "./conversation-fixture-sessions";
import { addedRecords, restoredProject, undoContent, type UndoEntryRecords } from "./conversation-fixture-undo";
import { fixtureWriteReport as writeReport, type FixtureProjectStore } from "./fixture-project-store";

/**
 * DEV-only AI conversation handlers (`conversationFixture: true` on the fixture marker). They read
 * and write the shared fixture project store, as the desktop commands read and write the project
 * folder:
 *
 * - A turn answers by keyword (`fixtureTurn`): "tighten" is a safe split and ripple delete,
 *   "generate" a review-level bundle of three generations and a placeholder clip, "fail" a proposal
 *   that doesn't validate, and anything else a safe caption fix.
 * - Apply re-prepares the proposal against the committed project, refuses changed action ids and
 *   unapproved review bundles with the backend's copy, and records an Undo entry with the project
 *   before the edit. Every write bumps the content revision, like a native save.
 * - Undo restores that project with the current job bookkeeping (minus what the edit added), or
 *   reports a conflict when the content changed since (`conversation-fixture-undo.ts`): job
 *   bookkeeping and the background progress of the generations the edit recorded aside. Undo removes
 *   those generations; a generation run still waiting finds its asset gone and records nothing.
 * - Frame capture records its job and returns a PNG path the e2e fixture media route serves.
 * - Chats and turn history live in memory. Saves, reloads and editor action writes are the project
 *   fixture's, over the same store, so Undo sees edits made in the editor.
 */

/** Backend copy (src-tauri/src/codex/conversation/apply.rs and project/split/agent_batch.rs). */
export const conversationFixtureCopy = {
  reviewRequired: "This edit needs your review before it can be applied.",
  staleProposal: "The project changed after this edit was prepared, so it was not applied. Ask for the edit again.",
  noLongerValid: "This edit no longer fits the project, so it was not applied: ",
  nothingToUndo: "There is no agent edit to undo.",
  entryNotInHistory: "This edit is no longer in the undo history.",
  newerEditExists: "A newer agent edit was applied after this one. Undo the newer edit first.",
  projectChanged: "The project changed after this edit was applied, so undoing it would discard later changes.",
  interrupted: "app-server turn was interrupted",
} as const;

interface HistoryEntry extends UndoEntryRecords {
  readonly id: string;
  readonly before: VideoProject;
  /** Canonical content (see `undoContent`) of the project the edit committed. */
  readonly afterContent: string;
  readonly actionCount: number;
  readonly sessionId: string | null;
}

function delay(milliseconds: number): Promise<void> {
  return milliseconds > 0 ? new Promise((resolve) => setTimeout(resolve, milliseconds)) : Promise.resolve();
}

interface ConversationFixtureOptions {
  /** The fixture project folder, shared with every stateful fixture. */
  readonly store: FixtureProjectStore;
  /** How long a turn "thinks" before answering, so the working state is visible (default 0). */
  readonly turnDelayMs?: number;
}

export function conversationFixtureOperations({ store, turnDelayMs = 0 }: ConversationFixtureOptions): ReadonlyMap<string, FixtureOperationHandler> {
  const chats = createFixtureChats();
  const history: HistoryEntry[] = [];
  let interruptTurn: (() => void) | null = null;

  const committed = () => store.require();
  /** A native write: the committed project with the next content revision. */
  const write = (project: VideoProject) => store.write(project);

  async function startTurn(input: Record<string, unknown>): Promise<CodexConversationEditCommandResult> {
    const { project: sent, request, projectDir = "" } = input as { project: VideoProject; request: CodexConversationEditRequest; projectDir?: string };
    if (interruptTurn) throw "An app-server turn is already active for this project.";
    await new Promise<void>((resolve, reject) => {
      interruptTurn = () => reject(conversationFixtureCopy.interrupted);
      void delay(turnDelayMs).then(resolve);
    }).finally(() => {
      interruptTurn = null;
    });
    const base = store.current ?? sent;
    const threadId = turnThreadId(chats, base.id);
    const turn = fixtureTurn(base, request.prompt, projectDir);
    recordTurn(chats, base.id, threadId, request, turn.proposal !== null);
    const project = store.current ? write({ ...store.current, codexThreadId: threadId }) : sent;
    return {
      project,
      threadId,
      threadResponse: {},
      turnResponse: {},
      proposal: turn.proposal,
      preparedProposal: turn.prepared,
      proposalValidationIssues: turn.issues,
    };
  }

  function applyProposal(input: Record<string, unknown>): ProjectAgentApplyResult {
    const request = input as { proposal: CodexConversationEditProposal; actionIds: string[]; reviewApproved?: boolean; sessionId?: string | null };
    const before = committed();
    let prepared;
    let after;
    try {
      ({ prepared, after } = prepareFixtureProposal(before, request.proposal));
    } catch (error) {
      throw `${conversationFixtureCopy.noLongerValid}${error instanceof FixtureProposalError ? error.message : String(error)}`;
    }
    if (canonicalJson(prepared.actionIds) !== canonicalJson(request.actionIds)) throw conversationFixtureCopy.staleProposal;
    if (prepared.risk.level === "review" && request.reviewApproved !== true) throw conversationFixtureCopy.reviewRequired;
    const project = write(after);
    const records = addedRecords(before, project);
    const entry: HistoryEntry = {
      id: `agent-edit-r${(project.contentRevision ?? 0).toString()}`,
      before,
      afterContent: undoContent(project, records.addedGeneratedAssetIds, records.backgroundGeneratedAssetIds),
      ...records,
      actionCount: prepared.actions.length,
      sessionId: markSessionTurn(chats, project.id, request.sessionId ?? null, "applied", prepared.actionIds),
    };
    history.push(entry);
    return { project, report: writeReport(), historyEntryId: entry.id, actionIds: prepared.actionIds, risk: prepared.risk, impact: prepared.impact, warnings: [] };
  }

  function undoLatest(input: Record<string, unknown>): ProjectAgentUndoOutcome {
    const expected = (input as { historyEntryId?: string | null }).historyEntryId ?? null;
    const latest = history[history.length - 1];
    if (!latest) return { status: "unavailable", message: conversationFixtureCopy.nothingToUndo };
    if (expected !== null && expected !== latest.id) {
      return history.some((entry) => entry.id === expected)
        ? { status: "conflict", entryId: expected, message: conversationFixtureCopy.newerEditExists }
        : { status: "unavailable", message: conversationFixtureCopy.entryNotInHistory };
    }
    const current = committed();
    if (undoContent(current, latest.addedGeneratedAssetIds, latest.backgroundGeneratedAssetIds) !== latest.afterContent) return { status: "conflict", entryId: latest.id, message: conversationFixtureCopy.projectChanged };
    history.pop();
    const project = write(restoredProject(latest.before, latest, current));
    if (latest.sessionId) markSessionTurn(chats, project.id, latest.sessionId, "undone", null);
    const removedGeneratedAssetIds = current.generatedAssets.flatMap((asset) => (project.generatedAssets.some((kept) => kept.id === asset.id) ? [] : [asset.id]));
    return {
      status: "undone",
      project,
      report: writeReport(),
      entryId: latest.id,
      actionCount: latest.actionCount,
      remainingAgentHistory: history.length,
      warnings: [],
      removedGeneratedAssetIds,
    };
  }

  function captureFrame(input: Record<string, unknown>): PreparedPreviewFrameResult {
    const { playheadSeconds, jobId, updatedAt } = input as { playheadSeconds: number; jobId: string; updatedAt: string };
    const base = committed();
    const duration = base.timeline.durationSeconds;
    if (!Number.isFinite(playheadSeconds) || playheadSeconds < 0 || playheadSeconds >= duration) throw "Canonical preview capture needs a playhead before the timeline end.";
    const job: ProjectJobSummary = { id: jobId, kind: "captureCanonicalPreviewFrame", status: "completed", updatedAt };
    const project = write({ ...base, jobs: [...base.jobs.filter((candidate) => candidate.id !== jobId), job] });
    const renderDir = `renders/${jobId}`;
    const sourceOutput = `${renderDir}/output.mov`;
    const projectRenderReport: ProjectRenderReport = {
      schemaVersion: 1,
      id: jobId,
      status: "completed",
      outputPath: sourceOutput,
      durationSeconds: Math.min(duration - playheadSeconds, 1 / (base.renderSettings.fps || 30)),
      streams: { video: true, audio: false },
      checks: { duration: "passed", streams: "passed", captionAlignment: "skipped" },
      artifacts: [`${renderDir}/report.json`],
      previewComparisonRequest: null,
      previewComparison: null,
      logPath: `${renderDir}/render.log`,
      createdAt: updatedAt,
    };
    return {
      project,
      playheadSeconds,
      previewFrame: `${renderDir}/preview-qa/preview-frames/preview-0001.png`,
      sourceOutput,
      evidenceReport: `${renderDir}/preview-qa/canonical-preview-frame.json`,
      renderReport: { ...buildSampleRenderReport("finalWebm"), jobId },
      projectRenderReport,
    };
  }

  return new Map<string, FixtureOperationHandler>([
    ["start_codex_conversation_edit_for_project", startTurn],
    [
      "cancel_codex_conversation_edit_for_project",
      () => {
        if (!interruptTurn) return false;
        interruptTurn();
        return true;
      },
    ],
    ["apply_codex_conversation_proposal", applyProposal],
    ["undo_latest_codex_conversation_edit", undoLatest],
    ["capture_canonical_preview_frame_in_split_project_folder", captureFrame],
    ["load_agent_sessions_from_split_project_folder", () => sessionManifest(chats, committed().id)],
    [
      "apply_agent_session_action_to_split_project_folder",
      (input) => {
        const { projectId, action } = input as { projectId: string; action: ProjectAgentSessionAction };
        if (committed().id !== projectId) throw "agent session project id does not match the split project";
        return applySessionAction(chats, projectId, action);
      },
    ],
    ["load_app_server_conversation_history_from_split_project_folder", () => conversationHistory(chats)],
  ]);
}
