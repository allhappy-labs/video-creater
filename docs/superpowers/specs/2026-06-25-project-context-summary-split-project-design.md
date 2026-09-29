# Split Project Context Summary

## Problem

Palmier-style connected agents need a fast way to understand a project before editing text files. Video Creater already stores canonical state across the manifest, timeline, media index, sidecars, and discovery indexes, but an agent must currently know every path convention before it can inspect the right files.

## Goal

Write a derived `context/project.json` summary on every split-project save. The file should give agents a bounded map of the project layout, file indexes, object counts, and latest activity IDs without duplicating canonical state.

## Non-Goals

- Do not load canonical project state from `context/project.json`.
- Do not validate project correctness from `context/project.json`.
- Do not store provider inputs, credentials, absolute project directories, prompts beyond existing sidecars, or raw Temporal start payloads.

## File Contract

`context/project.json` uses schema version `1` and contains:

- `projectId`, `name`, and `updatedAt`.
- `files` with relative paths for manifest, timeline, media, transcript, template, generated, render, job, export, and log locations.
- `indexes` with relative paths to all generated discovery indexes.
- `counts` for media, folders, timeline tracks, timeline items, transcripts, templates, generated assets, render reports, workflow jobs, and export artifacts.
- `latest` with optional IDs for the latest generated asset, render report, workflow job, and export artifact.

Latest IDs are deterministic:

- Generated asset: newest `createdAt`, then lexicographic ID.
- Render report: newest `createdAt`, then lexicographic ID.
- Workflow job: newest `updatedAt`, then lexicographic ID.
- Export artifact: newest `createdAt`, then lexicographic ID.

## Persistence

`save_split_project` creates `context/` and writes `context/project.json` after the other split files and indexes have been written. The write report includes the context file path. Load and validation flows ignore the context summary because it is derived metadata and can be safely regenerated.

## Validation

Tests cover:

- `save_split_project` writes `context/project.json`.
- The summary reports relative paths, indexes, counts, and latest IDs.
- The raw summary does not contain `projectDir`, absolute temp paths, or `providerCredentialEnvVar`.
- `load_split_project` and `validate_split_project` ignore edited context metadata.
