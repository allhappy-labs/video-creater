import {
  conversationReducer,
  isConversationBusy,
  type ConversationEvent,
} from "@/lib/agent/conversation-state";
import { conversationMentionTargets, resolveMentions } from "@/lib/agent/mentions";
import { resultFacts, reviewPlacements } from "@/lib/agent/result-facts";
import {
  applyCodexConversationProposal,
  cancelCodexConversationEditForProject,
  startCodexConversationEditForProject,
  undoLatestCodexConversationEdit,
  type CodexConversationEditCommandResult,
  type ProjectAction,
  type VideoProject,
} from "@/lib/project";
import { BackendOperationError, isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import type { AgentAssistantMessage, AgentFailure, AgentMessage, AgentProposalCard, AgentRuntime, AgentUserMessage } from "./agent-slice";
import type { EditorState, EditorStore } from "./editor-store";

export interface AgentTurnActions {
  /**
   * Sends the draft: resolves mentions (unresolved ones block the send with composer copy), merges the
   * context chip into the focus, runs the Codex turn, and auto-applies safe bundles when the setting
   * is on. Resolves true when the turn ran.
   */
  submitAgentPrompt(draft: string): Promise<boolean>;
  /** Applies a review card's bundle with the user's approval. */
  approveAgentProposal(messageId: string): Promise<boolean>;
  /** Dismisses a review card; nothing reaches the backend. */
  rejectAgentProposal(messageId: string): void;
  /** Undoes an applied card's batch; a conflict disables its Undo with the reason. */
  undoAgentEdit(messageId: string): Promise<boolean>;
  /** Retries the latest failed turn when safe: the apply again, or the prompt again. */
  retryAgentTurn(messageId: string): Promise<boolean>;
  /** Stops the in-flight agent turn (not an apply). `turnId` is the task id from the jobs slice. */
  cancelAgentTurn(turnId?: string): Promise<boolean>;
}

/** Plain-language apply refusals that another attempt can't fix; the user has to ask again. */
const staleApplyPattern = /project changed after this edit was prepared|no longer fits the project/i;
const reviewRequiredPattern = /needs your review/i;
const interruptedPattern = /turn was interrupted/i;
const alreadyActivePattern = /turn is already active/i;
/**
 * Refusals the user can act on, matched on the wording the backend's own turn
 * errors carry rather than on any one agent's raw output.
 */
const usageLimitPattern = /usage limit|rate limit/i;
const notSignedInPattern = /not (?:signed|logged) in|isn't signed in|sign in to/i;

const agentUnavailableCopy = "The AI agent isn't available right now.";
const usageLimitCopy = "The AI agent's usage limit is reached. Try again later.";
const notSignedInCopy = "The AI agent isn't signed in. Open Agent settings to fix it.";
const noProposalCopy = "The agent didn't suggest an edit. Try rephrasing the request.";
const busyCopy = "Another AI edit is still running for this project. Try again when it finishes.";

function now(): string {
  return new Date().toISOString();
}

/** Tauri commands reject with plain strings, which the backend client wraps as the error's cause. */
function backendMessage(error: unknown): string {
  const cause: unknown = error instanceof BackendOperationError ? error.cause : error;
  if (typeof cause === "string") return cause;
  return cause instanceof Error ? cause.message : String(cause);
}

function isProject(value: unknown): value is VideoProject {
  return typeof value === "object" && value !== null && "timeline" in value && "jobs" in value;
}

function turnLabel(prompt: string): string {
  const clean = prompt.replace(/\s+/g, " ").trim();
  return `AI edit: “${clean.length > 40 ? `${clean.slice(0, 40).trimEnd()}…` : clean}”`;
}

function timelineName(project: VideoProject): string {
  return project.timelines?.find((entry) => entry.id === project.activeTimelineId)?.name ?? "the timeline";
}

function unresolvedCopy(names: readonly string[]): string {
  const list = names.map((name) => `@${name}`).join(", ");
  return `Nothing in this project is named ${list}. Pick a name from the @ list.`;
}

function applyFailure(error: unknown): AgentFailure {
  if (isBackendUnavailableError(error)) return { kind: "applyFailed", message: agentUnavailableCopy, retryable: true };
  const message = backendMessage(error);
  const final = staleApplyPattern.test(message) || reviewRequiredPattern.test(message);
  return { kind: "applyFailed", message, retryable: !final };
}

function turnFailure(error: unknown): AgentFailure {
  if (isBackendUnavailableError(error)) return { kind: "agentUnavailable", message: agentUnavailableCopy, retryable: false };
  const message = backendMessage(error);
  if (usageLimitPattern.test(message)) return { kind: "agentUnavailable", message: usageLimitCopy, retryable: false };
  if (notSignedInPattern.test(message)) return { kind: "agentUnavailable", message: notSignedInCopy, retryable: false };
  return { kind: "turnFailed", message: alreadyActivePattern.test(message) ? busyCopy : message, retryable: true };
}

/**
 * The generation jobs a bundle records queued (Rust `generation_record_actions`). They make the bundle
 * review-level, so the user's approval ("Generate & place") is what starts them.
 */
function recordedGenerationJobIds(actions: readonly ProjectAction[]): string[] {
  return actions.flatMap((action) => (action.type === "recordJob" && action.job.kind === "generate_media" ? [action.job.id] : []));
}

const idleUndo: AgentProposalCard["undo"] = { available: false, pending: false, reason: null, error: null };

export function createAgentTurnActions(set: EditorStore["setState"], get: () => EditorState, runtime: AgentRuntime): AgentTurnActions {
  function dispatch(event: ConversationEvent): void {
    const current = get().agentConversation;
    const next = conversationReducer(current, event);
    if (next !== current) set({ agentConversation: next });
  }

  function isLatestTurn(userMessageId: string): boolean {
    const conversation = get().agentConversation;
    return "turn" in conversation && conversation.turn.messageId === userMessageId;
  }

  /** Looks in the open conversation, then in sessions switched away from (global Undo can reach those). */
  function findMessage<Role extends AgentMessage["role"]>(role: Role, id: string): Extract<AgentMessage, { role: Role }> | null {
    for (const messages of [get().agentMessages, ...runtime.messagesBySession.values()]) {
      const message = messages.find((candidate) => candidate.role === role && candidate.id === id);
      if (message) return message as Extract<AgentMessage, { role: Role }>;
    }
    return null;
  }

  function append(message: AgentMessage): void {
    set((state) => ({ agentMessages: [...state.agentMessages, message] }));
  }

  function patchAssistant(id: string, patch: (message: AgentAssistantMessage) => AgentAssistantMessage): void {
    const patchList = (messages: readonly AgentMessage[]) =>
      messages.map((message) => (message.role === "assistant" && message.id === id ? patch(message) : message));
    for (const [sessionId, messages] of runtime.messagesBySession) {
      if (messages.some((message) => message.id === id)) runtime.messagesBySession.set(sessionId, patchList(messages));
    }
    if (get().agentMessages.some((message) => message.id === id)) set((state) => ({ agentMessages: patchList(state.agentMessages) }));
  }

  /** Mirrors the turn into the background tasks list while the agent or an apply is working. */
  function track(userMessageId: string, status: "running" | "completed" | "failed" | "cancelled", phase: string | null, failureReason: string | null = null): void {
    const prompt = findMessage("user", userMessageId)?.text ?? "";
    get().setAgentTurn({ id: userMessageId, label: turnLabel(prompt), status, phase, failureReason, updatedAt: now() });
  }

  function stopped(userMessageId: string): boolean {
    dispatch({ type: "cancelled" });
    track(userMessageId, "cancelled", null);
    return false;
  }

  function failTurn(user: AgentUserMessage, failure: AgentFailure): boolean {
    if (failure.kind !== "applyFailed") dispatch({ type: "failed", kind: failure.kind, message: failure.message });
    append({ role: "assistant", id: runtime.nextId("assistant"), replyTo: user.id, createdAt: now(), text: null, status: "failed", card: null, failure });
    // A blocked edit keeps the prompt so the user can rephrase it.
    if (failure.kind === "validationBlocked" && !get().agentDraft) set({ agentDraft: user.text });
    track(user.id, "failed", null, failure.message);
    return false;
  }

  async function applyProposal(messageId: string, approved: boolean): Promise<boolean> {
    const message = findMessage("assistant", messageId);
    const card = message?.card;
    if (!message || !card) return false;
    patchAssistant(messageId, (current) => ({ ...current, status: "applying", failure: null, card: { ...card, approved } }));
    track(message.replyTo, "running", "Applying changes");
    const before = get().project;
    try {
      const result = await applyCodexConversationProposal({
        projectDir: get().projectDir,
        proposal: card.proposal,
        actionIds: [...card.prepared.actionIds],
        reviewApproved: approved,
        sessionId: get().agentSessions?.activeSessionId ?? null,
      });
      await get().commitAgentApply(result.project, messageId);
      // Starting is background work: a start that fails reports in `lastError` and the edit stays applied.
      get().startRecordedGenerations(recordedGenerationJobIds(card.prepared.actions));
      patchAssistant(messageId, (current) => ({
        ...current,
        status: "applied",
        card: {
          ...card,
          approved,
          facts: resultFacts(result.impact, card.prepared.actions, before, result.project),
          result: {
            historyEntryId: result.historyEntryId,
            actionIds: result.actionIds,
            impact: result.impact,
            risk: result.risk,
            warnings: result.warnings,
            revision: result.project.contentRevision ?? 0,
            timelineName: timelineName(result.project),
          },
          undo: { ...idleUndo, available: true },
        },
      }));
      if (isLatestTurn(message.replyTo)) dispatch({ type: "applied" });
      track(message.replyTo, "completed", null);
      return true;
    } catch (error) {
      const failure = applyFailure(error);
      patchAssistant(messageId, (current) => ({ ...current, status: "failed", failure }));
      if (isLatestTurn(message.replyTo)) dispatch({ type: "applyFailed", message: failure.message });
      track(message.replyTo, "failed", null, failure.message);
      return false;
    }
  }

  async function onTurnResult(user: AgentUserMessage, result: CodexConversationEditCommandResult): Promise<boolean> {
    if (runtime.cancelRequestedFor === user.id) return stopped(user.id);
    const { proposal, preparedProposal: prepared } = result;
    if (!proposal) return failTurn(user, { kind: "turnFailed", message: noProposalCopy, retryable: true });
    if (!prepared) {
      const issue = result.proposalValidationIssues?.[0]?.message ?? "The edit didn't pass validation.";
      return failTurn(user, { kind: "validationBlocked", message: issue, retryable: false });
    }
    dispatch({ type: "phase", phase: "validating" });
    dispatch({ type: "prepared", riskLevel: prepared.risk.level, autoApplySafe: get().autoApplySafe });
    const autoApply = get().agentConversation.status === "working";
    const project = get().project;
    const reply: AgentAssistantMessage = {
      role: "assistant",
      id: runtime.nextId("assistant"),
      replyTo: user.id,
      createdAt: now(),
      text: proposal.summary,
      status: autoApply ? "applying" : "awaitingReview",
      card: {
        proposal,
        prepared,
        facts: resultFacts(prepared.impact, prepared.actions, project, null),
        placements: reviewPlacements(prepared.actions, project),
        approved: false,
        result: null,
        undo: idleUndo,
      },
      failure: null,
    };
    append(reply);
    if (autoApply) return applyProposal(reply.id, false);
    // A review card waits for the user, not for background work: the turn leaves the tasks list
    // rather than reading as a completed edit. Approving tracks the apply as a new running task.
    get().setAgentTurn(null);
    return true;
  }

  async function runTurn(user: AgentUserMessage): Promise<boolean> {
    runtime.cancelRequestedFor = null;
    track(user.id, "running", "Reviewing the timeline");
    const { project, projectDir } = get();
    try {
      const result = await startCodexConversationEditForProject({
        ...(projectDir.trim() ? { projectDir } : {}),
        project,
        request: { prompt: user.text, focus: runtime.focusByTurn.get(user.id) ?? { mediaIds: [], timelineItemIds: [] }, createdAt: user.createdAt },
      });
      if (runtime.cancelRequestedFor === user.id) return stopped(user.id);
      // The turn persisted its thread and bumped the revision; merge that bookkeeping without an undo step.
      if (isProject(result.project)) await get().mergeLoadedProject(result.project);
      return await onTurnResult(user, result);
    } catch (error) {
      if (runtime.cancelRequestedFor === user.id || interruptedPattern.test(backendMessage(error))) return stopped(user.id);
      return failTurn(user, turnFailure(error));
    } finally {
      if (runtime.cancelRequestedFor === user.id) runtime.cancelRequestedFor = null;
    }
  }

  return {
    async submitAgentPrompt(draft) {
      const prompt = draft.trim();
      const state = get();
      if (!prompt || isConversationBusy(state.agentConversation)) return false;
      const { focus, unresolved } = resolveMentions(prompt, conversationMentionTargets(state.project));
      if (unresolved.length > 0) {
        set({ agentComposerError: unresolvedCopy(unresolved) });
        return false;
      }
      const chipItemIds = state.agentContextChip?.itemIds ?? [];
      const user: AgentUserMessage = { role: "user", id: runtime.nextId("user"), text: prompt, createdAt: now() };
      runtime.focusByTurn.set(user.id, {
        mediaIds: focus.mediaIds,
        timelineItemIds: [...new Set([...focus.timelineItemIds, ...chipItemIds])],
      });
      set((current) => ({
        // A new message supersedes an unanswered review.
        agentMessages: [
          ...current.agentMessages.map((message) =>
            message.role === "assistant" && message.status === "awaitingReview" ? { ...message, status: "dismissed" as const } : message,
          ),
          user,
        ],
        agentDraft: "",
        agentComposerError: null,
        agentConfirmSend: false,
      }));
      runtime.resetContextChip();
      dispatch({ type: "submit", messageId: user.id, draft: prompt });
      return runTurn(user);
    },

    async approveAgentProposal(messageId) {
      const message = findMessage("assistant", messageId);
      if (message?.status !== "awaitingReview" || !message.card) return false;
      dispatch({ type: "reviewApproved" });
      return applyProposal(messageId, true);
    },

    rejectAgentProposal(messageId) {
      const message = findMessage("assistant", messageId);
      if (message?.status !== "awaitingReview") return;
      patchAssistant(messageId, (current) => ({ ...current, status: "dismissed" }));
      if (isLatestTurn(message.replyTo)) dispatch({ type: "rejected" });
    },

    async undoAgentEdit(messageId) {
      const message = findMessage("assistant", messageId);
      const card = message?.card;
      if (!message || message.status !== "applied" || !card?.result || !card.undo.available || card.undo.pending) return false;
      const patchUndo = (undo: AgentProposalCard["undo"], status: AgentAssistantMessage["status"] = "applied") =>
        patchAssistant(messageId, (current) => (current.card ? { ...current, status, card: { ...current.card, undo } } : current));
      patchUndo({ ...card.undo, pending: true, error: null });
      try {
        const outcome = await undoLatestCodexConversationEdit({ projectDir: get().projectDir, historyEntryId: card.result.historyEntryId });
        if (outcome.status === "undone") {
          // The backend removed the batch's generations and cancelled the running ones; stop placing their output.
          get().forgetRemovedGenerations(outcome.removedGeneratedAssetIds);
          await get().commitAgentUndo(messageId, outcome.project);
          patchUndo(idleUndo, "undone");
          if (isLatestTurn(message.replyTo)) dispatch({ type: "undone" });
          return true;
        }
        // Neither outcome can succeed later, so global Undo moves on to older changes.
        await get().commitAgentUndo(messageId, null);
        patchUndo({ available: false, pending: false, reason: outcome.message, error: null });
        if (outcome.status === "conflict" && isLatestTurn(message.replyTo)) dispatch({ type: "undoConflict" });
        return false;
      } catch (error) {
        patchUndo({ available: true, pending: false, reason: null, error: isBackendUnavailableError(error) ? agentUnavailableCopy : backendMessage(error) });
        return false;
      }
    },

    async retryAgentTurn(messageId) {
      const message = findMessage("assistant", messageId);
      if (message?.status !== "failed" || !message.failure?.retryable || !isLatestTurn(message.replyTo)) return false;
      if (message.card) {
        dispatch({ type: "retry" });
        return applyProposal(messageId, message.card.approved);
      }
      const user = findMessage("user", message.replyTo);
      if (!user) return false;
      set((state) => ({ agentMessages: state.agentMessages.filter((candidate) => candidate.id !== messageId) }));
      dispatch({ type: "retry" });
      return runTurn(user);
    },

    async cancelAgentTurn(turnId) {
      const conversation = get().agentConversation;
      const inFlight = conversation.status === "validating" || (conversation.status === "working" && conversation.phase === "reviewing");
      if (!inFlight || (turnId !== undefined && turnId !== conversation.turn.messageId)) return false;
      // The start call settles as Stopped whether or not the backend still had a turn to interrupt.
      runtime.cancelRequestedFor = conversation.turn.messageId;
      try {
        await cancelCodexConversationEditForProject({ projectDir: get().projectDir });
      } catch {
        // Nothing to interrupt remotely; the pending result is still discarded.
      }
      return true;
    },
  };
}
