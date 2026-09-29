# Generation History Reuse Design

## Context

Palmier keeps generated media iteration inside the editor: users can generate assets, inspect prior
results, tweak prompts, and continue working without leaving the project. Video Creater already has a
compact media generation drawer with a `Generation history` panel, but those history cards are
read-only. Editors must manually copy a prior prompt and reset compatible settings before queueing a
variation-style draft from the media panel.

## Goal

Make generation history actionable by letting editors reuse a prior generated asset as the starting
point for a new composer draft.

## Behavior

- Each generation history card exposes `Use prompt`.
- Clicking `Use prompt` copies the asset prompt into `Generation prompt`.
- If the asset has a name, the composer name becomes `<asset name> variation`; unnamed assets leave
  the name field unchanged.
- Compatible asset settings are restored:
  - video assets switch to Video mode and restore duration, aspect ratio, and resolution when those
    values match available composer options;
  - image assets switch to Image mode and restore aspect ratio and resolution when compatible;
  - audio assets switch to Audio mode and restore duration when compatible.
- Compatible references are restored into the matching composer fields:
  - video: first frame, last frame, and first reference media;
  - image: first reference media;
  - audio: no visual references.
- The history panel remains open so the editor can try another history card if desired.
- Submitting after reuse still emits the existing `MediaGenerationRequest` shape. No project action,
  Temporal workflow, provider, or schema contract changes are made.

## Non-Goals

- No persisted draft history.
- No multi-reference composer UI in this slice.
- No automatic timeline replacement or insertion.
- No backend or Temporal changes.

## Tests

- `MediaBin` reuses a video generation history item, restoring prompt, variation name, mode,
  compatible settings, and references, then submits those values.
- `MediaBin` reuses an image generation history item, switches to image mode, restores compatible
  visual settings and reference media, and submits a flux/schnell image request.

## Acceptance Criteria

- Editors can start a new generation from a recent history item with one action.
- The resulting composer draft remains editable before queueing.
- Existing generation drawer, folder, placement, reference, and workflow tests continue to pass.
