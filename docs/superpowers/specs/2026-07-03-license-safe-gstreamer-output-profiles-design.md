# License-Safe GStreamer Output Profiles Design

## Goal

Add a long-term, license-safe GStreamer/GES export architecture for WebM, MP4, and ProRes without mixing quality names with codec names. The render pipeline should choose quality and output profile independently, verify every required GStreamer factory against a reviewed policy before rendering, and fail closed when an encoder, muxer, parser, or platform binding is missing or unapproved.

## Current Gap

The Rust render path is WebM-first. `RenderQualityProfile` currently carries `DraftWebm` and `FinalWebm`, `ProjectWebmRenderPaths` writes `output.webm`, and `GstreamerGesRenderBackend` rejects non-`.webm` output. MP4/H.265/ProRes export metadata exists, but codec exports are currently modeled as a Temporal workflow that renders final WebM and then shells out to an approved encoder command.

That works as a policy gate, but it is not the right long-term architecture. GStreamer already owns timeline composition, stream validation, and report artifacts, so the codec target should be a first-class render output profile with explicit plugin policy.

## Requirements

- Keep generated edits EDL-first: selected `sourceIn`/`sourceOut` ranges are rendered before captions, overlays, titles, effects, or HyperFrames layers are attached.
- Keep Rust as the owner of canonical project state, render plans, validation, logs, reports, cancellation, and artifact metadata.
- Separate review quality from container/codec target.
- Make every non-WebM profile unavailable unless all required GStreamer factories exist and pass policy.
- Prefer OS-provided macOS encoders for H.264, H.265, ProRes, and AAC.
- Keep unreviewed or high-risk codec paths denied even when installed locally.
- Keep existing WebM behavior working while migrating names.

## Model

Introduce profile-neutral render model types:

- `RenderQuality`: `draft`, `final`.
- `RenderOutputProfile`: `webmVp8Opus`, `webmVp9Opus`, `mp4H264Aac`, `mp4H265Aac`, `movProResPcm`.
- `GstreamerEncodingTarget`: resolved internal target with extension, MIME type, container caps, video caps, audio caps, required factories, encoder properties, expected stream checks, and platform constraints.

Example combinations:

- Draft review WebM: `quality=draft`, `outputProfile=webmVp8Opus`.
- Final delivery MP4: `quality=final`, `outputProfile=mp4H264Aac`.
- Final ProRes interchange: `quality=final`, `outputProfile=movProResPcm`.

Legacy `draftWebm` and `finalWebm` TypeScript/Rust wire values can remain temporarily only as compatibility aliases at the command boundary. New implementation internals and new tests should use the profile-neutral model.

## Approved Initial GStreamer Targets

### WebM

- `webmVp8Opus`: `webmmux`, `vp8enc`, `opusenc`.
- `webmVp9Opus`: `webmmux`, `vp9enc`, `opusenc`.

These preserve the existing portable LGPL-compatible path.

### MP4 H.264/H.265 With AAC

- `mp4H264Aac`: `mp4mux`, `vtenc_h264`, `atenc`, `aacparse`.
- `mp4H265Aac`: `mp4mux`, `vtenc_h265`, `atenc`, `aacparse`.

`vtenc_h264` and `vtenc_h265` come from the macOS/iOS `applemedia` plugin and route video encoding through Apple VideoToolbox. `atenc` comes from the `osxaudio` plugin and routes AAC through Apple AudioToolbox. Local inspection showed `mp4mux` and `aacparse` from GStreamer Good plug-ins with LGPL metadata, `atenc` from GStreamer Good plug-ins with LGPL metadata, and `vtenc_*` from `applemedia` with LGPL metadata.

These profiles are macOS-first. On non-macOS platforms they should report `unsupportedBuild` or `missingRuntime` rather than selecting another encoder automatically.

### ProRes MOV With PCM

- `movProResPcm`: `qtmux`, `vtenc_prores`, raw PCM audio.

`vtenc_prores` comes from `applemedia` and routes ProRes through Apple VideoToolbox. `qtmux` comes from GStreamer Good plug-ins and supports ProRes plus raw audio in QuickTime/MOV. PCM avoids introducing an AAC dependency for the ProRes interchange profile.

## Denied Initial Factories

Keep these denied even when installed:

- `x264enc`.
- Any `avenc_*` factory and any factory from the `libav` plugin.
- `fdkaac*`.
- `faac`.
- `voaacenc`.
- `openh264enc` until it has a separate licensing and binary distribution review.

