# Imported Inspector AI Edit Inline Design

## Context

Palmier documents AI edits as native to the timeline: click an imported or AI clip to upscale or AI
edit it without leaving the timeline. Video Creater still hides imported-source AI edit actions
behind a nested `Details / AI Edit` switch in the Source Inspector. That extra switch makes imported
clips feel less directly editable than generated clips and keeps a Palmier-inconsistent toggle in
the right rail.

Palmier reference: https://www.palmier.io/docs

## Goal

Remove the imported-source `Details / AI Edit` switch. Imported visual clips and library media should
show manual source details plus AI edit controls in one continuous inspector surface.

## Behavior

- Imported visual media in the Source Inspector does not render `Details` or `AI Edit` tab buttons.
- Existing file details and timeline trim controls remain visible by default.
- Imported AI edit controls render as an inline `Imported AI edit` section below the manual details.
- The inline edit section preserves existing behavior:
  - Temporal workflow route metadata
  - upscale queue for visual media
  - referenced generation prompt and queue action when referenced generation is available
  - trimming of referenced generation prompts before queueing
  - existing placement intent behavior for timeline referenced shots
- Audio clips continue to hide imported AI edit controls.
- Generated media remains unchanged from the generated inline AI edit slice.

## Non-Goals

- No changes to Temporal workflow registration or queue payloads.
- No changes to fal.ai model selection, mocks, or output schemas.
- No redesign of generated-media inspector behavior in this slice.
- No changes to top-level inspector rail tabs.

## Verification

- Source inspector tests prove imported visual media no longer renders `Details` or `AI Edit` tab
  buttons.
- Source inspector tests prove imported file/trim controls and AI edit controls are visible together.
- Existing referenced-generation and upscale queue tests pass without clicking an inspector-mode tab.
- Editor workspace tests continue to prove imported timeline clip queue actions create
  Temporal-backed project actions.
- Browser QA checks an imported timeline clip inspector for visible details plus inline AI edit
  controls, with no nested imported inspector tab.
