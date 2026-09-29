# Project-Native Editor Import And Timeline Design

## Summary

Upgrade the editor from a mostly static sample workspace into a project-backed editing surface. The first implementation slice covers two user-facing gaps together: importing media into a persisted project and directly adjusting caption timing on the timeline.

The app should copy imported files into the project `media/` directory, register them in `VideoProject.media`, show them in the Media Bin, and keep the editor panels bound to the same project state. Caption timing changes should use the existing validated Rust `TimelinePatch` contract instead of ad hoc frontend mutation.

This is not a full asset manager. The goal is to make the editor usable for the first real project import and caption timing correction workflow without taking on thumbnails, waveform rendering, tags, relinking, proxy generation, or media search.

## Goals

- Let users import media into a project so files survive app restart.
- Copy imported source files into the project folder instead of only storing external paths.
- Register imported assets in the canonical `VideoProject.media` array.
- Show project media in `MediaBin` with clear empty, importing, imported, and failed states.
- Let users place or select imported media for editing workflows.
- Let users move caption items horizontally on the timeline to change `startSeconds`.
- Let users resize caption items with timeline handles to change `durationSeconds`.
- Route caption timing edits through validated timeline patches.
- Surface invalid timing edits at the timeline target.
- Add focused Rust and TypeScript tests before implementation.

## Non-Goals

- Full asset-manager features such as bins, tags, search, relink, proxies, thumbnails, and waveform generation.
- Frame-accurate trim editing for video or audio clips.
- Multi-select timeline editing.
- Ripple edits, snapping groups, J/L cuts, transitions, or advanced NLE behavior.
- Automatic transcription on import.
- Canonical project mutation directly from Codex proposals.
- Direct editing of canonical project files by the app-server agent.

## Existing Foundation

The codebase already has the core pieces this design should reuse:

- Rust `VideoProject` includes `media`, `timeline`, `tracks`, and `TimelineItem` data.
- Project storage already creates `media/`, `transcripts/`, `generated/`, `renders/`, and `logs/`.
- Rust `TimelinePatch` already supports `moveItem`, `resizeItem`, and `editCaptionText`.
- `apply_timeline_patch_to_project` exposes validated timeline mutation to the frontend.
- React already has `EditorWorkspace`, `MediaBin`, `TimelineEditor`, and `CaptionInspector`, but they mostly read sample data and hardcoded media names.

Tauri desktop APIs can support the import workflow. The official dialog plugin provides file selection and returns file paths on Linux, Windows, and macOS. The official filesystem plugin supports file copying and app-controlled storage. The implementation may use frontend plugins or Rust commands, but canonical project mutation should stay in Rust.

## User Workflow

The first usable flow should be:

1. User creates or opens a project folder.
2. User clicks `Import`.
3. App opens a file picker for supported media files.
4. User selects one or more files.
5. App copies those files into `<project>/media/`.
6. App registers imported assets in `VideoProject.media`.
7. Media Bin updates to show imported assets.
8. User selects a media asset or creates a timeline item from it.
9. User selects a caption cue on the timeline.
10. User drags the cue body to change the cue start.
11. User drags the left or right handle to change cue start/duration or duration.
12. App applies a validated timeline patch and updates the canonical project state.

This keeps import and timing correction in one coherent editor surface.

## Architecture

### Rust Project Import

Add a Rust import API that accepts:

- `project_dir`
- current `VideoProject`
- one or more source file paths

It returns the updated `VideoProject` and enough import result detail for the UI to show what happened.

