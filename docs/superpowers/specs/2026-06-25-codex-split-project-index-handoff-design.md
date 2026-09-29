# Codex Split Project Index Handoff

## Context

Palmier-style agent editing depends on the assistant understanding project media, generated clips,
workflow jobs, templates, renders, exports, and timeline state before proposing edits. Video Creater
already persists those areas as text-file-editable split-project files with scan-first indexes and
canonical sidecars.

The Codex app-server project-file handoff currently names several sidecar folders, but not every
index file agents should inspect first. That makes connected agents infer path conventions instead
of following an explicit project map.

## Goal

List every scan-first split-project index in `project_files_summary` before the matching canonical
sidecar path.

## Requirements

- Include `transcripts/index.json`, `templates/index.json`, `generated/index.json`,
  `renders/index.json`, `jobs/index.json`, and `exports/index.json`.
- Keep `timeline.json`, `media/index.json`, and `context/project.json` in the summary.
- Clearly mark sidecar patterns as canonical mutation targets and index files as discovery
  surfaces.
- Preserve the existing fallback messages for missing project directories or non-schema-v2
  projects.
- Do not change proposal validation, project action validation, split-project storage, Temporal,
  fal.ai, or UI behavior.

## Verification

- Rust unit test proves a schema-v2 context with a project directory includes each scan-first index
  and the canonical sidecar patterns.
- Existing Codex context tests continue passing.
