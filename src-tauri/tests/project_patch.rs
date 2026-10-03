use video_creater_lib::project::action::{
    apply_project_action, ProjectAction, ProjectActionResize,
};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::import::{import_media_files, ImportMediaError};
use video_creater_lib::project::model::*;
use video_creater_lib::project::patch::{apply_timeline_patch, TimelinePatch, TimelinePatchError};
use video_creater_lib::project::split::{load_split_project, save_split_project};
use video_creater_lib::project::storage::{load_project, save_project, PROJECT_FILE_NAME};

fn assert_patch_error_leaves_project_unchanged(
    project: &mut VideoProject,
    patch: TimelinePatch,
    expected_error: TimelinePatchError,
) {
    let before = project.clone();

    let error = apply_timeline_patch(project, patch).expect_err("patch must fail");

    assert_eq!(error, expected_error);
    assert_eq!(*project, before);
}

#[test]
fn moves_item_when_track_and_item_exist() {
    let mut project = sample_project();

    apply_timeline_patch(
        &mut project,
        TimelinePatch::MoveItem {
            item_id: "item-1".to_string(),
            target_track_id: "track-video".to_string(),
            start_seconds: 2.0,
        },
    )
    .expect("patch should apply");

    let item = &project.timeline.tracks[0].items[0];
    assert_eq!(item.start_seconds, 2.0);
    assert_eq!(project.timeline.duration_seconds, 6.0);
}

#[test]
fn timeline_patch_uses_camel_case_wire_contract() {
    let patch = TimelinePatch::MoveItem {
        item_id: "item-1".to_string(),
        target_track_id: "track-video".to_string(),
        start_seconds: 2.25,
    };

    let json = serde_json::to_value(&patch).expect("serialize patch");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "moveItem",
            "itemId": "item-1",
            "targetTrackId": "track-video",
            "startSeconds": 2.25,
        })
    );

    let decoded: TimelinePatch = serde_json::from_value(json).expect("deserialize patch");
    assert_eq!(decoded, patch);
}

#[test]
fn applies_deserialized_timeline_patch() {
    let mut project = sample_project();
    let patch: TimelinePatch = serde_json::from_value(serde_json::json!({
        "type": "moveItem",
        "itemId": "item-1",
        "targetTrackId": "track-video",
        "startSeconds": 3.0,
    }))
    .expect("deserialize patch");

    apply_timeline_patch(&mut project, patch).expect("patch should apply");

    assert_eq!(project.timeline.tracks[0].items[0].start_seconds, 3.0);
    assert_eq!(project.timeline.duration_seconds, 7.0);
}

#[test]
fn rejects_negative_start() {
    let mut project = sample_project();

    assert_patch_error_leaves_project_unchanged(
        &mut project,
        TimelinePatch::MoveItem {
            item_id: "item-1".to_string(),
            target_track_id: "track-video".to_string(),
            start_seconds: -0.1,
        },
        TimelinePatchError::NegativeTime,
    );
}

#[test]
fn rejects_track_type_mismatch() {
    let mut project = sample_project();

    assert_patch_error_leaves_project_unchanged(
        &mut project,
        TimelinePatch::MoveItem {
            item_id: "item-1".to_string(),
            target_track_id: "track-captions".to_string(),
            start_seconds: 1.0,
        },
        TimelinePatchError::TrackTypeMismatch {
            item_kind: TimelineItemKind::VideoClip,
            track_kind: TrackKind::Caption,
        },
    );
}

#[test]
fn rejects_move_from_locked_source_track() {
    let mut project = sample_project();
    project.timeline.tracks[0].locked = true;
    project.timeline.tracks.insert(
        1,
        TimelineTrack::empty("track-video-2", "Video 2", TrackKind::Video),
    );

    assert_patch_error_leaves_project_unchanged(
        &mut project,
        TimelinePatch::MoveItem {
            item_id: "item-1".to_string(),
            target_track_id: "track-video-2".to_string(),
            start_seconds: 1.0,
        },
        TimelinePatchError::TrackLocked("track-video".to_string()),
    );
}

