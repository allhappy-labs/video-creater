# Generation Reference Preview Tiles Design

## Context

Palmier's generation composer treats first frame, last frame, and reference inputs as visual
targets: once a frame or reference is chosen, the preview is the dominant element. Video Creater
already has the same generation semantics, but selected references still render as compact metadata
rows. That makes it harder to scan the visual anchors that drive generation.

Palmier reference: https://www.palmier.io/docs

## Goal

Make selected generation references read as media tiles while preserving existing picker, drag/drop,
remove, and submit behavior.

## Behavior

- Selected `First frame` and `Last frame` previews render as full-width visual tiles inside their
  slots.
- Selected general `Reference` media render as visual cards in the reference list, not thumbnail
  metadata rows.
- Each tile keeps filename, media metadata, duration badge, thumbnail fallback, and accessible group
  labels.
- Existing slot controls, `Use selected`, remove actions, drag/drop handlers, and generation payloads
  remain unchanged.
- Empty slots keep the current dashed drop target treatment.

## Non-Goals

- No new media picker, batch selection, or reference semantics.
- No fal.ai, Temporal, workflow, or project schema changes.
- No change to audio mode, which remains without visual reference controls.

## Verification

- Add a `MediaBin` test proving selected generation references use tile classes instead of row
  classes for first frame and reference list previews.
- Existing reference slot, drag/drop, and submit payload tests continue to pass.
- Browser-smoke the focused generation sheet and confirm selected reference previews are visibly
  tile-like.
