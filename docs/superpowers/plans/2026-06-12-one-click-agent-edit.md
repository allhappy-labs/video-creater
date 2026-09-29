# One-Click Agent Edit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the MVP one-click agent edit pipeline: preset + prompt request, Parakeet transcript boundary, EDL-first timeline generation, render-plan validation, UI controls, and deterministic E2E validation.

**Architecture:** Rust owns the canonical domain, edit request validation, transcript parsing, rough-cut EDL, timeline mutation, render-plan construction, and ffmpeg command construction. The Python Parakeet runner is represented by a narrow command boundary for MVP, and the UI sends preset/prompt requests through typed app contracts.

**Tech Stack:** Rust 1.77, Tauri 2, React 19, TypeScript, shadcn/ui, Vitest, Cargo tests, ffmpeg/ffprobe command contracts.

---

## File Structure

- Create `src-tauri/src/edit/mod.rs`: module exports.
- Create `src-tauri/src/edit/preset.rs`: `EditPreset`, `CaptionStyle`, `LanguageMode`, duration defaults, prompt validation.
- Create `src-tauri/src/edit/transcript.rs`: Parakeet transcript JSON parser and caption grouping helpers.
- Create `src-tauri/src/edit/edl.rs`: candidate moment model, rough-cut EDL generation, EDL validation.
- Create `src-tauri/src/edit/render_plan.rs`: render plan structs and ffmpeg command construction.
- Create `src-tauri/tests/one_click_edit.rs`: Rust integration tests for preset validation, transcript parsing, EDL guardrails, and render plans.
- Modify `src-tauri/src/lib.rs`: expose `edit`.
- Modify `src-tauri/src/main.rs`: add Tauri command for one-click edit generation.
- Modify `src-tauri/src/project/model.rs`: add transcript metadata fields and generated artifact/job request fields in a backwards-compatible way.
- Modify `src/lib/timeline.ts`: add edit request and preset TypeScript types.
- Create `src/lib/edit.ts`: frontend request helpers and preset metadata.
- Create `src/lib/edit.test.ts`: frontend tests for request payload and preset defaults.
- Modify `src/components/workspace/agent-panel.tsx`: preset selector, prompt textarea, generate button, job summary UI.
- Modify `src/components/workspace/editor-workspace.tsx`: connect agent panel to local generated state for MVP.
- Modify `scripts/e2e-combined-video.mjs`: add deterministic one-click edit validation coverage.

## Task 1: Rust Edit Domain

**Files:**
- Create: `src-tauri/src/edit/mod.rs`
- Create: `src-tauri/src/edit/preset.rs`
- Test: `src-tauri/tests/one_click_edit.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing preset tests**

Add tests:

```rust
#[test]
fn edit_presets_have_expected_duration_ranges() {
    assert_eq!(EditPreset::TrailerCut.duration_range(), (30.0, 60.0));
    assert_eq!(EditPreset::HighlightReel.duration_range(), (45.0, 90.0));
    assert_eq!(EditPreset::StoryCut.duration_range(), (90.0, 180.0));
}

#[test]
fn edit_job_request_rejects_empty_prompt_and_bad_target_duration() {
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "   ".to_string(),
        target_duration_seconds: Some(500.0),
        language_mode: LanguageMode::Auto,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    assert_eq!(
        request.validate(),
        Err(EditRequestError::PromptRequired)
    );
}
```

- [ ] **Step 2: Run test to verify red**

Run: `cargo test --manifest-path src-tauri/Cargo.toml edit_presets_have_expected_duration_ranges edit_job_request_rejects_empty_prompt_and_bad_target_duration`

Expected: compile failure because `edit` module/types do not exist.

- [ ] **Step 3: Implement preset/request domain**

Implement:

- `EditPreset::{TrailerCut, HighlightReel, StoryCut}`
- `duration_range()`
- `default_prompt()`
- `EditJobRequest`
- `LanguageMode`
- `CaptionStyle`
- `EditRequestError`
- `EditJobRequest::validate()`

- [ ] **Step 4: Run green**

Run: `cargo test --manifest-path src-tauri/Cargo.toml edit_presets_have_expected_duration_ranges edit_job_request_rejects_empty_prompt_and_bad_target_duration`

Expected: tests pass.

## Task 2: Transcript Parser And Caption Grouping

**Files:**
- Create: `src-tauri/src/edit/transcript.rs`
- Test: `src-tauri/tests/one_click_edit.rs`
- Modify: `src-tauri/src/edit/mod.rs`
- Modify: `src-tauri/src/project/model.rs`

- [ ] **Step 1: Write failing transcript tests**

Add tests that parse Parakeet-like JSON:

```rust
#[test]
fn parses_parakeet_words_from_runner_json() {
    let json = serde_json::json!({
        "model": "nvidia/parakeet-tdt-0.6b-v3",
        "text": "Сильная рука. Мощная рука.",
        "tokens": [
            {"token": "Сильная", "start": 1.0, "end": 1.4},
            {"token": " рука", "start": 1.4, "end": 1.8},
            {"token": ".", "start": 1.8, "end": 1.8},
            {"token": " Мощная", "start": 2.4, "end": 2.8},
            {"token": " рука", "start": 2.8, "end": 3.2}
        ]
    });

    let transcript = parse_parakeet_transcript("media-1", &json).expect("parse transcript");

    assert_eq!(transcript.media_id, "media-1");
    assert_eq!(transcript.engine.as_deref(), Some("nvidia/parakeet-tdt-0.6b-v3"));
    assert_eq!(transcript.words.len(), 5);
    assert_eq!(transcript.words[0].text, "Сильная");
}
```

- [ ] **Step 2: Run red**

Run: `cargo test --manifest-path src-tauri/Cargo.toml parses_parakeet_words_from_runner_json`

Expected: compile failure for missing parser/fields.

- [ ] **Step 3: Add transcript metadata and parser**

Add optional fields to `Transcript`:

- `engine: Option<String>`
- `raw_artifact_path: Option<String>`
- `segments: Vec<TranscriptSegment>` with `#[serde(default)]`

