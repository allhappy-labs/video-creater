# Multi Source Viewer Tabs Design

## Context

Palmier's screenshots show the viewer header as a real editing navigation strip: `Timeline` can sit beside multiple opened source tabs, each source tab has a close affordance, and switching tabs changes the preview/right-rail context without losing the timeline. Video Creater now has a single selected source tab, which is useful but still behaves like a mirror of media selection rather than an open-source workspace.

Palmier's docs state that users can double-click clips to open source footage, inspect generated prompt/reference details, and iterate without leaving the project. Persistent source tabs support that workflow by letting a user compare imported media, generated outputs, and the timeline without re-opening each source repeatedly.

## Goals

- Keep `Timeline` as a permanent viewer tab.
- Track multiple opened source tabs in local workspace UI state.
- Open a source tab when the user selects media, reveals media, or opens source from a timeline clip.
- Make the most recently opened source tab active.
- Let the user switch active source tabs from the preview header.
- Let the user close source tabs without mutating canonical project files.
- Keep selected media in sync with the active source tab so existing media bin, Codex context, and right-rail source details continue to follow the active source.

## Non-Goals

- No persistent open-tab state in project files.
- No media decoding or real source playback changes.
- No multi-window or detached viewer support.
- No Temporal changes. Viewer tabs are immediate editor navigation, not queue, scheduling, or workflow execution.

## Behavior

`EditorWorkspace` owns local viewer tab state:

- `viewerMode`: `timeline` or `source`.
- `openViewerSourceIds`: ordered media ids opened in the viewer.
- `activeViewerSourceId`: the media id represented by the active source tab.

Opening media with `selectMediaSource(mediaId)` ensures that media id is present in `openViewerSourceIds`, moves the viewer into source mode, marks it active, and updates `selectedMediaId`.

Clicking a source tab activates that source and updates `selectedMediaId`. Clicking `Timeline` switches `viewerMode` to `timeline` without closing source tabs or clearing the active source. Closing an inactive source tab removes it. Closing the active source tab activates the nearest remaining source tab; if no source tabs remain, the viewer returns to `Timeline`.

`PreviewPanel` remains presentational. It receives ordered `sourceTabs`, an `activeSourceId`, and callbacks for selecting and closing source tabs. It renders close buttons inside source tabs with accessible labels such as `Close input.mp4 viewer tab`.

## Acceptance Criteria

- The default workspace shows `Timeline` plus the default `input.mp4` source tab.
- Opening a generated timeline clip adds an `output.mp4` tab while keeping `input.mp4` open.
- Clicking `input.mp4` after opening `output.mp4` switches the active viewer source back to `input.mp4`.
- Closing an inactive source tab removes only that tab.
- Closing the active source tab activates another open source when one exists.
- Closing the last source tab returns the viewer to `Timeline`.
- Existing source reveal, media selection, Codex selected-source context, right-rail source inspector, and render review behavior keeps working.
