# Folder-Targeted Generation Design

## Context

Palmier's media workflow lets editors generate assets and organize generations into project-library folders. Video Creater now has navigable media folders and validated folder actions, but queued generations do not remember which folder the editor was working in. When a mock or provider completion imports generated outputs, the output media is always unfiled.

## Goal

Let media generated from inside a project-library folder land in that folder through the same text-file-editable generated-asset and media-index workflow.

## Behavior

- `MediaBin` includes the active folder as `targetFolderId` when queueing a generation from a scoped folder view.
- The composer shows a compact destination row when a folder target is active.
- All-library generation keeps `targetFolderId: null`.
- `EditorWorkspace` forwards `targetFolderId` into the `recordGeneratedAsset` project action.
- Generated asset sidecars persist `targetFolderId` only when present.
- `recordGeneratedAsset` validates the target folder exists before accepting the queued asset.
- `completeGeneratedAsset` assigns generated output media to the pending asset's `targetFolderId`.
- Existing generated assets without `targetFolderId` remain valid and continue to complete into `Unfiled`.

## Data Flow

1. `MediaBin` owns `activeFolderId` for the project-library view.
2. `submitGeneration` emits `targetFolderId: activeFolderId ?? null`.
3. `EditorWorkspace.queueMediaGeneration` writes that value into the generated-asset action.
4. Rust stores a trimmed target folder id on `GeneratedAsset`.
5. When completion imports output media into `project.media`, the output media receives the generated asset's `targetFolderId`.

## Tests

- Media Bin proves a folder-scoped generation request includes the selected folder id and displays the destination.
- Media Bin proves all-library generation requests still emit `targetFolderId: null`.
- Editor Workspace proves queued generation project actions include the request target folder.
- Rust project action tests prove target folders are validated, persisted, and applied to completed output media.

## Acceptance Criteria

- A generation queued from `Generated selects` completes with its generated output media assigned to `folder-generated`.
- Blank or missing target folder ids serialize as absent.
- Missing folder ids are rejected before canonical project files are written.
- Existing generation, folder, mock completion, and generated-asset tests continue to pass.
