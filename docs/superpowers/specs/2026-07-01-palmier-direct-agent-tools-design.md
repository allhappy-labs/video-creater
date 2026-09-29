# Palmier Direct Agent Tools Design

## Context

Palmier exposes a direct shared tool catalog to in-app chat and MCP clients. Agents can inspect timeline/media, generate assets, mutate clips, remove words, set keyframes, apply looks, search media, and export without relying on freeform file edits.

Video Creater already has a Rust-owned Codex local tool dispatcher and MCP-compatible server work. The current tool surface is validation-heavy and safe, but still leans toward project-action batches and start-request builders. Palmier parity needs a clearer set of user-intent tools that map directly to editor operations while preserving Rust validation.

## Goal

Add direct agent tools that let external MCP clients and the in-app Codex surface perform common editing, generation, transcript, inspection, and export actions through typed Rust validation.

## Requirements

- Keep Rust as the only canonical mutation boundary.
- Keep `ProjectAction` as the internal mutation representation for timeline/project edits.
- Add direct tools that convert ergonomic agent arguments into validated project actions or workflow start requests.
- Expose the same tool names and schemas through the Tauri wrapper and MCP stdio server.
- Return structured results with changed ids, warnings, and next recommended inspection calls.
- Never expose provider secret values.

## Tool Set

### Query Tools

- `video_creater.get_timeline`: existing bounded timeline payload with ids, tracks, items, source ranges, and selected warnings.
- `video_creater.inspect_timeline`: upgrade from metadata-only to preview-aware once timeline preview compositor support exists; return active layers and optionally rendered frame artifacts later.
- `video_creater.get_media`: media library and folder payload.
- `video_creater.get_transcript`: timeline-mapped transcript words in output timeline seconds, derived from source transcript words and visible source ranges.
- `video_creater.search_media`: metadata/transcript search first; semantic visual search lands in the semantic-search spec.

### Mutation Tools

- `video_creater.add_clips`: validate media-backed timeline insertions and create project actions.
- `video_creater.insert_clips`: ripple insert into a target track.
- `video_creater.remove_clips`: remove items by id.
- `video_creater.move_clips`: move items across tracks and time.
- `video_creater.split_clips`: split items at timeline seconds.
- `video_creater.ripple_delete_ranges`: remove time spans and close gaps on compatible tracks.
- `video_creater.remove_words`: map transcript word indexes to ripple-delete ranges.
- `video_creater.set_clip_properties`: update source range, opacity, audio volume/fade, text content, and template timing.
- `video_creater.set_keyframes`: add a small keyframe schema for opacity and audio volume first.
- `video_creater.undo_agent_edit`: undo only the latest agent-authored action batch when project history support is available.

### Generation And Export Tools

- `video_creater.list_models`: return configured generation models and local transcription models.
- `video_creater.generate_video`, `generate_image`, `generate_audio`: record generated asset, record job, build start request, and optionally insert placeholder timeline items.
- `video_creater.upscale_media`: record an upscale generated asset and workflow request when a provider supports it.
- `video_creater.export_project`: start or build export requests for WebM, MP4 profiles, Premiere XML, and DaVinci FCPXML.

## Non-Goals

- No direct provider execution inside the MCP server process.
- No arbitrary shell/file tools exposed to agents.
- No full color/effects stack in the first tool slice; that can be a later parity spec once clip transform/keyframe foundations are stable.
- No mutation without project-action validation.

## Architecture

Add a `codex::direct_tools` layer above `ProjectAction`. Each direct tool:

1. Decodes ergonomic JSON arguments.
2. Loads current split-project state.
3. Resolves ids and timeline/media context.
4. Builds one or more `ProjectAction` values or workflow start requests.
5. Validates against a cloned project.
6. Applies the actions only through existing split-project writers when the tool is mutating.
7. Returns structured results with affected ids and warnings.

The MCP server remains transport-only. It calls the same dispatcher as Tauri commands, so app-server and external-agent behavior stay consistent.

## Safety

- Direct tools reject unknown fields.
- Direct tools reject mixed units unless the schema explicitly allows them.
- `remove_words` requires transcript availability and returns an actionable error when transcript words are stale or missing.
- Generation tools return start requests and job ids without provider credentials.
- Export tools reject unavailable profiles with the same policy report shown in the UI.

## Tests

- Rust tests for every new tool schema and unknown-field rejection.
- Rust tests for `remove_words` mapping transcript indexes to source-backed ripple ranges.
- Rust tests for `generate_video/image/audio` recording generated assets, jobs, references, first/last frames, and placement intent.
- Rust MCP tests for tool listing and `tools/call` structured content.
- Frontend tests for agent-panel copy and visible tool activity for direct generation/edit/export calls.

## Success Criteria

An external agent can use Video Creater like a Palmier-style editor: inspect, generate, edit, remove words, and export using named tools, while every canonical state change still passes through Rust project-action validation.

## Self-Review

- Scope is direct agent tools, not visual search implementation or final render productionization.
- The tool list is concrete and maps to existing project concepts.
- Safety keeps provider execution and arbitrary file mutation out of the MCP boundary.
