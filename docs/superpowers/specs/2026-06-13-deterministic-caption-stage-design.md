# Deterministic Caption Stage Design

## Summary

Add a deterministic Rust caption stage to the Video Creater workflow. The stage turns transcript word timestamps into polished, timed, styled caption timeline items before LLM-guided visual layers or rendering. Captions should be fast and reproducible, but transcript text can be wrong, so users must be able to correct generated caption text in the editor.

Rust owns caption segmentation, timing, style metadata, validation, EDL remapping, and canonical timeline mutation. The React UI displays caption cues and provides text editing for user correction. Codex must not write caption text for this first caption pass.

## Goals

- Make captions a first-class workflow step after transcription.
- Keep caption generation deterministic and fast in Rust.
- Reuse the same caption stage inside `Generate edit`.
- Generate readable caption cues from word-level transcript timestamps.
- Add deterministic visual metadata for every caption cue.
- Allow users to edit caption text after generation without rerunning transcription or asking an LLM.
- Preserve timing safety when caption text is edited.
- Add focused Rust tests before implementation.

## Non-Goals

- LLM-authored captions.
- Manual caption creation from scratch in the first implementation.
- Full caption timing editing in the first implementation.
- Burn-in rendering style implementation beyond the metadata contract.
- Speaker diarization UX.
- Per-word karaoke animation.

## Workflow

The app workflow becomes:

1. Import media.
2. Transcribe media into word-level timestamps.
3. Build deterministic captions from transcript words.
4. Build a rough-cut EDL.
5. Remap captions to the edited timeline.
6. Add overlays, title cards, HyperFrames scenes, and other visual layers.
7. Render draft.
8. Review duration, streams, captions, overlays, artifacts, and logs.

The caption stage is not a side utility. It is part of the generation pipeline and can also be rerun when transcript data changes.

## Architecture

### Rust Domain

Create a Rust caption domain module, likely `src-tauri/src/edit/captions.rs`, with a narrow API:

```rust
pub fn build_caption_cues(
    words: &[TranscriptWord],
    source_ranges: &[CaptionSourceRange],
    options: CaptionGenerationOptions,
) -> Vec<CaptionCue>
```

The module should handle:

- filtering words to source ranges
- splitting cues on pauses and sentence punctuation
- enforcing max words per cue
- enforcing max characters per line and per cue
- enforcing min and max cue durations
- avoiding empty or punctuation-only cues
- deterministic two-line breaking
- clamping cue timing to the source range
- mapping source time to output timeline time
- attaching caption style metadata

The first implementation can use fixed defaults, with preset-specific options passed later.

### Timeline Integration

`generate_one_click_edit_timeline` should call the caption module after the EDL is built. The current one-caption-per-clip helper should be replaced by caption cue generation over the EDL ranges.

Caption timeline items continue to use:

```rust
TimelineSource::Text { text }
```

Each caption item should include properties such as:

- `sourceIn`
- `sourceOut`
- `cueIndex`
- `lineBreaks`
- `stylePreset`
- `visualTreatment`
- `motion`
- `safeZone`
- `avoid`
- `generatedBy`
- `textEdited`

The exact property shape can remain JSON-backed for the first implementation, but the values must be produced by Rust, not by Codex.

### User Editing

Users can select a caption item and edit its text in an inspector. The edit updates only the caption text and marks the item as user edited.

The initial edit operation should not automatically retime the caption because timing edits create a larger interaction and validation surface. If the corrected text is much longer than the generated cue, the UI can show a validation warning and recommend splitting or retiming in a future workflow.

The patch path should use existing project mutation patterns rather than bypassing Rust validation. A caption text edit should be rejected if:

- the target item is not a caption
- the new text is empty after trimming
- the item does not exist
- the target track is locked

## Caption Rules

The first deterministic defaults should be conservative:

- Split at sentence punctuation when available.
- Split on pauses longer than roughly 0.45 to 0.60 seconds.
- Keep cues short enough for phone viewing.
- Prefer one or two lines, not dense paragraphs.
- Clamp each cue to the selected source range.
- Keep a small minimum duration for very short words.
- Avoid caption items that cover a whole generated clip when the clip contains many words.

Suggested initial limits:

- max words per cue: 7
- max characters per cue: 42
- max characters per line: 22
- min duration: 0.6 seconds
- max duration: 3.0 seconds
- pause split threshold: 0.55 seconds

These are implementation defaults, not user-facing settings.

## Visual Metadata

Every generated caption must include the visual contract required by the project graphics policy:

- `visualTreatment`
- `motion`
- `safeZone`
- `avoid`

The first preset can be named `boldReadableLower`. Example values:

- `visualTreatment`: bold phone-readable lower-third caption with subtle translucent backing and accent emphasis
- `motion`: quick pop-in, hold, and soft fade out
- `safeZone`: keep essential text inside 10% margins and above platform controls
- `avoid`: full-width opaque black slabs, centered text-only cards, faces, hands, product details, and main action

This metadata is deterministic. It gives later render and preview work a stable contract without asking an LLM to invent caption styling.

## UI Behavior

The timeline should show generated caption items on the caption track. Selecting a caption opens or updates an inspector area where the user can:

- read the current caption text
- edit the caption text
- see cue start and end times
- see whether the text has been user edited
- see a warning when the edited text is too long for the cue duration

The UI should not expose implementation details about the generator. It should feel like a video editor correction surface: select cue, edit text, apply.

## Data Flow

```text
Transcript words
  -> Rust caption segmenter
  -> source-time caption cues
  -> EDL remapper
  -> caption timeline items
  -> React timeline and inspector
  -> validated caption text patch
  -> canonical project timeline
```

For a full-source caption pass, the source range is the full media duration. For generated edits, the source ranges come from the EDL and include output timeline offsets.

## Error Handling

Caption generation should fail clearly when:

- transcript words are missing
- source ranges are empty
- source ranges have invalid durations
- a source range references a missing media id
- no caption track exists

Text editing should fail clearly when:

- the selected item is missing
- the selected item is not a caption
- the caption track is locked
- the submitted text is empty

Warnings should be non-blocking when:

- edited caption text exceeds the recommended character limit
- edited caption reading speed is likely too high for the cue duration

## Testing

Use test-driven development.

Rust tests should cover:

- words split into multiple cues by max word and character limits
- pauses split cues deterministically
- punctuation ends a cue without leaving punctuation-only captions
- cue timing clamps to source ranges
- EDL source ranges remap captions to output timeline time
- each generated caption includes required visual metadata
- `generate_one_click_edit_timeline` creates more than one caption for word-heavy clips
- caption text edits update only text and `textEdited`
- caption text edits reject non-caption items and empty text

Frontend tests should cover:

- selecting a caption item exposes editable caption text
- applying a text correction calls the expected edit handler
- long edited text shows a warning without corrupting timing

## Implementation Notes

- Keep the caption generator pure and independently testable.
- Prefer typed Rust structs for generation options and internal cue output.
- Convert to JSON timeline properties only at the timeline boundary.
- Do not require Codex or a sidecar to generate captions.
- Preserve user-edited caption text when rerunning later stages unless the user explicitly regenerates captions.
- The first implementation can use sample transcript fixtures rather than real media.

## Acceptance Criteria

- `Generate edit` uses the deterministic caption stage instead of one caption per clip.
- Captions are split into readable cue-sized timeline items.
- Every generated caption includes deterministic visual metadata.
- Users can correct caption text through a validated project mutation path.
- Rust tests prove segmentation, EDL remapping, metadata, and edit validation.
- Existing one-click edit behavior remains EDL-first and does not become a full-source pass-through.