Implement `parse_parakeet_transcript()` and `group_words_into_caption_segments()`.

- [ ] **Step 4: Run green**

Run: `cargo test --manifest-path src-tauri/Cargo.toml parses_parakeet_words_from_runner_json`

Expected: pass.

## Task 3: Rough-Cut EDL Generator

**Files:**
- Create: `src-tauri/src/edit/edl.rs`
- Test: `src-tauri/tests/one_click_edit.rs`
- Modify: `src-tauri/src/edit/mod.rs`

- [ ] **Step 1: Write failing EDL tests**

Add tests:

```rust
#[test]
fn generated_edl_is_shorter_than_source_and_uses_source_ranges() {
    let words = sample_words_across_one_minute();
    let request = sample_edit_request(EditPreset::TrailerCut);

    let edl = build_rough_cut_edl(&request, "media-1", 120.0, &words).expect("build edl");

    assert!(edl.duration_seconds() <= 60.0);
    assert!(edl.duration_seconds() < 120.0);
    assert!(edl.clips.iter().all(|clip| clip.source_out > clip.source_in));
}

#[test]
fn edl_validation_rejects_full_source_pass_through() {
    let edl = RoughCutEdl {
        media_id: "media-1".to_string(),
        source_duration_seconds: 54.0,
        clips: vec![EdlClip {
            source_in: 0.0,
            source_out: 54.0,
            reason: "bad full pass".to_string(),
        }],
    };

    assert_eq!(
        validate_one_click_edl(&EditPreset::TrailerCut, &edl),
        Err(EdlError::FullSourcePassThrough)
    );
}
```

- [ ] **Step 2: Run red**

Run: `cargo test --manifest-path src-tauri/Cargo.toml generated_edl_is_shorter_than_source_and_uses_source_ranges edl_validation_rejects_full_source_pass_through`

Expected: compile failure for missing EDL module.

- [ ] **Step 3: Implement deterministic EDL builder**

Implement:

- `RoughCutEdl`
- `EdlClip`
- `build_rough_cut_edl()`
- `validate_one_click_edl()`

The MVP scorer groups dense transcript windows, expands each selected window with small handles, respects preset max duration, sorts by source time, and rejects full-source pass-through.

- [ ] **Step 4: Run green**

Run: `cargo test --manifest-path src-tauri/Cargo.toml generated_edl_is_shorter_than_source_and_uses_source_ranges edl_validation_rejects_full_source_pass_through`

Expected: pass.

## Task 4: Timeline Generation And Render Plan

**Files:**
- Create: `src-tauri/src/edit/render_plan.rs`
- Test: `src-tauri/tests/one_click_edit.rs`
- Modify: `src-tauri/src/project/model.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Write failing timeline/render tests**

Add tests:

```rust
#[test]
fn rough_cut_edl_becomes_timeline_items_with_source_ranges() {
    let mut project = sample_project_with_media_and_transcript();
    let request = sample_edit_request(EditPreset::TrailerCut);

    let result = generate_one_click_edit_timeline(&mut project, request).expect("generate edit");

    assert!(result.timeline_duration_seconds < 120.0);
    let video_items = project.timeline.tracks.iter()
        .find(|track| track.kind == TrackKind::Video)
        .expect("video track")
        .items
        .clone();
    assert!(!video_items.is_empty());
    assert!(video_items.iter().all(|item| item.properties.contains_key("sourceIn")));
    assert!(video_items.iter().all(|item| item.properties.contains_key("sourceOut")));
}

