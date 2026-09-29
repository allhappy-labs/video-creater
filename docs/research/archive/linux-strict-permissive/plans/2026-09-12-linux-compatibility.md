# Linux compatibility implementation plan

Status: historical proposal; implementation has not started. The
[Linux bundled-runtime specification](../specs/2026-09-12-linux-compatibility-design.md)
supersedes this plan's GStreamer/GES bundling assumption and defines the user's
new permissive-only requirement. Do not execute this plan unchanged.
The user subsequently confirmed that LGPL system libraries are also prohibited;
the standard Tauri Linux host and glibc path require replacement, not porting.
Repository baseline: `1f6ff8e7`, inspected 2026-09-12.

## Target and scope

Start with Ubuntu 24.04 LTS on x86_64, with native Tauri operation under both
Wayland and X11. This is a proposed support baseline, not a compatibility claim.
Ship a `.deb` first; add AppImage after the native runtime is reproducible.
Defer ARM64, RPM, Flatpak, and additional distributions until this baseline passes.
Keep the current macOS implementation and its release gates working.

Deliver two explicit milestones:

- **Linux editor preview:** install, open/save/reopen projects, import supported
  media, edit and preview a timeline, render WebM, and use bundled agent/MCP tools.
  Unsupported features are disabled with an accurate explanation.
- **Linux product release:** add local word-timed transcription and EDL generation,
  common H.264/AAC input, a validated MP4 delivery path, secure provider credentials,
  and packaged desktop acceptance. Speech analysis, enhancement, semantic search,
  HEVC, and ProRes require individual parity decisions and must be documented if
  still unavailable. Do not describe the first milestone as full feature parity.

## Findings from the current checkout

| Area | Evidence | Work required |
| --- | --- | --- |
| Entrypoints | `package.json` and `src-tauri/tauri.conf.json` run Apple helpers and `build-macos-release.mjs` in shared dev/build hooks | Platform-aware preparation and bundle configuration |
| Runtime | `scripts/build-gstreamer-runtime.mjs` accepts only `aarch64-apple-darwin` | Linux GStreamer/GES runtime, dependency manifest, and loader verification |
| Sidecars | Compatibility decoder and release precompose scripts restrict packaging to Apple Silicon; Codex script maps Darwin packages | Native Linux artifacts and target-aware resolution for each shipped helper |
| Models | FluidAudio, audio enhancement, and semantic encoder build scripts target Apple Silicon/Core ML | Portable backends or explicit unavailable capabilities |
| Export | `src-tauri/src/render_pipeline/project_export.rs` and `settings/render_system.rs` contain AVFoundation delivery assumptions | Linux delivery selection and capability-based readiness |
| Credentials | `src-tauri/src/provider_credentials.rs` implements macOS storage; other platforms cannot save/delete | Linux secure desktop credential backend |
| Desktop | `src-tauri/src/main.rs` rejects non-macOS notifications and Finder reveal; Settings copy names Keychain/Finder | Native Linux services and platform-appropriate labels |
| Verification | Existing browser Linux visual baseline is a fixture; native/release scripts carry Darwin defaults | Separate Linux native and package evidence |

GStreamer/GES remains the composition engine. Preserve Rust ownership of project
state, typed proposal validation, cancellation, artifacts, and logs. Do not bypass
these boundaries to make Linux work.

## 1. Prove the difficult dependencies before porting the whole app

- Inventory every required shared library, plugin, sidecar, model format, and
  platform assumption, including compatibility-worker, precompose-worker, bundled
  Codex/MCP, ThorVG, font rendering, GPU frame paths, and Temporal dependencies.
- Define explicit Linux Cargo features; check the application and workspace with
  those features. Do not assume target-gated Core ML dependencies are compile
  errors merely because the default feature list includes `coreml-inspect`.
- On the proposed baseline, prototype real GES decode, seek, composition, and
  WebM output using tiny fixtures. Exercise CPU and available GPU paths separately.
- Resolve common H.264/AAC input and MP4 output early. The current
  `scripts/gstreamer-runtime-policy.mjs` excludes libav/FFmpeg, x264/x265,
  OpenH264, and other factories. Preserve that policy unless an explicit reviewed
  policy change is adopted; do not quietly depend on the host's plugin inventory.
- Record a concrete codec/backend decision, runtime provenance, allowed factories,
  and tested hardware requirements. Hardware-only MP4 must be advertised as such;
  it cannot stand in for a general CPU-supported release.

Exit: a dependency/capability matrix and reproducible media spike. If common input
or delivery remains blocked, keep the first milestone explicitly limited and
resolve the product-release blocker before promising general compatibility.

## 2. Introduce platform-aware build and runtime capabilities

- Extract shared preparation from `scripts/build-macos-release.mjs`; dispatch by
  an explicit validated target triple rather than defaulting silently to Darwin.
- Separate common Tauri configuration from macOS/Linux bundle settings and helper
  lists. Ensure config merging cannot retain unavailable Apple binaries on Linux.
- Port supported sidecar builders to `x86_64-unknown-linux-gnu`; preserve checksums,
  provenance, executable permissions, protocol compatibility, and version checks.
- Build a reviewed Linux GStreamer/GES and compatibility runtime. Validate ELF
  dependencies, relative loader paths, plugin scanner execution, plugin registry
  isolation, and interactions with WebKitGTK's media dependencies. Never bundle
  an arbitrary development machine's library directory.
