# Palmier Source Library Tab Removal

## Context

Palmier keeps the left library as a direct asset browser: import, folder, generate, and search controls
sit above a continuous grid of folders and media. Video Creater still renders a large `Media` /
`Templates` tab strip above the media toolbar. That tab strip makes templates feel like a separate
panel mode and consumes prime vertical space in the editor column.

Palmier references: supplied editor screenshots and https://www.palmier.io/docs

## Goal

Remove the source library mode switch while keeping both media management and motion template
insertion available from the left library.

## Behavior

- The source library panel no longer renders the `Source panel` tablist.
- The persistent `Media` and `Templates` tab buttons are removed from the library chrome.
- Media folders, imported media, generated assets, generation queue controls, and search remain visible
  as the primary library surface.
- Motion and shader templates render below the media library as a library section instead of behind a
  separate tab.
- Template insertion, template drag/drop, shader background insertion, Codex template selection, and
  project-action persistence keep their existing contracts.
- Revealing source media from the inspector no longer needs to switch a source-panel tab; it still
  selects and highlights the media item.
- No project schema, timeline item, render, Temporal workflow, or media-generation provider contract
  changes.

## Verification

- `EditorWorkspace` tests assert that the source panel tablist and `Media` / `Templates` tabs are absent.
- Template insertion tests use the always-visible template cards instead of switching tabs.
- Existing media reveal tests keep proving the selected media card is highlighted.
- Browser QA confirms the left source library starts at the toolbar and shows templates as a lower
  library section without the tab strip.
