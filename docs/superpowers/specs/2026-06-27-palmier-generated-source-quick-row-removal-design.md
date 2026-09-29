# Palmier Generated Source Quick Row Removal Design

## Context

Palmier's right Source rail keeps generated media actions inside the local `Details` / `AI Edit` flow. Video Creater still renders a persistent generated-source quick-action row above those tabs with `Reveal source` and `Composer`, which makes the inspector header feel like a command bar.

## Decision

Remove the generated-source quick-action row. Generated source reveal remains available from the generated output/reference controls in `Details`, and the composer handoff moves into `AI Edit` with the other generation actions.

## Requirements

- Generated sources do not render `Generated source quick actions`.
- `Details` continues to expose reveal buttons for generated output, first frame, last frame, and references.
- `AI Edit` exposes `Use generated source in composer` when composer handoff is available.
- The composer handoff keeps the existing placement payload, including replacement placement for selected generated timeline clips.
- Imported source reveal behavior is unchanged.

## Testing

- `SourceClipInspector` asserts the quick-action group is absent.
- `SourceClipInspector` proves generated output reveal still calls `onRevealSource`.
- `SourceClipInspector` and `EditorWorkspace` prove composer handoff is available from `AI Edit`.
- Run focused inspector/workspace tests, lint, full tests, and browser QA.
