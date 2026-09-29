# GStreamer Native MP4 And ProRes Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render WebM, MP4 H.264/H.265 with AAC, and ProRes MOV directly through license-safe GStreamer/GES profiles.

**Architecture:** Introduce a profile-driven GStreamer encoding target layer, extend plugin policy with reviewed macOS factories, and migrate project export, availability, Temporal, and UI paths away from WebM-only naming and external encoder commands for native profiles. The render remains EDL-first and mutates project state only after render/probe/validation/report success.

**Tech Stack:** Rust/Tauri, GStreamer/GES, GStreamer pbutils encoding profiles, React/TypeScript, Cargo tests, Vitest.

---

## File Structure

- Create `src-tauri/src/render_pipeline/output_profile.rs`: own profile metadata, required factories, output extension, caps, and availability checks.
- Modify `src-tauri/src/render_pipeline/mod.rs`: export `output_profile`.
- Modify `src-tauri/src/render_pipeline/plugin_policy.rs`: add reviewed allowlist entries for `mp4mux`, `qtmux`, `aacparse`, `atenc`, `vtenc_h264`, `vtenc_h265`, `vtenc_prores`; keep unsafe AAC/libav/x264 alternatives denied.
- Modify `src-tauri/src/render_pipeline/gstreamer_backend.rs`: resolve encoding profiles from output profile instead of WebM-only helpers.
- Modify `src-tauri/src/render_pipeline/project_export.rs`: add profile-neutral render paths/results and render MP4/MOV directly.
- Modify `src-tauri/src/project/export_profiles.rs`: report MP4/ProRes availability from GStreamer factory/policy state instead of external encoder env vars.
- Modify `src-tauri/src/workflows/mod.rs`: use profile-neutral render/export for native GStreamer profiles.
- Modify `src-tauri/src/main.rs`: expose a profile-neutral Tauri render command and keep WebM compatibility wrapper if needed.
- Modify `src/lib/project.ts`, `src/lib/render.ts`, `src/components/workspace/editor-workspace.tsx`, `src/components/workspace/render-quality-control.tsx`: reflect native profile availability and command shape.
- Tests: `src-tauri/tests/render_pipeline.rs`, `src-tauri/tests/project_export.rs`, `src-tauri/tests/export_profiles.rs`, `src-tauri/tests/temporal_workflows.rs`, `src/lib/render.test.ts`, `src/lib/project.test.ts`, `src/components/workspace/editor-workspace.test.tsx`.

## Task 1: Output Profile Metadata

**Files:**
- Create: `src-tauri/src/render_pipeline/output_profile.rs`
- Modify: `src-tauri/src/render_pipeline/mod.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing profile metadata tests**

Add tests:

```rust
#[test]
fn mp4_h264_profile_resolves_to_audiotoolbox_aac_target() {
    let target = gstreamer_output_profile_target(ExportProfile::Mp4H264)
        .expect("mp4 h264 target");

    assert_eq!(target.extension, "mp4");
    assert_eq!(target.mime_type, "video/mp4");
    assert_eq!(target.container_factory, "mp4mux");
    assert_eq!(target.video_factory, "vtenc_h264");
    assert_eq!(target.audio_factory, Some("atenc"));
    assert_eq!(target.parser_factories, vec!["aacparse"]);
    assert_eq!(target.required_factories(), vec!["mp4mux", "vtenc_h264", "atenc", "aacparse"]);
}

