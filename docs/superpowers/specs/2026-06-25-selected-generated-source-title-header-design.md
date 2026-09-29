# Selected Generated Source Title Header

## Context

Palmier's source inspector treats generated media as named source objects: the header shows the
selected generation title, an AI cue, and the detailed file path below it. Video Creater already
renders rich generated-source details in the Media panel, including file metadata, references,
workflow status, prompt, and AI-edit actions. The header still leads with the generic label
`Generated source`, so selected AI outputs feel less like first-class editor media than Palmier's
source panel.

## Goal

Make the selected generated source header identify the generated asset by name or prompt title,
while preserving the output path and status context.

## Behavior

- In the selected generated source panel, show the generated asset title as the primary header text.
- Keep a compact `AI` badge next to the title.
- Keep `Generated source` as supporting context, not the primary title.
- Keep the generated output relative path visible below the title.
- If the generated asset has no explicit name, use the existing generated asset title fallback.
- Do not change generated source actions, prompt copy, references, workflow status, or project data.

## Non-Goals

- No source inspector tab redesign.
- No project schema or generated asset metadata change.
- No timeline, Temporal, queue, or fal provider change.
- No new title editing UI in this slice.

## Verification

- A Media Bin test selects a named generated output and asserts the generated title is primary in the
  selected generated source region.
- The same test asserts the `AI` badge and output path remain visible.
- Existing selected generated source, generation composer, and media-bin tests keep passing.
