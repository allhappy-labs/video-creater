# Palmier Media Library Flat Folder Band

## Problem

Palmier presents folders as first-class tiles inside the media browser content. Video Creater now renders useful folder tiles, but they still sit inside a rounded, bordered `Project library` subpanel above the media grid. That extra chrome makes folders feel like a separate navigation widget instead of part of the editable media library.

## Goal

Keep the `Project library` region, item count, folder navigation, and folder tile behavior, but remove the nested panel treatment around the folder band. The folder count row and folder tiles should sit directly in the media browser flow, above the media grid.

## Requirements

- The `Project library` region remains available to accessibility queries.
- The item count and active-folder count remain visible.
- Folder cards remain tile-like with preview, item count, and compact label rows.
- The library band does not render the old rounded border, muted background, or padded panel wrapper.
- Existing search, folder navigation, and media grid behavior remain unchanged.

## Non-Goals

- Do not change folder hierarchy semantics or media assignment behavior.
- Do not remove the active-folder navigation buttons.
- Do not redesign individual media asset tiles.
- Do not introduce new sorting, filtering, or view toggles.

## Verification

- Component test asserts the `Project library` region remains present but no longer carries the old bordered wrapper classes.
- Focused media-bin tests pass.
- Lint and `git diff --check` pass.
- Browser QA checks the media panel and editor layout for obvious overlap or unreadable controls.
