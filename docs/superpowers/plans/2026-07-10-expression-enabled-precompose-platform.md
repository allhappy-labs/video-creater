# Expression-Enabled Precompose Platform Implementation Plan

> **For agentic workers:** Follow the repository feature-development and video-pipeline guidance. Keep every slice fail-closed, test-first, and evidence-backed.

**Goal:** Implement the complete render-preparation platform described in `docs/superpowers/specs/2026-07-10-expression-enabled-precompose-platform-design.md`, including expression-enabled Lottie, deterministic caches, PNG-MOV prepared sources, LUTs, richer blend flattening, optional WGPU acceleration, and retained comparison fixtures.

**Architecture:** Insert one capability planner after nested timeline expansion and before render-plan construction. Keep GES for native semantics. Materialize unsupported source-local visuals as `PreparedSource` and destination-dependent effects as `FlattenedComposite`. Isolate native Lottie/expression execution in a helper process. CPU is canonical; GPU implements the same frame program.

**Pinned sources:** dotlottie-rs `2ce5e48f5786c3e60301d66db4cfdff56c896895`; ThorVG `73045df5398690eb1c6c0946e0b2301032337776`; their pinned JerryScript revision.

## Task 1: Dependency Provenance And Offline Build Gate

- [x] Vendor pinned dotlottie-rs, ThorVG, JerryScript, and required embedded dependencies without nested VCS state.
- [x] Add `SOURCE.json`, source hashes, upstream licenses, third-party notices, and enabled-feature inventory.
- [x] Replace/remove compiled MPL-2.0 paths, including `cbindgen` and the interpolator.
- [x] Keep `tvg-wg` disabled and use existing application `wgpu` later.
- [x] Disable the unverified fallback-font payload until exact OFL provenance is recorded.
- [x] Add provenance and resolved Cargo-license gates.
- [ ] Add CI/release tests for offline native compilation and helper packaging.

## Task 2: Versioned Precompose Protocol And Helper Skeleton

- [x] Create the versioned request/progress/result/budget/error protocol.
- [x] Create `precompose-worker` as a separate workspace binary.
- [x] Add one-request-per-process NDJSON IPC.
- [x] Strip network/proxy/credential environment variables.
- [x] Restrict all outputs to a supplied staging directory.
- [x] Add parent timeout, process-group termination, and crash reporting.
- [x] Bound worker stdout/stderr and retain truncated diagnostics.
- [x] Configure Tauri sidecar packaging and verify the host-target build.
- [ ] Verify discovery, signing, notarization, and offline packaging on every supported target.

## Task 3: Input Validation And Metadata

- [x] Parse bounded Lottie JSON before native loading.
- [x] Prevalidate dotLottie ZIP entries without extraction.
- [x] Reject traversal, symlinks, external assets, and excessive entries/bytes/ratios/dimensions/frames/pixels.
- [ ] Record animations, selected/default animation, dimensions, FPS, duration, themes, slots, markers, state machines, assets, and expression presence.
- [x] Require explicit animation selection for ambiguous multi-animation archives.
- [ ] Add malicious archive and malformed metadata fixtures.

## Task 4: Expression-Enabled Deterministic CPU Lottie Bake

- [x] Enable `tvg-lottie-expressions` in the worker.
- [x] Implement JSON and dotLottie loading through the pinned player API.
- [x] Define rational frame sampling and source offset/rate semantics.
- [x] Normalize ThorVG output to straight-alpha RGBA8 sRGB PNG.
- [x] Enforce wall-clock, frame, pixel, output-byte, and buffer budgets.
- [x] Add fixtures for `time`, `value`, property references, `wiggle`, and `seedRandom`.
- [x] Surface expression exceptions as typed worker failures.
- [x] Run clean player contexts twice and assert decoded RGBA determinism and expression pixel impact.

## Task 5: Immutable Cache And Integrity Validation

- [x] Implement canonical Lottie cache fingerprints.
- [x] Render into a sibling UUID staging directory under a per-key lock.
- [x] Hash PNG and decoded RGBA bytes per frame.
- [x] Write cache manifest last and atomically publish complete artifacts.
- [x] Revalidate stage identity, frame order, PNG/RGBA hashes, dimensions, counts, and intermediate hashes on cache hits.
- [x] Test corruption/rebuild, duplicate frames, symlink escapes, exact-duration keys, abandoned locks, and staging cleanup.
- [ ] Validate preview/worker-report contracts and test concurrent publication plus cancellation during packaging.

