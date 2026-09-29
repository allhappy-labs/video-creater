# Desktop Editor Chrome and Timeline Actions Design

## Goal

Remove duplicate render controls from the desktop editor header, make project-save state unambiguous, contain media-library content inside its panel, and replace the selected-clip text action strip with accessible icon controls.

## Scope

This change affects the desktop editor workspace only. It does not change the export dialog, render backend, persistence protocol, project model, or narrow-screen action menu.

## Header

The centered header continues to show the project name. Its secondary state is mutually exclusive:

- When the project has unsaved changes, show `Edited` beside the name.
- While a save is in progress, show `Saving` below the name.
- On save failure, retain the existing alert state, `Save failed`.
- When the project is saved, show no success copy; `Saved` disappears.

The desktop header no longer contains `Render quality` or `Render draft` / `Render final`. `Export` remains the single entry point for selecting a destination, codec, quality, and starting an export. Existing render-review controls outside the header are unchanged.

## Media Panel Containment

The source-library panel owns scrolling for its media content. Its media grid/folder cards must remain inside the panel’s available height and may scroll vertically; they must not paint across the timeline tabs or timeline header. The panel shell remains clipped, while the library’s inner content is a minimum-height-zero flex child that can scroll.

## Selected Clip Action Strip

On desktop, retain the selection count as plain text and expose each currently available selected-clip command as a compact icon button:

| Command | Accessible name and tooltip |
| --- | --- |
| Link | `Link selected` |
| Unlink | `Unlink selected` |
| Remove | `Remove selected` |
| Nudge earlier | `Nudge −0.25s` |
| Nudge later | `Nudge +0.25s` |
| Apply grain | `Apply grain` |
| Apply vignette | `Apply vignette` |
| Clear effects | `Clear effects` |
| Decompose sequence, when available | `Decompose sequence` |

Controls use the existing shadcn `Button` primitive with `size="icon"`, visible keyboard focus, native `title` tooltips, and `aria-label` names. The Remove button keeps its destructive visual treatment. Separators divide linking, timing, and effects actions. Disabled state remains driven by the existing capability flags. The narrow-width menu stays text-based, preserving its current discoverability and keyboard behavior.

## Validation

Tests must prove that the header exposes Export but not the removed render controls, save-state copy is exclusive, desktop actions are icon buttons with exact names/tooltips and capability-driven disabled state, and media content does not overflow the panel into the timeline. The existing browser visual-QA desktop editor capture must be refreshed and checked for the corrected containment and compact action strip.
