# Temporal Generate Media Provider Activity Boundary

## Context

The generate-media workflow can now build fal queue submissions from split project files, run a
supplied submission through an injected client, download the provider output, and attach completion
actions back to the project folder. The Temporal worker still exposed `RunMediaProviderGeneration`
as a registration echo, so a real worker execution would not yet turn a queued generated asset into
editable media.

Palmier-style agent editing requires generated assets to move from queue records into normal project
media without frontend memory or ad hoc side effects. Temporal activities also need JSON-safe payloads
and results, while provider credentials stay in worker configuration.

## Goal

Add a serializable provider-run activity boundary for `RunMediaProviderGeneration` that receives a
Temporal start request, fal queue submission, completion timestamp, optional Temporal run id, and
polling options. The activity boundary executes the existing split-project run-and-attach helper and
returns a compact JSON-safe summary of the provider request and project writes.

## Behavior

- Activity input includes:
  - `startRequest`;
  - `submission`;
  - `updatedAt`;
  - optional `runId`;
  - `maxStatusPolls` and `pollIntervalMillis`.
- The pure helper accepts an injected blocking HTTP client and explicit credential value for tests.
- The feature-gated Temporal activity reads the credential from `submission.authEnvVar` at worker
  runtime, builds a blocking HTTP client, and calls the same pure helper.
- The output includes request id, project id, asset id, completed job status, run id, downloaded
  output path, written files, and removed files.
- The output must not include provider credentials.

## Non-Goals

- No live workflow activity sequencing change in this slice; non-generate-media activities can stay
  registered placeholders.
- No direct project mutation outside the split-project project-action writer.
- No real fal network call in automated tests.
- No frontend UI changes.

## Verification

- A workflow test runs the provider activity helper against a fake local fal queue server, then
  verifies:
  - the fake provider receives the credential only in the Authorization header;
  - the activity output serializes without the credential;
  - the output points at `generated/<asset-id>/fal-output.png`;
  - the split project now contains completed generated media;
  - the job is completed with the supplied Temporal run id.
- The Temporal feature-gated worker path must compile under `--features temporal-worker`.
