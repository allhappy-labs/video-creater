# Media Root Flat Grid Design

## Purpose

Bring the project media browser closer to Palmier's source panel. The library chrome already shows folder cards for navigation, so repeating the same folder names as large section headers below the chrome makes the root browser heavier than Palmier's compact grid.

## Behavior

- At the project-library root, render visible media as one flat thumbnail grid, regardless of folder assignment.
- Keep folder cards in the library chrome so users can drill into a folder.
- Keep active-folder browsing scoped to the selected folder and its nested child folders.
- Keep nested child-folder labels inside active-folder views, because they explain hierarchy after the user has chosen a folder scope.
- Preserve search behavior: root search filters the flat grid project-wide, and active-folder search remains scoped.

## UI Details

The root view should read as: toolbar, count, folder navigation cards, then a compact media grid. It should not show duplicate uppercase `Generated selects`, `B-roll`, or `Unfiled` section headings under the folder cards.

## Testing

- Update the root folder rendering test to assert all root media tiles remain visible in one `Project media grid`.
- Assert root folder, B-roll, and unfiled section groups are absent below the chrome.
- Keep existing active-folder and nested-folder tests so scoped navigation remains intact.
