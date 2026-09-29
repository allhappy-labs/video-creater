# Temporal Generate Media Worker Registration Cleanup

## Context

The generate-media workflow now executes `BuildFalGenerationRequest` followed by
`RunMediaProviderGeneration`. The workflow metadata and Temporal start request contract were updated
to advertise only those two activities, but the feature-gated worker activity struct still registered
the older placeholder activities:

- `RecordGenerationPrompt`;
- `ImportGeneratedOutput`;
- `AttachGeneratedAssetResult`.

That left the real worker surface broader than the manifest shown to the editor, project files, and
agent context.

## Goal

Keep the Temporal worker registration surface aligned with the current generate-media workflow
contract.

## Behavior

- `temporal_worker_manifest()` remains the source of truth for advertised activity names.
- Feature-gated worker options register the manifest activities.
- Feature-gated worker options no longer register obsolete generate-media placeholder activities.
- Existing render, transcription, Codex edit, export, and NLE XML placeholder activities remain
  registered until their workflow migrations are implemented.

## Non-Goals

- No render/export/transcription workflow migration in this slice.
- No provider behavior change.
- No project schema migration.
- No removal of historical project files or old saved job records.

## Verification

- Add a feature-gated worker registration test proving obsolete generate-media activities are absent.
- Existing Temporal integration and workflow unit tests continue passing.
