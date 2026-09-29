# Palmier Generation Sheet First-Frame Reference Seed Design

## Intent

Palmier's video generation sheet treats first/last frames as the primary video-control inputs, with additional references as an optional explicit choice. Video Creater currently seeds a selected visual media item into both `First frame` and the generic `Reference` slot when opening `Generate media`. That duplicates the same thumbnail and makes the sheet feel busier than the Palmier reference.

## Requirements

- When opening generation from a selected imported visual media item, seed `First frame` only.
- Keep the generic `Reference` slot visible but empty until the user explicitly adds reference media.
- Queue default video generation with `firstFrameMediaId` set and `references.mediaIds` empty.
- Preserve explicit reference media selection, including multiple removable references.
- Preserve generated-asset reuse behavior, where existing provenance references continue to seed the composer.

## Testing

- Update `MediaBin` tests so selected-media video generation queues `mediaIds: []` and `firstFrameMediaId` remains selected.
- Assert the Palmier-style generation sheet shows the selected media in `First frame` but not as a default generic reference.
- Keep manual reference selection tests proving reference media is included only after explicit selection.
- Run focused media-bin tests, workspace integration tests, typecheck, full tests, and browser QA.

## Self-Review

- No project schema, Temporal workflow, or fal.ai provider changes.
- The queued payload stays compatible; only the default reference list changes.
- The generation sheet remains usable for explicit reference-based prompting.
