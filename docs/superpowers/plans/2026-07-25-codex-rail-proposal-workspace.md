# Codex Rail Proposal Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the preset-driven Codex form with a focused project-edit conversation that keeps context invisible, auto-applies only Rust-classified safe edits, reviews risky edits before mutation, and shows an accurate reversible result.

**Architecture:** Add a preset-free Codex conversation request and proposal path beside the existing one-click generated-cut path. Rust builds the hidden adaptive context, validates and materializes the exact action bundle, computes impact, classifies risk, and atomically records applied batches for conflict-aware Undo. React presents that trusted state through focused conversation components and reuses the existing timeline compositor, main viewer, and timeline selection systems for post-apply preview and navigation.

**Tech Stack:** React 19, TypeScript 5.7, Vitest, Testing Library, Tailwind CSS, shadcn primitives, Tauri 2.11, Rust 1.87, serde, existing split-project storage and timeline compositor.

## Global Constraints

- The composer starts empty and sends no hidden preset or default creative prompt.
- Quick edits appear only before the first submitted message and fill the composer without submitting.
- Project context, selected items/ranges, retrieval truncation, canonical IDs, and token details never render in the conversation.
- Mentions render once, inline, using a human-readable label; canonical IDs remain transport-only.
- Small projects send complete canonical context; oversized projects send a focus-first bounded subset.
- Codex proposes structured actions; Rust validates and materializes them before mutation.
- A generated primary cut remains EDL-first; direct edits to an existing timeline do not invent a replacement EDL.
- Risk classification is a Rust allowlist with fail-closed review for destructive, external, expensive, or unclassified actions.
- Safe edits apply atomically and record a restorable batch; risky edits do not mutate before explicit Apply.
- The chat preview is a static frame from the changed project using the existing compositor; playback remains in the main viewer.
- Do not add a prompt-template system, a second playable video surface, or per-edit draft-video rendering.
- Preserve the preset-based `EditJobRequest` and one-click edit workflow for non-conversation callers.
- Use `lucide-react`, shadcn primitives, compact editor density, accessible names, visible focus, and reduced-motion-safe behavior.
- Prefix every repository shell command with `rtk`.
- Use Conventional Commits and stage only the files named by each task.

---

## File Map

### New files

- `src-tauri/src/codex/conversation.rs` — preset-free request/proposal types, validation, materialization, impact diff, and risk classification.
- `src-tauri/tests/codex_conversation.rs` — public-contract tests for safe, review, validation, and context behavior.
- `src/lib/codex-conversation.ts` — frontend request, prepared proposal, impact, result, and transcript types.
- `src/lib/codex-conversation.test.ts` — request focus, mention token, quick-edit, and transcript persistence tests.
- `src/components/workspace/codex-conversation-cards.tsx` — progress, applied-result, risky-review, and failure components.
- `src/components/workspace/codex-conversation-cards.test.tsx` — component behavior and accessibility tests.

### Modified files

- `src-tauri/src/codex/mod.rs` — export the conversation module.
- `src-tauri/src/codex/context.rs` — build focus-first bounded context without exposing it to the UI.
- `src-tauri/src/codex/app_server.rs` — add the conversation turn/schema beside the generated-cut turn.
- `src-tauri/src/project/split.rs` — atomically apply a prepared agent batch, update session outcome, and expose conflict-aware Undo.
- `src-tauri/src/main.rs` — expose conversation start/apply/undo Tauri commands.
- `src/lib/project.ts` — add Tauri bridge types and wrappers; loosen stored app-server request typing for legacy and conversation requests.
- `src/components/workspace/agent-panel.tsx` — replace default context/tool chrome and direct prompt dispatch with the focused conversation presentation.
- `src/components/workspace/agent-panel.test.tsx` — replace obsolete prompt/context expectations and retain mention/composer regression coverage.
- `src/components/workspace/codex-proposal-review.tsx` — retain generated-cut model helpers but stop using the large review surface in the conversation rail.
- `src/components/workspace/editor-workspace.tsx` — orchestrate start, safe auto-apply, risky review, results, Undo, viewer focus, and timeline highlighting.
- `src/components/workspace/editor-workspace.test.tsx` — cover end-to-end rail state transitions and navigation callbacks.
- `src/components/workspace/timeline-preview-compositor.tsx` — add a non-interactive static-frame mode.
- `src/components/workspace/timeline-preview-compositor.test.tsx` — prove static mode cannot edit or play.
- `src/components/workspace/timeline-editor.tsx` — render passive affected-range highlights.
- `src/components/workspace/timeline-editor.test.tsx` — verify multiple result ranges and selected affected items.
- `src/lib/modern-editor-visual-qa-fixtures.ts` — deterministic empty, working, applied, review, and failed rail states.
- `scripts/browser-visual-qa.mjs` — capture the new Codex rail scenarios.
- `src/browser-visual-qa-palmier-scenarios.test.ts` — require the new scenario matrix.

---

### Task 1: Add a preset-free Codex conversation contract

**Files:**
- Create: `src-tauri/src/codex/conversation.rs`
- Modify: `src-tauri/src/codex/mod.rs`
- Modify: `src-tauri/src/codex/app_server.rs:1-435,453-1030`
- Modify: `src-tauri/src/project/split.rs:575-645,2807-2910`
- Modify: `src/lib/project.ts:485-575,3132-3160,3638-3650`
- Create: `src-tauri/tests/codex_conversation.rs`
- Test: `src/lib/project.test.ts:2260-2350`

**Interfaces:**
- Consumes: existing `ProjectAction`, `VideoProject`, `CodexProposalClip`, `CodexRenderReview`, app-server transport, and split-project conversation history.
- Produces:
  - Rust `CodexConversationEditRequest`
  - Rust `CodexConversationEditProposal`
  - Rust `CodexConversationTurnResult`
  - TypeScript `CodexConversationEditRequest`
  - TypeScript `CodexConversationEditCommandResult`
  - `startCodexConversationEditForProject(input)`

- [ ] **Step 1: Write failing Rust request and proposal contract tests**

Add tests that prove a conversation request has no preset fields, accepts a
direct local action without an EDL, and rejects a newly added primary video item
without an EDL:

```rust
#[test]
fn conversation_request_has_no_creative_preset() {
    let request = CodexConversationEditRequest {
        prompt: "Balance the dialogue.".to_string(),
        focus: CodexConversationFocus::default(),
        created_at: "2026-07-25T00:00:00Z".to_string(),
    };
    let value = serde_json::to_value(request).expect("serialize request");
    assert_eq!(value["prompt"], "Balance the dialogue.");
    assert!(value.get("preset").is_none());
    assert!(value.get("targetDurationSeconds").is_none());
}

#[test]
fn direct_existing_item_edit_does_not_require_an_edl() {
    let project = conversation_fixture_project();
    let proposal = CodexConversationEditProposal {
        summary: "Lowered the interview clip by 2 dB.".to_string(),
        edl: Vec::new(),
        project_actions: vec![ProjectAction::UpdateAudioVolume {
            item_id: "audio-1".to_string(),
            volume_db: Some(-2.0),
        }],
        render_review: None,
    };
    let prepared = prepare_codex_conversation_proposal(&project, &proposal)
        .expect("direct edit should validate");
    assert_eq!(prepared.actions.len(), 1);
}
```

- [ ] **Step 2: Run the focused Rust tests and confirm failure**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation -- --test-threads=1
```

Expected: compilation fails because the conversation request, proposal, and
preparation function do not exist.

- [ ] **Step 3: Implement the new Rust request/proposal types**

Create these exact public contracts in
`src-tauri/src/codex/conversation.rs`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexConversationFocus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_media_id: Option<String>,
    #[serde(default)]
    pub media_ids: Vec<String>,
    #[serde(default)]
    pub timeline_item_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeline_range: Option<CodexConversationRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexConversationRange {
    pub start_seconds: f64,
    pub end_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexConversationEditRequest {
    pub prompt: String,
    #[serde(default)]
    pub focus: CodexConversationFocus,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexConversationEditProposal {
    pub summary: String,
    #[serde(default)]
    pub edl: Vec<CodexProposalClip>,
    #[serde(default)]
    pub project_actions: Vec<ProjectAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_review: Option<CodexRenderReview>,
}
```

Implement `CodexConversationEditRequest::validate` with these checks:

- trimmed prompt is non-empty;
- every focus media/item ID exists;
- a timeline range is finite, non-negative, ordered, and inside the timeline;
- duplicate focus IDs are normalized away before context construction.

Implement proposal validation so:

- all actions apply successfully to a cloned project;
- an empty action list is rejected;
- an EDL is optional for edits to existing items;
- any new video/audio primary item requires a non-empty EDL;
- every EDL range references existing media and has `sourceOut > sourceIn`;
- EDL-derived primary items are materialized before visual-layer actions;
- render-review criteria are required when an EDL or newly added visual layer is present.

- [ ] **Step 4: Add a separate conversation app-server schema and turn**

Keep `build_video_edit_turn_request` and `CodexEditProposal` unchanged for the
existing preset workflow. Add:

```rust
pub fn build_codex_conversation_turn_request(
    request_id: u64,
    thread_id: &str,
    context: &CodexConversationContext,
) -> Value;

pub fn start_codex_conversation_turn<T: CodexAppServerTransport>(
    transport: &mut T,
    request_id_start: u64,
    cwd: &str,
    project: &mut VideoProject,
    request: CodexConversationEditRequest,
    skills: &ProjectSkillBundle,
    project_dir: Option<&Path>,
) -> Result<CodexConversationTurnResult, CodexAppServerError>;
```

The output schema requires `summary`, `edl`, `projectActions`, and
`renderReview`, permits an empty EDL, and uses the existing
`project_action_schema()`. The prompt must say:

```text
Return only a structured project-edit proposal.
Use projectActions for edits to the existing timeline.
Include an EDL before adding a new primary video/audio cut.
Do not infer a trailer, highlight, or story preset.
```

Do not include a preset, target duration, or caption style in this prompt.

- [ ] **Step 5: Make stored conversation requests forward-compatible**

Change split-project app-server history from `EditJobRequest` to
`serde_json::Value` while preserving the denormalized `prompt` field. Update
`AppServerConversationTurn` to accept:

```rust
pub struct AppServerConversationTurn {
    pub turn_id: Option<String>,
    pub turn_status: Option<String>,
    pub prompt: String,
    pub created_at: String,
    pub request: Value,
    pub thread_response: Value,
    pub turn_response: Value,
    pub has_proposal: bool,
}
```

Use `prompt` and `created_at` for session title/timestamps, so legacy saved
`EditJobRequest` JSON and new conversation-request JSON both load.

- [ ] **Step 6: Add TypeScript bridge types and wrapper**

Add to `src/lib/project.ts`:

```ts
export interface CodexConversationRange {
  startSeconds: number;
  endSeconds: number;
}

export interface CodexConversationFocus {
  primaryMediaId?: string | null;
  mediaIds: string[];
  timelineItemIds: string[];
  timelineRange?: CodexConversationRange | null;
}

export interface CodexConversationEditRequest {
  prompt: string;
  focus: CodexConversationFocus;
  createdAt: string;
}

export interface CodexConversationEditProposal {
  summary: string;
  edl: CodexProposalClip[];
  projectActions: ProjectAction[];
  renderReview: CodexRenderReview | null;
}

export interface CodexConversationEditCommandResult {
  project: VideoProject;
  threadId: string;
  threadResponse: unknown;
  turnResponse: unknown;
  proposal: CodexConversationEditProposal | null;
  proposalValidationIssues: ProjectValidationIssue[] | null;
}

export async function startCodexConversationEditForProject(input: {
  projectRoot?: string;
  projectDir?: string;
  project: VideoProject;
  request: CodexConversationEditRequest;
}): Promise<CodexConversationEditCommandResult> {
  return invoke("start_codex_conversation_edit_for_project", input);
}
```

Type `AppServerConversationEntry.request` as
`EditJobRequest | CodexConversationEditRequest | Record<string, unknown>`.

Add the internal Rust turn result so Task 1 compiles independently:

```rust
#[derive(Debug, Clone)]
pub struct CodexConversationTurnResult {
    pub thread_id: String,
    pub thread_response: Value,
    pub turn_response: Value,
    pub proposal: Option<CodexConversationEditProposal>,
    pub proposal_validation_issues: Option<Vec<ProjectValidationIssue>>,
}
```

- [ ] **Step 7: Run focused contract tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation -- --test-threads=1
rtk pnpm test -- src/lib/project.test.ts
```

Expected: both commands pass; existing one-click proposal tests still compile.

- [ ] **Step 8: Commit the contract**

```bash
rtk git add src-tauri/src/codex/conversation.rs src-tauri/src/codex/mod.rs src-tauri/src/codex/app_server.rs src-tauri/src/project/split.rs src-tauri/tests/codex_conversation.rs src/lib/project.ts src/lib/project.test.ts
rtk git commit -m "feat(codex): add preset-free conversation contract"
```

---

### Task 2: Keep adaptive focus context internal

**Files:**
- Modify: `src-tauri/src/codex/context.rs:17-160,680-910,1010-1600`
- Modify: `src-tauri/src/codex/app_server.rs:489-750`
- Modify: `src/components/workspace/editor-workspace.tsx:3326-3390,4325-4360,10440-10520`
- Modify: `src/components/workspace/agent-panel.tsx:1710-1770,2160-2220,2870-2990`
- Create: `src/lib/codex-conversation.ts`
- Create: `src/lib/codex-conversation.test.ts`
- Test: `src-tauri/tests/codex_conversation.rs`
- Test: `src/components/workspace/agent-panel.test.tsx:1720-2160`

**Interfaces:**
- Consumes: `CodexConversationFocus`, current selected media/item/range state, and `AgentMentionTarget`.
- Produces:
  - `buildCodexConversationRequest(input): CodexConversationEditRequest`
  - `mentionTokenForTarget(target): string`
  - Rust `build_codex_conversation_context`

- [ ] **Step 1: Write failing focus and mention tests**

Add frontend tests:

```ts
it("keeps canonical mention ids in focus without rewriting the visible prompt", () => {
  const result = buildCodexConversationRequest({
    prompt: "Remove dead air from @Interview.",
    mentionTargets: [{ token: "Interview", canonicalId: "media-1" }],
    primaryMediaId: "media-2",
    selectedTimelineItemIds: ["clip-7"],
    selectedTimelineRange: { startSeconds: 12, endSeconds: 18 },
    createdAt: "2026-07-25T00:00:00Z",
  });
  const request = result.request;
  expect(result.unresolvedMentionTokens).toEqual([]);
  expect(request).not.toBeNull();
  if (!request) throw new Error("expected resolved conversation request");
  expect(request.prompt).toBe("Remove dead air from @Interview.");
  expect(request.focus.mediaIds).toEqual(["media-1"]);
  expect(request.focus.timelineItemIds).toEqual(["clip-7"]);
});
```

Add Rust tests proving:

- projects below every context cap include every media/track/item;
- when over a cap, focused media and selected timeline items appear before
  unfocused entries;
- truncation markers remain in the model prompt;
- context summaries are not part of any command result presentation model.

- [ ] **Step 2: Run the focused tests and confirm failure**

```bash
rtk pnpm test -- src/lib/codex-conversation.test.ts
rtk cargo test --manifest-path src-tauri/Cargo.toml codex_conversation_context -- --test-threads=1
```

Expected: missing request builder and conversation-context symbols.

- [ ] **Step 3: Implement stable human mention tokens**

In `src/lib/codex-conversation.ts`, implement token generation from the human
label:

```ts
export function mentionToken(label: string, fallbackId: string): string {
  const stem = label.replace(/\.[A-Za-z0-9]{1,8}$/u, "");
  const token = stem
    .normalize("NFKD")
    .replace(/[^\p{L}\p{N}_-]+/gu, "-")
    .replace(/^-+|-+$/g, "");
  return token || fallbackId;
}
```

Resolve collisions by appending `-2`, `-3`, and so on in project order. Extend
`AgentMentionTarget` with `token` and `canonicalId`; render and insert
`@${target.token}`, never `@${target.mediaId}`.

- [ ] **Step 4: Build hidden request focus in the workspace**

Use this result type:

```ts
export interface BuildCodexConversationRequestResult {
  request: CodexConversationEditRequest | null;
  unresolvedMentionTokens: string[];
}
```

`buildCodexConversationRequest` must:

- preserve the exact visible prompt;
- resolve every recognized mention token into canonical media IDs;
- include the current selected media as `primaryMediaId`;
- include selected timeline item IDs and range;
- deduplicate canonical IDs;
- return `request: null` plus every unresolved token so the composer can show an
  inline resolution error before submission.

Pass only the resulting request to `onGenerateEdit`. Do not add focus chips,
range strips, resolved-mention cards, or helper text.

- [ ] **Step 5: Implement focus-first context ranking in Rust**

Add `CodexConversationContext` and rank each bounded collection:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct CodexConversationContext {
    pub project_id: String,
    pub project_name: String,
    pub request: CodexConversationEditRequest,
    pub internal_focus_summary: String,
    pub media_library_summary: String,
    pub timeline_summary: String,
    pub transcript_excerpts_summary: String,
    pub generated_assets_summary: String,
    pub template_overrides_summary: String,
    pub render_reports_summary: String,
    pub workflow_jobs_summary: String,
    pub export_artifacts_summary: String,
    pub export_capabilities_summary: String,
    pub project_files_summary: String,
}
```

1. explicitly focused IDs;
2. objects referenced by focused timeline items;
3. objects referenced by the active timeline;
4. remaining objects in canonical project order.

Use the existing caps:

```rust
const MAX_CONTEXT_FOLDERS: usize = 40;
const MAX_CONTEXT_MEDIA: usize = 80;
const MAX_CONTEXT_TRACKS: usize = 12;
const MAX_CONTEXT_TIMELINE_ITEMS: usize = 120;
```

When counts fit, include the full collection. When counts exceed a cap, take the
ranked prefix and append the existing truncation marker. Include focus facts in
the model prompt under `Internal edit focus`; never return them as a UI field.