#[test]
fn render_plan_builds_ffmpeg_trim_concat_shape() {
    let plan = sample_render_plan();
    let command = build_ffmpeg_render_command(&plan).expect("ffmpeg command");

    assert!(command.args.iter().any(|arg| arg.contains("trim=start=")));
    assert!(command.args.iter().any(|arg| arg.contains("concat=n=")));
    assert!(command.args.iter().any(|arg| arg == "-map"));
}
```

- [ ] **Step 2: Run red**

Run: `cargo test --manifest-path src-tauri/Cargo.toml rough_cut_edl_becomes_timeline_items_with_source_ranges render_plan_builds_ffmpeg_trim_concat_shape`

Expected: compile failure for missing render/timeline generation functions.

- [ ] **Step 3: Implement generation and render-plan structs**

Implement:

- `GeneratedEditDraft`
- `generate_one_click_edit_timeline()`
- `RenderPlan`
- `RenderClip`
- `FfmpegCommand`
- `build_render_plan_from_project()`
- `build_ffmpeg_render_command()`

Do not execute ffmpeg from tests; validate command construction only.

- [ ] **Step 4: Run green**

Run: `cargo test --manifest-path src-tauri/Cargo.toml rough_cut_edl_becomes_timeline_items_with_source_ranges render_plan_builds_ffmpeg_trim_concat_shape`

Expected: pass.

## Task 5: Frontend Preset And Prompt UI

**Files:**
- Create: `src/lib/edit.ts`
- Create: `src/lib/edit.test.ts`
- Modify: `src/lib/timeline.ts`
- Modify: `src/components/workspace/agent-panel.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`

- [ ] **Step 1: Write failing frontend tests**

Add tests:

```typescript
import { describe, expect, it } from "vitest";
import { buildEditJobRequest, editPresetOptions } from "./edit";

describe("one-click edit request", () => {
  it("includes preset and prompt in the backend payload", () => {
    expect(
      buildEditJobRequest({
        mediaId: "media-1",
        preset: "trailer_cut",
        prompt: "Make it like an action movie",
      }),
    ).toMatchObject({
      mediaId: "media-1",
      preset: "trailer_cut",
      prompt: "Make it like an action movie",
      languageMode: "auto",
      captionStyle: "bold",
    });
  });

  it("defines the three MVP presets", () => {
    expect(editPresetOptions.map((option) => option.value)).toEqual([
      "trailer_cut",
      "highlight_reel",
      "story_cut",
    ]);
  });
});
```

- [ ] **Step 2: Run red**

Run: `pnpm test src/lib/edit.test.ts`

Expected: fail because `src/lib/edit.ts` does not exist.

- [ ] **Step 3: Implement frontend types and UI**

Implement preset metadata and request builder. Update `AgentPanel` to use shadcn `Button`, existing card primitives, native `select`, and `textarea` until additional shadcn form components are installed. The UI must expose preset selection, prompt input, and a generate button.

- [ ] **Step 4: Run green**

Run: `pnpm test src/lib/edit.test.ts`

Expected: pass.

## Task 6: Deterministic E2E Harness Update

**Files:**
- Modify: `scripts/e2e-combined-video.mjs`

- [ ] **Step 1: Add deterministic one-click coverage**

Update the validation report to include:

- selected preset
- prompt
- source duration
- generated edit duration
- assertion that generated edit duration is shorter than source
- assertion that generated project timeline has sourceIn/sourceOut clips

- [ ] **Step 2: Run E2E**

Run: `pnpm e2e:combined`

Expected: creates `output/e2e-combined/final-combined-validation.mp4` and report includes one-click edit coverage.

## Task 7: Full Verification And Commit

**Files:**
- All modified files.

- [ ] **Step 1: Run full verification**

Run: `pnpm verify`

Expected: TypeScript lint, Vitest, and Cargo tests all pass.

- [ ] **Step 2: Review diff**

Run: `git diff --stat` and inspect key files.

- [ ] **Step 3: Commit**

Run:

```bash
git add docs/superpowers/plans/2026-06-12-one-click-agent-edit.md src-tauri/src src-tauri/tests src scripts package.json src/lib src/components scripts/e2e-combined-video.mjs
git commit -m "feat: add one-click agent edit pipeline"
```

Expected: Conventional Commit on `codex/tauri-foundation`.

## Task 8: Parakeet Sidecar Runner Contract

**Files:**
- Create: `src-tauri/src/edit/transcriber.rs`
- Create: `src-tauri/python/parakeet_runner.py`
- Modify: `src-tauri/src/edit/mod.rs`
- Test: `src-tauri/tests/one_click_edit.rs`

- [ ] **Step 1: Write the failing command-contract test**

Add a Rust test that builds a Parakeet command from a Python path, runner script path, model id, WAV input path, and transcript JSON output path.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test one_click_edit parakeet_runner_command_includes_model_input_and_output`

Expected: compile failure because `edit::transcriber` does not exist.

- [ ] **Step 3: Add Rust command builder and Python runner script**

Implement `ParakeetRunnerConfig`, `ParakeetTranscriptionJob`, and `build_parakeet_command()`. Add `src-tauri/python/parakeet_runner.py` to run `nvidia/parakeet-tdt-0.6b-v3` through Transformers and write the JSON shape consumed by `edit::transcript`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test one_click_edit parakeet_runner_command_includes_model_input_and_output`

Expected: pass.
