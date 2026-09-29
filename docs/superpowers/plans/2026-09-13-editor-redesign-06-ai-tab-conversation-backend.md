# Editor Redesign 06 — AI Tab and Conversation Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Detail level:**
> - Tasks 1–4 are the approved VC-001 backend tasks and already have full detail in [the VC-001 plan](2026-07-25-codex-rail-proposal-workspace.md). Execute them from there with the amendments listed here.
> - Tasks 5–10 are task-level. Expand them into bite-sized TDD steps before executing.

**Goal:** Ship the AI tab as the director's primary editing surface, backed by the VC-001 conversation contract.
- **Input:** a preset-free conversation with hidden adaptive context.
- **Safety:** Rust validation, then fail-closed risk classification, then atomic batch apply, with snapshot Undo and conflict detection.
- **Results:** result cards with facts, real post-apply frames, Show changes, and Undo.
- **Review:** review cards for consequential work.
- **Controls:** the auto-apply setting, sessions, @mentions, a selection context chip, quick edits, variation-set results, and handoffs from other panels.

**Architecture:**
- **Rust.** `codex/conversation.rs` owns request and proposal types, context, preparation (validate, materialize, impact, risk), apply, and undo. It reuses:
  - `validate_codex_edit_proposal` / `materialize_codex_edit_proposal_actions`,
  - `apply_project_actions_to_split_project`,
  - the agent history in `project/split.rs`.
- **Store.** A new `agent` slice owns sessions, the message list, the state machine (`idle → working → validating → applied | awaitingReview | failed`), the pending proposal, and the auto-apply preference.
- **Client decision.** The client auto-applies only `safe` proposals while the setting is on. The Rust apply command independently refuses review-level bundles without `reviewApproved`.
- **Components.** They live under `src/editor/panels/ai/`.

**Tech Stack:** Rust 1.87, serde, the existing Codex app-server transport, Tauri 2.11, React 19, Zustand, Radix, Vitest, Playwright.

**Spec sections:** Left Tabs → AI, Backend Additions §1, Behavior Details (Undo scope), Acceptance flows 2–4.
**Depends on:** plans 01–05.

## Global Constraints

**Process**
- Prefix commands with `rtk`, use Conventional Commits, and stage only the named files.
- Keep files under 600 lines and use tokens only.

**Amendments to VC-001 Tasks 1–4**
- **Bridge calls.** Where VC-001 plan code calls `invoke(...)` in `src/lib/project.ts`, use `backendRequest(...)` from `src/lib/runtime/backend-client`. Direct Tauri imports outside adapters violate `scripts/runtime-boundary-policy.test.ts`.
- **Line numbers.** VC-001 file line references predate the 2026-09 redesign. Re-locate by symbol name.
- **Skip.** Skip VC-001 Tasks 5–8, the React rail. Tasks 5–10 below replace them.
- **Risk allowlist.** Keep the VC-001 fail-closed allowlist unchanged, except for one addition: transition actions from plan 08 are safe. Plan 08 adds them; do not pre-add them here.

**What the conversation UI never renders**
- internal IDs,
- adaptive context payloads,
- token budgets,
- a hard-coded default prompt.

**What the conversation UI may render**
- **Mentions.** Each renders once, inline in the user's message, by human name.
- **Selection context chip.** One removable chip, by human name. This is a spec-approved change from VC-001.

**Status and focus**
- Status uses text, not color alone.
- Review cards take focus only when user action is required. Applied results never steal focus from the composer.

---

## File Map

### Rust (from VC-001 Tasks 1–4)

**Create**
- `src-tauri/src/codex/conversation.rs` — request/proposal types, context, preparation, risk, impact.
- `src-tauri/tests/codex_conversation.rs`

**Modify**
- `src-tauri/src/codex/mod.rs`
- `src-tauri/src/codex/context.rs` — focus-first bounded context.
- `src-tauri/src/codex/app_server.rs` — conversation turn and schema.
- `src-tauri/src/project/split.rs` — `apply_agent_project_action_batch`, and forward-compatible app-server history.
- `src-tauri/src/main.rs` — commands, registered in the `invoke_handler` list near L6787:
  - `start_codex_conversation_edit_for_project`
  - `apply_codex_conversation_proposal`
  - `undo_latest_codex_conversation_edit`
- `src-tauri/tests/project_split.rs`

