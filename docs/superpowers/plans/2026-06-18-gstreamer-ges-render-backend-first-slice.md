# GStreamer GES Render Backend First Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first testable GStreamer/GES migration slice: LGPL-compatible plugin policy, a GES backend feature gate, and a GES backend skeleton that fails clearly when the feature is disabled.

**Architecture:** Keep the existing ffmpeg backend working while introducing GStreamer/GES as an explicit backend boundary. The first slice does not render media through GES yet; it establishes policy, dependency gating, and actionable backend errors so the next plan can add GES probe and render execution against an installed GES system library.

**Tech Stack:** Rust 1.94 locally, project Rust edition 2021, optional `gstreamer`/`gstreamer-pbutils`/`gstreamer-editing-services` crates behind `ges-render`, existing `serde`/`thiserror`/render pipeline modules, Cargo tests.

---

## Scope

This plan implements the first migration milestone from `docs/superpowers/specs/2026-06-18-gstreamer-ges-render-backend-design.md`.

In scope:

- plugin policy data structures and tests
- default-deny policy decisions for denied, review-required, allowed, and unknown factories
- `GstreamerGesRenderBackend` disabled-feature skeleton
- `ges-render` Cargo feature and optional GStreamer Rust dependencies
- tests that pass without local `gstreamer-editing-services-1.0`

Out of scope for this plan:

- real GES timeline construction
- GStreamer probing/discovery
- GStreamer media render execution
- overlay composition through appsrc/compositor
- removal of ffmpeg fallback

Those items require a system GES install and should be implemented in the next plan after this foundation is green.

## File Structure

- Create `src-tauri/src/render_pipeline/plugin_policy.rs`
  - Owns GStreamer factory metadata, policy verdicts, and default-deny evaluation.
- Create `src-tauri/src/render_pipeline/gstreamer_backend.rs`
  - Owns the GES backend type and disabled-feature actionable error.
- Modify `src-tauri/src/render_pipeline/mod.rs`
  - Exports new modules.
- Modify `src-tauri/src/render_pipeline/error.rs`
  - Adds `RenderBackendPolicyDenied` so licensing failures are first-class.
- Modify `src-tauri/Cargo.toml`
  - Adds `ges-render` feature and optional GStreamer Rust dependencies.
- Modify `src-tauri/tests/render_pipeline.rs`
  - Adds focused tests for policy and disabled backend behavior.

## Task 1: Add Plugin Policy Types And Tests

**Files:**
- Create: `src-tauri/src/render_pipeline/plugin_policy.rs`
- Modify: `src-tauri/src/render_pipeline/mod.rs`
- Modify: `src-tauri/src/render_pipeline/error.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing tests**

Add this import block near the existing render pipeline imports in `src-tauri/tests/render_pipeline.rs`:

```rust
use video_creater_lib::render_pipeline::plugin_policy::{
    evaluate_gstreamer_factory, GstFactoryInfo, PluginPolicyVerdict,
};
```

Add these tests near the other render backend tests:

```rust
#[test]
fn gstreamer_plugin_policy_denies_known_non_lgpl_or_libav_factories() {
    for factory in [
        GstFactoryInfo::new("x264enc")
            .plugin_name("x264")
            .license("GPL"),
        GstFactoryInfo::new("avenc_aac")
            .plugin_name("libav")
            .license("LGPL"),
        GstFactoryInfo::new("avdec_h264")
            .plugin_name("libav")
            .license("LGPL"),
        GstFactoryInfo::new("fdkaacenc")
            .plugin_name("fdkaac")
            .license("nonfree"),
    ] {
        let decision = evaluate_gstreamer_factory(&factory);

        assert_eq!(decision.verdict, PluginPolicyVerdict::Denied);
        assert!(
            decision.fix.contains("approved LGPL-compatible GStreamer element"),
            "fix should explain the approved-plugin policy: {decision:?}"
        );
    }
}

