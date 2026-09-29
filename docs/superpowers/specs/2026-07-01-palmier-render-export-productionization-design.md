# Palmier Render Export Productionization Design

## Context

Palmier advertises MP4 H.264, H.265, ProRes, and NLE XML export. Video Creater has a real GStreamer/GES WebM render path, NLE XML export, render reports, policy-gated MP4/MOV profile descriptors, and Temporal workflow records. The remaining gap is making export feel production-ready instead of partially policy-gated or workflow-shaped.

The app must keep its stricter EDL-first contract: selected `sourceIn`/`sourceOut` ranges are the primary render input, visual layers come after the cut, and render completion is not accepted until media streams and artifacts are validated.

## Goal

Productionize render/export so users and agents can reliably render draft WebM, final MP4 H.264, MP4 H.265, ProRes MOV, Premiere XML, and DaVinci FCPXML with durable reports and actionable failure states.

## Requirements

- Render plans support multiple source media assets on the primary timeline, not only one source media id.
- Render plans preserve source ranges, timeline ordering, gaps, disabled tracks, captions, text overlays, templates, and generated outputs where supported.
- MP4 H.264, MP4 H.265, and ProRes MOV profiles use approved local encoder runtime detection instead of a generic env gate.
- Render reports recompute checks from evidence: duration, video stream, audio stream when expected, artifact paths, log path, caption timing, and overlay timing.
- UI export status shows queued, running, failed, completed, retry, output path, report path, and log path.
- Agent export tools use the same export profile availability and render path as the UI.
- NLE XML remains available for supported timeline items and returns actionable errors for unsupported sources.

## Non-Goals

- No cloud render farm.
- No arbitrary codec/plugin loading outside the approved runtime policy.
- No masking/transition/effects parity beyond what the render pipeline can validate in this slice.
- No weakening of render-review validation to match a faster UX.

## Architecture

Extend `render_pipeline::project_export` into a profile-agnostic project renderer:

- `project_render_plan.rs`: converts canonical timeline into source-backed render segments and visual layer manifests.
- `encoding_profiles.rs`: maps `draftWebm`, `finalWebm`, `mp4H264`, `mp4H265`, and `proResMov` to container, encoder, audio codec, extension, and runtime requirements.
- `project_render.rs`: executes render, probes output, writes logs/reports, and attaches project actions.
- `render_evidence.rs`: recomputes render-review checks from probe output and timeline/layer metadata.

The existing GStreamer/GES backend remains the first backend. Profile support expands through explicit encoder profiles, not through arbitrary command strings.

## Data Flow

1. UI or agent calls export with profile and destination intent.
2. Rust loads the split project and validates profile availability.
3. Rust builds a canonical render plan from all enabled source-backed timeline items.
4. Rust expands captions/templates/overlays into renderable visual layers.
5. Backend renders to a temp output path.
6. Probe validates streams, duration, and container.
7. Render evidence recomputes checks and writes JSON/Markdown/log artifacts.
8. Rust atomically attaches job status, render report, and export artifact.
9. UI refreshes project state and shows output/report/log links.

## Error Handling

- Missing encoder: fail before rendering with a profile-specific install/configuration message.
- Unsupported timeline item: fail before rendering and name the item id, kind, and supported alternatives.
- Render backend failure: preserve log path and mark job failed without attaching a completed report.
- Probe mismatch: mark failed with expected and observed duration/stream details.
- Partial artifact write: do not mark job completed.

## Tests

- Rust tests for multi-source render-plan construction.
- Rust tests for disabled tracks, gaps, generated outputs, captions, overlays, and template timing.
- Rust tests for encoder profile availability without env-only gating.
- Rust tests for evidence-derived render checks.
- Rust integration tests with tiny fixture media for draft WebM and one approved MP4 profile when runtime exists.
- Frontend tests for export status, retry, report/log links, and disabled profile reasons.
- MCP/app-server tests for `export_project` request and failed profile payloads.

## Success Criteria

Export buttons and agent export tools produce durable render artifacts with real evidence-backed reports. MP4/MOV profiles are either available through an approved runtime or disabled with precise recovery copy, and multi-source timelines are no longer rejected solely because clips come from different media assets.

## Self-Review

- Scope is render/export productionization, not semantic search or product onboarding.
- The design preserves the EDL-first render-review contract.
- All required outputs, checks, and failure states are explicit.