**Additional Rust work in this plan (Task 4b)**
- Register `capture_canonical_preview_frame_in_split_project_folder` in TS. The command already exists at `main.rs:1700`.
- Add `cancel_codex_conversation_edit_for_project`, reusing the cancellation mechanism of `cancel_codex_video_edit_for_project`.

### TypeScript bridge
- `src/lib/project.ts`
  - VC-001 types and wrappers.
  - `captureCanonicalPreviewFrameInSplitProjectFolder(input): Promise<PreparedPreviewFrameResult>`. Export the existing interface.
  - `cancelCodexConversationEditForProject`.
- `src/lib/project.test.ts`

### Pure modules
| File | Contents |
|---|---|
| `src/lib/agent/conversation-state.ts` | Pure state machine: `conversationReducer(state, event)`. Events: `submit`, `phase`, `prepared`, `applied`, `applyFailed`, `reviewApproved`, `rejected`, `undone`, `undoConflict`, `cancelled`, `failed`. |
| `src/lib/agent/result-facts.ts` | `resultFacts(impact, actions, beforeProject, afterProject)`. Returns fact chips such as `"2:10 → 0:45"`, `"14 cuts"`, `"38 captions"`, `"1 title"`, `"3 × video · 6s"`, `"≈ $1.20"`, `"Replaces nothing"`. Also `reviewPlacements(actions, project)`, plain-language placement lines. |
| `src/lib/agent/quick-edits.ts` | Quick edit suggestions: Tighten the pacing, Remove dead air, Add clean captions, Balance the audio, Make a shorter cut. |
| `src/lib/agent/mentions.ts` | Mention token parse and serialize. The draft shows `@<human name>`; the request carries `focus.mediaIds` / `timelineItemIds` resolved from `agentMentionTargets`. `resolveMentions(draft, targets)` returns `{ focus, unresolved: string[] }`. |

### Store
- `src/editor/store/agent-slice.ts` and `agent-slice.test.ts` — sessions, messages, conversation state, `autoApplySafe` (persisted under `video-creater.editor.v2.agent.autoApplySafe`, default `true`), `pendingAgentRequest` consumption.

### Components (`src/editor/panels/ai/`)
| File | Contents |
|---|---|
| `ai-panel.tsx` | Composes the pieces below |
| `ai-header.tsx` | Session dropdown (switch, rename dialog, delete confirm, restore), new chat, history, auto-apply switch |
| `message-list.tsx` | User and assistant messages, progress row, cards |
| `progress-row.tsx` | One current phase; optional Details disclosure |
| `applied-card.tsx` | Applied status, summary, facts, post-apply frames, Show changes, Undo (disabled with reason on conflict) |
| `review-card.tsx` | Needs-review status, summary, facts, placements, primary action, Revise/Edit prompts, Dismiss |
| `failure-card.tsx` | Failure summary, next step, Retry when safe |
| `variation-result-card.tsx` | Thumbnails with "Use this" per variant |
| `result-frames.tsx` | Frame strip from `captureCanonicalPreviewFrameInSplitProjectFolder`, with fallback "Open viewer · Retry preview" |
| `composer.tsx` | Draft textarea, mention picker, context chip, attach, send/stop |
| `mention-picker.tsx` | Popover listing `agentMentionTargets` by human name and kind |
| `quick-edits.tsx` | Suggestion list |
| `missing-agent-state.tsx` | Shown when the Codex app-server is unavailable; "Open Agent settings" |

### Modified
- `src/editor/shell/left-panel.tsx`, `mobile-layout.tsx` — AI tab and tall sheet.
- `src/editor/timeline/timeline-panel.tsx` — render `highlightedItemIds` and `highlightedRanges`.
- `knip.jsonc`

---

### Task 1: Preset-free conversation contract (VC-001 Task 1)

