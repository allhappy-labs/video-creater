# Independent Export Codec and Quality Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users select an available video codec and Draft or Final quality independently, with Draft defaulting to HD while retaining a user-controlled resolution override.

**Architecture:** Add a typed export-options contract containing codec, quality, and requested dimensions. Map legacy `draftWebm` and `finalWebm` workflow values at the boundary, then pass the new contract through Tauri, Temporal, render-plan construction, backend routing, validation, artifact metadata, and the export dialog. AVFoundation and GStreamer use the chosen dimensions and quality settings; ProRes Draft requires an explicit native ProRes Proxy capability.

**Tech Stack:** React, TypeScript, Vitest, Tauri, Rust, Temporal, GStreamer/GES, AVFoundation/Swift, Playwright CLI.

## Global Constraints

- Codec and quality are independent for every Video export.
- Draft defaults to at most 1280×720, preserves aspect ratio, and never upscales.
- Resolution is always user-editable; an explicit selection survives codec and Draft/Final changes.
- Final defaults to Match Timeline only when no explicit resolution override exists.
- Every render remains EDL-first and records duration, stream presence, codec/container, dimensions, artifacts, and logs.
- Native capability checks are fail-closed; do not silently substitute a codec or ProRes variant.
- Browser visual QA must use an explicit healthy native-capability fixture; only native verification proves actual codec support.
- Prefix every shell command with `rtk`.

---

## File Structure

- Create: `src-tauri/src/project/export_options.rs` — serialized export codec/quality/dimension request, legacy-profile adapter, dimension validation, and unit tests.
- Modify: `src-tauri/src/project/mod.rs` — export the new module.
- Modify: `src-tauri/src/edit/render_plan.rs` — replace the combined quality field on new plans with `RenderQuality` and retain the legacy enum only at compatibility boundaries.
- Modify: `src-tauri/src/render_pipeline/quality.rs` — calculate Draft and Final dimensions, bitrate, and speed hints from the independent quality and requested dimensions.
- Modify: `src-tauri/src/project/export_profiles.rs` — report per-quality availability, including native ProRes Proxy for Draft.
- Modify: `src-tauri/src/render_pipeline/avfoundation_backend.rs` — version the helper protocol, select a profile by codec plus quality, and preserve the selected dimensions.
- Modify: `src-tauri/native/avfoundation-export/Sources/main.swift` — expose ProRes Proxy capability and use a Draft/Final profile mapping.
- Modify: `src-tauri/src/render_pipeline/project_export.rs` — build and validate plans from `ExportRenderOptions` and carry selected dimensions into reports.
- Modify: `src-tauri/src/workflows/mod.rs`, `src-tauri/src/main.rs` — accept the new contract, map legacy workflow input, and persist quality/dimensions in Temporal input and artifact validation.
- Modify: `src/lib/project.ts` and `src/lib/project.test.ts` — publish matching TypeScript types and Tauri command arguments.
- Modify: `src/components/workspace/export-sheet.tsx` and `src/components/workspace/export-sheet.test.tsx` — render independent controls, resolution defaults/overrides, and capability states.
- Modify: `src/components/workspace/editor-workspace.tsx` and `src/components/workspace/editor-workspace.test.tsx` — load capability state, build the typed request, and queue the chosen export.
- Modify: `scripts/browser-visual-qa.mjs` and `src/browser-visual-qa-script.test.ts` — inject a healthy export-capability fixture before the browser page loads and assert the ready export surface.

### Task 1: Define the independent export contract and quality settings

**Files:**
- Create: `src-tauri/src/project/export_options.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/edit/render_plan.rs`
- Modify: `src-tauri/src/render_pipeline/quality.rs`

**Interfaces:**
- Consumes: existing `ExportProfile`, `RenderQuality`, and project render settings.
- Produces: `ExportRenderOptions { profile, quality, width, height }`, `RenderQuality::Draft | Final`, `render_quality_from_legacy`, and deterministic effective quality settings.

- [ ] **Step 1: Write failing Rust tests for the contract and HD default calculation**

