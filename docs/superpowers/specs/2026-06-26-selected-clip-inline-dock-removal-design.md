# Selected Clip Inline Dock Removal Design

## Context

The timeline canvas currently shows a floating `Selected clip actions ...` dock above the selected clip. That dock duplicates actions already covered by keyboard shortcuts, timeline toolbar controls, the source viewer, the inspector, and the agent rail. It also makes the timeline feel busier than the Palmier-style reference direction, where selected clips remain clean timeline objects instead of carrying a large contextual button cluster.

## Decision

Remove the inline selected-clip action dock from timeline items. A selected clip should still show selection state, resize handles when editable, generated/provenance/status badges, source markers, warnings, and timeline metadata. It should not render a floating group of buttons over the canvas.

Manual editing remains available through existing non-inline surfaces:

- `S`, toolbar split, source mark buttons, and source mark shortcuts keep clip editing flows available.
- `Delete`, `Backspace`, and platform duplicate shortcuts keep destructive and duplication flows available.
- `Shift+ArrowUp` and `Shift+ArrowDown` keep cross-track movement available.
- Double-click keeps source opening available for media-backed clips.
- Generated media actions stay in the media/source/inspector/agent surfaces, not overlaid on every selected timeline clip.

## Scope

This slice changes `TimelineEditor` rendering and its tests only. It does not remove callback props from the public component interface because parent surfaces may still pass them for future or adjacent control surfaces. It does remove unused local dock state and icon imports if the timeline no longer references them.

## Interaction Requirements

- Selecting a timeline item must not add any `role="group"` whose accessible name starts with `Selected clip actions`.
- The selected clip remains selectable and keeps its accessible clip button.
- Existing keyboard editing paths continue to pass their current tests.
- Existing toolbar source mark and split paths continue to pass their current tests.
- Locked-track behavior remains enforced by the existing keyboard, toolbar, drag, and resize gating.

## Testing

Update `timeline-editor.test.tsx` to assert the absence of the inline dock on selected clips and remove dock-only callback assertions. Keep or rely on existing tests for keyboard cross-track movement, split, duplicate, delete, source marks, drag/resize, and double-click source opening. Run the focused timeline editor suite plus TypeScript checks and browser QA after the production edit.

## Out Of Scope

- Adding a new replacement context menu.
- Moving generated clip rerun/edit controls to a new surface.
- Changing the right rail, source inspector, or Codex agent contract.
