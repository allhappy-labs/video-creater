# Palmier Template Assets Grid Design

## Goal

Make motion templates and shader backgrounds feel like source-library assets instead of a separate left-rail panel. Palmier keeps source media, generated assets, and timeline-editable materials in one dense browser flow, so Video Creater should avoid a standalone `Templates` card inside the media rail.

## Design

- Replace the outer `Templates` card and heading with a direct `Template assets` group inside the source library flow.
- Render motion templates and shader backgrounds in one compact grid.
- Remove the separate `Shader backgrounds` subsection heading.
- Preserve template thumbnails, drag payloads, insert actions, track badges, duration/category metadata, and empty state.
- Keep the existing media library actions and generated media sections unchanged.

## Testing

- Component tests assert the direct `Template assets` group exists, the old `Templates` heading is absent, and empty copy uses `No template assets available`.
- Workspace tests assert the source library still contains media and templates without source-panel tabs or the old `Templates` panel heading.
- Focused tests cover template insert callbacks, shader background insert callbacks, and draggable template cards.