#[test]
fn prores_profile_resolves_to_quicktime_prores_pcm_target() {
    let target = gstreamer_output_profile_target(ExportProfile::ProResMov)
        .expect("prores mov target");

    assert_eq!(target.extension, "mov");
    assert_eq!(target.mime_type, "video/quicktime");
    assert_eq!(target.container_factory, "qtmux");
    assert_eq!(target.video_factory, "vtenc_prores");
    assert_eq!(target.audio_factory, None);
    assert_eq!(target.audio_mode, GstreamerAudioMode::RawPcm);
    assert_eq!(target.required_factories(), vec!["qtmux", "vtenc_prores"]);
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline mp4_h264_profile_resolves_to_audiotoolbox_aac_target -- --test-threads=1 --nocapture
```

Expected: compile fails because `output_profile` target types do not exist.

- [ ] **Step 3: Implement target metadata**

Create `src-tauri/src/render_pipeline/output_profile.rs`:

```rust
use crate::project::export_profiles::ExportProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GstreamerAudioMode {
    EncodedAac,
    RawPcm,
    Opus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GstreamerOutputProfileTarget {
    pub profile: ExportProfile,
    pub extension: &'static str,
    pub mime_type: &'static str,
    pub container_factory: &'static str,
    pub container_caps: &'static str,
    pub video_factory: &'static str,
    pub video_caps: &'static str,
    pub audio_factory: Option<&'static str>,
    pub audio_caps: Option<&'static str>,
    pub audio_mode: GstreamerAudioMode,
    pub parser_factories: Vec<&'static str>,
    pub macos_only: bool,
}

impl GstreamerOutputProfileTarget {
    pub fn required_factories(&self) -> Vec<&'static str> {
        let mut factories = vec![self.container_factory, self.video_factory];
        if let Some(audio_factory) = self.audio_factory {
            factories.push(audio_factory);
        }
        factories.extend(self.parser_factories.iter().copied());
        factories
    }
}

pub fn gstreamer_output_profile_target(
    profile: ExportProfile,
) -> Option<GstreamerOutputProfileTarget> {
    match profile {
        ExportProfile::Mp4H264 => Some(GstreamerOutputProfileTarget {
            profile,
            extension: "mp4",
            mime_type: "video/mp4",
            container_factory: "mp4mux",
            container_caps: "video/quicktime",
            video_factory: "vtenc_h264",
            video_caps: "video/x-h264",
            audio_factory: Some("atenc"),
            audio_caps: Some("audio/mpeg"),
            audio_mode: GstreamerAudioMode::EncodedAac,
            parser_factories: vec!["aacparse"],
            macos_only: true,
        }),
        ExportProfile::Mp4H265 => Some(GstreamerOutputProfileTarget {
            profile,
            extension: "mp4",
            mime_type: "video/mp4",
            container_factory: "mp4mux",
            container_caps: "video/quicktime",
            video_factory: "vtenc_h265",
            video_caps: "video/x-h265",
            audio_factory: Some("atenc"),
            audio_caps: Some("audio/mpeg"),
            audio_mode: GstreamerAudioMode::EncodedAac,
            parser_factories: vec!["aacparse"],
            macos_only: true,
        }),
        ExportProfile::ProResMov => Some(GstreamerOutputProfileTarget {
            profile,
            extension: "mov",
            mime_type: "video/quicktime",
            container_factory: "qtmux",
            container_caps: "video/quicktime",
            video_factory: "vtenc_prores",
            video_caps: "video/x-prores",
            audio_factory: None,
            audio_caps: Some("audio/x-raw"),
            audio_mode: GstreamerAudioMode::RawPcm,
            parser_factories: Vec::new(),
            macos_only: true,
        }),
        ExportProfile::PalmierProject => None,
    }
}
```

Add `pub mod output_profile;` to `src-tauri/src/render_pipeline/mod.rs`.

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline mp4_h264_profile_resolves_to_audiotoolbox_aac_target -- --test-threads=1 --nocapture
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline prores_profile_resolves_to_quicktime_prores_pcm_target -- --test-threads=1 --nocapture
```

Expected: both tests pass.

- [ ] **Step 5: Commit**

```bash
rtk git add src-tauri/src/render_pipeline/output_profile.rs src-tauri/src/render_pipeline/mod.rs src-tauri/tests/render_pipeline.rs
rtk git -c commit.gpgsign=false commit -m "feat: add gstreamer output profile targets"
```

## Task 2: Reviewed Plugin Policy

**Files:**
- Modify: `src-tauri/src/render_pipeline/plugin_policy.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing allow/deny tests**

Add tests:

```rust
#[test]
fn plugin_policy_allows_reviewed_native_export_factories() {
    let factories = [
        GstFactoryInfo::new("mp4mux")
            .plugin_name("isomp4")
            .package("GStreamer Good Plug-ins source release")
            .license("LGPL"),
        GstFactoryInfo::new("qtmux")
            .plugin_name("isomp4")
            .package("GStreamer Good Plug-ins source release")
            .license("LGPL"),
        GstFactoryInfo::new("aacparse")
            .plugin_name("audioparsers")
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
        GstFactoryInfo::new("vtenc_h265")
            .plugin_name("applemedia")
            .package("GStreamer Bad Plug-ins source release")
            .license("LGPL"),
        GstFactoryInfo::new("vtenc_prores")
            .plugin_name("applemedia")
            .package("GStreamer Bad Plug-ins source release")
            .license("LGPL"),
    ];

    for factory in factories {
        assert_eq!(
            evaluate_gstreamer_factory(&factory).verdict,
            PluginPolicyVerdict::Allowed,
            "{factory:?}"
        );
    }
}

#[test]
fn plugin_policy_denies_unreviewed_aac_and_libav_factories() {
    let factories = [
        GstFactoryInfo::new("x264enc").plugin_name("x264").license("GPL"),
        GstFactoryInfo::new("avenc_aac").plugin_name("libav").license("LGPL"),
        GstFactoryInfo::new("avenc_aac_at").plugin_name("libav").license("LGPL"),
        GstFactoryInfo::new("fdkaacenc").plugin_name("fdkaac").license("LGPL"),
        GstFactoryInfo::new("faac").plugin_name("faac").license("LGPL"),
        GstFactoryInfo::new("voaacenc").plugin_name("voaacenc").license("LGPL"),
    ];

    for factory in factories {
        assert_ne!(
            evaluate_gstreamer_factory(&factory).verdict,
            PluginPolicyVerdict::Allowed,
            "{factory:?}"
        );
    }
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline plugin_policy_allows_reviewed_native_export_factories -- --test-threads=1 --nocapture
```

Expected: fails for at least the new allowed factories.

- [ ] **Step 3: Implement policy updates**

Update `is_known_denied_factory` to deny:

```rust
name == "x264enc"
    || name.starts_with("avenc_")
    || name.starts_with("avdec_")
    || name.contains("fdkaac")
    || matches!(name, "faac" | "voaacenc" | "openh264enc")
```

Add package families:

```rust
const GSTREAMER_BAD_PACKAGES: &[&str] =
    &["gstreamerbadplugins", "gstreamerbadpluginssourcerelease"];
```

Add `AllowedFactoryPolicy` entries for `mp4mux`, `qtmux`, `aacparse`, `atenc`, `vtenc_h264`, `vtenc_h265`, and `vtenc_prores` with exact plugin names from the spec.

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline plugin_policy_allows_reviewed_native_export_factories -- --test-threads=1 --nocapture
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline plugin_policy_denies_unreviewed_aac_and_libav_factories -- --test-threads=1 --nocapture
```

Expected: both tests pass.

- [ ] **Step 5: Commit**

```bash
rtk git add src-tauri/src/render_pipeline/plugin_policy.rs src-tauri/tests/render_pipeline.rs
rtk git -c commit.gpgsign=false commit -m "feat: allow reviewed native gstreamer factories"
```

## Task 3: GStreamer Backend Profile Selection

**Files:**
- Modify: `src-tauri/src/render_pipeline/gstreamer_backend.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing backend tests**

Add tests that call a test-only summary helper for MP4 and ProRes targets:

```rust
#[cfg(feature = "ges-render")]
#[test]
fn mp4_h264_encoding_profile_uses_native_factories() {
    let summary = encoding_profile_summary_for_test(
        &render_plan(),
        ExportProfile::Mp4H264,
    )
    .expect("mp4 h264 profile summary");

    assert_eq!(summary.container_factory, "mp4mux");
    assert_eq!(summary.video_factory, "vtenc_h264");
    assert_eq!(summary.audio_factory.as_deref(), Some("atenc"));
    assert_eq!(summary.parser_factories, vec!["aacparse"]);
}

#[cfg(feature = "ges-render")]
#[test]
fn prores_encoding_profile_uses_quicktime_and_pcm_audio() {
    let summary = encoding_profile_summary_for_test(
        &render_plan(),
        ExportProfile::ProResMov,
    )
    .expect("prores profile summary");

    assert_eq!(summary.container_factory, "qtmux");
    assert_eq!(summary.video_factory, "vtenc_prores");
    assert_eq!(summary.audio_factory, None);
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline mp4_h264_encoding_profile_uses_native_factories -- --test-threads=1 --nocapture
```

Expected: compile fails because the helper/profile support does not exist.

- [ ] **Step 3: Add generic encoding profile builder**

Refactor `webm_encoding_profile(plan)` into `encoding_profile(plan, target)` while preserving the existing WebM helper as a wrapper.

Use `GstreamerOutputProfileTarget` for:

- container caps
- video caps
- audio caps
- required factories
- output extension validation

Keep WebM properties from `webm_encoder_settings` unchanged.

- [ ] **Step 4: Update render path validation**

Replace `validate_webm_output(plan)` with profile-aware validation:

```rust
fn validate_output_path_for_target(
    output_path: &str,
    target: &GstreamerOutputProfileTarget,
) -> PipelineResult<()>
```

It should reject mismatched extensions and include expected/actual extension in details.

- [ ] **Step 5: Run tests to verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline mp4_h264_encoding_profile_uses_native_factories -- --test-threads=1 --nocapture
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline prores_encoding_profile_uses_quicktime_and_pcm_audio -- --test-threads=1 --nocapture
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline webm_encoding_profile -- --test-threads=1 --nocapture
```

Expected: new profile tests pass and existing WebM profile tests still pass.

- [ ] **Step 6: Commit**

```bash
rtk git add src-tauri/src/render_pipeline/gstreamer_backend.rs src-tauri/tests/render_pipeline.rs
rtk git -c commit.gpgsign=false commit -m "feat: build gstreamer encoding profiles by export target"
```

## Task 4: Profile-Neutral Project Export

**Files:**
- Modify: `src-tauri/src/render_pipeline/project_export.rs`
- Test: `src-tauri/tests/project_export.rs`

- [ ] **Step 1: Write failing project export tests**

Add tests:

```rust
#[test]
fn project_media_render_paths_use_profile_extension() {
    let mp4 = ProjectMediaRenderPaths::new("export-mp4", ExportProfile::Mp4H264)
        .expect("mp4 paths");
    assert_eq!(mp4.output_path, "renders/export-mp4/output.mp4");

    let mov = ProjectMediaRenderPaths::new("export-prores", ExportProfile::ProResMov)
        .expect("mov paths");
    assert_eq!(mov.output_path, "renders/export-prores/output.mov");
}

#[test]
fn project_media_render_plan_uses_profile_output_path() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let project = sample_project();

    let plan = build_project_media_render_plan(
        dir.path(),
        &project,
        "export-mp4",
        RenderQualityProfile::FinalWebm,
        ExportProfile::Mp4H264,
    )
    .expect("mp4 render plan");

    assert_eq!(
        plan.output_path,
        dir.path().join("renders/export-mp4/output.mp4").display().to_string()
    );
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_export project_media_render_paths_use_profile_extension -- --test-threads=1 --nocapture
```

Expected: compile fails because profile-neutral project export types do not exist.

- [ ] **Step 3: Implement profile-neutral paths and plan builder**

Add `ProjectMediaRenderPaths` and `ProjectMediaRenderResult`. Keep `ProjectWebmRenderPaths` and `ProjectWebmRenderResult` as compatibility aliases/wrappers.

Add `build_project_media_render_plan(project_dir, project, job_id, quality, export_profile)` and have existing WebM builder call it with a WebM-compatible target.

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_export project_media_render_paths_use_profile_extension -- --test-threads=1 --nocapture
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_export project_media_render_plan_uses_profile_output_path -- --test-threads=1 --nocapture
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_export -- --test-threads=1
```

Expected: new tests and existing project export tests pass.

- [ ] **Step 5: Commit**

```bash
rtk git add src-tauri/src/render_pipeline/project_export.rs src-tauri/tests/project_export.rs
rtk git -c commit.gpgsign=false commit -m "feat: make project exports profile-neutral"
```

## Task 5: Export Availability From GStreamer Policy

**Files:**
- Modify: `src-tauri/src/project/export_profiles.rs`
- Test: `src-tauri/tests/export_profiles.rs`

- [ ] **Step 1: Write failing availability tests**

Update tests so MP4/ProRes profiles report GStreamer factories:

```rust
#[test]
fn mp4_profiles_report_gstreamer_factory_requirements() {
    let _guard = EXPORT_PROFILE_ENV_LOCK.lock().expect("export env lock");
    let temp = tempfile::tempdir().expect("runtime path");
    let _env = IsolatedExportRuntimeEnv::new(temp.path());
    let report = mp4_export_profile_availability_report();

    let h264 = report
        .iter()
        .find(|profile| profile.profile == ExportProfile::Mp4H264)
        .expect("h264 profile");

    assert_eq!(
        h264.required_runtime,
        vec![
            "gstreamer:mp4mux".to_string(),
            "gstreamer:vtenc_h264".to_string(),
            "gstreamer:atenc".to_string(),
            "gstreamer:aacparse".to_string(),
        ]
    );
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test export_profiles mp4_profiles_report_gstreamer_factory_requirements -- --test-threads=1 --nocapture
```

Expected: fails because current required runtime names are external encoder command names.

- [ ] **Step 3: Implement GStreamer-backed profile report**

Use `gstreamer_output_profile_target` for MP4/ProRes profiles. `required_runtime` should be `gstreamer:<factory>`. Availability should be false when the factory is missing or denied, with an actionable reason.

Keep `PalmierProject` behavior separate.

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test export_profiles -- --test-threads=1
```

Expected: export profile tests pass.

- [ ] **Step 5: Commit**

```bash
rtk git add src-tauri/src/project/export_profiles.rs src-tauri/tests/export_profiles.rs
rtk git -c commit.gpgsign=false commit -m "feat: derive export availability from gstreamer policy"
```

## Task 6: Tauri And Temporal Native Export Routing

**Files:**
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/workflows/mod.rs`
- Modify: `src-tauri/src/render_pipeline/project_export.rs`
- Test: `src-tauri/tests/temporal_workflows.rs`

- [ ] **Step 1: Write failing Temporal tests**

Add tests proving codec profile export does not build an external encoder command and instead plans direct native render:

```rust
#[test]
fn temporal_export_media_codec_profile_uses_native_gstreamer_render() {
    let start_request = temporal_export_media_start_request(
        "project-1",
        "/tmp/project",
        "export-mp4",
        ExportProfile::Mp4H264,
        "exports/project-1.mp4",
    );

    let plan = temporal_export_media_workflow_activity_plan_value(
        start_request["args"][0].clone(),
        "2026-07-03T00:00:00Z",
        Some("run-1"),
    )
    .expect("activity plan");

    assert_eq!(plan["profile"], "mp4H264");
    assert_eq!(plan["renderMediaInput"]["exportProfile"], "mp4H264");
    assert!(plan.get("writeExportArtifactBaseInput").is_none());
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows temporal_export_media_codec_profile_uses_native_gstreamer_render -- --test-threads=1 --nocapture
```

Expected: fails because codec profile export currently uses write artifact / external encoder flow.

- [ ] **Step 3: Add profile-neutral Tauri command**

Expose `render_media_to_split_project_folder(projectDir, profile, jobId, updatedAt, rangeStartSeconds, rangeEndSeconds)`.

Keep `render_webm_to_split_project_folder` as a wrapper for the existing UI until frontend migration.

- [ ] **Step 4: Update Temporal activity plan**

For `Mp4H264`, `Mp4H265`, and `ProResMov`, plan direct build/render/validate/attach using the shared project export implementation. Do not include external encoder command inputs for these profiles.

- [ ] **Step 5: Run tests to verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows temporal_export_media_codec_profile_uses_native_gstreamer_render -- --test-threads=1 --nocapture
```

Expected: test passes.

- [ ] **Step 6: Commit**

```bash
rtk git add src-tauri/src/main.rs src-tauri/src/workflows/mod.rs src-tauri/src/render_pipeline/project_export.rs src-tauri/tests/temporal_workflows.rs
rtk git -c commit.gpgsign=false commit -m "feat: route codec exports through native gstreamer render"
```

## Task 7: Frontend Command And Availability Wiring

**Files:**
- Modify: `src/lib/project.ts`
- Modify: `src/lib/render.ts`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/render-quality-control.tsx`
- Test: `src/lib/project.test.ts`
- Test: `src/lib/render.test.ts`
- Test: `src/components/workspace/editor-workspace.test.tsx`

- [ ] **Step 1: Write failing frontend tests**

Update tests to assert MP4/ProRes export buttons call the native render command or native Temporal render path, not the external encoder workflow. Update render model tests to include native profile labels.

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk pnpm test src/lib/render.test.ts src/lib/project.test.ts src/components/workspace/editor-workspace.test.tsx
```

Expected: tests fail because frontend still assumes external codec export workflow.

- [ ] **Step 3: Add TypeScript bridge**

Add `renderMediaToSplitProjectFolder` in `src/lib/project.ts` with the profile-neutral command payload. Keep `renderWebmToSplitProjectFolder` for compatibility.

- [ ] **Step 4: Update UI export behavior**

Use availability reports to enable MP4/ProRes. Make MP4/ProRes export actions call the native profile path. Keep disabled copy based on `unavailableReason`.

- [ ] **Step 5: Run tests to verify GREEN**

Run:

```bash
rtk pnpm test src/lib/render.test.ts src/lib/project.test.ts src/components/workspace/editor-workspace.test.tsx
```

Expected: frontend tests pass.

- [ ] **Step 6: Commit**

```bash
rtk git add src/lib/project.ts src/lib/render.ts src/components/workspace/editor-workspace.tsx src/components/workspace/render-quality-control.tsx src/lib/project.test.ts src/lib/render.test.ts src/components/workspace/editor-workspace.test.tsx
rtk git -c commit.gpgsign=false commit -m "feat: wire native gstreamer export profiles in ui"
```

## Task 8: Final Verification

**Files:**
- Existing source and tests only.

- [ ] **Step 1: Run targeted Rust tests**

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline -- --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_export -- --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml --test export_profiles -- --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows -- --test-threads=1
```

Expected: all targeted Rust tests pass.

- [ ] **Step 2: Run targeted frontend tests**

```bash
rtk pnpm test src/lib/render.test.ts src/lib/project.test.ts src/components/workspace/editor-workspace.test.tsx
```

Expected: all targeted frontend tests pass.

- [ ] **Step 3: Inspect final worktree**

```bash
rtk git status --short
rtk git diff --stat
```

Expected: only intentional tracked implementation changes remain. Pre-existing untracked `docs/parity.md`, `palmier-pro/`, and `src-tauri/native/fluidaudio-parakeet/.build/` remain untouched unless separately requested.

## Self-Review

- Spec coverage: profile metadata, MP4 `atenc` audio, ProRes PCM, plugin policy, backend selection, project export, availability, Temporal, UI, and verification are covered.
- Placeholder scan: no implementation step is deferred without instructions.
- Type consistency: plan uses existing `ExportProfile` values for delivery profiles and keeps existing WebM quality profile compatibility where needed.