#[test]
fn gstreamer_plugin_policy_marks_bad_plugins_for_review() {
    let factory = GstFactoryInfo::new("openh264enc")
        .plugin_name("openh264")
        .package("GStreamer Bad Plug-ins")
        .license("LGPL");

    let decision = evaluate_gstreamer_factory(&factory);

    assert_eq!(decision.verdict, PluginPolicyVerdict::ReviewRequired);
    assert!(
        decision.reason.contains("requires review"),
        "reason should name review requirement: {decision:?}"
    );
}

#[test]
fn gstreamer_plugin_policy_allows_reviewed_lgpl_core_base_and_good_factories() {
    for factory in [
        GstFactoryInfo::new("filesrc")
            .plugin_name("coreelements")
            .package("GStreamer")
            .license("LGPL"),
        GstFactoryInfo::new("audioconvert")
            .plugin_name("audioconvert")
            .package("GStreamer Base Plug-ins")
            .license("LGPL"),
        GstFactoryInfo::new("mp4mux")
            .plugin_name("isomp4")
            .package("GStreamer Good Plug-ins")
            .license("LGPL"),
    ] {
        let decision = evaluate_gstreamer_factory(&factory);

        assert_eq!(decision.verdict, PluginPolicyVerdict::Allowed);
    }
}

