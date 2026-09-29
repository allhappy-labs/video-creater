# Generate Media UI Activity Contract Cleanup

## Context

Video Creater's Temporal generate-media workflow now runs two worker activities:
`BuildFalGenerationRequest` and `RunMediaProviderGeneration`. The Rust workflow manifest and worker
registration tests enforce that contract, and obsolete activities are no longer registered.

Several frontend and Codex context fixtures still describe queued or running generate-media jobs with
the older activity names: `RecordGenerationPrompt`, `ImportGeneratedOutput`, and
`AttachGeneratedAssetResult`. Those fixtures drive the visible workflow queue, source inspector,
media-bin status, and agent context examples, so they can make the editor and tests document a
workflow that no longer exists.

## Goal

Align UI-facing and agent-context generate-media workflow fixtures with the current Temporal worker
contract.

## Behavior

- Generate-media workflow examples and test fixtures list only:
  - `BuildFalGenerationRequest`
  - `RunMediaProviderGeneration`
- Workflow queue UI tests expect `2 activities` for generate-media jobs using the current contract.
- Source inspector, media bin, Codex rail, and frontend project API tests keep showing the current
  activity names where they expose workflow status.
- Rust Codex context tests no longer include obsolete generate-media activity names in sample agent
  context text.
- Historical docs and migration notes may keep old activity names when they describe older designs.

## Non-Goals

- No Temporal worker implementation change.
- No project schema change.
- No workflow start request payload change.
- No UI redesign beyond fixture-visible copy.

## Verification

- Add frontend coverage that a queued generate-media job fixture exposes the current two-activity
  contract and not obsolete activity names.
- Update affected frontend and Rust tests to the current activity list.
- Run focused workflow/status tests and the repository secret-fragment scan.
