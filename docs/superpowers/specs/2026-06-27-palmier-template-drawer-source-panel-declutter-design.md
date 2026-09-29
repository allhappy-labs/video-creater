# Palmier Template Drawer Source Panel Declutter Design

## Context

Palmier's screenshots keep the left source panel focused on project media, folders, search, and generation. Video Creater currently appends the full `Template assets` catalog below the media browser in the same left rail. That makes the default source panel much longer than Palmier's media panel and pushes media browsing into a mixed media/template library.

Palmier still exposes text and visual overlay creation from the timeline toolbar, so Video Creater keeps manual template insertion, but the catalog appears only when the editor asks for it.

Palmier reference: https://www.palmier.io/docs

## Goal

Move the motion/template catalog out of the always-visible source library and into a compact timeline-adjacent drawer opened from the timeline toolbar.

## Requirements

- The default `Source library panel` renders project media and folders, but does not render `Template assets`.
- The timeline toolbar exposes an icon button labeled `Open template assets`.
- Activating `Open template assets` shows a `Timeline template drawer` near the timeline, containing the existing `Template assets` library.
- Existing template insertion and shader background insertion callbacks remain unchanged once the drawer is open.
- The existing `Add text overlay` button remains available and continues to create a plain text overlay directly.
- Do not add source-panel tabs, media/template mode switches, or an always-visible template section.
- Do not change project schema, Temporal workflow records, template definitions, or drag/drop payload formats.

## Testing

- Add a red test proving the default `Source library panel` does not contain `Template assets`, then opening the toolbar drawer reveals it.
- Add coverage that `Open template assets` calls a toolbar callback from `TimelineEditor`.
- Update workspace template insertion tests to open the drawer before clicking template insert buttons.
- Run focused workspace/timeline/template tests, typecheck, full Vitest, diff check, and browser QA for the default source panel and opened template drawer.
