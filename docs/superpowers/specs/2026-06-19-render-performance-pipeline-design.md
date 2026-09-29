# Render Performance Pipeline Design

## Summary

Improve video generation performance in three staged backend slices:

1. Add render-stage observability to proposal render reports.
2. Cache graphics artifacts whose inputs have not changed.
3. Make `draftWebm` cheaper than `finalWebm` through explicit draft proxy settings.

The first slice is intentionally measurement-first. It gives every later optimization a report-backed way to prove whether it helped without weakening the EDL-first contract or Rust-owned validation.

## Goals

- Record render-stage timings and artifact metrics in JSON and Markdown render reports.
- Preserve the current EDL-first render flow: selected source clips are built before captions, overlays, GPU visuals, or final media validation.
- Cache CPU and GPU graphics artifacts by a deterministic input fingerprint.
- Reuse cached graphics only when manifest files and required frame artifacts are present and match the requested layer contract.
- Add explicit draft quality settings that can reduce render cost without changing the canonical timeline or proposal validation.
- Keep final renders on the current full-resolution WebM path unless a later codec/container spec changes it.

## Non-Goals

- Replacing GStreamer/GES.
- Removing the GStreamer runtime lock.
- Adding MP4/H.264 or AAC.
- Implementing a persistent Parakeet worker.
- Changing Codex proposal schemas.
- Changing canonical project files from the render proposal CLI.

## Current Context

The current render proposal flow in `src-tauri/src/render_pipeline/proposal.rs` performs these stages:

1. Read and validate the Codex proposal report.
2. Probe the source video with GStreamer.
3. Build a temporary `VideoProject` from probe metadata.
4. Validate the proposal and build a `RenderPlan`.
5. Render CPU and GPU graphics artifacts.
6. Render the selected EDL and graphics through `GstreamerGesRenderBackend`.
7. Probe and validate the final artifact.
8. Write JSON and Markdown render reports.

The current bottleneck candidates are visible in code:

- The proposal flow does not measure individual stages, so slow paths are not attributable.
- CPU and GPU graphics layers are regenerated on every render even when layer inputs are unchanged.
- GPU visual rendering writes every frame as PNG and then GES loads every PNG frame as an individual `UriClipAsset`.
- `RenderQualityProfile::DraftWebm` and `FinalWebm` are currently command metadata more than materially different render settings.

## Chosen Architecture

Keep the render pipeline shape and add performance metadata around it.

```text
Codex report
  -> source probe [timed]
  -> project + render plan [timed]
  -> graphics artifacts [timed, cache-aware]
  -> GStreamer/GES render [timed]
  -> final probe + validation [timed]
  -> render report with stage metrics
```

The first slice adds a small, backend-neutral report model:

```rust
pub struct RenderStageReport {
    pub name: String,
    pub status: String,
    pub duration_ms: u128,
    pub details: BTreeMap<String, String>,
}

pub struct RenderPerformanceSummary {
    pub total_duration_ms: u128,
    pub stages: Vec<RenderStageReport>,
}
```

`RenderReport` gains an optional `performance` field. Keeping it optional protects existing frontend and tests that do not need performance metadata yet.

## Slice 1: Render Observability

Add a small timing helper in the render pipeline layer. Each measured stage records:

- stable stage name
- status: `succeeded` or `failed`
- elapsed milliseconds
- compact string details such as clip count, graphics layer count, frame count, artifact bytes, width, height, fps, and output path

Required stages:

- `readReport`
- `sourceProbe`
- `buildRenderPlan`
- `graphics`
- `gesRender`
- `finalProbe`
- `writeReport`

When a stage returns an error, the error path must still produce the existing actionable error. The timing helper should not swallow or rewrite pipeline errors.

Reports should include stage data in both JSON and Markdown. Markdown should stay compact: one row per stage with name, status, duration, and key details.

## Slice 2: Graphics Artifact Cache

Add cache metadata for graphics layers without changing the graphics IR or proposal schema.

For each CPU graphics layer and GPU graphics layer, compute a deterministic fingerprint from:

- layer JSON
- dimensions
- fps
- duration
- renderer kind: `rust`, `gpu`, or `software`
- quality profile when present

Store a cache metadata file beside the graphics manifest:

```json
{
  "schemaVersion": 1,
  "fingerprint": "...",
  "renderer": "gpu",
  "qualityProfile": "hq-neon-wireframe-shader-v1"
}
```

Before rendering a layer, check:

- cache metadata exists and fingerprint matches
- manifest exists and deserializes
- preview path exists and is non-empty
- required frame paths exist and are non-empty
- manifest dimensions, fps, duration, alpha, and frame count match the requested layer

If all checks pass, reuse the artifact and record `cache=hit` in the graphics stage details and graphics report row. If any check fails, render normally and record `cache=miss`.

This cache must never reuse an artifact with mismatched timing, dimensions, fps, alpha, or frame count.

## Slice 3: Draft Proxy Render Settings

Make `draftWebm` cheaper while keeping the render plan and EDL validation unchanged.

Add backend-owned quality settings:

```rust
pub struct RenderQualitySettings {
    pub output_width: u32,
    pub output_height: u32,
    pub output_fps: f64,
    pub video_bitrate_kbps: Option<u32>,
    pub speed_hint: String,
}
```

Draft settings:

- cap width to 960 while preserving aspect ratio
- cap fps to 24 when source fps is higher
- keep audio and selected clip timing unchanged
- mark reports with `qualityProfile=draftWebm` and the effective output dimensions/fps

Final settings:

- keep current render plan width, height, and fps
- mark reports with `qualityProfile=finalWebm`

The first implementation can expose settings in report metadata and command summaries before deeper encoder property tuning. It must not silently change final output.

## Error Handling

- Existing `PipelineError` values remain the source of truth for failures.
- Timing and cache helpers return ordinary pipeline errors when metadata files are unreadable in ways that should block reuse.
- Cache misses caused by absent or stale artifacts are not errors; they trigger a normal render.
- Failed stage timings are recorded only if the caller has enough context to include them in a failed report. If the current CLI exits before writing a failed report, the primary requirement is that the original error remains intact.

## Testing Strategy

Use TDD for every behavior change.

Rust tests:

- `RenderReport` serializes and deserializes with `performance`.
- Markdown reports include a performance section when performance is present.
- Stage timing helper records a successful stage.
- Stage timing helper records a failed stage without changing returned errors.
- Graphics cache rejects missing manifests, missing frames, mismatched dimensions, mismatched fps, mismatched frame count, and stale fingerprints.
- Graphics cache accepts a complete matching artifact.
- Draft quality settings cap dimensions and fps while final settings preserve dimensions and fps.

Frontend tests:

- Existing render report parsing remains compatible when `performance` is omitted.
- Render report model can read performance data when Rust reports include it.

Commands:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml render_pipeline --test render_pipeline -- --test-threads=1
rtk pnpm test -- --run src/lib/render.test.ts
rtk pnpm lint
```

## Rollout

1. Add observability types and report serialization.
2. Instrument proposal render stages.
3. Add graphics cache metadata and validation.
4. Reuse cached graphics artifacts in proposal rendering.
5. Add draft quality settings and expose effective settings in reports.
6. Update frontend render types for optional performance metadata.
7. Run targeted tests and then the project verification script.

## Acceptance Criteria

- Render reports can show per-stage timing and key render details.
- Existing render reports without performance metadata still deserialize and render in the UI.
- Graphics artifacts are reused only when their fingerprint and manifest contract match.
- Cache hits and misses are visible in render report metadata.
- Draft render settings are explicitly different from final settings in backend metadata.
- Final render settings preserve the existing dimensions and fps.
- All changes keep Rust validation and EDL-first rendering intact.

## Spec Self-Review

- No placeholders or deferred undefined behavior remain.
- The design is split into three sequential implementation slices, each testable on its own.
- Cache reuse is conservative and cannot bypass manifest/timing validation.
- Draft quality changes are scoped to effective render settings and report metadata first, avoiding a risky codec-policy change.
