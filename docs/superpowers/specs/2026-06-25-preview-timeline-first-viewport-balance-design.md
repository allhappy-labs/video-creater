# Preview Timeline First Viewport Balance

## Context

Palmier's editor screenshots keep the media library, viewer, inspector, and timeline visible in the
first desktop viewport. Video Creater has the same pane model, but the central `PreviewPanel` now
stretches to fill the available desktop column height before the timeline. On a 1440x1000 desktop
capture, the preview card leaves a large empty area below the render report placeholder and pushes
most of the timeline below the fold.

That weakens manual editing because the user cannot inspect the viewer, transport, tools, and
timeline tracks at once without scrolling. It also makes the editor feel less like Palmier's dense
working surface.

## Goal

Make the center editor column preserve first-viewport visibility for both preview and timeline on
desktop layouts while keeping the existing source tabs, transport, render report, and timeline
behavior.

## Behavior

- `PreviewPanel` should keep natural height at desktop instead of stretching to fill the central
  column.
- The preview viewport remains aspect-video and bounded by its existing card content.
- The empty render-report state stays compact and does not reserve extra vertical space.
- The timeline remains directly below the preview and starts within the first viewport on standard
  desktop sizes.
- Narrow and stacked layouts keep the same component order and avoid horizontal overflow.

## Non-Goals

- No new resizable split-pane system.
- No media playback, render report, or timeline interaction changes.
- No project schema, Temporal, queue, or fal provider change.
- No removal of the preview render report section.

## Verification

- A `PreviewPanel` test asserts the preview card does not use desktop `h-full` stretching.
- Existing preview source-tab and transport tests keep passing.
- A desktop Playwright capture verifies the timeline header and first track row are visible below the
  preview without scrolling.
- Frontend type/lint checks pass.
