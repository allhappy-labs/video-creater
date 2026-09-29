import {
  initialConversationState,
  type ConversationFailureKind,
  type ConversationState,
} from "@/lib/agent/conversation-state";
import { conversationMentionTargets, insertMention } from "@/lib/agent/mentions";
import type { ResultFact } from "@/lib/agent/result-facts";
import type {
  CodexConversationEditProposal,
  CodexConversationFocus,
  CodexPreparedProposal,
  CodexProposalImpact,
  CodexProposalRisk,
  ProjectAgentSessionManifest,
  VideoProject,
} from "@/lib/project";
import { createAgentSessionActions, type AgentSessionActions } from "./agent-session-actions";
import { createAgentTurnActions, type AgentTurnActions } from "./agent-turn-actions";
import type { EditorSliceCreator } from "./editor-store";
import { loadAgentAutoApplySafe, saveAgentAutoApplySafe } from "./persisted-layout";
import type { PendingAgentRequest } from "./ui-slice";

/**
 * The AI tab conversation: chat sessions, the message list with per-card state, the latest turn's
 * `conversationReducer` state, the composer, and the auto-apply preference. Backend calls live in
 * `agent-turn-actions.ts` (turns, apply, undo, cancel) and `agent-session-actions.ts` (sessions).
 */

export interface AgentUserMessage {
  readonly role: "user";
  readonly id: string;
  /** The user's words as sent; mentions stay as `@<human name>`. */
  readonly text: string;
  readonly createdAt: string;
}

/** What the applied batch did, as recomputed by Rust. `historyEntryId` is the batch's Undo identity. */
export interface AgentAppliedResult {
  readonly historyEntryId: string;
  readonly actionIds: readonly string[];
  readonly impact: CodexProposalImpact;
  readonly risk: CodexProposalRisk;
  /** Session bookkeeping notes; the edit still applied. */
  readonly warnings: readonly string[];
  /** The applied project's content revision; a newer project revision makes result frames stale. */
  readonly revision: number;
  readonly timelineName: string;
}

interface AgentCardUndo {
  readonly available: boolean;
  readonly pending: boolean;
  /** Why Undo is disabled (a conflict or unavailable history), in plain language. */
  readonly reason: string | null;
  /** A failed Undo request that may be retried; Undo stays available. */
  readonly error: string | null;
}

/** The validated bundle a result or review card shows, plus its apply outcome. */
export interface AgentProposalCard {
  readonly proposal: CodexConversationEditProposal;
  readonly prepared: CodexPreparedProposal;
  /** Review cards use the pre-apply project; applied cards are recomputed against the result. */
  readonly facts: readonly ResultFact[];
  readonly placements: readonly string[];
  /** Whether the user approved it from a review card (Retry keeps the approval). */
  readonly approved: boolean;
  readonly result: AgentAppliedResult | null;
  readonly undo: AgentCardUndo;
}

export interface AgentFailure {
  readonly kind: ConversationFailureKind;
  readonly message: string;
  readonly retryable: boolean;
}

type AgentCardStatus = "awaitingReview" | "applying" | "applied" | "undone" | "failed" | "dismissed";

export interface AgentAssistantMessage {
  readonly role: "assistant";
  readonly id: string;
  /** The user message this reply answers. */
  readonly replyTo: string;
  readonly createdAt: string;
  /** The proposal summary; null for failures without a proposal. */
  readonly text: string | null;
  readonly status: AgentCardStatus;
  readonly card: AgentProposalCard | null;
  readonly failure: AgentFailure | null;
}

export type AgentMessage = AgentUserMessage | AgentAssistantMessage;

/** The selection shown as one removable composer chip; `itemIds` join the next request's focus. */
interface AgentContextChip {
  readonly itemIds: readonly string[];
  readonly label: string;
}

/** Closure state shared by the action modules; none of it renders. */
export interface AgentRuntime {
  /** The user message whose in-flight turn the user asked to stop. */
  cancelRequestedFor: string | null;
  readonly focusByTurn: Map<string, CodexConversationFocus>;
  /** Messages of sessions switched away from, so live cards survive switching back. */
  readonly messagesBySession: Map<string, readonly AgentMessage[]>;
  nextId(prefix: string): string;
  /** Clears the chip's handoff items and removal after a send. */
  resetContextChip(): void;
}

export interface AgentSlice extends AgentSessionActions, AgentTurnActions {
  readonly agentSessions: ProjectAgentSessionManifest | null;
  readonly agentSessionError: string | null;
  readonly agentMessages: readonly AgentMessage[];
  /** The latest turn's state machine; the progress row reads its phase. */
  readonly agentConversation: ConversationState;
  readonly autoApplySafe: boolean;
  readonly agentDraft: string;
  readonly agentContextChip: AgentContextChip | null;
  /** Inline composer copy, e.g. an unresolved mention; cleared when the draft changes. */
  readonly agentComposerError: string | null;
  /** A handoff drafted a prompt to send after the user confirms ("Send this request?"); a send clears it. */
  readonly agentConfirmSend: boolean;
  /** Registers the agent cancel with the jobs slice and loads sessions (`EditorRoot` mount). */
  startAgent(): () => void;
  setAutoApplySafe(value: boolean): void;
  setAgentDraft(draft: string): void;
  /** Excludes the chip's items from the next send; a new selection brings the chip back. */
  removeAgentContextChip(): void;
  /** Hands a request to the AI tab: stores it and switches to the tab (its sheet when one is open). */
  requestAgent(request: PendingAgentRequest): void;
  /** Attach from the composer: opens the Media tab (its sheet when one is open) in attach mode. */
  startAgentMediaAttach(): void;
  /** Leaves attach mode without a pick and returns to the AI tab. */
  cancelAgentMediaAttach(): void;
  /** Ends attach mode with a pick: mentions the media at the end of the draft and returns to the AI tab. */
  attachAgentMedia(mediaId: string): boolean;
  /**
   * Fills the draft and chip from the pending request without sending; a `confirmSend` request asks
   * for confirmation first. Returns false when none.
   */
  consumePendingAgentRequest(): boolean;
  /** Confirms a handoff: sends the current draft. Resolves true when the turn ran. */
  confirmAgentSend(): Promise<boolean>;
  /** Declines a handoff's send; the draft stays for editing. */
  cancelAgentSend(): void;
  /** Re-derives the context chip; the store calls it on every change. */
  syncAgentContext(): void;
}