- [ ] **Step 6: Remove duplicate context presentation**

Delete from `AgentPanel` rendering:

- the `@effectiveMediaId` badge;
- "Mention @media-1 in the prompt to anchor this edit";
- selected range status;
- resolved mention card;
- default model/timeline/project-context tool rows;
- "I can see the project media, timeline, templates, and render settings."
  synthetic assistant message;
- target metadata beside user messages.

Keep mention autocomplete directly below the composer. Its option text is the
human token and label; canonical IDs may exist only in React keys/data.

- [ ] **Step 7: Run focused context/privacy tests**

```bash
rtk pnpm test -- src/lib/codex-conversation.test.ts src/components/workspace/agent-panel.test.tsx
rtk cargo test --manifest-path src-tauri/Cargo.toml codex_conversation_context -- --test-threads=1
```

Expected: full/bounded context tests pass and the DOM contains no selected
context strip, resolved mention card, `@media-1` helper, or default tool rows.

- [ ] **Step 8: Commit hidden context behavior**

```bash
rtk git add src-tauri/src/codex/context.rs src-tauri/src/codex/app_server.rs src-tauri/tests/codex_conversation.rs src/lib/codex-conversation.ts src/lib/codex-conversation.test.ts src/components/workspace/editor-workspace.tsx src/components/workspace/agent-panel.tsx src/components/workspace/agent-panel.test.tsx
rtk git commit -m "feat(codex): keep adaptive edit context internal"
```

---

### Task 3: Prepare exact actions, impact, and risk in Rust

**Files:**
- Modify: `src-tauri/src/codex/conversation.rs`
- Modify: `src-tauri/src/codex/app_server.rs:20-45,280-345`
- Modify: `src-tauri/src/main.rs:540-565,5700-5760,6200-6220`
- Modify: `src/lib/project.ts:485-540,3638-3660`
- Test: `src-tauri/tests/codex_conversation.rs`
- Test: `src/lib/project.test.ts`

**Interfaces:**
- Consumes: validated `CodexConversationEditProposal` and the canonical project.
- Produces:
  - `CodexPreparedProposal`
  - `CodexProposalImpact`
  - `CodexProposalRisk`
  - `prepare_codex_conversation_proposal(project, proposal)`

- [ ] **Step 1: Write failing allowlist and impact tests**

Cover these cases:

```rust
#[test]
fn ripple_dead_air_edit_is_safe_and_reports_duration_change() {
    let prepared = prepare_with_actions(vec![ProjectAction::RippleDeleteRanges {
        ranges: vec![ProjectActionRippleDeleteRange {
            start_seconds: 4.0,
            end_seconds: 6.0,
            track_ids: vec!["video".to_string(), "audio".to_string()],
        }],
    }]);
    assert_eq!(prepared.risk.level, CodexProposalRiskLevel::Safe);
    assert_eq!(prepared.impact.affected_ranges[0].start_seconds, 4.0);
    assert!(prepared.impact.after_duration_seconds < prepared.impact.before_duration_seconds);
}

#[test]
fn full_timeline_removal_requires_review() {
    let prepared = prepare_with_actions(vec![ProjectAction::RemoveItems {
        item_ids: vec!["video-1".to_string(), "audio-1".to_string()],
    }]);
    assert_eq!(prepared.risk.level, CodexProposalRiskLevel::Review);
    assert!(prepared.risk.reasons.iter().any(|reason| reason.code == "deletesExistingItems"));
}
```

Also test `DeleteMedia`, `RemoveTracks`, `DeleteTimeline`,
`UpdateRenderSettings`, generation/job/export actions, and template overrides as
review; local trims, splits, ripple deletion, volume, captions, effects, and
non-destructive additions as safe.

- [ ] **Step 2: Run the risk tests and confirm failure**

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation risk -- --test-threads=1
```

Expected: `CodexPreparedProposal` and risk types are missing.

- [ ] **Step 3: Implement result contracts**

Use these serialized types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CodexProposalRiskLevel {
    Safe,
    Review,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexRiskReason {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexProposalRisk {
    pub level: CodexProposalRiskLevel,
    pub reasons: Vec<CodexRiskReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexProposalImpact {
    pub summary: String,
    pub before_duration_seconds: f64,
    pub after_duration_seconds: f64,
    pub affected_item_ids: Vec<String>,
    pub affected_ranges: Vec<CodexConversationRange>,
    pub preview_timestamp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexPreparedProposal {
    pub actions: Vec<ProjectAction>,
    pub action_ids: Vec<String>,
    pub risk: CodexProposalRisk,
    pub impact: CodexProposalImpact,
}
```

Mirror the serialized contracts in `src/lib/project.ts`:

```ts
export type CodexProposalRiskLevel = "safe" | "review";

export interface CodexProposalRisk {
  level: CodexProposalRiskLevel;
  reasons: Array<{ code: string; message: string }>;
}

export interface CodexProposalImpact {
  summary: string;
  beforeDurationSeconds: number;
  afterDurationSeconds: number;
  affectedItemIds: string[];
  affectedRanges: CodexConversationRange[];
  previewTimestamp: number;
}

export interface CodexPreparedProposal {
  actions: ProjectAction[];
  actionIds: string[];
  risk: CodexProposalRisk;
  impact: CodexProposalImpact;
}
```

Materialize once in Rust, apply the actions to a clone, and diff the before/after
timelines by canonical item ID. Affected ranges are the normalized union of old
and new item spans. The preview timestamp is the first affected range start,
clamped below the resulting timeline duration.

- [ ] **Step 4: Implement the fail-closed risk allowlist**

`is_safe_local_action` returns true only for:

- add/insert/move/reorder/resize/trim/ripple-trim/ripple-delete/split;
- non-destructive track creation and state changes;
- caption/text/audio/visual/property/keyframe/effect/color updates;
- link/unlink, caption repair, transcript repair, and local text/template item
  updates.

It returns false for removals of full items/tracks/timelines/media/folders,
decomposition, generated-asset/provider/job actions, render/export metadata,
project/render settings, template overrides, and every unmatched future
variant. Add specific reason codes for the known review classes and
`unclassifiedAction` for the wildcard branch.