#[test]
fn gstreamer_plugin_policy_denies_unknown_factories_by_default() {
    let factory = GstFactoryInfo::new("mysteryenc")
        .plugin_name("mystery")
        .license("LGPL");

    let decision = evaluate_gstreamer_factory(&factory);

    assert_eq!(decision.verdict, PluginPolicyVerdict::Denied);
    assert!(
        decision.reason.contains("not in the reviewed allowlist"),
        "reason should explain the default-deny allowlist: {decision:?}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gstreamer_plugin_policy
```

Expected: FAIL because `render_pipeline::plugin_policy` does not exist.

- [ ] **Step 3: Add the minimal implementation**

Append this export to `src-tauri/src/render_pipeline/mod.rs`:

```rust
pub mod plugin_policy;
```

Add `RenderBackendPolicyDenied` to `PipelineErrorCode` in `src-tauri/src/render_pipeline/error.rs` immediately after `RenderBackendTimeout`:

```rust
    RenderBackendTimeout,
    RenderBackendPolicyDenied,
    RenderProbeInvalidJson,
```

Create `src-tauri/src/render_pipeline/plugin_policy.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GstFactoryInfo {
    pub name: String,
    pub plugin_name: Option<String>,
    pub package: Option<String>,
    pub license: Option<String>,
}

impl GstFactoryInfo {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            plugin_name: None,
            package: None,
            license: None,
        }
    }

    pub fn plugin_name(mut self, plugin_name: impl Into<String>) -> Self {
        self.plugin_name = Some(plugin_name.into());
        self
    }

    pub fn package(mut self, package: impl Into<String>) -> Self {
        self.package = Some(package.into());
        self
    }

    pub fn license(mut self, license: impl Into<String>) -> Self {
        self.license = Some(license.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginPolicyVerdict {
    Allowed,
    Denied,
    ReviewRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPolicyDecision {
    pub verdict: PluginPolicyVerdict,
    pub reason: String,
    pub fix: String,
}

pub fn evaluate_gstreamer_factory(factory: &GstFactoryInfo) -> PluginPolicyDecision {
    let name = factory.name.to_ascii_lowercase();
    let plugin_name = normalized_optional(&factory.plugin_name);
    let package = normalized_optional(&factory.package);
    let license = normalized_optional(&factory.license);

    if is_known_denied_factory(&name, plugin_name.as_deref())
        || license.as_deref().is_some_and(is_denied_license)
    {
        return decision(
            PluginPolicyVerdict::Denied,
            format!(
                "GStreamer factory '{}' is denied by the render plugin policy.",
                factory.name
            ),
            "Choose an approved LGPL-compatible GStreamer element or add a reviewed policy entry.",
        );
    }

    if package
        .as_deref()
        .is_some_and(|value| value.contains("bad plug-ins"))
    {
        return decision(
            PluginPolicyVerdict::ReviewRequired,
            format!(
                "GStreamer factory '{}' is from a plugin set that requires review.",
                factory.name
            ),
            "Review the plugin wrapper, linked libraries, patent implications, and distribution terms before allowing it.",
        );
    }

    if is_allowed_factory(&name, plugin_name.as_deref(), package.as_deref())
        && license.as_deref().is_some_and(is_lgpl_compatible_license)
    {
        return decision(
            PluginPolicyVerdict::Allowed,
            format!(
                "GStreamer factory '{}' is in the reviewed LGPL-compatible allowlist.",
                factory.name
            ),
            "No action required.",
        );
    }

    decision(
        PluginPolicyVerdict::Denied,
        format!(
            "GStreamer factory '{}' is not in the reviewed allowlist.",
            factory.name
        ),
        "Choose an approved LGPL-compatible GStreamer element or add a reviewed policy entry.",
    )
}

fn normalized_optional(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
}

fn is_known_denied_factory(name: &str, plugin_name: Option<&str>) -> bool {
    name == "x264enc"
        || name.starts_with("avenc_")
        || name.starts_with("avdec_")
        || name.contains("fdkaac")
        || plugin_name.is_some_and(|plugin| {
            plugin == "libav" || plugin == "x264" || plugin.contains("fdkaac")
        })
}

fn is_allowed_factory(name: &str, plugin_name: Option<&str>, package: Option<&str>) -> bool {
    const ALLOWED_FACTORY_NAMES: &[&str] = &[
        "filesrc",
        "queue",
        "decodebin",
        "uridecodebin",
        "videoconvert",
        "videoscale",
        "videorate",
        "audioconvert",
        "audioresample",
        "compositor",
        "appsrc",
        "capsfilter",
        "mp4mux",
        "matroskamux",
        "webmmux",
        "vp8enc",
        "vp9enc",
        "opusenc",
    ];
    const ALLOWED_PLUGIN_NAMES: &[&str] = &[
        "coreelements",
        "typefindfunctions",
        "playback",
        "videoconvertscale",
        "videorate",
        "audioconvert",
        "audioresample",
        "compositor",
        "app",
        "isomp4",
        "matroska",
        "vpx",
        "opus",
    ];

    ALLOWED_FACTORY_NAMES.contains(&name)
        || plugin_name.is_some_and(|plugin| ALLOWED_PLUGIN_NAMES.contains(&plugin))
        || package.is_some_and(|package| {
            package == "gstreamer"
                || package.contains("base plug-ins")
                || package.contains("good plug-ins")
        })
}

fn is_denied_license(license: &str) -> bool {
    license.contains("nonfree")
        || license.contains("non-free")
        || license.contains("proprietary")
        || license_tokens(license).any(|token| token == "gpl" || token.starts_with("gpl-"))
}

fn is_lgpl_compatible_license(license: &str) -> bool {
    license.contains("lgpl")
}

fn license_tokens(license: &str) -> impl Iterator<Item = &str> {
    license.split(|character: char| {
        character.is_whitespace()
            || matches!(character, '/' | ',' | ';' | '(' | ')' | '[' | ']')
    })
}

fn decision(
    verdict: PluginPolicyVerdict,
    reason: impl Into<String>,
    fix: impl Into<String>,
) -> PluginPolicyDecision {
    PluginPolicyDecision {
        verdict,
        reason: reason.into(),
        fix: fix.into(),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gstreamer_plugin_policy
```

Expected: PASS for the four `gstreamer_plugin_policy_*` tests.

- [ ] **Step 5: Commit**

Run:

```bash
rtk git add src-tauri/src/render_pipeline/mod.rs src-tauri/src/render_pipeline/error.rs src-tauri/src/render_pipeline/plugin_policy.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: add gstreamer plugin policy"
```

Expected: commit succeeds with only the files from this task.

## Task 2: Add Disabled GES Backend Skeleton

**Files:**
- Create: `src-tauri/src/render_pipeline/gstreamer_backend.rs`
- Modify: `src-tauri/src/render_pipeline/mod.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write the failing test**

Extend the existing backend import in `src-tauri/tests/render_pipeline.rs` from:

```rust
use video_creater_lib::render_pipeline::backend::{
    build_graphics_overlay_input, FfmpegRenderBackend, RenderBackend,
};
```

to:

```rust
use video_creater_lib::render_pipeline::backend::{
    build_graphics_overlay_input, FfmpegRenderBackend, RenderBackend,
};
use video_creater_lib::render_pipeline::gstreamer_backend::GstreamerGesRenderBackend;
```

Add this test near `ffmpeg_backend_builds_command_without_spawning_process`:

```rust
#[test]
fn gstreamer_ges_backend_reports_feature_disabled_without_spawning_process() {
    let backend = GstreamerGesRenderBackend::new();
    let errors = backend
        .build_command(&render_plan(), &[])
        .expect_err("disabled GES backend should return an actionable error");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::RenderBackendUnavailable);
    assert_eq!(errors[0].path, "gstreamer.ges");
    assert!(
        errors[0].message.contains("not enabled"),
        "message should explain feature state: {}",
        errors[0].message
    );
    assert!(
        errors[0].fix.contains("--features ges-render"),
        "fix should tell developers how to enable the backend: {}",
        errors[0].fix
    );
    assert_eq!(
        errors[0].details.get("backend"),
        Some(&"gstreamer-ges".to_string())
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gstreamer_ges_backend_reports_feature_disabled_without_spawning_process
```

Expected: FAIL because `render_pipeline::gstreamer_backend` does not exist.

- [ ] **Step 3: Add the minimal implementation**

Append this export to `src-tauri/src/render_pipeline/mod.rs`:

```rust
pub mod gstreamer_backend;
```

Create `src-tauri/src/render_pipeline/gstreamer_backend.rs`:

```rust
use crate::edit::render_plan::RenderPlan;
use crate::graphics::manifest::GraphicsArtifactManifest;
use std::path::PathBuf;

use super::backend::RenderBackend;
use super::error::{PipelineError, PipelineErrorCode, PipelineResult};
use super::process::CommandSpec;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GstreamerGesRenderBackend;

impl GstreamerGesRenderBackend {
    pub fn new() -> Self {
        Self
    }
}

impl RenderBackend for GstreamerGesRenderBackend {
    fn build_command(
        &self,
        _plan: &RenderPlan,
        _graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
    ) -> PipelineResult<CommandSpec> {
        Err(vec![ges_feature_disabled_error()])
    }
}

fn ges_feature_disabled_error() -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::RenderBackendUnavailable,
        "gstreamer.ges",
        "GStreamer/GES render backend is not enabled in this build.",
        "Rebuild with `rtk cargo test --manifest-path src-tauri/Cargo.toml --features ges-render` after installing GStreamer Editing Services.",
    )
    .with_detail("backend", "gstreamer-ges")
    .with_detail("feature", "ges-render")
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gstreamer_ges_backend_reports_feature_disabled_without_spawning_process
```

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
rtk git add src-tauri/src/render_pipeline/mod.rs src-tauri/src/render_pipeline/gstreamer_backend.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: add gstreamer ges backend gate"
```

Expected: commit succeeds with only the files from this task.

## Task 3: Add Optional GES Render Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/render_pipeline/gstreamer_backend.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write the failing test for feature-disabled selection metadata**

Add this test below the disabled backend test:

```rust
#[test]
fn gstreamer_ges_backend_names_required_feature() {
    let backend = GstreamerGesRenderBackend::new();

    assert_eq!(backend.backend_name(), "gstreamer-ges");
    assert_eq!(backend.required_feature(), "ges-render");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gstreamer_ges_backend_names_required_feature
```

Expected: FAIL because `backend_name` and `required_feature` do not exist.

- [ ] **Step 3: Add optional dependencies and metadata methods**

In `src-tauri/Cargo.toml`, add this feature block after the `[lib]` section and before `[[bin]]`:

```toml
[features]
default = []
ges-render = [
  "dep:gstreamer",
  "dep:gstreamer-pbutils",
  "dep:gstreamer-editing-services",
]
```

Add these optional dependencies to `[dependencies]`:

```toml
gstreamer = { version = "0.25", optional = true }
gstreamer-editing-services = { version = "0.25", optional = true }
gstreamer-pbutils = { version = "0.25", optional = true }
```

Add these methods to `impl GstreamerGesRenderBackend` in `src-tauri/src/render_pipeline/gstreamer_backend.rs`:

```rust
    pub fn backend_name(&self) -> &'static str {
        "gstreamer-ges"
    }

    pub fn required_feature(&self) -> &'static str {
        "ges-render"
    }
```

Update `ges_feature_disabled_error()` in `src-tauri/src/render_pipeline/gstreamer_backend.rs` so the detail values come from constants rather than duplicated strings:

```rust
const BACKEND_NAME: &str = "gstreamer-ges";
const REQUIRED_FEATURE: &str = "ges-render";
```

The final `gstreamer_backend.rs` should look like this:

```rust
use crate::edit::render_plan::RenderPlan;
use crate::graphics::manifest::GraphicsArtifactManifest;
use std::path::PathBuf;

use super::backend::RenderBackend;
use super::error::{PipelineError, PipelineErrorCode, PipelineResult};
use super::process::CommandSpec;

const BACKEND_NAME: &str = "gstreamer-ges";
const REQUIRED_FEATURE: &str = "ges-render";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GstreamerGesRenderBackend;

impl GstreamerGesRenderBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn backend_name(&self) -> &'static str {
        BACKEND_NAME
    }

    pub fn required_feature(&self) -> &'static str {
        REQUIRED_FEATURE
    }
}

impl RenderBackend for GstreamerGesRenderBackend {
    fn build_command(
        &self,
        _plan: &RenderPlan,
        _graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
    ) -> PipelineResult<CommandSpec> {
        Err(vec![ges_feature_disabled_error()])
    }
}

fn ges_feature_disabled_error() -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::RenderBackendUnavailable,
        "gstreamer.ges",
        "GStreamer/GES render backend is not enabled in this build.",
        "Rebuild with `rtk cargo test --manifest-path src-tauri/Cargo.toml --features ges-render` after installing GStreamer Editing Services.",
    )
    .with_detail("backend", BACKEND_NAME)
    .with_detail("feature", REQUIRED_FEATURE)
}
```

- [ ] **Step 4: Run tests to verify they pass without the GES feature**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gstreamer_ges_backend
```

