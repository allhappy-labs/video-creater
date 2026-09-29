# Linux Full Compatibility Delivery Roadmap

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this roadmap task-by-task. Create a
> focused child implementation plan for a delivery packet before changing code;
> steps in those plans use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a full-featured Video Creater for Ubuntu 24.04 LTS x86_64 on
Wayland and X11 with app-owned tools and baseline models, no user toolchain or
first-run runtime download, and no LGPL or other prohibited code linked or loaded
by any app-owned process.

**Architecture:** Keep the canonical project, EDL, proposal, job, cancellation,
artifact, and report logic in Rust. Linux uses a static-musl native desktop host,
direct audited protocols to independent OS services, a versioned app-owned media
worker, and versioned app-owned model and agent workers. The current React/Tauri,
AVFoundation, Core ML, and GStreamer defaults remain the macOS implementation;
Linux target selection cannot enable or package them.

**Tech Stack:** Rust 2021, `x86_64-unknown-linux-musl`, direct X11 and Wayland
protocol clients, a candidate pinned Iced 0.14 component graph rendered only by
`iced_tiny_skia` into caller-owned CPU RGBA buffers, PulseAudio protocol, D-Bus
protocols for portals/notifications/Secret Service, OpenH264 2.6.0, libxaac
0.1.13, `mp4` 0.14.0 as the accepted proof baseline, app-owned model workers,
Node-based build/evidence tooling, and Debian packaging. Iced is not qualified
until its exact resolved graph, features, fonts, ELF, and live mappings pass the
same inventory gate as every other candidate.

**Spec:**
[`docs/superpowers/specs/2026-09-12-linux-compatibility-design.md`](../specs/2026-09-12-linux-compatibility-design.md)

## Global Constraints

- No LGPL in the app, its helpers, or any libraries they link or load. This
  applies to static code, direct and transitive dynamic dependencies, and
  client/driver libraries found on the system.
- Independent installed OS services are allowed: the window compositor/X server,
  audio server, portal, notification service, and credential service. Their
  client libraries remain prohibited unless separately bundled and qualified.
- Every application utility, codec, helper, model runtime, agent executable, MCP
  executable, required interpreter, baseline weight, font, template, and runtime
  library ships in the installed application. Baseline local features work with
  the network disabled after installation.
- Runtime resolution uses only absolute paths below a verified immutable install
  root. It never searches `PATH`, shell startup files, package-manager locations,
  developer checkouts, writable model stores, or global plugin directories.
- Source and artifact manifests fail closed on unknown, missing, ambiguous,
  copyleft, or unreviewed license terms. The currently eligible terms are `MIT`,
  `Apache-2.0`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, and the separately
  approved exact term `Apache-2.0 WITH LLVM-exception`.
- Rust owns canonical project mutation, proposal validation, job lifecycle,
  cancellation, helper cleanup, logs, and artifact metadata. Helpers write only
  to allocated job directories.
- Generated edits are EDL-first: every primary clip has selected `sourceIn` and
  `sourceOut` ranges before captions, overlays, title cards, effects, or
  HyperFrames layers are added.
- Every visual layer retains `visualTreatment`, `motion`, `safeZone`, and `avoid`.
  Render review covers duration, streams, caption alignment, overlay timing,
  artifact paths, log references, alpha, and legibility over real footage.
- CPU rendering, decoding, encoding, and playback are the correctness baseline.
  Acceleration is additive and cannot change supported baseline formats or
  timeline semantics.
- The existing macOS default feature list, commands, runtime contents, project
  schema, and behavior remain unchanged unless a shared refactor is proven by
  fresh macOS checks on a Mac.
- A failed dependency, license, codec, model, or patent gate blocks the affected
  release claim. It cannot be bypassed by a system package, helper process,
  first-run download, different container, or reduced feature claim.
- Third-party CI actions are pinned to immutable commit SHAs.

## Accepted starting evidence

The branch starts from `ea020cd3`. The following evidence can be reused within
its stated boundary and must not be rebuilt merely to begin production work:

| Evidence | Accepted result | Boundary that remains open |
| --- | --- | --- |
| Canonical core probe | Real `video-creater` project/storage library persists and reopens a full typed fixture under static musl | No production app host, cross-platform fixture round trip, or package |
| X11 proof | Mapped window, CPU pixel, synthetic input, direct file access, cleanup | No full UI, real desktop input, scaling, clipboard, or service integration |
| Wayland proof | xdg-shell window, CPU buffer, frame callback, cleanup | No input, compositor readback, full UI, scaling, or installed-desktop acceptance |
| Audio proof | PCM sent and recovered through a private PulseAudio sink | No production playback clock, reconnect, device switching, or A/V sync |
| Combined media proof | Exact static-musl OpenH264/libxaac/`mp4` one-process H.264/AAC MP4 round trip | Fixed 64x48, 30 fps, one-second owned fixture; no production backend or common-MP4 contract |
| Combined proof review | 268 tests passed; exact binary and receipt chain passed independent full review | No production backend routing, full UI, packaging, broad conformance, or release claim |
| LLVM term gate | Exact `Apache-2.0 WITH LLVM-exception` approved | No blanket exception-bearing-license approval |

Preserve the accepted evidence roots. Production work consumes their locked
inputs and findings but uses new manifests, crates, tests, and evidence roots.

## Current acceptance scope (2026-09-12)

The user directed current implementation and acceptance testing to run on this
Ubuntu VM. Headless protocol, static-musl, fixture, package-safe, and other
VM-available checks can complete packets without waiting for a physical desktop,
audio device, or Mac. Keep those unavailable checks explicitly unverified:
real Wayland/X11 input and audio-device acceptance remains Packet 19 evidence,
and macOS regression remains Packet 20 evidence. Their absence is not a blocker
for current VM delivery and must not be reported as a pass.

## Release architecture

```text
native Linux desktop host
  -> shared Rust application services
       -> canonical project / actions / EDL / jobs / reports
       -> permissive media worker protocol
       -> model worker protocols
       -> Codex app-server and Video Creater MCP protocols
  -> direct X11 or Wayland client
  -> direct PulseAudio client
  -> D-Bus protocols to portals, notifications, and Secret Service

immutable installation root
  -> app executables + workers + native archives/runtime closure
  -> baseline model weights + tokenizers
  -> fonts + templates + skills
  -> runtime manifest + SBOM + notices + source provenance

writable XDG roots
  -> projects + logs + job artifacts + caches + optional larger models
```

