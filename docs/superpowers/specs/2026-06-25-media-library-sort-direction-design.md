# Media Library Sort Direction Design

## Context

Palmier presents project media as a first-class editing surface: imported assets,
generated outputs, and organized folders all stay close to timeline work. Video
Creater already supports library folders, search, grid/list views, and a sort
selector for project order, name, and kind. The missing affordance is a direct
way to reverse the visible asset order without changing the selected sort key.

## Goals

- Add a compact sort direction control next to the existing media sort selector.
- Default to ascending order so existing project-order and name-sort behavior is
  unchanged on first render.
- Let editors reverse the currently visible media asset order while preserving
  search, folder filtering, selection, drag, and generated asset behavior.
- Keep the change local to UI state. No project file schema or backend command is
  required for this slice.

## Non-Goals

- Persisting sort direction to project files or user preferences.
- Sorting generated history cards, workflow jobs, or folder cards.
- Adding multi-column sort or custom manual ordering.

## Interaction

- The existing `Sort project media` select continues to choose `Project order`,
  `Name`, or `Kind`.
- A neighboring icon button toggles between ascending and descending direction.
- The button uses `aria-pressed=true` when descending is active.
- The accessible label describes the action that will happen next:
  `Sort descending` when currently ascending, and `Sort ascending` when
  currently descending.
- Descending order reverses the result of the active sort key. For `Project
  order`, this means reversing the current project-provided asset order.

## Verification

- Unit test: sorting by name remains ascending by default.
- Unit test: clicking the direction control reverses the visible media tiles and
  updates the control label/state.
- Browser QA: the media toolbar exposes the sort direction button next to the
  sort selector and toggles without disturbing library layout.