```rust
#[test]
fn draft_defaults_a_4k_request_to_aspect_preserving_hd() {
    assert_eq!(draft_dimensions(3840, 2160), (1280, 720));
}

#[test]
fn explicit_resolution_is_not_changed_by_draft_quality() {
    let options = ExportRenderOptions::new(
        ExportProfile::Mp4H264,
        RenderQuality::Draft,
        1920,
        1080,
    ).expect("explicit full-hd request is valid");
    assert_eq!((options.width, options.height), (1920, 1080));
}

#[test]
fn legacy_final_webm_maps_to_final_quality_and_webm_delivery() {
    assert_eq!(legacy_render_profile_options(RenderQualityProfile::FinalWebm).quality, RenderQuality::Final);
}
```

- [ ] **Step 2: Run the focused tests and confirm they fail because the contract does not exist**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml export_options`

Expected: FAIL with unresolved `ExportRenderOptions`, `draft_dimensions`, and `legacy_render_profile_options` symbols.

- [ ] **Step 3: Add the typed contract and migration adapter**

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExportRenderOptions {
    pub profile: ExportProfile,
    pub quality: RenderQuality,
    pub width: u32,
    pub height: u32,
}

pub fn legacy_render_profile_options(profile: RenderQualityProfile) -> ExportRenderOptions {
    let quality = match profile {
        RenderQualityProfile::DraftWebm => RenderQuality::Draft,
        RenderQualityProfile::FinalWebm => RenderQuality::Final,
    };
    ExportRenderOptions::new(ExportProfile::Webm, quality, 1280, 720)
        .expect("legacy WebM dimensions are valid")
}
```

Move the output-profile identifier into `ExportProfile` so it includes `Webm`; keep `PalmierProject` outside `ExportRenderOptions`. Add `ExportRenderOptions::new` validation for positive, even dimensions no larger than 16,384 pixels. Store `RenderQuality` directly on newly created `RenderPlan` values; retain `RenderQualityProfile` only on the legacy adapter and existing WebM command entry points until Task 3 removes those callers.

Replace `effective_quality_settings(RenderQualityProfile, …)` with `effective_quality_settings(RenderQuality, width, height, fps)`. Draft settings must retain supplied dimensions, use `draft-fast`, cap FPS at 24, and derive a lower bitrate; the UI, rather than the encoder, chooses the default HD dimensions.

- [ ] **Step 4: Run formatting and focused Rust tests**

Run: `rtk rustfmt --edition 2021 src-tauri/src/project/export_options.rs src-tauri/src/edit/render_plan.rs src-tauri/src/render_pipeline/quality.rs`

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml export_options`

Expected: PASS, including legacy mapping, no-upscale Draft default, explicit HD/Full-HD override, and Final settings tests.

- [ ] **Step 5: Commit the contract slice**

```bash
rtk git add src-tauri/src/project/export_options.rs src-tauri/src/project/mod.rs src-tauri/src/edit/render_plan.rs src-tauri/src/render_pipeline/quality.rs
rtk git commit -m "feat(export): separate codec from render quality"
```

### Task 2: Make native render backends quality-aware and capability-accurate

**Files:**
- Modify: `src-tauri/src/project/export_profiles.rs`
- Modify: `src-tauri/src/render_pipeline/avfoundation_backend.rs`
- Modify: `src-tauri/native/avfoundation-export/Sources/main.swift`
- Modify: `src-tauri/src/render_pipeline/gstreamer_backend.rs`
- Test: `src-tauri/tests/export_profiles.rs`

**Interfaces:**
- Consumes: `ExportRenderOptions` and `RenderQuality` from Task 1.
- Produces: a per-quality capability report and codec-plus-quality backend selection with no hidden fallback.

- [ ] **Step 1: Write failing tests for Draft/Final capabilities and backend profile selection**

```rust
#[test]
fn draft_prores_requires_proxy_capability() {
    let availability = export_profile_availability(ExportProfile::ProResMov);
    assert!(!availability.quality_available(RenderQuality::Draft));
    assert!(availability.draft_unavailable_reason().unwrap().contains("ProRes Proxy"));
}

#[test]
fn avfoundation_uses_proxy_only_for_draft_prores() {
    assert_eq!(avfoundation_profile(ExportProfile::ProResMov, RenderQuality::Draft), AvFoundationExportProfile::ProResProxy);
    assert_eq!(avfoundation_profile(ExportProfile::ProResMov, RenderQuality::Final), AvFoundationExportProfile::ProRes422);
}
```

- [ ] **Step 2: Run the focused tests and confirm they fail**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml export_profiles::tests::draft_prores`

