# Generated Source Reference Tile Grid Design

## Context

Palmier's generated source inspector shows generation references as visual thumbnails inside the
right rail. Video Creater already exposes first frame, last frame, references, output, prompt, and
settings, but the reference media still render as compact two-column metadata rows. The data is
correct, but the visual hierarchy makes generated provenance harder to scan.

Palmier reference: https://www.palmier.io/docs

## Goal

Render generated-source references as compact visual tiles while preserving reveal and provenance
behavior.

## Behavior

- `Generated references` uses a two-column tile grid on wider right-rail space and a single column
  when constrained.
- First frame, last frame, reference, and output entries keep their accessible group labels and
  reveal buttons.
- Each generated reference tile shows the thumbnail/fallback as the dominant top area and keeps the
  label, filename, media summary, and path below it.
- Real local preview URLs and preview error fallbacks continue to work.
- Imported source media cards keep their existing row layout.

## Non-Goals

- No generated asset schema, reference semantics, or output-selection changes.
- No new media actions, fal.ai calls, Temporal workflows, or queue behavior.
- No change to the details/AI edit tab model.

## Verification

- Add a `SourceClipInspector` test proving generated references use tile grid and tile card classes,
  not row card classes.
- Existing generated provenance, reveal, preview URL, and fallback tests continue to pass.
- Browser-smoke the right rail and confirm generated references display as visual tiles without
  overlapping prompt/settings content.
