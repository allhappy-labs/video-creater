# Timeline Undo Redo Design

## Context

Palmier's editor screenshots show undo and redo controls as first-class timeline toolbar actions alongside select, split, source trim brackets, text, and zoom. Video Creater already routes manual edits through validated `ProjectAction` values and shows a compact toolbar, but it does not let a user recover from an edit without manually reversing the fields. That makes manual editing feel less durable than Palmier and weaker than the agent-driven action model.

Palmier's docs also describe agents that can trim, split, reorder, and adjust clips with full project context. Undo and redo should therefore sit above the same validated project-action path used by manual and agent edits, rather than becoming a separate mutation system.

## Goals

- Add undo and redo buttons at the start of the timeline toolbar.
- Track in-memory editor history for successful project mutations.
- Let the user undo and redo immediate manual project changes such as split, trim, reorder, insert, generated asset queueing, and render report recording.
- Clear redo history when a new action is applied after undo.
- Keep history out of canonical project files in this slice.
- Preserve schema v1 in-memory projects and schema v2 split-folder writes by saving the restored project snapshot when undo or redo is used.

## Non-Goals

- No persistent undo stack in project files.
- No collaborative or cross-session history.
- No Temporal workflow changes. Undo and redo are synchronous editor actions, while render and generation queues continue to use Temporal-backed job metadata.
- No deep semantic inverse action generation. This slice stores project snapshots to minimize risk across many existing `ProjectAction` variants.

## Behavior

`EditorWorkspace` owns a local history state:

- `past`: snapshots before each successful project mutation.
- `future`: snapshots popped by undo and available for redo.

When `applyProjectAction` or `applyProjectActions` succeeds, the prior project snapshot is pushed to `past` and `future` is cleared. Failed actions do not change history. Batch actions produce one history step.

Undo restores the most recent snapshot, pushes the current project to `future`, clears transient timeline errors, and writes the restored snapshot to the split project folder when the project is schema v2 and `projectDir` is set. Redo restores the next future snapshot, pushes the current project back to `past`, and also persists split projects.

`TimelineEditor` receives `canUndo`, `canRedo`, `onUndo`, and `onRedo` props. It renders two icon buttons before the select tool, matching the Palmier toolbar order. Buttons are disabled when unavailable and have accessible labels.

## Acceptance Criteria

- Timeline toolbar exposes `Undo timeline edit` and `Redo timeline edit` before the select tool.
- Undo/redo buttons are disabled until a successful project mutation creates history.
- Splitting a clip enables undo; undo restores the previous single-clip timeline and enables redo.
- Redo reapplies the split timeline state.
- Applying a new edit after undo clears redo.
- Existing timeline toolbar, split, open-source, trim, media generation, workflow queue, and render tests keep passing.