Expected: FAIL because quality-specific availability and `ProResProxy` are absent.

- [ ] **Step 3: Extend the capability and backend protocols**

Use one version bump for `AVFOUNDATION_EXPORT_PROTOCOL_VERSION` and Swift `protocolVersion`. Add `quality: draft | final` to the Rust `AvFoundationExportRequest` and Swift `ExportRequest`. Make the Swift profile mapping explicit:

```swift
private enum ExportProfile: String, Decodable {
    case h264
    case hevc
    case proResProxy
    case proRes422
}

private func profile(for codec: ExportProfile, quality: RenderQuality) -> ExportProfile {
    if codec == .proRes422 && quality == .draft { return .proResProxy }
    return codec
}
```

Report `proResProxy` separately from `proRes422` in `--capabilities`. In Rust, expose `qualityAvailability: { draft, final }` and explicit quality-scoped reasons on `ExportProfileAvailability`. H.264, HEVC, and WebM use the same available codec path for both qualities; Draft achieves lower effort through Task 1 dimensions and GStreamer bitrate/settings. ProRes Draft is disabled when `proResProxy` is absent.

Update GStreamer encoding profiles to pass `effective_quality_settings(plan.quality, plan.width, plan.height, plan.fps)` into all supported video factories. Preserve the selected container/codec and change only encoder bitrate/effort. Do not allow a quality setting to change H.264 into HEVC, or substitute a different file type.

- [ ] **Step 4: Verify helper protocol and Rust test coverage**

Run: `rtk pnpm build:avfoundation-exporter:dev`

Run: `rtk ./src-tauri/target/debug/video-creater-avfoundation-exporter --capabilities`

Expected: JSON includes `h264`, `hevc`, `proResProxy`, and `proRes422` capability keys and the new protocol version.

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml export_profiles`

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml avfoundation_backend`

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml gstreamer_backend`

Expected: PASS with Draft/Final selection, no-fallback, and output-profile tests.

- [ ] **Step 5: Commit the backend capability slice**

```bash
rtk git add src-tauri/src/project/export_profiles.rs src-tauri/src/render_pipeline/avfoundation_backend.rs src-tauri/native/avfoundation-export/Sources/main.swift src-tauri/src/render_pipeline/gstreamer_backend.rs src-tauri/tests/export_profiles.rs
rtk git commit -m "feat(export): add quality-aware native profiles"
```

### Task 3: Carry codec, quality, and requested dimensions through Tauri and Temporal

**Files:**
- Modify: `src-tauri/src/render_pipeline/project_export.rs`
- Modify: `src-tauri/src/workflows/mod.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/lib/project.ts`
- Test: `src/lib/project.test.ts`
- Test: `src-tauri/tests/project_export.rs`

**Interfaces:**
- Consumes: `ExportRenderOptions` and per-quality availability from Tasks 1–2.
- Produces: `build_temporal_export_media_start_request({ profile, quality, width, height, outputPath })` and a render report containing requested/actual dimensions and selected quality.

- [ ] **Step 1: Add failing Tauri/Temporal payload tests**

```ts
await buildTemporalExportMediaStartRequest({
  projectId: "project-1",
  projectDir: "/tmp/project",
  jobId: "h264-draft-1",
  profile: "mp4H264",
  quality: "draft",
  width: 1280,
  height: 720,
  outputPath: "exports/project-1-h264-draft.mp4",
});

