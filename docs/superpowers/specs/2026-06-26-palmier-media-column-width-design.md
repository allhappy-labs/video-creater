# Palmier Media Column Width Design

## Intent

Palmier treats the media browser as a working column: thumbnails, folders, generation controls, and search have enough horizontal room to scan. Video Creater's four-column layout still gives the media/source panel only a 220px minimum when the Codex rail is open, which leaves the media search cramped after the action row was compacted. Widen the media column while keeping the full editor visible at the current desktop smoke-test width.

## Requirements

- In the four-column editor layout with Codex visible, the media/source library column has a 260px minimum width.
- The Codex rail and Inspector rail keep their current 220px and 260px minimums.
- The center preview/timeline column keeps a 500px minimum so the full four-column layout still fits a 1280px viewport with existing gaps.
- The stacked mobile/tablet layout remains unchanged.
- The manual-editing three-column layout with Codex hidden remains unchanged.
- No media, timeline, generation, or inspector behavior changes.

## Verification

- Update `EditorWorkspace` layout tests for the new four-column grid class.
- Keep the Codex-hide manual layout test passing.
- Browser-smoke the desktop editor and confirm the Media panel search/action row is less cramped without hiding the Inspector.
