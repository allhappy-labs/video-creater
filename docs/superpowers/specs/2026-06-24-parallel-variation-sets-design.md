# Parallel Variation Sets Design

## Context

Palmier makes generated media feel native to editing: its docs describe generating video on the timeline, rerunning or tweaking AI-generated clips by prompt, inspecting prompt/reference provenance, and using chat or connected agents to generate assets in context without leaving the project. The provided Palmier screenshots also show a common creative workflow: the assistant asks for several variations of a selected image, reports the named directions, and the resulting AI assets appear in the media library while the editor timeline stays available.

Video Creater already has the core primitives for one variation at a time:

- `SourceClipInspector` and the media-bin generated-source detail view can queue a single generated variation from a generated asset.
- `AgentPanel` can queue a single selected-source variation from the Codex prompt.
- `EditorWorkspace.queueGeneratedClipVariation` records a `VideoCreaterGenerateMediaWorkflow` job and a queued `recordGeneratedAsset` action in one split-project batch.
- Generated outputs preserve prompt, references, model, settings, parent lineage, retry lineage, workflow state, and optional timeline placement intent.

The missing slice is a structured way to queue a small set of named variation prompts together, so the user or agent can explore several directions from the same source without repeating the same action four times.

## Goal

Add a Palmier-style "variation set" workflow for selected generated sources: an editor can queue 3-4 named prompt directions from the selected generated asset, and Video Creater records each direction as its own Temporal-backed generated asset while keeping them visibly grouped by shared lineage.

## Recommended Approach

Implement this as a frontend orchestration layer over the existing project-action and Temporal job primitives.

- Keep `ProjectAction` unchanged.
- Keep each generated variation as its own `recordGeneratedAsset` action and `recordJob` action.
- Add a small UI model for variation set drafts, not a new persisted project entity.
- Persist grouping through existing fields first: `parentAssetId`, `retryOfAssetId`, `name`, prompt text, and workflow job ids.
- Do not add `variationSetId` in this slice; if grouping later needs a durable id, that should be a separate schema design.

This keeps the file-editable project contract simple and lets real Temporal workers process each generation independently.

## User Experience

### Source Inspector

When a generated output is selected and `AI Edit` is open, the panel gains a compact `Variation set` mode beside the current single-prompt flow.

Default directions:

1. `Storm Clouds` - darker contrast and more atmosphere.
2. `Radiant Backlight` - brighter rim light and a more hopeful feeling.
3. `Noir Drama` - deeper shadows and cinematic contrast.
4. `Soft Melancholy` - gentler diffusion and quieter mood.

The editor can edit each name and prompt before queueing. Empty rows are skipped. A `Queue variation set` button is enabled when at least two rows have non-empty prompts.

### Codex Rail

When the selected Codex source is a generated asset, the selected-source block keeps the current `Queue variation` action and adds `Queue variation set` as a secondary action. The Codex prompt is used as the base instruction; Video Creater expands it into named variation directions using deterministic local templates for this slice.

Example:

- User prompt: `create four variations of this image`
- Queued prompts:
  - `Storm Clouds: create four variations of this image, with dramatic storm clouds, stronger contrast, and a more turbulent atmosphere.`
  - `Radiant Backlight: create four variations of this image, with serene backlight, soft cloud wisps, and a more transcendent mood.`
  - `Noir Drama: create four variations of this image, with deep shadows, high contrast noir lighting, and a surreal cinematic feel.`
  - `Soft Melancholy: create four variations of this image, with gentle diffused light, vintage film grain, and a quieter introspective mood.`

The assistant/status transcript should summarize the queued directions in plain language, matching Palmier's "all four variations are generating" interaction.

### Media Library

Queued and completed variations continue to appear in `AI generations`. Each row/card should make lineage scannable:

- `variation of <parentAssetId>`
- `retry of <sourceAssetId>`
- the direction name when present
- workflow status
- `Timeline target` when inherited from a timeline-targeted source