The policy should deny by exact factory name and by plugin family where applicable. Factory availability alone must never approve a target.

## Plugin Policy

Extend `render_pipeline::plugin_policy` with reviewed entries for:

- `mp4mux` and `qtmux`: plugin `isomp4`, package family GStreamer Good, LGPL-compatible license.
- `aacparse`: plugin `audioparsers`, package family GStreamer Good, LGPL-compatible license.
- `atenc`: plugin `osxaudio`, package family GStreamer Good, LGPL-compatible license, macOS only.
- `vtenc_h264`, `vtenc_h265`, `vtenc_prores`: plugin `applemedia`, package family GStreamer Bad, LGPL-compatible license, macOS only.

Add an explicit platform constraint to allowed policy entries where the reviewed safety depends on Apple system frameworks. Do not allow a similarly named third-party factory on another OS.

## Render Flow

1. UI or Temporal requests a render/export with `quality` and `outputProfile`.
2. Rust loads the split project and builds an EDL-backed render plan from enabled source-backed video clips.
3. Rust resolves `GstreamerEncodingTarget` for the requested profile.
4. Rust validates every required factory using `require_allowed_factories`.
5. Graphics layers render as transparent PNG frame sequences as they do today.
6. GES renders directly to the target output path.
7. GStreamer discoverer probes the output.
8. Rust validates duration, required streams, dimensions, and expected codec/container metadata.
9. Rust writes JSON/Markdown reports and render logs.
10. Rust applies durable project actions only after successful render and validation.

## UI and Export Behavior

The UI should present output profiles as delivery targets and quality as a separate control. Existing WebM buttons can be migrated incrementally, but new copy should avoid implying that "draft/final" is a codec.

MP4 and ProRes export buttons become available when the GStreamer profile availability report says all required factories are present and policy-approved. The disabled reason should identify missing or denied factories.

## Temporal Behavior

Temporal export-media and render-draft activities should use the same profile-neutral render implementation. For GStreamer-native profiles, Temporal should not run the external approved encoder command. The external encoder command path may remain as a compatibility fallback only if a profile is explicitly marked as external-command-backed, which none of the initial GStreamer-native targets should be.

## Error Handling

- Missing profile: fail before mutation with a profile-specific error.
- Unsupported platform: profile unavailable before queueing a workflow.
- Missing factory: fail before render and name the factory.
- Policy-denied factory: fail before render and include factory name, plugin name, package, license, and verdict.
- Render failure: write logs where available but do not attach a successful project report.
- Probe or validation mismatch: fail before project mutation.
- Audio-required mismatch: MP4 profiles require AAC audio when the project timeline has enabled audio; ProRes MOV uses PCM audio when audio is present.

## Testing

Rust tests:

- `RenderQuality` and `RenderOutputProfile` serialize with profile-neutral names.
- Output profiles resolve to the expected extension, MIME type, caps, factories, and platform constraints.
- MP4 H.264/H.265 profiles require `mp4mux`, `vtenc_h264` or `vtenc_h265`, `atenc`, and `aacparse`.
- ProRes MOV requires `qtmux` and `vtenc_prores`.
- Policy allows reviewed factory metadata and denies `x264enc`, `avenc_*`, `libav`, `fdkaac`, `faac`, and `voaacenc`.
- Render path rejects output extensions that do not match the selected profile.
- Project render paths are profile-neutral and choose the correct extension.
- Export availability reports profile-specific missing or denied factory reasons.

Frontend tests:

- UI can represent quality and output profile separately.
- MP4/ProRes buttons use profile availability rather than external encoder env vars.
- Disabled copy includes missing or policy-denied runtime details.

Integration tests:

- Existing WebM render tests keep passing after migration.
- GStreamer profile summary tests verify MP4 and MOV encoding targets when the factories are available.

## Rollout

1. Add model types and compatibility aliases.
2. Add GStreamer target resolution and policy entries.
3. Migrate WebM path to profile-neutral internals.
4. Add MP4 H.264/H.265 with `atenc` AAC.
5. Add ProRes MOV with PCM.
6. Migrate UI and Temporal exports to the profile-neutral command shape.
7. Remove external encoder command dependence for GStreamer-native profiles.

## Self-Review

- No placeholders remain.
- Scope is focused on the render/export profile architecture and license policy needed for MP4 and ProRes.
- Naming separates quality from codec/container.
- MP4 audio is not deferred: `atenc` is included in the initial approved macOS MP4 profile set.
- Risky alternatives are explicitly denied rather than left ambiguous.