- [ ] **Step 5: Return only Rust-prepared bundles to the client**

Extend `CodexConversationEditCommandResult` with `preparedProposal` while
retaining its existing `proposalValidationIssues` field:

```ts
preparedProposal: CodexPreparedProposal | null;
proposalValidationIssues: ProjectValidationIssue[] | null;
```

When validation fails, return issues and no prepared proposal. When it passes,
return the exact materialized actions, action IDs, risk, and impact. Remove any
client rematerialization from the new conversation path.

- [ ] **Step 6: Run risk, serialization, and legacy proposal tests**

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation -- --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml codex_edit_proposal -- --test-threads=1
rtk pnpm test -- src/lib/project.test.ts
```

Expected: safe/review cases serialize in camelCase and legacy generated-cut
validation still passes.

- [ ] **Step 7: Commit trusted preparation**

```bash
rtk git add src-tauri/src/codex/conversation.rs src-tauri/src/codex/app_server.rs src-tauri/src/main.rs src-tauri/tests/codex_conversation.rs src/lib/project.ts src/lib/project.test.ts
rtk git commit -m "feat(codex): classify validated proposal risk"
```

---

### Task 4: Apply and undo agent batches atomically

**Files:**
- Modify: `src-tauri/src/project/split.rs:540-700,2744-2805,3040-3080,7700-7785`
- Modify: `src-tauri/src/main.rs:2040-2120,6140-6180`
- Modify: `src/lib/project.ts:540-575,3580-3630`
- Test: `src-tauri/tests/project_split.rs`
- Test: `src/lib/project.test.ts`

**Interfaces:**
- Consumes: `projectDir`, prepared action IDs/actions, active session ID, and risk confirmation.
- Produces:
  - Rust `apply_agent_project_action_batch`
  - Tauri `apply_codex_conversation_proposal`
  - Tauri `undo_latest_codex_conversation_edit`
  - TypeScript wrappers with `ProjectAgentApplyResult` and `ProjectAgentUndoResult`

- [ ] **Step 1: Write failing atomic apply and conflict tests**

Test:

- safe batch applies and records before/after under one project lease;
- review batch is rejected unless `reviewApproved` is true;
- failed action leaves the project and history unchanged;
- active session/last turn becomes `applied` with exact action IDs;
- Undo succeeds while current project equals recorded `after`;
- a later canonical edit makes Undo return the existing conflict error.

- [ ] **Step 2: Run focused split-project tests and confirm failure**

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml agent_project_action_batch -- --test-threads=1
```

Expected: the dedicated atomic apply function and commands are missing.

- [ ] **Step 3: Implement one-lease apply and history recording**

Add:

```rust
pub fn apply_agent_project_action_batch(
    project_dir: &Path,
    actions: Vec<ProjectAction>,
    action_ids: Vec<String>,
    session_id: Option<&str>,
) -> Result<ProjectAgentApplyResult, SplitProjectError>;
```

Within one `split_project_mutation_lease`:

1. load `before`;
2. apply every action to a clone;
3. save `after` transactionally;
4. append the before/after history entry;
5. update the active session and last turn to `applied`;
6. return the saved project, write report, history identity, and action IDs.

If any step before the transactional save fails, write neither project nor
history. If session metadata fails after the project transaction, return a
successful applied result with the metadata problem in `warnings`; do not report
an apply failure after the canonical edit and recoverable history entry exist.

- [ ] **Step 4: Expose guarded apply and persistent Undo commands**

The apply command accepts `request`, `proposal`, and `reviewApproved`. It
refuses a recomputed `Review` bundle when false. Before applying, recompute
validation, impact, and risk from the current loaded project and proposal; do
not trust a client-edited risk label or action array.

The Undo command delegates to `undo_latest_agent_project_action`, returning its
conflict reason without partial rollback.

- [ ] **Step 5: Add TypeScript wrappers**

```ts
export async function applyCodexConversationProposal(input: {
  projectDir: string;
  request: CodexConversationEditRequest;
  proposal: CodexConversationEditProposal;
  reviewApproved: boolean;
  sessionId?: string | null;
}): Promise<ProjectAgentApplyResult> {
  return invoke("apply_codex_conversation_proposal", input);
}

export async function undoLatestCodexConversationEdit(input: {
  projectDir: string;
}): Promise<ProjectAgentUndoResult> {
  return invoke("undo_latest_codex_conversation_edit", input);
}
```

Use this return contract for the apply wrapper:

```ts
export interface ProjectAgentApplyResult {
  project: VideoProject;
  report: ProjectWriteReport;
  historyEntryId: string;
  actionIds: string[];
  warnings: string[];
}

export interface ProjectAgentUndoResult {
  project: VideoProject;
  report: ProjectWriteReport;
  entryId: string;
  actionCount: number;
  remainingAgentHistory: number;
}
```

