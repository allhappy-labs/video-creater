# Selected Generated Source Settings Context Design

## Problem

Palmier makes generated media inspectable from the editor: model, prompt, first/last frames, references, resolution, duration, and aspect ratio remain visible when a generation is selected. Video Creater already shows those settings for selected generated timeline clips in the Codex rail, but selected generated sources only show model, prompt, and references. This creates a gap when an editor selects a generated output from the media library before rerunning, replacing, or inserting it.

## Goals

- Show selected generated source output settings in the Codex rail.
- Reuse existing compact setting labels: resolution, duration, fps, and aspect ratio when present.
- Thread settings from `generatedAsset.settings` through the selected media context without changing project files.
- Keep existing selected source reference, prompt, variation, replacement, and insert actions unchanged.

## Non-Goals

- No schema changes, provider changes, or generation request changes.
- No editable settings controls in the selected source block.
- No changes for imported, non-generated media.

## Design

`AgentSelectedMediaContext` gains optional `settings?: GeneratedAssetSettings | null`. `EditorWorkspace` fills it from the generated asset behind the selected media output. `AgentPanel` reuses the existing generated settings label helper and renders chips next to the generated source model/reference summary.

## Acceptance Criteria

- Selected generated source context can include generation settings.
- AgentPanel renders `1280x720`, `4s`, `24 fps`, and `16:9` chips when those settings are provided.
- EditorWorkspace passes generated asset settings into the Codex selected source context.
- Existing selected generated source prompt, reference open/mention, variation, replacement, and insert tests continue passing.
