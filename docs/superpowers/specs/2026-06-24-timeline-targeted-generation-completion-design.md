# Timeline-Targeted Generation Completion Design

## Context

Palmier makes media generation feel native to editing: users can generate video on the timeline, regenerate or swap clips in place, and use chat or agents to place generated assets directly on the timeline. Video Creater now exposes a `Library` / `Timeline` placement intent in the Media generation composer, but that intent is local UI state only. Completing a queued mock generation still files the output into the project library and requires a separate insert action.

Video Creater already has durable text-file-editable project actions for generated asset provenance, generated output completion, and timeline insertion. The next slice should connect the new placement intent to the existing action path with a backward-compatible optional metadata field instead of adding a new timeline-specific action.

## Goal

When a user queues a generation with `Timeline` selected, keep the queued asset visibly marked as timeline-targeted and, during mock completion, append the completed output to the appropriate timeline track through the existing validated `addItems` project action.

## Behavior

- `MediaGenerationRequest` carries a frontend-only `placementIntent` value of `library` or `timeline`.
- The composer still defaults to `Library`, and closing/reopening resets to `Library`.
- Queued `recordGeneratedAsset` actions store the placement intent on the generated asset as optional `placementIntent`.
- Generated asset cards show `Timeline target` when that metadata is present.
- Completing a queued timeline-targeted generation with the mock worker:
  - first runs the existing mock completion command, which records the output and updates workflow status,
  - then appends the completed output to the first compatible timeline track using the same `addItems` action shape as the current manual insert path,
  - selects the inserted timeline item and generated media output after the action succeeds.
- Library-targeted generations keep the current behavior: completion adds the output to generated assets/media only.

## Data Flow

1. `MediaBin` includes `placementIntent` in the `onGenerateMedia` request.
2. `EditorWorkspace.queueMediaGeneration` writes the intent into the generated asset.
3. `MediaBin` reads `placementIntent` defensively from `GeneratedAsset` and renders the target badge/status.
4. `EditorWorkspace.completeMockGeneration` inspects the completed asset metadata in the returned project and, if it is timeline-targeted, builds a generated-output timeline item and applies `addItems` to the returned project through `applyProjectActionToSplitProjectFolder`.

## Non-Goals

- No new Rust project action variant or timeline-specific completion action.
- No real fal.ai worker execution changes.
- No Temporal worker scheduling changes beyond the existing queued/completed job records.
- No timeline insertion for failed or still-running assets.
- No insertion into locked or missing compatible tracks.

## Testing

- `MediaBin` proves Timeline placement queues a request with `placementIntent: "timeline"`.
- `MediaBin` proves timeline-targeted generated assets show the visible target status.
- `EditorWorkspace` proves queuing a timeline-targeted generation records `placementIntent` metadata.
- `EditorWorkspace` proves completing a timeline-targeted mock generation applies completion and then adds the generated output to the timeline.
- Existing Library placement tests are updated to prove the default request remains `placementIntent: "library"`.

## Browser QA

- Desktop: queue/open the generation composer and verify the Timeline placement control remains readable.
- Generated asset list: verify a timeline-targeted queued/completed card exposes `Timeline target` without crowding model/status text.
- Narrow width: verify the placement control and target status do not overlap library cards or the composer footer.
