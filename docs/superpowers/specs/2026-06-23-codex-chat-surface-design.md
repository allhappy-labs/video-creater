# Codex Chat Surface Design

## Context

Palmier exposes an AI assistant as a contextual chat: users describe edits in natural language, reference media with `@`, generate assets, place them on the timeline, and iterate prompts or timing without leaving the project. Video Creater already has a working Codex panel with presets, language hints, structured edit requests, proposal review, template suggestions, and validated project actions. The missing UI layer is a chat-like surface that makes the agent feel like a timeline collaborator rather than a static form.

## Goals

- Make the Codex panel read like a contextual editor chat while preserving existing preset and language controls.
- Show the active media reference using `@media-id` so the user can see what the request targets.
- Render a compact transcript from current state:
  - a system/context message for project-aware editing,
  - the user's latest prompt when a request has been sent,
  - a Codex status/proposal summary when generation is running, ready, applying, blocked, or errored.
- Keep the same `EditJobRequest` output shape for the first slice; no backend protocol changes.
- Keep the panel dense enough to remain usable below the project timeline inspector.

## Non-Goals

- No free-form multi-turn backend chat protocol in this slice.
- No new MCP commands or project actions.
- No parsed `@` mention resolver beyond showing the current target media id.
- No timeline mutation outside the existing apply-proposal and template-suggestion actions.

## UI

The Codex card should show:

- Title: `Codex`, with the existing sparkles icon.
- Intro copy rewritten around contextual chat, not rough-cut form submission.
- A `Project context` strip that includes `@media-id` and the current preset/language mode.
- A `Chat` region with compact message bubbles:
  - Context message: explains that Codex sees project media, timeline, templates, and render settings.
  - User message: appears after `latestRequest`, showing the saved prompt and target media reference.
  - Assistant message: reflects current state:
    - idle: ready to generate a timeline edit,
    - running: generating a timeline edit,
    - ready: proposal ready with clip/action counts,
    - applying: applying validated project actions,
    - error: failed with the error message.
- The existing preset and language controls remain available.
- The prompt textarea becomes the chat composer, with helper text mentioning `@media-id`.
- The primary button remains `Generate edit` for compatibility.

## Data Flow

`AgentPanel` continues to own only local UI state for selected preset, language, and prompt. It receives `mediaId`, `latestRequest`, `codexStatus`, `codexError`, proposal counts, and proposal clips from `EditorWorkspace`. The chat transcript is derived from those props and local controls; generating still calls `onGenerateEdit(buildEditJobRequest(...))`.

## Testing

- AgentPanel tests verify the context strip exposes `@media-id`.
- AgentPanel tests verify the transcript starts with a project-aware context message.
- AgentPanel tests verify a sent request appears as a user chat message.
- Existing blocked-model, runtime, and existing-transcript tests continue to prove request gating.

## Future Work

- Add real multi-turn conversation state once the backend exposes chat turns separate from one-click edit generation.
- Parse `@` mentions against project media and generated assets.
- Let chat commands enqueue media generation, insert generated outputs, and apply clip timing changes directly through validated `ProjectAction` values.
