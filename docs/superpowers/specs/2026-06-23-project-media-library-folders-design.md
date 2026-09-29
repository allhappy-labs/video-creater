# Project Media Library Folders Design

## Context

Palmier's editor flow includes generating/importing media from a project library and organizing generations into folders before placing or swapping clips on the timeline. Video Creater already stores split projects as editable text files and tracks generated asset provenance, but the runtime model previously flattened `media/index.json` folders away after load.

## Goals

- Preserve media library folders in the runtime `VideoProject` model.
- Allow text-file edits to `media/index.json` folders and asset `folderId` values to round-trip through save/load.
- Validate folder integrity so agents cannot leave broken folder references in canonical project files.
- Surface folder grouping in the media bin without hiding normal select, replacement, and generated-output workflows.

## First Slice

- Add optional `folderId` to media assets and `mediaFolders` to projects.
- Persist `mediaFolders` as `media/index.json.folders`.
- Render media-bin sections for declared folders plus an `Unfiled` group.
- Keep imported and generated media unfiled by default until a future manual or agent folder-assignment action exists.

## Second Slice

- Add `assignMediaFolder` as a validated `ProjectAction`.
- Let manual media-bin selection move an asset into any declared folder or back to `Unfiled`.
- Advertise `assignMediaFolder` in the app-server ProjectAction schema so agents can organize project-library assets through the same validated split-file workflow.
- Reject missing media, missing folders, and empty folder ids before writing canonical project files.

## Third Slice

- Add validated `createMediaFolder`, `renameMediaFolder`, and `deleteMediaFolder` project actions.
- Let the media bin create folders, rename folders, and delete folders through the same split-project write path as media assignment.
- Generate deterministic folder ids from manual folder names, with collision suffixes for duplicate names.
- Keep deletion non-destructive: remove only the folder, clear affected media `folderId` values, and preserve media assets.
- Advertise folder-management actions in the app-server ProjectAction schema so agents can organize project-library files without bypassing validation.

## Fourth Slice

- Render folder hierarchies from `parentId` in the media bin instead of flattening child folders.
- Keep parent folders visible when they contain only nested child assets.
- Use full folder paths, such as `Generated selects / Scene A`, for assignment actions and folder-management labels.
- Keep orphaned or invalid parent references defensive in the UI by treating those folders as top-level, while split-project validation continues to report broken references.

## Fifth Slice

- Expose a bounded project-library summary in Codex app-server edit turns so agents can see folder ids, nested folder paths, media ids, media paths, media kinds, durations, and each asset's current folder.
- Tell agents to use folder ids rather than display paths when returning `assignMediaFolder`, `createMediaFolder`, `renameMediaFolder`, and `deleteMediaFolder` project actions.
- Keep the write path unchanged: folder organization still flows through validated `ProjectAction` values and split-project writes instead of direct mutation.

## Deferred

- Dedicated MCP commands for project-library organization once Video Creater exposes a standalone MCP tool surface beyond the Codex app-server prompt/schema boundary.
