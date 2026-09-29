# GStreamer Native MP4 And ProRes Export Design

## Goal

Add a long-term, license-safe native GStreamer/GES export architecture for MP4 H.264, MP4 H.265, and ProRes MOV while preserving the existing WebM render path as the portable fallback.

The implementation should render the project timeline directly through GStreamer/GES for approved profiles instead of rendering final WebM first and passing it to an external encoder command.

## Current State

The render backend already uses GStreamer/GES, but the project render path is WebM-specific:

- `RenderQualityProfile` only supports `draftWebm` and `finalWebm`.
- `ProjectWebmRenderPaths` always writes `renders/<jobId>/output.webm`.
- `GstreamerGesRenderBackend` rejects non-`.webm` output.
- MP4 H.264, MP4 H.265, and ProRes MOV exist as export profile metadata, but availability is based on approved external encoder commands.
- Temporal export-media currently models codec export as final WebM render plus external encoder artifact writing.

## Reviewed Factory Set

The initial native codec implementation targets macOS because the reviewed encoder set uses Apple system encoders through GStreamer wrappers.

### WebM

WebM remains unchanged:

- Container: `webmmux`
- Video: `vp8enc` for draft, `vp9enc` for final
- Audio: `opusenc`

### MP4 H.264

- Container: `mp4mux`
- Video: `vtenc_h264`
- Audio: `atenc`
- Parser: `aacparse`
- Audio format: MPEG-4 AAC LC, raw stream format

### MP4 H.265

- Container: `mp4mux`
- Video: `vtenc_h265`
- Audio: `atenc`
- Parser: `aacparse`
- Audio format: MPEG-4 AAC LC, raw stream format

### ProRes MOV

- Container: `qtmux`
- Video: `vtenc_prores`
- Audio: raw PCM through approved audio conversion/resampling

## Denied Factories

The policy must continue to deny known unsafe or unreviewed alternatives even when installed locally:

- `x264enc`
- all `avenc_*`
- plugin `libav`
- `fdkaac*`
- `faac`
- `voaacenc`

`atenc` is the only approved AAC encoder in the initial implementation. It must be accepted only when the factory metadata identifies the `osxaudio` plugin, GStreamer Good package family, and an LGPL-compatible license.

`vtenc_h264`, `vtenc_h265`, and `vtenc_prores` must be accepted only on macOS when factory metadata identifies the `applemedia` plugin, the expected GStreamer package family, and an LGPL-compatible license.

`mp4mux`, `qtmux`, and `aacparse` must be accepted only when their metadata identifies the expected GStreamer Good plugin provenance and an LGPL-compatible license.

## Architecture

Replace WebM-specific naming and behavior with a profile-driven render/export model.

Add a profile concept that covers:

- `draftWebm`
- `finalWebm`
- `mp4H264`
- `mp4H265`
- `proResMov`

Each profile defines:

- output extension
- MIME type
- container caps
- video caps
- audio caps or raw audio mode
- required GStreamer factories
- encoder properties
- validation expectations

The renderer continues to build one EDL-backed `RenderPlan` from split-project source ranges. The backend then selects the correct GStreamer encoding profile and required factory set from the requested output profile.

The current WebM render functions and structs should be renamed or wrapped with profile-neutral names, for example:

- `ProjectWebmRenderPaths` to `ProjectMediaRenderPaths`
- `ProjectWebmRenderResult` to `ProjectMediaRenderResult`
- `render_webm_to_split_project_folder` to `render_media_to_split_project_folder`
- `build_project_webm_render_plan` to a profile-neutral render-plan builder

Backward-compatible wrappers may remain where the frontend or tests still need them during migration.

## Data Flow

1. The UI asks Rust for export profile availability.
2. Rust inspects the required GStreamer factories for each profile and evaluates them through the plugin policy.
3. The UI disables unavailable profiles and shows the missing or denied factory reason.
4. The UI calls `render_media_to_split_project_folder` with `projectDir`, `profile`, `jobId`, and `updatedAt`.
5. Rust loads the split project and builds the EDL-backed render plan from selected `sourceIn` and `sourceOut` ranges.
6. Rust writes output under:
   - `renders/<jobId>/output.webm`
   - `renders/<jobId>/output.mp4`
   - `renders/<jobId>/output.mov`
