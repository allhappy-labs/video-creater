# Expression-Enabled Precompose Platform Design

## Summary

Add a Rust-owned render-preparation platform between canonical timelines and GStreamer Editing Services (GES). GES remains the normal timeline, audio, encoding, and muxing backend. Visual sources or effects that GES cannot express faithfully are prepared into verified, alpha-preserving intermediates before the ordinary render plan is built.

The platform initially supports expression-enabled Lottie and dotLottie sources. It then adds per-source LUT preparation, destination-dependent blend flattening, and optional WGPU acceleration against one canonical CPU frame contract.

The selected dependency stack is source-vendored and immutable:

- `dotlottie-rs` v0.1.58 at `2ce5e48f5786c3e60301d66db4cfdff56c896895` (MIT).
- ThorVG at `73045df5398690eb1c6c0946e0b2301032337776` (MIT).
- The ThorVG-pinned JerryScript revision for expressions (Apache-2.0).
- Existing application `wgpu` for optional GPU composition; upstream `tvg-wg` is not enabled because its build downloads unsigned runtime archives.

No renderer, model, binary, or asset is fetched at application runtime.

## Goals

- Render `.json` and `.lottie` timeline sources, including supported expressions.
- Preserve timeline timing, source ranges, speed, track order, transforms, crop, opacity, fades, keyframes, and audio.
- Keep ordinary supported timelines on GES.
- Introduce one capability planner for Lottie, LUTs, richer blend modes, and future unsupported visual effects.
- Make CPU output deterministic and authoritative.
- Isolate native animation parsing and expression execution from the Tauri process.
- Publish immutable caches atomically with decoded-pixel checksums.
- Preserve alpha through a single GES-readable prepared source instead of one GES clip per PNG frame.
- Retain first, middle, and last frame comparisons for every prepared effect class.
- Enforce permissive dependency licensing with pinned provenance, offline builds, notices, and SBOM evidence.

## Non-Goals

- Replacing GES for ordinary editing, audio, final encoding, or muxing.
- Treating the existing shader-background renderer as a general video compositor.
- Silently approximating unsupported expressions, effects, LUTs, or blend modes.
- Claiming an isolated helper is a complete operating-system sandbox.
- Making GPU output the correctness reference.
- Mutating canonical split-project files during preparation.

## Pipeline

```text
VideoProject
  -> expand nested timelines
  -> evaluate ordered visual stack
  -> capability planner
       -> NativeGes
       -> PreparedSource
       -> FlattenedComposite
       -> Reject
  -> isolated workers + immutable prepared-media cache
  -> ephemeral PreparedTimeline
  -> ordinary RenderPlan
  -> GES render/audio/export
  -> probe, frame comparisons, reports
```

The integration seam is `render_project_media_after_job_started`, after nested timeline expansion and before `build_project_render_plan_with_range`.

## Capability Classes

### `NativeGes`

Use the original source and existing GES implementation for supported media, transform, crop, opacity, fades, speed, keyframes, basic color balance, Normal/Source/Add compositing, and audio.

### `PreparedSource`

Use when one source can be prepared independently of destination pixels:

- Lottie/dotLottie animation.
- Per-source `.cube` LUT.
- Source-local shader/effect whose output depends only on that source.

Preparation replaces only the ephemeral source path and source range. The original timeline start, duration, track index, transforms, opacity, fades, keyframes, and higher/lower layer relationships stay intact.

### `FlattenedComposite`

Use when an effect depends on already-composited destination pixels, including Multiply, Screen, Overlay, and similar blend families.

The planner finds the minimal connected time interval and lower-layer dependency stack, splits participating clips at interval boundaries, renders the bottom-through-effect result, replaces that interval with one prepared source at the highest participating track, and leaves higher tracks plus audio untouched.

### `Reject`

Unknown or unimplemented semantics fail closed with item/media IDs and an actionable error. The renderer never drops an expression, LUT, blend, or visual layer and never substitutes untreated footage.

## Module Layout

