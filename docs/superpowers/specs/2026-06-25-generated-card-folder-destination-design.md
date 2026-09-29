# Generated Card Folder Destination

## Context

Palmier's media-panel workflow lets editors generate media and organize those generations inside
project-library folders. Video Creater already records `targetFolderId` for folder-scoped
generations and completes generated output media into that folder, but the AI generations list does
not show the target folder after queueing.

Palmier reference: https://www.palmier.io/docs

## Goal

Keep folder-organized generations visible in the AI generations list so editors can confirm where a
queued or completed generated asset belongs without opening the folder or inspecting project files.

## Behavior

- If a generated asset has a `targetFolderId` that matches a known media folder, its AI generation
  card shows a compact `Destination` badge.
- Nested folder paths use the same label format as the rest of the media bin, such as
  `Generated selects / Scene A`.
- Generated assets without a known target folder keep the existing card layout.

## Non-Goals

- No project schema or Rust action change.
- No new folder assignment action for generated sidecars.
- No behavior change for generated output media folder assignment on completion.

## Verification

- Media Bin test: a generated asset with `targetFolderId` renders the matching nested destination
  label in the AI generations list.
- Existing folder-scoped generation, generated-card, and media-folder tests continue passing.
