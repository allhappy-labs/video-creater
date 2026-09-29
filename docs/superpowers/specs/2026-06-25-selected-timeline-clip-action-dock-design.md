# Selected Timeline Clip Action Dock

## Context

Palmier's editor keeps clip actions close to the timeline: selected clips can be opened, split,
trimmed, deleted, or AI-edited without leaving the edit. Video Creater already has toolbar buttons
for open source, split, delete, and source marks, plus double-click source opening. The missing
manual-editing affordance is a compact action dock directly attached to the selected timeline clip,
so common operations remain visible at the point of selection.

Palmier docs also emphasize that users can click any clip, double-click source footage, and iterate
without leaving the project: https://www.palmier.io/docs

## Goal

Show a small inline action dock for the selected timeline item that mirrors the most common toolbar
clip actions while preserving the existing toolbar and right-rail inspectors.

## Behavior

- Render the dock only for the selected timeline item.
- Dock actions:
  - open source when the item resolves to media or generated media;
  - queue a referenced AI shot for selected visual media clips using the same Temporal/fal-backed
    generation action path as the Codex rail and source inspector;
  - queue an upscale for selected visual media clips using the same generation action path as the
    Codex rail and source inspector;
  - rerun selected generated video clips with their original prompt using the same generated
    variation queue as the Codex rail and source inspector;
  - open selected generated video clips directly in the Source Inspector AI Edit tab so the original
    prompt is ready for tweaking without adding a text field to the timeline dock;
  - set source in/out marks to the playhead using the same source-trim path as the toolbar;
  - split at the same selected split point used by the toolbar;
  - delete the selected timeline item.
- The action buttons stop pointer/click propagation so using them does not start a drag, resize, or
  selection move.
- Locked-track items keep destructive or mutating actions disabled.
- The dock uses icon buttons with accessible names and `title` text.

## Non-Goals

- No new project action, schema field, Temporal workflow, fal.ai behavior, or render behavior; AI
  dock actions reuse the existing generation queue.
- No prompt-tweak text entry in this dock; prompt editing and variation sets remain in Codex and
  Source Inspector, with the dock acting as a deep link into that existing prompt surface.
- No new drag handles or trim semantics; source mark controls reuse the existing selected-clip
  trim patch builder.

## Verification

- Add TimelineEditor tests that the dock appears only on the selected clip.
- Add tests that open source, AI queue actions, generated rerun, generated AI Edit handoff, source
  in/out marks, split, and delete call the same callbacks or patch paths as the toolbar/inspector.
- Add tests that AI queue actions, generated rerun, generated AI Edit handoff, source marks, split,
  and delete are disabled for locked-track selections.
- Run focused timeline tests, lint, formatting/whitespace checks, and a secret-fragment scan.
