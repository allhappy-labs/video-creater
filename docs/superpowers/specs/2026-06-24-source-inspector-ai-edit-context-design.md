# Source Inspector AI Edit Context Design

## Context

Palmier keeps generated media editable in place: when a generated clip is selected, the side
inspector shows the prompt, model, first frame, last frame, references, and AI edit controls in the
same workflow. Video Creater already has generated details and AI Edit tabs, but the AI Edit tab
does not carry the generation context forward. Editors must switch back to Details to inspect the
frames and references they are about to reuse.

## Goal

Make the Source Inspector AI Edit tab context-aware for generated clips. While tweaking a generated
clip, the editor should still see the source model, output, first/last frame, and reference media
that will be reused for a rerun or variation.

## UI Behavior

In the generated `AI Edit` tab:

- Show a compact `Generation context` block above the prompt controls.
- Include model, output, and settings summary rows.
- Render the same revealable first frame, last frame, reference, and output cards used by Details.
- Keep queue actions unchanged: upscale, single variation, and variation set still call the existing
  callbacks with the same payloads.

The block should be dense enough for the right rail and should not duplicate the full file section
from Details. It is a working context aid, not a second details page.

## Data Flow

Use existing `SourceClipInspector` props only:

- `generatedAssets` for model, settings, prompt, and references.
- `media` and `mediaPreviewUrls` for thumbnails and filenames.
- `onRevealSource` for reveal actions.

No project schema, command, or backend changes are required.

## Testing

- Add a Source Inspector unit test that opens `AI Edit`, finds `Generation context`, and verifies the
  model, output, first frame, last frame, reference, and output cards are visible.
- Verify reveal actions still call `onRevealSource` from inside the AI Edit tab.
- Existing AI edit tests for variation, variation sets, and upscale must keep passing.

## Acceptance

- Generated clip AI Edit displays generation context without leaving the tab.
- First/last/reference/output reveal actions work from AI Edit.
- Focused Source Inspector tests, frontend tests, lint, diff check, and secret scan pass.
