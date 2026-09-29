# Agent-Editable Project Files Design

## Summary

Upgrade Video Creater's project storage from a single canonical JSON file into a project folder contract that is efficient for both humans and agents to inspect, edit, diff, validate, and render.

The current app already has a local-first `VideoProject` runtime model, a `video-creater.project.json` file, media import, transcript data, timeline tracks, Codex proposal validation, render reports, motion templates, and shader template folders. This design keeps that foundation but changes the persisted project format into a split, text-file-editable project folder:

```text
project/
  video-creater.project.json
  timeline.json
  media/index.json
  transcripts/<media-id>.json
  templates/<template-id>.json
  generated/<asset-id>/asset.json
  renders/<render-id>/report.json
  logs/
```

`video-creater.project.json` remains the entry point and manifest. Rust loads the manifest and referenced split files into the existing `VideoProject` runtime model, validates every file before mutation or render, and saves accepted changes atomically back into the split files. Agents may read or edit the text files directly, and MCP/tool APIs may still submit typed actions, but Rust remains the only authority that turns edits into canonical application state.

This is the first missing foundation needed to move Video Creater toward a Palmier-style editor: agents can operate on durable project context, while users still get a real manual editor for adjusting timeline cuts, transcription, timing, templates, colors, generated assets, and render output.

## Goals

- Make project, media, transcript, template, generated asset, timeline, and render state readable as small text files.
- Keep `video-creater.project.json` as the stable project entry point and compatibility anchor.
- Load split project files into the existing Rust `VideoProject` runtime model.
- Save the runtime model back into split project files without losing existing project data.
- Add a broad Rust-validated project action surface for agent and UI edits.
- Route meaningful UI mutations through the same Rust validation path used for agent edits.
- Preserve EDL-first generation: primary source ranges are selected before captions, overlays, templates, HyperFrames, GPU visuals, or render jobs are applied.
- Store generation provenance with generated assets: prompt, model, references, first and last frame inputs, status, outputs, and retry lineage.
- Store transcript repair data in editable transcript files without breaking caption alignment.
- Store template fields and style overrides in editable template files, including `visualTreatment`, `motion`, `safeZone`, and `avoid`.
- Store render reports as project artifacts with duration, stream presence, caption alignment, overlay timing, artifact paths, and logs.
- Keep secrets, credentials, subscription state, and provider tokens outside project files.

## Non-Goals

- Replacing the existing `VideoProject` runtime model in the first implementation.
- Building a full public plugin marketplace.
- Letting Codex or sidecars mutate canonical state without Rust validation.
- Making generated visual layers before a real EDL exists.
- Implementing every professional NLE feature in this storage slice.
- Exporting Premiere or DaVinci XML in this first slice.
- Supporting hosted collaboration or multi-user conflict resolution.

## Existing Foundation

The current project has useful pieces to preserve:

- `src-tauri/src/project/model.rs` defines `VideoProject`, `MediaAsset`, `Transcript`, `Timeline`, `TimelineTrack`, and `TimelineItem`.
- `src-tauri/src/project/storage.rs` saves and loads `video-creater.project.json`.
- `src-tauri/src/project/patch.rs` validates a small `TimelinePatch` set.
- `src-tauri/src/project/import.rs` imports media into the project folder.
- `src-tauri/src/edit/render_plan.rs` converts transcript and preset requests into EDL-first timeline items.
- `src-tauri/src/codex/proposal.rs` validates EDL-oriented Codex proposals and template overlay metadata.
- `src/components/workspace/editor-workspace.tsx` has a project-backed editor workspace seed, but still applies some project edits locally in React.
- `src/lib/timeline.ts` mirrors the small patch shape in TypeScript.
- `src-tauri/assets/shader-background-templates/` and project-local shader template support already point toward file-backed template catalogs.

The first implementation should extend these paths rather than introduce a second project model.

## Palmier-Derived Requirements

Palmier establishes the product bar this design targets:

- Generation lives in the editor, not in a disconnected web import loop.
- User footage and generated media share one timeline.
- Agents can read the full project context and perform real editor actions.
- Agents can trim, split, reorder, adjust, and regenerate clips.
- Generated clips retain their prompt, model, reference media, first frame, and last frame context.
- Users can manually inspect and repair what the agent created.
- Export and render artifacts remain project-linked.

Video Creater should express the same pattern through a local-first Rust/Tauri project contract. MCP remains useful, but a durable text-file surface gives agents lower-friction access to state, better diffs, easier review, and a recovery path when MCP context is not enough.

## Codex App-Server Action Handoff

Codex video-edit turns should return the existing EDL-first proposal plus an ordered `projectActions` array. The EDL remains mandatory and must be valid before visual layers or actions are considered. `projectActions` is for file-backed editor changes that should survive as reviewable project state: caption repairs, transcript word edits, generated asset provenance, render reports, template item timing/fields, template style overrides, and timeline edits.

The app-server JSON schema advertises the supported camelCase `ProjectAction` variants, and Rust deserializes the actions into the same enum used by manual UI edits. Validation applies every action to a cloned `VideoProject` before any save occurs. A single invalid action rejects the proposal, keeping Codex from mutating canonical split project files without Rust validation.

## Project Folder Contract

### Manifest

`video-creater.project.json` becomes the project manifest. It keeps project identity and points to split files.

```json
{
  "schemaVersion": 2,
  "id": "project-123",
  "name": "Launch Video",
  "createdAt": "2026-06-21T10:00:00Z",
  "updatedAt": "2026-06-21T10:05:00Z",
  "layout": "split",
  "files": {
    "timeline": "timeline.json",
    "media": "media/index.json",
    "transcripts": "transcripts",
    "templates": "templates",
    "generated": "generated",
    "renders": "renders",
    "logs": "logs"
  },
  "renderSettings": {
    "width": 1920,
    "height": 1080,
    "fps": 24,
    "loudnessLufs": -14,
    "captions": "burn_in"
  },
  "codexThreadId": null
}
```

The manifest must not duplicate large timeline, transcript, media, template, generated asset, or render report arrays. Runtime compatibility can still expose a complete `VideoProject` after Rust loads all referenced files.

### Timeline File

`timeline.json` stores tracks, items, selected source ranges, and item-level references.

```json
{
  "schemaVersion": 1,
  "durationSeconds": 42.5,
  "tracks": [
    {
      "id": "track-video",
      "name": "Video",
      "kind": "video",
      "locked": false,
      "items": [
        {
          "id": "clip-1",
          "kind": "video_clip",
          "startSeconds": 0,
          "durationSeconds": 4.2,
          "source": { "type": "media", "mediaId": "media-main" },
          "label": "Hook",
          "properties": {
            "sourceIn": 12.4,
            "sourceOut": 16.6,
            "reason": "hook"
          }
        }
      ]
    }
  ]
}
```

Every primary generated edit must include real `sourceIn` and `sourceOut` values for video clips before visual layers are accepted. Full-source pass-through edits should fail validation unless explicitly marked as a manual full-length sequence outside the generate-edit path.

### Media Index

`media/index.json` stores imported and generated asset references, folders, probe metadata, and generation status.

```json
{
  "schemaVersion": 1,
  "folders": [
    { "id": "folder-broll", "name": "B-roll", "parentId": null }
  ],
  "assets": [
    {
      "id": "media-main",
      "relativePath": "media/interview.mp4",
      "kind": "video",
      "folderId": null,
      "durationSeconds": 600.0,
      "width": 1920,
      "height": 1080,
      "fps": 29.97,
      "generationStatus": "none"
    }
  ]
}
```

Generated assets can appear here as `kind: "generated"` but their full provenance lives in `generated/<asset-id>/asset.json`.

### Transcript Files

`transcripts/<media-id>.json` stores editable source transcripts and repair metadata.