The Linux UI is a native Rust presentation layer. No currently qualified web
engine can host the React build within the approved dependency boundary. The
first product candidate uses only `iced_core`, `iced_widget`, `iced_runtime`,
`iced_renderer` with its `tiny-skia` renderer, and `iced_tiny_skia` with default
features disabled, driven by the app's own Wayland/X11 shells. Do not add top-level
Iced defaults, `iced_winit`, eframe, softbuffer display features, wgpu, system
fonts, or rfd. This candidate advances only after the exact locked graph passes;
if it fails, keep the native host boundary and replace the renderer candidate.
The native layer uses the same domain types and application services as macOS and
is held to the existing editor behavior and visual acceptance fixtures. React
remains the macOS presentation layer; this roadmap does not weaken either path.

## Critical path and concurrency

The release critical path is:

```text
runtime manifest/resolver
  -> production host + shared application services -> complete native editor UI
  -> production media worker -> common MP4 -> compositor/export/playback
  -> bundled transcription -> remaining local model parity
  -> bundled Codex/MCP
  -> deterministic package -> installed-desktop matrix -> macOS regression
```

After the runtime contract lands, three lanes can proceed without sharing
implementation files:

1. Packets 2 through 2C, the media protocol, production worker, canonical
   PNG/WAV compositor, and routed MP4 vertical slice.
2. Packet 3, the production Wayland Project Home and host lifecycle, after its
   exact renderer/font graph passes the Packet 1 contract.
3. Model and agent artifact research that writes no shared runtime source.

After those land, media packets 6-9, desktop packets 10-13, and model packets
14-15 can advance independently. Only integration, packaging, and final release
qualification serialize all lanes. An unavailable real desktop, Mac, hardware,
provider account, or signing service blocks only its evidence packet while code,
fixtures, and other independent gates continue.

## Delivery packets

Each packet is one review boundary. Its child plan must include failing tests,
the smallest implementation, focused verification, and a Conventional Commit.
No packet may change the accepted evidence report or compatibility spec.

### Packet 1: Production runtime manifest, resolver, and audit contract

**Status (2026-09-12): Packet 1A complete.** Runtime resolver commit `52a800d`
passed independent review with zero findings, 14 host contract tests, 14 musl
contract tests, 5 core tests, 31 preflight tests, and static ELF verification.

**Files:**

- Create `src-tauri/src/platform_runtime/manifest.rs`
- Create `src-tauri/src/platform_runtime/resolver.rs`
- Create `src-tauri/src/platform_runtime/services.rs`
- Create `src-tauri/src/platform_runtime/mod.rs`
- Create `src-tauri/resources/linux-runtime/runtime-manifest.schema.json`
- Create `native/linux-runtime-contract/Cargo.toml`
- Create `native/linux-runtime-contract/Cargo.lock`
- Create `native/linux-runtime-contract/src/main.rs`
- Create `native/linux-runtime-contract/tests/contract.rs`
- Create `scripts/build-linux-runtime-manifest.mjs`
- Create `scripts/build-linux-runtime-manifest.test.ts`
- Create `scripts/verify-linux-runtime-payload.mjs`
- Create `scripts/verify-linux-runtime-payload.test.ts`
- Modify `src-tauri/src/lib.rs`

**Interfaces:**

- Produce `RuntimeManifest`, `RuntimeArtifact`, `IndependentService`,
  `RuntimeResolver::from_install_root(root, manifest)`, and
  `RuntimeResolver::resolve(artifact_id)`.
- Every artifact records target, relative path, SHA-256, bytes, protocol name and
  version where applicable, source revision, selected SPDX expression, notices,
  build flags, and dependencies.
- Independent services record protocol, endpoint discovery, availability probe,
  reconnect policy, and failure behavior; they never appear as library graph
  exemptions.

**Acceptance:**

- Reject traversal, absolute manifest paths, writable-root executables, hash or
  target mismatch, duplicate IDs, missing dependencies, forbidden search
  variables, mixed protocol versions, and app-owned executables classified as OS
  services.
- `rtk node --test scripts/build-linux-runtime-manifest.test.ts scripts/verify-linux-runtime-payload.test.ts`
  passes.
- `native/linux-runtime-contract` declares its own empty workspace and path
  dependency on `../../src-tauri` with `default-features = false`, so contract
  tests cannot pull the root package's unconditional Tauri dev-dependency.
- `rtk cargo test --manifest-path native/linux-runtime-contract/Cargo.toml --locked --target x86_64-unknown-linux-musl`
  passes.

### Packet 2: Fail-closed permissive media protocol and client seam

**Status (2026-09-12): In progress.** The reviewed isolated-harness
contract/client and Packet 2A worker gates are active after Packet 1A closure.
Packet 2B has an executable child plan at
[`2026-09-12-linux-canonical-media-spools.md`](2026-09-12-linux-canonical-media-spools.md)
and starts only after the Packet 2A source and worker gate passes.

**Files:**

- Create `src-tauri/crates/permissive-media-protocol/Cargo.toml`
- Create `src-tauri/crates/permissive-media-protocol/src/lib.rs`
- Create `src-tauri/src/render_pipeline/permissive_media_backend.rs`
- Create `native/linux-permissive-media-contract/Cargo.toml`
- Create `native/linux-permissive-media-contract/Cargo.lock`
- Create `native/linux-permissive-media-contract/src/main.rs`
- Create `native/linux-permissive-media-contract/tests/contract.rs`
- Modify `src-tauri/Cargo.toml`
- Modify `src-tauri/src/lib.rs`
- Modify `src-tauri/src/render_pipeline/mod.rs`

**Interfaces:**

- Protocol `video-creater.permissive-media`, schema version 1, newline-delimited
  JSON, `deny_unknown_fields`, request IDs, progress events, and exactly one final
  event. The request envelope is `protocol`, `schemaVersion`, `requestId`,
  `operation`, and `budgets`.
