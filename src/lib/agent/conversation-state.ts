import type { CodexProposalRiskLevel } from "@/lib/project";

/**
 * Pure state machine for the latest AI conversation turn:
 * `idle → working → validating → applied | awaitingReview | failed`, plus `undone`.
 * The store slice performs the backend calls; this reducer only decides what happens next.
 */

/** Reason shown on a disabled Undo when a later edit changed the project. */
export const undoConflictReason = "A newer edit changed the project.";

type ConversationPhase = "reviewing" | "validating" | "applying" | "preparingPreview";

type TurnPhase = "reviewing" | "validating";
type ApplyPhase = "applying" | "preparingPreview";

interface ConversationTurn {
  /** The user message this turn answers. */
  readonly messageId: string;
  /** The submitted prompt, kept so failures can restore or resubmit it. */
  readonly draft: string;
}

export type ConversationFailureKind = "validationBlocked" | "applyFailed" | "turnFailed" | "agentUnavailable" | "outcomeUnknown";

type ConversationRetry =
  | { readonly kind: "apply"; readonly reviewApproved: boolean }
  | { readonly kind: "resubmit" };

interface ConversationFailure {
  readonly kind: ConversationFailureKind;
  readonly message: string;
  /** Null when retrying is not safe or not meaningful. */
  readonly retry: ConversationRetry | null;
}

interface ConversationUndo {
  readonly available: boolean;
  readonly reason: string | null;
}

export type ConversationState =
  | { readonly status: "idle"; readonly note: "Stopped" | "Dismissed" | null }
  | {
      readonly status: "working";
      readonly phase: "reviewing" | ApplyPhase;
      readonly turn: ConversationTurn;
      /** Meaningful while applying: whether the user approved a review card. */
      readonly reviewApproved: boolean;
    }
  | { readonly status: "validating"; readonly turn: ConversationTurn }
  | { readonly status: "awaitingReview"; readonly turn: ConversationTurn; readonly riskLevel: CodexProposalRiskLevel }
  | {
      readonly status: "applied";
      readonly turn: ConversationTurn;
      readonly reviewApproved: boolean;
      readonly undo: ConversationUndo;
    }
  | { readonly status: "undone"; readonly turn: ConversationTurn }
  | { readonly status: "failed"; readonly turn: ConversationTurn; readonly failure: ConversationFailure };

export type ConversationEvent =
  | { readonly type: "submit"; readonly messageId: string; readonly draft: string }
  | { readonly type: "phase"; readonly phase: ConversationPhase }
  | { readonly type: "prepared"; readonly riskLevel: CodexProposalRiskLevel; readonly autoApplySafe: boolean }
  | { readonly type: "applied" }
  | { readonly type: "applyFailed"; readonly message: string }
  | { readonly type: "reviewApproved" }
  | { readonly type: "rejected" }
  | { readonly type: "undone" }
  | { readonly type: "undoConflict" }
  | { readonly type: "cancelled" }
  | {
      readonly type: "failed";
      readonly kind: Exclude<ConversationFailureKind, "applyFailed">;
      readonly message: string;
    }
  | { readonly type: "retry" };

export const initialConversationState: ConversationState = { status: "idle", note: null };

function applying(turn: ConversationTurn, reviewApproved: boolean): ConversationState {
  return { status: "working", phase: "applying", turn, reviewApproved };
}

function reviewing(turn: ConversationTurn): ConversationState {
  return { status: "working", phase: "reviewing", turn, reviewApproved: false };
}

/** The turn is waiting on the agent (not yet prepared, not yet applying). */
function inFlightTurn(state: ConversationState): ConversationTurn | null {
  if (state.status === "validating") return state.turn;
  if (state.status === "working" && state.phase === "reviewing") return state.turn;
  return null;
}

function isApplying(state: ConversationState): state is Extract<ConversationState, { status: "working" }> {
  return state.status === "working" && state.phase !== "reviewing";
}

export function isConversationBusy(state: ConversationState): boolean {
  return state.status === "working" || state.status === "validating";
}

