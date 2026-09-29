# Generated Timeline Source Title Header

## Context

Palmier keeps generated clips understandable as source objects even while they are selected on the
timeline: the source rail shows the generated source identity beside the `Details` and `AI Edit`
tabs, while timeline placement remains supporting context. Video Creater now titles generated
library selections with the generated asset name, but selected generated timeline clips still lead
with the timeline item label. That makes the same generated output feel less like a named AI source
once it is placed on the timeline.

## Goal

When a generated timeline clip is selected, make the right-rail `Source Inspector` lead with the
generated asset title while keeping the timeline clip label, timeline range, source range, reason,
and output path visible.

## Behavior

- When the selected timeline item resolves to a generated asset, show the generated asset title as
  the primary inspector heading.
- Keep a compact `AI` badge next to the title.
- Show `Generated timeline source` as the supporting source type.
- Show the timeline clip label and timeline range directly under the source type.
- Keep the generated output relative path visible near the header.
- Keep the existing source range and reason chips for manual editing context.
- Keep all existing details, references, workflow status, output swap, trim, split, and AI edit
  controls unchanged.
- Imported timeline clips and non-generated selected media keep their existing headers.

## Non-Goals

- No generated asset schema change.
- No timeline item label rename or title editing UI.
- No tab redesign.
- No Temporal workflow, queue, or fal provider change.

## Verification

- A Source Clip Inspector test selects a named generated timeline clip and asserts the generated
  title is the primary heading in `Source Inspector`.
- The same test asserts the `AI` badge, `Generated timeline source` context, clip label, timeline
  range, and generated output path remain visible.
- Existing source inspector, generated-source, lint, build, visual, and secret checks pass.