#[test]
fn rejects_nan_move_start() {
    let mut project = sample_project();

    assert_patch_error_leaves_project_unchanged(
        &mut project,
        TimelinePatch::MoveItem {
            item_id: "item-1".to_string(),
            target_track_id: "track-video".to_string(),
            start_seconds: f64::NAN,
        },
        TimelinePatchError::NegativeTime,
    );
}

#[test]
fn rejects_infinite_move_start() {
    let mut project = sample_project();

    assert_patch_error_leaves_project_unchanged(
        &mut project,
        TimelinePatch::MoveItem {
            item_id: "item-1".to_string(),
            target_track_id: "track-video".to_string(),
            start_seconds: f64::INFINITY,
        },
        TimelinePatchError::NegativeTime,
    );
}

#[test]
fn resizes_item_duration() {
    let mut project = sample_project();

    apply_timeline_patch(
        &mut project,
        TimelinePatch::ResizeItem {
            item_id: "item-1".to_string(),
            duration_seconds: 6.5,
        },
    )
    .expect("resize should apply");

    assert_eq!(project.timeline.tracks[0].items[0].duration_seconds, 6.5);
    assert_eq!(project.timeline.duration_seconds, 6.5);
}

#[test]
fn trims_item_timing_and_source_range() {
    let mut project = sample_project();

    apply_timeline_patch(
        &mut project,
        TimelinePatch::TrimItem {
            item_id: "item-1".to_string(),
            start_seconds: 1.5,
            duration_seconds: 2.5,
            source_in: Some(3.0),
            source_out: Some(5.5),
        },
    )
    .expect("trim patch should apply");

    let item = &project.timeline.tracks[0].items[0];
    assert_eq!(item.start_seconds, 1.5);
    assert_eq!(item.duration_seconds, 2.5);
    assert_eq!(item.properties["sourceIn"], serde_json::json!(3.0));
    assert_eq!(item.properties["sourceOut"], serde_json::json!(5.5));
}

#[test]
fn invalid_trim_source_range_returns_a_recoverable_patch_error() {
    let mut project = sample_project();

    assert_patch_error_leaves_project_unchanged(
        &mut project,
        TimelinePatch::TrimItem {
            item_id: "item-1".to_string(),
            start_seconds: 1.5,
            duration_seconds: 2.5,
            source_in: Some(3.0),
            source_out: Some(4.0),
        },
        TimelinePatchError::InvalidPatch(
            "source range duration does not match timeline duration: item-1".to_string(),
        ),
    );
}

#[test]
fn rejects_zero_duration() {
    let mut project = sample_project();

    assert_patch_error_leaves_project_unchanged(
        &mut project,
        TimelinePatch::ResizeItem {
            item_id: "item-1".to_string(),
            duration_seconds: 0.0,
        },
        TimelinePatchError::NonPositiveDuration,
    );
}

#[test]
fn caption_text_patch_updates_text_and_marks_item_edited() {
    let mut project = sample_project_with_caption_item();

    apply_timeline_patch(
        &mut project,
        TimelinePatch::EditCaptionText {
            item_id: "caption-1".to_string(),
            text: "Corrected caption".to_string(),
        },
    )
    .expect("edit caption");

    let item = find_timeline_item(&project, "caption-1").expect("caption");
    assert_eq!(item.start_seconds, 1.0);
    assert_eq!(item.duration_seconds, 1.25);
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
    let mut project = sample_project_with_caption_item();

    assert_patch_error_leaves_project_unchanged(
        &mut project,
        TimelinePatch::EditCaptionText {
            item_id: "caption-1".to_string(),
            text: "   ".to_string(),
        },
        TimelinePatchError::EmptyCaptionText,
    );

    assert_patch_error_leaves_project_unchanged(
        &mut project,
        TimelinePatch::EditCaptionText {
            item_id: "item-1".to_string(),
            text: "Wrong target".to_string(),
        },
        TimelinePatchError::NotCaptionItem("item-1".to_string()),
    );
}

