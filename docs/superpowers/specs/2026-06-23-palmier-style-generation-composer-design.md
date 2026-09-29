# Palmier-Style Generation Composer

## Context

Palmier treats generation as a native editor workflow: users can import or generate media in the project library, choose image/video/audio mode, provide first/last frame and reference inputs, tune model output settings, and generate without leaving the timeline workspace. Video Creater already has the underlying generated-asset action contract and a compact media generation form, but the form reads as a generic settings panel rather than a production editing composer.

Current gaps:

- Reference inputs are plain selects with small text chips instead of visual slots.
- Model, duration, resolution, aspect ratio, and cost/status are not presented as one generation footer.
- The primary action is correct but does not feel connected to timeline-native generation.
- The existing form is dense, but not scan-friendly enough for repeated editing work.

## Goal

Redesign the existing `MediaBin` generation composer into a compact Palmier-style composer while preserving the current request payload and split-project persistence behavior.

## Behavior

- The composer remains inside the Media panel and opens from `Generate media`.
- Image, Video, and Audio modes remain a segmented control.
- Video mode shows three visual reference slots: `First frame`, `Last frame`, and `Reference`.
- `First frame` and `Last frame` render as a paired visual slot group in the video composer,
  matching the side-by-side setup in the Palmier reference flow.
- Image mode shows one visual reference slot: `Reference`.
- Audio mode hides visual reference slots and disables visual output settings.
- Each visual slot shows either an empty state or a selected media preview card with filename, kind metadata, duration, and a remove button.
- The selected media should prefill video `First frame` and `Reference` when the composer opens if the current selection is visual.
- Prompt input remains required and uses the existing `Generation prompt` accessible label.
- Editable model, duration, aspect ratio, and resolution controls live in the composer footer,
  colocated with the generated settings summary and queue action.
- The footer summarizes the selected model, duration, aspect ratio, resolution, and an estimated credit cost before the action.
- Submitting still emits the existing `MediaGenerationRequest` shape:
  - `kind: "generated"`
  - image requests use `fal.ai/fal-ai/flux/schnell`
  - video requests use `fal.ai/fal-ai/wan-25-preview/text-to-video`
  - audio requests use `elevenlabs/music-v1`
  - references and settings match the selected mode and controls

## Visual Direction

The component should feel like a focused editor instrument:

- dark, compact, utilitarian
- reference slots as actual visual targets, not data-entry fields
- tiny badges for model, credits, duration, and output shape
- no decorative marketing copy
- no nested cards; framed controls are acceptable inside the existing Media panel

## Non-Goals

- No new generated media execution backend.
- No schema changes.
- No automatic insertion of queued generations onto the timeline.
- No drag-and-drop reference assignment yet.
- No real credit accounting; the displayed estimate is deterministic UI metadata.

## Data Flow

1. `MediaBin` owns composer state for mode, references, duration, aspect ratio, resolution, and prompt.
2. Opening the composer seeds reference fields from the selected visual media.
3. Slot remove buttons clear the corresponding reference field.
4. Submit builds the same `MediaGenerationRequest` currently sent to `EditorWorkspace.queueMediaGeneration`.
5. `EditorWorkspace` continues to record a queued generated asset through `recordGeneratedAsset`.

## Tests

- Existing media generation request tests must continue to pass.
- Add focused tests that prove:
  - video mode renders visual reference slots with selected media metadata
  - audio mode hides visual slots and preserves the audio model/settings payload
  - the composer footer shows model, output shape, and estimated credits
  - clearing a first-frame slot removes `firstFrameMediaId` from the request

## Acceptance Criteria

- The composer visually resembles the Palmier reference flow: mode tabs, visual reference inputs, prompt, compact model/settings footer, and a clear queue action.
- The request payloads for video and audio generation remain compatible with existing project actions.
- No existing media bin, generated source, folder, or workspace tests regress.
- Browser QA captures the open composer and verifies no overlapping text at the default desktop viewport.
