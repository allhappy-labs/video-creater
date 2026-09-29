# License-Safe GStreamer Output Profiles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add profile-neutral, license-safe GStreamer output profiles for WebM, MP4 H.264/H.265 with AAC, and ProRes MOV with PCM.

**Architecture:** Split render quality from output profile, resolve each output profile to a reviewed GStreamer encoding target, and fail closed when required factories are missing or denied. Keep the EDL-backed render plan and project-action flow, but make paths, validation, and export availability profile-neutral.

**Tech Stack:** Rust/Tauri, GStreamer/GES, GStreamer pbutils encoding profiles, React/TypeScript, Vitest, Cargo tests.

---

## File Structure

- Modify `src-tauri/src/edit/render_plan.rs`: add `RenderQuality`, `RenderOutputProfile`, compatibility parsing, and profile-neutral render plan fields.
- Create `src-tauri/src/render_pipeline/output_profile.rs`: resolve output profiles to `GstreamerEncodingTarget` values with caps/factory/extension metadata.
- Modify `src-tauri/src/render_pipeline/plugin_policy.rs`: add reviewed policy entries and platform-aware checks for `mp4mux`, `qtmux`, `aacparse`, `atenc`, and `vtenc_*`; keep risky factories denied.
- Modify `src-tauri/src/render_pipeline/gstreamer_backend.rs`: select encoding profiles from `GstreamerEncodingTarget` instead of WebM-only helpers.
- Modify `src-tauri/src/render_pipeline/project_export.rs`: rename WebM-specific structs/functions internally to profile-neutral equivalents and choose output paths by output profile.
- Modify `src-tauri/src/project/export_profiles.rs`: base MP4/ProRes availability on GStreamer profile policy instead of external encoder env vars.
- Modify `src-tauri/src/workflows/mod.rs`: route GStreamer-native exports directly through profile-neutral render activities.
- Modify `src-tauri/src/main.rs` and `src/lib/project.ts`: expose profile-neutral render/export command args while keeping compatibility aliases.
- Modify `src/lib/render.ts`, `src/components/workspace/render-quality-control.tsx`, and `src/components/workspace/editor-workspace.tsx`: separate quality from output profile in the UI.
- Test files: `src-tauri/tests/render_pipeline.rs`, `src-tauri/tests/project_export.rs`, `src-tauri/tests/export_profiles.rs`, `src-tauri/tests/temporal_workflows.rs`, `src/lib/render.test.ts`, `src/lib/project.test.ts`, `src/components/workspace/editor-workspace.test.tsx`.

## Task 1: Profile-Neutral Render Model

**Files:**
- Modify: `src-tauri/src/edit/render_plan.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing serialization tests**

Add tests asserting the new neutral wire names:

```rust
#[test]
fn render_quality_serializes_profile_neutral_values() {
    assert_eq!(
        serde_json::to_value(RenderQuality::Draft).expect("draft quality"),
        serde_json::json!("draft")
    );
    assert_eq!(
        serde_json::to_value(RenderQuality::Final).expect("final quality"),
        serde_json::json!("final")
    );
}

#[test]
fn render_output_profile_serializes_container_codec_values() {
    assert_eq!(
        serde_json::to_value(RenderOutputProfile::Mp4H264Aac).expect("mp4 profile"),
        serde_json::json!("mp4H264Aac")
    );
    assert_eq!(
        serde_json::to_value(RenderOutputProfile::MovProResPcm).expect("prores profile"),
        serde_json::json!("movProResPcm")
    );
}
```

- [ ] **Step 2: Run the tests to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml render_quality_serializes_profile_neutral_values render_output_profile_serializes_container_codec_values -- --test-threads=1
```

Expected: compile fails because `RenderQuality` and `RenderOutputProfile` do not exist.

- [ ] **Step 3: Implement minimal model types**

In `src-tauri/src/edit/render_plan.rs`, add:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RenderQuality {
    Draft,
    Final,
}

