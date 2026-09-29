# Generated Output History Cards

## Context

Palmier's media workflows make generated assets visible and reusable from the media panel and timeline: users can inspect prompts, references, model settings, rerun outputs, and swap generated clips without leaving the project. Video Creater already tracks generated assets and completed outputs, but the `AI generations` section still renders completed outputs as text-first rows.

## Goal

Render completed generation outputs as compact media cards inside `AI generations` so generated clips feel like reusable project assets, not log lines.

## Behavior

- Each completed output keeps the existing select action and accessible label.
- When a matching media record exists, the output card shows the same thumbnail treatment used by the project media list.
- Generated video outputs show an `AI` badge and a duration badge beside the filename.
- The card continues to show media id and readable resolution, duration, and fps metadata.
- Replace and insert timeline actions remain directly below the output card.
- If the media record is missing, the card falls back to the current text metadata layout.

## Non-Goals

- No project JSON schema changes.
- No Temporal or fal.ai workflow changes.
- No new media preview loading path.
- No change to generation retry or variation behavior.

## Tests

- Extend the Media Bin generated-output test to require the generated thumbnail, AI badge, duration, file label, metadata, and existing selection behavior.
