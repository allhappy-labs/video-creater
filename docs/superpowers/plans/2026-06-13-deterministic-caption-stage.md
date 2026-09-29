# Deterministic Caption Stage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a deterministic Rust caption generation stage, integrate it into one-click edit generation, and allow validated caption text corrections.

**Architecture:** Rust owns caption cue generation, EDL remapping, timeline mutation, and text-edit validation. React gets a small editor-facing caption inspector over timeline items, but does not compute caption timings. The first UI slice uses local sample timeline data so the caption editing interaction is testable before full Tauri state wiring.

**Tech Stack:** Rust, serde JSON timeline properties, Tauri project model, React, TypeScript, Vitest, Testing Library.

---

## File Structure

- Create `src-tauri/src/edit/captions.rs`: pure deterministic caption cue generation and style metadata.
- Modify `src-tauri/src/edit/mod.rs`: export the new caption module.
- Modify `src-tauri/src/edit/render_plan.rs`: replace one-caption-per-clip logic with generated caption cues over EDL ranges.
- Modify `src-tauri/src/project/patch.rs`: add a validated `EditCaptionText` timeline patch.
- Modify `src-tauri/tests/one_click_edit.rs`: Rust TDD coverage for caption segmentation, remapping, metadata, and edit validation.
- Modify `src/lib/timeline.ts`: add `editCaptionText` patch type, sample generated captions, helper functions for caption selection/edit warnings.
- Modify `src/lib/timeline.test.ts`: TypeScript tests for caption helper behavior.
- Modify `src/components/workspace/timeline-editor.tsx`: accept timeline props, selection, and kind-specific item styling.
- Add `src/components/workspace/caption-inspector.tsx`: focused caption text correction UI.
- Modify `src/components/workspace/editor-workspace.tsx`: hold sample timeline state and wire caption selection/editing.
- Add `src/components/workspace/caption-inspector.test.tsx`: React tests for editing and warning behavior.

## Task 1: Rust Caption Cue Generator

**Files:**
- Create: `src-tauri/src/edit/captions.rs`
- Modify: `src-tauri/src/edit/mod.rs`
- Test: `src-tauri/tests/one_click_edit.rs`

- [ ] **Step 1: Write failing Rust tests for caption cue generation**

Add imports:

```rust
use video_creater_lib::edit::captions::{
    build_caption_cues, CaptionGenerationOptions, CaptionSourceRange,
};
```

Add tests:

```rust
#[test]
fn caption_stage_splits_words_into_readable_cues_with_metadata() {
    let words = vec![
        transcript_word("This", 1.0, 1.2),
        transcript_word("opening", 1.2, 1.5),
        transcript_word("line", 1.5, 1.8),
        transcript_word("needs", 1.8, 2.0),
        transcript_word("a", 2.0, 2.1),
        transcript_word("clean", 2.1, 2.4),
        transcript_word("split", 2.4, 2.8),
        transcript_word("now", 3.5, 3.9),
    ];
    let ranges = vec![CaptionSourceRange {
        media_id: "media-1".to_string(),
        source_in: 1.0,
        source_out: 4.0,
        output_start: 0.0,
    }];

    let cues = build_caption_cues(&words, &ranges, CaptionGenerationOptions::default())
        .expect("caption cues");

    assert!(cues.len() >= 2);
    assert!(cues.iter().all(|cue| cue.text.chars().count() <= 42));
    assert!(cues.iter().all(|cue| cue.duration_seconds >= 0.6));
    assert_eq!(cues[0].start_seconds, 0.0);
    assert_eq!(cues[0].style.style_preset, "boldReadableLower");
    assert!(cues[0].style.avoid.contains("full-width opaque black slabs"));
}

#[test]
fn caption_stage_remaps_source_ranges_to_output_timeline_time() {
    let words = vec![
        transcript_word("first", 10.0, 10.5),
        transcript_word("range", 10.5, 11.0),
        transcript_word("second", 40.0, 40.5),
        transcript_word("range", 40.5, 41.0),
    ];
    let ranges = vec![
        CaptionSourceRange {
            media_id: "media-1".to_string(),
            source_in: 10.0,
            source_out: 12.0,
            output_start: 0.0,
        },
        CaptionSourceRange {
            media_id: "media-1".to_string(),
            source_in: 40.0,
            source_out: 42.0,
            output_start: 2.0,
        },
    ];

    let cues = build_caption_cues(&words, &ranges, CaptionGenerationOptions::default())
        .expect("caption cues");

    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0].start_seconds, 0.0);
    assert_eq!(cues[1].start_seconds, 2.0);
    assert_eq!(cues[1].source_in, 40.0);
    assert_eq!(cues[1].source_out, 41.0);
}
```