- Version 1 operations are `Capabilities`; `EncodeMp4 { caller_job_root,
  output_root, output_path, video, audio, profile }`; `Probe { caller_job_root,
  source_path }`; and `ExtractFrames { caller_job_root, source_path, output_root,
  width, height, frames }`. The only initial advertised profile is
  `H264AacProgressiveBaselineV1 { video_bitrate,
  keyframe_interval_frames, audio_bitrate }`; this bounded
  raw-RGBA8/optional-S16LE profile cannot be presented as the common MP4
  contract.
- `RationalFrameRate` uses numerator/denominator and requests include exact
  frame/sample counts plus wall-time, input-byte, output-byte, video-frame,
  pixels-per-frame, and audio-frame budgets.
- `RawVideoInput` fixes `pixel_format = Rgba8` and carries path, dimensions,
  rational rate, and frame count. `RawAudioInput` fixes
  `sample_format = S16leInterleaved` and carries path, sample rate, channels, and
  frame count. `Capabilities` enumerates exact operation/profile/format/range
  tuples.
- The client reuses `process_supervisor::run_to_completion` and the existing
  `RenderCancellationToken: CancellationSignal`; it clears the environment,
  launches only the runtime-resolver absolute sibling worker, writes one NDJSON
  request, caps stdout at 1 MiB and stderr at 128 KiB, and sets a parent deadline
  no later than the request budget.
- Production never accepts an arbitrary worker override. A worker-path constructor
  exists only behind test/development configuration.

**Acceptance:**

- Validate canonical source files and a caller-owned, storage-validated fresh job
  root before and after execution; reject symlinks, path escape, changed inputs,
  escaped results, and capability omission. Do not hard-code a new
  `project/renders` layout where existing storage allocates a different one.
- Completed encode results use a relative output path and include byte length,
  `MediaProbe`, coded video/audio duration, and encoder delay/padding when known;
  worker-returned absolute paths are always rejected.
- Tests cover invalid envelope/budget/rational, correct fake worker, missing exact
  profile, mismatched ID/version, duplicate terminal, progress-only output,
  nonzero exit, oversized output, cancellation, timeout, and descendant reaping.
- Do not change backend routing, readiness, default features, Tauri configuration,
  or macOS selection in this packet.
- `native/linux-permissive-media-contract` declares its own empty workspace and
  path-depends on `../../src-tauri` with `default-features = false` plus the
  protocol crate, avoiding the root package's unconditional Tauri dev-dependency
  and any GTK pull.
- `rtk cargo test --manifest-path src-tauri/crates/permissive-media-protocol/Cargo.toml --locked`,
  `rtk cargo test --manifest-path native/linux-permissive-media-contract/Cargo.toml --locked`,
  and `rtk cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` pass.

### Packet 2A: Production-shaped static-musl media worker

**Files:**

- Create `src-tauri/crates/permissive-media-worker/Cargo.toml`
- Create `src-tauri/crates/permissive-media-worker/build.rs`
- Create `src-tauri/crates/permissive-media-worker/src/main.rs`
- Create `src-tauri/crates/permissive-media-worker/src/codec.rs`
- Create `src-tauri/crates/permissive-media-worker/src/mp4.rs`
- Create `src-tauri/crates/permissive-media-worker/src/color.rs`
- Create `src-tauri/crates/permissive-media-worker/native/codec_bridge.h`
- Create `src-tauri/crates/permissive-media-worker/native/h264_bridge.cpp`
- Create `src-tauri/crates/permissive-media-worker/native/aac_bridge.c`
- Create `scripts/build-permissive-media-worker.mjs`
- Create `scripts/build-permissive-media-worker.test.ts`
- Create `scripts/verify-permissive-media-worker.mjs`
- Create `scripts/verify-permissive-media-worker.test.ts`

**Interfaces and acceptance:**

- Create a new production source lock and notice inventory. Reuse locked upstream
  inputs and lessons from the proof; do not depend on or mutate its fixed crate,
  source, build, or evidence roots.
- Support arbitrary bounded even dimensions, frame counts, rational frame rates,
  bitrate, and key interval; stream ownership in bounded chunks instead of one
  proof-sized allocation. Feed AAC incrementally and flush it explicitly.
- Read exact-size raw spools below the allocated output root, convert RGBA8 to I420
  under a declared BT.709 limited-range contract, write `.part`, fsync, validate,
  atomically rename, and report encoder delay, padding, and track durations.
- Initial `Probe` and `ExtractFrames` accept only the worker's own no-B-frame,
  no-edit-list output profile. Two dimensions, 24 and 30000/1001 fps, more than
  one second of audio, truncation, extra bytes, odd dimensions, codec/allocation
  faults, cancellation cleanup, output budgets, atomicity, and color bars pass.
- Fresh static-musl contributor, source-license, notice, SBOM, ELF, and process
  evidence is required. Attempt 13 does not qualify changed production code.

### Packet 2B: Canonical PNG/WAV compositor and audio mixer

**Files:**

- Create `src-tauri/src/render_pipeline/permissive_compositor.rs`
- Create `src-tauri/src/render_pipeline/permissive_audio_mixer.rs`
- Create `native/linux-permissive-media-contract/tests/canonical_render.rs`
- Modify `src-tauri/src/render_pipeline/mod.rs`

**Interfaces and acceptance:**

- Consume a real `RenderPlan` and `GraphicsArtifactManifest`; render frame by
  frame into `worker/video.rgba` without retaining the complete frame set.
- Decode PNG/still/prepared PNG sequences with the existing image path; apply
  `PreparedEffectStack`, `transform_rgba8_srgb`, `FrameBlendCompositor`, and
  graphics manifest sequence/static-hold timing and alpha.
- Mix bounded WAV PCM into 48 kHz stereo S16LE using source ranges, timeline
  starts, speed, gain, fades, and silence. Reject every other source/container or
  audio shape explicitly in this first slice.
- Video DTS is `frame_index * fps.denominator` at timescale `fps.numerator`; audio
  begins at sample zero on the 48 kHz clock; output frame count defines duration.
- A canonical two-cut PNG/WAV project with a prepared overlay proves cut-frame
  changes, scheduled-only overlay pixels, gain/fades, silence gaps, range renders
  starting at zero, no full-source pass-through, and bounded spool cleanup on
  cancellation.

