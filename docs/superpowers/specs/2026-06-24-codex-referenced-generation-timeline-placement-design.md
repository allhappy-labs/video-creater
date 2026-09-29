# Codex Referenced Generation Timeline Placement Design

## Context

Palmier's docs describe chat that can generate assets and place them on the timeline. Video Creater
already has the pieces for that workflow: Codex can queue referenced shots from a selected source or
typed `@` mention, generated assets can carry `placementIntent: "timeline"`, and timeline-targeted
mock completion inserts completed generated outputs through the existing validated `addItems`
project action.

The gap is that Codex referenced-shot actions still queue generated assets with
`placementIntent: "library"`, so the later completion path treats them as library-only outputs.

## Goal

Make Codex-originated referenced shots timeline-targeted by default, using the existing
`placementIntent` and completion behavior.

## Behavior

- `Queue referenced shot` from the Codex selected-source block queues the generated asset with
  `placementIntent: "timeline"`.
- `Queue referenced shot from mention` queues the generated asset with `placementIntent: "timeline"`.
- The queued job, fal.ai model, references, settings, transcript rows, and Temporal-shaped workflow
  metadata remain unchanged.
- Source Inspector imported-clip AI edits keep their existing library placement behavior.
- No new project action or Temporal workflow type is introduced; completion continues to use the
  existing timeline-targeted generated asset path.

## Verification

- `EditorWorkspace` proves selected-source Codex referenced generation records
  `placementIntent: "timeline"`.
- `EditorWorkspace` proves typed-mention Codex referenced generation records
  `placementIntent: "timeline"`.
- Existing timeline-targeted mock completion tests continue to prove completion inserts the output.
