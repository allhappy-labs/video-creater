# Split Project Source Range Duration Validation

## Context

Video Creater already blocks manual Source Inspector trim edits when `sourceOut - sourceIn` does not
match the timeline item `durationSeconds`, and the Rust project action reducer rejects the same
invalid trim action. The text-file-editable split-project validator still only enforces that
duration relationship for generated-edit clips, so an imported media clip hand-edited in
`timeline.json` can describe ambiguous playback state.

Palmier-style editing keeps timeline-native trims inspectable and agent-editable, but until Video
Creater has explicit retiming or speed fields, a media clip's selected source span must match its
timeline duration.

## Goal

Make split-project validation catch mismatched source spans for all media-backed timeline clips that
declare both `sourceIn` and `sourceOut`.

## Behavior

- If neither source boundary is present, existing clip-duration-only validation remains valid.
- If both source boundaries are present, `sourceOut` must be greater than `sourceIn`.
- If both source boundaries are present, `sourceOut - sourceIn` must match `durationSeconds` within
  the existing validation tolerance.
- The generated-edit-only full-source-pass-through rule remains unchanged.
- The validator reports the issue at `properties.sourceOut`, matching the existing source-range
  validation location.

## Non-Goals

- No retiming or speed schema.
- No UI changes.
- No project action behavior changes.
- No renderer behavior changes.

## Verification

- `project_split` tests prove an imported media clip with a mismatched source span fails validation.
- Existing generated-edit source-range validation tests continue passing.
