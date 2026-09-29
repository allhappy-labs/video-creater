# Export Artifact Index Drift Validation

## Context

Video Creater stores export deliverables as editable sidecars under `exports/<artifact-id>/artifact.json` and writes `exports/index.json` as the quick discovery surface for agents. The sidecar is canonical, but the index is the file agents will scan first when deciding which NLE XML, MP4, MOV, or WebM export to inspect or hand off.

Palmier-style agent editing depends on these project files staying trustworthy after manual edits. Other split-project discovery indexes already report drift during validation; export artifacts need the same guardrail.

## Decision

`validate_split_project` will validate `exports/index.json` when export artifact sidecars exist.

The validator will:

- Treat export artifact sidecars as canonical.
- Report invalid `exports/index.json` JSON as a validation issue instead of failing the full validation run.
- Require non-empty, unique `artifactId` values in index entries.
- Require index `path` values to be project-relative.
- Report index entries whose `artifactId` has no matching `exports/<artifact-id>/artifact.json` sidecar.
- Compare `kind`, `format`, `path`, `mimeType`, `jobId`, and `createdAt` against the matching sidecar.
- Report every sidecar missing from the index when the index exists.

`load_split_project` will continue ignoring `exports/index.json` metadata. That keeps the sidecar-first text editing contract intact while still giving agents and users validation feedback for stale discovery data.

## Scope

In scope:

- Rust split-project validation.
- Focused integration tests for stale and incomplete export artifact indexes.
- A spec documenting the canonical sidecar/index relationship.

Out of scope:

- UI changes.
- Export generation behavior.
- Changing the saved export index schema.
- Loading export artifacts from `exports/index.json`.

## Verification

- Focused export-index validation tests fail before implementation.
- Focused project split tests pass after implementation.
- Formatting and diff checks pass.
- Secret scan confirms no generation credentials were written to docs or source.
