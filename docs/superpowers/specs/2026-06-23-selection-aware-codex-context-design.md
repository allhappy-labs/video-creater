# Selection-Aware Codex Context Design

## Context

Palmier keeps the assistant, media library, source viewer, and timeline tied to the current editing context. Its docs describe using `@` to reference media in chat, and its screenshots show chat prompts aimed at a selected asset while the media grid and timeline remain visible. Video Creater now has a Codex chat surface, but `EditorWorkspace` still lets `AgentPanel` fall back to its default `media-1` target even after the user selects another media item or generated output.

## Goal

Make Codex target the same media that the editor has selected. When the user selects imported media or a generated output in the media bin, the Codex project context strip, prompt helper, and generated `EditJobRequest` should use that selected media id.

## Non-Goals

- No parsed multi-mention composer in this slice.
- No backend chat protocol change.
- No change to media selection behavior in `MediaBin`.
- No automatic timeline selection change when media selection changes.

## UI And Data Flow

`EditorWorkspace` remains the owner of `selectedMediaId`. It already updates that state when the user selects media, reveals a source clip, imports media, inserts a generated output, or replaces a selected clip with generated output. The next step is to pass `selectedMediaId ?? "media-1"` into `AgentPanel`.

`AgentPanel` already derives its visible chat context and request payload from its `mediaId` prop. Passing the workspace selection into that prop keeps the visible `@media-id`, helper text, and `buildEditJobRequest` target aligned.

## Testing

- `EditorWorkspace` should render `@media-1` in the Codex context by default.
- After selecting a generated output, `EditorWorkspace` should render `@sample-generated-output` in the Codex context.
- Generating an edit after that selection should send `start_codex_video_edit_for_project` a request whose `mediaId` is `sample-generated-output`.

## Future Work

- Add a real mention resolver for multiple referenced media ids.
- Let the Codex composer expose selected generated asset prompt/provenance inline.
- Add an agent action for placing or replacing selected media directly from chat.
