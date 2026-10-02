import { describe, expect, it } from "vitest";
import {
  conversationReducer,
  conversationStatusLabel,
  initialConversationState,
  isConversationBusy,
  undoConflictReason,
  type ConversationEvent,
  type ConversationState,
} from "@/lib/agent/conversation-state";

const turn = { messageId: "message-1", draft: "Tighten the pacing" };

function run(events: readonly ConversationEvent[], state: ConversationState = initialConversationState) {
  return events.reduce(conversationReducer, state);
}

const submitted = run([{ type: "submit", ...turn }]);
const validating = run([{ type: "phase", phase: "validating" }], submitted);

describe("conversationReducer", () => {
  it("starts idle with no note", () => {
    expect(initialConversationState).toEqual({ status: "idle", note: null });
    expect(conversationStatusLabel(initialConversationState)).toBe("Ready");
  });

  it("submit starts a reviewing turn that keeps the draft", () => {
    expect(submitted).toEqual({ status: "working", phase: "reviewing", turn, reviewApproved: false });
    expect(conversationStatusLabel(submitted)).toBe("Reviewing the timeline");
    expect(isConversationBusy(submitted)).toBe(true);
  });

  it("ignores blank submissions and submissions while busy", () => {
    expect(run([{ type: "submit", messageId: "m", draft: "   " }])).toBe(initialConversationState);
    expect(conversationReducer(submitted, { type: "submit", messageId: "m-2", draft: "Other" })).toBe(submitted);
  });

  it("phase moves between working and validating", () => {
    expect(validating).toEqual({ status: "validating", turn });
    expect(conversationStatusLabel(validating)).toBe("Validating the edit");
    expect(isConversationBusy(validating)).toBe(true);
    expect(conversationReducer(validating, { type: "phase", phase: "reviewing" })).toEqual(submitted);
    expect(conversationReducer(initialConversationState, { type: "phase", phase: "validating" })).toBe(
      initialConversationState,
    );
  });

  it("a safe proposal with auto-apply on goes prepared → working(apply) → applied", () => {
    const applying = conversationReducer(validating, { type: "prepared", riskLevel: "safe", autoApplySafe: true });
    expect(applying).toEqual({ status: "working", phase: "applying", turn, reviewApproved: false });
    expect(conversationStatusLabel(applying)).toBe("Applying changes");

    const previewing = conversationReducer(applying, { type: "phase", phase: "preparingPreview" });
    expect(conversationStatusLabel(previewing)).toBe("Preparing the preview");

    const applied = conversationReducer(previewing, { type: "applied" });
    expect(applied).toEqual({
      status: "applied",
      turn,
      reviewApproved: false,
      undo: { available: true, reason: null },
    });
    expect(conversationStatusLabel(applied)).toBe("Applied");
    expect(isConversationBusy(applied)).toBe(false);
  });

  it("a safe proposal with auto-apply off waits for review", () => {
    const state = conversationReducer(validating, { type: "prepared", riskLevel: "safe", autoApplySafe: false });
    expect(state).toEqual({ status: "awaitingReview", turn, riskLevel: "safe" });
    expect(conversationStatusLabel(state)).toBe("Needs your review");
  });

  it("a review proposal waits for review regardless of the setting", () => {
    for (const autoApplySafe of [true, false]) {
      expect(conversationReducer(submitted, { type: "prepared", riskLevel: "review", autoApplySafe })).toEqual({
        status: "awaitingReview",
        turn,
        riskLevel: "review",
      });
    }
  });

  it("review approval applies with reviewApproved and reject dismisses without applying", () => {
    const review = conversationReducer(validating, { type: "prepared", riskLevel: "review", autoApplySafe: true });
    const approved = conversationReducer(review, { type: "reviewApproved" });
    expect(approved).toEqual({ status: "working", phase: "applying", turn, reviewApproved: true });
    expect(conversationReducer(approved, { type: "applied" })).toMatchObject({ status: "applied", reviewApproved: true });

    expect(conversationReducer(review, { type: "rejected" })).toEqual({ status: "idle", note: "Dismissed" });
    expect(conversationReducer(submitted, { type: "reviewApproved" })).toBe(submitted);
    expect(conversationReducer(submitted, { type: "rejected" })).toBe(submitted);
  });

  it("validation issues fail as validationBlocked and keep the draft without retry", () => {
    const state = conversationReducer(validating, {
      type: "failed",
      kind: "validationBlocked",
      message: "Caption overlaps the next caption.",
    });
    expect(state).toEqual({
      status: "failed",
      turn,
      failure: { kind: "validationBlocked", message: "Caption overlaps the next caption.", retry: null },
    });
    expect(conversationStatusLabel(state)).toBe("Couldn't validate the edit");
    expect(state.status === "failed" ? state.turn.draft : null).toBe("Tighten the pacing");
  });

  it("apply failure fails as applyFailed with Retry that reapplies the same approval", () => {
    const approved = run(
      [
        { type: "prepared", riskLevel: "review", autoApplySafe: true },
        { type: "reviewApproved" },
        { type: "applyFailed", message: "The project changed on disk." },
      ],
      validating,
    );
    expect(approved).toEqual({
      status: "failed",
      turn,
      failure: { kind: "applyFailed", message: "The project changed on disk.", retry: { kind: "apply", reviewApproved: true } },
    });
    expect(conversationStatusLabel(approved)).toBe("Nothing was changed");
    expect(conversationReducer(approved, { type: "retry" })).toEqual({
      status: "working",
      phase: "applying",
      turn,
      reviewApproved: true,
    });
    expect(conversationReducer(submitted, { type: "applyFailed", message: "x" })).toBe(submitted);
  });

  it("turn failures resubmit the kept draft on retry, and agent unavailability has no retry", () => {
    const failed = conversationReducer(submitted, { type: "failed", kind: "turnFailed", message: "Timed out." });
    expect(failed).toMatchObject({ failure: { kind: "turnFailed", retry: { kind: "resubmit" } } });
    expect(conversationStatusLabel(failed)).toBe("The edit didn't finish");
    expect(conversationReducer(failed, { type: "retry" })).toEqual(submitted);

    const unavailable = conversationReducer(submitted, { type: "failed", kind: "agentUnavailable", message: "No app-server." });
    expect(unavailable).toMatchObject({ failure: { kind: "agentUnavailable", retry: null } });
    expect(conversationStatusLabel(unavailable)).toBe("Agent unavailable");
    expect(conversationReducer(unavailable, { type: "retry" })).toBe(unavailable);
    expect(conversationReducer(initialConversationState, { type: "retry" })).toBe(initialConversationState);
  });

  it("retains an unknown apply outcome without an apply retry", () => {
    const applying = conversationReducer(validating, { type: "prepared", riskLevel: "safe", autoApplySafe: true });
    const unknown = conversationReducer(applying, { type: "failed", kind: "outcomeUnknown", message: "The edit may have completed." });
    expect(unknown).toMatchObject({ status: "failed", turn, failure: { kind: "outcomeUnknown", retry: null } });
    expect(conversationStatusLabel(unknown)).toBe("Edit unconfirmed");
    expect(conversationReducer(unknown, { type: "retry" })).toBe(unknown);
  });

  it("undo success produces undone and the status reads Undone", () => {
    const applied = run([{ type: "prepared", riskLevel: "safe", autoApplySafe: true }, { type: "applied" }], validating);
    const undone = conversationReducer(applied, { type: "undone" });
    expect(undone).toEqual({ status: "undone", turn });
    expect(conversationStatusLabel(undone)).toBe("Undone");
    expect(conversationReducer(undone, { type: "undone" })).toBe(undone);
  });

  it("an undo conflict disables Undo with the exact reason", () => {
    const applied = run([{ type: "prepared", riskLevel: "safe", autoApplySafe: true }, { type: "applied" }], validating);
    const conflict = conversationReducer(applied, { type: "undoConflict" });
    expect(undoConflictReason).toBe("A newer edit changed the project.");
    expect(conflict).toEqual({
      status: "applied",
      turn,
      reviewApproved: false,
      undo: { available: false, reason: "A newer edit changed the project." },
    });
    expect(conversationStatusLabel(conflict)).toBe("Applied");
    expect(conversationReducer(conflict, { type: "undone" })).toBe(conflict);
    expect(conversationReducer(submitted, { type: "undoConflict" })).toBe(submitted);
  });

  it("cancel returns to idle with a Stopped note, but not once applying started", () => {
    const stopped = conversationReducer(validating, { type: "cancelled" });
    expect(stopped).toEqual({ status: "idle", note: "Stopped" });
    expect(conversationStatusLabel(stopped)).toBe("Stopped");
    expect(conversationReducer(submitted, { type: "cancelled" })).toEqual(stopped);

    const applying = conversationReducer(validating, { type: "prepared", riskLevel: "safe", autoApplySafe: true });
    expect(conversationReducer(applying, { type: "cancelled" })).toBe(applying);
    expect(conversationReducer(initialConversationState, { type: "cancelled" })).toBe(initialConversationState);
  });

  it("prepared and failed are ignored outside an in-flight turn", () => {
    const applying = conversationReducer(validating, { type: "prepared", riskLevel: "safe", autoApplySafe: true });
    expect(conversationReducer(applying, { type: "prepared", riskLevel: "safe", autoApplySafe: true })).toBe(applying);
    expect(conversationReducer(applying, { type: "failed", kind: "turnFailed", message: "x" })).toBe(applying);
    expect(conversationReducer(initialConversationState, { type: "applied" })).toBe(initialConversationState);
  });

  it("a new submission is allowed from every settled state", () => {
    const next = { type: "submit", messageId: "message-2", draft: "Add clean captions" } as const;
    const settled: ConversationState[] = [
      { status: "idle", note: "Stopped" },
      { status: "awaitingReview", turn, riskLevel: "review" },
      { status: "applied", turn, reviewApproved: false, undo: { available: true, reason: null } },
      { status: "undone", turn },
      { status: "failed", turn, failure: { kind: "validationBlocked", message: "x", retry: null } },
    ];
    for (const state of settled) {
      expect(isConversationBusy(state)).toBe(false);
      expect(conversationReducer(state, next)).toEqual({
        status: "working",
        phase: "reviewing",
        turn: { messageId: "message-2", draft: "Add clean captions" },
        reviewApproved: false,
      });
    }
  });
});
