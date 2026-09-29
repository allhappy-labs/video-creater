# Palmier Media Tile Caption Simplification Design

## Context

Palmier's media browser is thumbnail-first. Tiles show the media image or waveform, short filename text, and compact badges such as `AI` or duration. Video Creater already has rich thumbnails, generated status strips, folders, search, and a right Source Inspector for full metadata, but each project media tile still renders a second text line such as `video - 1920x1080 - 24 fps`.

That second line makes the media grid read more like a file table and adds vertical noise in the left source rail. Metadata remains useful, but it is already searchable and visible in the Source Inspector, preview transport, generated details, and timeline provenance.

Palmier reference: https://www.palmier.io/docs

## Goal

Simplify project media grid tile captions to a single filename line while preserving thumbnail badges and metadata-backed behavior.

## Requirements

- Project media grid tiles show the filename below the thumbnail.
- Project media grid tiles do not show a second visible metadata line for kind, dimensions, or frame rate.
- Duration badges, generated `AI` badges, generated status strips, and thumbnail treatments remain visible.
- Search still matches media kind, filename, folder label, and relative path.
- Generation reference slots and generated output summaries keep their existing metadata text; this change is only for the primary project media grid.
- No new controls, toggles, switches, or details panels are added.

## Testing

- Update MediaBin tests so imported, generated, image, and audio grid tiles do not expose visible metadata captions under the filename.
- Keep existing tests for duration badges, generated `AI` badges, generated status strips, kind-specific thumbnails, real preview thumbnails, and search matching.
- Run focused MediaBin tests, full MediaBin tests, typecheck, full Vitest, diff check, and browser QA of the source media grid.
