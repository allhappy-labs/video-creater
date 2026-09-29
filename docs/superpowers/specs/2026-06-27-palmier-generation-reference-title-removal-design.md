# Palmier Generation Reference Title Removal Design

## Context

Video Creater's generation composer previously kept selected first, last, and reference media
visible in a read-only `Generation references` summary. That summary rendered a visible
`Generation references` title above rows that already labeled each reference. Palmier-style editor
chrome favors compact, directly scannable metadata over repeated internal section titles.

Superseded by `2026-06-27-palmier-generation-reference-summary-removal-design.md`: the standalone
summary was removed after the active generation recipe became the canonical read-only summary.

Palmier reference: https://www.palmier.io/docs

## Goal

Remove the redundant visible reference-summary title while preserving the accessible summary group
and every reference row.

## Behavior

- The open `Media generation` composer exposes reference state through editable slots and the
  `Active generation recipe`, not a standalone `Generation references` group.
- Video summaries still show first frame, last frame, and reference rows.
- Image summaries still show only reference rows.
- Audio generation still hides the reference summary.
- The superseding removal keeps reference state visible through the active recipe and editable
  slots, so no standalone `Generation references` title or group remains.
- Reference slots, prompt, recipe metadata, request payloads, queue behavior, Temporal records,
  fal.ai provider calls, and generated asset metadata remain unchanged.

## Non-Goals

- No reference summary data redesign.
- No reference slot layout changes.
- No schema, provider, queue, Temporal, or generated asset behavior changes.

## Verification

- Update `MediaBin` coverage to assert the standalone `Generation references` group is absent
  while the active recipe still shows reference rows.
- Keep existing editable reference slot coverage passing for video, image, and audio modes.
- Browser QA the open generation sheet and confirm reference state remains visible without the
  old prompt-adjacent summary block.
