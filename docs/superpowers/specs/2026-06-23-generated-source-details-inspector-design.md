# Generated Source Details Inspector Design

## Context

Palmier's source inspector shows selected generated media with a `Details` tab, reference thumbnails, generated metadata, and the original prompt beside the timeline. Video Creater already has `SourceClipInspector` tabs for generated clips and the ability to queue variations. The missing piece is a denser, more Palmier-like generated details view that lets editors understand what produced the selected clip without opening the media bin.

## Goals

- Keep the right inspector focused on the selected timeline item.
- Show file metadata for the generated output: type, duration, dimensions, frame rate, and path.
- Show generated metadata: model, aspect ratio, resolution, duration, and prompt.
- Present first frame, last frame, references, and output as compact thumbnail rows with reveal actions.
- Preserve existing AI Edit behavior and queued variation payloads.

## Non-Goals

- No backend schema changes.
- No media thumbnail decoding or waveform rendering.
- No new timeline mutation behavior.
- No automatic replacement or regeneration from the details tab.

## UI Behavior

When a selected source clip resolves to a generated asset, the `Source Inspector` keeps the existing `Details` and `AI Edit` segmented control. In `Details`, the panel shows:

1. A `File` section with rows for type, duration, dimensions, frame rate, and path.
2. A `References` section with compact media cards for first frame, last frame, each reference media id, and output.
3. A `Generated` section with rows for model, aspect ratio, resolution, and duration.
4. A `Prompt` section with the source prompt in a scrollable text block.

Reference cards stay clickable when `onRevealSource` is available. Each card has a stable accessible label so tests and keyboard users can identify the relationship, for example `Reveal first frame media-1`.

## Data Flow

`SourceClipInspector` derives all displayed values from existing props:

- `item` for timeline timing, label, source range, and selected media id.
- `media` for output and reference file metadata.
- `generatedAssets` for prompt, model, settings, references, and outputs.

No new props are required.

## Testing

- Unit test generated details render file rows, generated rows, prompt, and reference cards.
- Unit test reference reveal buttons still call `onRevealSource` with first frame, last frame, reference, and output media ids.
- Existing AI Edit and trim tests must keep passing.

## Acceptance Criteria

- Selecting a generated source clip shows file, references, generated metadata, and prompt in the right inspector.
- The model and resolution are readable without truncating the essential value.
- Reference reveal actions continue to work.
- Focused inspector tests, full frontend tests, lint, and browser visual QA pass.
