# GStreamer GES Render Backend Design

## Summary

Migrate Video Creater's render backend from legacy CLI compositor/probe execution to a Rust-owned GStreamer backend built on GStreamer Editing Services (GES). The migration keeps the existing EDL-first model, Rust graphics artifact generation, render reports, structured errors, and media validation contract while replacing media assembly, encoding, muxing, and probing with GStreamer/GES APIs.

The backend must enforce an explicit LGPL-compatible plugin policy. It must not silently use arbitrary installed GStreamer plugins.

## Goals

- Add a `GstreamerGesRenderBackend` behind the existing render backend boundary.
- Use GES timelines for selected source ranges instead of generating legacy trim/concat filters.
- Replace legacy probe validation with GStreamer-backed discovery/probing.
- Keep Rust-owned render jobs, logs, cancellation, artifact paths, duration checks, stream checks, and reports.
- Feed Rust-generated graphics artifacts into the GStreamer composition path.
- Deny non-approved plugins and fail before rendering when the resolved pipeline violates the licensing policy.
- Remove legacy command-line render tools from the accepted-proposal and combined validation render paths once the GStreamer path reaches parity for EDL rendering, static graphics overlays, media discovery, and e2e coverage.

## Non-Goals

- Building a pure-Rust codec stack.
- Letting GStreamer own canonical project state.
- Allowing arbitrary system plugins based on local availability.
- Replacing the Rust graphics IR, rasterizer, manifests, or proposal validation.
- Solving every codec/container combination in the first slice.
- Shipping GPL or nonfree codec plugins as implicit dependencies.

## Current State

The current implementation already has useful boundaries:

- `src-tauri/src/render_pipeline/backend.rs` defines `RenderBackend`.
- `src-tauri/src/edit/render_plan.rs` defines the backend-neutral `RenderPlan` selected EDL shape.
- `src-tauri/src/render_pipeline/probe.rs` validates backend-neutral media expectations.
- `src-tauri/src/render_pipeline/gstreamer_backend.rs` renders selected EDL clips through GES and maps GStreamer Discoverer output into `MediaProbe`.
- Proposal and combined e2e flows render through `GstreamerGesRenderBackend` and validate output with GStreamer Discoverer.
- Rust graphics already produces backend-neutral PNG/RGBA frame sequences plus manifests.

GES is the default Cargo feature for the Tauri crate. Builds without `ges-render` still compile and return actionable backend-unavailable errors from render/probe entry points.

## Chosen Architecture

```text
VideoProject / Codex proposal
  -> validated EDL / RenderPlan
  -> Rust graphics manifests
  -> GstreamerGesRenderBackend
  -> GES timeline with source clips and overlay layers
  -> GStreamer encoder/muxer pipeline
  -> GStreamer probe/discoverer validation
  -> JSON/Markdown render report
```

Rust remains the authority for:

- canonical project state
- EDL construction
- proposal validation
- render job construction
- graphics IR and frame generation
- plugin policy checks
- report writing and actionable errors

GStreamer/GES owns only media graph execution for a single render job.

## Backend Interface

The existing command-oriented `RenderBackend` trait should evolve so in-process backends do not have to pretend they build shell commands.

Target shape:

```rust
pub trait RenderBackend {
    fn plan_summary(
        &self,
        plan: &RenderPlan,
        graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
    ) -> PipelineResult<RenderBackendPlan>;

    fn render(
        &self,
        plan: &RenderPlan,
        graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
        timeout: Duration,
    ) -> PipelineResult<RenderBackendOutput>;
}
```

The active backend returns GStreamer/GES command metadata for reports while executing media work in process through GStreamer/GES APIs.

## Dependency And Feature Gate

GES bindings link to the system GES library, so missing `gstreamer-editing-services-1.0` is primarily a build-time concern.

- `ges-render` is enabled by default.
- `gstreamer`, `gstreamer-pbutils`, and `gstreamer-editing-services` Rust dependencies are behind that feature.
- Policy, report, backend-selection, and fallback code compile without the feature.
- Feature-disabled render/probe entry points return a diagnostic error that tells users to rebuild with `ges-render`.

This keeps default local verification on the real GES path while preserving a smaller no-default build for dependency diagnostics.

## GES Timeline Mapping

For the first renderable milestone:

- Each `RenderClip` becomes a GES source clip.
- `source_in` and `source_out` define the source in-point and duration.
- Timeline position is the accumulated output cursor.
- Project render settings define output width, height, fps, and target container.
- Invalid clips fail before GES construction, using the existing render-plan error codes.

The backend must prove that the primary video duration is the selected EDL duration, not full-source pass-through.

## Graphics And Overlays

Rust graphics remains backend-neutral. It continues producing:

- transparent frame sequences
- dimensions, fps, alpha, duration, frame count, and timing metadata
- preview and manifest artifacts

