# Timeline Track Accent Rails Design

## Intent

Palmier's timeline makes lane identity visible at a glance with compact track labels and colored lane cues. Video Creater already uses compact labels such as `V1`, `H1`, `O1`, `C1`, and `A1`, but the row itself still reads as a generic editor band. Add a thin track-kind accent rail to each timeline row so manual editors and agents can scan video, generated-scene, overlay, caption, and audio lanes without opening track controls or clip details.

## Requirements

- Render a visible track-kind accent rail in each left track header row.
- Render a matching track-kind accent rail at the start of each editable canvas row.
- Use stable, distinct colors by track kind:
  - video: cyan/blue
  - HyperFrames: violet
  - overlay: amber
  - caption: sky/blue
  - audio: green
- Preserve compact lane labels and full track names.
- Keep rails non-interactive so they do not block track lock/media controls, canvas seeking, template drops, clip selection, dragging, or resizing.
- Expose deterministic labels for QA in the form `Timeline <lane> <track kind> track accent rail` and `Timeline <lane> <track kind> canvas accent rail`.
- Preserve existing drag target accepted/rejected feedback.

## Non-Goals

- No timeline schema changes.
- No track color customization or persistence.
- No clip color changes.
- No snapping, split, trim, render, Temporal, or asset-generation changes.
