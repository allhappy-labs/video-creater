# Selected Generated Reference Viewer Action Design

## Context

Palmier's source details make generation references directly inspectable: first frame, last frame,
style references, and output media can be opened while the editor stays in context. Video Creater's
Source Inspector already reveals generated references into the viewer, and the Codex rail can open
selected references. The Media panel's selected generated source details only reveal references in
the media library, so a user reviewing an AI generation has to switch attention before inspecting a
reference frame in the viewer.

Palmier reference: https://www.palmier.io/docs

## Goal

Let the Media panel's selected generated source details open any generated reference or output in
the viewer while preserving the existing reveal-in-library action.

## Behavior

- Selected generated source reference cards keep their current primary action: reveal/select the
  referenced media in the library.
- When the workspace provides a viewer callback, each reference card also shows an icon button with
  an accessible label `Open <role> <media id> in viewer`.
- Pressing the viewer action calls the callback with the reference media id and does not trigger
  the reveal/select action.
- `EditorWorkspace` wires the callback to the existing `activateViewerSource` path.
- Existing generated output insert, replace, variation, composer, and reference reveal behavior
  remain unchanged.

## Non-Goals

- No new project schema field.
- No viewer source range calculation for generation references in this slice.
- No replacement of the existing reveal/select action.

## Verification

- Add a `MediaBin` test proving a selected generated reference can be opened in the viewer without
  calling `onSelectMedia`.
- Add an `EditorWorkspace` test proving the Media panel reference viewer action opens the source
  viewer tab.
- Run the focused MediaBin and EditorWorkspace tests.
- Run `pnpm lint`.
- Browser-smoke selected generated source details and confirm the new icon action is visible and
  does not overlap the reference card.
