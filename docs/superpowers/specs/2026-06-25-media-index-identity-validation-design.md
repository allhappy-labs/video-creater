# Media Index Identity Validation

## Context

Palmier-style agent editing depends on a project library that agents can scan, reference, and edit without guessing. Video Creater already keeps media and folders in the text-editable `media/index.json`, and timeline clips, generated-asset provenance, folders, transcripts, and Codex `@media` references all depend on stable media identifiers.

The current split-project validator catches duplicate media IDs, unsafe media paths, missing folders, and folder cycles. It does not reject empty or path-unsafe media and folder IDs. A hand-edited `media/index.json` can therefore introduce identifiers that are ambiguous in issue paths, awkward in agent prompts, or inconsistent with the safe identifier rules used by generated assets, workflow jobs, templates, renders, and export artifacts.

## Decision

`validate_split_project` will treat media asset IDs and media folder IDs as required safe path segments.

The validator will report:

- Empty media asset IDs.
- Media asset IDs containing slashes, backslashes, or parent path components.
- Empty media folder IDs.
- Media folder IDs containing slashes, backslashes, or parent path components.

Existing checks remain intact:

- Duplicate asset IDs.
- Duplicate folder IDs.
- Missing folder references.
- Folder self-reference and cycle detection.
- Unsafe `relativePath` values.

## Scope

In scope:

- Rust split-project validation for `media/index.json`.
- Integration tests for invalid media and folder IDs.

Out of scope:

- Changing the saved media index schema.
- Creating per-media sidecar files.
- UI changes.
- Migrating existing valid media IDs.

## Verification

- Focused validation tests fail before implementation.
- Focused project split tests pass after implementation.
- Formatting, diff checks, placeholder scan, and secret scan pass.