#[test]
fn timeline_patch_matches_project_action_for_resize_and_caption_text() {
    let mut patch_project = sample_project_with_caption_item();
    let mut action_project = patch_project.clone();

    apply_timeline_patch(
        &mut patch_project,
        TimelinePatch::ResizeItem {
            item_id: "item-1".to_string(),
            duration_seconds: 6.5,
        },
    )
    .expect("resize patch");
    apply_project_action(
        &mut action_project,
        ProjectAction::ResizeItems {
            resizes: vec![ProjectActionResize {
                item_id: "item-1".to_string(),
                duration_seconds: 6.5,
            }],
        },
    )
    .expect("resize action");
    assert_eq!(patch_project, action_project);

    apply_timeline_patch(
        &mut patch_project,
        TimelinePatch::EditCaptionText {
            item_id: "caption-1".to_string(),
            text: "Corrected caption".to_string(),
        },
    )
    .expect("caption patch");
    apply_project_action(
        &mut action_project,
        ProjectAction::EditCaptionText {
            item_id: "caption-1".to_string(),
            text: "Corrected caption".to_string(),
        },
    )
    .expect("caption action");
    assert_eq!(patch_project, action_project);
}

#[test]
fn rejects_nan_duration() {
    let mut project = sample_project();

    assert_patch_error_leaves_project_unchanged(
        &mut project,
        TimelinePatch::ResizeItem {
            item_id: "item-1".to_string(),
            duration_seconds: f64::NAN,
        },
        TimelinePatchError::NonPositiveDuration,
    );
}

#[test]
fn rejects_infinite_duration() {
    let mut project = sample_project();

    assert_patch_error_leaves_project_unchanged(
        &mut project,
        TimelinePatch::ResizeItem {
            item_id: "item-1".to_string(),
            duration_seconds: f64::INFINITY,
        },
        TimelinePatchError::NonPositiveDuration,
    );
}

#[test]
fn saves_and_loads_project_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let project = sample_project();

    let project_file = save_project(dir.path(), &project).expect("save project");

    assert_eq!(
        project_file.file_name().and_then(|name| name.to_str()),
        Some(PROJECT_FILE_NAME)
    );
    for child in [
        "media",
        "transcripts",
        "generated/hyperframes",
        "generated/previews",
        "renders",
        "logs",
    ] {
        assert!(dir.path().join(child).is_dir(), "{child} dir should exist");
    }

    let saved_json = std::fs::read_to_string(&project_file).expect("read saved json");
    assert!(saved_json.contains('\n'));
    assert!(saved_json.contains("  \"schemaVersion\""));

    let loaded_project = load_project(dir.path()).expect("load project");

    assert_eq!(loaded_project, project);
}

#[test]
fn saves_over_existing_project_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let initial_project = sample_project();
    save_project(dir.path(), &initial_project).expect("save initial project");

    let mut updated_project = sample_project();
    updated_project.name = "Updated Project".to_string();
    updated_project.timeline.duration_seconds = 8.0;

    let project_file = save_project(dir.path(), &updated_project).expect("save updated project");
    let saved_json = std::fs::read_to_string(project_file).expect("read updated json");
    assert!(saved_json.contains("Updated Project"));

    let loaded_project = load_project(dir.path()).expect("load updated project");
    assert_eq!(loaded_project, updated_project);
}

#[test]
fn imports_supported_media_file_into_project_media_folder() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let source = source_dir.path().join("camera clip.mp4");
    std::fs::write(&source, b"video bytes").expect("source file");
    let project = sample_project();

    let result = import_media_files(dir.path(), project, std::slice::from_ref(&source))
        .expect("import media");

    assert_eq!(result.imported.len(), 1);
    assert_eq!(result.skipped.len(), 0);
    assert_eq!(result.project.media.len(), 2);
    let imported = &result.imported[0];
    assert_eq!(imported.kind, MediaKind::Video);
    assert!(imported.relative_path.starts_with("media/"));
    assert!(imported.relative_path.ends_with(".mp4"));
    assert!(dir.path().join(&imported.relative_path).is_file());
    assert_eq!(
        std::fs::read(dir.path().join(&imported.relative_path)).expect("copied file"),
        b"video bytes"
    );

    let loaded = load_project(dir.path()).expect("saved project should load");
    assert_eq!(loaded.media.len(), 2);
    assert_eq!(loaded.media.last(), Some(imported));
}