expect(invoke).toHaveBeenCalledWith("build_temporal_export_media_start_request", expect.objectContaining({
  profile: "mp4H264", quality: "draft", width: 1280, height: 720,
}));
```

Add a Rust workflow test that passes legacy `{ profile: "finalWebm" }` and asserts the compatibility adapter creates `{ profile: Webm, quality: Final }`. Add a media-render test that requests H.264 Draft at 1920×1080 and asserts the generated plan and final validation expect 1920×1080, proving an explicit user override is honoured.

- [ ] **Step 2: Run focused tests and confirm they fail**

Run: `rtk pnpm vitest run src/lib/project.test.ts`

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml temporal_export_media`

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml project_export`

Expected: FAIL because the public command and workflow input lack quality and dimensions.

- [ ] **Step 3: Implement the end-to-end request contract**

Change the Tauri command and TypeScript wrapper to accept exactly these fields:

```ts
export interface ExportMediaStartInput {
  projectId: string;
  projectDir: string;
  jobId: string;
  profile: Exclude<ExportProfile, "palmierProject">;
  quality: ExportQuality;
  width: number;
  height: number;
  outputPath: string;
}
```

Define `export type ExportQuality = "draft" | "final";` beside the TypeScript export-profile types and use that alias in the sheet state and request wrapper.

Pass the decoded `ExportRenderOptions` into `temporal_export_media_start_request`, `BuildRenderPlan`, `ValidateExportProfile`, `RenderMedia`, `ValidateRenderedMedia`, and `AttachRenderReport`. Build plans using the request width and height instead of overwriting them with `project.render_settings`. Validate the extension, container, video stream, audio requirement, and actual dimensions against the selected options. Add `quality`, `requestedWidth`, `requestedHeight`, `actualWidth`, and `actualHeight` to the persisted export report payload.

Keep only the legacy WebM entry points accepting `RenderQualityProfile`; translate immediately with `legacy_render_profile_options` before constructing a plan. New Temporal ExportMedia requests must never serialize `draftWebm` or `finalWebm`.

- [ ] **Step 4: Run contract and native-plan verification**

Run: `rtk pnpm vitest run src/lib/project.test.ts`

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml temporal_export_media`

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml project_export`

Expected: PASS; assertions show Draft/Final and requested dimensions in the Tauri input, Temporal workflow input, render plan, and validation report.

- [ ] **Step 5: Commit the workflow slice**

```bash
rtk git add src-tauri/src/render_pipeline/project_export.rs src-tauri/src/workflows/mod.rs src-tauri/src/main.rs src/lib/project.ts src/lib/project.test.ts src-tauri/tests/project_export.rs
rtk git commit -m "feat(export): pass codec quality and resolution through workflows"
```

### Task 4: Rebuild the export dialog around independent controls and honest capability state

**Files:**
- Modify: `src/components/workspace/export-sheet.tsx`
- Modify: `src/components/workspace/export-sheet.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

**Interfaces:**
- Consumes: `ExportMediaStartInput`, `ExportProfileAvailability.qualityAvailability`, and selected project dimensions.
- Produces: an `ExportSheetSelection` with `profileId`, `quality`, `width`, `height`, and `resolutionOverridden`.

- [ ] **Step 1: Write failing frontend tests for the user-visible behavior**

```tsx
fireEvent.change(screen.getByRole("combobox", { name: "Codec" }), {
  target: { value: "mp4H265" },
});
expect(screen.getByRole("combobox", { name: "Quality" })).toHaveValue("draft");
expect(screen.getByRole("combobox", { name: "Resolution" })).toHaveValue("hd");

fireEvent.change(screen.getByRole("combobox", { name: "Resolution" }), {
  target: { value: "full-hd" },
});
fireEvent.change(screen.getByRole("combobox", { name: "Quality" }), {
  target: { value: "final" },
});
expect(screen.getByRole("combobox", { name: "Resolution" })).toHaveValue("full-hd");
```

Add tests for: Codec options named `H.264`, `H.265 (HEVC)`, `ProRes`, and `WebM`; Draft ProRes disabled with its Proxy capability reason; neutral checking copy before the availability request resolves; actual failure copy after rejection; and submit payload `{ quality, width, height }`.

- [ ] **Step 2: Run focused UI tests and confirm they fail**

Run: `rtk pnpm vitest run src/components/workspace/export-sheet.test.tsx src/components/workspace/editor-workspace.test.tsx`

Expected: FAIL because `ExportSheetSelection` has no independent quality/dimensions and the initial fallback is rendered as a GStreamer warning.

- [ ] **Step 3: Implement the selection state machine**

Use this resolution transition function in the sheet:

```ts
function defaultResolutionForQuality(
  quality: ExportQuality,
  timeline: Resolution,
  overridden: boolean,
  current: Resolution,
): Resolution {
  if (overridden) return current;
  if (quality === "final") return "match-timeline";
  return timeline.width > 1280 || timeline.height > 720 ? "hd" : "match-timeline";
}
```

Show a single `Quality` row for every video codec. Keep `Resolution` enabled. Mark `resolutionOverridden` true only after the user changes the Resolution selector; codec and quality changes call the function above and therefore preserve an override. Display the actual selected container/extension and dimensions in the footer.

