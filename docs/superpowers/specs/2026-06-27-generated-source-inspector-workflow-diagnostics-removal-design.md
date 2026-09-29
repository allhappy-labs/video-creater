# Generated Source Inspector Workflow Diagnostics Removal Design

## Context

The Source Inspector now shows generated clip recipe, file, reference, prompt, and AI edit controls inline. It still renders Temporal workflow diagnostics inside the generated-source details and AI edit surfaces: workflow ID, workflow type, queue, run ID, activities, credential label, and start-request state. Palmier keeps generated media inspection focused on what was created and how to reuse it, not on infrastructure routing.

Palmier reference: https://www.palmier.io/docs

## Goal

Remove Temporal workflow diagnostics from the generated Source Inspector while keeping generated recipe context, reference inspection, prompt provenance, and AI edit actions visible.

## Behavior

- `Generated details` still shows recipe summary, file metadata, references, prompt, and generated output actions.
- `Generated AI edit` still shows variation mode, prompt editing, upscale/variation/replacement actions, and replacement target context when available.
- The Source Inspector no longer renders `Generated workflow status`.
- The generated AI edit panel no longer shows workflow status, workflow type, task queue, run ID, activity list, credential label, or start-request state.
- Workflow metadata remains available in Temporal-backed project jobs, media generation cards, timeline workflow badges, project workflow/activity surfaces, and serialized project state.
- Queue payloads, Temporal workflow names, task queues, fal.ai provider metadata, and generated asset schemas remain unchanged.

## Verification

- Update `SourceClipInspector` and workspace tests to assert generated-source inspector details/editing omit workflow diagnostics while retaining recipe, references, prompt, and queue actions.
- Keep workflow metadata tests for job/activity/timeline/media-card surfaces unchanged.
- Browser-smoke a selected generated source and confirm the right inspector shows generated details plus AI edit actions without workflow route internals.
