# Palmier Generated AI Edit Action Strip Design

## Context

Palmier keeps the source inspector compact: generated media details, references, prompt, and AI edit actions are visible in a narrow right rail without stacked card-like controls. Video Creater already exposes the right actions for generated outputs, including use in composer, rerun, rerun-and-replace, upscale, variation prompt, and queue variation. The current `Generated AI edit` region still starts with an uppercase `Regenerate variation` label and then renders immediate actions as full-width stacked buttons. This makes the right rail taller and heavier than the Palmier target.

## Goal

Flatten the generated-source AI edit actions into a compact action strip while preserving all behavior:

- Keep the `Generated AI edit` region.
- Remove the visible `Regenerate variation` heading.
- Add a `Generated AI edit actions` group for immediate actions.
- Render immediate actions in a compact two-column grid where space allows.
- Keep existing accessible button names and callbacks:
  - `Use generated source in composer`
  - `Rerun same prompt`
  - `Rerun and replace selected clip`
  - `Queue upscale`
- Keep the variation prompt editor and queue buttons below the action strip.
- Keep unavailable-state copy for cases with no actions.

## Non-Goals

- No changes to generation request payloads.
- No changes to generated asset metadata.
- No Temporal workflow changes.
- No changes to prompt defaulting or restore behavior.
- No new action types.

## UI Requirements

1. `Generated AI edit` starts directly with action controls or the variation prompt, not an uppercase heading.
2. Immediate action buttons no longer use a full-width stacked layout.
3. The compact action group uses `grid grid-cols-2 gap-1.5`.
4. Long action labels may wrap inside their button but must not overflow the right rail.
5. The variation prompt remains readable and full-width below the action strip.

## Tests

Update `EditorWorkspace` source-inspector coverage to prove:

- `Generated AI edit` no longer contains the `Regenerate variation` heading.
- It exposes a `Generated AI edit actions` group.
- The action group contains the existing immediate actions.
- Immediate action buttons are not styled as full-width stacked buttons.
- Existing rerun, replacement, variation, and composer-draft tests still pass.

Run:

```sh
rtk ./node_modules/.bin/vitest run src/components/workspace/editor-workspace.test.tsx --testNamePattern "renders the sample generated output as an editable default timeline clip"
rtk ./node_modules/.bin/vitest run src/components/workspace/editor-workspace.test.tsx
rtk ./node_modules/.bin/tsc --noEmit
rtk git diff --check
rtk ./node_modules/.bin/vitest run
```