Suggested wire shape:

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportMediaRequest {
    pub project_dir: String,
    pub source_paths: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportMediaResult {
    pub project: VideoProject,
    pub imported: Vec<MediaAsset>,
    pub skipped: Vec<ImportSkippedFile>,
}
```

The command should:

- resolve and validate `project_dir` with the existing project directory rules
- ensure project storage folders exist
- reject empty source paths
- reject missing files
- reject directories for the first implementation
- infer `MediaKind` from extension
- copy supported files into `media/`
- generate stable project-relative paths such as `media/<asset-id>-<sanitized-name>.mp4`
- append `MediaAsset` records to `project.media`
- preserve existing project media
- save the updated project file

Initial metadata can be conservative. Duration, width, height, and FPS may be `0.0` or `None` until ffprobe-backed probing is added. In a multi-file import, unsupported files should be reported in `skipped` while supported files still import. If no selected files can be imported, the command should return an error and leave the project unchanged.

### Media Kind Support

Initial supported extensions:

- video: `mp4`, `mov`, `m4v`, `webm`
- audio: `wav`, `mp3`, `m4a`, `aac`, `flac`
- image: `png`, `jpg`, `jpeg`, `webp`

Generated assets remain a separate `MediaKind::Generated` path and are not part of manual import in this slice.

### Frontend Project State

`EditorWorkspace` should own a single current project state and pass project-derived data down:

- `MediaBin` receives `project.media`
- `TimelineEditor` receives `project.timeline`
- `CaptionInspector` receives the selected caption item
- import and timeline patch callbacks update the same project state

For this slice, sample data can remain as a development fallback, but the editor components should be designed around project-backed props.

### Media Bin

`MediaBin` should become an operational panel, not static text.

It should display:

- empty project state
- importing state
- imported media assets with filename, kind, and relative path
- selected media state
- failed import message with next action

Each media row should be compact and scannable. It should support selection. Adding a selected media asset to the timeline can be included if the implementation can do it through a validated project patch; otherwise selection can be the first slice and timeline insertion can follow.

### Timeline Direct Manipulation

`TimelineEditor` should convert pointer movement into timeline patches with deterministic math:

- pixels to seconds uses the existing timeline scale
- drag body creates `moveItem`
- right handle resize creates `resizeItem`
- left handle resize creates a combined start/duration change

Because the Rust patch contract currently models `moveItem` and `resizeItem` separately, left-edge resize can apply as two patches in order:

1. `moveItem` to the new start
2. `resizeItem` to the new duration

If atomic left-edge resizing is needed to avoid temporary invalid states, add a Rust patch variant such as `trimItemStart` later. The first slice can keep left-edge resizing scoped to captions and validate the final state in the frontend before sending patches.

Caption timing constraints:

- start cannot be negative
- duration must stay above a minimum, suggested `0.1s`
- caption can stay on caption tracks only
- locked tracks reject edits
- timeline duration recalculates after successful patch

### Timeline UI States

Timeline items should include:

- selected state
- hover state
- drag ghost or active drag state
- resize handle hover/focus state
- invalid target state
- compact labels that truncate when duration is short

Invalid edits should show near the affected item or track. Examples:

- `Cannot move before 00:00`
- `Caption duration is too short`
- `Track is locked`

The timeline should remain keyboard accessible where feasible. At minimum, selected captions should still be editable through the inspector, and icon-only or handle controls need accessible labels.

## Data Flow

```text
File picker
  -> import_media_to_project(projectDir, project, sourcePaths)
  -> copy files into project media/
  -> append VideoProject.media
  -> save project JSON
  -> React project state
  -> Media Bin and timeline refresh

Pointer drag or resize
  -> timeline pixel math
  -> TimelinePatch
  -> apply_timeline_patch_to_project
  -> validated VideoProject
  -> React project state
  -> Timeline and Caption Inspector refresh
```

## Error Handling

Import should fail or skip clearly when:

- project directory is missing or invalid
- selected file does not exist
- selected path is a directory
- file extension is unsupported
- destination file cannot be created
- copy fails
- project save fails

Timeline edits should fail clearly when:

- item does not exist
- track does not exist
- track is locked
- item kind does not match the target track
- start is negative
- duration is non-positive or below the UI minimum

Recoverable frontend failures should keep the current project state unchanged and show a localized error.

## Testing

Use test-driven development.

Rust tests should cover:

- importing a supported file copies it into `media/`
- imported media uses project-relative paths
- importing appends to existing media without dropping existing assets
- duplicate filenames produce distinct destination paths
- unsupported extensions are reported in `skipped` while supported files still import
- all-unsupported import returns an error and leaves the project unchanged
- missing source files fail clearly
- imported project saves and loads with the new media asset
- timeline move and resize patches continue to validate caption timing

Frontend tests should cover:

- media bin renders empty state
- media bin renders imported project assets
- import success updates the project state
- import failure renders an actionable error
- dragging a caption body creates the expected move patch
- resizing the right edge creates the expected resize patch
- resizing the left edge creates the expected final start and duration behavior
- invalid drag or resize does not submit a patch and shows inline feedback

## Visual QA

After implementation:

1. Run the narrowest relevant tests.
2. Run `pnpm lint` for TypeScript and Tailwind changes.
3. Run Rust tests touching project import and timeline patches.
4. Open the app in the browser or Tauri dev target.
5. Check desktop and narrow widths for Media Bin, timeline handles, captions, and inspector layout.
6. Verify empty, importing, failed import, selected media, selected caption, dragging, resizing, and invalid edit states.

## Implementation Notes

- Prefer existing shadcn primitives for chrome and custom rendering only for timeline-specific surfaces.
- Keep editor density high; avoid landing-page or marketing treatment.
- Use lucide icons for import, media kind, and timeline controls where applicable.
- Do not introduce a broad asset-manager abstraction until thumbnails, relink, and search are in scope.
- Keep generated or imported file paths relative inside `VideoProject`.
- Keep Codex app-server behavior proposal-based; it must not mutate canonical project files directly.
