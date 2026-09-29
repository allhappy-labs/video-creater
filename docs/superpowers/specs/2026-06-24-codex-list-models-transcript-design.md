# Codex List Models Transcript

## Problem

Palmier's assistant makes tool activity visible inside chat, including model listing before generation or edit work. Video Creater now shows `list_models` in the Codex context tools panel, but a generated edit request only records `get_timeline` and `project_context` in the transcript. The static row helps, but the conversation still hides the model-readiness check.

## Goals

- Record `list_models` as a loaded Codex tool call when the user sends an edit generation request.
- Keep the tool call compact and consistent with existing transcript rows.
- Avoid backend calls, credential handling, or changes to generation payloads.
- Preserve the existing latest-request de-duplication behavior.

## Non-goals

- No new model registry.
- No model picker in the Codex rail.
- No Temporal workflow change.
- No fal.ai network call.

## UX Requirements

- The transcript should show `list_models` before `get_timeline` and `project_context`.
- The message should summarize readiness without exposing provider credentials.
- Existing queued generation and variation tool calls should remain unchanged.

## Verification

- Extend AgentPanel transcript tests to require the loaded `list_models` tool call.
- Run focused AgentPanel tests, then full tests, lint, and diff checks.
- Smoke-check the Codex chat transcript in the browser.