### Packet 2C: Route the narrow vertical slice through canonical jobs

**Files:**

- Modify `src-tauri/src/render_pipeline/project_export.rs`
- Modify `src-tauri/src/project/export_profiles.rs`
- Modify `src-tauri/src/settings/render_system.rs`
- Modify `src-tauri/src/settings/health.rs`
- Modify target build and bundle configuration introduced by Packet 4
- Create `native/linux-permissive-media-contract/tests/project_export.rs`

**Interfaces and acceptance:**

- Select the permissive worker on Linux only after exact capability probing; keep
  AVFoundation/GStreamer macOS selection unchanged. Decouple
  `ProjectMediaRenderPaths::new` from a GStreamer target used only to choose an
  extension.
- Use worker `ExtractFrames` for post-render review on this route. Reuse current job
  start/terminal actions, cancellation quarantine, `validate_rendered_media`,
  reports, logs, and artifact attachment.
- End-to-end test the canonical `render_media_to_split_project_folder` entry with
  the PNG/WAV/graphics project and assert completed job, report, output probe, and
  sampled frames. Missing, old, or corrupt worker stays unavailable and never
  falls through to GStreamer, FFmpeg, or `PATH`.
- This packet proves one narrow integrated profile. It does not mark common MP4,
  general composition, WebM, or Linux release readiness complete.

### Packet 3: Production Wayland Project Home vertical slice

**Files:**

- Create `native/linux-desktop/Cargo.toml`
- Create `native/linux-desktop/Cargo.lock`
- Create `native/linux-desktop/src/main.rs`
- Create `native/linux-desktop/src/app.rs`
- Create `native/linux-desktop/src/project_session.rs`
- Create `native/linux-desktop/src/platform/mod.rs`
- Create `native/linux-desktop/src/platform/wayland.rs`
- Create `native/linux-desktop/src/ui/mod.rs`
- Create `native/linux-desktop/src/ui/project_home.rs`
- Create `native/linux-desktop/src/ui/theme.rs`
- Create `native/linux-desktop/src/ui/fonts.rs`
- Create `native/linux-desktop/src/accessibility.rs`
- Create `native/linux-desktop/tests/project_home.rs`
- Create `native/linux-desktop/tests/host_event_translation.rs`
- Create `native/linux-desktop/tests/software_frame.rs`
- Create `scripts/verify-linux-desktop-wayland.mjs`
- Create `scripts/verify-linux-desktop-wayland.test.ts`
- Create `src-tauri/resources/linux-runtime/fonts/README.md`

**Interfaces:**

- `DesktopBackend` exposes `capabilities`, `poll_event`, `configure`, `present`,
  and `set_clipboard`; `HostEvent` represents configure, close, key, text,
  pointer, scroll, focus, scale, and frame events.
- The initial UI opens an explicitly supplied `--project DIR` through a path
  dependency on the real `video_creater_lib` with `default-features = false` and
  `project::split::load_split_project`, then shows the project name, duration,
  and tracks from canonical state. The crate has its own empty workspace and lock
  and does not modify the root lock.
- Translate pointer, keyboard, and text events into the qualified Iced component
  runtime and render the dense Project Home into a caller-owned tiny-skia buffer.
- Carry a reviewed pinned patch that initializes an empty cosmic-text font
  database and loads only the bundled, hash-pinned Apache-licensed font. Reject
  the candidate if it scans system fonts.
- Define stable semantic node IDs and a serialized accessibility snapshot in this
  slice; full AT-SPI/Orca behavior remains Packet 10's release gate.
- Service connections use the inherited desktop endpoint variables documented in
  Packet 1 while all library/plugin search variables remain cleared.

**Acceptance:**

- The exact locked Iced component graph and bundled font enter Packet 1's
  machine-readable inventory and pass declaration, source, contributor, ELF, and
  live-map checks before this slice is called accepted.
- Unit tests cover event translation, scale/damage, project messages, invalid
  project paths, and real project open/list/save. A headless golden test verifies
  the Project Home pixel buffer.
- Headless Weston integration asserts globals, configure, keyboard/pointer/text,
  frame callback, clean exit, static ELF with no interpreter or `DT_NEEDED`, and
  complete live mappings.
- `rtk cargo test --manifest-path native/linux-desktop/Cargo.toml --locked --target x86_64-unknown-linux-musl`
  and the matching release build pass, followed by
  `rtk node scripts/verify-linux-desktop-wayland.mjs --binary "$PWD/native/linux-desktop/target/x86_64-unknown-linux-musl/release/video-creater-linux-desktop" --project-fixture "$PWD/src-tauri/resources/sample-project" --output "$PWD/output/linux-desktop-wayland-evidence-attempt1"`.

### Packet 4: Linux target isolation and shared application services

**Files:**

- Create `src-tauri/src/app_services/mod.rs`
- Create `src-tauri/src/app_services/commands.rs`
- Create `src-tauri/src/app_services/events.rs`
- Create `src-tauri/src/app_services/state.rs`
- Create `src-tauri/src/platform_target.rs`
- Modify `src-tauri/src/main.rs`
- Modify `src-tauri/Cargo.toml`
- Modify `package.json`

**Interfaces:**

- `PlatformTarget::{MacOsApp, LinuxPermissive}` is selected from an explicit
  target triple at build time.
- `AppServices` owns the state currently initialized by `main.rs`; typed service
  methods become the single implementation behind thin Tauri commands and native
  Linux actions.
- `AppEventSink` decouples project/job/progress events from `tauri::Emitter`.

**Acceptance:**

- Linux checks use only the `linux-permissive` feature set and cannot activate
  `app-runtime`, `ges-render`, `coreml-inspect`, `temporal-worker`, or a macOS
  helper build.
- macOS defaults in `src-tauri/Cargo.toml`, `package.json`, and
  `src-tauri/tauri.conf.json` remain behaviorally unchanged.
- Existing command contract tests pass through the Tauri adapter; new tests run
  the same save/load/action/proposal/job operations through `AppServices` without
  Tauri.

### Packet 5: Capability and failure model

**Files:**