Expected: PASS for both `gstreamer_ges_backend_*` tests.

If Cargo needs to resolve the new optional dependencies and network access is blocked, rerun the exact command with escalation as required by the workspace policy.

- [ ] **Step 5: Verify the GES feature fails clearly when the system package is missing**

Run:

```bash
rtk cargo check --manifest-path src-tauri/Cargo.toml --features ges-render
```

Expected in the current local environment: FAIL from the GES sys crate because `gstreamer-editing-services-1.0` is not available to `pkg-config`.

Expected after installing GES system development files: PASS or normal Rust compile errors from code that this plan can then fix.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/render_pipeline/gstreamer_backend.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: gate gstreamer ges dependencies"
```

Expected: commit succeeds with Cargo changes and the backend metadata test.

## Task 4: Convert Policy Decisions Into Pipeline Errors

**Files:**
- Modify: `src-tauri/src/render_pipeline/plugin_policy.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write the failing test**

Add this import to the existing `plugin_policy` import in `src-tauri/tests/render_pipeline.rs`:

```rust
    policy_error_for_factory,
```

The import should become:

```rust
use video_creater_lib::render_pipeline::plugin_policy::{
    evaluate_gstreamer_factory, policy_error_for_factory, GstFactoryInfo, PluginPolicyVerdict,
};
```