function onPhase(state: ConversationState, phase: ConversationPhase): ConversationState {
  const turn = inFlightTurn(state);
  if (turn) {
    const next: TurnPhase | null = phase === "reviewing" || phase === "validating" ? phase : null;
    if (!next) return state;
    return next === "validating" ? { status: "validating", turn } : reviewing(turn);
  }
  if (isApplying(state) && (phase === "applying" || phase === "preparingPreview")) {
    return state.phase === phase ? state : { ...state, phase };
  }
  return state;
}

function retryFailure(state: Extract<ConversationState, { status: "failed" }>): ConversationState {
  const { retry } = state.failure;
  if (!retry) return state;
  return retry.kind === "apply" ? applying(state.turn, retry.reviewApproved) : reviewing(state.turn);
}

export function conversationReducer(state: ConversationState, event: ConversationEvent): ConversationState {
  switch (event.type) {
    case "submit": {
      if (isConversationBusy(state) || !event.draft.trim()) return state;
      return reviewing({ messageId: event.messageId, draft: event.draft });
    }
    case "phase":
      return onPhase(state, event.phase);
    case "prepared": {
      const turn = inFlightTurn(state);
      if (!turn) return state;
      // Review-level bundles always wait; safe ones wait only when auto-apply is off.
      if (event.riskLevel === "safe" && event.autoApplySafe) return applying(turn, false);
      return { status: "awaitingReview", turn, riskLevel: event.riskLevel };
    }
    case "reviewApproved":
      return state.status === "awaitingReview" ? applying(state.turn, true) : state;
    case "rejected":
      return state.status === "awaitingReview" ? { status: "idle", note: "Dismissed" } : state;
    case "applied":
      return isApplying(state)
        ? {
            status: "applied",
            turn: state.turn,
            reviewApproved: state.reviewApproved,
            undo: { available: true, reason: null },
          }
        : state;
    case "applyFailed":
      return isApplying(state)
        ? {
            status: "failed",
            turn: state.turn,
            failure: {
              kind: "applyFailed",
              message: event.message,
              retry: { kind: "apply", reviewApproved: state.reviewApproved },
            },
          }
        : state;
    case "failed": {
      const turn = inFlightTurn(state) ?? (event.kind === "outcomeUnknown" && isApplying(state) ? state.turn : null);
      if (!turn) return state;
      const retry: ConversationRetry | null = event.kind === "turnFailed" ? { kind: "resubmit" } : null;
      return { status: "failed", turn, failure: { kind: event.kind, message: event.message, retry } };
    }
    case "undone":
      return state.status === "applied" && state.undo.available ? { status: "undone", turn: state.turn } : state;
    case "undoConflict":
      return state.status === "applied"
        ? { ...state, undo: { available: false, reason: undoConflictReason } }
        : state;
    case "cancelled":
      // An atomic apply cannot be stopped midway, so cancel only affects the agent turn.
      return inFlightTurn(state) ? { status: "idle", note: "Stopped" } : state;
    case "retry":
      return state.status === "failed" ? retryFailure(state) : state;
  }
}

const phaseLabels: Record<ConversationPhase, string> = {
  reviewing: "Reviewing the timeline",
  validating: "Validating the edit",
  applying: "Applying changes",
  preparingPreview: "Preparing the preview",
};

const failureLabels: Record<ConversationFailureKind, string> = {
  validationBlocked: "Couldn't validate the edit",
  applyFailed: "Nothing was changed",
  turnFailed: "The edit didn't finish",
  agentUnavailable: "Agent unavailable",
  outcomeUnknown: "Edit unconfirmed",
};

/** The failure card's text status. */
export function conversationFailureLabel(kind: ConversationFailureKind): string {
  return failureLabels[kind];
}

/** Text status for the progress row and cards, so status never relies on color alone. */
export function conversationStatusLabel(state: ConversationState): string {
  switch (state.status) {
    case "idle":
      return state.note ?? "Ready";
    case "working":
      return phaseLabels[state.phase];
    case "validating":
      return phaseLabels.validating;
    case "awaitingReview":
      return "Needs your review";
    case "applied":
      return "Applied";
    case "undone":
      return "Undone";
    case "failed":
      return failureLabels[state.failure.kind];
  }
}