- Create `src-tauri/src/platform_runtime/capabilities.rs`
- Modify `src-tauri/src/settings/health.rs`
- Modify `src-tauri/src/settings/render_system.rs`
- Modify `src-tauri/src/project/export_profiles.rs`
- Modify `src/lib/settings/health.ts`
- Modify `src/components/settings/system-health.tsx`

**Interfaces:**

- `RuntimeCapabilities` reports backend identity, protocol version, supported
  operations and exact formats/profiles, service availability, and one of
  `ready`, `unavailable`, `missing`, `corrupt`, `incompatible`, or `failed`.
- Linux render health describes the permissive media worker and never treats
  absent GStreamer or AVFoundation as the Linux failure reason.

**Acceptance:**

- A capability is ready only after its manifest artifact and self-test pass.
- Missing optional services do not block offline editing; missing required media,
  model, or host artifacts make the corresponding workflow unavailable with a
  reinstall/repair diagnostic rather than package-manager advice.
- Mac health snapshots retain existing IDs and outcomes.

### Packet 6: Common MP4 ingress, probing, and seeking

**Files:**

- Create `src-tauri/crates/permissive-media-worker/src/mp4_input.rs`
- Create `src-tauri/crates/permissive-media-worker/src/h264.rs`
- Create `src-tauri/crates/permissive-media-worker/src/aac.rs`
- Create `src-tauri/crates/permissive-media-worker/tests/mp4_conformance.rs`
- Modify `src-tauri/crates/permissive-media-protocol/src/lib.rs`
- Modify `src-tauri/src/project/source_probe.rs`
- Modify `src-tauri/src/media_inspection.rs`
- Modify `src-tauri/src/precompose/frame_source.rs`

**Interfaces:**

- Add probe, decode-frame, decode-audio-window, thumbnail, and seek operations.
- Support the documented baseline progressive MP4 contract including H.264
  Baseline/Main/High 8-bit 4:2:0, AAC-LC mono/stereo at 44.1/48 kHz, signed
  composition offsets, B-frame reordering, `stss`, `ctts`, rotation matrices,
  `co64`, variable frame rate, and edit lists. Fragmented MP4 is accepted only if
  the conformance task proves the same safety and seek guarantees.

**Acceptance:**

- Owned and redistributable conformance fixtures cover every claimed profile,
  rotation, VFR, B-frame, edit-list, large-offset table, truncation, integer
  overflow, inconsistent-table, and malformed-NAL case.
- Independent inspection verifies duration, stream shape, presentation order,
  seek to the preceding sync sample, decoded frame checksums, PCM length, and
  bounded error returns with no panic.
- Existing import, media analysis, filmstrip, precompose, and audio-sync callers no
  longer call GStreamer on Linux.

### Packet 7: Timeline compositor, preview clock, and audio mixer

**Files:**

- Create `src-tauri/src/media_backend/compositor.rs`
- Create `src-tauri/src/media_backend/audio_mixer.rs`
- Create `src-tauri/src/media_backend/preview.rs`
- Create `src-tauri/src/media_backend/timebase.rs`
- Create `src-tauri/tests/linux_timeline_media.rs`
- Modify `src-tauri/src/frame_compositor/`
- Modify `src-tauri/src/precompose/`
- Modify `src-tauri/src/timeline_filmstrip.rs`

**Interfaces:**

- Consume validated `RenderPlan` timestamps and output RGBA8 frames plus interleaved
  f32 PCM on one rational timebase.
- Reuse current CPU transforms, effects, blend modes, graphics manifests, caption
  timing, and alpha logic. Preview and export use the same sampling rules.
- `PreviewSession` exposes seek, play, pause, rate, dropped-frame count, underrun,
  current timeline time, and cancellation.

**Acceptance:**

- Fixtures cover trim, split, reorder, gaps, overlaps, speed, opacity, transforms,
  nested timelines, captions, transparent layers, audio gain/fades, channel
  conversion, resampling, and cancellation.
- Preview and rendered frames match within recorded per-channel tolerances at
  first/middle/last and every cut boundary.
- Ten-minute synthetic and real-media runs remain within one video frame of audio
  at the start, each cut, and the end, with no unbounded memory growth.

### Packet 8: MP4 and WebM delivery

**Files:**

- Create `src-tauri/crates/permissive-media-worker/src/mp4_output.rs`
- Create `src-tauri/crates/permissive-media-worker/src/webm.rs`
- Create `src-tauri/src/render_pipeline/permissive_backend.rs`
- Create `src-tauri/tests/linux_delivery_profiles.rs`
- Modify `src-tauri/src/render_pipeline/backend.rs`
- Modify `src-tauri/src/render_pipeline/output_profile.rs`
- Modify `src-tauri/src/render_pipeline/project_export.rs`
- Modify `src-tauri/src/project/export_profiles.rs`

**Interfaces:**

- Preserve the existing `RenderBackend` contract while selecting
  `PermissiveRenderBackend` for Linux MP4 H.264/AAC and WebM profiles.
- Exact accepted libvpx/libwebm/Opus revisions, build flags, archives, notices,
  hashes, and contributor closure are locked before WebM is enabled.
- Delivery finalizes atomically only after reopening the artifact and validating
  duration, streams, frame content, audio, and report metadata.

**Acceptance:**

- Draft and final outputs satisfy declared bitrate/quality/profile bounds and are
  independently reopened.
- Cut-boundary sync, encoder delay/priming/padding, long-run drift, alpha handling
  where supported, malformed input, cancellation, disk-full, and interrupted
  finalize tests pass.
- MP4 and WebM appear available on Linux only when the exact packaged worker
  self-test passes; Mac profile selection is unchanged.

### Packet 9: Media qualification and performance evidence

**Files:**

- Create `scripts/verify-linux-production-media.mjs`
- Create `scripts/verify-linux-production-media.test.ts`
- Create `docs/development/linux-media-fixtures.md`
- Create `docs/research/2026-09-12-linux-production-media-evidence.md`

**Interfaces and acceptance:**

- Record expected outcomes and tolerances before executing each fixture.
- Run broad H.264/AAC/ISO BMFF and WebM conformance, malformed corpora,
  independent decode/inspection, seek, cut, captions, graphics, cancellation,
  concurrency, and 10-minute sync tests.
