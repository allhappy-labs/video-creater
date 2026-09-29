# Palmier Media Rail Shell Flattening Design

## Context

The media library already has Palmier-style toolbar, folder band, single grid, and compact generated
asset chrome. The remaining outer `Card` wrapper still makes the whole source column read as a
nested panel inside the editor rather than a direct media rail.

## Goal

Flatten the `MediaBin` outer shell so the source library panel owns the rail framing and media
controls sit directly in the editor column.

## Behavior

- `MediaBin` exposes a direct `Media browser` region for the source rail content.
- The media browser no longer uses the shadcn card root classes: `rounded-md`, `border`,
  `bg-card`, `text-card-foreground`, or `shadow-sm`.
- The existing media toolbar, project library band, generation sheet, folder manager, media grid,
  generated assets, empty states, and import errors remain in the same reading order.
- The `Project library`, `Media library actions`, `Project media grid`, `AI generations`, and
  `Media generation` accessible names remain unchanged.
- No media, folder, generation, Temporal, fal.ai, timeline, or project schema behavior changes.

## Verification

- `MediaBin` tests assert the direct `Media browser` region exists and does not carry card shell
  classes.
- Existing media-bin tests continue proving toolbar, folders, search, generation, generated assets,
  replacement, insertion, mock completion, and retry behavior.
- Browser QA checks the source rail at desktop width for a flatter source column without lost
  controls.
