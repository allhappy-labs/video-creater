# Generated Inspector AI Edit Inline Design

## Context

Palmier keeps the right inspector direct: selected media metadata, provenance, and edit actions are
visible without a second nested inspector mode. Video Creater still renders a generated-source
`Details / AI Edit` segmented toggle inside the Source Inspector. That extra switch hides variation
and replacement controls, adds an editor mode that Palmier does not have, and makes generated clips
feel less directly editable.

Palmier reference: https://www.palmier.io/docs

## Goal

Remove the generated-source nested `Details / AI Edit` switch. Generated clips should show their
details and AI edit controls in one continuous inspector surface.

## Behavior

- Generated media in the Source Inspector does not render `Details` or `AI Edit` tab buttons.
- `Generated details` remains visible by default with model, output, references, and prompt
  provenance.
- Generated AI edit controls render as an inline `Generated AI edit` section below details.
- The inline edit section does not duplicate the generated reference/model context already visible
  in details.
- The inline edit section preserves existing behavior:
  - variation prompt editing and restore action
  - single variation queue
  - replacement variation queue when a timeline item is selected
  - variation-set queue mode
  - generated output upscale queue
- The existing `Single / Variation set` mode control remains inside the edit section because it
  chooses the generation request shape, not the inspector view.
- Imported-media AI edit keeps its current `Details / AI Edit` switch in this slice.

## Non-Goals

- No changes to Temporal workflow registration or queue payloads.
- No changes to fal.ai model selection, mocks, or output schemas.
- No imported-media inspector redesign in this slice.
- No changes to the top-level inspector rail `Source / Timeline` tabs.

## Verification

- Source inspector tests prove generated media no longer renders `Details` or `AI Edit` tab buttons.
- Existing generated rerun, variation, replacement, variation-set, upscale, and context tests pass
  with generated details and edit controls visible at the same time.
- Editor workspace tests continue to prove generated clip queue actions create Temporal-backed
  project actions.
- Browser QA checks the selected generated clip inspector for visible details plus inline edit
  controls, with no nested generated inspector tab.