The GStreamer backend consumes these artifacts through one of two implementation paths:

1. Preferred path: image sequence or appsrc layer with alpha into a compositor layer.
2. Fallback path: pre-pack graphics frames into an intermediate GStreamer-readable alpha stream using approved plugins only.

Overlay timing must preserve existing semantics:

- timeline start is explicit
- overlay end is exclusive
- dimensions and fps must match the render plan or fail before render
- alpha is required for overlay graphics

## Probe And Validation

Use a GStreamer probe module that returns the existing `MediaProbe` shape:

- duration
- size
- first video stream codec, width, height, fps
- first audio stream codec

Validation stays in Rust. `validate_rendered_media` should remain backend-neutral and continue checking:

- video stream presence
- audio stream presence
- non-empty artifact
- duration with tolerance
- dimensions

## Plugin Licensing Policy

The backend must use a default-deny plugin policy. A render job must fail before execution if any resolved element factory is denied or unreviewed.

Denied by default:

- `x264enc`
- `avenc_*`
- `avdec_*`
- `libav`
- `fdkaac`
- any element from known GPL, nonfree, or proprietary plugin sets
- any factory without enough metadata to evaluate package/license

Allowed by default:

- GStreamer Core elements required for scheduling and caps negotiation.
- GStreamer Base elements required for app integration and simple transforms.
- Reviewed GStreamer Good elements, including `mp4mux` where used.
- Project-reviewed LGPL-compatible elements recorded in a static allowlist.

Review-required:

- GStreamer Bad elements.
- Codec elements with patent, binary distribution, or third-party-library implications, even when the plugin wrapper is LGPL.
- Platform hardware encoders.

Every render report must include the selected factories and plugin policy verdict.

## Codec And Container Strategy

The first target is still a user-playable video artifact, but codec choice must respect the plugin policy.

Preferred strategy:

- Use MP4 only when the selected H.264/AAC or platform encoder path passes policy review.
- Use WebM with reviewed LGPL-compatible VPx/Opus elements as the safe fallback format if MP4 encoding cannot pass policy.
- Keep output format explicit in render settings or backend config; do not silently switch containers without report metadata.

This means the first implementation may produce a GStreamer-rendered fixture in a policy-safe format before MP4 parity is complete.

## Error Handling

Use the existing actionable error shape:

```json
{
  "code": "RENDER_BACKEND_UNAVAILABLE",
  "path": "gstreamer.ges",
  "message": "GStreamer Editing Services is not available.",
  "fix": "Install GStreamer Editing Services and ensure pkg-config can find gstreamer-editing-services-1.0."
}
```

New failure paths should cover:

- missing GStreamer library
- GES backend feature disabled
- missing GES library during GES-feature builds
- plugin policy denial
- unsupported container/codec for current policy
- graph construction failure
- render timeout
- bus error
- malformed or missing output artifact
- probe/discovery failure

## Rollout

1. Add dependency feature gate and environment detection tests for GStreamer/GES availability.
2. Add plugin policy data structures and tests for allow, deny, and unknown factories.
3. Refactor the backend trait so command and in-process backends can coexist.
4. Add `GstreamerGesRenderBackend` skeleton that validates plan inputs and reports missing dependencies cleanly.
5. Implement GStreamer-backed probe/discovery into the existing `MediaProbe` contract.
6. Implement EDL-only render through GES for tiny fixture media.
7. Add graphics overlay composition through approved GStreamer elements.
8. Update proposal and combined e2e paths to use GStreamer/GES.
9. Remove CLI compositor flags, parser paths, and the old process-backed render dependency after parity tests pass.

## Testing

Focused tests:

- plugin policy rejects denied factories
- plugin policy rejects unknown factories by default
- plugin policy allows only explicitly reviewed factories
- disabled GES feature returns actionable `RENDER_BACKEND_UNAVAILABLE`
- missing GES system dependency is documented for `--features ges-render` builds
- invalid EDL clips fail before GStreamer graph construction
- backend summary records selected factories and policy verdicts
- GStreamer probe maps discovery output into `MediaProbe`
- EDL-only fixture render validates duration, stream presence, dimensions, and non-empty artifact
- overlay fixture render validates timing and dimensions

Full verification:

- targeted Rust tests during each TDD step
- combined media e2e once local GES and required reviewed plugins are installed
- `rtk pnpm verify` after the migration slice is green

## Acceptance Criteria

- A GStreamer/GES backend exists behind the Rust render backend boundary.
- The backend refuses to render with denied or unknown plugins.
- Render reports include plugin policy evidence.
- GStreamer probing can validate rendered media without external probe tools.
- The first GES render path builds output from selected EDL clips.
- The backend fails clearly when the GES feature, GES system dependency, or required reviewed plugins are unavailable.
- Live Rust source and tests no longer depend on the old command-line compositor/probe APIs.