Add this test near the other policy tests:

```rust
#[test]
fn gstreamer_plugin_policy_error_serializes_for_render_reports() {
    let factory = GstFactoryInfo::new("x264enc")
        .plugin_name("x264")
        .license("GPL");

    let error = policy_error_for_factory(&factory)
        .expect("denied factory should produce a pipeline error");

    assert_eq!(error.code, PipelineErrorCode::RenderBackendPolicyDenied);
    assert_eq!(error.path, "gstreamer.plugins.x264enc");
    assert!(error.message.contains("x264enc"));
    assert!(
        error.fix.contains("approved LGPL-compatible GStreamer element"),
        "fix should point to the allowlist workflow: {}",
        error.fix
    );
    assert_eq!(error.details.get("factory"), Some(&"x264enc".to_string()));
    assert_eq!(error.details.get("verdict"), Some(&"denied".to_string()));

    let json = serde_json::to_value(&error).expect("policy error serializes");
    assert_eq!(json["code"], "RENDER_BACKEND_POLICY_DENIED");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gstreamer_plugin_policy_error_serializes_for_render_reports
```

Expected: FAIL because `policy_error_for_factory` does not exist.

- [ ] **Step 3: Implement policy error conversion**

Add this import to `src-tauri/src/render_pipeline/plugin_policy.rs`:

```rust
use super::error::{PipelineError, PipelineErrorCode};
```

Add this function to `src-tauri/src/render_pipeline/plugin_policy.rs`:

```rust
pub fn policy_error_for_factory(factory: &GstFactoryInfo) -> Option<PipelineError> {
    let decision = evaluate_gstreamer_factory(factory);
    if decision.verdict == PluginPolicyVerdict::Allowed {
        return None;
    }

    let verdict = match decision.verdict {
        PluginPolicyVerdict::Allowed => "allowed",
        PluginPolicyVerdict::Denied => "denied",
        PluginPolicyVerdict::ReviewRequired => "review_required",
    };

    Some(
        PipelineError::new(
            PipelineErrorCode::RenderBackendPolicyDenied,
            format!("gstreamer.plugins.{}", factory.name),
            decision.reason,
            decision.fix,
        )
        .with_detail("factory", factory.name.clone())
        .with_detail("verdict", verdict),
    )
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gstreamer_plugin_policy_error_serializes_for_render_reports
```

Expected: PASS.

- [ ] **Step 5: Run all focused policy/backend tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gstreamer_
```

Expected: PASS for all tests whose names start with `gstreamer_`.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add src-tauri/src/render_pipeline/plugin_policy.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: report gstreamer policy denials"
```

Expected: commit succeeds with only policy conversion changes.

## Task 5: Final Verification For First Slice

**Files:**
- Modify only if verification exposes a bug in files touched by Tasks 1-4.

- [ ] **Step 1: Run targeted render pipeline tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gstreamer_
```

Expected: PASS.

- [ ] **Step 2: Run full Rust tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS. Existing tests unrelated to GStreamer should remain green.

- [ ] **Step 3: Run full project verification**

Run:

```bash
rtk pnpm verify
```

Expected: PASS. If verification requires dependency download and network access is blocked, rerun with escalation according to the workspace policy.

- [ ] **Step 4: Record local GES system dependency status**

Run:

```bash
rtk pkg-config --modversion gstreamer-1.0
rtk pkg-config --modversion gstreamer-editing-services-1.0
```

Expected in the current local environment:

- `gstreamer-1.0` prints a version.
- `gstreamer-editing-services-1.0` fails until the GES development package is installed.

- [ ] **Step 5: Commit verification fixes**

If Steps 1-3 required fixes, run:

```bash
rtk git add src-tauri/src/render_pipeline src-tauri/tests/render_pipeline.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
rtk git commit -m "fix: stabilize gstreamer backend foundation"
```

Expected: commit exists only if verification fixes were needed.

If no fixes were needed, do not create an empty commit.

## Self-Review Notes

Spec requirements covered by this plan:

- plugin policy data structures
- default-deny behavior for denied and unknown factories
- review-required handling for GStreamer Bad factories
- actionable backend error when the GES backend is selected without the feature enabled
- optional Cargo feature for GStreamer/GES dependencies
- focused tests that keep the default repo build usable before the local GES system package is installed

Spec requirements assigned to the next implementation plan:

- GStreamer/GES system dependency installation in local and CI environments
- GStreamer discovery/probe into `MediaProbe`
- real GES timeline construction from `RenderPlan`
- EDL-only render fixture through GES
- graphics overlay composition through GStreamer
- render report plugin evidence gathered from actual resolved GStreamer factories
- proposal and combined e2e paths preferring the GStreamer/GES backend
- ffmpeg and ffprobe removal after parity
