# Temporal Generate Media Activity Contract Cleanup

## Context

`VideoCreaterGenerateMediaWorkflow` now executes two real activities:
`BuildFalGenerationRequest` and `RunMediaProviderGeneration`. The provider-run activity owns provider
submission, polling, output download, and applying generated-asset completion actions to the split
project. The declared generate-media activity list still exposed the older five-step plan, including
placeholder activities that are no longer scheduled by the workflow.

Palmier-style workflow visibility should show the work the agent and editor actually perform. A
queue card that lists unused activities makes the workflow harder to reason about and weakens the
text-file-editable job contract.

## Goal

Make the generate-media workflow metadata and start request activity list match the activities the
Temporal workflow executes.

## Behavior

- `TemporalWorkflowKind::GenerateMedia.activity_types()` returns:
  - `BuildFalGenerationRequest`;
  - `RunMediaProviderGeneration`.
- Generated media job summaries and start requests built from Rust helpers inherit that two-activity
  list.
- The sample editor project uses the same two-activity list so the visible queue chrome reflects the
  worker behavior.
- Older placeholder activity implementations can remain registered temporarily for compatibility,
  but they are no longer advertised as the active generate-media workflow path.

## Non-Goals

- No schema migration for existing saved project files in this slice.
- No removal of registered placeholder activity functions yet.
- No provider runtime behavior change.
- No visual redesign.

## Verification

- Add a Rust workflow contract test for the executed generate-media activity sequence.
- Existing workflow and fal tests continue passing.
- The Temporal feature-gated worker path still compiles under `--features temporal-worker`.