- [ ] **Step 6: Run persistence tests**

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml agent_project_action_batch -- --test-threads=1
rtk pnpm test -- src/lib/project.test.ts
```

Expected: apply/undo is durable, atomic, and conflict-aware.

- [ ] **Step 7: Commit atomic apply and Undo**

```bash
rtk git add src-tauri/src/project/split.rs src-tauri/src/main.rs src-tauri/tests/project_split.rs src/lib/project.ts src/lib/project.test.ts
rtk git commit -m "feat(codex): persist reversible proposal batches"
```

---

### Task 5: Build the focused conversation rail

**Files:**
- Create: `src/components/workspace/codex-conversation-cards.tsx`
- Create: `src/components/workspace/codex-conversation-cards.test.tsx`
- Modify: `src/lib/codex-conversation.ts`
- Modify: `src/lib/codex-conversation.test.ts`
- Modify: `src/components/workspace/agent-panel.tsx:1-330,602-930,1980-2235,2430-3045`
- Modify: `src/components/workspace/agent-panel.test.tsx`
- Modify: `src/components/workspace/codex-proposal-review.tsx:471-710`

**Interfaces:**
- Consumes: typed conversation entries, current progress, prepared review, applied result, composer state, and callbacks.
- Produces:
  - `CodexProgressRow`
  - `CodexAppliedEditCard`
  - `CodexRiskReviewCard`
  - `CodexFailureMessage`
  - empty-chat-only quick edits

- [ ] **Step 1: Write failing component tests**

Cover:

- empty textarea and no preset text;
- exact quick-edit labels from the spec;
- quick-edit click fills but does not submit;
- quick edits disappear after first user entry;
- a mention is visible once in the user message;
- one compact progress row and optional Details;
- risk card exposes Apply, Reject, and "Ask for a safer edit";
- applied card exposes Show changes and Undo;
- disabled Undo has an accessible reason;
- validation, apply, and preview failures use the specified recovery actions.

- [ ] **Step 2: Run component tests and confirm failure**

```bash
rtk pnpm test -- src/components/workspace/codex-conversation-cards.test.tsx src/components/workspace/agent-panel.test.tsx
```

Expected: new cards are missing and current default/context assertions conflict.

- [ ] **Step 3: Implement typed transcript entries**

Use:

```ts
export type CodexConversationEntry =
  | { id: string; kind: "user"; text: string; createdAt: string }
  | { id: string; kind: "assistant"; text: string; createdAt: string }
  | { id: string; kind: "applied"; result: CodexAppliedEditResult; createdAt: string }
  | { id: string; kind: "error"; title: string; detail: string; createdAt: string };

export const codexQuickEdits = [
  "Tighten the pacing",
  "Remove dead air",
  "Add clean captions",
  "Balance the audio",
  "Make a shorter cut",
] as const;
```

Define the durable applied result separately from its live preview snapshot:

```ts
export interface CodexAppliedEditResult {
  id: string;
  projectUpdatedAt: string;
  impact: CodexProposalImpact;
  undoIdentity: string;
  undoAvailable: boolean;
  undoUnavailableReason: string | null;
}
```

Persist only durable entries. Progress and an awaiting-review proposal are
live state, not fake transcript messages. Migrate the existing local transcript
key by reading v1 entries and writing the new v2 union.

- [ ] **Step 4: Implement compact cards**

`CodexProgressRow` shows exactly one phase:
`reviewing | validating | applying | preparingPreview`. It accepts
`details: readonly string[]`; when non-empty, render them inside one collapsed
native `<details>` element labeled **Details**, not as separate tool cards.

`CodexRiskReviewCard` renders the trusted summary and reasons, never raw
project actions or context. `CodexAppliedEditCard` renders summary,
before/after duration, preview slot, Show changes, and Undo.

Use flat bordered rows/cards; do not nest decorative cards or add hero
headings. Mark changing progress copy with `aria-live="polite"`, retain visible
focus rings, and support Enter/Space activation for the preview-frame button.

- [ ] **Step 5: Simplify AgentPanel submission**

Initialize `prompt` to `""`. Remove `selectedPreset`, `selectedLanguage`, and
the composer branches that directly execute regex-detected clip, caption,
audio, template, generation, or delete operations. Every submitted composer
request calls the new `onSubmitConversationEdit(request)`.

Keep the legacy direct-edit callback props temporarily only if other non-composer
controls still call them; remove each unused prop after TypeScript confirms no
callers.

Render:

1. session toolbar;
2. transcript;
3. one live progress/review state;
4. composer.

Do not render workflow job cards in the conversation. A real Codex job may feed
the single progress row through workspace state.

- [ ] **Step 6: Keep the old generated-cut review out of the rail**

Retain `buildCodexProposalReviewModel` and its tests for legacy generated-cut
callers. Stop mounting `CodexProposalReview` inside `AgentPanel`; the
conversation uses `CodexRiskReviewCard`.

- [ ] **Step 7: Run rail tests**

```bash
rtk pnpm test -- src/lib/codex-conversation.test.ts src/components/workspace/codex-conversation-cards.test.tsx src/components/workspace/agent-panel.test.tsx src/components/workspace/codex-proposal-review.test.tsx
```

Expected: rail tests pass, legacy review-model tests remain green, and no
default creative prompt or context chrome is rendered.

- [ ] **Step 8: Commit the conversation UI**

```bash
rtk git add src/lib/codex-conversation.ts src/lib/codex-conversation.test.ts src/components/workspace/codex-conversation-cards.tsx src/components/workspace/codex-conversation-cards.test.tsx src/components/workspace/agent-panel.tsx src/components/workspace/agent-panel.test.tsx src/components/workspace/codex-proposal-review.tsx
rtk git commit -m "feat(codex): rebuild rail as focused conversation"
```

---

### Task 6: Wire safe apply, risky review, and failure recovery

**Files:**
- Modify: `src/components/workspace/editor-workspace.tsx:3500-3520,4050-4120,7520-7690,10430-10560`
- Modify: `src/components/workspace/editor-workspace.test.tsx`
- Modify: `src/components/workspace/agent-panel.tsx`
- Modify: `src/components/workspace/codex-conversation-cards.tsx`

**Interfaces:**
- Consumes: `startCodexConversationEditForProject`, prepared risk/impact, atomic apply/undo wrappers.
- Produces:
  - `CodexConversationStatus`
  - `pendingCodexReview`
  - `latestCodexAppliedResult`
  - submit/apply/reject/revise/undo handlers

- [ ] **Step 1: Write failing workspace state-transition tests**

Use mocked Tauri bridge results to prove:

- submit transitions `reviewing -> validating`;
- safe prepared proposal calls apply without showing review;
- review proposal does not call apply before user Apply;
- Reject leaves project unchanged;
- review Apply passes `reviewApproved: true`;
- validation failure retains the request and offers revision;
- apply failure does not replace the project;
- successful apply appends an applied transcript result;
- Undo refreshes the project from Rust;
- later-project conflict disables Undo with the Rust reason.

- [ ] **Step 2: Run workspace tests and confirm failure**

```bash
rtk pnpm test -- src/components/workspace/editor-workspace.test.tsx
```

Expected: old `codexStatus` and manual proposal flow do not satisfy the new
transitions.

- [ ] **Step 3: Replace the old proposal state**

Use:

```ts
type CodexConversationStatus =
  | "idle"
  | "reviewing"
  | "validating"
  | "applying"
  | "preparingPreview";