```text
src-tauri/src/precompose/
  mod.rs
  capabilities.rs
  plan.rs
  cache.rs
  worker_client.rs
  intermediate.rs
  report.rs
  qa.rs

src-tauri/src/frame_compositor/
  mod.rs
  semantics.rs
  program.rs
  blend.rs
  lut.rs
  cpu.rs
  gpu.rs
  planner.rs

src-tauri/crates/precompose-protocol/
  src/lib.rs

src-tauri/crates/precompose-worker/
  src/main.rs
  src/lottie.rs
  src/compositor.rs
  src/limits.rs

src-tauri/vendor/dotlottie-rs/
src-tauri/vendor/thorvg/
```

Only `precompose-worker` links dotLottie, ThorVG, and JerryScript. Shared protocol types have no native renderer dependency.

## Worker Isolation And Expressions

Expressions are enabled with `tvg-lottie-expressions`.

The worker contract is one versioned JSON request and NDJSON progress/final responses. The worker receives staged bounded inputs and writes only to a unique staging directory. It receives no network, proxy, credential, or project-authority environment variables.

The parent process:

- prevalidates JSON and dotLottie archives;
- stages only approved local files;
- sets wall-clock, frame, pixel, output-byte, archive, and log budgets;
- terminates the worker process group on timeout or cancellation;
- validates every output before publication;
- retains bounded failure diagnostics under the render job;
- never publishes a successful cache after a crash, timeout, cancellation, or budget failure.

The worker exposes no host filesystem or network APIs to expressions. Native parser or expression faults therefore terminate the helper, not the editor. This is a stability and authority boundary, not a claim of full OS sandboxing.

Expression failures distinguish evaluation error, unsupported expression feature, resource budget exceeded, and nondeterministic output. Animations are not rejected merely because they contain expressions.

## Input Safety

Before calling dotLottie, reject:

- absolute paths or archive traversal;
- external URLs or network assets;
- excessive source bytes;
- excessive archive entry count;
- excessive per-entry or aggregate expanded bytes;
- excessive compression ratio;
- excessive images/fonts/assets;
- invalid dimensions, FPS, duration, or selected animation ID;
- excessive frame count, total decoded pixels, or estimated output bytes.

Every embedded asset contributing pixels is included in the source fingerprint.

## Canonical Pixel And Timing Contract

CPU rendering is authoritative:

- output frames: PNG containing straight-alpha RGBA8;
- color space: versioned sRGB boundary;
- internal frame composition: linear-light premultiplied alpha;
- time interval: half-open `[start, end)`;
- frame `i` sample time: rational `start + i / fps` under a versioned sampling policy;
- fixed transform order, resampling, edge handling, alpha normalization, and rounding;
- explicit loop, direction, source-range, speed, selected-animation, theme, slot, and expression policies.

GPU consumes the same serialized frame program. GPU output is accepted only within documented decoded-pixel thresholds against CPU fixtures. CPU remains available as fallback and is recorded as the reference in reports.

## Cache

Project-local immutable layout:

```text
cache/precompose/v1/sha256/ab/<fingerprint>/
  manifest.json
  frames/frame-000000.png
  intermediate.mov
  worker-report.json
  qa-report.json
  checksums.json
```

The fingerprint is canonical JSON covering:

- source and embedded asset/font SHA-256 hashes;
- selected animation/theme/slots;
- source range, speed, loop, direction, dimensions, and rational FPS;
- expressions-enabled policy and budgets;
- dotLottie, ThorVG, and JerryScript revisions;
- frame-compositor and bake-policy versions;
- alpha, color-space, transform, and resampling contracts;
- canonical LUT/effect/blend program;
- worker build identity and initially target triple.

Publication uses a sibling UUID staging directory, per-key lock, manifest-last write, complete checksum/dimension/frame validation, and atomic rename. Cache hits revalidate every referenced file, decoded dimensions, decoded RGBA checksum, file checksum, intermediate probe, and manifest contract.

## Alpha Intermediate

Canonical cache output remains PNG frames. A parent-owned packaging adapter creates a lossless PNG-in-QuickTime MOV using reviewed GStreamer factories:

```text
multifilesrc caps=image/png,framerate=...
  -> qtmux
  -> filesink
```

GES consumes one MOV source at the original track index. This avoids the existing 600-frame/one-clip-per-frame path. The transport is unavailable until an integration fixture proves alpha survives PNG encode, MOV mux, MOV decode, GES composition, and final output at first/middle/last frames.

