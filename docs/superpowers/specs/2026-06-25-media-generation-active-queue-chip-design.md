# Media Generation Active Queue Chip

## Context

Palmier keeps generation state close to the media composer: users can generate from the media
panel, see recent work, and keep editing while queued or running clips resolve. Video Creater now
has a Palmier-style media generation composer with credits, history, model/settings, Temporal route
metadata, and mock completion controls on generated asset cards.

The composer header does not summarize active queued or running generations. When history is closed
and generated cards are below the fold, an editor can queue multiple generations without an
immediate queue-status cue in the composer itself.

## Goal

Show a compact active generation queue chip in the media generation composer header whenever there
are queued or running generated assets.

## Requirements

- Count generated assets whose status is `queued` or `running`.
- Render no queue chip when there are no active generated assets.
- Render singular copy for one active item and plural copy for multiple active items.
- Keep the chip compact and non-interactive, beside the existing mock credit and estimate chips.
- Do not change generated asset storage, workflow jobs, mock completion, fal.ai requests, history,
  timeline placement, or media cards.

## Verification

- MediaBin test proves the composer header shows `1 active generation` for one queued/running
  generated asset.
- MediaBin test proves the composer header shows plural copy for multiple active generated assets
  and omits completed assets from the count.
- Existing MediaBin tests continue passing.
