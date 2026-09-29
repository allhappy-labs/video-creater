# Codex Rail Proposal Workspace Design

**Date:** 2026-07-25  
**Status:** Approved design, ready for implementation planning

## Summary

Redesign the Codex rail as a focused editing conversation. The rail should show
only what the user intentionally says, the current work state, and the result or
decision that needs attention.

Project context remains adaptive and internal. Small projects may send the full
canonical project context; large projects may send a relevant bounded subset.
That payload is not repeated in the interface. Mentions are visible only where
the user typed them in the prompt.

Validated, local, reversible edits apply immediately and produce an accurate
post-apply result card with a preview frame, change navigation, and Undo.
Destructive, external, expensive, or otherwise uncertain edits stop at a compact
review card before mutation.

## Problem

The current Codex panel exposes implementation mechanics instead of supporting
an editing conversation:

- The composer starts with a cinematic-trailer prompt the user did not request.
- The panel displays an internal media identifier such as `@media-1` and helper
  text asking the user to repeat it.
- Context can be represented more than once even though it is already supplied
  to Codex internally or written directly in the prompt.
- Proposal review is details-heavy and makes ordinary edits feel like a form.
- A result does not immediately communicate what changed, where it changed, or
  how to recover.

This creates two competing sources of intent: what the user wrote and what the
panel prefilled or surfaced on their behalf.

## Goals

- Keep the visible conversation faithful to the user's words.
- Preserve the existing size-aware project-context behavior without exposing
  its transport payload in the UI.
- Help a first-time user begin with concise quick-edit suggestions only in a
  genuinely empty conversation.
- Make ordinary reversible edits fast while preserving Rust validation and
  canonical project ownership.
- Make each completed edit understandable through an accurate result summary,
  a real post-apply frame, timeline navigation, and Undo.
- Stop risky work before mutation with a short, actionable review.
- Keep the rail visually quiet and useful at desktop and narrow widths.

## Non-goals

- Adding a reusable prompt-template system.
- Showing selected-context strips, resolved-mention cards, context summaries,
  token budgets, retrieval chunks, or internal media IDs.
- Adding a second playable video surface inside the conversation.
- Rendering a draft video for every chat edit.
- Replacing the existing adaptive project-context selection algorithm.
- Allowing Codex to mutate canonical project files directly.
- Redesigning the main viewer or timeline beyond the navigation hooks needed by
  the result card.

## Product Principles

### One visible source of intent

The submitted message is the visible request. A mention appears inline in that
message and nowhere else. The composer has no default creative direction,
hidden preset, or automatically inserted prompt.

### Context is infrastructure

Context selection exists to help the model make a correct proposal. It is not a
second user-facing artifact. The UI must not render the adaptive context
envelope or create synthetic chat messages from it.

### Validation precedes mutation

Codex returns structured project actions. Rust validates and normalizes those
actions before risk classification and before any canonical project mutation.
A failed validation never partially applies.

### Preview the truth

The chat preview frame is derived from the actual changed project, not a mock
interpretation of a pending proposal. Playback remains in the existing main
viewer so transport, playhead, and render state have one owner.

### Recovery is part of the result

Every immediately applied edit must be atomic and snapshot-reversible. Undo is a
primary action on the result card, not buried in proposal details.

## Conversation States

### 1. Empty conversation

Show:

- An empty composer.
- A small "Quick edits" group with short actions such as:
  - Tighten the pacing
  - Remove dead air
  - Add clean captions
  - Balance the audio
  - Make a shorter cut

Selecting a quick edit fills the composer. It does not submit automatically, so
the user can refine it, add a mention, or discard it.

The quick-edit group is shown only when the conversation has no submitted user
message and no prior assistant result. It disappears permanently for that
conversation after the first submission. These actions are local starter
suggestions, not stored prompt templates or a hidden preset system.

### 2. Working

After submission, append the user's message and show one compact progress row.
The row may expose a secondary **Details** disclosure for useful diagnostics.
Do not create a stack of simulated tool cards or echo the internal context
payload into the transcript.

The progress copy should describe a real phase, for example:

- Reviewing the timeline
- Validating the edit
- Applying changes
- Preparing the preview

Only one phase is presented as current at a time.

### 3. Safe edit applied

After validation, risk classification, and an atomic apply, show a compact
result card containing:

- A clear **Applied** status.
- A concise human summary, for example "Removed 26 seconds of silence across
  four gaps."
- A small before/after fact when it materially helps, such as
  `03:42 -> 03:16`.
- One preview frame derived from the changed project.
- **Show changes** and **Undo** actions.

Selecting the preview frame focuses the main viewer on the first meaningful
affected time and begins playback there. The frame itself is not an inline
player.