#[test]
fn import_media_preserves_and_reloads_the_split_project_manifest() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let source = source_dir.path().join("split import.png");
    std::fs::write(&source, b"png bytes").expect("source file");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save initial split project");

    let result = import_media_files(dir.path(), project, std::slice::from_ref(&source))
        .expect("import into split project");
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(dir.path().join(PROJECT_FILE_NAME)).expect("read split manifest"),
    )
    .expect("parse split manifest");
    let reloaded = load_split_project(dir.path()).expect("reload imported split project");

    assert_eq!(manifest["layout"], serde_json::json!("split"));
    assert_eq!(reloaded.media, result.project.media);
    assert_eq!(reloaded.media.last(), result.imported.first());
}

#[test]
fn imports_palmier_audio_and_image_source_extensions() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let sources = [
        ("dialogue.aiff", b"aiff bytes".as_slice(), MediaKind::Audio),
        ("music.AIFC", b"aifc bytes".as_slice(), MediaKind::Audio),
        ("scan.tiff", b"tiff bytes".as_slice(), MediaKind::Image),
        ("still.HEIC", b"heic bytes".as_slice(), MediaKind::Image),
    ];
    let source_paths = sources
        .iter()
        .map(|(name, bytes, _kind)| {
            let path = source_dir.path().join(name);
            std::fs::write(&path, bytes).expect("source file");
            path
        })
        .collect::<Vec<_>>();

    let result =
        import_media_files(dir.path(), sample_project(), &source_paths).expect("import media");

    assert_eq!(result.imported.len(), 4);
    assert_eq!(result.skipped.len(), 0);
    for ((name, bytes, kind), imported) in sources.iter().zip(result.imported.iter()) {
        assert_eq!(&imported.kind, kind);
        assert!(imported.relative_path.ends_with(&format!(
            ".{}",
            name.rsplit('.').next().unwrap().to_ascii_lowercase()
        )));
        assert_eq!(
            std::fs::read(dir.path().join(&imported.relative_path)).expect("copied file"),
            *bytes
        );
    }
}

#[test]
fn import_wav_records_duration_without_an_external_media_probe() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let source = source_dir.path().join("one-second.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 8_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&source, spec).expect("wav writer");
    for _ in 0..8_000 {
        writer.write_sample::<i16>(0).expect("wav sample");
    }
    writer.finalize().expect("finalize wav");

    let result = import_media_files(dir.path(), sample_project(), &[source]).expect("import wav");

    assert_eq!(result.imported[0].kind, MediaKind::Audio);
    assert!((result.imported[0].duration_seconds - 1.0).abs() < 1e-6);
}

#[test]
fn imports_lottie_json_with_animation_metadata() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let source = source_dir.path().join("Logo Loop.json");
    std::fs::write(
        &source,
        br#"{"v":"5.12.0","fr":30,"ip":0,"op":60,"w":640,"h":360,"layers":[]}"#,
    )
    .expect("lottie source");

    let result = import_media_files(dir.path(), sample_project(), std::slice::from_ref(&source))
        .expect("import lottie");

    assert_eq!(result.imported.len(), 1);
    let imported = &result.imported[0];
    assert_eq!(imported.kind, MediaKind::Lottie);
    assert_eq!(imported.duration_seconds, 2.0);
    assert_eq!(imported.width, Some(640));
    assert_eq!(imported.height, Some(360));
    assert_eq!(imported.fps, Some(30.0));
    assert!(imported.relative_path.ends_with(".json"));
}

#[test]
fn imports_dotlottie_bundle_by_extension() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let source = source_dir.path().join("animated-logo.lottie");
    std::fs::write(
        &source,
        minimal_dotlottie_archive(&["manifest.json", "animations/anim.json"]),
    )
    .expect("dotlottie source");

    let result = import_media_files(dir.path(), sample_project(), std::slice::from_ref(&source))
        .expect("import dotlottie");

    assert_eq!(result.imported.len(), 1);
    assert_eq!(result.imported[0].kind, MediaKind::Lottie);
    assert!(result.imported[0].relative_path.ends_with(".lottie"));
    assert_eq!(result.imported[0].duration_seconds, 4.0);
    assert_eq!(result.imported[0].fps, Some(24.0));
    assert_eq!(result.imported[0].width, Some(128));
    assert_eq!(result.imported[0].height, Some(72));
}

