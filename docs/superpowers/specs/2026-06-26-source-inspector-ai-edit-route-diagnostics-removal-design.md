# Source Inspector AI Edit Route Diagnostics Removal Design

## Context

The source inspector now exposes inline AI edit controls for imported and generated media, but both panels still show a `Source AI edit workflow route` card with workflow, queue, provider, placement, and mock-worker metadata. Palmier presents AI editing as direct clip actions in the editing surface, not as infrastructure routing inside the edit form.

## Goal

Remove visible workflow-route diagnostics from the imported and generated source-inspector AI edit panels while keeping the existing Temporal-backed queue behavior unchanged.

## Behavior

- Imported media still shows `Imported AI edit`, `Queue referenced shot`, and `Queue upscale` when the same callbacks are available.
- Generated media still shows `Generated AI edit`, the `Single` / `Variation set` mode control, prompt editing, variation/replacement actions, and upscale actions when available.
- Neither imported nor generated AI edit panels render a `Source AI edit workflow route` region.
- The route card's internal fields, including workflow type, task queue, provider, placement, and mock worker status, are not moved elsewhere in the inspector.
- Workflow and queue metadata may remain visible in job, run-status, timeline, media-card, or
  activity surfaces where it helps diagnose background work, but not in generated Source Inspector
  details or AI edit controls.

## Non-Goals

- Do not change Temporal workflow names, task queues, fal.ai provider payloads, or queue callback signatures.
- Do not change project-file serialization, generated asset schemas, or timeline mutation behavior.
- Do not remove the `Single` / `Variation set` selector because it controls request shape rather than diagnostic routing.

## Verification

- Source-inspector tests assert that imported and generated AI edit panels keep their queue actions but do not render the route region.
- Existing workspace tests continue to verify that queued generation and upscale actions produce the same Temporal-backed project actions.
- Browser QA selects imported and generated source clips and confirms the inspector shows edit actions without workflow-route diagnostics.
