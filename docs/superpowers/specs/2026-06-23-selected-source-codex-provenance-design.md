# Selected Source Codex Provenance Design

## Context

Palmier treats generated media as inspectable editing context: users can see the prompt, model, first frame, last frame, references, and then ask the assistant to iterate without leaving the editor. Video Creater already exposes selected generated source details in the media rail, and Codex now targets the selected media id. The next gap is that the Codex chat surface still shows only `@media-id`, so the assistant context does not visibly carry the selected source's file and generation provenance.

## Goal

Show compact selected-source metadata inside Codex so the user can confirm what the agent is about to edit. For generated outputs, Codex should display the selected output file, source kind, generating model, prompt, and reference ids. For imported media, Codex should display the file label and kind.

## Non-Goals

- No change to the `EditJobRequest` payload.
- No new backend chat protocol or multi-turn transcript state.
- No parsed mention resolver.
- No duplicated full source inspector; the Codex version stays compact and action-focused.

## UI And Data Flow

`EditorWorkspace` derives a small `AgentSelectedMediaContext` from the selected media id, `project.media`, and `project.generatedAssets`.

The context includes:

- `mediaId`
- `label`, from the media or generated output relative path filename
- `kind`, from the selected media asset
- optional `modelLabel`, from the generated asset provider/id
- optional `prompt`, from the generated asset prompt
- optional `referenceIds`, combining first frame, last frame, and reference media ids without duplicates

`AgentPanel` receives this context and renders it in the existing `Project context` area. The display remains dense:

- top row keeps `@media-id` and preset/language,
- source row shows `Selected source`, file label, and kind,
- generated row shows model and reference ids when available,
- prompt preview is truncated but present when available.

## Testing

- `AgentPanel` renders selected source label and kind when context is provided.
- `AgentPanel` renders generated prompt, model, and references when provided.
- `EditorWorkspace` derives generated output context from the default sample project after selecting `sample-generated-output`.

## Future Work

- Feed this compact context into a richer multi-turn prompt once Codex supports chat turns.
- Add inline copy/open-source actions for references.
- Let the Codex composer insert `@` mentions from selected source context.