## Lottie Metadata

Import and preparation record:

- available animation IDs and selected animation;
- source dimensions, in/out frames, frame rate, and duration;
- embedded image/font inventory;
- expression presence;
- themes, slots, markers, and state-machine identifiers when present.

Multi-animation dotLottie files require an explicit selected animation unless the manifest defines an unambiguous initial animation.

## LUTs

The compositor accepts bounded `.cube` LUTs after strict parsing:

- supported dimensions and domain bounds are explicit;
- non-finite or excessive entries fail closed;
- interpolation and color-space behavior are versioned;
- identity and known-vector fixtures are exact CPU gates.

Per-source LUTs use `PreparedSource`. A LUT applied after destination-dependent composition uses `FlattenedComposite`.

## Blend Modes

CPU implements explicit, tested equations for each supported blend mode under the canonical alpha/color contract. Multiply, Screen, and Overlay are the first richer modes. Each mode requires known pixel-vector tests and multi-layer timeline fixtures before it is exposed.

## Temporal And Local Rendering

Local and Temporal paths call the same preparation API. Temporal activities separate planning, materialization, prepared render-plan construction, rendering, validation, and reporting. Worker polling heartbeats, cancellation terminates descendants, retries hit a verified immutable cache, and provider-input range renders use the same preparation path.

## Reporting

Every prepared artifact records:

- capability class and reason;
- source item/media IDs;
- cache hit/miss/rebuild status;
- source and cache fingerprints;
- renderer/backend/revision identities;
- expression status and budgets;
- resource usage and timings;
- manifest, log, checksum, frame, intermediate, and QA paths;
- first/middle/last comparison metrics;
- GPU fallback or tolerance evidence.

All paths are project-relative. Render reports include preparation artifacts in their main artifact list.

## License And Supply-Chain Policy

- Vendor exact source revisions and record source archive SHA-256.
- Include upstream license files and a generated third-party notice/SBOM.
- Reject GPL, AGPL, SSPL, BSL, Commons-Clause, or unknown/unlicensed runtime dependencies.
- dotLottie and ThorVG are MIT; JerryScript is Apache-2.0.
- Remove or feature-gate upstream's unused MPL-2.0 `cbindgen` build dependency because the C API is disabled.
- Do not ship the unverified bundled fallback font until its exact DM Sans subset provenance and OFL-1.1 notice are recorded; embedded/project fonts require explicit local provenance.
- Do not enable upstream `tvg-wg` because its build fetches unsigned archives. GPU work uses the application's pinned `wgpu` dependency.
- CI builds vendored dependencies offline and fails on revision, hash, license, or notice drift.
- Customer installations require no compiler, libclang, build tool, or network fetch; native code is compiled and signed during release builds.

## Required Verification

- Minimal JSON and dotLottie fixtures.
- Multi-animation selection and malformed/unsafe archive failures.
- Embedded images, masks, alpha edges, and approved fonts.
- Expressions: `time`, `value`, property references, `wiggle`, and `seedRandom`.
- Pathological expression timeout/resource-budget fixture; editor survives.
- Two clean CPU bakes produce identical decoded RGBA hashes and manifests.
- Cache-key sensitivity, corruption, concurrency, cancellation, and atomic publication.
- PNG-MOV alpha survival through GES.
- Track-order fixture: lower video, middle Lottie, higher source/graphic.
- Trim, speed, loop, transform, crop, opacity, fades, and keyframes around prepared sources.
- Identity and nonidentity LUT fixtures.
- Golden pixel and real multi-layer fixtures for each richer blend mode.
- CPU/GPU comparison and CPU fallback.
- Temporal retry/cancellation and provider-input range integration.
- Retained first/middle/last expected, actual, diff, metrics, manifests, logs, and reports.
- Packaged helper discovery/signing and offline release builds per supported target.

## Rollout

The platform is implemented as one architecture with gated vertical slices: provenance/protocol, expression-enabled Lottie CPU bake, cache/intermediate/GES source replacement, LUTs, flattened blends, WGPU acceleration, and packaged release matrix. A later slice may ship only after its own correctness and retained-evidence gates pass; no slice introduces a competing architecture.