- Measure 720p and 1080p preview/export throughput, startup latency, peak RSS,
  scratch use, and package-size contribution on the baseline CPU.
- Retain commands, tool hashes, fixture provenance, logs, artifact hashes, and
  passed/failed/blocked/unverified status. A self-decoding codec round trip alone
  cannot pass this packet.

### Packet 10: X11 backend, native editor scene, and interaction framework

**Files:**

- Create `native/linux-desktop/src/platform/x11.rs`
- Create `native/linux-desktop/src/ui/scene.rs`
- Create `native/linux-desktop/src/ui/layout.rs`
- Create `native/linux-desktop/src/ui/widgets.rs`
- Create `native/linux-desktop/src/ui/text.rs`
- Create `native/linux-desktop/src/ui/focus.rs`
- Create `native/linux-desktop/src/ui/accessibility.rs`
- Create `scripts/verify-linux-desktop-x11.mjs`

**Interfaces:**

- Retained `EditorScene` renders deterministic RGBA8 buffers through bundled,
  audited fonts and existing CPU graphics primitives.
- Widget state covers default, hover, focus-visible, disabled, selected,
  multi-selected, drag/resize, invalid target, loading, queued, blocked, failed,
  completed, and empty states.
- Input maps platform events to logical Ctrl shortcuts, text/IME, focus traversal,
  pointer capture, drag/drop, and accessibility descriptions.
- Runtime selection prefers a valid Wayland session and falls back to X11. An
  explicit test selector is allowed; production does not hide protocol/security
  failures by switching backends after initialization.
- Accessibility is an explicit release gate: qualify the exact AT-SPI client path
  and test with Orca. Stable Iced accessibility gaps cannot be accepted as a
  reason to omit semantics.

**Acceptance:**

- No font, icon, theme, or renderer dependency loads from an unverified system
  path.
- Deterministic scene tests cover scaling at 1.0, 1.25, 1.5, and 2.0; keyboard-only
  navigation; reduced motion; and non-ASCII text.
- Benchmarks must sustain the release frame budget for dense timeline text at
  1080p on the baseline CPU; current public reports of Iced CPU text cost are a
  material candidate risk, not an accepted limitation.
- Dependency and actual-load evidence for the renderer is inventory-eligible with
  zero unclassified contributors.

### Packet 11: Complete native editor workspace

**Files:**

- Create `native/linux-desktop/src/ui/workspace.rs`
- Create `native/linux-desktop/src/ui/media_bin.rs`
- Create `native/linux-desktop/src/ui/preview.rs`
- Create `native/linux-desktop/src/ui/timeline.rs`
- Create `native/linux-desktop/src/ui/inspector.rs`
- Create `native/linux-desktop/src/ui/codex_panel.rs`
- Create `native/linux-desktop/src/ui/settings.rs`
- Create `native/linux-desktop/tests/editor_workflows.rs`

**Interfaces and acceptance:**

- Bind UI actions to `AppServices`; do not mutate `VideoProject` directly from the
  scene layer.
- Implement every release-contract operation: create/open/import, trim, split,
  reorder, multi-select, resize, seek, play/pause, preview, undo/redo, save,
  reopen, relink, generate edit, inspect/apply/reject agent proposal, caption and
  layer editing, render, cancel/retry, artifact inspection, and Settings health.
- Keep media bin, preview, inspector, timeline, and Codex panel visible or one
  action away. Track identity, source range, time, duration, and validation errors
  are inspectable at the item.
- Compare fixed native screenshots at 1440x920, 1280x720, 1100x760, and a narrow
  recovery layout against the established editor information architecture. The
  native UI cannot pass with missing workflows hidden or marked unsupported.

### Packet 12: Desktop services and secure credentials

**Files:**

- Create `src-tauri/src/desktop_services/mod.rs`
- Create `src-tauri/src/desktop_services/dbus.rs`
- Create `src-tauri/src/desktop_services/portal.rs`
- Create `src-tauri/src/desktop_services/notifications.rs`
- Create `src-tauri/src/desktop_services/secret_service.rs`
- Create `src-tauri/src/desktop_services/xdg.rs`
- Modify `src-tauri/src/provider_credentials.rs`
- Modify `src-tauri/src/native_menu.rs`

**Interfaces and acceptance:**

- Direct audited D-Bus protocol clients provide file/folder dialogs, open/reveal,
  notifications, and Secret Service. XDG paths separate config, data, cache, and
  state with restrictive permissions for secrets and logs.
- Credential tests cover save, replace, resolve internally, delete, restart,
  locked collection, unavailable service, malformed response, timeout, and an
  isolated test namespace. No plaintext fallback or secret value reaches UI,
  logs, diagnostics, process arguments, or environment snapshots.
- File operations cover Unicode, spaces, read-only roots, denied portal requests,
  missing file managers, and cancellation. Optional service failure leaves offline
  editing usable and health accurately degraded.

### Packet 13: Production audio service and playback

**Files:**

- Create `src-tauri/src/desktop_services/audio.rs`
- Modify `native/linux-desktop/src/ui/preview.rs`
- Modify `src-tauri/src/media_backend/preview.rs`
- Create `src-tauri/tests/linux_audio_playback.rs`

**Interfaces and acceptance:**

- Use the audited PulseAudio wire protocol against the independent desktop audio
  service; document PipeWire's PulseAudio-compatible endpoint as the Ubuntu
  baseline service route.
- Implement format negotiation, clock/latency reporting, pause/resume, seek flush,
  volume/mute, default-device changes, reconnect, underrun recovery, and bounded
  buffering.
- Private sink tests and real-session tests verify audible playback, device
  switching, server restart, cut-boundary sync, and no stale audio after seek.

### Packet 14: Bundled model runtime and word-timed transcription

**Files:**

- Create `src-tauri/crates/model-worker-protocol/Cargo.toml`
- Create `src-tauri/crates/model-worker-protocol/src/lib.rs`
- Create `src-tauri/crates/linux-model-worker/Cargo.toml`
- Create `src-tauri/crates/linux-model-worker/src/main.rs`
- Create `src-tauri/src/transcription/linux_worker.rs`
- Modify `src-tauri/src/transcription/runtime.rs`
- Modify `src-tauri/src/transcription/model.rs`
- Modify `src-tauri/src/transcription/store.rs`
- Modify `src-tauri/src/edit/render_plan.rs`