impl Default for RenderQuality {
    fn default() -> Self {
        Self::Draft
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RenderOutputProfile {
    WebmVp8Opus,
    WebmVp9Opus,
    Mp4H264Aac,
    Mp4H265Aac,
    MovProResPcm,
}

impl Default for RenderOutputProfile {
    fn default() -> Self {
        Self::WebmVp8Opus
    }
}
```

Keep `RenderQualityProfile` for compatibility in this task, but mark future internal use for migration.

- [ ] **Step 4: Run the tests to verify GREEN**

Run the same cargo test command. Expected: both new serialization tests pass.

- [ ] **Step 5: Commit**

```bash
rtk git add src-tauri/src/edit/render_plan.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: add profile-neutral render model"
```

Use `--no-gpg-sign` only if SSH signing fails with the same missing-agent-key error seen during the spec commit.

## Task 2: GStreamer Output Target Resolution

**Files:**
- Create: `src-tauri/src/render_pipeline/output_profile.rs`
- Modify: `src-tauri/src/render_pipeline/mod.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing target resolution tests**

Add tests:

```rust
#[test]
fn mp4_h264_aac_target_requires_videotoolbox_audiotoolbox_and_mp4_factories() {
    let target = gstreamer_encoding_target(RenderOutputProfile::Mp4H264Aac, RenderQuality::Final)
        .expect("mp4 h264 target");

    assert_eq!(target.extension, "mp4");
    assert_eq!(target.mime_type, "video/mp4");
    assert_eq!(target.container_caps, "video/quicktime");
    assert_eq!(target.video_caps, "video/x-h264");
    assert_eq!(target.audio_caps.as_deref(), Some("audio/mpeg"));
    assert_eq!(
        target.required_factories,
        vec!["mp4mux", "vtenc_h264", "atenc", "aacparse"]
    );
}

#[test]
fn prores_mov_target_requires_qtmux_vtenc_prores_and_pcm_audio() {
    let target = gstreamer_encoding_target(RenderOutputProfile::MovProResPcm, RenderQuality::Final)
        .expect("prores target");

    assert_eq!(target.extension, "mov");
    assert_eq!(target.mime_type, "video/quicktime");
    assert_eq!(target.video_caps, "video/x-prores");
    assert_eq!(target.audio_caps.as_deref(), Some("audio/x-raw"));
    assert_eq!(target.required_factories, vec!["qtmux", "vtenc_prores"]);
}
```

- [ ] **Step 2: Run target tests to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml mp4_h264_aac_target_requires_videotoolbox_audiotoolbox_and_mp4_factories prores_mov_target_requires_qtmux_vtenc_prores_and_pcm_audio -- --test-threads=1
```

Expected: compile fails because `output_profile` and `gstreamer_encoding_target` do not exist.

- [ ] **Step 3: Implement `output_profile.rs`**

Create `src-tauri/src/render_pipeline/output_profile.rs`:

```rust
use crate::edit::render_plan::{RenderOutputProfile, RenderQuality};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GstreamerEncodingTarget {
    pub output_profile: RenderOutputProfile,
    pub quality: RenderQuality,
    pub extension: &'static str,
    pub mime_type: &'static str,
    pub container_caps: &'static str,
    pub video_caps: &'static str,
    pub audio_caps: Option<&'static str>,
    pub required_factories: Vec<&'static str>,
    pub macos_only: bool,
}

pub fn gstreamer_encoding_target(
    output_profile: RenderOutputProfile,
    quality: RenderQuality,
) -> Result<GstreamerEncodingTarget, &'static str> {
    let target = match output_profile {
        RenderOutputProfile::WebmVp8Opus => GstreamerEncodingTarget {
            output_profile,
            quality,
            extension: "webm",
            mime_type: "video/webm",
            container_caps: "video/webm",
            video_caps: "video/x-vp8",
            audio_caps: Some("audio/x-opus"),
            required_factories: vec!["webmmux", "vp8enc", "opusenc"],
            macos_only: false,
        },
        RenderOutputProfile::WebmVp9Opus => GstreamerEncodingTarget {
            output_profile,
            quality,
            extension: "webm",
            mime_type: "video/webm",
            container_caps: "video/webm",
            video_caps: "video/x-vp9",
            audio_caps: Some("audio/x-opus"),
            required_factories: vec!["webmmux", "vp9enc", "opusenc"],
            macos_only: false,
        },
        RenderOutputProfile::Mp4H264Aac => GstreamerEncodingTarget {
            output_profile,
            quality,
            extension: "mp4",
            mime_type: "video/mp4",
            container_caps: "video/quicktime",
            video_caps: "video/x-h264",
            audio_caps: Some("audio/mpeg"),
            required_factories: vec!["mp4mux", "vtenc_h264", "atenc", "aacparse"],
            macos_only: true,
        },
        RenderOutputProfile::Mp4H265Aac => GstreamerEncodingTarget {
            output_profile,
            quality,
            extension: "mp4",
            mime_type: "video/mp4",
            container_caps: "video/quicktime",
            video_caps: "video/x-h265",
            audio_caps: Some("audio/mpeg"),
            required_factories: vec!["mp4mux", "vtenc_h265", "atenc", "aacparse"],
            macos_only: true,
        },
        RenderOutputProfile::MovProResPcm => GstreamerEncodingTarget {
            output_profile,
            quality,
            extension: "mov",
            mime_type: "video/quicktime",
            container_caps: "video/quicktime",
            video_caps: "video/x-prores",
            audio_caps: Some("audio/x-raw"),
            required_factories: vec!["qtmux", "vtenc_prores"],
            macos_only: true,
        },
    };
    Ok(target)
}
```

Export it from `src-tauri/src/render_pipeline/mod.rs`:

```rust
pub mod output_profile;
```

- [ ] **Step 4: Run target tests to verify GREEN**

Run the same cargo test command. Expected: both target resolution tests pass.

- [ ] **Step 5: Commit**

```bash
rtk git add src-tauri/src/render_pipeline/output_profile.rs src-tauri/src/render_pipeline/mod.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: resolve gstreamer output profiles"
```

## Task 3: License-Safe Plugin Policy

**Files:**
- Modify: `src-tauri/src/render_pipeline/plugin_policy.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing policy tests**

Add tests for reviewed allowed factories and denied AAC/libav alternatives:

```rust
#[test]
fn plugin_policy_allows_reviewed_macos_gstreamer_factories() {
    for factory in [
        GstFactoryInfo::new("mp4mux")
            .plugin_name("isomp4")
            .package("GStreamer Good Plug-ins source release")
            .license("LGPL"),
        GstFactoryInfo::new("atenc")
            .plugin_name("osxaudio")
            .package("GStreamer Good Plug-ins source release")
            .license("LGPL"),
        GstFactoryInfo::new("vtenc_h264")
            .plugin_name("applemedia")
            .package("GStreamer Bad Plug-ins source release")
            .license("LGPL"),
        GstFactoryInfo::new("vtenc_prores")
            .plugin_name("applemedia")
            .package("GStreamer Bad Plug-ins source release")
            .license("LGPL"),
    ] {
        assert_eq!(
            evaluate_gstreamer_factory(&factory).verdict,
            PluginPolicyVerdict::Allowed,
            "{factory:?}"
        );
    }
}

#[test]
fn plugin_policy_denies_unapproved_aac_and_libav_factories() {
    for factory in [
        GstFactoryInfo::new("fdkaacenc").plugin_name("fdkaac").license("LGPL"),
        GstFactoryInfo::new("faac").plugin_name("faac").license("LGPL"),
        GstFactoryInfo::new("voaacenc").plugin_name("voaacenc").license("LGPL"),
        GstFactoryInfo::new("avenc_aac").plugin_name("libav").license("LGPL"),
        GstFactoryInfo::new("avenc_aac_at").plugin_name("libav").license("LGPL"),
    ] {
        assert_ne!(
            evaluate_gstreamer_factory(&factory).verdict,
            PluginPolicyVerdict::Allowed,
            "{factory:?}"
        );
    }
}
```

- [ ] **Step 2: Run policy tests to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml plugin_policy_allows_reviewed_macos_gstreamer_factories plugin_policy_denies_unapproved_aac_and_libav_factories -- --test-threads=1
```

Expected: allowed test fails for at least `mp4mux`, `atenc`, and `vtenc_*`.

- [ ] **Step 3: Update allowed and denied policy**

In `is_known_denied_factory`, add exact denials:

```rust
matches!(name, "faac" | "voaacenc" | "openh264enc")
```

Add GStreamer Good and Bad package families if needed:

```rust
const GSTREAMER_BAD_PACKAGES: &[&str] =
    &["gstreamerbadplugins", "gstreamerbadpluginssourcerelease"];
```

Add `AllowedFactoryPolicy` entries for `mp4mux`, `qtmux`, `aacparse`, `atenc`, `vtenc_h264`, `vtenc_h265`, and `vtenc_prores` with the plugin/package/license constraints described in the spec.

- [ ] **Step 4: Run policy tests to verify GREEN**

Run the same policy test command. Expected: tests pass.

- [ ] **Step 5: Commit**

```bash
rtk git add src-tauri/src/render_pipeline/plugin_policy.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: allow reviewed gstreamer export factories"
```

## Task 4: Backend Encoding Profile Selection

**Files:**
- Modify: `src-tauri/src/render_pipeline/gstreamer_backend.rs`
- Modify: `src-tauri/src/edit/render_plan.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing encoding summary tests**

Add a test that constructs a render plan with `quality=Final` and `output_profile=Mp4H264Aac`, then asserts the backend summary reports `mp4mux`, `vtenc_h264`, and `atenc`. Add a second test for `MovProResPcm`.

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml mp4_encoding_profile_summary prores_encoding_profile_summary -- --test-threads=1
```

Expected: compile fails or tests fail because render plans cannot carry neutral output profiles and summaries are WebM-only.

- [ ] **Step 3: Extend `RenderPlan`**

Add fields:

```rust
#[serde(default)]
pub quality: RenderQuality,
#[serde(default)]
pub output_profile: RenderOutputProfile,
```

Retain `quality_profile: RenderQualityProfile` temporarily with `skip_serializing_if` or compatibility conversion only if existing callers still depend on it. New code should set `quality` and `output_profile`.

- [ ] **Step 4: Replace WebM-only encoding helpers**

In `gstreamer_backend.rs`, replace `webm_encoding_profile(plan)` with `encoding_profile(plan, &target)` using `gstreamer_encoding_target(plan.output_profile, plan.quality)`. Build:

- WebM container/video/audio as existing.
- MP4 container caps `video/quicktime`, video caps `video/x-h264` or `video/x-h265`, audio caps `audio/mpeg`.
- MOV container caps `video/quicktime`, video caps `video/x-prores`, audio caps `audio/x-raw`.

Call `require_allowed_factories(&target.required_factories)` before render.

- [ ] **Step 5: Replace extension validation**

Replace `validate_webm_output` with `validate_output_path_for_target(plan, &target)` that checks `plan.output_path` extension equals `target.extension`.

- [ ] **Step 6: Run tests to verify GREEN**

Run the same encoding summary command, then:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml render_pipeline -- --test-threads=1
```

Expected: profile summary tests pass and existing render pipeline tests remain green.

- [ ] **Step 7: Commit**

```bash
rtk git add src-tauri/src/edit/render_plan.rs src-tauri/src/render_pipeline/gstreamer_backend.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: select gstreamer encoding targets by output profile"
```

## Task 5: Profile-Neutral Project Render Paths

**Files:**
- Modify: `src-tauri/src/render_pipeline/project_export.rs`
- Test: `src-tauri/tests/project_export.rs`

- [ ] **Step 1: Write failing path and plan tests**

Add tests that build project render paths for `Mp4H264Aac` and `MovProResPcm` and assert `output.mp4` and `output.mov`. Add a test that `build_project_render_plan` sets `quality=Final` and `output_profile=Mp4H264Aac`.

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml project_render_paths_use_output_profile_extension project_render_plan_sets_quality_and_output_profile -- --test-threads=1
```

Expected: compile fails because project export is still WebM-specific.

- [ ] **Step 3: Rename path/result types**

Rename or alias:

- `ProjectWebmRenderPaths` -> `ProjectRenderPaths`.
- `ProjectWebmRenderResult` -> `ProjectRenderResult`.
- `build_project_webm_render_plan` -> `build_project_render_plan`.
- `render_webm_to_split_project_folder` -> `render_output_to_split_project_folder`.

Keep wrappers with old names for compatibility until UI migration is complete.

- [ ] **Step 4: Use output target for output path**

`ProjectRenderPaths::new(job_id, output_profile)` should resolve `target.extension` and write `renders/<jobId>/output.<extension>`.

- [ ] **Step 5: Run tests to verify GREEN**

Run the same project export test command, then:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml project_export -- --test-threads=1
```

Expected: project export tests pass.

- [ ] **Step 6: Commit**

```bash
rtk git add src-tauri/src/render_pipeline/project_export.rs src-tauri/tests/project_export.rs
rtk git commit -m "feat: make project render paths output-profile aware"
```

## Task 6: Export Profile Availability From GStreamer Policy

**Files:**
- Modify: `src-tauri/src/project/export_profiles.rs`
- Test: `src-tauri/tests/export_profiles.rs`

- [ ] **Step 1: Write failing availability tests**

Change availability expectations so MP4/ProRes profiles require GStreamer factories, not external encoder commands. Add assertions that `mp4H264` requires `mp4mux`, `vtenc_h264`, `atenc`, and `aacparse`.

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml export_profiles -- --test-threads=1
```

Expected: tests fail because current availability uses `VIDEO_CREATER_APPROVED_*_ENCODER` env vars.

- [ ] **Step 3: Implement GStreamer-backed availability**

Update `ExportProfileAvailability.required_runtime` to list required GStreamer factories for MP4/ProRes. Use `gstreamer_encoding_target` and a new availability helper that checks factory presence and `policy_error_for_factory`.

- [ ] **Step 4: Preserve fail-closed behavior in tests**

Tests should isolate `GST_PLUGIN_SYSTEM_PATH_1_0` or use an injected factory checker so default CI can verify missing-runtime behavior deterministically.

- [ ] **Step 5: Run tests to verify GREEN**

Run the same export profile command. Expected: export profile tests pass.

- [ ] **Step 6: Commit**

```bash
rtk git add src-tauri/src/project/export_profiles.rs src-tauri/tests/export_profiles.rs
rtk git commit -m "feat: report export availability from gstreamer policy"
```

## Task 7: Temporal and Tauri Command Migration

**Files:**
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/workflows/mod.rs`
- Modify: `src/lib/project.ts`
- Test: `src-tauri/tests/temporal_workflows.rs`
- Test: `src/lib/project.test.ts`

- [ ] **Step 1: Write failing command contract tests**

Add tests proving render/export requests carry `quality` and `outputProfile` separately.

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml temporal_export_media_workflow_activity_plan_sequences_codec_profile_export -- --test-threads=1
rtk pnpm test src/lib/project.test.ts
```

Expected: tests fail because commands still use `profile: finalWebm` or external encoder flow.

- [ ] **Step 3: Add profile-neutral Tauri command**

Add `render_output_to_split_project_folder(projectDir, quality, outputProfile, jobId, updatedAt, rangeStartSeconds, rangeEndSeconds)`. Keep `render_webm_to_split_project_folder` as a wrapper that maps `draftWebm` to `quality=draft, outputProfile=webmVp8Opus` and `finalWebm` to `quality=final, outputProfile=webmVp9Opus`.

- [ ] **Step 4: Update Temporal export-media**

For MP4/ProRes profiles, build render input with `quality=final` and the matching `RenderOutputProfile`. Remove external approved encoder command use for these GStreamer-native profiles.

- [ ] **Step 5: Run tests to verify GREEN**

Run the same Rust and TypeScript command contract tests. Expected: tests pass.

- [ ] **Step 6: Commit**

```bash
rtk git add src-tauri/src/main.rs src-tauri/src/workflows/mod.rs src/lib/project.ts src-tauri/tests/temporal_workflows.rs src/lib/project.test.ts
rtk git commit -m "feat: route exports through profile-neutral render commands"
```

## Task 8: UI Quality/Profile Separation

**Files:**
- Modify: `src/lib/render.ts`
- Modify: `src/components/workspace/render-quality-control.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Test: `src/lib/render.test.ts`
- Test: `src/components/workspace/editor-workspace.test.tsx`

- [ ] **Step 1: Write failing UI model tests**

Update `src/lib/render.test.ts` to expect `renderQualityOptions` values `draft` and `final`, and a separate `renderOutputProfileOptions` list including `webmVp8Opus`, `mp4H264Aac`, and `movProResPcm`.

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk pnpm test src/lib/render.test.ts src/components/workspace/editor-workspace.test.tsx
```

Expected: tests fail because frontend still models `draftWebm` and `finalWebm`.

- [ ] **Step 3: Update frontend render model**

In `src/lib/render.ts`, define:

```ts
export type RenderQuality = "draft" | "final";
export type RenderOutputProfile =
  | "webmVp8Opus"
  | "webmVp9Opus"
  | "mp4H264Aac"
  | "mp4H265Aac"
  | "movProResPcm";
```

Keep old `RenderQualityProfile` only as a compatibility type if needed by old report parsing.

- [ ] **Step 4: Update editor export controls**

Make WebM render buttons call `renderOutputToSplitProjectFolder` with separate `quality` and `outputProfile`. Make MP4/ProRes buttons use profile availability and matching output profile.

- [ ] **Step 5: Run UI tests to verify GREEN**

Run the same pnpm test command. Expected: frontend tests pass.

- [ ] **Step 6: Commit**

```bash
rtk git add src/lib/render.ts src/components/workspace/render-quality-control.tsx src/components/workspace/editor-workspace.tsx src/lib/render.test.ts src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat: separate render quality from output profiles"
```

## Task 9: Final Verification

**Files:**
- Existing source and tests only.

- [ ] **Step 1: Run Rust render/export tests**

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml render_pipeline project_export export_profiles temporal_workflows -- --test-threads=1
```

Expected: all targeted Rust tests pass.

- [ ] **Step 2: Run frontend tests**

```bash
rtk pnpm test src/lib/render.test.ts src/lib/project.test.ts src/components/workspace/editor-workspace.test.tsx
```

Expected: all targeted frontend tests pass.

- [ ] **Step 3: Inspect git diff**

```bash
rtk git status --short
rtk git diff --stat
```

Expected: only intentional implementation files changed; pre-existing untracked `docs/parity.md`, `palmier-pro/`, and `src-tauri/native/fluidaudio-parakeet/.build/` remain untouched unless the user separately asks to handle them.

## Self-Review

- Spec coverage: model separation, `atenc` MP4 audio, ProRes PCM, plugin policy denials, GStreamer target resolution, UI/Temporal migration, and validation are all assigned to tasks.
- Placeholder scan: no deferred implementation placeholders remain; each task has concrete files, tests, commands, and expected outcomes.
- Type consistency: `RenderQuality` and `RenderOutputProfile` are the canonical names throughout; legacy `RenderQualityProfile` is compatibility-only during migration.
