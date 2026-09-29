# Palmier Generation Sheet Destination Chrome Removal Design

## Intent

Palmier's generation sheet keeps the creative decision surface compact: media type, references, prompt, model/format, credits, and a single send action. Video Creater already opens generation as a focused sheet, but it still exposes a persistent `Destination` folder selector and destination summary row. Folder placement is project organization, not part of the shot-generation decision.

## Requirements

- Remove the visible `Destination` dropdown from `MediaBin`'s `Media generation` sheet.
- Remove the `Destination` row from the active generation recipe summary.
- Preserve automatic folder targeting when generation is opened from inside an active project library folder.
- Preserve target-folder metadata in generated asset and Temporal-backed start requests.
- Keep the project library folder move/assignment workflow unchanged outside the generation sheet.

## Testing

- Add/update `MediaBin` tests to prove the generation sheet no longer renders `Generation destination folder` or a recipe `Destination` row.
- Keep `MediaBin` coverage proving active folder context still sets `targetFolderId` in queued generation requests.
- Keep `EditorWorkspace` coverage proving media-panel generation still sends `targetFolderId` through the Temporal generate-media path.
- Run focused generation tests, typecheck, full tests, and browser QA for the composer.

## Self-Review

- No project schema, Temporal workflow, generated asset, or fal.ai provider changes.
- Scope is limited to visible generation-sheet chrome and tests around the preserved automatic folder route.
- Users can still organize generated media through the library folder controls after assets exist.