**Interfaces and acceptance:**

- Lock and bundle the smallest multilingual Whisper/whisper.cpp artifact that
  passes the fixed speech corpus; record exact revisions, weights/tokenizer hashes,
  notices, memory, latency, and installer cost. Optional larger models retain the
  explicit verified download flow, but the baseline model is never downloaded at
  first launch.
- Preserve `ModelRuntime`, installed-model validation, word timestamps, progress,
  cancellation, retries, restart recovery, and model-version isolation.
- Corpus acceptance is WER no more than two percentage points worse than the
  retained macOS baseline, median word-boundary error at most 200 ms, p95 at most
  500 ms, and correct language-mode behavior on every published language fixture.
- A spoken fixture generates a real multi-range EDL and captions remap after cuts;
  a full-source styled pass-through fails the test.

### Packet 15: Speech analysis, enhancement, and semantic parity

**Files:**

- Create `src-tauri/crates/linux-model-worker/src/speech_analysis.rs`
- Create `src-tauri/crates/linux-model-worker/src/audio_enhancement.rs`
- Create `src-tauri/crates/linux-model-worker/src/semantic.rs`
- Modify `src-tauri/src/speech_analysis.rs`
- Modify `src-tauri/src/speech_models.rs`
- Modify `src-tauri/src/precompose/audio_denoise.rs`
- Modify `src-tauri/src/search/semantic_runtime.rs`
- Create `src-tauri/tests/linux_model_parity.rs`

**Interfaces and acceptance:**

- Evaluate candidate engines and models only through exact artifact manifests and
  fixed corpora. ONNX Runtime or another candidate is adopted only after its
  enabled execution provider and complete static-musl closure qualify.
- Diarization matches retained speaker-count and segment-overlap expectations;
  enhancement improves noisy-fixture SI-SDR by at least 3 dB without degrading the
  clean fixture by more than 1 dB; semantic retrieval matches at least 9 of 10
  retained top-result expectations and version-mismatched indexes are rejected.
- Baseline weights for every local feature ship in the package. No Core ML helper,
  Swift runtime, Python, model server, or writable-store executable is used on
  Linux.

### Packet 16: Bundled Codex and MCP parity

**Files:**

- Modify `scripts/build-codex-sidecar.mjs`
- Create `scripts/verify-linux-agent-runtime.mjs`
- Create `scripts/verify-linux-agent-runtime.test.ts`
- Modify `src-tauri/src/codex/app_server.rs`
- Modify `src-tauri/src/settings/agent.rs`
- Modify `src-tauri/src/codex/context.rs`
- Modify `src-tauri/src/bin/video-creater-mcp-server.rs`

**Interfaces and acceptance:**

- Build or select an exact x86_64 Linux Codex executable and static-musl MCP
  executable, then bind both to the runtime manifest and complete source/binary
  dependency closure. If the published Codex artifact fails, rebuild the pinned
  source; do not depend on host Node, shell, Git, or package-manager tools.
- Launch with an empty `PATH` and sanitized environment. The app-server receives
  the mandatory project video skills, proposes structured EDL-first edits, and
  cannot mutate canonical state directly.
- Tests cover initialize/version mismatch, missing/corrupt binaries, valid proposal
  apply/reject, invalid proposal rejection, cancellation, timeout, process-tree
  cleanup, restart recovery, bounded stdout/stderr, and credential redaction.
- Provider-backed live tests are recorded separately from deterministic offline
  protocol fixtures; absence of provider credentials cannot be reported as local
  runtime failure.

### Packet 17: Cross-platform project and end-to-end workflow parity

**Files:**

- Create `src-tauri/tests/linux_full_workflow.rs`
- Create `src-tauri/tests/cross_platform_project_roundtrip.rs`
- Create `scripts/linux-editor-e2e.mjs`
- Create `scripts/linux-editor-e2e.test.ts`
- Modify `src-tauri/src/project/fixtures.rs`

**Interfaces and acceptance:**

- Linux creates, edits, saves, closes, reopens, relinks, previews, transcribes,
  generates an EDL, applies a structured proposal, renders MP4 and WebM, cancels a
  second render, and reopens the outputs from one retained project fixture.
- A fixture written on Mac opens and round-trips on Linux, and the Linux result
  opens and round-trips on Mac without losing unknown capability references,
  originals, source ranges, reports, or jobs.
- Paths cover Unicode, spaces, case sensitivity, symlink denial where required,
  moved project roots, missing media, read-only installation roots, and user-owned
  writable directories.

### Packet 18: Deterministic `.deb`, payload audit, and lifecycle

**Files:**

- Create `scripts/build-linux-release.mjs`
- Create `scripts/build-linux-release.test.ts`
- Create `scripts/verify-linux-package.mjs`
- Create `scripts/verify-linux-package.test.ts`
- Create `packaging/linux/video-creater.desktop`
- Create `packaging/linux/control.template`
- Create `packaging/linux/install-layout.json`
- Modify `package.json`

**Interfaces and acceptance:**

- Install immutable app-owned payload under a private versioned root and desktop
  metadata under standard locations; user data stays under XDG roots.
- The package contains every required executable, runtime input, baseline model,
  font, template, skill, notice, manifest, and SBOM. It does not declare a distro
  package as a substitute for app functionality or install libraries globally.
- Verify every ELF interpreter/`DT_NEEDED`, archive contributor, lazy-loaded file,
  executable trace, absolute path, hash, permission, protocol, notice, and SBOM
  relationship. Zero denied or unclassified items is required.
- Clean-machine tests cover install, desktop launch, offline baseline workflow,
  upgrade, failed-update recovery, uninstall, retained projects, reinstall repair,
  non-ASCII paths, restrictive umask, and read-only installation roots.
- AppImage is added only through a later packet if its exact runtime and payload
  independently pass the same boundary. `.deb` release does not wait on AppImage.

### Packet 19: CI, real-desktop matrix, and release evidence

**Files:**

- Create `.forgejo/workflows/linux-release.yml`
- Create `scripts/verify-linux-release-evidence.mjs`
- Create `scripts/verify-linux-release-evidence.test.ts`
- Create `docs/development/linux-release.md`
- Create `docs/research/2026-09-12-linux-release-evidence.md`
- Modify `README.md`

