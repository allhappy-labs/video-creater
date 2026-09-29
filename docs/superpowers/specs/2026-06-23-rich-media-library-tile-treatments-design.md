# Rich Media Library Tile Treatments

## Context

Palmier's media panel is thumbnail-first: generated videos, imported videos, images, and audio clips are visually distinguishable before the user reads filenames. Video Creater already has project media cards, duration badges, generated badges, folders, and search, but media artwork is still mostly a flat placeholder with an icon.

## Goal

Make project library media cards communicate asset type through compact thumbnail treatments that work without decoded thumbnails.

## Behavior

- Video and generated media tiles use a film-frame treatment with edge perforations and a subtle scan line.
- Generated media keeps the visible `AI` badge and gains a distinct generated treatment.
- Image media tiles use an image-frame treatment.
- Audio media tiles use a waveform-style thumbnail treatment.
- Filename, metadata, duration badge, selection, and click behavior remain unchanged.

## Non-Goals

- No real thumbnail extraction or media decoding.
- No project schema change.
- No media import, probing, or cache changes.
- No folder layout changes.

## Tests

- Audio media tiles expose an accessible waveform thumbnail.
- Image media tiles expose an accessible image thumbnail.
- Generated media tiles expose an accessible generated video thumbnail and keep the `AI` badge.