- [ ] Execute [VC-001 Task 1](2026-07-25-codex-rail-proposal-workspace.md#task-1-add-a-preset-free-codex-conversation-contract) with the amendments above.
- [ ] Commit using VC-001's commit message.

### Task 2: Adaptive focus context stays internal (VC-001 Task 2)

- [ ] Execute VC-001 Task 2.
- [ ] Add one amendment test: a focus built from the redesign's selection chip, where the timeline item IDs come from `selectedItemIds`, ranks those items first. Assert the returned presentation model contains no context payload.
- [ ] Commit.

### Task 3: Prepared actions, impact and risk (VC-001 Task 3)

- [ ] Execute VC-001 Task 3.
- [ ] Commit.

### Task 4: Atomic batch apply and undo (VC-001 Task 4)

- [ ] Execute VC-001 Task 4.
- [ ] Commit.

### Task 4b: Frame capture wrapper and conversation cancel

- [ ] **Rust test** in `src-tauri/tests/codex_conversation.rs`: cancelling an in-flight conversation turn returns a cancelled result, and the project is unchanged.
- [ ] **Implement** `cancel_codex_conversation_edit_for_project`, mirroring `cancel_codex_video_edit_for_project` (locate it in `main.rs` and `codex/app_server.rs`). Register it.
- [ ] **TypeScript wrappers and tests** (`src/lib/project.test.ts`, with a mocked `backendRequest`):
  - `captureCanonicalPreviewFrameInSplitProjectFolder({ projectDir, timelineId?, atSeconds, width, height })`. Match the Rust command's argument names by reading its signature at `main.rs:1700`.
  - `cancelCodexConversationEditForProject({ projectDir })`.
- [ ] **Verify:**
  - `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation -- --test-threads=1`
  - `rtk pnpm vitest run src/lib/project.test.ts`
- [ ] **Commit:** `feat(codex): expose conversation cancel and canonical frame capture to the editor`

### Task 5: Pure conversation state, facts, mentions, quick edits

- [ ] **`conversation-state.test.ts`**, covering every transition:
  - A safe proposal with auto-apply on goes `prepared` → `working(apply)` → `applied`.
  - A safe proposal with auto-apply off goes `prepared` → `awaitingReview`.
  - A review proposal goes to `awaitingReview` regardless of the setting.
  - Validation issues produce `failed(validationBlocked)`, keeping the draft.
  - Apply failure produces `failed(applyFailed)`, with Retry allowed.
  - Undo success produces `undone`, and the card shows "Undone".
  - An undo conflict disables Undo with the reason "A newer edit changed the project."
  - Cancel produces `idle`, with a "Stopped" note.
- [ ] **`result-facts.test.ts`:**
  - Duration fact formatting.
  - Counts per action type: `splitItems` and `rippleDeleteRanges` count as cuts, caption `addItems` as captions, and template overlays as titles.
  - Generation facts from `recordGeneratedAsset` actions: count, kind, duration, provider label, and cost via `selectedGenerationCost`.
  - "Replaces nothing" when no `removeItems` or `replaceTimelineItemWithGeneratedOutput` action is present.
  - Placement lines use human names and timecodes.
- [ ] **`mentions.test.ts`:**
  - Parse `@input.mp4` and multi-word names in quotes: `@"archive broll.mp4"`.
  - A missing name produces `unresolved`.
  - Duplicates are deduplicated.
  - Serializing produces a focus with IDs, and the visible text keeps the names.
- [ ] **Commit:** `feat(agent): add conversation state machine, result facts, and mention resolution`

### Task 6: Agent store slice

- [ ] **Sessions.** On editor mount, load them with `loadAgentSessionsFromSplitProjectFolder`. Create, select, rename, delete and restore go through `applyAgentSessionActionToSplitProjectFolder`. The message history comes from `loadAppServerConversationHistoryFromSplitProjectFolder` for the active session.
- [ ] **`submit(draft)` flow:**
  1. Resolve mentions. If any are unresolved, set an inline composer error and do not send.
  2. Build the focus: mentions, plus the selection chip if it is present.
  3. Append the user message and set the phase to "Reviewing the timeline".
  4. Call `startCodexConversationEditForProject`.
  5. On `preparedProposal`, run the reducer. If the result is auto-apply, call `applyCodexConversationProposal({ reviewApproved: false })`.
  6. On apply success, call `project.replaceProject(result.project)` with no local history entry. Record `undoIdentity` on the message.
- [ ] **Other actions:**
  - `approve(messageId)` applies with `reviewApproved: true`.
  - `reject(messageId)` makes no backend call and marks the card dismissed.
  - `undo(messageId)` calls `undoLatestCodexConversationEdit`. Success replaces the project; a conflict disables Undo with the reason.
  - `cancel()` stops the in-flight turn.
  - `setAutoApplySafe(boolean)`.
  - `consumePendingRequest()` handles requests from "Organize with AI" and "Ask AI about this clip". It fills the draft and focus chip without sending, and switches to the AI tab.
- [ ] **Global Undo interaction.** Agent batches are not in the local snapshot history. Global ⌘Z first checks whether the latest mutation was an agent batch (the `lastMutationSource` field): if so, it calls `agent.undo` for that message; otherwise it calls `project.undo`. Add `lastMutationSource: "user" | "agent"` to the project slice, set by `applyActions` and by agent apply.
- [ ] **Tests** (mock `backendRequest`):
  - A safe auto-apply replaces the project once.
  - A review proposal leaves the project unchanged until approve.
  - Reject leaves the project unchanged.
  - An unresolved mention blocks send.
  - Global undo after an agent apply routes to the agent undo.
  - The conflict path works.
  - The auto-apply preference persists.
- [ ] **Commit:** `feat(agent): add the agent store slice with safe auto-apply and batch undo`

### Task 7: AI panel components

- [ ] **Header.** Session dropdown and "New chat". Rename uses a Dialog; delete uses a confirm dialog naming the session. History toggles the archived sessions list. The "Auto-apply safe edits" switch has the tooltip "Paid, generative, export, and broad changes always wait for review."
- [ ] **Message list.**
  - The user message shows mentions inline as styled spans, by human name only.
  - The assistant reply text comes from `proposal.summary`.
  - The progress row shows one phase: "Reviewing the timeline", "Validating the edit", "Applying changes" or "Preparing the preview". The Details disclosure shows raw diagnostics text only when present.
- [ ] **Applied card:**
  - The "Applied to <timeline name>" status.
  - Facts.
  - `result-frames`: up to 4 frames at the impact preview timestamp and at the starts of the first three affected ranges, captured at 160×90 (with an image-cache key of revision plus time). On failure, show "Open viewer" and "Retry preview", without changing the status.
  - **Show changes** calls `selection.setHighlights(affectedItemIds, affectedRanges)`, then `seek(previewTimestamp)`, scrolls the timeline to the first range, and on mobile closes the AI sheet.
  - Clicking a frame seeks and plays.
  - **Undo**. When the preview is stale, meaning the project revision is newer than the result revision, the frames get an "Earlier version" label.
- [ ] **Review card.** Status "Needs your review", summary, facts, and placement lines. The primary action label comes from the proposal kind: "Generate & place" when it contains generation, otherwise "Apply". It also has "Revise" (prefills the composer with "Revise: ") and "Dismiss". Focus moves to the card's primary button when it appears.
- [ ] **Failure card.** Uses `lastError` copy mapping:
  - Validation blocked: "The edit couldn't be validated: <first issue message>. Try rephrasing or narrowing the request."
  - Apply failed: "Nothing was changed. <reason>" with Retry.
  - Agent unavailable: `missing-agent-state` with "Open Agent settings", which calls `onOpenSettings`, using the settings target for Agent & MCP from `lib/settings/target.ts`.
- [ ] **Composer:**
  - The textarea starts empty.
  - Enter sends; Shift+Enter inserts a newline.
  - `@` opens the mention picker (arrow keys, Enter).
  - When a selection exists, a context chip "<clip or media name> ×" appears; removing it excludes the selection from the focus for the next send.
  - The attach button opens the Media tab picker in "attach" mode, which adds a mention.
  - While a turn is running, Send becomes Stop.
  - Quick edits show only in an empty conversation and fill the draft without sending.
- [ ] **Variation result card.** Shown when the applied or approved actions produced a variation set: thumbnails, and "Use this" commits `replaceTimelineItemWithGeneratedOutput` through the project slice as a user edit.
- [ ] **Tests** for each component: accessible names, focus rules (applied never steals focus; review focuses its primary action), stale frame label, Undo disabled reason, chip removal, quick edits hidden after the first message, and that no internal ID text renders for the fixture proposal (a regex of media IDs against the rendered text).
- [ ] **Commit:** `feat(agent): add the AI tab conversation, result, and review cards`

### Task 8: Timeline highlights and handoffs

- [ ] **`timeline-panel.tsx`.** Highlighted items get an accent outline. Highlighted ranges draw a translucent `--accent-soft` band across their tracks. Highlights clear on the next user edit or on Esc.
- [ ] **Handoffs:**
  - The media "Organize with AI" button and the clip "Ask AI about this clip" menu item switch to the AI tab (or open the AI sheet on mobile) and call `consumePendingRequest`.
  - "Organize with AI" pre-fills its prompt and, with confirmation, auto-sends.
  - "Ask AI" only fills the chip and focuses the composer.
- [ ] **Tests:** Show changes highlights the correct items and ranges; the two handoffs.
- [ ] **Commit:** `feat(agent): highlight agent changes on the timeline and accept panel handoffs`

### Task 9: Fixture handlers for conversation flows

- [ ] **Location.** Add deterministic handlers in `src/lib/runtime/fixtures/conversation-fixtures.ts`, registered in `fixture-bootstrap.ts` when `marker.conversationFixture === true`. Extend `EditorFixtureRuntimeMarker`.
- [ ] **`start_codex_conversation_edit_for_project`.** Picks the proposal by keyword, matched case-insensitively:
  - **"tighten":** a safe proposal of `splitItems` plus `rippleDeleteRanges` on the sample timeline, with an impact computed via `applyProjectActionsLocally` and real duration facts.
  - **"generate":** a review proposal of `recordGeneratedAsset` × 3 plus a placeholder.
  - **"fail":** validation issues.
  - **Anything else:** a safe proposal of a caption text edit.
- [ ] **Stateful handlers.** They share an in-memory fixture project:
  - `apply_codex_conversation_proposal` applies locally and refuses review-level bundles without approval.
  - `undo_latest_codex_conversation_edit` restores the saved before-state and reports a conflict if the project changed since.
- [ ] **`capture_canonical_preview_frame_in_split_project_folder`.** Returns a data URL of a 160×90 solid frame, or a fixture PNG path served via the fixture media route.
- [ ] **Sessions.** `load_agent_sessions_from_split_project_folder` and `apply_agent_session_action_to_split_project_folder` use an in-memory manifest.
- [ ] **Tests:** in `conversation-fixtures.test.ts`, one per handler, covering the keyword routing and the refusal of an unapproved review.
- [ ] **Commit:** `test(agent): add deterministic conversation fixture handlers`

### Task 10: Integration, knip, e2e

- [ ] **Wire the tab.** Route the AI tab (desktop) and the AI tall sheet (mobile) to `AiPanel`, and remove the AI placeholder.
- [ ] **knip.** Remove the consumed entries from the temporary `knip.jsonc` block.
- [ ] **`e2e/editor-ai.spec.ts`**, with the fixture marker `conversationFixture: true`. Each flow runs at desktop 1440×900 and at phone 402×874:
  1. **Auto-apply on:** send "Tighten the pacing".
     - The Applied card shows a duration fact.
     - Show changes highlights at least one clip.
     - Undo returns the clip count to its original value.
  2. **Review:** send "Generate three lab shots".
     - The Needs review card focuses "Generate & place".
     - The project clip count is unchanged.
     - Dismiss leaves it unchanged.
  3. **Auto-apply off:** turn the switch off, then send "Tighten the pacing". A review card appears first; Apply produces the Applied card.
  4. **Failure:** send "fail please". The failure card shows the validation copy and the draft is preserved.
  5. **Hygiene:**
     - No text matching `/media-\d+/` inside the AI panel.
     - No browser console errors.
- [ ] **Visual check.** Capture empty, working, applied, review and failure states at 1440×900 and 402×874 into `output/editor-ai/`. Read the PNGs.
- [ ] **Native check (recorded, not gating).** In `rtk pnpm dev` with a real project and an installed Codex app-server, run flow 1. Record in the commit body:
  - whether frames match the viewer,
  - whether Show changes highlights the correct ranges,
  - whether Undo restores the project.
- [ ] **Gates:**
  - `rtk pnpm verify:frontend`
  - `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation --test project_split -- --test-threads=1`
- [ ] **Commit:** `test(agent): cover auto-apply, review, and failure conversation flows`

## Acceptance

- VC-001 acceptance holds:
  - no hidden prompt, preset, context strip, or internal ID;
  - safe actions apply atomically;
  - risky, external, expensive, or unknown actions stop for review;
  - applied results show accurate facts, real frames, timeline navigation, and Undo;
  - failures never partially mutate.
- The auto-apply setting behaves as specified, and the Rust apply command refuses unapproved review bundles independently.
- Spec acceptance flows 2–4 pass at both viewports.
- Update the product backlog status for VC-001 in plan 09's docs task.
