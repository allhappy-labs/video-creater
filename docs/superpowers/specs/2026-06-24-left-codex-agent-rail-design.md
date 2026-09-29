# Left Codex Agent Rail Design

## Context

Palmier screenshots keep chat visible as a left rail while the media library, viewer, timeline, and right source inspector stay available. Video Creater currently places Codex below the inspector in the right rail. That works functionally, but it competes with source details and makes the app feel less like an agent-controlled editor where conversation is always available beside the library.

Palmier docs describe chat as generating and editing in context, using `@` references, placing assets on the timeline, and letting the connected agent trim, split, reorder, adjust, and rerun clips.

## Goal

Move `AgentPanel` into its own left Codex rail on desktop while keeping the media library as the second rail and preserving the right rail for inspection.

## Behavior

- The editor panes grid uses four desktop columns: Codex, source library, preview/timeline, inspector.
- The Codex rail has `aria-label="Codex agent rail"` and contains the existing `AgentPanel`.
- The source library panel remains visible beside chat on desktop.
- The right rail keeps `SourceClipInspector` or `ProjectTimelineInspector` only.
- Existing Codex behavior, selected media context, generated output insert/replace actions, and mention targets stay unchanged.
- On narrow layouts, the panes still stack in source order so chat remains accessible before the library.

## Non-Goals

- No changes to Codex request schema or proposal handling.
- No new chat threading behavior.
- No Temporal workflow changes.
- No visual redesign inside `AgentPanel`.

## Testing

- `EditorWorkspace` renders a dedicated Codex agent rail with the Codex panel.
- The inspector rail no longer contains Codex and still shows source or project inspection.
- Selecting a source clip keeps `Source Inspector` in the right rail and Codex in the left rail.
- Existing Codex generation and media action tests keep passing.
