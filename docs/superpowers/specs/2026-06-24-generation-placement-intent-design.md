# Generation Placement Intent Design

## Context

Palmier's docs and screenshots make generation feel native to editing: users can generate media from the media panel, generate video on the timeline, regenerate, and swap clips without leaving the project. Video Creater already queues generated assets through Temporal-shaped job records and text-file-editable `recordGeneratedAsset` actions, but the Media panel composer only presents generation as a library action. It does not show whether the queued asset is meant for the library or the timeline.

Video Creater does not yet have a canonical project action for completing a queued generation and inserting the completed output onto the timeline. That should remain a later backend/action slice. The UI can still expose placement intent now so the workflow is visible and future project actions have a clear surface to connect to.

## Goal

Add a compact placement selector to the Media generation composer with `Library` and `Timeline` choices. The first implementation records UI intent locally and keeps the queued project action behavior unchanged.

## Behavior

- The Media generation composer shows a placement segmented control above the prompt/settings area.
- `Library` is selected by default and keeps the current queue behavior.
- `Timeline` can be selected and shows a compact disabled-status note that timeline insertion will happen after completion once the project action path exists.
- The generation request sent to `EditorWorkspace` remains unchanged for this slice: it still queues a generated asset and Temporal-shaped workflow job only.
- Closing and reopening the composer resets placement to `Library`.
- The control is hidden from audio-specific behavior only if the composer later needs mode-specific placement support; in this slice all modes can show the same intent.

## Non-Goals

- No new project schema field.
- No new `recordGeneratedAsset` placement metadata.
- No automatic insertion of queued generations onto the timeline.
- No Temporal worker execution changes.
- No generated output completion changes.

## Testing

- `MediaBin` tests prove the composer defaults to `Library` placement.
- Tests prove selecting `Timeline` updates the visible status copy while `Queue generation` still emits the existing request shape.
- Tests prove reopening the composer resets placement to `Library`.
- Existing generation request, folder targeting, reference, naming, and generated-source tests continue to pass.

## Browser QA

- Desktop: verify the placement control fits in the generation composer and does not crowd prompt/settings controls.
- Narrow: verify placement buttons and status copy wrap or truncate cleanly without overlapping the composer footer.
