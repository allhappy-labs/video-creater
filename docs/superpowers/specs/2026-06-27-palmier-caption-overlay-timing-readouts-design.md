# Palmier Caption And Overlay Timing Readouts

## Goal

Make caption and text-overlay timing readouts feel like inline inspector facts instead of small nested cards. Palmier-style timeline editing should keep selected item context visible in the right rail without wrapping every read-only value in bordered slabs.

Palmier reference: https://www.palmier.io/docs

## Requirements

- Caption inspector keeps showing cue start and end times.
- Text overlay inspector keeps showing overlay start and end times.
- Timing readouts are exposed as accessible groups: `Caption timing` and `Text overlay timing`.
- Timing value rows must not use rounded borders, muted background slabs, or padded card styling.
- Editable controls remain visually distinct: caption textarea, overlay start/duration inputs, overlay text, visual treatment, motion, safe zone, and avoid fields keep their current control styling.
- Apply actions and validation behavior remain unchanged.

## Non-Goals

- No timeline data model, project file, Temporal workflow, generation, or render changes.
- No changes to caption density warning rules.
- No changes to overlay field validation or proposal metadata requirements.
- No changes to timeline clip rendering.

## Testing

- `CaptionInspector` verifies the timing group is present and its start/end rows are flat.
- `TextOverlayInspector` verifies the timing group is present and its start/end rows are flat.
- Existing apply and warning tests continue to cover unchanged behavior.
- Component tests are sufficient for this narrow right-rail change because the changed UI is deterministic from selected item props.