**Interfaces and acceptance:**

- CI builds from locked inputs, pins every third-party action to a commit SHA, and
  retains runtime manifest, package hashes, SBOM, notices, source revision, test
  reports, media artifacts, actual-load traces, and package lifecycle logs.
- Real installed Ubuntu 24.04 sessions pass on Wayland and X11 using software
  rendering. Record GPU paths separately; a GPU path cannot become the baseline
  until its loaded client/driver closure qualifies.
- Browser fixtures, private compositor tests, installed desktop checks, networked
  provider tests, and Mac checks remain separately labeled evidence.
- README and release docs state the exact supported distribution, architecture,
  display/audio/service protocols, formats/profiles, package hash verification,
  installer footprint, baseline model footprint, and repair behavior.

### Packet 20: macOS regression and release decision

**Files:**

- Modify only files required by fresh findings from the Mac gate.
- Create `docs/research/2026-09-12-linux-macos-regression.md`

**Acceptance:**

- On a Mac, run affected Rust and frontend tests, packaged app launch, current
  GStreamer/AVFoundation/Core ML helper checks, project round trip from Packet 17,
  signing/notarization checks required by the existing release process, and the
  established editor workflow smoke suite.
- Re-run Linux focused tests for any shared-code fix. Mac defaults and behavior
  must remain unchanged.
- Release only when every required Linux and Mac gate is passed, all package items
  are classified, and no Critical or Important review finding remains.

## Executable acceptance matrix

| Layer | Required command/evidence | Pass condition |
| --- | --- | --- |
| Source | `rtk pnpm check:source-quality` and Linux manifest source verification | Locked source, notices, revisions, and selected licenses agree |
| Rust core | `rtk cargo test --manifest-path src-tauri/Cargo.toml --no-default-features -- --test-threads=1` | Canonical project, EDL, proposal, jobs, reports pass without desktop/media defaults |
| Linux host | package-specific locked/offline tests plus retained X11 and Wayland executions | Static-musl lifecycle, input, buffers, scaling, cleanup pass |
| Media | `rtk node scripts/verify-linux-production-media.mjs "$PWD/output/linux-production-media-evidence-attempt1"` | Common MP4, WebM, sync, seeking, graphics, cancellation, performance pass |
| Models | Linux model parity suite with packaged weights and network disabled | Transcription, EDL/captions, speech, enhancement, semantic thresholds pass |
| Agent | `rtk node scripts/verify-linux-agent-runtime.mjs "$PWD/output/linux-package-root-attempt1/opt/video-creater"` | Bundled exact binaries pass protocol, proposal, cancel, restart, redaction checks |
| UI | native scene fixtures and real-desktop workflow | All editor operations, states, shortcuts, focus, scaling, services pass |
| Package | `rtk node scripts/verify-linux-package.mjs "$PWD/output/linux-release/video-creater_0.1.0_amd64.deb" "$PWD/output/linux-package-evidence-attempt1"` | No undeclared runtime lookup; payload, lifecycle, offline workflow pass |
| Full branch | existing frontend/native gates plus Linux release evidence gate | Shared behavior passes; evidence labels remain honest |
| Mac | existing packaged release checks run on a Mac | No inferred Mac pass from Ubuntu; current defaults and workflows pass |

## Genuine external blockers

These conditions cannot be proven solely by implementation on the current Ubuntu
VM. They do not stop unrelated packets.

| Blocker | Blocks | Work that continues |
| --- | --- | --- |
| No real Ubuntu Wayland/X11 desktop session with input, scaling, portal, keyring, and audio devices | Final installed-desktop acceptance for Packets 11-13 and 19 | Protocol tests, headless host, UI fixtures, media/models, package audit |
| No clean supported-machine matrix | Package isolation, upgrade/uninstall, distribution claim | Reproducible package build and container/VM-safe static checks |
| No Mac/Xcode/signing environment | Packet 20 and final shared-code regression verdict | All Linux implementation and evidence |
| No approved release signing identity or publishing authorization | Signing/publishing only | Unsigned deterministic package and full verification |
| No live provider credentials/network authorization | Live Codex/provider evidence | Offline agent protocol, MCP, proposal, cancellation, redaction |
| Unresolved H.264/AAC patent and distribution review | Public MP4 release | Implementation, source-license audit, conformance, internal evidence |
| A candidate model or runtime fails redistribution, permissive closure, or fixed quality thresholds | That local feature and therefore full-parity release | Other model candidates and all independent product work |
| Hardware acceleration loads an unqualified driver/client closure | GPU claim | Required CPU baseline and software-rendered release |

If an external blocker remains at release review, report it as blocked with its
retained evidence. Do not convert it to a pass, replace the dependency with a
forbidden component, or lower the full-parity contract.

## Next dispatch order

1. Packet 1A is complete at reviewed commit `52a800d`; its manifest and resolver
   are now the trusted launch boundary for app-owned workers and packaged artifacts.
2. Packet 2 is running; after review, continue through 2A-2C. This
   turns the accepted exact-binary media proof into a real canonical render
   vertical slice.
3. Dispatch Packet 3 concurrently only in `native/linux-desktop/` and its new
   verifier: it turns the Wayland proof into a real-project Project Home without
   waiting for the full editor.
4. Complete Packet 4, then Packet 5. These are the shared seams that unblock
   independent media, desktop, model, and agent integration.
5. Run media Packets 6-9 and desktop Packets 10-13 as separate reviewed lanes.
6. Run model Packets 14-15 and agent Packet 16 after the resolver and service seam
   are stable.
7. Serialize Packets 17-20 for whole-product integration, packaging, installed
   acceptance, and final cross-platform release review.

The first three packets must use fresh evidence roots and small fixtures. Do not
repeat the expensive combined native build until a changed locked input or the
production media qualification packet requires it.

## Roadmap verification

This roadmap is grounded in the current `ea020cd3` source and accepted reports.
It does not claim that a production Linux backend, native full UI, model runtime,
installed package, real desktop, patent review, or Mac regression has passed.
Those claims become true only at their explicit packet gates.
