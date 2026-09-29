# Imported Source Inspector AI Edit Tabs

## Context

Palmier keeps the right source rail compact by separating source metadata from AI edit
actions with `Details` and `AI Edit` tabs. Video Creater now uses that pattern for
generated sources, but imported visual media still renders file details and imported AI
edit controls in one continuous stack.

## Goal

Apply the same right-rail tab pattern to imported visual sources that can be upscaled or
used as a referenced generation input. The default view remains source details, while
AI edit actions are one tab away.

## Requirements

- Imported visual Source Inspector content renders `Details` and `AI Edit` tabs when
  referenced-generation or upscale actions are available.
- `Details` is selected by default and keeps insert-on-timeline and file metadata visible.
- `AI Edit` shows the existing upscale and referenced-generation prompt controls.
- Switching between `Details` and `AI Edit` preserves the referenced-generation prompt
  draft while the selected media source remains the same.
- Imported audio sources and imported sources with no AI edit actions do not render the
  tab row.
- No project schema, Temporal workflow, fal.ai provider, or project action contract
  changes.

## Verification

- Source Inspector tests assert imported visual sources default to `Details`.
- Source Inspector tests assert imported AI edit controls are hidden until `AI Edit` is
  selected.
- Source Inspector tests assert the referenced-generation prompt survives a tab round trip.
- Existing imported audio tests continue to prove audio sources do not expose visual AI
  edit controls.
- Browser QA confirms the right rail visually matches the generated-source tab pattern
  without overlapping or clipping.
