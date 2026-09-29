# Palmier Viewport-Bounded Editor Panes Design

## Goal

Make Video Creater's main workspace behave more like Palmier's editor: the media rail, preview/timeline, and inspector rail should stay inside the visible editor viewport, with each rail scrolling internally. The current `Editor panes` grid can grow several thousand pixels tall because it uses max-content rows and page-level overflow, which makes the app feel like a document instead of a bounded editing surface.

## Requirements

- `Editor panes` remains the main grid that switches between the Codex four-column layout and the manual three-column layout.
- On wide screens, the grid rows should be viewport-bounded (`lg:grid-rows-[minmax(0,1fr)]`) instead of content-sized.
- On narrow stacked layouts, each pane should keep a usable minimum row height instead of compressing all panes into equal tiny rows.
- The grid should hide page-level overflow on wide screens (`lg:overflow-hidden`) so the rail contents scroll inside their existing internal scroll containers.
- The source library panel and inspector rail should explicitly allow internal overflow with `min-h-0` and `overflow-hidden`.
- The center preview/timeline column should keep its local timeline overflow behavior and avoid horizontal page growth.
- No editor controls, media selection, generated asset actions, or timeline actions should change.

## Testing

- Update the existing editor pane layout test so it rejects `auto-rows-max` as the wide-screen contract and expects the bounded row/overflow classes.
- Keep source, center, and inspector columns asserting `min-w-0`; add `min-h-0` and `overflow-hidden` expectations for the rails.
- Verify with a browser DOM check that the media browser and inspector rail no longer report multi-thousand-pixel heights at a desktop viewport.
- Run the focused editor layout test, the full editor workspace test file, TypeScript, diff check, and the full Vitest suite.