function timelineItemName(project: VideoProject, itemId: string): string {
  return conversationMentionTargets(project).find((target) => target.kind === "timelineItem" && target.id === itemId)?.name ?? "Clip";
}

function contextChipFor(project: VideoProject, itemIds: readonly string[]): AgentContextChip | null {
  const present = new Set(project.timeline.tracks.flatMap((track) => track.items.map((item) => item.id)));
  const kept = itemIds.filter((id) => present.has(id));
  const [first] = kept;
  if (first === undefined) return null;
  const name = timelineItemName(project, first);
  return { itemIds: kept, label: kept.length === 1 ? name : `${name} + ${(kept.length - 1).toString()} more` };
}

let messageSequence = 0;

export function createAgentSlice(): EditorSliceCreator<AgentSlice> {
  return (set, get) => {
    let handoffItemIds: readonly string[] | null = null;
    let removed = false;
    let seenSelection: readonly string[] | null = null;
    let derivedFrom: readonly unknown[] = [];

    function syncChip(force: boolean): void {
      const state = get();
      if (seenSelection !== null && state.selectedItemIds !== seenSelection) {
        // A new selection replaces a handoff's items and brings a removed chip back.
        handoffItemIds = null;
        removed = false;
      }
      seenSelection = state.selectedItemIds;
      const inputs = [state.project, state.selectedItemIds, handoffItemIds, removed];
      if (!force && inputs.every((input, index) => input === derivedFrom[index])) return;
      derivedFrom = inputs;
      const chip = removed ? null : contextChipFor(state.project, handoffItemIds ?? state.selectedItemIds);
      if (JSON.stringify(chip) !== JSON.stringify(state.agentContextChip)) set({ agentContextChip: chip });
    }

    const runtime: AgentRuntime = {
      cancelRequestedFor: null,
      focusByTurn: new Map(),
      messagesBySession: new Map(),
      nextId(prefix) {
        messageSequence += 1;
        return `${prefix}-${Date.now().toString(36)}-${messageSequence.toString()}`;
      },
      resetContextChip() {
        handoffItemIds = null;
        removed = false;
        syncChip(true);
      },
    };

    return {
      agentSessions: null,
      agentSessionError: null,
      agentMessages: [],
      agentConversation: initialConversationState,
      autoApplySafe: loadAgentAutoApplySafe(),
      agentDraft: "",
      agentContextChip: null,
      agentComposerError: null,
      agentConfirmSend: false,
      ...createAgentSessionActions(set, get, runtime),
      ...createAgentTurnActions(set, get, runtime),

      startAgent() {
        const unregister = get().registerAgentCancel((turnId) => get().cancelAgentTurn(turnId));
        void get().loadAgentSessions();
        return unregister;
      },

      setAutoApplySafe(value) {
        set({ autoApplySafe: value });
        saveAgentAutoApplySafe(value);
      },

      setAgentDraft: (draft) => set({ agentDraft: draft, agentComposerError: null }),

      removeAgentContextChip() {
        removed = true;
        syncChip(true);
      },

      requestAgent(request) {
        const state = get();
        state.setPendingAgentRequest(request);
        state.setActiveTab("ai");
        if (state.openSheetId !== null) state.openSheet("ai");
      },

      startAgentMediaAttach() {
        const state = get();
        state.setReplaceTargetItemId(null);
        state.setMediaAttachMode(true);
        state.setActiveTab("media");
        if (state.openSheetId !== null) state.openSheet("media");
      },

      cancelAgentMediaAttach() {
        const state = get();
        state.setMediaAttachMode(false);
        state.setActiveTab("ai");
        if (state.openSheetId !== null) state.openSheet("ai");
      },

      attachAgentMedia(mediaId) {
        const state = get();
        const target = conversationMentionTargets(state.project).find((candidate) => candidate.kind === "media" && candidate.id === mediaId);
        state.setMediaAttachMode(false);
        if (target) {
          const { draft } = insertMention(state.agentDraft, state.agentDraft.length, target);
          set({ agentDraft: draft, agentComposerError: null });
        }
        state.setActiveTab("ai");
        if (state.openSheetId !== null) state.openSheet("ai");
        return target !== undefined;
      },

      consumePendingAgentRequest() {
        const state = get();
        const request = state.pendingAgentRequest;
        if (!request) return false;
        handoffItemIds = request.itemIds.length > 0 ? request.itemIds : null;
        removed = false;
        set({
          pendingAgentRequest: null,
          agentComposerError: null,
          agentConfirmSend: request.confirmSend === true && request.prompt !== undefined,
          ...(request.prompt === undefined ? {} : { agentDraft: request.prompt }),
        });
        syncChip(true);
        if (state.activeTab !== "ai") state.setActiveTab("ai");
        if (state.openSheetId !== null && state.openSheetId !== "ai") state.openSheet("ai");
        return true;
      },

      confirmAgentSend: () => get().submitAgentPrompt(get().agentDraft),

      cancelAgentSend: () => set({ agentConfirmSend: false }),

      syncAgentContext: () => syncChip(false),
    };
  };
}
