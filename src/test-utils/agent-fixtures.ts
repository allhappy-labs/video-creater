import type { AgentAppliedResult, AgentAssistantMessage, AgentProposalCard, AgentUserMessage } from "@/editor/store/agent-slice";
import type { ConversationFailureKind } from "@/lib/agent/conversation-state";
import { resultFacts, reviewPlacements } from "@/lib/agent/result-facts";
import type { CodexPreparedProposal, CodexProposalRiskLevel, ProjectAction, VideoProject } from "@/lib/project";
import { fixtureItem, fixtureProject } from "./editor-fixtures";

/** The sample project as a saved split project at content revision 5. */
export function agentProject(): VideoProject {
  return { ...fixtureProject(), schemaVersion: 2, contentRevision: 5 };
}

export function userMessage(id: string, text: string): AgentUserMessage {
  return { role: "user", id, text, createdAt: "2026-09-15T10:00:00.000Z" };
}

/** An opacity change plus a cut on the first video clip, touching the 0–4 s range. */
function fixtureActions(project: VideoProject): ProjectAction[] {
  const video = fixtureItem(project, "video");
  return [
    { type: "updateVisualClipOpacity", itemId: video.id, opacity: 0.5 },
    { type: "splitItems", splits: [{ itemId: video.id, newItemId: `${video.id}-split`, splitSeconds: 2 }] },
  ];
}

function preparedProposal(project: VideoProject, level: CodexProposalRiskLevel = "safe", actions: ProjectAction[] = fixtureActions(project)): CodexPreparedProposal {
  const video = fixtureItem(project, "video");
  return {
    actions,
    actionIds: actions.map((_, index) => `codex-action-${index.toString()}-0a1b2c3d`),
    risk: { level, reasons: level === "review" ? [{ code: "generation", message: "Generates new media." }] : [] },
    impact: {
      summary: "Changes 1 item.",
      beforeDurationSeconds: 8,
      afterDurationSeconds: 6,
      affectedItemIds: [video.id],
      affectedRanges: [
        { startSeconds: 1, endSeconds: 2 },
        { startSeconds: 3, endSeconds: 3.5 },
      ],
      previewTimestamp: 0.5,
    },
  };
}

interface CardOptions {
  readonly level?: CodexProposalRiskLevel;
  readonly actions?: ProjectAction[];
  readonly result?: Partial<AgentAppliedResult> | null;
  readonly undo?: Partial<AgentProposalCard["undo"]>;
  readonly approved?: boolean;
}

function proposalCard(project: VideoProject, options: CardOptions = {}): AgentProposalCard {
  const prepared = preparedProposal(project, options.level, options.actions);
  const result: AgentAppliedResult | null =
    options.result === null || options.result === undefined
      ? null
      : {
          historyEntryId: "agent-edit-1",
          actionIds: prepared.actionIds,
          impact: prepared.impact,
          risk: prepared.risk,
          warnings: [],
          revision: project.contentRevision ?? 0,
          timelineName: "Timeline 1",
          ...options.result,
        };
  return {
    proposal: { summary: "Tightened the opening.", edl: [], projectActions: prepared.actions, renderReview: null },
    prepared,
    facts: resultFacts(prepared.impact, prepared.actions, project, result ? project : null),
    placements: reviewPlacements(prepared.actions, project),
    approved: options.approved ?? false,
    result,
    undo: { available: result !== null, pending: false, reason: null, error: null, ...options.undo },
  };
}

export function appliedMessage(project: VideoProject, id: string, replyTo: string, options: CardOptions = {}): AgentAssistantMessage {
  const card = proposalCard(project, { ...options, result: options.result ?? {} });
  return { role: "assistant", id, replyTo, createdAt: "2026-09-15T10:00:01.000Z", text: card.proposal.summary, status: "applied", card, failure: null };
}

export function reviewMessage(project: VideoProject, id: string, replyTo: string, options: CardOptions = {}): AgentAssistantMessage {
  const card = proposalCard(project, { level: "review", ...options, result: null });
  return { role: "assistant", id, replyTo, createdAt: "2026-09-15T10:00:01.000Z", text: card.proposal.summary, status: "awaitingReview", card, failure: null };
}

export function failedMessage(id: string, replyTo: string, kind: ConversationFailureKind, message: string, retryable: boolean): AgentAssistantMessage {
  return { role: "assistant", id, replyTo, createdAt: "2026-09-15T10:00:01.000Z", text: null, status: "failed", card: null, failure: { kind, message, retryable } };
}
