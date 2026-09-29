# Compact Desktop Editor Pane Grid Design

## Purpose

Make the editor open more like Palmier at common laptop and desktop widths. Palmier keeps the chat/source rail, media library, preview/timeline, and inspector in one horizontal editing workspace. Video Creater currently waits until `xl` widths before switching out of stacked panes, so the source library can appear as an oversized full-width row in normal QA/browser sizes.

## Behavior

- Switch the editor pane grid from stacked to desktop columns at the `lg` breakpoint.
- Use narrower minimum column sizes so four-pane and manual three-pane layouts fit typical 1280px windows without turning the source library into a full-width row.
- Keep the single-column stacked layout below `lg` for narrow screens.
- Preserve Codex rail toggling: four columns when Codex is visible, three columns when hidden.

## UI Details

The source library should read as a narrow Palmier-like source rail beside the preview and timeline, not as a full-width panel above them. The center editor remains the widest column; the inspector and source rail stay compact.

## Testing

- Update workspace layout tests to assert the desktop column templates activate at `lg`.
- Keep assertions that the stacked layout remains `grid-cols-1` with auto rows by default.
- Verify both Codex-visible and manual-editing layouts use compact column templates.
