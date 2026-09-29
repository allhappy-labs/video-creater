# Temporal Generate Media Build Request Activity

## Context

Video Creater now starts generate-media Temporal workflows with a self-contained runtime payload:
`{"startRequest": <TemporalWorkflowStartRequest>}`. The worker already has boundaries for loading a
split project, building a fal queue submission, running the provider, and attaching completion
actions. The feature-gated `VideoCreaterGenerateMediaWorkflow` still behaved like a registration
echo, and `BuildFalGenerationRequest` did not yet execute the project-file-backed fal request
builder.

Palmier-style generated media needs agent and manual generation requests to become normal project
media from durable project files, not transient frontend snapshots. The first real Temporal workflow
step is therefore to build the provider request from the persisted start request and split project.

## Goal

Replace the generate-media workflow echo with a first real Temporal activity handoff:
`VideoCreaterGenerateMediaWorkflow` schedules `BuildFalGenerationRequest`, and that activity returns
a validated fal queue submission loaded from the split project folder.

## Behavior

- `BuildFalGenerationRequest` accepts the generate-media runtime payload containing `startRequest`.
- The activity decodes the full start request, validates the generate-media workflow boundary, loads
  the split project from `projectDir`, finds the generated asset, and builds a fal queue submission.
- The returned submission includes endpoint, method, URL, request input, and credential environment
  variable name only.
- The feature-gated `VideoCreaterGenerateMediaWorkflow` schedules `BuildFalGenerationRequest` via
  Temporal SDK `start_activity` instead of returning `status: registered`.
- Other workflow types can remain registration stubs in this slice.

## Non-Goals

- No provider execution sequencing in the workflow yet; `RunMediaProviderGeneration` remains the next
  workflow activity to wire after the build-request activity output is available.
- No frontend UI changes.
- No real fal network calls in automated tests.
- No direct project mutation in this build-request activity.

## Verification

- A workflow test saves a split project with a queued fal generated asset, calls the
  `BuildFalGenerationRequest` activity helper with the runtime payload, and verifies the returned fal
  submission matches the project asset and does not include a credential value.
- The Temporal feature-gated worker path compiles under `--features temporal-worker`.
- Existing workflow and fal tests continue passing.
