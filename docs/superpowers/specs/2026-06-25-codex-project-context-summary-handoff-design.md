# Codex Project Context Summary Handoff

## Problem

Video Creater now writes a derived `context/project.json` for split projects, but Codex app-server prompts still list only the older core split files. Connected agents need a clear pointer to the summary and the newer sidecar/index folders so they can inspect project context before proposing timeline-native actions.

## Goal

Expose `context/project.json` in the Codex turn request under `Project files`, alongside the canonical split files and discovery indexes. The prompt should make the summary discoverable without copying sensitive Temporal start payloads or provider credentials.

## Contract

For schema-v2 split projects with a supplied project directory, `project_files_summary` includes:

- The absolute project root and canonical split files.
- The derived context summary path: `context/project.json`.
- Discovery index paths for transcripts, templates, generated assets, render reports, workflow jobs, and export artifacts.
- Sidecar path conventions for generated assets, render reports, workflow jobs, and export artifacts.
- A short note that `context/project.json` is derived discovery metadata, while mutations must still be returned as `projectActions`.

For non-split projects or missing project directories, existing fallback behavior remains unchanged.

## Security

The handoff must not include provider credential values, Temporal start request inputs, `providerCredentialEnvVar`, or any generated job payload. It may list file paths and path conventions because those are already local project paths supplied to the agent process.

## Tests

Update the Codex app-server turn-request test to assert:

- `context/project.json` appears in `Project files`.
- Job and export sidecar/index paths appear.
- The prompt explains the summary is derived metadata.
- Sensitive job payload keys are not present in the project-files handoff.