- Extend the existing health model with backend/capability availability and reasons.
  Keep platform knowledge in Rust and build tooling, with UI consuming typed
  capabilities. Distinguish unavailable, missing, failed, and ready components.

Exit: the normal development command opens the real Linux desktop app; no Swift,
Xcode, AVFoundation, or macOS release helper is required on its execution path.

## 3. Complete the Linux editing and media path

- Adapt `render_runtime.rs`, precompose frame decoding/intermediates, compatibility
  decoding, export profiles, and render health to the selected Linux backends.
- Verify thumbnails, filmstrips, scrubbing, playback/audio sync, cuts, effects,
  precomposed graphics, transparent layers, and cancellation with real media.
- Use GES for the initial WebM path. Add the approved Linux delivery backend and
  capability-filter export choices; absence of AVFoundation alone must not make
  an otherwise ready Linux runtime report failure.
- Verify project round-trips between platforms, relative media paths, relinking,
  Unicode/spaces, case-sensitive paths, and unavailable effect/model references.
  Preserve originals and project data when a capability is missing.

Exit: a packaged candidate can import, edit, save/reopen, preview, and export a
small project; output checks cover duration, actual streams, frame content, audio
sync, layer timing, cancellation cleanup, artifact paths, and retained logs.

## 4. Port local intelligence without changing edit semantics

- Select and benchmark a portable transcription backend against the current
  model/job boundary. Evaluate accuracy, word timestamps, language coverage,
  memory, latency, model redistribution, and a CPU baseline before choosing it.
- Preserve verified downloads, hashes, model provenance, progress, cancellation,
  retries, and restart recovery. Keep models isolated by backend/version.
- Route transcription through the existing Rust-owned pipeline. Generated edits
  must select real `sourceIn`/`sourceOut` ranges before captions and visual layers;
  validate timestamp remapping after cuts using a known speech fixture.
- Evaluate separate portable implementations for speech analysis, DeepFilterNet
  enhancement, and semantic encoding. Prevent incompatible embedding/index reuse
  across model versions. Mark each unavailable feature honestly until validated.
- Verify bundled agent and MCP startup, structured proposal rejection/application,
  cancellation, and restart without relying on a developer-installed executable.

Exit: local transcription produces word timestamps and a genuine edited timeline
on Linux, with explicit per-feature parity and benchmark evidence.

## 5. Finish desktop integration and capability-driven UI

- Implement Linux credential storage behind the existing `CredentialStore`
  boundary using a reviewed Secret Service integration. Test locked/unavailable
  services, save/replace/delete/restart, and an isolated test namespace. Never
  substitute plaintext settings storage or return saved secrets to React.
- Implement file-manager reveal/open, native dialogs, notifications, and XDG
  storage paths. Reuse existing Linux storage handling where it already applies.
- Replace macOS-specific Settings wording and platform shortcut labels. Validate
  Ctrl shortcuts, focus, drag/drop, clipboard, file URLs, and asset-protocol/CSP
  behavior in the actual WebKitGTK application.
- Test display scaling, fonts, reduced motion, preview surfaces, audio devices,
  Wayland and X11. Keep capabilities accurate when optional services are absent.

Exit: real desktop interactions pass and missing optional services do not block
basic editing. Provider credential acceptance includes a real secure-store test.

## 6. Package, gate, and document support

- Add Linux preflight/build/verification commands alongside macOS commands.
  Declare supported system dependencies and app-owned runtime contents explicitly.
- Produce `.deb` with desktop metadata/icons and dependency declarations. Test
  install, launch from the desktop, upgrade, uninstall, and retained user projects.
- Add AppImage only after clean-machine loader/media checks pass; build against
  the oldest supported base, then verify each claimed distribution separately.
- Add CI for frontend checks, Linux Rust checks, sidecar/runtime policy tests,
  real media integration, and package smoke tests. Pin third-party actions to
  immutable SHAs; retain checksums, source revision, manifests, logs, and reports.
- Keep browser fixture, headless media, native desktop, and installed-package
  evidence distinct. Run actual desktop acceptance under both display systems;
  headless success is insufficient for preview, dialogs, keyring, or GPU claims.
- Update README, development/runtime documentation, Settings readiness, and the
  canonical product backlog with supported formats, distributions, hardware
  requirements, installation instructions, and remaining parity gaps.
- Run existing macOS regression/release checks on a Mac before merging shared
  platform changes. Ubuntu evidence cannot validate macOS packaging or signing.

Exit: clean Ubuntu machines can complete the published workflow without development
tools or undeclared runtime dependencies. Release evidence identifies passed,
blocked, partial, and unverified checks; no unsupported feature is reported ready.

## Suggested delivery order

Use reviewable changes in this order: dependency/codec spike; target dispatch and
capabilities; Linux runtime/sidecars; editing and export; desktop integration;
local transcription; remaining model parity; packaging and acceptance gates.
Keep each slice covered by focused tests; run broad gates once integrated.
Estimate effort after the codec and portable-model spikes, which dominate the
uncertainty. Do not make shipping dates depend solely on the app compiling.

## Verification of this plan

This is a source-based plan. No application build, native launch, media render,
package installation, or feature-parity test was run while preparing it.
The existing untracked backend architecture review was left untouched.

Tauri's official prerequisites specify Linux system dependencies including
WebKitGTK 4.1: [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).
Its packaging guidance recommends building AppImages on the oldest supported
base: [Tauri AppImage distribution](https://v2.tauri.app/distribute/appimage/).
Exact packages and ABI compatibility must be checked during the dependency spike.