## Task 6: Prepared-Source Intermediate And GES Integration

- [x] Package timestamped PNG buffers with `appsrc` and `qtmux`, without `pngparse`.
- [x] Decode the MOV fixture and verify retained alpha.
- [x] Add distinct first/middle/last alpha-survival and layer-order fixtures through GES.
- [x] Introduce an ephemeral prepared project without mutating canonical project files.
- [x] Replace Lottie source paths/ranges while preserving item placement and properties.
- [x] Add a sequential long-duration Lottie fixture beyond 600 frames.
- [x] Add a three-layer prepared-source z-order fixture.

## Task 7: Capability Planner And Render Orchestration

- [x] Add native/prepared-source/flattened-composite capability decisions.
- [x] Insert preparation after nested expansion and before render-plan construction.
- [ ] Route local, Temporal, provider-input, timeline-scoped, and range renders through the same preparation API.
- [ ] Add idempotent preparation activities, heartbeats, cancellation, retry/cache behavior, and typed failures.
- [ ] Extend render reports/performance stages with preparation evidence.

## Task 8: Canonical Frame-Compositor IR And CPU Backend

- [ ] Define ordered source inputs, canvas, transforms, crop, opacity, alpha, blend, LUT, sampling, and color-space semantics.
- [x] Serialize and execute deterministic transforms, crop, opacity, fades, and keyframes through a shared frame program.
- [x] Implement linear-light premultiplied-alpha CPU composition and straight-alpha output.
- [x] Add exact pixel vectors for Over/Source/Add/Multiply/Screen/Overlay and alpha edges.
- [x] Add deterministic source decoding and interval sampling, including reviewed H.264 VideoToolbox decode.

## Task 9: LUT Prepared Sources

- [x] Add bounded `.cube` parsing, domain validation, and cache fingerprinting.
- [x] Add canonical `.cube` serialization and use it for cache identity.
- [x] Implement documented interpolation and color-space policy.
- [x] Route clip-local LUTs through `PreparedSource`.
- [x] Add identity, known-vector, real-frame, cache, H.264 decode, and LUT-through-GES fixtures.

## Task 10: Richer Blend Flattening

- [x] Implement active-interval segmentation and lower-layer dependency grouping.
- [x] Implement Multiply, Screen, and Overlay CPU equations under the canonical color/alpha contract.
- [x] Replace only the minimal dependent stack interval with a flattened prepared source.
- [ ] Preserve higher tracks, audio, source ranges, transitions, and boundary timing.
- [x] Add exact pixel vectors plus real multi-layer timeline fixtures for Source, Add, Multiply, Screen, and Overlay.

## Task 11: WGPU Backend And CPU Fallback

- [x] Add a reusable WGPU RGBA-frame blend compositor beside the canonical CPU backend.
- [ ] Serialize transforms, crop, alpha, LUT, and blend operations into one shared frame program.
- [ ] Implement arbitrary texture inputs, transforms, crop, alpha, and LUT kernels in existing pinned `wgpu`.
- [x] Record backend selection/fallback in manifests and reports.
- [x] Gate every GPU blend mode against CPU pixels with exact alpha and one-byte RGB tolerance.
- [x] Retry CPU on initialization, device, execution, or comparison failure without changing semantics.

## Task 12: Preview And Retained Comparison Evidence

- [x] Replace browser approximations for Lottie, LUTs, and richer blends with the ephemeral prepared project.
- [ ] Retain expected, CPU, GPU, GES, diff, and metric artifacts for Lottie, expressions, LUTs, and every richer blend class.
- [ ] Attach comparison evidence to split-project render reports.
- [ ] Run packaged clean-machine matrices per supported platform.

## Task 13: Completion Audit

- [x] Run focused protocol, worker, cache, compositor, GES, runtime, provenance, and license suites.
- [ ] Run full Rust/TypeScript/lint/format gates.
- [ ] Verify offline builds, licenses, notices, source hashes, SBOM, helper signing, and no runtime downloads.
- [x] Verify canonical project objects are unchanged by preparation.
- [x] Update `docs/parity.md` with the verified Lottie slice.
- [ ] Audit every requirement in the design and this plan before marking complete.