Replace `fallbackExportProfileAvailability` with state `{ status: "checking" | "ready" | "unavailable", profiles }`. During checking, disable only capability-dependent choices and display `Checking native export capabilities…`; after a command failure, retain the choices disabled and show `Native export capabilities are unavailable in this app session.` Do not mention GStreamer unless the native report itself provides that as its concrete reason.

Update `EditorWorkspace.submitExportSheet` to construct `ExportMediaStartInput`, then display `H.264 Draft queued` style status text using the actual quality and resolution.

- [ ] **Step 4: Run focused UI tests and static validation**

Run: `rtk pnpm vitest run src/components/workspace/export-sheet.test.tsx src/components/workspace/editor-workspace.test.tsx`

Run: `rtk pnpm lint`

Expected: PASS with keyboard-accessible controls, Draft HD default, persistent override, capability-state copy, and typed Temporal request coverage.

- [ ] **Step 5: Commit the UI slice**

```bash
rtk git add src/components/workspace/export-sheet.tsx src/components/workspace/export-sheet.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat(export): add independent codec and quality controls"
```

### Task 5: Make visual QA representative and validate real render artifacts

**Files:**
- Modify: `scripts/browser-visual-qa.mjs`
- Modify: `src/browser-visual-qa-script.test.ts`
- Modify: `src-tauri/tests/project_export.rs`
- Modify: `docs/parity.md`

**Interfaces:**
- Consumes: ready-state export capability fixture and quality-aware render report from Tasks 2–4.
- Produces: a browser export screenshot with usable codec/quality controls and native fixture evidence for approved Draft/Final paths.

- [ ] **Step 1: Write failing visual-QA fixture and artifact-validation tests**

```ts
expect(scriptSource).toContain("__VIDEO_CREATER_VISUAL_EXPORT_CAPABILITIES__");
expect(scriptSource).toContain('qualityAvailability: { draft: true, final: true }');
```

```rust
assert_eq!(report.requested_quality, RenderQuality::Draft);
assert_eq!((report.actual_width, report.actual_height), (1280, 720));
assert!(report.streams.video);
```

- [ ] **Step 2: Run the focused tests and confirm they fail**

Run: `rtk pnpm vitest run src/browser-visual-qa-script.test.ts`

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml project_export`

Expected: FAIL because the browser runner has no explicit capability fixture and reports lack requested/actual quality dimensions.

- [ ] **Step 3: Inject the ready capability fixture and add evidence checks**

Before navigation in `browser-visual-qa.mjs`, use Playwright `page.addInitScript` to define `window.__VIDEO_CREATER_VISUAL_EXPORT_CAPABILITIES__` with approved H.264, HEVC, ProRes Draft/Final where supported by the fixture, and WebM Draft/Final. In `getExportProfileAvailabilityReport`, read this value only when present; production desktop calls continue to use Tauri `invoke`.

Update the export visual assertion to require visible Codec, Quality, Resolution, File Type, and the absence of the checking/unavailable list after the fixture resolves. Update `docs/parity.md` with the verification date, fixture limitation, and the requirement that native tests—not the browser image—prove codecs and streams.

- [ ] **Step 4: Run visual and native verification**

Run: `rtk pnpm vitest run src/browser-visual-qa-script.test.ts`

Run: `rtk pnpm dev`

In a second terminal, run: `rtk pnpm visual:qa:browser -- --only export::desktop --out output/visual-qa/export-codec-quality`

Expected: `output/visual-qa/export-codec-quality/export-video-dialog-desktop.png` shows `WebM` as a codec, separate Draft/Final quality, a selectable Resolution, and no unfinished-GStreamer warning.

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml project_export`

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml export_profiles`

Run: `rtk pnpm build:avfoundation-exporter:dev`

Run: `rtk ./src-tauri/target/debug/video-creater-avfoundation-exporter --capabilities`

Expected: PASS. The native report and fixture render evidence agree on every capability that is enabled in the desktop export dialog.

- [ ] **Step 5: Commit the QA and documentation slice**

```bash
rtk git add scripts/browser-visual-qa.mjs src/browser-visual-qa-script.test.ts src-tauri/tests/project_export.rs docs/parity.md
rtk git commit -m "test(export): capture ready codec quality dialog"
```
