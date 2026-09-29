# Mock Generation Completion Replacement UI

## Context

Palmier lets editors tweak or rerun an AI-generated clip and swap the result into the timeline without leaving the edit. Video Creater now has a mock completion command for queued generated assets and a validated `completeGeneratedAsset.replacement` project-action primitive, but the mock command and Media Bin UI cannot exercise both together.

## Goal

Let development users complete a queued mock generation and replace the selected timeline clip in one command path.

## Behavior

- `complete_mock_generated_asset_in_split_project_folder` accepts an optional `replacementItemId`.
- When present, Rust builds the deterministic mock output and sends `completeGeneratedAsset.replacement` with `itemId: replacementItemId` and `mediaId` set to that mock output id.
- The existing project-action validator remains authoritative; invalid replacement item ids or mismatched output state fail the whole write.
- `MediaBin` shows `Complete mock & replace` for queued or running generated assets only when a replacement target label is present.
- `EditorWorkspace` passes the selected replacement timeline item id to the mock completion command when that action is used.

## Non-Goals

- No real fal call in this slice.
- No automatic replacement for completed generated assets.
- No new Temporal workflow type; this prepares the completion activity contract.

## Acceptance

- Frontend project wrapper accepts and forwards `replacementItemId`.
- Media Bin tests prove the replace action is only exposed with a pending generation and replacement target.
- EditorWorkspace tests prove the replace action sends `replacementItemId` and updates the visible project state from the command result.
- Rust tests prove the split-project mock completion command can complete a queued asset and swap a timeline clip through the existing project-action validator.