#[test]
fn import_media_rejects_invalid_dotlottie_bundle() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let source = source_dir.path().join("broken.lottie");
    std::fs::write(&source, b"not a zip").expect("dotlottie source");

    let error = import_media_files(dir.path(), sample_project(), std::slice::from_ref(&source))
        .expect_err("invalid dotLottie should not import as media");

    assert_eq!(error, ImportMediaError::NoImportableFiles);
}

#[test]
fn import_media_rejects_plain_json_without_lottie_markers() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let source = source_dir.path().join("data.json");
    std::fs::write(&source, br#"{"title":"not animation"}"#).expect("json source");

    let error = import_media_files(dir.path(), sample_project(), std::slice::from_ref(&source))
        .expect_err("plain JSON should not import as media");

    assert_eq!(error, ImportMediaError::NoImportableFiles);
}

fn minimal_dotlottie_archive(entries: &[&str]) -> Vec<u8> {
    use std::io::Write;
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for entry in entries {
        archive
            .start_file(*entry, zip::write::SimpleFileOptions::default())
            .unwrap();
        let bytes = if *entry == "manifest.json" {
            br#"{"version":"1.0","animations":[{"id":"anim"}]}"#.as_slice()
        } else {
            include_bytes!("fixtures/transitions/lottie-colour-steps.json").as_slice()
        };
        archive.write_all(bytes).unwrap();
    }
    archive.finish().unwrap().into_inner()
}

#[test]
fn import_media_uses_distinct_destinations_for_duplicate_names() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let first = source_dir.path().join("clip.mp4");
    let second_dir = tempfile::tempdir().expect("second source dir");
    let second = second_dir.path().join("clip.mp4");
    std::fs::write(&first, b"first").expect("first source");
    std::fs::write(&second, b"second").expect("second source");

    let result =
        import_media_files(dir.path(), sample_project(), &[first, second]).expect("import files");

    assert_eq!(result.imported.len(), 2);
    assert_ne!(
        result.imported[0].relative_path,
        result.imported[1].relative_path
    );
}

#[test]
fn import_media_skips_unsupported_files_but_imports_supported_files() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let supported = source_dir.path().join("voice.wav");
    let unsupported = source_dir.path().join("notes.txt");
    std::fs::write(&supported, b"audio").expect("audio source");
    std::fs::write(&unsupported, b"notes").expect("notes source");

    let result = import_media_files(dir.path(), sample_project(), &[supported, unsupported])
        .expect("mixed import");

    assert_eq!(result.imported.len(), 1);
    assert_eq!(result.imported[0].kind, MediaKind::Audio);
    assert_eq!(result.skipped.len(), 1);
    assert!(result.skipped[0].reason.contains("unsupported"));
}

#[test]
fn import_media_rejects_all_unsupported_files_without_changing_project() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let unsupported = source_dir.path().join("notes.txt");
    std::fs::write(&unsupported, b"notes").expect("notes source");

    let error = import_media_files(dir.path(), sample_project(), &[unsupported])
        .expect_err("all unsupported should fail");

    assert_eq!(error, ImportMediaError::NoImportableFiles);
    assert!(load_project(dir.path()).is_err());
}

fn sample_project_with_caption_item() -> VideoProject {
    let mut project = sample_project();
    let caption_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Caption)
        .expect("caption track");
    caption_track.items.push(TimelineItem {
        id: "caption-1".to_string(),
        kind: TimelineItemKind::Caption,
        start_seconds: 1.0,
        duration_seconds: 1.25,
        source: TimelineSource::Text {
            text: "Original caption".to_string(),
        },
        label: "Caption 1".to_string(),
        properties: std::collections::BTreeMap::from([(
            "textEdited".to_string(),
            serde_json::json!(false),
        )]),
    });
    project
}

fn find_timeline_item<'a>(project: &'a VideoProject, item_id: &str) -> Option<&'a TimelineItem> {
    project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == item_id)
}
