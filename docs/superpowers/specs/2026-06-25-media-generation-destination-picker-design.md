# Media Generation Destination Picker

## Context

Palmier's media panel keeps generated images, video, and audio in the project library and lets users
organize generations into folders. Video Creater already queues media generations from the media
panel and can target the active project library folder, but folder targeting is implicit: users must
navigate into a folder before opening the composer. From the all-library view, the request has no
direct destination control even when the project already has a generated-assets folder structure.

## Goal

Make the media generation composer expose an explicit project-library destination picker so queued
generations can be filed into any existing folder from the composer.

## Behavior

- When project library folders exist, show a compact `Destination` control in the media generation
  composer.
- Include a root option for the top-level project library and every existing folder.
- Show nested folders with their full path labels, such as `Generated selects / Scene A`.
- Default the destination to the active project library folder when the composer opens from inside a
  folder.
- Default the destination to the project library root when the composer opens from the all-library
  view.
- Submit the selected destination as `targetFolderId` on `MediaGenerationRequest`.
- Keep the existing library/timeline placement intent unchanged; the destination picker controls
  where generated assets are filed, not whether the output should also be inserted on the timeline.

## Non-Goals

- No folder creation inside the generation composer.
- No generated asset schema change.
- No Temporal workflow, provider, or media import change.
- No changes to generation references, model choices, duration, aspect ratio, or resolution.

## Verification

- A Media Bin test opens the generation composer from the all-library view, chooses a nested
  destination folder, queues a generation, and asserts `targetFolderId` is the selected folder.
- Existing active-folder generation targeting continues to pass.
- Media Bin tests, lint, build, diff, and secret scans pass.
