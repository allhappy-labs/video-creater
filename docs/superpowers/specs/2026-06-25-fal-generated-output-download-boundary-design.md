# fal.ai Generated Output Download Boundary

## Context

Video Creater can now build fal.ai generation payloads, submit them to the queue, fetch queue status,
and retrieve the final model result JSON. The next worker-owned boundary is `ImportGeneratedOutput`:
copy the returned media URL into the local split project before project actions record the generated
asset output.

## Goal

Add a Rust helper that downloads a `FalGeneratedOutputImport` source URL into the project-relative
generated output path that will later be used by `completeGeneratedAsset`.

## Behavior

- The helper accepts a project directory and `FalGeneratedOutputImport`.
- It validates `output.relativePath` before any network or filesystem write.
- Valid output paths must be project-relative and under `generated/`.
- Parent-directory, absolute, prefix, empty, directory-only, and reserved `asset.json` paths are
  rejected.
- The helper creates parent directories, writes to a temporary `.part` file, flushes it, and renames
  it to the final output path.
- The helper returns the final absolute output path.
- It does not mutate project JSON, add media records, or complete generated assets.

## Verification

- Provider tests use a local HTTP listener to prove bytes are written under
  `generated/<asset-id>/`.
- Provider tests reject path traversal before files are written.
- Existing fal request, queue, result, and completion-action tests keep passing.

## Non-Goals

- No live fal.ai media download in CI.
- No MIME sniffing or media probing in this slice.
- No Temporal worker activity registration in this slice.
- No project action mutation in this slice.
