# Temporal Rust Worker Migration Design

## Context

Palmier's docs describe an editor where chat, connected agents, media generation, timeline edits,
and export all happen in one project context. Video Creater has the same direction: generated media,
render jobs, transcription, Codex edits, and NLE export already record Temporal-shaped job metadata
inside text-file-editable project state.

The remaining gap is runtime execution. Current code records workflow IDs, task queue names,
workflow types, and activity types, but it does not yet run a Temporal Rust worker or start workflow
executions through a Temporal client.

Temporal's Rust SDK documentation defines the runtime shape this app must migrate toward:

- add the Rust SDK crates: `temporalio-client`, `temporalio-common`, `temporalio-macros`,
  `temporalio-sdk`, and `temporalio-sdk-core`;
- install or otherwise provide `protoc` for Temporal SDK protobuf build scripts;
- run a local Temporal service for development with `temporal server start-dev`;
- use the local service target `http://localhost:7233`;
- create workers with `WorkerOptions::new(task_queue)`;
- register workflow and activity implementations on the worker;
- start workflow executions against the same task queue from a Temporal client.

References:

- Palmier docs: https://www.palmier.io/docs
- Temporal Rust SDK guide: https://docs.temporal.io/develop/rust
- Temporal Rust quickstart: https://docs.temporal.io/develop/rust/quickstart

## Goal

Migrate Video Creater's queues, scheduling, and workflow execution from local command-only paths to
Temporal Rust while preserving the current project-file contract.

## Current State

- `src-tauri/src/workflows/mod.rs` defines stable workflow kinds, task queue, workflow IDs, and
  activity type names.
- React calls Rust to build Temporal-backed `JobSummary` records for media generation, render,
  NLE export, and related queued work.
- Generated asset completion, render reports, transcription gates, and export artifacts remain
  text-file-editable project actions.
- No Temporal SDK dependency, worker binary, workflow implementation, activity implementation, or
  client-start path exists yet.

## Target Architecture

### Worker Binary

Add a Rust worker binary, for example `src-tauri/src/bin/video-creater-temporal-worker.rs`, that:

1. loads Temporal client configuration;
2. connects to the configured Temporal service target;
3. creates a worker on `video-creater-workflows`;
4. registers every workflow and activity listed by `temporal_worker_manifest()`;
5. runs until terminated.

### Workflow Implementations

Implement typed workflow structs matching the existing workflow type names:

- `VideoCreaterGenerateMediaWorkflow`;
- `VideoCreaterRenderDraftWorkflow`;
- `VideoCreaterTranscribeMediaWorkflow`;
- `VideoCreaterCodexEditWorkflow`;
- `VideoCreaterExportNleXmlWorkflow`.

Each workflow should orchestrate activities only. Canonical project mutations stay inside validated
project actions.

### Activity Boundaries

Initial activities should map one-to-one to the existing manifest names so project job records,
worker registration, and UI status stay aligned.

Examples:

- `BuildFalGenerationRequest` calls the existing fal.ai request builder.
- `RunMediaProviderGeneration` submits/polls the provider without writing secrets to project files.
- `ImportGeneratedOutput` downloads/imports outputs under the project directory.
- `AttachGeneratedAssetResult` applies `completeGeneratedAsset`.
- `BuildNleXml`, `ValidateNleXml`, `WriteExportArtifact`, and `AttachExportReport` reuse the
  current NLE export module and project actions.

### Client Start Path

Queued UI actions should eventually choose between:

- local mock/development mode, which keeps the current split-project project-action path;
- Temporal mode, which starts the relevant workflow execution and records the resulting run id on
  the matching `JobSummary`.

The UI should not know Temporal crate names, activity lists, or workflow type names. It should keep
calling Rust command boundaries.

## Migration Slices

### Slice 1: Runtime Contract Metadata

Add Temporal Rust SDK prerequisite metadata to `temporal_worker_manifest()`:

- SDK crate names;
- local service target;
- local development server command;
- existing task queue, workflow types, and activity types.

This gives the app server, docs, and worker scaffolding one authoritative contract.

### Slice 2: Worker Scaffolding

Add SDK dependencies behind a Cargo feature such as `temporal-worker`.
Create the worker binary and compile it behind that feature.
Register no-op or deterministic test workflows first so local builds do not require a running
Temporal service unless the worker binary is executed.

Add a preflight report before live execution is enabled:

- include `protoc` and `temporal` in the manifest required tools;
- expose local service URL, local Web UI URL, local development server command, feature name, and worker run command;
- report missing required tools with install hints before a developer tries the feature build;
- surface the same readiness report in the right-rail Workflow queue inspector so queued jobs show
  task queue, service target, Web UI, local development server command, feature flag, worker
  command, and missing tool hints;
- keep manual workflow start controls disabled while the report says setup is missing;
- keep the report free of provider credentials or project secrets.

### Slice 3: Generate Media Workflow

Move real fal.ai image/video generation into the worker:

- use environment or app settings for provider credentials;
- preserve `recordGeneratedAsset` and `completeGeneratedAsset` project mutations;
- record run ids on jobs;
- keep the existing mock worker for development and tests.

### Slice 4: Render And Export Workflows

Move render draft/final WebM and NLE XML export to Temporal workflows while preserving current
artifact reports and project action validation.

### Slice 5: Transcription And Codex Edit Workflows

Move transcription, EDL-first Codex proposal generation, validation, and accepted project mutation
into Temporal workflows. The EDL-first rule remains mandatory: primary source ranges are selected
before captions, overlays, HyperFrames, or render layers are added.

## Acceptance Criteria

- `temporal_worker_manifest()` is the single source of truth for task queue, SDK prerequisites,
  workflow types, and activity types.
- A Temporal worker can register the same workflow/activity names shown in project job metadata.
- Starting a generation workflow records a run id and later completes the generated asset through
  validated project actions.
- Mock/local completion remains available for development without a running Temporal service.
- No provider credentials or API key material is written to source, docs, tests, logs, screenshots,
  or project files.