```json
{
  "schemaVersion": 1,
  "id": "transcript-media-main",
  "mediaId": "media-main",
  "engine": "nvidia/parakeet-tdt-0.6b-v3",
  "rawArtifactPath": "transcripts/media-main.raw.json",
  "repairs": [
    {
      "id": "repair-1",
      "kind": "word_text",
      "wordIndex": 42,
      "before": "creater",
      "after": "creator",
      "createdAt": "2026-06-21T10:15:00Z"
    }
  ],
  "segments": [],
  "words": [
    {
      "text": "Creator",
      "startSeconds": 12.4,
      "endSeconds": 12.8,
      "confidence": 0.94,
      "speaker": null
    }
  ]
}
```

Transcript repair changes must preserve monotonic timestamps, source-media timing, and caption derivation metadata.

### Template Override Files

`templates/<template-id>.json` stores project-level template overrides. Built-in template definitions remain in code or asset catalogs; project files hold user and agent changes.

```json
{
  "schemaVersion": 1,
  "templateId": "kinetic-lower-third-v1",
  "name": "Kinetic Lower Third",
  "fields": {
    "headline": "Launch day",
    "subline": "Built with Video Creater"
  },
  "style": {
    "accentColor": "#22d3ee",
    "backgroundColor": "rgba(2, 6, 23, 0.72)",
    "textColor": "#ffffff"
  },
  "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
  "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
  "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
  "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds"
}
```

Every caption, overlay, title card, diagram, callout, lower third, and HyperFrame-related template override must include `visualTreatment`, `motion`, `safeZone`, and `avoid`.

### Generated Asset Files

`generated/<asset-id>/asset.json` stores generation provenance and output paths.

```json
{
  "schemaVersion": 1,
  "id": "generated-shot-1",
  "kind": "video",
  "status": "completed",
  "prompt": "slow push-in on the product on a clean studio table",
  "model": {
    "provider": "fal.ai",
    "id": "fal-ai/wan-25-preview/text-to-video"
  },
  "references": {
    "mediaIds": ["media-product-still"],
    "firstFrameMediaId": "media-product-still",
    "lastFrameMediaId": null
  },
  "outputs": [
    {
      "relativePath": "generated/generated-shot-1/output.mp4",
      "width": 1280,
      "height": 720,
      "durationSeconds": 4.0,
      "fps": 24
    }
  ],
  "createdAt": "2026-06-21T10:20:00Z",
  "parentAssetId": null,
  "retryOfAssetId": null
}
```

Agents can inspect this file to rerun, tweak, or replace generated media without losing prompt and reference context.

### Render Report Files

`renders/<render-id>/report.json` stores render validation results.

```json
{
  "schemaVersion": 1,
  "id": "render-draft-1",
  "status": "completed",
  "outputPath": "renders/render-draft-1/output.mp4",
  "durationSeconds": 42.5,
  "streams": {
    "video": true,
    "audio": true
  },
  "checks": {
    "duration": "passed",
    "captionAlignment": "passed",
    "overlayTiming": "passed",
    "artifactPaths": "passed"
  },
  "logPath": "logs/render-draft-1.log",
  "createdAt": "2026-06-21T10:30:00Z"
}
```

Render reports are part of the project context for manual review and agent troubleshooting.

## Rust Runtime Contract

Rust exposes a split storage layer beside the current single-file storage:

- `load_project_manifest(project_dir) -> ProjectManifest`
- `load_split_project(project_dir) -> VideoProject`
- `save_split_project(project_dir, &VideoProject) -> ProjectWriteReport`
- `validate_split_project(project_dir) -> ProjectValidationReport`
- `migrate_single_file_project(project_dir) -> ProjectWriteReport`

The first implementation should keep `VideoProject` as the in-memory model to avoid a wide render and UI rewrite. The split loader maps file-level data into the existing runtime shape. The split saver maps the runtime shape back to manifest, timeline, media, transcript, template, generated asset, and render report files.

The storage layer should support schema version 1 single-file projects and schema version 2 split projects:

- Loading schema version 1 continues to work.
- Saving a schema version 1 project through the split save path creates schema version 2 files.
- A project can include a temporary compatibility snapshot only if it is explicitly named and not treated as the canonical source.

