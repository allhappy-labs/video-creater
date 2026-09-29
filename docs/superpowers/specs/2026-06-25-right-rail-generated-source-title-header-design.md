# Right Rail Generated Source Title Header

## Context

Palmier's right source rail keeps generated media identifiable as generated source objects: the
selected source title stays visible beside the `Details` and `AI Edit` controls, while file path,
references, generated settings, workflow status, and prompt remain available below. Video Creater's
Media Bin selected generated source panel now follows that pattern, but the right-rail
`Source Inspector` still leads selected generated library media with the output filename and the
generic `Library media` label. That makes generated outputs feel less like named AI assets when the
editor is working from the main inspector.

## Goal

Make the right-rail `Source Inspector` identify selected generated library media by the generated
asset name or prompt-derived title while preserving the output file path and existing generated
details.

## Behavior

- When the selected source is a generated library output, show the generated asset title as the
  primary header text.
- Keep a compact `AI` badge next to the title.
- Show `Generated source` as supporting context instead of `Library media`.
- Keep the generated output relative path visible under the header.
- Keep existing generated details, references, workflow status, prompt, tabs, and AI edit actions.
- Imported library media and selected timeline clip headers keep their existing behavior.
- If a generated asset has no explicit name, derive the title from the first prompt line and fall
  back to the generated asset id.

## Non-Goals

- No tab redesign for `Details` or `AI Edit`.
- No generated asset schema change.
- No timeline action, Temporal workflow, or fal provider change.
- No title editing UI in this slice.

## Verification

- A Source Clip Inspector test selects a named generated output and asserts the generated title is
  the primary heading in `Source Inspector`.
- The same test asserts the `AI` badge, `Generated source` context, and generated output path remain
  visible.
- Existing source inspector, generated-source, lint, build, visual, and secret checks pass.
