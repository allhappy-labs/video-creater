# Temporal Start Result Action Design

## Context

Generated-media jobs can now persist a Temporal `startRequest`, and the editor can inspect that
request. The next runtime gap is the client-start handoff: once a Temporal client starts a workflow,
Video Creater needs a small, validated contract that turns the returned run id into a project
mutation without rebuilding workflow identity from UI state.

## Goal

Add a Rust workflow helper that accepts a recorded job, Temporal run id, and timestamp, then returns
the existing `updateJobStatus` project action needed to mark the workflow as running.

## Contract

- The helper requires `job.workflow` and `job.startRequest`.
- The start request must match workflow id, workflow type, task queue, and activity type order.
- The returned action is `updateJobStatus` with `status: "running"` and the supplied non-empty
  `runId`.
- The helper never reads provider credentials and never serializes full start-request input.

## Non-Goals

- Do not connect to a live Temporal service in this slice.
- Do not poll workflow status or complete generated assets.
- Do not change the frontend queueing path.

## Verification

- Rust workflow tests prove a matching generated-media job returns the expected action.
- Rust workflow tests reject missing start requests, mismatched start requests, and blank run ids.
- Existing project-action validation continues to own canonical mutation.