Add helper:

```rust
fn transcript_word(text: &str, start_seconds: f64, end_seconds: f64) -> TranscriptWord {
    TranscriptWord {
        text: text.to_string(),
        start_seconds,
        end_seconds,
        confidence: Some(0.95),
        speaker: None,
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml caption_stage_`

Expected: FAIL because `edit::captions` does not exist.

- [ ] **Step 3: Implement the caption module**

Create `src-tauri/src/edit/captions.rs` with public structs `CaptionGenerationOptions`, `CaptionSourceRange`, `CaptionStyleMetadata`, `CaptionCue`, `CaptionGenerationError`, and `build_caption_cues()`.

Use these defaults:

```rust
Self {
    max_words_per_cue: 7,
    max_chars_per_cue: 42,
    max_chars_per_line: 22,
    min_duration_seconds: 0.6,
    max_duration_seconds: 3.0,
    pause_split_threshold_seconds: 0.55,
}
```

The implementation should gather words per source range, split before adding a word when the pause, word count, character count, or punctuation rule requires it, then emit source-clamped cue timing remapped to output timeline time.

- [ ] **Step 4: Export the module**

Add to `src-tauri/src/edit/mod.rs`:

```rust
pub mod captions;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml caption_stage_`

Expected: PASS.

## Task 2: Integrate Captions Into One-Click Timeline Generation

**Files:**
- Modify: `src-tauri/src/edit/render_plan.rs`
- Test: `src-tauri/tests/one_click_edit.rs`

- [ ] **Step 1: Write failing Rust test for timeline captions**

Add:

```rust
#[test]
fn generated_timeline_uses_deterministic_caption_stage_metadata() {
    let mut project = sample_project_with_dense_caption_words();
    let request = sample_edit_request(EditPreset::TrailerCut);

    let result = generate_one_click_edit_timeline(&mut project, request).expect("generate edit");

    let caption_items = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Caption)
        .expect("caption track")
        .items
        .clone();

    assert!(caption_items.len() > result.clip_count);
    assert!(caption_items.iter().all(|item| item.properties.contains_key("visualTreatment")));
    assert!(caption_items.iter().all(|item| item.properties.contains_key("motion")));
    assert!(caption_items.iter().all(|item| item.properties.contains_key("safeZone")));
    assert!(caption_items.iter().all(|item| item.properties.contains_key("avoid")));
    assert!(caption_items.iter().all(|item| item.properties["textEdited"] == serde_json::json!(false)));
}
```

Add helper `sample_project_with_dense_caption_words()` that creates a project with a single media asset and several dense transcript windows using `transcript_word()`.

- [ ] **Step 2: Run test to verify it fails**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml generated_timeline_uses_deterministic_caption_stage_metadata`

Expected: FAIL because generated captions are still one item per clip and lack metadata.

- [ ] **Step 3: Replace `caption_for_clip` usage**

In `generate_one_click_edit_timeline`, build `CaptionSourceRange` entries while walking the EDL clips:

```rust
caption_ranges.push(CaptionSourceRange {
    media_id: media.id.clone(),
    source_in: clip.source_in,
    source_out: clip.source_out,
    output_start: output_cursor,
});
```

After video/audio item creation, call `build_caption_cues()` once and convert cues into `TimelineItemKind::Caption` items with deterministic properties.

- [ ] **Step 4: Remove the old helper**

Delete `ClipCaption` and `caption_for_clip()` from `render_plan.rs`.

- [ ] **Step 5: Run integration test**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml generated_timeline_uses_deterministic_caption_stage_metadata`

Expected: PASS.

## Task 3: Validated Caption Text Patch

**Files:**
- Modify: `src-tauri/src/project/patch.rs`
- Test: `src-tauri/tests/project_patch.rs`

- [ ] **Step 1: Write failing patch tests**

Add tests:

```rust
#[test]
fn caption_text_patch_updates_text_and_marks_item_edited() {
    let mut project = sample_project();

    apply_timeline_patch(
        &mut project,
        TimelinePatch::EditCaptionText {
            item_id: "caption-1".to_string(),
            text: "Corrected caption".to_string(),
        },
    )
    .expect("edit caption");

    let item = find_timeline_item(&project, "caption-1").expect("caption");
    assert_eq!(
        item.source,
        TimelineSource::Text {
            text: "Corrected caption".to_string()
        }
    );
    assert_eq!(item.properties["textEdited"], serde_json::json!(true));
}

#[test]
fn caption_text_patch_rejects_empty_text_and_non_caption_items() {
    let mut project = sample_project();

    assert_eq!(
        apply_timeline_patch(
            &mut project,
            TimelinePatch::EditCaptionText {
                item_id: "caption-1".to_string(),
                text: "   ".to_string(),
            },
        ),
        Err(TimelinePatchError::EmptyCaptionText)
    );

    assert_eq!(
        apply_timeline_patch(
            &mut project,
            TimelinePatch::EditCaptionText {
                item_id: "video-1".to_string(),
                text: "Wrong target".to_string(),
            },
        ),
        Err(TimelinePatchError::NotCaptionItem("video-1".to_string()))
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml caption_text_patch`

Expected: FAIL because `EditCaptionText`, `EmptyCaptionText`, and `NotCaptionItem` do not exist.

- [ ] **Step 3: Implement `EditCaptionText`**

Add enum variant:

```rust
EditCaptionText {
    item_id: String,
    text: String,
},
```

Add errors:

```rust
#[error("caption text cannot be empty")]
EmptyCaptionText,
#[error("timeline item is not a caption: {0}")]
NotCaptionItem(String),
```

Implement edit logic by finding the item, rejecting locked tracks, requiring `TimelineItemKind::Caption` and `TimelineSource::Text`, trimming the text, updating the source text, setting `properties["textEdited"] = true`, and preserving timing.

- [ ] **Step 4: Run tests to verify they pass**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml caption_text_patch`

Expected: PASS.

## Task 4: Frontend Caption Editing Slice

**Files:**
- Modify: `src/lib/timeline.ts`
- Modify: `src/lib/timeline.test.ts`
- Add: `src/components/workspace/caption-inspector.tsx`
- Add: `src/components/workspace/caption-inspector.test.tsx`
- Modify: `src/components/workspace/timeline-editor.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`

- [ ] **Step 1: Write failing timeline helper tests**

Add tests for:

```ts
createEditCaptionTextPatch({ itemId: "caption-1", text: "Corrected" })
```

and:

```ts
getCaptionReadingWarning({ text: "A very long correction that exceeds limits", durationSeconds: 0.8 })
```

Expected patch type: `editCaptionText`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `rtk pnpm test src/lib/timeline.test.ts`

Expected: FAIL because helpers do not exist.

- [ ] **Step 3: Implement timeline helper code**

Add `editCaptionText` to `TimelinePatch`, add sample caption items to `sampleTimeline`, implement `getTimelineItemText()`, `createEditCaptionTextPatch()`, and `getCaptionReadingWarning()`.

- [ ] **Step 4: Run helper tests to verify they pass**

Run: `rtk pnpm test src/lib/timeline.test.ts`

Expected: PASS.

- [ ] **Step 5: Write failing inspector tests**

Add tests that render `CaptionInspector`, change textarea text, apply it, and assert `onApply("caption-1", "Corrected")`. Add a second test that renders a long caption and expects a warning.

- [ ] **Step 6: Run inspector tests to verify they fail**

Run: `rtk pnpm test src/components/workspace/caption-inspector.test.tsx`

Expected: FAIL because `CaptionInspector` does not exist.

- [ ] **Step 7: Implement inspector and timeline selection**

Implement `CaptionInspector` with a textarea, start/end timestamps, edited-state badge, warning text, and Apply button. Update `TimelineEditor` to accept `timeline`, `selectedItemId`, and `onSelectItem`. Update `EditorWorkspace` to hold sample timeline state, selected caption id, and apply caption text by replacing only the selected item's `source.text` and `properties.textEdited`.

- [ ] **Step 8: Run frontend tests**

Run: `rtk pnpm test`

Expected: PASS.

## Task 5: Verification

**Files:**
- No new files.

- [ ] **Step 1: Run Rust tests**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml`

Expected: PASS.

- [ ] **Step 2: Run frontend tests**

Run: `rtk pnpm test`

Expected: PASS.

- [ ] **Step 3: Run lint**

Run: `rtk pnpm lint`

Expected: PASS.

- [ ] **Step 4: Review git diff**

Run: `rtk git diff --stat`

Expected: only caption-stage, patch, timeline, inspector, and tests changed.
