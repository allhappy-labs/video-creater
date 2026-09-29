# Required Render Runtime and General Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make GStreamer/GES required, bundled production infrastructure and replace General’s placeholder runtime/update controls with truthful operational health.

**Architecture:** A deterministic build script stages an allowlisted macOS runtime, rewrites Mach-O dependencies, emits a signed manifest, and gives the renderer and health probe the same environment. Rust reports GStreamer/GES, AVFoundation, and compatibility decoder separately through the shared Settings snapshot.

**Tech Stack:** Rust GStreamer/GES bindings, Node.js build tooling, Mach-O tools, Tauri 2 bundling, React, Vitest.

## Global Constraints

- Production builds must include the `ges-render` feature.
- Packaged apps must not require Homebrew or `/usr/local` at runtime.
- The allowlist and existing fail-closed factory policy remain authoritative.
- AVFoundation readiness must never mask missing GStreamer/GES readiness.
- Do not add a fake updater endpoint or update toggle.

---

## Task 1: Specify and test the curated runtime manifest

**Files:**

- Create: `scripts/gstreamer-runtime-policy.mjs`
- Create: `scripts/gstreamer-runtime-policy.test.ts`
- Create: `scripts/build-gstreamer-runtime.mjs`
- Create: `scripts/verify-gstreamer-runtime.mjs`
- Modify: `package.json`

- [ ] Write failing policy tests for:

- accepted libraries/plugins;
- denied/unreviewed plugin rejection;
- immutable SHA-256 manifest entries;
- rejection of absolute Homebrew and `/usr/local` dependencies;
- rejection when `gst-plugin-scanner`, GES, or a required factory is absent.

- [ ] Define explicit exports:

```js
export const requiredLibraries = [
  "libgstreamer-1.0.0.dylib",
  "libgstbase-1.0.0.dylib",
  "libgstapp-1.0.0.dylib",
  "libgstvideo-1.0.0.dylib",
  "libgstaudio-1.0.0.dylib",
  "libgstpbutils-1.0.0.dylib",
  "libges-1.0.0.dylib",
];

export const requiredFactories = [
  "appsink", "decodebin", "qtdemux", "matroskademux", "pngdec",
  "h264parse", "h265parse", "vtdec", "vp8dec", "vp9dec",
  "videoconvert", "videoscale", "videorate", "videocrop", "capsfilter",
  "mp4mux", "qtmux", "aacparse", "atenc", "vtenc_h264",
  "vtenc_h265", "vtenc_prores", "webmmux", "vp8enc", "vp9enc",
  "opusenc", "opusdec",
];
```

Generate the final union from `scripts/check-native-runtime.mjs`, output-profile
factories, precompose requirements, and GES initialization requirements. Every
entry must also pass `evaluate_gstreamer_factory`; add a factory only when a
real render test proves it is required.

- [ ] Implement staging under
`src-tauri/resources/render-runtime/{lib,plugins,libexec,licenses}` and write
`manifest.json` with schema version, GStreamer/GES versions, source root,
relative path, SHA-256, license, and Mach-O dependency list for every file.

- [ ] Rewrite each staged dylib ID to `@rpath/${basename(path)}` and its
dependencies to `@loader_path`-relative locations. The verifier must run
`otool -L` over every Mach-O and reject external package-manager paths.

- [ ] Add scripts:

```json
"build:gstreamer-runtime": "node scripts/build-gstreamer-runtime.mjs",
"build:gstreamer-runtime:dev": "node scripts/build-gstreamer-runtime.mjs --development",
"verify:gstreamer-runtime": "node scripts/verify-gstreamer-runtime.mjs --target aarch64-apple-darwin"
```

- [ ] Run policy tests and the verifier against a deliberately incomplete
fixture, then the actual staged runtime.

- [ ] Commit:

```bash
rtk git add scripts package.json
rtk git commit -m "build(render): define curated gstreamer runtime"
```

## Task 2: Bundle the runtime and enable GES in release builds

**Files:**

- Modify: `src-tauri/tauri.conf.json`
- Modify: `scripts/build-macos-release.mjs`
- Modify: `package.json`
- Create: `scripts/rewrite-gstreamer-rpaths.mjs`
- Test: `scripts/gstreamer-release-policy.test.ts`

- [ ] Write a failing test that parses the release command and asserts
`ges-render` is present.

- [ ] Change the Tauri build feature list to:

```text
app-runtime,coreml-inspect,ges-render,gpu-render,graphics-render,temporal-worker
```

- [ ] Run `build:gstreamer-runtime` from `beforeDevCommand` and
`beforeBuildCommand` before compiling Rust.

- [ ] Add the staged directory as:

```json
"resources": {
  "resources/render-runtime/": "render-runtime/",
  "resources/compatibility-runtime/": "compatibility-runtime/",
  "resources/sample-project/": "sample-project/"
}
```

- [ ] Ensure the app binary and nested dylibs resolve runtime libraries from
`@executable_path/../Resources/render-runtime/lib` before signing. The release
script must fail if any post-sign mutation would be required.

- [ ] Extend release evidence with runtime manifest hash, factory probe result,
and the list of nested signed Mach-O files.

- [ ] Run:

```bash
rtk pnpm test -- scripts/gstreamer-release-policy.test.ts
rtk pnpm verify:gstreamer-runtime
rtk cargo check --manifest-path src-tauri/Cargo.toml --no-default-features --features app-runtime,coreml-inspect,ges-render,gpu-render,graphics-render,temporal-worker
```

- [ ] Commit:

```bash
rtk git add src-tauri/tauri.conf.json scripts package.json
rtk git commit -m "build(render): require ges in macos releases"
```

## Task 3: Centralize render-runtime resolution

**Files:**

- Create: `src-tauri/src/render_runtime.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/render_pipeline/backend.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Modify: `src-tauri/src/workflows/mod.rs`
- Test: `src-tauri/src/render_runtime.rs`

- [ ] Write failing tests for bundled-first and development fallback resolution.

- [ ] Implement:

```rust
pub struct RenderRuntimeEnvironment {
    pub source: RenderRuntimeSource,
    pub root: PathBuf,
    pub plugin_path: PathBuf,
    pub scanner_path: PathBuf,
    pub manifest_path: PathBuf,
}

pub enum RenderRuntimeSource {
    Bundled,
    DevelopmentConfigured,
    DevelopmentSystem,
}
```

`resolve_render_runtime` must prefer the Tauri resource directory, allow
development overrides only in debug builds, and produce one environment used by
both probes and render jobs.

- [ ] Initialize GStreamer exactly once after applying:

- `GST_PLUGIN_SYSTEM_PATH_1_0` to the empty string;
- `GST_PLUGIN_PATH_1_0` to `environment.plugin_path`;
- `GST_PLUGIN_SCANNER` to `environment.scanner_path`;
- a registry path inside application support keyed by manifest hash.

- [ ] Replace scattered initialization in proposal, export, provider-input,
discovery, and compatibility paths with the shared resolver.

- [ ] Run all render-pipeline tests with `ges-render`.

- [ ] Commit:

```bash
rtk git add src-tauri/src/render_runtime.rs src-tauri/src/lib.rs src-tauri/src/render_pipeline src-tauri/src/workflows
rtk git commit -m "refactor(render): centralize runtime resolution"
```

## Task 4: Add separate render-system health

**Files:**

- Create: `src-tauri/src/settings/render_system.rs`
- Modify: `src-tauri/src/settings/health.rs`
- Modify: `src-tauri/src/main.rs`
- Test: `src-tauri/src/settings/render_system.rs`

- [ ] Write failing tests for:

1. GStreamer failed + AVFoundation ready => overall action required.
2. GStreamer ready + AVFoundation failed => composition ready, native delivery degraded.
3. Missing denied factory reports policy failure and the exact factory.
4. Probe timeout returns `Failed`, not a hung command.

- [ ] Implement `RenderSystemHealth` items:

- `render.gstreamerGes`;
- `render.pluginPolicy`;
- `render.avfoundation`;
- `render.compatibilityDecoder`.

Provenance includes runtime source, version, manifest hash, plugin count,
executable path, supported profiles, and last check time.

- [ ] Add bounded commands:

```rust
get_render_system_health()
check_render_system()
```

`check_render_system` is a shared Settings operation and must exercise the same
registry and environment as a render.

- [ ] Commit:

```bash
rtk git add src-tauri/src/settings src-tauri/src/main.rs
rtk git commit -m "feat(settings): report render system health"
```

## Task 5: Replace General placeholders

**Files:**

- Create: `src/components/settings/general-settings.tsx`
- Create: `src/components/settings/general-settings.test.tsx`
- Create: `src/lib/settings/render-system.ts`
- Modify: `src/lib/app-settings.ts`
- Modify: `src/lib/app-settings.test.ts`
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src-tauri/src/main.rs`

- [ ] Write failing frontend tests asserting:

- GStreamer/GES is labeled required composition runtime.
- AVFoundation is labeled final-delivery exporter.
- `Check render system` starts a real operation.
- no `Copy repair command` or `Copy verify command` exists.
- installed version and “Updates are not configured for this build” render.
- no update-policy selector renders.
- notifications are disabled when native permission/delivery is unavailable.

- [ ] Remove `UpdatePolicy` and `updatePolicy` from persisted preferences. Keep
the v1 storage key readable and ignore the legacy field without rewriting other
preferences.

- [ ] Remove the App mount effect and menu path that call the stub
`get_app_update_status`. Keep the native menu command, but route it to General
with the update capability row focused.

- [ ] Replace the stub with a truthful health item:

```rust
UpdateHealth {
    state: SettingsHealthState::Unavailable,
    installed_version: env!("CARGO_PKG_VERSION").to_string(),
    summary: "Updates are not configured for this build".to_string(),
}
```

- [ ] Add a notification capability command that reports permission/delivery
availability. Do not request permission until the user clicks `Enable
notifications`.

- [ ] Render flat sections for Privacy, Notifications, Updates, and Render
System using shared status/diagnostics components.

- [ ] Run frontend tests and `rtk pnpm lint`.

- [ ] Commit:

```bash
rtk git add src/components/settings/general-settings* src/lib src/App.tsx src/App.test.tsx src-tauri/src/main.rs
rtk git commit -m "feat(settings): replace general placeholders"
```

## Task 6: Real packaged-runtime evidence

- [ ] Build a local packaged app with the release feature set.

- [ ] On a test environment without Homebrew paths in `PATH`, prove:

1. app launch succeeds;
2. Settings reports bundled GStreamer/GES;
3. a selected-range draft with video and audio succeeds;
4. a graphics-layer draft succeeds;
5. WebM export succeeds;
6. H.264 final export selects AVFoundation when supported;
7. `otool -L` finds no Homebrew or `/usr/local` dependency.

- [ ] Store reports under `output/settings-readiness/render-runtime/`.

- [ ] Run:

```bash
rtk pnpm verify
rtk pnpm verify:gstreamer-runtime
```