## Expanded Project Actions

The current `TimelinePatch` is too small for an agent-controlled editor. Introduce a broader `ProjectAction` surface that UI and MCP tools can both use.

Initial action set:

- `addItems`: add clips, captions, overlays, audio items, or generated assets to compatible tracks.
- `insertItems`: ripple insert items at a timeline position.
- `moveItems`: move items across compatible tracks and timeline positions.
- `trimItem`: adjust timeline duration and media source range.
- `splitItem`: split an item at a timeline time and preserve source offsets.
- `removeItems`: remove selected items.
- `reorderItems`: sort or place items deterministically on a track.
- `updateTemplateFields`: update template text fields.
- `updateTemplateStyle`: update approved style fields such as accent color, text color, backing color, opacity, or scale.
- `editTranscriptWords`: update transcript word text or timing with validation.
- `applyCaptionRepair`: update caption text/timing and record transcript/caption repair metadata.
- `recordGeneratedAsset`: add generation provenance. Completed generations include outputs immediately; queued, running, or failed generations may be recorded before outputs exist.
- `completeGeneratedAsset`: attach generated media outputs to an existing non-completed generated asset and mark it completed.
- `attachRenderReport`: link a render report to project state.

Every action must validate track compatibility, timing, source references, generated asset references, template metadata, and locked-track rules before changing state.

The existing `TimelinePatch` can remain as a compatibility layer. It should be translated into `ProjectAction` internally once the broader action path exists.

## Agent Workflow

Agents get two equivalent paths:

1. Read text files, propose or edit split project files, then ask Rust to validate and apply.
2. Call MCP or app tools that wrap `ProjectAction` and update the same split files.

The recommended agent flow for generated edits is:

```text
inspect manifest and media index
  -> inspect transcript files
  -> produce or refine EDL source ranges
  -> submit add/trim/split/move actions
  -> add captions and visual layers after EDL validation
  -> record generated asset provenance
  -> complete generated assets when media outputs arrive
  -> render draft
  -> attach render report
```

When an agent reruns or tweaks a generated clip, it should preserve the original generated asset sidecar for inspection, create a new generated asset with `parentAssetId` or `retryOfAssetId`, and use `completeGeneratedAsset` when the replacement media output is ready. Timeline placement can then swap to the completed output while retaining prompt, first-frame, last-frame, reference, and lineage context.

The agent must not add captions, overlays, title cards, HyperFrames, shader scenes, or GPU visuals before the primary EDL exists and validates.

## Manual UI Workflow

The editor should expose the same operations as the agent action surface:

- Timeline direct manipulation creates `ProjectAction` requests, not local-only mutations.
- The inspector updates caption text, caption timing, template fields, and template style through Rust validation.
- Transcript repair UI writes to transcript files and marks derived caption repairs.
- Media bin shows imported and generated assets from `media/index.json`.
- Template browser shows built-in templates plus project overrides from `templates/`.
- Render report panel reads reports from `renders/<render-id>/report.json`.

The UI may preview edits optimistically, but accepted edits must round-trip through Rust and return an updated project. This keeps manual edits, agent edits, render input, and file state consistent.

## Validation And Error Handling

Rust validation must reject or report:

- Manifest paths that escape the project folder.
- Missing split files referenced by the manifest.
- Invalid JSON or unsupported schema versions.
- Duplicate media, transcript, template, generated asset, render, track, or item IDs.
- Timeline items that reference missing media or generated assets.
- Negative, non-finite, or zero durations where invalid.
- Source ranges outside media duration.
- Generated edit video clips without `sourceIn` and `sourceOut`.
- Track and item kind mismatches.
- Edits to locked tracks.
- Caption text that becomes empty.
- Transcript words with non-monotonic timestamps.
- Template overrides missing `visualTreatment`, `motion`, `safeZone`, or `avoid`.
- Render reports missing duration, stream checks, artifact paths, or log references.
- Writes that would leave the project with a partially written canonical file.

Errors should identify the file path and field path, for example:

```text
timeline.json tracks[0].items[2].properties.sourceOut: source range exceeds media duration
templates/kinetic-lower-third-v1.json safeZone: required visual metadata is missing
```

## Atomic Writes

Split saving should write each JSON file through a temp file in the same directory, sync the file, then rename it into place. The write report should list all files created, updated, or removed.

For a multi-file save, the first implementation does not need full transactional rollback. It must avoid corrupting any individual JSON file and must report partial write failures clearly. A later checkpoint file can improve crash recovery after the initial split project path is working.

## Compatibility And Migration

Migration should be explicit:

1. Load existing schema version 1 `video-creater.project.json`.
2. Create schema version 2 split files.
3. Preserve media, transcripts, timeline, render settings, Codex thread id, and jobs.
4. Move large arrays out of the manifest into split files.
5. Save a migration report under `logs/project-migration-<timestamp>.json`.

The app can continue opening schema version 1 projects. New project creation should use schema version 2 once the split storage layer is implemented.

## Testing Strategy

Rust tests:

- Load a valid split project into `VideoProject`.
- Save a `VideoProject` into split files.
- Migrate a schema version 1 project into split files.
- Reject missing manifest references.
- Reject duplicate IDs across split files.
- Reject timeline items referencing missing media or generated assets.
- Reject invalid source ranges and generated edit pass-throughs.
- Apply each initial `ProjectAction` variant.
- Complete pending generated assets and persist generated media outputs without losing provenance.
- Preserve transcript repairs and template visual metadata through load/save.
- Attach render reports with required checks and logs.

TypeScript tests:

- Project types represent split project metadata where exposed to the UI.
- Timeline edits call the Rust action bridge instead of local-only mutation.
- Template inspector updates fields and style through an action request.
- Caption inspector submits caption repair actions.
- Media bin renders imported and generated assets from project state.
- Render report panel renders report data from project artifacts.

Integration tests:

- A sample split project can load, generate an EDL-first timeline, save, reload, and render a draft.
- A simulated agent file edit fails with a field-specific validation error when it breaks a reference.
- A simulated transcript repair updates captions without losing selected source ranges.

## Implementation Slices

1. Add split manifest structs, split load, split save, and validation reports while keeping `VideoProject` as the runtime model.
2. Add schema version 1 to schema version 2 migration.
3. Add `ProjectAction` validation and application for timeline add, insert, move, trim, split, remove, and reorder.
4. Add transcript, caption repair, template update, generated asset lifecycle, and render report actions.
5. Route UI timeline, caption, template, media, and render report mutations through Rust actions.
6. Add MCP/app-server wrappers that expose the same action surface to agents.

## Risks

- Split files can drift if writes bypass Rust validation. Mitigation: provide a validator command and make the app validate on open, before render, and before applying agent edits.
- Migration can lose data if the runtime mapping is incomplete. Mitigation: add fixture projects with media, transcripts, templates, generated assets, and render reports before implementing migration.
- A large action surface can become inconsistent. Mitigation: keep action validation centralized in Rust and translate old `TimelinePatch` calls into the same path.
- UI optimism can hide validation failures. Mitigation: display validation errors near the affected timeline item, inspector field, media row, or render report.

## Acceptance Criteria

- A schema version 2 project opens from split files and produces the same `VideoProject` runtime shape the renderer and editor expect.
- Saving a project writes manifest, timeline, media index, transcript files, template overrides, generated asset metadata, and render reports as separate JSON files.
- Existing schema version 1 projects still load.
- A migration path creates split files and a migration report.
- The initial `ProjectAction` set covers add, insert, move, trim, split, remove, reorder, template field/style updates, transcript edits, caption repairs, generated asset lifecycle actions, and render reports.
- UI edits for timeline, captions, templates, generated assets, and render reports no longer rely on local-only project mutation.
- Agent-facing context can be built from split files without requiring a full in-memory prompt dump.
- EDL-first rules remain enforced for generated edits.
- Render review data is stored in project files and available to the UI and agents.