No separate variation-set folder is created automatically. If the source asset is already in a media folder, the variation set inherits `targetFolderId`.

## Data Flow

1. `AgentPanel` or `SourceClipInspector` emits a list of variation drafts: `{ name, prompt }[]`.
2. `EditorWorkspace.queueGeneratedClipVariationSet(assetId, drafts)` resolves the source generated asset.
3. For each non-empty draft, generate a unique variation asset id and one Temporal job summary via `buildTemporalJobSummary("generate_media", project.id, variationAssetId, createdAt)`.
4. Apply one split-project batch containing alternating `recordJob` and `recordGeneratedAsset` actions, or all `recordJob` actions followed by all `recordGeneratedAsset` actions.
5. Each generated asset copies source kind, model, references, settings, target folder, and placement intent, then records:
   - `name`: draft name,
   - `prompt`: draft prompt,
   - `parentAssetId`: source parent if present, otherwise source id,
   - `retryOfAssetId`: source id,
   - `outputs`: empty,
   - `status`: `queued`.
6. Existing mock completion and real generation provider paths complete each asset independently.

## Temporal And Provider Requirements

- Every variation in the set is represented by a distinct `VideoCreaterGenerateMediaWorkflow` job.
- The UI must not fake a single shared workflow for the whole set.
- The current mock path remains valid for development.
- Real fal.ai execution continues to use the existing provider contract:
  - `fal-ai/flux/schnell` for text-to-image,
  - `fal-ai/wan-25-preview/text-to-video` for text-to-video.
- The API key is never stored in project files, specs, source code, tests, snapshots, screenshots, logs, or committed artifacts.

## Non-Goals

- No new Rust project action variant.
- No new persisted project-level `VariationSet` type in this slice.
- No automatic timeline replacement when variations complete.
- No automatic timeline insertion unless the inherited `placementIntent` is already `timeline` and the existing completion behavior handles it.
- No LLM prompt expansion in this slice; direction expansion is deterministic and local.
- No real Temporal worker implementation changes beyond queue records already used by generated media jobs.
- No batch cancellation UI.

## Error Handling

- If the source generated asset is missing, do nothing and surface the existing workspace error pattern if a caller needs feedback.
- If fewer than two prompts are valid, disable the queue button.
- If any Temporal job summary build or project action batch fails, no partial UI optimism should be shown; rely on the split-project batch result.
- If one variation later fails, other variations remain independent and can complete.

## Testing

### Unit And Component Tests

- `SourceClipInspector` renders a variation-set mode for generated sources and calls the callback with at least two named prompts.
- `AgentPanel` renders `Queue variation set` for selected generated sources and emits deterministic directions from the current Codex prompt.
- Existing single `Queue variation` tests keep passing.
- Media-bin generated list shows direction names and existing lineage labels without hiding workflow status.

### Workspace Integration Tests

- `EditorWorkspace` queues a variation set through one split-project batch with one `recordJob` and one `recordGeneratedAsset` action per direction.
- Each generated asset preserves source model, settings, references, target folder, parent lineage, retry lineage, and placement intent.
- A timeline-targeted source queues timeline-targeted variation assets, so existing mock completion can insert them on completion.

### Browser QA

- Desktop: select a generated clip, open `AI Edit`, verify variation-set rows fit in the right inspector without text overlap.
- Desktop with Codex rail: selected-source block exposes both single variation and variation-set actions without crowding insert/replace/upscale actions.
- Narrow width: the variation-set editor remains usable, rows stack cleanly, and the composer footer remains visible.

## Acceptance Criteria

- A selected generated asset can queue at least two named variations with one user action.
- The queued assets are visible as normal generated assets with clear direction names and lineage.
- Each variation has its own Temporal-backed `generate_media` job summary.
- The implementation uses the existing split-project action batch path.
- No new schema is required for old projects to load.
- No secret material is written to the repo or project files.
