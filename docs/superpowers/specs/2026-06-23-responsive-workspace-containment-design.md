# Responsive Workspace Containment Design

## Context

Palmier's desktop screenshots show a dense editor with media, viewer, timeline, and inspector panes sharing the same workspace. Video Creater now has similar panes, but narrow visual QA exposed a layout defect: after the columns stack, wide timeline content can bleed across the following right-rail inspector instead of staying inside the center editor surface. That makes the manual editor feel broken even though each surface has useful controls.

## Goals

- Preserve the current desktop three-column Palmier-style editor.
- On stacked widths, keep the source library, center editor, and inspector rail as independent vertical sections.
- Ensure wide timeline internals scroll inside the timeline/editor surface instead of overlapping the inspector rail.
- Add a focused regression test for the layout containment classes that protect stacked widths.

## Non-Goals

- No redesign of timeline tracks, clip rendering, or mobile-specific editing mode.
- No change to project files, Temporal workflow records, render queues, or media generation behavior.
- No attempt to make the entire video editor fully phone-optimized in this slice.

## UI Behavior

The workspace grid remains `grid-cols-1` with content-sized rows below the desktop breakpoint and `lg:grid-cols-[260px_minmax(0,1fr)_300px]` at desktop. Each grid child must be allowed to shrink below its min-content width, and timeline internals should use their own local horizontal scrollers. The center editor column receives explicit width containment so the timeline's fixed canvas width cannot visually cover the right rail when the layout stacks. Its minimum height stays natural before `lg`, then switches to `lg:min-h-0` for the desktop flex/grid shell.

## Testing

- `EditorWorkspace` tests assert that the workspace grid is a responsive one-column layout before `lg`.
- Tests assert that the center editor column has a stable landmark and containment classes.
- Existing timeline, preview, source inspector, and workspace tests keep passing.

## Acceptance Criteria

- Narrow browser QA no longer shows the timeline over the project/Codex rail.
- Desktop browser QA still shows the three-pane editor with no obvious regression.
- `rtk pnpm test -- src/components/workspace/editor-workspace.test.tsx` passes.
- `rtk pnpm lint`, full `rtk pnpm test`, and `rtk git diff --check` pass.