7. GStreamer/GES renders the timeline directly into the requested container.
8. Rust probes the output and validates duration, stream presence, codec/container expectations where available, report paths, and logs.
9. Rust attaches a durable render report and export artifact metadata to the split project.
10. Temporal export-media uses the same implementation for native GStreamer profiles instead of invoking an external encoder command.

## Error Handling

The system fails before mutation when:

- the split project cannot be loaded
- no enabled source-backed video clip exists
- a clip has invalid `sourceIn` or `sourceOut`
- a required GStreamer factory is missing
- a factory fails plugin policy
- GStreamer cannot configure the selected encoding profile
- rendering fails or times out
- probing fails
- validation fails
- report or artifact writes fail

Failed availability checks should name the profile, factory, policy verdict, plugin name, package, and license when available.

The renderer should mark a job completed only after rendering, probing, validation, report writing, and project action application all succeed.

## Validation

Rendered output validation must check:

- nonzero output file
- video stream presence
- audio stream presence when the timeline has audio
- expected duration within tolerance
- expected dimensions when discoverer reports them
- expected output extension
- expected render artifact and log paths

Codec validation should be best-effort because GStreamer discoverer caps can vary by platform. The report should include the discovered video and audio codec caps for review even when strict codec matching is not possible.

## UI Behavior

The export menu keeps the existing profile choices:

- Draft WebM
- Final WebM
- MP4 / H.264
- MP4 / H.265
- ProRes MOV
- Palmier Project
- NLE XML exports

MP4 and ProRes buttons become enabled only when the native GStreamer factory set is present and policy-approved.

Disabled profile copy should be actionable, for example:

- `Missing GStreamer factory: atenc`
- `Denied GStreamer factory: avenc_aac from libav`
- `Unsupported on this platform: vtenc_h264 requires macOS`

## Temporal Behavior

Temporal export-media should call the same profile-neutral Rust implementation for `mp4H264`, `mp4H265`, and `proResMov`.

The old external approved encoder command path should not be the default for these profiles once native GStreamer support exists. If retained, it must be an explicitly named fallback path and remain policy-gated.

## Testing

Use test-first implementation.

Rust profile tests:

- each profile maps to the correct extension, MIME type, container caps, codecs, and required factories
- MP4 profiles require `atenc` and `aacparse`
- ProRes requires `qtmux` and `vtenc_prores`

Rust policy tests:

- allow reviewed `mp4mux`, `qtmux`, `aacparse`, `atenc`, `vtenc_h264`, `vtenc_h265`, and `vtenc_prores`
- deny `x264enc`, `avenc_aac`, `avenc_aac_at`, `fdkaacenc`, `faac`, and `voaacenc`
- reject same-name factories with unexpected plugin, package, or license metadata

Rust backend tests:

- command specs include the selected output profile
- non-WebM output is accepted only for approved profiles
- WebM behavior remains unchanged

Project export tests:

- output paths become `output.mp4` and `output.mov`
- render reports and export artifacts are attached for MP4 and ProRes
- failure paths do not mutate project state before validation succeeds

Frontend tests:

- availability report enables or disables profiles based on GStreamer policy output
- export buttons call the native profile-neutral render command
- disabled profile copy includes policy or missing-factory reasons

Temporal tests:

- export-media profiles render directly through the shared project export implementation
- external encoder command path is not used by default for native GStreamer profiles

## Rollout

The first rollout is macOS-only for MP4 and ProRes native GStreamer export.

WebM remains the cross-platform fallback. Windows and Linux MP4 support stay unavailable until their native encoder and AAC factory sets are reviewed with the same policy rules.

## Self-Review

- No placeholders remain.
- MP4 audio is included through `atenc`, not deferred.
- The design is license-safe by failing closed on missing or unexpected factory metadata.
- Scope is one subsystem: native GStreamer MP4/ProRes profile support across backend, availability, UI, and Temporal export-media.
- Cross-platform support is explicitly scoped out until each platform has reviewed factories.