interface PendingCodexReview {
  request: CodexConversationEditRequest;
  proposal: CodexConversationEditProposal;
  prepared: CodexPreparedProposal;
}

type ApplyPreparedCodexEdit = (
  request: CodexConversationEditRequest,
  proposal: CodexConversationEditProposal,
  prepared: CodexPreparedProposal,
  reviewApproved: boolean,
) => Promise<void>;
```

Remove the conversation path's dependency on
`codexProposalReviewModel.canApply` and
`materializeCodexProposalActions`. Keep those helpers only for legacy callers
until repository search confirms they are unused.

- [ ] **Step 4: Implement submit and safe auto-apply**

`submitCodexConversationEdit`:

1. append the exact user prompt;
2. call the conversation start command;
3. show validation issues without mutation when preparation is absent;
4. store a `Review` bundle as `pendingCodexReview`;
5. immediately call
   `applyPreparedCodexEdit(request, result.proposal, result.preparedProposal, false)`
   for `Safe`.

Never use a model-provided risk label.

- [ ] **Step 5: Implement review actions and recovery**

- Apply calls the dedicated Rust command with `reviewApproved: true`.
- Reject clears the pending proposal and appends a short assistant response.
- "Ask for a safer edit" restores the original prompt with
  `"Revise this as a safer, narrower edit: "` prepended and focuses the
  composer.
- Validation/apply errors retain the original request and expose Retry.

- [ ] **Step 6: Replace in-memory-only Undo for split projects**

Use persistent Rust Undo when `schemaVersion >= 2` and `projectDir` is present.
Keep the existing in-memory snapshot fallback only for unsaved browser fixtures.
On conflict, leave the applied card visible, disable Undo, and store the exact
reason.

- [ ] **Step 7: Run state-transition tests**

```bash
rtk pnpm test -- src/components/workspace/editor-workspace.test.tsx src/components/workspace/agent-panel.test.tsx src/components/workspace/codex-conversation-cards.test.tsx
```

Expected: safe and risky paths have distinct mutation behavior and every
failure is recoverable.

- [ ] **Step 8: Commit orchestration**

```bash
rtk git add src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx src/components/workspace/agent-panel.tsx src/components/workspace/codex-conversation-cards.tsx
rtk git commit -m "feat(codex): route edits through risk-aware outcomes"
```

---

### Task 7: Add accurate static result preview and change navigation

**Files:**
- Modify: `src/components/workspace/timeline-preview-compositor.tsx:449-1025`
- Modify: `src/components/workspace/timeline-preview-compositor.test.tsx`
- Modify: `src/components/workspace/codex-conversation-cards.tsx`
- Modify: `src/components/workspace/codex-conversation-cards.test.tsx`
- Modify: `src/components/workspace/timeline-editor.tsx:190-225,2270-2290,4590-4980`
- Modify: `src/components/workspace/timeline-editor.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx:3540-3620,6660-6760,10620-10930`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

**Interfaces:**
- Consumes: applied result impact, exact post-apply project, existing compositor, viewer controls, timeline selection/range rendering.
- Produces:
  - `TimelinePreviewCompositor` `interactionMode="static"`
  - `highlightedRanges`
  - `showCodexResultInViewer(result)`
  - `showCodexChanges(result)`

- [ ] **Step 1: Write failing static-preview and highlight tests**

Prove:

- static compositor evaluates the post-apply project at `previewTimestamp`;
- it has no edit handles, pointer commit callbacks, audio playback, or transport;
- clicking the card wrapper calls Open/Play in the main viewer;
- preview retry remounts the static compositor;
- retry failure leaves summary, Show changes, and Undo;
- Show changes selects all existing affected item IDs and renders every affected
  range as a passive highlight;
- stale project revision marks the frame as an earlier result.

- [ ] **Step 2: Run focused preview/timeline tests and confirm failure**

```bash
rtk pnpm test -- src/components/workspace/timeline-preview-compositor.test.tsx src/components/workspace/timeline-editor.test.tsx src/components/workspace/codex-conversation-cards.test.tsx
```

Expected: static mode and passive result highlights are missing.

- [ ] **Step 3: Add non-interactive compositor mode**

Extend props with:

```ts
interactionMode?: "editor" | "static";
onPreviewStateChange?: (state: TimelinePreviewCanvasState) => void;
```

For `static`:

- force `isPlaying={false}`;
- do not render audio elements;
- omit pointer handlers and selection overlays;
- keep media, prepared frames, captions, templates, and visual layers;
- report `clear | warning | retry` through `onPreviewStateChange`;
- add `aria-label="Changed project preview frame"`.

- [ ] **Step 4: Mount the static frame in the applied card**

Pass the exact `after` project snapshot and `impact.previewTimestamp` to the
compositor. Wrap it in a keyboard-activatable button that calls
`onOpenInViewer`. If the compositor reports `retry`, render **Open viewer** and
**Retry preview** without hiding result actions.

- [ ] **Step 5: Add passive affected-range highlights**

Extend `TimelineEditorProps`:

```ts
highlightedRanges?: readonly TimelineRangeSelection[];
```

Render each range as a non-interactive cyan band on ruler and canvas with
`pointer-events-none`; do not reuse draggable selected-range handles. Item IDs
continue to use existing multi-selection styling.

- [ ] **Step 6: Wire viewer and timeline navigation**

`showCodexResultInViewer`:

```ts
setViewerMode("timeline");
setTimelinePlayheadSeconds(result.impact.previewTimestamp);
setTimelinePlaying(true);
focusWorkspacePane("timeline");
```

`showCodexChanges`:

- filter affected IDs against the current timeline;
- set multi-selection to the remaining IDs;
- set `codexHighlightedRanges` to all impact ranges;
- seek to the first range;
- focus the timeline pane;
- clear highlights on the next unrelated user selection or project mutation.

- [ ] **Step 7: Run preview/navigation tests**

```bash
rtk pnpm test -- src/components/workspace/timeline-preview-compositor.test.tsx src/components/workspace/timeline-editor.test.tsx src/components/workspace/codex-conversation-cards.test.tsx src/components/workspace/editor-workspace.test.tsx
```

Expected: preview is accurate, static, and navigates the single main viewer;
all affected ranges are visible.

- [ ] **Step 8: Commit result preview/navigation**

```bash
rtk git add src/components/workspace/timeline-preview-compositor.tsx src/components/workspace/timeline-preview-compositor.test.tsx src/components/workspace/codex-conversation-cards.tsx src/components/workspace/codex-conversation-cards.test.tsx src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat(codex): preview and reveal applied edits"
```

---

### Task 8: Add visual states and complete acceptance

**Files:**
- Modify: `src/lib/modern-editor-visual-qa-fixtures.ts`
- Modify: `scripts/browser-visual-qa.mjs`
- Modify: `src/browser-visual-qa-palmier-scenarios.test.ts`
- Modify: `docs/visual-qa/browser-visual-baseline/`
- Test: all focused files from Tasks 1-7

**Interfaces:**
- Consumes: completed conversation states and deterministic sample project.
- Produces: browser regression fixtures plus native Tauri acceptance evidence.

- [ ] **Step 1: Write failing scenario-matrix tests**

Require these fixtures:

```ts
[
  "modern-editor-codex-empty",
  "modern-editor-codex-working",
  "modern-editor-codex-applied",
  "modern-editor-codex-review",
  "modern-editor-codex-failed",
]
```

Require desktop and narrow rail captures for empty, applied, and review states.

- [ ] **Step 2: Run scenario tests and confirm failure**

```bash
rtk pnpm test -- src/browser-visual-qa-palmier-scenarios.test.ts
```

Expected: the new fixtures and capture definitions are absent.

- [ ] **Step 3: Add deterministic visual fixtures**

Use human mention text such as `@Interview`, safe result copy
`Removed 26 seconds of silence across four gaps`, a 03:42 to 03:16 duration
fact, and a risky `Replace the full timeline` review. Do not put canonical
media IDs, context rows, tool cards, or preset labels in fixtures.

- [ ] **Step 4: Run focused TypeScript and Rust verification**

```bash
rtk pnpm test -- src/lib/codex-conversation.test.ts src/components/workspace/codex-conversation-cards.test.tsx src/components/workspace/agent-panel.test.tsx src/components/workspace/editor-workspace.test.tsx src/components/workspace/timeline-preview-compositor.test.tsx src/components/workspace/timeline-editor.test.tsx src/browser-visual-qa-palmier-scenarios.test.ts
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation -- --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml agent_project_action_batch -- --test-threads=1
```

Expected: all focused tests pass.

- [ ] **Step 5: Run static quality checks**

```bash
rtk pnpm lint
rtk pnpm rust:fmt
rtk pnpm rust:clippy
```

Expected: TypeScript, formatting, and Clippy complete without errors or
warnings.

- [ ] **Step 6: Run browser visual QA and review every changed state**

```bash
rtk pnpm visual:qa:browser
```

Inspect desktop and narrow screenshots for:

- readable compact density;
- no duplicate mention/context display;
- stable preview aspect ratio;
- visible focus and disabled Undo reason;
- no overflow, clipped actions, blank frame, or nested-card appearance.

After approval, update only the affected baseline images and run:

```bash
rtk pnpm visual:qa:browser-release
```

Expected: baseline comparison passes at the repository thresholds.

- [ ] **Step 7: Run packaged Tauri manual acceptance**

```bash
rtk pnpm tauri:dev
```

In the real app:

1. start a new chat and verify the empty composer/quick edits;
2. click a quick edit and verify it fills without submitting;
3. type a human mention and confirm it appears once;
4. submit a safe edit and confirm automatic apply, accurate static frame,
   main-viewer playback, timeline highlights, and Undo;
5. submit a full-timeline replacement and confirm no mutation before Apply;
6. make a later manual edit and confirm prior Undo disables with a reason;
7. force a preview-source failure and confirm Open viewer/Retry without losing
   Show changes or Undo.

Capture acceptance screenshots for empty, applied, review, and preview-failure
states.

- [ ] **Step 8: Run the final repository verification**

Run after the user says the planned implementation wave is finished:

```bash
rtk pnpm verify
```

Expected: source-quality, TypeScript, unit, build, Rust, and release visual
checks all pass.

- [ ] **Step 9: Commit acceptance coverage**

```bash
rtk git add src/lib/modern-editor-visual-qa-fixtures.ts scripts/browser-visual-qa.mjs src/browser-visual-qa-palmier-scenarios.test.ts docs/visual-qa/browser-visual-baseline/modern-editor-codex-empty.png docs/visual-qa/browser-visual-baseline/modern-editor-codex-working.png docs/visual-qa/browser-visual-baseline/modern-editor-codex-applied.png docs/visual-qa/browser-visual-baseline/modern-editor-codex-review.png docs/visual-qa/browser-visual-baseline/modern-editor-codex-failed.png docs/visual-qa/browser-visual-baseline/modern-editor-codex-empty-narrow.png docs/visual-qa/browser-visual-baseline/modern-editor-codex-applied-narrow.png docs/visual-qa/browser-visual-baseline/modern-editor-codex-review-narrow.png
rtk git commit -m "test(codex): cover proposal rail acceptance states"
```

---

## Completion Criteria

- The Codex rail starts with an empty composer and no hidden preset.
- Quick edits are empty-chat-only, concise, and fill without submitting.
- Human mentions appear once; canonical focus/context remains invisible.
- The conversation request is separate from preset-based generated cuts.
- Rust returns the exact validated action bundle, impact, and fail-closed risk.
- Safe edits auto-apply atomically; risky edits wait for Apply.
- Applied results show accurate facts, a static changed-project frame, Show
  changes, and conflict-aware Undo.
- Preview clicks play the existing main viewer; no inline player exists.
- Show changes selects affected items and renders all affected ranges.
- Validation, apply, preview, and Undo-conflict failures preserve recovery paths.
- Focused tests, lint, Rust checks, browser visual QA, and packaged Tauri
  acceptance pass with retained evidence.
