# Palmier Semantic Media Search Design

## Context

Palmier's `search_media` lets agents and users find visual and spoken moments by natural language. The local repo includes SigLIP/CLIP-style visual indexing and transcript-backed spoken search. This matters because agentic editing needs retrieval: agents must find a shot, quote, visual detail, or generated asset without manually scanning every clip.

Video Creater currently has media metadata, folders, transcripts, generated asset prompts, and a `search_media` tool surface. The missing piece is durable per-project indexing and ranked search over visual frames and spoken transcript segments.

## Goal

Build a local-first semantic media search layer that supports transcript search immediately and adds visual frame embedding search behind a model/runtime readiness gate.

## Requirements

- Search covers imported media, generated outputs, transcript words/segments, generated prompts, and timeline labels.
- Search returns media ids, optional timeline item ids, source time ranges, scores, reasons, and index readiness status.
- Spoken search works without visual model installation when transcripts exist.
- Visual search is local-first and optional; when no model is installed, search returns a clear `visualStatus` without pretending no results exist.
- Index state is stored under the split project and can be rebuilt.
- Agents can call the same search through MCP/app-server tools and the UI media library can reuse the query API.

## Non-Goals

- No cloud embedding provider in this slice.
- No automatic download of large visual models without explicit user action.
- No frame-perfect shot detection requirement in the first slice; deterministic sampled frames are acceptable.
- No face/person identification or biometric labeling.

## Architecture

Add a search module with three layers:

- `src-tauri/src/search/transcript.rs`: exact and fuzzy spoken search over transcript segments and words.
- `src-tauri/src/search/visual_index.rs`: per-asset sampled frame manifest, embedding metadata, and readiness state.
- `src-tauri/src/search/query.rs`: combines spoken, visual, generated-prompt, filename, folder, and timeline-label matches into a bounded response.

Persist search sidecars under:

```text
search/
  index.json
  visual/<media-id>/frames.json
  visual/<media-id>/embeddings.bin
  spoken/<media-id>.json
```

The first implementation can ship transcript/metadata/generated-prompt search and visual index readiness plumbing. The embedding backend can land behind the same interfaces when the runtime is available.

## Data Flow

1. Media import or transcript completion records index candidates.
2. Search index rebuild scans media, transcripts, generated assets, and timeline labels.
3. Spoken search tokenizes query text and transcript segments.
4. Visual search checks model readiness and visual sidecars.
5. Combined search returns separate groups: `spoken`, `visual`, `metadata`, and `generated`.
6. Agent tools and UI display readiness so incomplete indexes do not look like negative evidence.

## Search Result Shape

```json
{
  "query": "founder says launch day",
  "visualStatus": "ready | notInstalled | indexing | unavailable | failed",
  "spokenStatus": "ready | noTranscripts | indexing",
  "groups": {
    "spoken": [
      {
        "mediaId": "media-1",
        "timelineItemId": "clip-1",
        "startSeconds": 12.4,
        "endSeconds": 16.2,
        "score": 0.82,
        "reason": "matched transcript segment"
      }
    ],
    "visual": [],
    "metadata": [],
    "generated": []
  }
}
```

## Error Handling

- Missing index: return `indexing` or `notInstalled`, not an empty false negative.
- Missing transcript: return `noTranscripts` for spoken search and suggest transcription.
- Corrupt sidecar: mark the asset index failed and continue searching other assets.
- Query too broad or empty: reject with a short actionable error.

## Tests

- Rust transcript search tests for exact phrase, token overlap, word ranges, and bounded result limits.
- Rust combined search tests for metadata, generated prompt, timeline labels, and transcript groups.
- Rust split-project tests for search sidecar drift and rebuild behavior.
- MCP/app-server tests for `video_creater.search_media` status payloads.
- Frontend tests for media library search results and index-readiness copy.

## Success Criteria

An agent can ask for a quote, generated prompt, filename, folder, or visual description and get ranked media/time-range candidates with honest readiness status. Spoken search is useful before visual embeddings ship, and the visual index interfaces are ready for a local embedding backend.

## Self-Review

- Scope is search/indexing, not direct editing or rendering.
- The first useful slice does not depend on a large model download.
- Result shape is concrete and status-aware.
