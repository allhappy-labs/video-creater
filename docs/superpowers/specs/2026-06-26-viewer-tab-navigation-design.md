# Viewer Tab Navigation

## Context

Palmier's editor screenshots show the center viewer as a tabbed workspace with small navigation controls before the open source tabs. Video Creater already has timeline/source viewer tabs, multiple opened source tabs, source details, and close actions. The missing affordance is quick keyboard/mouse-friendly movement through the open viewer tabs without aiming at each tab label.

## Goal

Add compact previous/next viewer-tab controls to `PreviewPanel` so manual editors can move between the timeline view and opened source tabs from the viewer header.

## Behavior

- The viewer tab strip shows icon buttons for `Previous viewer tab` and `Next viewer tab` before the `Timeline` tab.
- The navigation order is `Timeline`, then each opened source tab in the order already supplied by `sourceTabs`.
- Previous is disabled on the first item. Next is disabled on the last item.
- Clicking a navigation button calls the existing viewer callbacks:
  - timeline targets call `onSelectViewerMode("timeline")`
  - source targets call `onSelectSourceTab(sourceId)` and `onSelectViewerMode("source")`
- The controls use accessible labels and titles and stay compact enough for the dense editor chrome.

## Non-Goals

- No persisted viewer history stack.
- No new project file fields.
- No changes to source tab opening, closing, or source selection ownership in `EditorWorkspace`.
- No browser-level back/forward behavior.

## Tests

- `PreviewPanel` renders previous/next controls in the `Viewer tabs` tablist.
- The controls navigate from timeline to the first source tab, then to the next source tab.
- Boundary controls are disabled at the first and last viewer items.
- Existing preview panel tests continue passing.