**Show changes** focuses the timeline and highlights the affected item IDs and
time ranges returned by the apply result. It must not infer affected ranges
later from prose.

**Undo** restores the edit's captured project snapshot. It remains available
only while no later conflicting project mutation has occurred. When invalid,
the action stays visible but disabled with a concise reason.

### 4. Risky edit awaiting review

Validated risky work does not mutate the project. Show a review card with:

- A one-line summary of the requested outcome.
- A short impact list based on the normalized action diff.
- **Apply** and **Reject** actions.
- A conversational path to request a safer or narrower revision.

Do not show a fabricated pre-apply playback preview. If the user applies the
proposal, use the same atomic apply and post-apply result flow as a safe edit.

## Composer and Mentions

- The initial composer value is empty.
- Remove the hard-coded cinematic-trailer prompt and preset coupling.
- Do not show a standalone `@media-1` badge or "Mention @media-1" helper.
- A mention is rendered only as part of the user's draft or submitted message.
- The mention label should use the source's human title, not an internal ID.
- Mention resolution may attach canonical IDs to the submitted request
  internally, but those IDs are not rendered.
- A mention narrows or emphasizes the requested target. It does not disable the
  broader adaptive project context needed to reason correctly about the edit.
- If a source can no longer be resolved, keep the visible text and present a
  clear inline resolution error before submission.

## Adaptive Context Contract

The request builder creates an internal context envelope after the user submits:

1. The user's exact prompt and resolved mention references.
2. The canonical project description required by the edit workflow.
3. Full project context when the project is within the existing size budget.
4. A relevance-ranked bounded subset when it exceeds that budget.
5. The mandatory video-pipeline and graphics guidance required by the request.

The backend owns the size decision. The client may show a generic working state
but must not receive a presentation model containing context chips, selected
objects, retrieval snippets, or token-budget details.

Mentions influence relevance ranking but do not become the sole project context.
The structured proposal must still be grounded in canonical source ranges and
build a real EDL before captions, overlays, titles, effects, or generated visual
layers are added.

## Deterministic Risk Policy

Risk classification runs in trusted Rust code over validated, normalized project
actions and their computed impact. Codex-provided risk labels may be displayed
as explanation but are not authoritative.

An edit may auto-apply only when all of the following are true:

- Every normalized action belongs to the safe local-action allowlist.
- The complete action set can apply atomically.
- A restorable project snapshot can be captured before mutation.
- The edit has no external side effect.
- The computed impact contains none of the force-review conditions below.

The safe allowlist covers ordinary reversible editor operations:

- Trimming, splitting, moving, and ripple gap removal.
- Local clip volume and mix adjustments.
- Creating or updating captions.
- Adding or updating local timeline layers, transitions, and effects.
- Other explicitly registered local actions with equivalent atomic Undo support.

Any of the following forces review:

- Replacing, clearing, or rebuilding the full timeline.
- Deleting an entire pre-existing source, media asset, track, or unrelated
  timeline item.
- Destructively changing project, source-media, or export settings.
- Starting an upload, provider generation, network job, render, or export.
- Applying an action that cannot participate in the same atomic snapshot.
- Encountering an unregistered or unknown action kind.
- Producing impact outside the validated proposal's declared target items and
  time ranges.

This is an allowlist with fail-closed behavior, not a confidence score or a
model-selected threshold. "Remove dead air" can auto-apply when it normalizes to
registered trim, split, and ripple-gap actions with atomic Undo; "replace the
full timeline" always requires review.

## Data Flow

1. The user submits a prompt, optionally containing human-readable mentions.
2. The request builder resolves mentions and assembles the invisible adaptive
   project context.
3. Codex returns a structured edit proposal; it does not write project files.
4. Rust validates the proposal and canonical source ranges.
5. The app materializes normalized project actions and computes an impact diff.
6. Trusted Rust code classifies the action set:
   - Safe: capture snapshot and apply atomically.
   - Risky or unknown: retain the proposal without mutation and show review.
7. After an apply, the app returns a result model with:
   - A concise before/after summary.
   - Affected canonical item IDs and time ranges.
   - The resulting project revision and Undo identity.
   - A meaningful preview timestamp.
8. The preview subsystem obtains a frame from the canonical changed revision at
   that timestamp.
9. Chat actions navigate the existing viewer and timeline using the returned
   identifiers.

## UI Responsibilities

The implementation should separate the rail into focused components rather than
continue growing a single proposal form:

- `CodexConversationRail`: transcript and state orchestration.
- `CodexEmptyQuickEdits`: empty-chat-only suggestion buttons.
- `CodexComposer`: prompt entry and inline mention editing.
- `CodexProgressRow`: one current phase plus optional details.
- `CodexAppliedEditCard`: summary, frame, navigation, and Undo.
- `CodexRiskReviewCard`: impact, Apply, Reject, and revision affordance.

These names are architectural guidance, not a requirement to create one file per
component. State must remain explicit:

`idle -> working -> validating -> applied | awaiting_review | failed`

Applying an awaiting-review proposal returns to `working` and then reaches
`applied` or `failed`.

## Result and Navigation Contract

The apply result should expose stable data rather than force the UI to parse
assistant prose:

- `projectRevision`
- `undoIdentity`
- `summary`
- `beforeFacts`
- `afterFacts`
- `affectedItemIds`
- `affectedRanges`
- `previewTimestamp`
- `previewFrameState`

The exact Rust and TypeScript shapes can follow repository conventions. The
contract must preserve canonical IDs internally while rendering human names.

The preview frame represents the post-apply revision. If the preview becomes
stale because the project changes again, the card must not silently present it
as current; mark it as belonging to the earlier result or refresh it against the
new revision.

## Failure and Recovery

### Validation blocked

Show what prevented validation and, when known, the smallest corrective action.
No project mutation occurs. Keep the user's request available to revise or
retry.

### Apply failed

Atomic apply failure leaves the canonical project unchanged. Keep the validated
proposal and offer Retry when safe.

### Preview frame failed

Keep the successful result summary, **Show changes**, and **Undo**. Replace the
frame with **Open viewer** and **Retry preview**. A frame failure must not
misrepresent the edit as failed.

### Undo conflict

If a later project mutation invalidates the stored snapshot, disable **Undo**
and explain that a newer edit has changed the project. Do not attempt a partial
or best-effort rollback.

### External or render job failed

Because these actions require review before starting, report their job failure
without rolling back unrelated local project state. Retrying the job remains a
separate explicit action.

## Accessibility and Responsive Behavior

- Quick edits, preview frames, disclosures, Apply, Reject, Show changes, and
  Undo are keyboard reachable and have explicit accessible names.
- Status is conveyed by text, not color alone.
- Focus moves predictably to a newly inserted review card only when user action
  is required; routine applied results do not steal focus from the composer.
- At narrow widths, cards stack their facts and actions without horizontal
  scrolling.
- Preview frames maintain a stable aspect ratio and useful alternative text.
- Disabled Undo exposes its reason to keyboard and assistive-technology users.

## Test and Acceptance Strategy

### Component and state tests

- The composer initializes empty and contains no trailer prompt or selected
  preset.
- No `@media-1` badge, helper, context strip, or duplicate mention is rendered.
- Quick edits appear only in a conversation with no prior messages.
- Selecting a quick edit fills the composer without submitting.
- A submitted mention appears once, inline in the user's message.
- Working state renders one progress row.
- A safe validated action auto-applies and creates an Undo-capable result.
- A force-review or unknown action does not mutate before Apply.
- Unknown action types fail closed to review.
- Reject leaves the project unchanged.
- Apply failure is atomic.
- Undo invalidates after a conflicting later mutation and explains why.

### Integration tests

- Small projects send full canonical context; projects over the configured
  budget use the bounded selection path.
- Both paths keep transport context out of the presentation model.
- Mention references influence context selection while broader project context
  remains available.
- Rust validation happens before risk classification and mutation.
- Safe dead-air removal yields registered trim/split/ripple actions and an
  accurate before/after duration.
- Full-timeline replacement always reaches review.
- Result item IDs and ranges drive timeline highlighting.
- Selecting a result frame focuses and plays the main viewer at the returned
  post-apply timestamp.
- Preview failure does not remove Undo or change the applied result status.

### Visual and manual acceptance

- Capture empty, working, safe-applied, risky-review, and failure states at
  desktop and narrow rail widths.
- Verify the complete conversation flow in the real app, not only isolated
  Storybook or component fixtures.
- Confirm in the packaged Tauri app that the frame matches the changed project,
  the main viewer opens at the intended moment, Show changes highlights the
  correct timeline ranges, and Undo restores the prior project.
- Keep browser visual scenarios for fast regression coverage, while treating
  native Tauri evidence as the acceptance proof for viewer and timeline
  integration.

## Rollout Boundary

Implement this as a replacement of the current Codex prompt/proposal surface,
not as a second optional mode. Existing validation, project-action
materialization, canonical apply, preview, and agent-specific Undo paths should
be reused where they satisfy this contract.

The implementation plan should identify and remove the default prompt,
`media-1` presentation, preset coupling, duplicate context UI, and
details-heavy proposal review. It should then layer the new conversation states
over the existing trusted project-action pipeline.
