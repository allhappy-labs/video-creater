use base64::Engine;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::time::Duration;
use video_creater_lib::edit::render_plan::{ExportEncodeTier, RenderQuality, RenderQualityProfile};
use video_creater_lib::graphics::assets::AssetRegistry;
use video_creater_lib::graphics::ir::{GraphicNode, GraphicRole};
use video_creater_lib::graphics::renderer::{render_graphics_preview, GraphicsRenderOptions};
use video_creater_lib::project::export_options::{draft_dimensions, ExportRenderOptions};
use video_creater_lib::project::export_profiles::{
    mp4_export_profile_availability_report, ExportProfile,
};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetOutput, GeneratedAssetReferences, GeneratedAssetSettings,
    GeneratedAssetStatus, GenerationModel, JobStatus, JobSummary, MediaAsset, MediaKind,
    ProjectTimeline, RenderReportCheckStatus, RenderReportStatus, TimelineItem, TimelineItemKind,
    TimelineSource, TimelineTrack, TrackKind,
};
use video_creater_lib::project::split::{
    load_split_project, save_split_project, validate_split_project,
};
use video_creater_lib::render_pipeline::error::PipelineErrorCode;
use video_creater_lib::render_pipeline::gstreamer_backend::generate_fixture_source_with_gstreamer;
use video_creater_lib::render_pipeline::process::CommandSpec;
use video_creater_lib::render_pipeline::project_export::{
    build_project_graphics_layers, build_project_media_render_plan,
    build_project_media_render_plan_with_options, build_project_webm_render_plan,
    build_project_webm_render_plan_for_range, expand_project_nested_timelines_for_render,
    expected_media_for_render_plan, load_project_render_pipeline_report,
    project_render_report_from_pipeline_report, render_media_to_split_project_folder,
    render_media_to_split_project_folder_for_timeline, validate_render_storage_budget,
    ProjectMediaRenderPaths, ProjectRenderReportEvidence, ProjectWebmRenderPaths,
};
use video_creater_lib::render_pipeline::report::{
    RenderGraphicsReport, RenderPreviewComparison, RenderPreviewComparisonFrame,
    RenderPreviewComparisonRequest, RenderReport,
    RenderReportStreams as PipelineRenderReportStreams, RenderReportSummary,
};
use video_creater_lib::render_runtime::start_render_process_runtime;

#[path = "project_export/export_workflow_completion.rs"]
mod export_workflow_completion;
#[path = "project_export/named_exports.rs"]
mod named_exports;
#[path = "project_export/render_lease.rs"]
mod render_lease;

#[test]
fn project_media_render_paths_use_profile_extension() {
    let mp4 =
        ProjectMediaRenderPaths::new("export-mp4", ExportProfile::Mp4H264).expect("mp4 paths");
    assert_eq!(mp4.output_path, "renders/export-mp4/output.mp4");

    let mov =
        ProjectMediaRenderPaths::new("export-prores", ExportProfile::ProResMov).expect("mov paths");
    assert_eq!(mov.output_path, "renders/export-prores/output.mov");
}

#[test]
fn draft_default_normalizes_smaller_odd_dimensions_without_upscaling() {
    assert_eq!(draft_dimensions(735, 399), (734, 398));
}

#[test]
fn render_storage_preflight_fails_closed_with_actionable_byte_evidence() {
    let dir = tempfile::tempdir().expect("project volume");
    let error = validate_render_storage_budget(dir.path(), 64 * 1024 * 1024, 512 * 1024 * 1024)
        .expect_err("low disk must block before render work starts");

    assert_eq!(error[0].code, PipelineErrorCode::RenderStorageInsufficient);
    assert_eq!(error[0].path, "render.storage");
    assert_eq!(error[0].details["availableBytes"], "67108864");
    assert_eq!(error[0].details["requiredBytes"], "536870912");
    assert!(error[0].fix.contains("retry"));
}

#[test]
fn draft_project_media_render_plan_uses_export_options_hd_defaults() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    project.render_settings.width = 3840;
    project.render_settings.height = 2160;
    let item = &mut project.timeline.tracks[0].items[0];
    item.properties.insert("sourceIn".to_string(), json!(1.0));
    item.properties.insert("sourceOut".to_string(), json!(3.0));

    let plan = build_project_media_render_plan(
        dir.path(),
        &project,
        "export-mp4",
        RenderQuality::Draft,
        ExportProfile::Mp4H264,
    )
    .expect("mp4 render plan");

    assert_eq!(
        plan.output_path,
        dir.path()
            .join("renders/export-mp4/output.mp4")
            .display()
            .to_string()
    );
    assert_eq!((plan.width, plan.height), (1280, 720));
    assert_eq!(plan.quality, RenderQuality::Draft);
}

#[test]
fn legacy_draft_webm_plan_fits_portrait_project_inside_hd() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    project.render_settings.width = 1080;
    project.render_settings.height = 1920;
    let item = &mut project.timeline.tracks[0].items[0];
    item.properties.insert("sourceIn".to_string(), json!(0.0));
    item.properties.insert("sourceOut".to_string(), json!(4.0));

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "legacy-draft-portrait",
        RenderQualityProfile::DraftWebm,
    )
    .expect("legacy Draft WebM plan");

    assert_eq!((plan.width, plan.height), (404, 720));
    assert_eq!(plan.quality, RenderQuality::Draft);
}

#[test]
fn legacy_final_webm_plan_preserves_project_resolution() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    project.render_settings.width = 1080;
    project.render_settings.height = 1920;
    let item = &mut project.timeline.tracks[0].items[0];
    item.properties.insert("sourceIn".to_string(), json!(0.0));
    item.properties.insert("sourceOut".to_string(), json!(4.0));

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "legacy-final-portrait",
        RenderQualityProfile::FinalWebm,
    )
    .expect("legacy Final WebM plan");

    assert_eq!((plan.width, plan.height), (1080, 1920));
    assert_eq!(plan.quality, RenderQuality::Final);
}

#[test]
fn project_media_render_plan_preserves_explicit_full_hd_draft_options() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    project.render_settings.width = 3840;
    project.render_settings.height = 2160;
    let item = &mut project.timeline.tracks[0].items[0];
    item.properties.insert("sourceIn".to_string(), json!(1.0));
    item.properties.insert("sourceOut".to_string(), json!(3.0));
    let options =
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Draft, 1920, 1080)
            .expect("explicit full-hd draft options are valid");

    let plan = build_project_media_render_plan_with_options(
        dir.path(),
        &project,
        "export-explicit-full-hd",
        options,
    )
    .expect("explicit options should build a render plan");

    assert_eq!((plan.width, plan.height), (1920, 1080));
    assert_eq!(plan.quality, RenderQuality::Draft);

    let expected = expected_media_for_render_plan(&plan, true);
    assert_eq!((expected.width, expected.height), (Some(1920), Some(1080)));
    assert!(expected.video_required);
    assert!(expected.audio_required);
}

/// A timeline without audio still renders through an audio/video GES timeline
/// into a container that carries an audio track, so the audio stream stays
/// optional while its codec is still the profile's.
#[test]
fn project_media_expected_output_keeps_the_profile_audio_codec_without_timeline_audio() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let item = &mut project.timeline.tracks[0].items[0];
    item.properties.insert("sourceIn".to_string(), json!(0.0));
    item.properties.insert("sourceOut".to_string(), json!(4.0));
    let options =
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Final, 1920, 1080)
            .expect("full-hd final options are valid");

    let plan = build_project_media_render_plan_with_options(
        dir.path(),
        &project,
        "export-without-audio",
        options,
    )
    .expect("audio-free project should build a render plan");

    let expected = expected_media_for_render_plan(&plan, false);
    assert!(!expected.audio_required);
    assert_eq!(expected.audio_codec.as_deref(), Some("aac"));
}

#[test]
fn project_media_render_plan_applies_frame_rate_override_and_encode_tier() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    assert_eq!(project.render_settings.fps, 24.0);
    let item = &mut project.timeline.tracks[0].items[0];
    item.properties.insert("sourceIn".to_string(), json!(0.0));
    item.properties.insert("sourceOut".to_string(), json!(4.0));
    let options =
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Final, 1920, 1080)
            .and_then(|options| options.with_fps(Some(30.0)))
            .and_then(|options| options.with_encode_tier(ExportEncodeTier::Master))
            .expect("master options with a frame-rate override are valid");

    let plan = build_project_media_render_plan_with_options(
        dir.path(),
        &project,
        "export-master",
        options,
    )
    .expect("options should build a render plan");

    assert_eq!(plan.fps, 30.0);
    assert_eq!(plan.encode_tier, ExportEncodeTier::Master);
}

#[test]
fn project_render_plan_preserves_distinct_timeline_duration_for_visual_speed() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let item = &mut project.timeline.tracks[0].items[0];
    item.start_seconds = 0.0;
    item.duration_seconds = 1.5;
    item.properties.insert("sourceIn".to_string(), json!(0.0));
    item.properties.insert("sourceOut".to_string(), json!(3.0));
    item.properties.insert("speed".to_string(), json!(2.0));

    let plan = build_project_media_render_plan(
        dir.path(),
        &project,
        "speed-plan",
        RenderQuality::Final,
        ExportProfile::Mp4H264,
    )
    .expect("speed render plan");
    assert_eq!(plan.clips[0].source_in, 0.0);
    assert_eq!(plan.clips[0].source_out, 3.0);
    assert_eq!(plan.clips[0].properties["speed"], json!(2.0));
    assert_eq!(
        plan.clips[0].properties["timelineDurationSeconds"],
        json!(1.5)
    );
}

#[test]
fn project_render_plan_recursively_expands_nested_timeline_clips() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let mut alternate = project.timeline.clone();
    alternate.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    alternate.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate cut".to_string(),
        timeline: alternate,
    });
    let root_item = &mut project.timeline.tracks[0].items[0];
    root_item.start_seconds = 1.0;
    root_item.duration_seconds = 3.0;
    root_item.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    root_item.properties = BTreeMap::from([("opacity".to_string(), json!(0.5))]);

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-nested",
        RenderQualityProfile::DraftWebm,
    )
    .expect("nested timeline should build a render plan");

    assert_eq!(plan.clips.len(), 1);
    assert_eq!(plan.clips[0].timeline_start_seconds, Some(1.0));
    assert_eq!(plan.clips[0].source_in, 0.0);
    assert_eq!(plan.clips[0].source_out, 3.0);
    assert_eq!(plan.clips[0].properties.get("opacity"), Some(&json!(0.5)));
}

#[test]
fn nested_wrapper_and_child_effects_share_canonical_prepared_stack() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let mut alternate = project.timeline.clone();
    alternate.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    alternate.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));
    alternate.tracks[0].items[0].properties.insert(
        "effects".to_string(),
        json!([
            { "effectType": "color.contrast", "enabled": true, "params": { "amount": 1.2 } }
        ]),
    );
    project.timelines.push(ProjectTimeline {
        id: "effects-cut".to_string(),
        name: "Effects".to_string(),
        timeline: alternate,
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "effects-cut".to_string(),
    };
    wrapper.properties = BTreeMap::from([(
        "effects".to_string(),
        json!([
            { "effectType": "stylize.vignette", "enabled": true, "params": { "amount": -0.4, "midpoint": 0.5, "feather": 0.5 } }
        ]),
    )]);

    let expanded = expand_project_nested_timelines_for_render(&project).expect("expand effects");
    let child = &expanded
        .timeline
        .tracks
        .iter()
        .find(|track| !track.items.is_empty())
        .expect("expanded track")
        .items[0];
    assert!(child.properties["preparedEffectStackFingerprint"]
        .as_str()
        .is_some_and(|value| value.len() == 64));
    let prepared = child.properties["preparedEffectStack"]["effects"]
        .as_array()
        .expect("prepared effects");
    assert_eq!(prepared.len(), 2);
    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "nested-effects",
        RenderQualityProfile::DraftWebm,
    )
    .expect("nested effect render plan");
    assert_eq!(
        plan.clips[0].properties["effects"].as_array().map(Vec::len),
        Some(2)
    );
}

#[test]
fn nested_wrapper_rejects_audio_or_incompatible_effect_stack() {
    let mut project = sample_project();
    let alternate = project.timeline.clone();
    project.timelines.push(ProjectTimeline {
        id: "bad-effects".to_string(),
        name: "Bad".to_string(),
        timeline: alternate,
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "bad-effects".to_string(),
    };
    wrapper.properties = BTreeMap::from([(
        "effects".to_string(),
        json!([
            { "effectType": "audio.denoise", "enabled": true, "params": {} }
        ]),
    )]);
    let error = expand_project_nested_timelines_for_render(&project)
        .expect_err("audio effect must fail closed");
    assert!(error[0].message.contains("not supported"));
}

#[test]
fn nested_timeline_wrapper_speed_retimes_child_render_clip() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let mut alternate = project.timeline.clone();
    alternate.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    alternate.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));
    project.timelines.push(ProjectTimeline {
        id: "alternate-speed".to_string(),
        name: "Alternate speed".to_string(),
        timeline: alternate,
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.start_seconds = 1.0;
    wrapper.duration_seconds = 2.0;
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "alternate-speed".to_string(),
    };
    wrapper.properties = BTreeMap::from([("speed".to_string(), json!(2.0))]);

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-nested-speed",
        RenderQualityProfile::DraftWebm,
    )
    .expect("nested speed render plan");
    assert_eq!(plan.clips[0].timeline_start_seconds, Some(1.0));
    assert_eq!(plan.clips[0].source_in, 0.0);
    assert_eq!(plan.clips[0].source_out, 4.0);
    assert_eq!(plan.clips[0].properties["speed"], json!(2.0));
    assert_eq!(
        plan.clips[0].properties["timelineDurationSeconds"],
        json!(2.0)
    );
}

#[test]
fn nested_timeline_wrapper_composes_motion_keyframes_into_child() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let mut alternate = project.timeline.clone();
    let child = &mut alternate.tracks[0].items[0];
    child.properties.insert("sourceIn".to_string(), json!(0.0));
    child.properties.insert("sourceOut".to_string(), json!(4.0));
    child
        .properties
        .insert("positionX".to_string(), json!(10.0));
    project.timelines.push(ProjectTimeline {
        id: "alternate-motion".to_string(),
        name: "Alternate motion".to_string(),
        timeline: alternate,
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "alternate-motion".to_string(),
    };
    wrapper.properties = BTreeMap::from([(
        "keyframes".to_string(),
        json!({
            "positionX": [
                {"atSeconds": 0.0, "value": 0.0, "easing": "linear"},
                {"atSeconds": 4.0, "value": 100.0, "easing": "linear"}
            ]
        }),
    )]);
    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "nested-motion",
        RenderQualityProfile::DraftWebm,
    )
    .expect("nested motion render plan");
    let points = plan.clips[0].properties["keyframes"]["positionX"]
        .as_array()
        .expect("position points");
    assert_eq!(points.first().unwrap()["value"], json!(10.0));
    assert!(points
        .iter()
        .any(|point| point["atSeconds"] == json!(2.0) && point["value"] == json!(60.0)));
}

#[test]
fn nested_timeline_wrapper_composes_opacity_keyframes_on_render_frames() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let mut alternate = project.timeline.clone();
    let child = &mut alternate.tracks[0].items[0];
    child.properties.insert("sourceIn".to_string(), json!(0.0));
    child.properties.insert("sourceOut".to_string(), json!(4.0));
    child.properties.insert("opacity".to_string(), json!(0.8));
    child.properties.insert(
        "keyframes".to_string(),
        json!({ "opacity": [
            { "atSeconds": 0.0, "value": 0.8 },
            { "atSeconds": 3.0, "value": 0.4 }
        ] }),
    );
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate cut".to_string(),
        timeline: alternate,
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.start_seconds = 1.0;
    wrapper.duration_seconds = 3.0;
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    wrapper.properties = BTreeMap::from([(
        "keyframes".to_string(),
        json!({ "opacity": [
            { "atSeconds": 0.0, "value": 0.5 },
            { "atSeconds": 3.0, "value": 0.25 }
        ] }),
    )]);

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-nested-opacity-keyframes",
        RenderQualityProfile::DraftWebm,
    )
    .expect("nested opacity keyframes should build a render plan");

    assert_eq!(plan.clips[0].properties["opacity"], json!(0.4));
    let opacity_keyframes = plan.clips[0].properties["keyframes"]["opacity"]
        .as_array()
        .expect("flattened opacity keyframes");
    assert_eq!(
        opacity_keyframes[0],
        json!({ "atSeconds": 0.0, "value": 0.4 })
    );
    assert!(opacity_keyframes.iter().any(|keyframe| {
        keyframe["atSeconds"] == json!(1.5) && keyframe["value"] == json!(0.225)
    }));
    assert_eq!(
        opacity_keyframes.last(),
        Some(&json!({ "atSeconds": 3.0, "value": 0.1 }))
    );
}

#[test]
fn nested_timeline_wrapper_composes_visual_fades_on_render_frames() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let mut alternate = project.timeline.clone();
    alternate.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    alternate.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));
    alternate.tracks[0].items[0]
        .properties
        .insert("opacity".to_string(), json!(0.8));
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate cut".to_string(),
        timeline: alternate,
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.start_seconds = 1.0;
    wrapper.duration_seconds = 3.0;
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    wrapper.properties = BTreeMap::from([
        ("fadeInSeconds".to_string(), json!(1.0)),
        ("fadeOutSeconds".to_string(), json!(1.0)),
    ]);

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-nested-fades",
        RenderQualityProfile::DraftWebm,
    )
    .expect("nested visual fades should build a render plan");

    assert_eq!(plan.clips[0].properties["opacity"], json!(0.0));
    let opacity_keyframes = plan.clips[0].properties["keyframes"]["opacity"]
        .as_array()
        .expect("flattened fade keyframes");
    assert!(opacity_keyframes.iter().any(|keyframe| {
        keyframe["atSeconds"] == json!(0.5) && keyframe["value"] == json!(0.4)
    }));
    assert!(opacity_keyframes.iter().any(|keyframe| {
        keyframe["atSeconds"] == json!(1.0) && keyframe["value"] == json!(0.8)
    }));
    assert_eq!(
        opacity_keyframes.last(),
        Some(&json!({ "atSeconds": 3.0, "value": 0.0 }))
    );
}

#[test]
fn nested_timeline_wrapper_composes_audio_gain_for_render() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "voiceover".to_string(),
        name: Some("Voiceover".to_string()),
        relative_path: "media/voiceover.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 4.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    let mut alternate = project.timeline.clone();
    alternate.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    alternate.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));
    alternate.tracks.push(TimelineTrack {
        transitions: Vec::new(),
        id: "alternate-audio".to_string(),
        name: "Voiceover".to_string(),
        kind: TrackKind::Audio,
        locked: false,
        sync_locked: false,
        enabled: true,
        items: vec![TimelineItem {
            id: "alternate-audio-item".to_string(),
            kind: TimelineItemKind::AudioClip,
            start_seconds: 0.0,
            duration_seconds: 4.0,
            source: TimelineSource::Media {
                media_id: "voiceover".to_string(),
            },
            label: "Voiceover".to_string(),
            properties: BTreeMap::from([("volumeDb".to_string(), json!(-3.0))]),
        }],
    });
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate cut".to_string(),
        timeline: alternate,
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    wrapper.properties = BTreeMap::from([("volumeDb".to_string(), json!(-6.0))]);

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-nested-audio-gain",
        RenderQualityProfile::DraftWebm,
    )
    .expect("nested audio gain should build a render plan");

    assert_eq!(plan.audio_clips.len(), 1);
    assert_eq!(plan.audio_clips[0].properties["volumeDb"], json!(-9.0));
    assert!(!plan.clips[0].properties.contains_key("volumeDb"));
}

#[test]
fn nested_timeline_wrapper_composes_static_canvas_transform_for_render() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let mut alternate = project.timeline.clone();
    let child = &mut alternate.tracks[0].items[0];
    child.properties.insert("sourceIn".to_string(), json!(0.0));
    child.properties.insert("sourceOut".to_string(), json!(4.0));
    child.properties.insert(
        "transform".to_string(),
        json!({ "centerX": 0.2, "centerY": 0.5, "width": 0.5, "height": 1.0, "flipHorizontal": true }),
    );
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate cut".to_string(),
        timeline: alternate,
    });
    let root_item = &mut project.timeline.tracks[0].items[0];
    root_item.start_seconds = 1.0;
    root_item.duration_seconds = 3.0;
    root_item.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    root_item.properties = BTreeMap::from([(
        "transform".to_string(),
        json!({ "centerX": 0.25, "centerY": 0.5, "width": 0.5, "height": 1.0, "flipHorizontal": true }),
    )]);

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-nested-transform",
        RenderQualityProfile::DraftWebm,
    )
    .expect("nested transform should build a render plan");

    assert_eq!(
        plan.clips[0].properties.get("transform"),
        Some(&json!({
            "centerX": 0.4,
            "centerY": 0.5,
            "width": 0.25,
            "height": 1.0,
            "flipHorizontal": false,
            "flipVertical": false,
        }))
    );
}

#[test]
fn direct_media_render_rejects_unknown_timeline_before_native_work() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let error = render_media_to_split_project_folder_for_timeline(
        dir.path(),
        &project.id,
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Final, 1920, 1080)
            .expect("explicit export options"),
        export_job_summary("export-alternate", "2026-07-10T00:00:00Z"),
        "2026-07-10T00:00:00Z",
        None,
        None,
        Some("missing-timeline"),
    )
    .expect_err("unknown timeline should be rejected before rendering");

    assert_eq!(error[0].path, "timelineId");
    assert!(error[0].message.contains("missing-timeline"));
}

#[derive(Default)]
struct RenderFixtureVisuals<'a> {
    color_grade: Option<serde_json::Value>,
    blend_mode: Option<&'a str>,
    visual_fades: Option<(f64, f64)>,
    visual_speed: Option<f64>,
    visual_keyframes: Option<serde_json::Value>,
    visual_effects: Option<serde_json::Value>,
}

fn render_export_profile_with_fixture(
    profile: ExportProfile,
    job_id: &str,
    expected_output: &str,
    visuals: RenderFixtureVisuals<'_>,
) {
    start_render_process_runtime().expect("initialize curated render runtime");
    let RenderFixtureVisuals {
        color_grade,
        blend_mode,
        visual_fades,
        visual_speed,
        visual_keyframes,
        visual_effects,
    } = visuals;
    let Some(availability) = mp4_export_profile_availability_report()
        .into_iter()
        .find(|candidate| candidate.profile == profile)
    else {
        panic!("{profile:?} profile should be reported");
    };
    if !availability.available {
        assert!(availability.unavailable_reason.is_some());
        return;
    }
    // Arbitrary visual keyframes are baked by the AppKit-hosted preparation path before
    // AVFoundation export. This plain Rust harness can still prove the compatibility render;
    // the dedicated project_export_nested_effect_appkit fixture asserts the native backend.
    let expects_avfoundation = visual_keyframes.is_none()
        && availability
            .required_runtime
            .iter()
            .any(|runtime| runtime == "system:avfoundation");

    let dir = tempfile::tempdir().expect("temp project dir");
    fs::create_dir_all(dir.path().join("media")).expect("media dir");
    generate_fixture_source_with_gstreamer(
        &dir.path().join("media/input.mp4"),
        320,
        180,
        24.0,
        4.0,
        Duration::from_secs(60),
    )
    .expect("generate fixture source");
    let mut project = sample_project();
    project.render_settings.width = 320;
    project.render_settings.height = 180;
    project.render_settings.fps = 24.0;
    project.timeline.duration_seconds = 3.0;
    project.media[0].duration_seconds = 4.0;
    project.media[0].width = Some(320);
    project.media[0].height = Some(180);
    project.media[0].fps = Some(24.0);
    let item = &mut project.timeline.tracks[0].items[0];
    item.duration_seconds = 3.0;
    item.properties.insert("sourceIn".to_string(), json!(0.0));
    item.properties.insert("sourceOut".to_string(), json!(3.0));
    if let Some(color_grade) = color_grade {
        item.properties
            .insert("colorGrade".to_string(), color_grade);
    }
    if let Some(blend_mode) = blend_mode {
        item.properties
            .insert("blendMode".to_string(), json!(blend_mode));
    }
    if let Some((fade_in_seconds, fade_out_seconds)) = visual_fades {
        item.properties
            .insert("fadeInSeconds".to_string(), json!(fade_in_seconds));
        item.properties
            .insert("fadeOutSeconds".to_string(), json!(fade_out_seconds));
    }
    if let Some(speed) = visual_speed {
        item.duration_seconds = 3.0 / speed;
        item.properties.insert("speed".to_string(), json!(speed));
    }
    if let Some(keyframes) = visual_keyframes {
        item.properties.insert("keyframes".to_string(), keyframes);
    }
    if let Some(effects) = visual_effects {
        item.properties.insert("effects".to_string(), effects);
    }
    save_split_project(dir.path(), &project).expect("save split project");

    let result = render_media_to_split_project_folder(
        dir.path(),
        &project.id,
        ExportRenderOptions::new(profile, RenderQuality::Final, 320, 180)
            .expect("explicit export options"),
        export_job_summary(job_id, "2026-07-03T20:00:00Z"),
        "2026-07-03T20:00:00Z",
        None,
        None,
    )
    .expect("render native export profile");

    assert_eq!(result.output_path, expected_output);
    assert_eq!(
        result.render_report.summary.output_path.as_deref(),
        Some(expected_output)
    );
    assert!(dir.path().join(&result.output_path).is_file());
    assert!(
        result
            .render_report
            .streams
            .as_ref()
            .expect("render report streams")
            .video
    );
    assert!(!result.output_path.ends_with(".webm"));
    assert_eq!(
        result.render_report.summary.quality,
        Some(RenderQuality::Final)
    );
    assert_eq!(result.render_report.summary.requested_width, Some(320));
    assert_eq!(result.render_report.summary.requested_height, Some(180));
    assert_eq!(result.render_report.summary.actual_width, Some(320));
    assert_eq!(result.render_report.summary.actual_height, Some(180));
    let persisted_json = fs::read_to_string(
        dir.path()
            .join(format!("renders/{job_id}/pipeline-report.json")),
    )
    .expect("read persisted JSON report");
    let persisted_json: Value =
        serde_json::from_str(&persisted_json).expect("parse persisted JSON report");
    assert_eq!(persisted_json["summary"]["quality"], "final");
    assert_eq!(persisted_json["summary"]["requestedWidth"], 320);
    assert_eq!(persisted_json["summary"]["actualHeight"], 180);
    let persisted_markdown = fs::read_to_string(
        dir.path()
            .join(format!("renders/{job_id}/pipeline-report.md")),
    )
    .expect("read persisted Markdown report");
    assert!(persisted_markdown.contains("- Quality: final"));
    assert!(persisted_markdown.contains("- Requested dimensions: 320x180"));
    assert!(persisted_markdown.contains("- Actual dimensions: 320x180"));
    let reloaded_project = load_split_project(dir.path()).expect("reload persisted project");
    let persisted_project_report = reloaded_project
        .render_reports
        .iter()
        .find(|report| report.id == job_id)
        .expect("persisted project render report");
    assert_eq!(persisted_project_report.quality, Some(RenderQuality::Final));
    assert_eq!(persisted_project_report.requested_width, Some(320));
    assert_eq!(persisted_project_report.requested_height, Some(180));
    assert_eq!(persisted_project_report.actual_width, Some(320));
    assert_eq!(persisted_project_report.actual_height, Some(180));
    if expects_avfoundation {
        assert_eq!(result.render_report.command.program, "avfoundation-native");
        assert!(result.render_report.stdout.contains("videoCodec"));
    }
}

#[test]
fn project_media_render_outputs_mp4_for_h264_profile() {
    render_export_profile_with_fixture(
        ExportProfile::Mp4H264,
        "export-h264-e2e",
        "renders/export-h264-e2e/output.mp4",
        RenderFixtureVisuals::default(),
    );
}

#[test]
fn project_media_render_outputs_hd_draft_h264_with_quality_evidence() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let Some(availability) = mp4_export_profile_availability_report()
        .into_iter()
        .find(|candidate| candidate.profile == ExportProfile::Mp4H264)
    else {
        panic!("H.264 profile should be reported");
    };
    if !availability.quality_available(RenderQuality::Draft) {
        assert!(availability
            .quality_unavailable_reason(RenderQuality::Draft)
            .is_some());
        return;
    }

    let dir = tempfile::tempdir().expect("temp project dir");
    fs::create_dir_all(dir.path().join("media")).expect("media dir");
    generate_fixture_source_with_gstreamer(
        &dir.path().join("media/input.mp4"),
        1280,
        720,
        24.0,
        1.0,
        Duration::from_secs(60),
    )
    .expect("generate HD fixture source");

    let mut project = sample_project();
    project.render_settings.width = 1920;
    project.render_settings.height = 1080;
    project.render_settings.fps = 24.0;
    project.timeline.duration_seconds = 1.0;
    project.media[0].duration_seconds = 1.0;
    project.media[0].width = Some(1280);
    project.media[0].height = Some(720);
    project.media[0].fps = Some(24.0);
    let item = &mut project.timeline.tracks[0].items[0];
    item.duration_seconds = 1.0;
    item.properties.insert("sourceIn".to_string(), json!(0.0));
    item.properties.insert("sourceOut".to_string(), json!(1.0));
    save_split_project(dir.path(), &project).expect("save split project");

    let report = render_media_to_split_project_folder(
        dir.path(),
        &project.id,
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Draft, 1280, 720)
            .expect("draft HD export options"),
        export_job_summary("export-h264-draft-hd-e2e", "2026-07-14T12:00:00Z"),
        "2026-07-14T12:00:00Z",
        None,
        None,
    )
    .expect("render Draft H.264 fixture");

    assert_eq!(
        report.render_report.summary.quality,
        Some(RenderQuality::Draft)
    );
    assert_eq!(
        (
            report.render_report.summary.requested_width,
            report.render_report.summary.requested_height,
        ),
        (Some(1280), Some(720))
    );
    assert_eq!(
        (
            report.render_report.summary.actual_width,
            report.render_report.summary.actual_height,
        ),
        (Some(1280), Some(720))
    );
    assert!(
        report
            .render_report
            .streams
            .as_ref()
            .expect("render report streams")
            .video
    );
    assert_eq!(
        report.render_report.command.program,
        platform_delivery_program()
    );
    if cfg!(target_os = "macos") {
        assert!(report.render_report.stdout.contains("videoCodec"));
    }
    assert!(dir.path().join(&report.output_path).is_file());
}

#[test]
fn project_media_render_uses_avfoundation_for_trimmed_video_audio_and_graphics() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let Some(availability) = mp4_export_profile_availability_report()
        .into_iter()
        .find(|candidate| candidate.profile == ExportProfile::Mp4H264)
    else {
        panic!("H.264 profile should be reported");
    };
    if !availability
        .required_runtime
        .iter()
        .any(|runtime| runtime == "system:avfoundation")
    {
        return;
    }

    let dir = tempfile::tempdir().expect("temp project dir");
    fs::create_dir_all(dir.path().join("media")).expect("media dir");
    generate_fixture_source_with_gstreamer(
        &dir.path().join("media/input.mp4"),
        320,
        180,
        24.0,
        4.0,
        Duration::from_secs(60),
    )
    .expect("generate system-decodable fixture source");
    let mut project = sample_project();
    project.render_settings.width = 320;
    project.render_settings.height = 180;
    project.render_settings.fps = 24.0;
    project.timeline.duration_seconds = 3.0;
    project.media[0].duration_seconds = 4.0;
    project.media[0].width = Some(320);
    project.media[0].height = Some(180);
    project.media[0].fps = Some(24.0);
    project.media.push(MediaAsset {
        id: "avfoundation-audio-source".to_string(),
        name: Some("System export audio".to_string()),
        relative_path: "media/input.mp4".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 4.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    let item = &mut project.timeline.tracks[0].items[0];
    item.duration_seconds = 3.0;
    item.properties.insert("sourceIn".to_string(), json!(0.5));
    item.properties.insert("sourceOut".to_string(), json!(3.5));
    item.properties
        .insert("fadeInSeconds".to_string(), json!(0.1));
    item.properties
        .insert("fadeOutSeconds".to_string(), json!(0.1));
    let caption_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Caption)
        .expect("caption track");
    caption_track.items.push(visual_text_item(
        "avfoundation-caption",
        TimelineItemKind::Caption,
        "SYSTEM EXPORT",
        0.75,
        1.25,
        "System export caption",
    ));
    let audio_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Audio)
        .expect("audio track");
    audio_track.items.push(TimelineItem {
        id: "avfoundation-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 3.0,
        source: TimelineSource::Media {
            media_id: "avfoundation-audio-source".to_string(),
        },
        label: "System export audio".to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(0.5)),
            ("sourceOut".to_string(), json!(3.5)),
            ("volumeDb".to_string(), json!(-3.0)),
            ("fadeInSeconds".to_string(), json!(0.2)),
            ("fadeOutSeconds".to_string(), json!(0.3)),
        ]),
    });
    save_split_project(dir.path(), &project).expect("save split project");

    let result = render_media_to_split_project_folder(
        dir.path(),
        &project.id,
        ExportRenderOptions::new(
            ExportProfile::Mp4H264,
            RenderQuality::Final,
            project.render_settings.width,
            project.render_settings.height,
        )
        .expect("explicit export options"),
        export_job_summary("export-avfoundation-combined", "2026-07-10T00:00:00Z"),
        "2026-07-10T00:00:00Z",
        None,
        None,
    )
    .expect("AVFoundation combined render");

    assert_eq!(result.render_report.command.program, "avfoundation-native");
    assert!(result.render_report.stdout.contains("\"overlayCount\":1"));
    assert_eq!(result.render_report.graphics.len(), 1);
    assert!(!result.render_report.graphics[0].sampled_frames.is_empty());
    let streams = result.render_report.streams.expect("rendered streams");
    assert!(streams.video);
    assert!(streams.audio);
    assert!(dir.path().join(result.output_path).is_file());
}

#[test]
fn project_media_render_expands_a_nested_timeline_into_native_output() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let Some(availability) = mp4_export_profile_availability_report()
        .into_iter()
        .find(|candidate| candidate.profile == ExportProfile::Mp4H264)
    else {
        panic!("H.264 profile should be reported");
    };
    if !availability.available {
        assert!(availability.unavailable_reason.is_some());
        return;
    }

    let dir = tempfile::tempdir().expect("temp project dir");
    fs::create_dir_all(dir.path().join("media")).expect("media dir");
    generate_fixture_source_with_gstreamer(
        &dir.path().join("media/input.mp4"),
        320,
        180,
        24.0,
        4.0,
        Duration::from_secs(60),
    )
    .expect("generate fixture source");

    let mut project = sample_project();
    project.media[0].duration_seconds = 4.0;
    project.media[0].width = Some(320);
    project.media[0].height = Some(180);
    project.media[0].fps = Some(24.0);
    let mut child_timeline = project.timeline.clone();
    child_timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    child_timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(3.0));
    project.timelines.push(ProjectTimeline {
        id: "nested-cut".to_string(),
        name: "Nested cut".to_string(),
        timeline: child_timeline,
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.start_seconds = 0.0;
    wrapper.duration_seconds = 3.0;
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "nested-cut".to_string(),
    };
    wrapper.properties.clear();
    save_split_project(dir.path(), &project).expect("save nested split project");

    let result = render_media_to_split_project_folder(
        dir.path(),
        &project.id,
        ExportRenderOptions::new(
            ExportProfile::Mp4H264,
            RenderQuality::Final,
            project.render_settings.width,
            project.render_settings.height,
        )
        .expect("explicit export options"),
        export_job_summary("export-nested-h264-e2e", "2026-07-10T00:00:00Z"),
        "2026-07-10T00:00:00Z",
        None,
        None,
    )
    .expect("nested timeline should render natively");

    assert_eq!(
        result.output_path,
        "renders/export-nested-h264-e2e/output.mp4"
    );
    assert!(dir.path().join(&result.output_path).is_file());
    assert!(
        result
            .render_report
            .streams
            .as_ref()
            .expect("render report streams")
            .video
    );
}

#[test]
fn project_media_render_outputs_mp4_for_h265_profile() {
    render_export_profile_with_fixture(
        ExportProfile::Mp4H265,
        "export-h265-e2e",
        "renders/export-h265-e2e/output.mp4",
        RenderFixtureVisuals::default(),
    );
}

#[test]
fn project_media_render_outputs_mp4_from_still_image_with_avfoundation() {
    let availability = mp4_export_profile_availability_report()
        .into_iter()
        .find(|candidate| candidate.profile == ExportProfile::Mp4H264)
        .expect("H.264 availability");
    if !availability.available {
        assert!(availability.unavailable_reason.is_some());
        return;
    }

    let dir = tempfile::tempdir().expect("temp project dir");
    fs::create_dir_all(dir.path().join("media")).expect("media dir");
    let png = base64::engine::general_purpose::STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")
        .expect("tiny PNG fixture");
    fs::write(dir.path().join("media/still.png"), png).expect("write PNG fixture");

    let mut project = sample_project();
    project.render_settings.width = 320;
    project.render_settings.height = 180;
    project.render_settings.fps = 12.0;
    project.timeline.duration_seconds = 2.0;
    project.media[0] = MediaAsset {
        id: "still-1".to_string(),
        name: Some("Still".to_string()),
        relative_path: "media/still.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1),
        height: Some(1),
        fps: None,
        folder_id: None,
    };
    let item = &mut project.timeline.tracks[0].items[0];
    item.kind = TimelineItemKind::ImageClip;
    item.source = TimelineSource::Media {
        media_id: "still-1".to_string(),
    };
    item.duration_seconds = 2.0;
    item.properties.remove("sourceIn");
    item.properties.remove("sourceOut");
    save_split_project(dir.path(), &project).expect("save split project");

    let result = render_media_to_split_project_folder(
        dir.path(),
        &project.id,
        ExportRenderOptions::new(
            ExportProfile::Mp4H264,
            RenderQuality::Final,
            project.render_settings.width,
            project.render_settings.height,
        )
        .expect("explicit export options"),
        export_job_summary("export-still-avfoundation", "2026-07-11T00:00:00Z"),
        "2026-07-11T00:00:00Z",
        None,
        None,
    )
    .expect("render still image through AVFoundation");

    assert_eq!(
        result.render_report.command.program,
        platform_delivery_program()
    );
    assert!(dir.path().join(&result.output_path).is_file());
    assert!(result
        .render_report
        .streams
        .as_ref()
        .is_some_and(|streams| streams.video));
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "covered by project_export_nested_effect_appkit; GStreamer video decode requires an AppKit main loop on macOS"
)]
fn project_media_render_outputs_mp4_with_basic_color_grade() {
    render_export_profile_with_fixture(
        ExportProfile::Mp4H264,
        "export-h264-grade-e2e",
        "renders/export-h264-grade-e2e/output.mp4",
        RenderFixtureVisuals {
            color_grade: Some(json!({ "exposure": 0.5, "contrast": 1.2, "saturation": 0.8 })),
            ..RenderFixtureVisuals::default()
        },
    );
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "covered by project_export_nested_effect_appkit; GStreamer video decode requires an AppKit main loop on macOS"
)]
fn project_media_render_outputs_mp4_with_add_blend_mode() {
    render_export_profile_with_fixture(
        ExportProfile::Mp4H264,
        "export-h264-blend-e2e",
        "renders/export-h264-blend-e2e/output.mp4",
        RenderFixtureVisuals {
            blend_mode: Some("add"),
            ..RenderFixtureVisuals::default()
        },
    );
}

#[test]
fn project_media_render_outputs_mp4_with_visual_fades() {
    render_export_profile_with_fixture(
        ExportProfile::Mp4H264,
        "export-h264-fades-e2e",
        "renders/export-h264-fades-e2e/output.mp4",
        RenderFixtureVisuals {
            visual_fades: Some((0.5, 0.5)),
            ..RenderFixtureVisuals::default()
        },
    );
}

#[test]
fn project_media_render_outputs_mp4_with_visual_speed() {
    render_export_profile_with_fixture(
        ExportProfile::Mp4H264,
        "export-h264-speed-e2e",
        "renders/export-h264-speed-e2e/output.mp4",
        RenderFixtureVisuals {
            visual_speed: Some(2.0),
            ..RenderFixtureVisuals::default()
        },
    );
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "covered by project_export_nested_effect_appkit; arbitrary visual keyframes require the AppKit-hosted preparation path on macOS"
)]
fn project_media_render_outputs_mp4_with_visual_opacity_keyframes() {
    render_export_profile_with_fixture(
        ExportProfile::Mp4H264,
        "export-h264-opacity-keyframes-e2e",
        "renders/export-h264-opacity-keyframes-e2e/output.mp4",
        RenderFixtureVisuals {
            visual_fades: Some((0.25, 0.25)),
            visual_keyframes: Some(json!({
                "opacity": [
                    { "atSeconds": 0.0, "value": 0.2 },
                    { "atSeconds": 1.5, "value": 0.85 },
                    { "atSeconds": 3.0, "value": 0.4 }
                ]
            })),
            ..RenderFixtureVisuals::default()
        },
    );
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "covered by project_export_nested_effect_appkit; arbitrary visual keyframes require the AppKit-hosted preparation path on macOS"
)]
fn project_media_render_outputs_mp4_with_visual_position_keyframes() {
    render_export_profile_with_fixture(
        ExportProfile::Mp4H264,
        "export-h264-position-keyframes-e2e",
        "renders/export-h264-position-keyframes-e2e/output.mp4",
        RenderFixtureVisuals {
            visual_keyframes: Some(json!({
                "positionX": [
                    { "atSeconds": 0.0, "value": 0.0 },
                    { "atSeconds": 1.5, "value": 24.0 },
                    { "atSeconds": 3.0, "value": 0.0 }
                ],
                "positionY": [
                    { "atSeconds": 0.0, "value": 0.0 },
                    { "atSeconds": 1.5, "value": 12.0 },
                    { "atSeconds": 3.0, "value": 0.0 }
                ]
            })),
            ..RenderFixtureVisuals::default()
        },
    );
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "covered by project_export_nested_effect_appkit; arbitrary visual keyframes require the AppKit-hosted preparation path on macOS"
)]
fn project_media_render_outputs_mp4_with_visual_scale_keyframes() {
    render_export_profile_with_fixture(
        ExportProfile::Mp4H264,
        "export-h264-scale-keyframes-e2e",
        "renders/export-h264-scale-keyframes-e2e/output.mp4",
        RenderFixtureVisuals {
            visual_keyframes: Some(json!({
                "scale": [
                    { "atSeconds": 0.0, "value": 0.85 },
                    { "atSeconds": 1.5, "value": 1.0 },
                    { "atSeconds": 3.0, "value": 0.9 }
                ]
            })),
            ..RenderFixtureVisuals::default()
        },
    );
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "covered by project_export_nested_effect_appkit; arbitrary visual keyframes require the AppKit-hosted preparation path on macOS"
)]
fn project_media_render_outputs_mp4_with_visual_rotation_keyframes() {
    render_export_profile_with_fixture(
        ExportProfile::Mp4H264,
        "export-h264-rotation-keyframes-e2e",
        "renders/export-h264-rotation-keyframes-e2e/output.mp4",
        RenderFixtureVisuals {
            visual_keyframes: Some(json!({
                "rotationDegrees": [
                    { "atSeconds": 0.0, "value": 0.0 },
                    { "atSeconds": 1.5, "value": 12.0 },
                    { "atSeconds": 3.0, "value": 0.0 }
                ]
            })),
            ..RenderFixtureVisuals::default()
        },
    );
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "covered by project_export_nested_effect_appkit; arbitrary visual keyframes require the AppKit-hosted preparation path on macOS"
)]
fn project_media_render_outputs_mp4_with_visual_crop_keyframes() {
    render_export_profile_with_fixture(
        ExportProfile::Mp4H264,
        "export-h264-crop-keyframes-e2e",
        "renders/export-h264-crop-keyframes-e2e/output.mp4",
        RenderFixtureVisuals {
            visual_keyframes: Some(json!({
                "cropTop": [
                    { "atSeconds": 0.0, "value": 0.0 },
                    { "atSeconds": 1.5, "value": 0.08 },
                    { "atSeconds": 3.0, "value": 0.0 }
                ],
                "cropRight": [
                    { "atSeconds": 0.0, "value": 0.0 },
                    { "atSeconds": 1.5, "value": 0.06 },
                    { "atSeconds": 3.0, "value": 0.0 }
                ]
            })),
            ..RenderFixtureVisuals::default()
        },
    );
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "covered by project_export_nested_effect_appkit; GStreamer video decode requires an AppKit main loop on macOS"
)]
fn project_media_render_outputs_mp4_with_grain_and_vignette() {
    render_export_profile_with_fixture(
        ExportProfile::Mp4H264,
        "export-h264-finishing-effects-e2e",
        "renders/export-h264-finishing-effects-e2e/output.mp4",
        RenderFixtureVisuals {
            visual_effects: Some(json!([
                { "effectType": "stylize.grain", "enabled": true, "params": { "amount": 0.18, "size": 1.5 } },
                { "effectType": "stylize.vignette", "enabled": true, "params": { "amount": -0.25, "midpoint": 0.5, "roundness": 0.0, "feather": 0.5 } }
            ])),
            ..RenderFixtureVisuals::default()
        },
    );
}

#[test]
fn project_media_render_records_failed_job_when_preflight_rejects_range() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let result = render_media_to_split_project_folder(
        dir.path(),
        &project.id,
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Draft, 1280, 720)
            .expect("explicit export options"),
        export_job_summary("export-invalid-range", "2026-07-03T20:00:00Z"),
        "2026-07-03T20:00:01Z",
        None,
        Some((3.0, 1.0)),
    )
    .expect_err("invalid render range should fail before render");

    assert!(result.iter().any(|error| error.path == "timeline.range"));

    let project = load_split_project(dir.path()).expect("load split project");
    let job = project
        .jobs
        .iter()
        .find(|job| job.id == "export-invalid-range")
        .expect("failed render job should be persisted");
    assert_eq!(job.status, JobStatus::Failed);
    assert_eq!(job.updated_at, "2026-07-03T20:00:01Z");

    let validation = validate_split_project(dir.path()).expect("validate split project");
    assert!(
        validation.ok,
        "failed diagnostic reports should not corrupt split-project sidecar validation: {:?}",
        validation.issues
    );

    let report_path = dir
        .path()
        .join("renders/export-invalid-range/pipeline-report.json");
    let report_json = fs::read_to_string(&report_path).expect("failed render report json");
    let report: RenderReport = serde_json::from_str(&report_json).expect("failed render report");
    assert_eq!(report.job_id, "export-invalid-range");
    assert_eq!(report.summary.status, "failed");
    assert!(report
        .errors
        .iter()
        .any(|error| error.path == "timeline.range"));
    assert!(report
        .artifacts
        .contains(&"renders/export-invalid-range/pipeline-report.json".to_string()));
    assert!(report
        .artifacts
        .contains(&"renders/export-invalid-range/pipeline-report.md".to_string()));
    assert!(report
        .artifacts
        .contains(&"renders/export-invalid-range/render.log".to_string()));
    let loaded_report = load_project_render_pipeline_report(dir.path(), "export-invalid-range")
        .expect("failed report should be loadable for the retry UI");
    assert_eq!(loaded_report, report);
    assert!(dir
        .path()
        .join("renders/export-invalid-range/pipeline-report.md")
        .is_file());
    let log = fs::read_to_string(dir.path().join("renders/export-invalid-range/render.log"))
        .expect("failed render log");
    assert!(log.contains("timeline.range"));
}

#[test]
fn project_media_render_plan_preserves_h265_output_profile() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let item = &mut project.timeline.tracks[0].items[0];
    item.properties.insert("sourceIn".to_string(), json!(1.0));
    item.properties.insert("sourceOut".to_string(), json!(3.0));

    let plan = build_project_media_render_plan(
        dir.path(),
        &project,
        "export-h265",
        RenderQuality::Final,
        ExportProfile::Mp4H265,
    )
    .expect("h265 render plan");

    assert_eq!(
        serde_json::to_value(plan.output_profile).expect("output profile"),
        json!("mp4Modern")
    );
    assert_eq!(
        plan.output_path,
        dir.path()
            .join("renders/export-h265/output.mp4")
            .display()
            .to_string()
    );
}

#[test]
fn project_webm_render_plan_uses_ordered_timeline_source_ranges() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let video_track = &mut project.timeline.tracks[0];
    video_track.items[0].start_seconds = 3.0;
    video_track.items[0].duration_seconds = 1.5;
    video_track.items[0]
        .properties
        .insert("sourceIn".to_string(), json!(4.0));
    video_track.items[0]
        .properties
        .insert("sourceOut".to_string(), json!(5.5));
    let mut second = video_track.items[0].clone();
    second.id = "item-2".to_string();
    second.start_seconds = 0.0;
    second.duration_seconds = 2.0;
    second.properties.insert("sourceIn".to_string(), json!(1.0));
    second
        .properties
        .insert("sourceOut".to_string(), json!(3.0));
    video_track.items.push(second);

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-draft-1",
        RenderQualityProfile::DraftWebm,
    )
    .expect("project timeline should build a render plan");

    assert_eq!(
        plan.input_path,
        dir.path().join("media/input.mp4").display().to_string()
    );
    assert_eq!(
        plan.output_path,
        dir.path()
            .join("renders/render-draft-1/output.webm")
            .display()
            .to_string()
    );
    assert_eq!(plan.width, 1280);
    assert_eq!(plan.height, 720);
    assert_eq!(plan.fps, 24.0);
    assert_eq!(plan.quality, RenderQuality::Draft);
    assert_eq!(plan.clips.len(), 2);
    assert_eq!(plan.clips[0].timeline_start_seconds, Some(0.0));
    assert_eq!(plan.clips[0].source_in, 1.0);
    assert_eq!(plan.clips[0].source_out, 3.0);
    assert_eq!(plan.clips[1].timeline_start_seconds, Some(3.0));
    assert_eq!(plan.clips[1].source_in, 4.0);
    assert_eq!(plan.clips[1].source_out, 5.5);
}

#[test]
fn project_webm_render_plan_routes_enabled_visual_and_audio_tracks() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let mut upper_clip = {
        let primary = &mut project.timeline.tracks[0].items[0];
        primary
            .properties
            .insert("sourceIn".to_string(), json!(0.0));
        primary
            .properties
            .insert("sourceOut".to_string(), json!(2.0));
        primary.clone()
    };
    let visual_track_index = project.timeline.tracks.len() as u32;
    upper_clip.id = "upper-video-item".to_string();
    upper_clip.start_seconds = 0.5;
    upper_clip.duration_seconds = 1.5;
    upper_clip
        .properties
        .insert("sourceIn".to_string(), json!(2.0));
    upper_clip
        .properties
        .insert("sourceOut".to_string(), json!(3.5));
    project.timeline.tracks.push(TimelineTrack {
        transitions: Vec::new(),
        id: "track-upper-video".to_string(),
        name: "Upper video".to_string(),
        kind: TrackKind::Video,
        locked: false,
        sync_locked: false,
        enabled: true,
        items: vec![upper_clip],
    });

    project.media.push(MediaAsset {
        id: "music-media".to_string(),
        name: Some("Music".to_string()),
        relative_path: "media/music.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 4.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    let audio_track_index = project.timeline.tracks.len() as u32;
    project.timeline.tracks.push(TimelineTrack {
        transitions: Vec::new(),
        id: "track-audio".to_string(),
        name: "Audio".to_string(),
        kind: TrackKind::Audio,
        locked: false,
        sync_locked: false,
        enabled: true,
        items: vec![TimelineItem {
            id: "music-item".to_string(),
            kind: TimelineItemKind::AudioClip,
            start_seconds: 0.25,
            duration_seconds: 2.0,
            source: TimelineSource::Media {
                media_id: "music-media".to_string(),
            },
            label: "Music".to_string(),
            properties: BTreeMap::from([
                ("sourceIn".to_string(), json!(1.0)),
                ("sourceOut".to_string(), json!(3.0)),
                ("volumeDb".to_string(), json!(-6.0)),
                ("fadeInSeconds".to_string(), json!(0.25)),
                ("fadeOutSeconds".to_string(), json!(0.5)),
                (
                    "keyframes".to_string(),
                    json!({"volumeDb":[
                        {"atSeconds":0.0,"value":-18.0,"easing":"easeInOut"},
                        {"atSeconds":2.0,"value":-3.0,"easing":"linear"}
                    ]}),
                ),
            ]),
        }],
    });

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-multitrack",
        RenderQualityProfile::DraftWebm,
    )
    .expect("enabled visual and audio tracks should enter the render plan");

    assert_eq!(plan.clips.len(), 2);
    assert_eq!(plan.clips[0].timeline_track_index, 0);
    assert_eq!(plan.clips[1].timeline_track_index, visual_track_index);
    assert_eq!(plan.clips[0].properties["sourceWidth"], json!(1920));
    assert_eq!(plan.clips[0].properties["sourceHeight"], json!(1080));
    assert_eq!(plan.audio_clips.len(), 1);
    assert_eq!(plan.audio_clips[0].timeline_track_index, audio_track_index);
    assert_eq!(plan.audio_clips[0].timeline_start_seconds, Some(0.25));
    assert_eq!(plan.audio_clips[0].source_in, 1.0);
    assert_eq!(plan.audio_clips[0].source_out, 3.0);
    assert_eq!(plan.audio_clips[0].properties["volumeDb"], json!(-6.0));
    assert_eq!(plan.audio_clips[0].properties["fadeInSeconds"], json!(0.25));
    assert_eq!(plan.audio_clips[0].properties["fadeOutSeconds"], json!(0.5));
    assert_eq!(
        plan.audio_clips[0].properties["keyframes"]["volumeDb"][0]["value"],
        json!(-18.0)
    );
}

#[test]
fn project_webm_render_plan_accepts_generated_audio_outputs() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let video_item = &mut project.timeline.tracks[0].items[0];
    video_item
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    video_item
        .properties
        .insert("sourceOut".to_string(), json!(4.0));
    project.media.push(MediaAsset {
        id: "generated-audio".to_string(),
        name: Some("Generated voiceover".to_string()),
        relative_path: "generated/voiceover.mp3".to_string(),
        kind: MediaKind::Generated,
        duration_seconds: 3.5,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    project.timeline.tracks.push(TimelineTrack {
        transitions: Vec::new(),
        id: "track-generated-audio".to_string(),
        name: "Generated audio".to_string(),
        kind: TrackKind::Audio,
        locked: false,
        sync_locked: false,
        enabled: true,
        items: vec![TimelineItem {
            id: "generated-audio-item".to_string(),
            kind: TimelineItemKind::AudioClip,
            start_seconds: 0.0,
            duration_seconds: 3.5,
            source: TimelineSource::Media {
                media_id: "generated-audio".to_string(),
            },
            label: "Generated voiceover".to_string(),
            properties: BTreeMap::from([
                ("sourceIn".to_string(), json!(0.0)),
                ("sourceOut".to_string(), json!(3.5)),
            ]),
        }],
    });

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-generated-audio",
        RenderQualityProfile::DraftWebm,
    )
    .expect("completed generated audio should be renderable on an audio track");

    assert_eq!(plan.audio_clips.len(), 1);
    assert_eq!(plan.audio_clips[0].source_in, 0.0);
    assert_eq!(plan.audio_clips[0].source_out, 3.5);
    assert!(plan.audio_clips[0]
        .source_path
        .as_deref()
        .is_some_and(|path| path.ends_with("generated/voiceover.mp3")));
}

#[test]
fn project_webm_render_plan_preserves_clip_look_and_mix_properties() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let item = &mut project.timeline.tracks[0].items[0];
    item.properties.insert("sourceIn".to_string(), json!(1.0));
    item.properties.insert("sourceOut".to_string(), json!(3.5));
    item.properties.insert("opacity".to_string(), json!(0.42));
    item.properties
        .insert("blendMode".to_string(), json!("add"));
    item.properties.insert("volumeDb".to_string(), json!(-8.0));
    item.properties
        .insert("fadeOutSeconds".to_string(), json!(0.75));
    item.properties.insert("centerX".to_string(), json!(0.45));
    item.properties
        .insert("flipHorizontal".to_string(), json!(true));
    item.properties.insert(
        "colorGrade".to_string(),
        json!({
            "exposure": 0.3,
            "vibrance": 0.4,
            "lut": {
                "path": "looks/warm.cube",
                "strength": 0.6
            }
        }),
    );
    item.properties.insert(
        "effects".to_string(),
        json!([
            {
                "effectType": "stylize.glow",
                "enabled": true,
                "params": {
                    "radius": 12.0,
                    "intensity": 0.5
                }
            }
        ]),
    );

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-look-mix",
        RenderQualityProfile::DraftWebm,
    )
    .expect("clip look and mix metadata should not block render-plan construction");

    assert_eq!(plan.clips.len(), 1);
    assert_eq!(plan.clips[0].properties["opacity"], json!(0.42));
    assert_eq!(plan.clips[0].properties["blendMode"], json!("add"));
    assert_eq!(plan.clips[0].properties["volumeDb"], json!(-8.0));
    assert_eq!(plan.clips[0].properties["fadeOutSeconds"], json!(0.75));
    assert_eq!(plan.clips[0].properties["centerX"], json!(0.45));
    assert_eq!(plan.clips[0].properties["flipHorizontal"], json!(true));
    assert_eq!(
        plan.clips[0].properties["colorGrade"]["lut"]["path"],
        json!("looks/warm.cube")
    );
    let effects = plan.clips[0].properties["effects"]
        .as_array()
        .expect("canonical effect stack");
    assert_eq!(effects[0]["effectType"], json!("color.exposure"));
    assert!(effects
        .iter()
        .any(|effect| effect["effectType"] == json!("stylize.glow")));
}

#[test]
fn project_webm_render_plan_rejects_audio_denoise_without_bake_support() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let video_item = &mut project.timeline.tracks[0].items[0];
    video_item
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    video_item
        .properties
        .insert("sourceOut".to_string(), json!(4.0));
    project.media.push(MediaAsset {
        id: "audio-1".to_string(),
        name: Some("Voiceover".to_string()),
        relative_path: "media/voiceover.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 4.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    project.timeline.tracks.push(TimelineTrack {
        transitions: Vec::new(),
        id: "track-audio".to_string(),
        name: "Audio".to_string(),
        kind: TrackKind::Audio,
        locked: false,
        sync_locked: false,
        enabled: true,
        items: vec![TimelineItem {
            id: "audio-item-1".to_string(),
            kind: TimelineItemKind::AudioClip,
            start_seconds: 0.0,
            duration_seconds: 4.0,
            source: TimelineSource::Media {
                media_id: "audio-1".to_string(),
            },
            label: "Voiceover".to_string(),
            properties: BTreeMap::from([(
                "effects".to_string(),
                json!([
                    {
                        "effectType": "audio.denoise",
                        "enabled": true,
                        "params": { "amount": 0.6 }
                    }
                ]),
            )]),
        }],
    });

    let errors = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-denoise",
        RenderQualityProfile::DraftWebm,
    )
    .expect_err("enabled audio denoise should require bake support before rendering");

    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(
        errors[0].path,
        "timeline.tracks.audio.items[0].effects.audio.denoise"
    );
    assert!(errors[0].message.contains("audio denoise"));
    assert!(errors[0].fix.contains("Denoise"));
}

#[test]
fn project_webm_render_plan_for_range_trims_and_rebases_source_clips() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let video_track = &mut project.timeline.tracks[0];
    video_track.items[0].start_seconds = 0.0;
    video_track.items[0].duration_seconds = 4.0;
    video_track.items[0]
        .properties
        .insert("sourceIn".to_string(), json!(2.0));
    video_track.items[0]
        .properties
        .insert("sourceOut".to_string(), json!(10.0));
    video_track.items[0]
        .properties
        .insert("speed".to_string(), json!(2.0));
    let mut second = video_track.items[0].clone();
    second.id = "item-2".to_string();
    second.start_seconds = 4.0;
    second.duration_seconds = 3.0;
    second.properties.insert("sourceIn".to_string(), json!(8.0));
    second
        .properties
        .insert("sourceOut".to_string(), json!(11.0));
    second.properties.insert("speed".to_string(), json!(1.0));
    video_track.items.push(second);

    let plan = build_project_webm_render_plan_for_range(
        dir.path(),
        &project,
        "save-range-1",
        RenderQualityProfile::DraftWebm,
        1.5,
        5.25,
    )
    .expect("selected timeline range should build a rebased render plan");

    assert_eq!(
        plan.output_path,
        dir.path()
            .join("renders/save-range-1/output.webm")
            .display()
            .to_string()
    );
    assert_eq!(plan.clips.len(), 2);
    assert_eq!(plan.clips[0].timeline_start_seconds, Some(0.0));
    assert_eq!(plan.clips[0].source_in, 5.0);
    assert_eq!(plan.clips[0].source_out, 10.0);
    assert_eq!(plan.clips[1].timeline_start_seconds, Some(2.5));
    assert_eq!(plan.clips[1].source_in, 8.0);
    assert_eq!(plan.clips[1].source_out, 9.25);
}

#[test]
fn project_webm_render_plan_for_range_ignores_invalid_non_overlapping_clips() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let video_track = &mut project.timeline.tracks[0];
    video_track.items[0].start_seconds = 0.0;
    video_track.items[0].duration_seconds = 2.0;
    video_track.items[0]
        .properties
        .insert("sourceIn".to_string(), json!(1.0));
    video_track.items[0]
        .properties
        .insert("sourceOut".to_string(), json!(3.0));
    let mut non_overlapping = video_track.items[0].clone();
    non_overlapping.id = "invalid-outside-range".to_string();
    non_overlapping.start_seconds = 5.0;
    non_overlapping.duration_seconds = 2.0;
    non_overlapping.properties.remove("sourceOut");
    video_track.items.push(non_overlapping);

    let plan = build_project_webm_render_plan_for_range(
        dir.path(),
        &project,
        "save-range-2",
        RenderQualityProfile::DraftWebm,
        0.25,
        1.25,
    )
    .expect("range render should ignore invalid clips outside the selected range");

    assert_eq!(plan.clips.len(), 1);
    assert_eq!(plan.clips[0].timeline_start_seconds, Some(0.0));
    assert_eq!(plan.clips[0].source_in, 1.25);
    assert_eq!(plan.clips[0].source_out, 2.25);
}

#[test]
fn project_webm_render_plan_supports_multiple_source_media_assets() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "media-2".to_string(),
        name: Some("Second camera".to_string()),
        relative_path: "media/second-camera.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 8.0,
        width: Some(1280),
        height: Some(720),
        fps: Some(30.0),
        folder_id: None,
    });
    let video_track = &mut project.timeline.tracks[0];
    video_track.items[0].start_seconds = 0.0;
    video_track.items[0].duration_seconds = 1.5;
    video_track.items[0]
        .properties
        .insert("sourceIn".to_string(), json!(4.0));
    video_track.items[0]
        .properties
        .insert("sourceOut".to_string(), json!(5.5));
    let mut second = video_track.items[0].clone();
    second.id = "item-2".to_string();
    second.start_seconds = 1.5;
    second.duration_seconds = 2.0;
    second.source = TimelineSource::Media {
        media_id: "media-2".to_string(),
    };
    second.properties.insert("sourceIn".to_string(), json!(1.0));
    second
        .properties
        .insert("sourceOut".to_string(), json!(3.0));
    video_track.items.push(second);

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-draft-2",
        RenderQualityProfile::DraftWebm,
    )
    .expect("multi-source timeline should build a render plan");

    let first_source_path = dir.path().join("media/input.mp4").display().to_string();
    let second_source_path = dir
        .path()
        .join("media/second-camera.mp4")
        .display()
        .to_string();

    assert_eq!(plan.clips.len(), 2);
    assert_eq!(
        plan.clips[0].source_path.as_deref(),
        Some(first_source_path.as_str())
    );
    assert_eq!(
        plan.clips[1].source_path.as_deref(),
        Some(second_source_path.as_str())
    );
    assert_eq!(plan.clips[0].timeline_start_seconds, Some(0.0));
    assert_eq!(plan.clips[0].source_in, 4.0);
    assert_eq!(plan.clips[0].source_out, 5.5);
    assert_eq!(plan.clips[1].timeline_start_seconds, Some(1.5));
    assert_eq!(plan.clips[1].source_in, 1.0);
    assert_eq!(plan.clips[1].source_out, 3.0);
}

#[test]
fn project_webm_render_plan_rejects_lottie_clip_with_actionable_error() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "lottie-1".to_string(),
        name: Some("Animated Badge".to_string()),
        relative_path: "media/animated-badge.json".to_string(),
        kind: MediaKind::Lottie,
        duration_seconds: 2.0,
        width: Some(1280),
        height: Some(720),
        fps: Some(30.0),
        folder_id: None,
    });
    let video_track = &mut project.timeline.tracks[0];
    video_track.items[0].source = TimelineSource::Media {
        media_id: "lottie-1".to_string(),
    };
    video_track.items[0].start_seconds = 0.0;
    video_track.items[0].duration_seconds = 2.0;
    video_track.items[0]
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    video_track.items[0]
        .properties
        .insert("sourceOut".to_string(), json!(2.0));

    let errors = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-lottie",
        RenderQualityProfile::DraftWebm,
    )
    .expect_err("Lottie clips need a bake step before WebM rendering");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(
        errors[0].path,
        "timeline.tracks.video.items[0].source.mediaId"
    );
    assert_eq!(
        errors[0].message,
        "Lottie media must be baked before WebM rendering."
    );
    assert_eq!(errors[0].details["mediaId"], "lottie-1");
    assert_eq!(errors[0].details["mediaKind"], "lottie");
}

#[test]
fn project_webm_render_plan_preserves_timeline_gaps() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let video_track = &mut project.timeline.tracks[0];
    video_track.items[0].start_seconds = 0.75;
    video_track.items[0].duration_seconds = 1.5;
    video_track.items[0]
        .properties
        .insert("sourceIn".to_string(), json!(4.0));
    video_track.items[0]
        .properties
        .insert("sourceOut".to_string(), json!(5.5));
    let mut second = video_track.items[0].clone();
    second.id = "item-2".to_string();
    second.start_seconds = 4.25;
    second.duration_seconds = 2.0;
    second.properties.insert("sourceIn".to_string(), json!(1.0));
    second
        .properties
        .insert("sourceOut".to_string(), json!(3.0));
    video_track.items.push(second);

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-draft-gaps",
        RenderQualityProfile::DraftWebm,
    )
    .expect("gapped project timeline should build a render plan");

    assert_eq!(plan.clips.len(), 2);
    assert_eq!(plan.clips[0].timeline_start_seconds, Some(0.75));
    assert_eq!(plan.clips[1].timeline_start_seconds, Some(4.25));
}

#[test]
fn project_webm_render_plan_ignores_disabled_video_tracks() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    let disabled_item = project.timeline.tracks[0].items[0].clone();
    project.timeline.tracks[0].enabled = false;
    let mut enabled_item = disabled_item.clone();
    enabled_item.id = "enabled-video-item".to_string();
    enabled_item.start_seconds = 2.0;
    enabled_item.duration_seconds = 1.25;
    enabled_item
        .properties
        .insert("sourceIn".to_string(), json!(6.0));
    enabled_item
        .properties
        .insert("sourceOut".to_string(), json!(7.25));
    let mut enabled_track =
        TimelineTrack::empty("track-video-enabled", "Enabled Video", TrackKind::Video);
    enabled_track.items.push(enabled_item);
    project.timeline.tracks.push(enabled_track);

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-enabled-video",
        RenderQualityProfile::DraftWebm,
    )
    .expect("enabled video track should build a render plan");

    assert_eq!(plan.clips.len(), 1);
    assert_eq!(plan.clips[0].timeline_start_seconds, Some(2.0));
    assert_eq!(plan.clips[0].source_in, 6.0);
    assert_eq!(plan.clips[0].source_out, 7.25);
}

#[test]
fn project_webm_render_plan_resolves_completed_generated_outputs() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "generated-shot-1-output".to_string(),
        name: Some("Generated shot".to_string()),
        relative_path: "generated/generated-shot-1/output.mp4".to_string(),
        kind: MediaKind::Generated,
        duration_seconds: 4.0,
        width: Some(1280),
        height: Some(720),
        fps: Some(30.0),
        folder_id: None,
    });
    project.generated_assets.push(GeneratedAsset {
        schema_version: 1,
        id: "generated-shot-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Completed,
        name: Some("Generated shot".to_string()),
        target_folder_id: None,
        placement_intent: None,
        prompt: "Create an establishing shot.".to_string(),
        model: GenerationModel {
            provider: "mock".to_string(),
            id: "mock-video".to_string(),
        },
        references: GeneratedAssetReferences {
            media_ids: Vec::new(),
            first_frame_media_id: None,
            last_frame_media_id: None,
            provider_input_urls: Vec::new(),
            ..Default::default()
        },
        settings: GeneratedAssetSettings::default(),
        outputs: vec![GeneratedAssetOutput {
            media_id: "generated-shot-1-output".to_string(),
            relative_path: "generated/generated-shot-1/output.mp4".to_string(),
            source_url: None,
            width: 1280,
            height: 720,
            duration_seconds: 4.0,
            fps: 30.0,
        }],
        created_at: "2026-07-01T00:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    });
    let video_track = &mut project.timeline.tracks[0];
    video_track.items[0].source = TimelineSource::Generated {
        artifact_id: "generated-shot-1".to_string(),
    };
    video_track.items[0].start_seconds = 1.25;
    video_track.items[0].duration_seconds = 2.0;
    video_track.items[0]
        .properties
        .insert("sourceIn".to_string(), json!(0.5));
    video_track.items[0]
        .properties
        .insert("sourceOut".to_string(), json!(2.5));

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-generated-output",
        RenderQualityProfile::DraftWebm,
    )
    .expect("completed generated output should build a render plan");

    let expected_source_path = dir
        .path()
        .join("generated/generated-shot-1/output.mp4")
        .display()
        .to_string();

    assert_eq!(plan.clips.len(), 1);
    assert_eq!(
        plan.clips[0].source_path.as_deref(),
        Some(expected_source_path.as_str())
    );
    assert_eq!(plan.clips[0].timeline_start_seconds, Some(1.25));
    assert_eq!(plan.clips[0].source_in, 0.5);
    assert_eq!(plan.clips[0].source_out, 2.5);
}

#[test]
fn project_webm_render_plan_accepts_source_backed_image_clips() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "still-1".to_string(),
        name: Some("Still frame".to_string()),
        relative_path: "media/still-1.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 3.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(1.0),
        folder_id: None,
    });

    let image_item = &mut project.timeline.tracks[0].items[0];
    image_item.id = "image-clip-1".to_string();
    image_item.kind = TimelineItemKind::ImageClip;
    image_item.source = TimelineSource::Media {
        media_id: "still-1".to_string(),
    };
    image_item.start_seconds = 1.5;
    image_item.duration_seconds = 2.0;
    image_item
        .properties
        .insert("sourceIn".to_string(), json!(0.25));
    image_item
        .properties
        .insert("sourceOut".to_string(), json!(2.25));

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "render-image-clip",
        RenderQualityProfile::DraftWebm,
    )
    .expect("source-backed image clip should build a render plan");

    let expected_source_path = dir.path().join("media/still-1.png").display().to_string();

    assert_eq!(plan.clips.len(), 1);
    assert_eq!(
        plan.clips[0].source_path.as_deref(),
        Some(expected_source_path.as_str())
    );
    assert_eq!(plan.clips[0].timeline_start_seconds, Some(1.5));
    assert_eq!(plan.clips[0].source_in, 0.25);
    assert_eq!(plan.clips[0].source_out, 2.25);
}

#[test]
fn project_graphics_layers_preserve_timeline_captions_overlays_and_templates() {
    let mut project = sample_project();
    project.timeline.tracks[3].items.push(visual_text_item(
        "caption-hook",
        TimelineItemKind::Caption,
        "Watch the reveal",
        0.5,
        1.4,
        "Hook caption",
    ));
    project.timeline.tracks[2].items.push(visual_text_item(
        "overlay-proof",
        TimelineItemKind::Overlay,
        "3x faster",
        2.0,
        2.5,
        "Proof overlay",
    ));
    project.timeline.tracks[2]
        .items
        .push(template_overlay_item());

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("timeline visuals should convert to graphics layers");

    assert_eq!(layers.len(), 3);
    assert_eq!(layers[0].id, "caption-hook");
    assert_eq!(layers[0].role, GraphicRole::Caption);
    assert_eq!(layers[0].timeline_start, 0.5);
    assert_eq!(layers[0].duration_seconds, 1.4);
    assert_eq!(layers[1].id, "overlay-proof");
    assert_eq!(layers[1].role, GraphicRole::Overlay);
    assert_eq!(layers[1].timeline_start, 2.0);
    assert_eq!(layers[1].duration_seconds, 2.5);
    assert_eq!(layers[2].id, "template-lower-third");
    assert_eq!(layers[2].role, GraphicRole::LowerThird);
    assert_eq!(layers[2].timeline_start, 4.0);
    assert_eq!(layers[2].duration_seconds, 2.0);
}

#[test]
fn project_graphics_layers_add_native_grain_and_vignette_for_visual_clips() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "effects".to_string(),
        json!([
            { "effectType": "stylize.grain", "enabled": true, "params": { "amount": 0.18 } },
            { "effectType": "stylize.vignette", "enabled": true, "params": { "amount": -0.25 } }
        ]),
    );

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("visual finishing effects should build graphics layers");
    let layer = layers
        .iter()
        .find(|layer| layer.id == "timeline-effects-item-1")
        .expect("effect graphics layer");

    assert_eq!(layer.role, GraphicRole::Overlay);
    assert_eq!(layer.timeline_start, 0.0);
    assert_eq!(layer.duration_seconds, 4.0);
    assert_eq!(
        layer.nodes.iter().map(GraphicNode::id).collect::<Vec<_>>(),
        vec!["stylize-grain", "stylize-vignette"]
    );
}

#[test]
fn project_graphics_layers_apply_caption_group_placement_to_the_rendered_layout() {
    let mut project = sample_project();
    let mut caption = visual_text_item(
        "caption-upper",
        TimelineItemKind::Caption,
        "Keep the lower frame clear",
        0.5,
        1.4,
        "Upper caption",
    );
    caption
        .properties
        .insert("captionPlacement".to_string(), json!("upper"));
    caption
        .properties
        .insert("stylePreset".to_string(), json!("kineticFocus"));
    project.timeline.tracks[3].items.push(caption);

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("caption placement should build");
    let layer = layers
        .iter()
        .find(|layer| layer.id == "caption-upper")
        .expect("upper caption graphics layer");
    let GraphicNode::RoundedRect(backplate) = &layer.nodes[0] else {
        panic!("caption starts with a backplate");
    };

    assert!(backplate.box_rect.y < 100.0);
    assert_eq!(
        backplate.fill,
        video_creater_lib::graphics::ir::Color::Hex("#1018208C".to_string())
    );
    let GraphicNode::Text(text) = &layer.nodes[2] else {
        panic!("caption ends with text");
    };
    assert_eq!(text.font_weight, 900);
}

#[test]
fn caption_word_animation_preview_matches_native_frame_sequence_contract() {
    let mut project = sample_project();
    let mut caption = visual_text_item(
        "caption-word-motion",
        TimelineItemKind::Caption,
        "Make it move",
        0.0,
        0.3,
        "Word motion",
    );
    caption
        .properties
        .insert("emphasizedWordIndices".to_string(), json!([1]));
    caption.properties.insert(
        "captionWordTimings".to_string(),
        json!([
            {"wordIndex":0,"startSeconds":0.0,"endSeconds":0.1},
            {"wordIndex":1,"startSeconds":0.1,"endSeconds":0.3},
            {"wordIndex":2,"startSeconds":0.2,"endSeconds":0.3}
        ]),
    );
    caption.properties.insert(
        "captionWordAnimations".to_string(),
        json!([{
            "wordIndex":1,"enterStartSeconds":0.1,"enterEndSeconds":0.15,
            "holdEndSeconds":0.24,"exitEndSeconds":0.3,"emphasisScale":1.2,
            "emphasisColor":"#ff3355","emphasisOpacity":0.9,"easing":"outBack"
        }]),
    );
    project.timeline.tracks[3].items.push(caption);
    let layer = build_project_graphics_layers(&project, 320, 180, 10.0)
        .expect("caption animation layer")
        .into_iter()
        .find(|layer| layer.id == "caption-word-motion")
        .expect("caption layer");
    let directory = tempfile::tempdir().expect("caption frames");
    let manifest = render_graphics_preview(
        &layer,
        &AssetRegistry::new(directory.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: directory.path().to_path_buf(),
        },
    )
    .expect("native caption frames");
    assert_eq!(manifest.frame_count, 3);
    assert_eq!(
        fs::read(directory.path().join("preview.png")).expect("preview"),
        fs::read(directory.path().join("frames/frame-000000.png")).expect("first frame")
    );
    assert_ne!(
        fs::read(directory.path().join("frames/frame-000001.png")).expect("enter frame"),
        fs::read(directory.path().join("frames/frame-000002.png")).expect("exit frame")
    );
}

#[test]
fn caption_word_animation_fails_closed_outside_transcript_word_bounds() {
    let mut project = sample_project();
    let mut caption = visual_text_item(
        "caption-invalid-motion",
        TimelineItemKind::Caption,
        "No drift",
        0.0,
        1.0,
        "Invalid motion",
    );
    caption
        .properties
        .insert("emphasizedWordIndices".to_string(), json!([1]));
    caption.properties.insert(
        "captionWordTimings".to_string(),
        json!([{"wordIndex":1,"startSeconds":0.4,"endSeconds":0.8}]),
    );
    caption.properties.insert("captionWordAnimations".to_string(), json!([{
        "wordIndex":1,"enterStartSeconds":0.2,"enterEndSeconds":0.5,"holdEndSeconds":0.7,
        "exitEndSeconds":0.9,"emphasisScale":1.2,"emphasisColor":"#ffffff","emphasisOpacity":1.0,"easing":"outQuad"
    }]));
    project.timeline.tracks[3].items.push(caption);
    let errors = build_project_graphics_layers(&project, 320, 180, 10.0)
        .expect_err("out-of-bounds word motion must fail");
    assert!(errors
        .iter()
        .any(|error| error.path.contains("captionWordAnimations")));
}

#[test]
fn project_graphics_layers_expand_nested_timeline_captions() {
    let mut project = sample_project();
    let mut alternate = project.timeline.clone();
    alternate.tracks[3].items.push(visual_text_item(
        "nested-caption",
        TimelineItemKind::Caption,
        "Inside the sequence",
        0.5,
        1.0,
        "Nested caption",
    ));
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate cut".to_string(),
        timeline: alternate,
    });
    let root_item = &mut project.timeline.tracks[0].items[0];
    root_item.start_seconds = 1.0;
    root_item.duration_seconds = 3.0;
    root_item.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    root_item.properties.clear();

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("nested timeline graphics should expand");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "root:0:item-1:nested-caption");
    assert_eq!(layers[0].timeline_start, 1.5);
    assert_eq!(layers[0].duration_seconds, 1.0);
}

#[test]
fn project_graphics_layers_reject_hyperframe_scene_without_visual_contract() {
    let mut project = sample_project();
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "hyperframe-hook".to_string(),
        kind: TimelineItemKind::HyperframeScene,
        start_seconds: 0.0,
        duration_seconds: 2.0,
        source: TimelineSource::Text {
            text: "Opening".to_string(),
        },
        label: "Opening HyperFrame".to_string(),
        properties: BTreeMap::from([
            ("templateId".to_string(), json!("chapter-card-v1")),
            (
                "templateFields".to_string(),
                json!({
                    "headline": "Opening",
                    "subline": "The setup"
                }),
            ),
        ]),
    });

    let errors = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect_err("HyperFrame scenes without visual metadata should be rejected");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "timeline.visuals.visualTreatment");
    assert_eq!(
        errors[0].message,
        "Timeline visual item is missing required render metadata."
    );
    assert_eq!(errors[0].details["itemId"], "hyperframe-hook");
}

#[test]
fn project_graphics_layers_render_template_hyperframe_scene() {
    let mut project = sample_project();
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "hyperframe-chapter".to_string(),
        kind: TimelineItemKind::HyperframeScene,
        start_seconds: 0.25,
        duration_seconds: 1.75,
        source: TimelineSource::Text {
            text: "Opening".to_string(),
        },
        label: "Opening HyperFrame".to_string(),
        properties: BTreeMap::from([
            ("templateId".to_string(), json!("chapter-card-v1")),
            (
                "templateFields".to_string(),
                json!({
                    "headline": "Opening",
                    "subline": "The setup"
                }),
            ),
            (
                "visualTreatment".to_string(),
                json!("full-frame chapter card with translucent panel"),
            ),
            (
                "motion".to_string(),
                json!("vertical line wipe, type-on, short hold, mask out"),
            ),
            (
                "safeZone".to_string(),
                json!("keep text inside 10% margins and center action visible"),
            ),
            (
                "avoid".to_string(),
                json!("static text-only cards and full-width opaque black slabs"),
            ),
        ]),
    });

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("template HyperFrame scenes should convert to graphics layers");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "hyperframe-chapter");
    assert_eq!(layers[0].role, GraphicRole::TitleCard);
    assert!(layers[0].alpha);
    assert_eq!(layers[0].timeline_start, 0.25);
    assert_eq!(layers[0].duration_seconds, 1.75);
    assert_eq!(
        layers[0].visual_treatment,
        "full-frame chapter card with translucent panel"
    );
    assert_eq!(
        layers[0].motion,
        "vertical line wipe, type-on, short hold, mask out"
    );
}

#[test]
fn project_graphics_layers_render_title_card_hyperframe_scene() {
    let mut project = sample_project();
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "hyperframe-title-card".to_string(),
        kind: TimelineItemKind::HyperframeScene,
        start_seconds: 0.25,
        duration_seconds: 1.75,
        source: TimelineSource::Text {
            text: "Opening".to_string(),
        },
        label: "Opening HyperFrame".to_string(),
        properties: BTreeMap::from([
            ("kind".to_string(), json!("title_card")),
            (
                "visualTreatment".to_string(),
                json!("full-frame chapter card with translucent motion layers"),
            ),
            (
                "motion".to_string(),
                json!("fast type-on with a short camera push"),
            ),
            (
                "safeZone".to_string(),
                json!("keep text inside 10% margins and center action visible"),
            ),
            (
                "avoid".to_string(),
                json!("static text-only cards and full-width opaque black slabs"),
            ),
        ]),
    });

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("title-card HyperFrame scenes should convert to graphics layers");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "hyperframe-title-card");
    assert_eq!(layers[0].role, GraphicRole::TitleCard);
    assert_eq!(layers[0].timeline_start, 0.25);
    assert_eq!(layers[0].duration_seconds, 1.75);
}

#[test]
fn project_graphics_layers_render_lower_third_hyperframe_scene() {
    let mut project = sample_project();
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "hyperframe-lower-third".to_string(),
        kind: TimelineItemKind::HyperframeScene,
        start_seconds: 0.75,
        duration_seconds: 2.0,
        source: TimelineSource::Text {
            text: "Olha API".to_string(),
        },
        label: "Speaker lower third".to_string(),
        properties: BTreeMap::from([
            ("kind".to_string(), json!("lower_third")),
            (
                "sourceBeat".to_string(),
                json!("Identify the speaker before the quote"),
            ),
            (
                "templateFields".to_string(),
                json!({
                    "headline": "Olha API",
                    "subline": "Creator and editor"
                }),
            ),
            (
                "visualTreatment".to_string(),
                json!("compact translucent lower third with cyan accent and strong hierarchy"),
            ),
            ("motion".to_string(), json!("slide in, hold, soft fade")),
            (
                "safeZone".to_string(),
                json!("keep essential text inside lower-third safe margins"),
            ),
            (
                "avoid".to_string(),
                json!("full-width opaque black slabs and static name cards"),
            ),
        ]),
    });

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("lower-third HyperFrame scenes should convert to graphics layers");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "hyperframe-lower-third");
    assert_eq!(layers[0].role, GraphicRole::LowerThird);
    assert_eq!(layers[0].timeline_start, 0.75);
    assert_eq!(layers[0].duration_seconds, 2.0);
    assert_eq!(
        layers[0].source_beat,
        "Identify the speaker before the quote"
    );
    assert!(!layers[0].nodes.is_empty());
}

#[test]
fn project_graphics_layers_render_diagram_hyperframe_scene() {
    let mut project = sample_project();
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "hyperframe-diagram".to_string(),
        kind: TimelineItemKind::HyperframeScene,
        start_seconds: 0.5,
        duration_seconds: 2.25,
        source: TimelineSource::Text {
            text: "Plan -> Cut -> Review".to_string(),
        },
        label: "Workflow diagram".to_string(),
        properties: BTreeMap::from([
            ("kind".to_string(), json!("diagram")),
            ("sourceBeat".to_string(), json!("Explain the edit workflow")),
            (
                "templateFields".to_string(),
                json!({
                    "headline": "Plan -> Cut -> Review",
                    "subline": "Graphics after a selected EDL"
                }),
            ),
            (
                "visualTreatment".to_string(),
                json!("transparent process diagram with concise labels"),
            ),
            (
                "motion".to_string(),
                json!("staggered node reveal with connector wipe"),
            ),
            (
                "safeZone".to_string(),
                json!("keep diagram labels inside center safe area"),
            ),
            (
                "avoid".to_string(),
                json!("static slide layout and paragraph text"),
            ),
        ]),
    });

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("diagram HyperFrame scenes should convert to graphics layers");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "hyperframe-diagram");
    assert_eq!(layers[0].role, GraphicRole::Diagram);
    assert_eq!(layers[0].timeline_start, 0.5);
    assert_eq!(layers[0].duration_seconds, 2.25);
    assert_eq!(layers[0].source_beat, "Explain the edit workflow");
    assert!(!layers[0].nodes.is_empty());
}

#[test]
fn project_graphics_layers_render_transition_hyperframe_scene() {
    let mut project = sample_project();
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "hyperframe-transition".to_string(),
        kind: TimelineItemKind::HyperframeScene,
        start_seconds: 2.0,
        duration_seconds: 1.0,
        source: TimelineSource::Text {
            text: "Next: The Payoff".to_string(),
        },
        label: "Payoff bridge".to_string(),
        properties: BTreeMap::from([
            ("kind".to_string(), json!("transition")),
            (
                "sourceBeat".to_string(),
                json!("Bridge the setup into the payoff"),
            ),
            (
                "templateFields".to_string(),
                json!({
                    "headline": "Next: The Payoff"
                }),
            ),
            (
                "visualTreatment".to_string(),
                json!("full-frame kinetic color wipe with readable cue text"),
            ),
            (
                "motion".to_string(),
                json!("fast panel wipe with text hold and clean exit"),
            ),
            (
                "safeZone".to_string(),
                json!("keep transition text inside center safe area"),
            ),
            (
                "avoid".to_string(),
                json!("long static interstitials and plain title slides"),
            ),
        ]),
    });

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("transition HyperFrame scenes should convert to graphics layers");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "hyperframe-transition");
    assert_eq!(layers[0].role, GraphicRole::Transition);
    assert_eq!(layers[0].timeline_start, 2.0);
    assert_eq!(layers[0].duration_seconds, 1.0);
    assert_eq!(layers[0].source_beat, "Bridge the setup into the payoff");
    assert!(!layers[0].nodes.is_empty());
}

#[test]
fn project_graphics_layers_render_immersive_hyperframe_scene() {
    let mut project = sample_project();
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "hyperframe-immersive".to_string(),
        kind: TimelineItemKind::HyperframeScene,
        start_seconds: 0.0,
        duration_seconds: 2.0,
        source: TimelineSource::Text {
            text: "Inside the cut".to_string(),
        },
        label: "Immersive opener".to_string(),
        properties: BTreeMap::from([
            ("kind".to_string(), json!("immersive_scene")),
            (
                "sourceBeat".to_string(),
                json!("Open with a dimensional generated scene"),
            ),
            (
                "templateFields".to_string(),
                json!({
                    "headline": "Inside the Cut"
                }),
            ),
            (
                "visualTreatment".to_string(),
                json!("full-frame immersive editorial scene with dimensional color panels"),
            ),
            (
                "motion".to_string(),
                json!("slow parallax drift with clean entrance and exit"),
            ),
            (
                "safeZone".to_string(),
                json!("keep cue text inside center safe area"),
            ),
            (
                "avoid".to_string(),
                json!("static slide design and opaque black slabs"),
            ),
        ]),
    });

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("immersive HyperFrame scenes should convert to graphics layers");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "hyperframe-immersive");
    assert_eq!(layers[0].role, GraphicRole::TitleCard);
    assert_eq!(layers[0].timeline_start, 0.0);
    assert_eq!(layers[0].duration_seconds, 2.0);
    assert_eq!(
        layers[0].source_beat,
        "Open with a dimensional generated scene"
    );
    assert!(!layers[0].nodes.is_empty());
}

#[test]
fn project_graphics_layers_render_logo_immersive_scene_with_holographic_nodes() {
    let mut project = sample_project();
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "hyperframe-logo-scene".to_string(),
        kind: TimelineItemKind::HyperframeScene,
        start_seconds: 0.0,
        duration_seconds: 2.0,
        source: TimelineSource::Text {
            text: "Brand reveal".to_string(),
        },
        label: "Brand reveal".to_string(),
        properties: BTreeMap::from([
            ("kind".to_string(), json!("immersive_scene")),
            (
                "sourceBeat".to_string(),
                json!("Reveal the product brand before the payoff"),
            ),
            (
                "visualTreatment".to_string(),
                json!("full-frame holographic logo cutout with pearlescent shader bands"),
            ),
            (
                "motion".to_string(),
                json!("logo shimmer drifts through the mask, holds, then fades cleanly"),
            ),
            (
                "safeZone".to_string(),
                json!("keep logo inside the central safe area"),
            ),
            (
                "avoid".to_string(),
                json!("plain centered text, static logo cards, and opaque black slabs"),
            ),
        ]),
    });

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("logo immersive HyperFrame scene should convert to graphics layer");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "hyperframe-logo-scene");
    assert_eq!(layers[0].role, GraphicRole::TitleCard);
    assert!(layers[0]
        .nodes
        .iter()
        .any(|node| node.id() == "logo-cutout"));
}

#[test]
fn project_graphics_layers_render_metric_immersive_scene_with_metric_nodes() {
    let mut project = sample_project();
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "hyperframe-metric-scene".to_string(),
        kind: TimelineItemKind::HyperframeScene,
        start_seconds: 0.0,
        duration_seconds: 2.0,
        source: TimelineSource::Text {
            text: "42% higher retention".to_string(),
        },
        label: "Retention metric".to_string(),
        properties: BTreeMap::from([
            ("kind".to_string(), json!("immersive_scene")),
            (
                "sourceBeat".to_string(),
                json!("Show the key data point before the product proof"),
            ),
            (
                "templateFields".to_string(),
                json!({
                    "headline": "42%",
                    "subline": "higher retention"
                }),
            ),
            (
                "visualTreatment".to_string(),
                json!("floating metric tile with high-contrast number and directional accent"),
            ),
            (
                "motion".to_string(),
                json!("metric count-up feel, accent sweep, hold, then slide out"),
            ),
            (
                "safeZone".to_string(),
                json!("keep the metric tile inside the upper-right safe area"),
            ),
            (
                "avoid".to_string(),
                json!("spreadsheet boxes, dense labels, and static title cards"),
            ),
        ]),
    });

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("metric immersive HyperFrame scene should convert to graphics layer");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "hyperframe-metric-scene");
    assert_eq!(layers[0].role, GraphicRole::Overlay);
    assert!(layers[0].nodes.iter().any(|node| node.id() == "backing"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "accent"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "subline"));
}

#[test]
fn project_graphics_layers_render_detail_immersive_scene_with_tracking_nodes() {
    let mut project = sample_project();
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "hyperframe-detail-scene".to_string(),
        kind: TimelineItemKind::HyperframeScene,
        start_seconds: 0.0,
        duration_seconds: 2.0,
        source: TimelineSource::Text {
            text: "Look here".to_string(),
        },
        label: "Product detail callout".to_string(),
        properties: BTreeMap::from([
            ("kind".to_string(), json!("immersive_scene")),
            (
                "sourceBeat".to_string(),
                json!("Direct attention to the visual detail that proves the claim"),
            ),
            (
                "templateFields".to_string(),
                json!({
                    "headline": "Look here",
                    "subline": "key detail"
                }),
            ),
            (
                "visualTreatment".to_string(),
                json!("thin tracking ring with compact label and pointer line"),
            ),
            (
                "motion".to_string(),
                json!("tracking highlight draws on, label slides from pointer, then fades"),
            ),
            (
                "safeZone".to_string(),
                json!("keep the label inside the safe area while the pointer tracks the detail"),
            ),
            (
                "avoid".to_string(),
                json!("large opaque callout boxes, covering hands, and static arrows"),
            ),
        ]),
    });

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("detail immersive HyperFrame scene should convert to graphics layer");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "hyperframe-detail-scene");
    assert_eq!(layers[0].role, GraphicRole::Overlay);
    assert!(layers[0]
        .nodes
        .iter()
        .any(|node| node.id() == "highlight-frame"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "accent-top"));
    assert!(layers[0]
        .nodes
        .iter()
        .any(|node| node.id() == "label-backing"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "headline"));
}

#[test]
fn project_graphics_layers_render_quote_immersive_scene_with_punchy_caption_nodes() {
    let mut project = sample_project();
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "hyperframe-quote-scene".to_string(),
        kind: TimelineItemKind::HyperframeScene,
        start_seconds: 0.0,
        duration_seconds: 2.0,
        source: TimelineSource::Text {
            text: "This changes everything".to_string(),
        },
        label: "Key quote".to_string(),
        properties: BTreeMap::from([
            ("kind".to_string(), json!("immersive_scene")),
            (
                "sourceBeat".to_string(),
                json!("Emphasize the key quote before the payoff cut"),
            ),
            (
                "templateFields".to_string(),
                json!({
                    "headline": "THIS CHANGES EVERYTHING"
                }),
            ),
            (
                "visualTreatment".to_string(),
                json!("large phone-readable quote caption lockup with accent underline and soft backing"),
            ),
            (
                "motion".to_string(),
                json!("snap pop in, underline wipe, hold, then quick fade"),
            ),
            (
                "safeZone".to_string(),
                json!("keep the quote inside 10% margins and above bottom controls"),
            ),
            (
                "avoid".to_string(),
                json!("subtitle slabs, tiny type, and centered static paragraphs"),
            ),
        ]),
    });

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("quote immersive HyperFrame scene should convert to graphics layer");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "hyperframe-quote-scene");
    assert_eq!(layers[0].role, GraphicRole::Overlay);
    assert!(layers[0].nodes.iter().any(|node| node.id() == "backing"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "headline"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "accent"));
}

#[test]
fn project_graphics_layers_render_chapter_immersive_scene_with_chapter_card_nodes() {
    let mut project = sample_project();
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "hyperframe-chapter-scene".to_string(),
        kind: TimelineItemKind::HyperframeScene,
        start_seconds: 0.0,
        duration_seconds: 2.0,
        source: TimelineSource::Text {
            text: "Chapter 02".to_string(),
        },
        label: "Story chapter reset".to_string(),
        properties: BTreeMap::from([
            ("kind".to_string(), json!("immersive_scene")),
            (
                "sourceBeat".to_string(),
                json!("Introduce the next chapter of the story before the payoff"),
            ),
            (
                "templateFields".to_string(),
                json!({
                    "headline": "CHAPTER 02",
                    "subline": "The proof"
                }),
            ),
            (
                "visualTreatment".to_string(),
                json!(
                    "left-weighted chapter marker with translucent panel and vertical reveal line"
                ),
            ),
            (
                "motion".to_string(),
                json!("vertical line wipe, text type-on, short hold, then mask out"),
            ),
            (
                "safeZone".to_string(),
                json!("keep chapter text inside 10% margins and leave center action visible"),
            ),
            (
                "avoid".to_string(),
                json!("full-frame static slides, plain centered text, and long title holds"),
            ),
        ]),
    });

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("chapter immersive HyperFrame scene should convert to graphics layer");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "hyperframe-chapter-scene");
    assert_eq!(layers[0].role, GraphicRole::TitleCard);
    assert!(layers[0].nodes.iter().any(|node| node.id() == "backing"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "accent-top"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "headline"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "subline"));
}

#[test]
fn project_graphics_layers_render_comparison_immersive_scene_with_metric_nodes() {
    let mut project = sample_project();
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "hyperframe-comparison-scene".to_string(),
        kind: TimelineItemKind::HyperframeScene,
        start_seconds: 0.0,
        duration_seconds: 2.0,
        source: TimelineSource::Text {
            text: "Before vs After".to_string(),
        },
        label: "Before vs after comparison".to_string(),
        properties: BTreeMap::from([
            ("kind".to_string(), json!("immersive_scene")),
            (
                "sourceBeat".to_string(),
                json!("Compare the old workflow against the new outcome before the proof cut"),
            ),
            (
                "templateFields".to_string(),
                json!({
                    "headline": "Before vs After",
                    "subline": "manual handoff -> automated review"
                }),
            ),
            (
                "visualTreatment".to_string(),
                json!("split comparison tile with two concise labels and a directional accent"),
            ),
            (
                "motion".to_string(),
                json!(
                    "left label enters, right label counters, accent sweep bridges the two states"
                ),
            ),
            (
                "safeZone".to_string(),
                json!("keep comparison labels inside 10% margins and away from captions"),
            ),
            (
                "avoid".to_string(),
                json!("generic gradient loop, full-screen static table, and dense paragraphs"),
            ),
        ]),
    });

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("comparison immersive HyperFrame scene should convert to graphics layer");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "hyperframe-comparison-scene");
    assert_eq!(layers[0].role, GraphicRole::Overlay);
    assert!(layers[0].nodes.iter().any(|node| node.id() == "backing"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "headline"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "subline"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "accent"));
}

#[test]
fn project_graphics_layers_render_process_immersive_scene_with_chapter_card_nodes() {
    let mut project = sample_project();
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "hyperframe-process-scene".to_string(),
        kind: TimelineItemKind::HyperframeScene,
        start_seconds: 0.0,
        duration_seconds: 2.0,
        source: TimelineSource::Text {
            text: "Plan -> Build -> Review".to_string(),
        },
        label: "Three-step rollout map".to_string(),
        properties: BTreeMap::from([
            ("kind".to_string(), json!("immersive_scene")),
            (
                "sourceBeat".to_string(),
                json!("Explain the rollout process before the final proof cut"),
            ),
            (
                "templateFields".to_string(),
                json!({
                    "headline": "Plan -> Build -> Review",
                    "subline": "three-stage process"
                }),
            ),
            (
                "visualTreatment".to_string(),
                json!("staged roadmap with three milestone markers and a vertical reveal line"),
            ),
            (
                "motion".to_string(),
                json!("step markers reveal in sequence, connector line wipes through the process"),
            ),
            (
                "safeZone".to_string(),
                json!("keep process labels inside 10% margins and away from captions"),
            ),
            (
                "avoid".to_string(),
                json!("generic gradient loop, dense flowchart boxes, and static slide design"),
            ),
        ]),
    });

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("process immersive HyperFrame scene should convert to graphics layer");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "hyperframe-process-scene");
    assert_eq!(layers[0].role, GraphicRole::TitleCard);
    assert!(layers[0].nodes.iter().any(|node| node.id() == "backing"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "accent-top"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "headline"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "subline"));
}

#[test]
fn project_graphics_layers_render_testimonial_feature_and_launch_scenes_with_specific_nodes() {
    let mut project = sample_project();
    project.timeline.tracks[1].items.extend([
        TimelineItem {
            id: "hyperframe-testimonial-scene".to_string(),
            kind: TimelineItemKind::HyperframeScene,
            start_seconds: 0.0,
            duration_seconds: 2.0,
            source: TimelineSource::Text {
                text: "It finally clicked".to_string(),
            },
            label: "Customer testimonial interview".to_string(),
            properties: BTreeMap::from([
                ("kind".to_string(), json!("immersive_scene")),
                (
                    "sourceBeat".to_string(),
                    json!("Lift the customer testimonial into a phone-readable interview beat"),
                ),
                (
                    "templateFields".to_string(),
                    json!({
                        "headline": "IT FINALLY CLICKED"
                    }),
                ),
                (
                    "visualTreatment".to_string(),
                    json!("testimonial lockup with speaker treatment and compact backing"),
                ),
                (
                    "motion".to_string(),
                    json!("testimonial text snaps in, speaker line wipes, then fades"),
                ),
                (
                    "safeZone".to_string(),
                    json!("keep testimonial text inside 10% margins"),
                ),
                (
                    "avoid".to_string(),
                    json!("plain subtitle slabs and dense paragraphs"),
                ),
            ]),
        },
        TimelineItem {
            id: "hyperframe-feature-scene".to_string(),
            kind: TimelineItemKind::HyperframeScene,
            start_seconds: 2.1,
            duration_seconds: 2.0,
            source: TimelineSource::Text {
                text: "Auto sync".to_string(),
            },
            label: "Product feature spec closeup".to_string(),
            properties: BTreeMap::from([
                ("kind".to_string(), json!("immersive_scene")),
                (
                    "sourceBeat".to_string(),
                    json!("Show the product feature and spec before the proof cut"),
                ),
                (
                    "templateFields".to_string(),
                    json!({
                        "headline": "Auto sync",
                        "subline": "frame-accurate"
                    }),
                ),
                (
                    "visualTreatment".to_string(),
                    json!("precise feature lens with leader line and compact spec label"),
                ),
                (
                    "motion".to_string(),
                    json!("feature lens draws in, spec label slides from the edge, then clears"),
                ),
                (
                    "safeZone".to_string(),
                    json!("keep spec label away from captions and faces"),
                ),
                (
                    "avoid".to_string(),
                    json!("large opaque boxes and generic gradients"),
                ),
            ]),
        },
        TimelineItem {
            id: "hyperframe-launch-scene".to_string(),
            kind: TimelineItemKind::HyperframeScene,
            start_seconds: 4.2,
            duration_seconds: 2.0,
            source: TimelineSource::Text {
                text: "Launch ready".to_string(),
            },
            label: "Launch announcement beat".to_string(),
            properties: BTreeMap::from([
                ("kind".to_string(), json!("immersive_scene")),
                (
                    "sourceBeat".to_string(),
                    json!("Introduce the launch announcement before the reveal"),
                ),
                (
                    "templateFields".to_string(),
                    json!({
                        "headline": "LAUNCH READY",
                        "subline": "new workflow"
                    }),
                ),
                (
                    "visualTreatment".to_string(),
                    json!("editorial announcement marker with vertical reveal and subline"),
                ),
                (
                    "motion".to_string(),
                    json!(
                        "announcement line wipes up, headline reveals, short hold, then masks out"
                    ),
                ),
                (
                    "safeZone".to_string(),
                    json!("keep announcement copy inside 10% margins"),
                ),
                (
                    "avoid".to_string(),
                    json!("static title slide and centered plain text"),
                ),
            ]),
        },
        TimelineItem {
            id: "hyperframe-route-scene".to_string(),
            kind: TimelineItemKind::HyperframeScene,
            start_seconds: 6.3,
            duration_seconds: 2.0,
            source: TimelineSource::Text {
                text: "Downtown route".to_string(),
            },
            label: "Route map transition".to_string(),
            properties: BTreeMap::from([
                ("kind".to_string(), json!("immersive_scene")),
                (
                    "sourceBeat".to_string(),
                    json!("Show the location route before the arrival reveal"),
                ),
                (
                    "templateFields".to_string(),
                    json!({
                        "headline": "Downtown route",
                        "subline": "12 min"
                    }),
                ),
                (
                    "visualTreatment".to_string(),
                    json!("animated city map with a place marker and route line"),
                ),
                (
                    "motion".to_string(),
                    json!("route line draws across the map, place marker pulses, text slides in"),
                ),
                (
                    "safeZone".to_string(),
                    json!("keep map labels inside 10% margins"),
                ),
                (
                    "avoid".to_string(),
                    json!("dense map screenshots and static location cards"),
                ),
            ]),
        },
        TimelineItem {
            id: "hyperframe-reaction-scene".to_string(),
            kind: TimelineItemKind::HyperframeScene,
            start_seconds: 8.4,
            duration_seconds: 2.0,
            source: TimelineSource::Text {
                text: "Wait, what?".to_string(),
            },
            label: "Surprised reaction beat".to_string(),
            properties: BTreeMap::from([
                ("kind".to_string(), json!("immersive_scene")),
                (
                    "sourceBeat".to_string(),
                    json!("Show the emotional reaction before the next cut"),
                ),
                (
                    "templateFields".to_string(),
                    json!({
                        "headline": "WAIT, WHAT?"
                    }),
                ),
                (
                    "visualTreatment".to_string(),
                    json!("large reaction typography with expressive accent stroke"),
                ),
                (
                    "motion".to_string(),
                    json!("reaction word pops on, accent stroke snaps, then clears quickly"),
                ),
                (
                    "safeZone".to_string(),
                    json!("keep reaction text inside 10% margins"),
                ),
                (
                    "avoid".to_string(),
                    json!("plain subtitle slabs and long holds"),
                ),
            ]),
        },
        TimelineItem {
            id: "hyperframe-pricing-scene".to_string(),
            kind: TimelineItemKind::HyperframeScene,
            start_seconds: 10.5,
            duration_seconds: 2.0,
            source: TimelineSource::Text {
                text: "Pricing savings".to_string(),
            },
            label: "Buyer proof beat".to_string(),
            properties: BTreeMap::from([
                ("kind".to_string(), json!("immersive_scene")),
                (
                    "sourceBeat".to_string(),
                    json!("Show the buyer proof before the close"),
                ),
                (
                    "templateFields".to_string(),
                    json!({
                        "headline": "Pricing savings",
                        "subline": "Budget impact and ROI"
                    }),
                ),
                (
                    "visualTreatment".to_string(),
                    json!("compact proof tile with directional accent"),
                ),
                (
                    "motion".to_string(),
                    json!("figure counts up, delta sweeps, then settles"),
                ),
                (
                    "safeZone".to_string(),
                    json!("keep pricing proof inside 10% margins"),
                ),
                (
                    "avoid".to_string(),
                    json!("spreadsheet screenshots and static pricing cards"),
                ),
            ]),
        },
        TimelineItem {
            id: "hyperframe-risk-scene".to_string(),
            kind: TimelineItemKind::HyperframeScene,
            start_seconds: 12.6,
            duration_seconds: 2.0,
            source: TimelineSource::Text {
                text: "Risk flag".to_string(),
            },
            label: "Compliance risk warning".to_string(),
            properties: BTreeMap::from([
                ("kind".to_string(), json!("immersive_scene")),
                (
                    "sourceBeat".to_string(),
                    json!("Flag the security risk before the remediation step"),
                ),
                (
                    "templateFields".to_string(),
                    json!({
                        "headline": "RISK FLAG"
                    }),
                ),
                (
                    "visualTreatment".to_string(),
                    json!("urgent warning caption with compact compliance marker"),
                ),
                (
                    "motion".to_string(),
                    json!("alert word snaps on, warning rule wipes, then clears"),
                ),
                (
                    "safeZone".to_string(),
                    json!("keep warning text inside 10% margins"),
                ),
                (
                    "avoid".to_string(),
                    json!("generic process cards and static warning slides"),
                ),
            ]),
        },
        TimelineItem {
            id: "hyperframe-deadline-scene".to_string(),
            kind: TimelineItemKind::HyperframeScene,
            start_seconds: 14.7,
            duration_seconds: 2.0,
            source: TimelineSource::Text {
                text: "Friday".to_string(),
            },
            label: "Deadline schedule beat".to_string(),
            properties: BTreeMap::from([
                ("kind".to_string(), json!("immersive_scene")),
                (
                    "sourceBeat".to_string(),
                    json!("Show the calendar deadline and due date"),
                ),
                (
                    "templateFields".to_string(),
                    json!({
                        "headline": "Friday",
                        "subline": "submission deadline"
                    }),
                ),
                (
                    "visualTreatment".to_string(),
                    json!("editorial schedule marker with date lockup and reveal line"),
                ),
                (
                    "motion".to_string(),
                    json!("date marker wipes in, deadline label reveals, then masks out"),
                ),
                (
                    "safeZone".to_string(),
                    json!("keep date and deadline copy inside 10% margins"),
                ),
                (
                    "avoid".to_string(),
                    json!("generic gradient loop and static calendar screenshots"),
                ),
            ]),
        },
    ]);

    let layers = build_project_graphics_layers(&project, 1280, 720, 30.0)
        .expect("immersive HyperFrame scenes should convert to graphics layers");

    assert_eq!(layers.len(), 8);
    assert_eq!(layers[0].id, "hyperframe-testimonial-scene");
    assert_eq!(layers[0].role, GraphicRole::Overlay);
    assert!(layers[0].nodes.iter().any(|node| node.id() == "backing"));
    assert!(layers[0].nodes.iter().any(|node| node.id() == "accent"));
    assert_eq!(layers[1].id, "hyperframe-feature-scene");
    assert_eq!(layers[1].role, GraphicRole::Overlay);
    assert!(layers[1]
        .nodes
        .iter()
        .any(|node| node.id() == "highlight-frame"));
    assert!(layers[1]
        .nodes
        .iter()
        .any(|node| node.id() == "label-backing"));
    assert_eq!(layers[2].id, "hyperframe-launch-scene");
    assert_eq!(layers[2].role, GraphicRole::TitleCard);
    assert!(layers[2].nodes.iter().any(|node| node.id() == "backing"));
    assert!(layers[2].nodes.iter().any(|node| node.id() == "subline"));
    assert_eq!(layers[3].id, "hyperframe-route-scene");
    assert_eq!(layers[3].role, GraphicRole::Overlay);
    assert!(layers[3]
        .nodes
        .iter()
        .any(|node| node.id() == "highlight-frame"));
    assert!(layers[3]
        .nodes
        .iter()
        .any(|node| node.id() == "label-backing"));
    assert_eq!(layers[4].id, "hyperframe-reaction-scene");
    assert_eq!(layers[4].role, GraphicRole::Overlay);
    assert!(layers[4].nodes.iter().any(|node| node.id() == "backing"));
    assert!(layers[4].nodes.iter().any(|node| node.id() == "accent"));
    assert_eq!(layers[5].id, "hyperframe-pricing-scene");
    assert_eq!(layers[5].role, GraphicRole::Overlay);
    assert!(layers[5].nodes.iter().any(|node| node.id() == "backing"));
    assert!(layers[5].nodes.iter().any(|node| node.id() == "accent"));
    assert!(layers[5].nodes.iter().any(|node| node.id() == "subline"));
    assert_eq!(layers[6].id, "hyperframe-risk-scene");
    assert_eq!(layers[6].role, GraphicRole::Overlay);
    assert!(layers[6].nodes.iter().any(|node| node.id() == "backing"));
    assert!(layers[6].nodes.iter().any(|node| node.id() == "accent"));
    assert_eq!(layers[7].id, "hyperframe-deadline-scene");
    assert_eq!(layers[7].role, GraphicRole::TitleCard);
    assert!(layers[7].nodes.iter().any(|node| node.id() == "backing"));
    assert!(layers[7].nodes.iter().any(|node| node.id() == "subline"));
}

#[test]
fn project_render_report_conversion_records_validation_contract() {
    let paths = ProjectWebmRenderPaths::new("render-final-1");
    let report = RenderReport {
        job_id: "render-final-1".to_string(),
        summary: RenderReportSummary {
            status: "succeeded".to_string(),
            duration_seconds: Some(2.5),
            output_path: Some("renders/render-final-1/output.webm".to_string()),
            quality: Some(RenderQuality::Final),
            requested_width: Some(1920),
            requested_height: Some(1080),
            actual_width: Some(1920),
            actual_height: Some(1080),
            container: Some("webm".to_string()),
            video_codec: Some("vp9".to_string()),
            audio_codec: Some("opus".to_string()),
        },
        command: CommandSpec::new("gstreamer-ges").arg("--quality=finalWebm"),
        stdout: String::new(),
        stderr: String::new(),
        streams: Some(PipelineRenderReportStreams {
            video: true,
            audio: true,
        }),
        errors: Vec::new(),
        artifacts: vec![
            "renders/render-final-1/output.webm".to_string(),
            "renders/render-final-1/pipeline-report.json".to_string(),
            "renders/render-final-1/pipeline-report.md".to_string(),
            "renders/render-final-1/render.log".to_string(),
        ],
        graphics: Vec::new(),
        performance: None,
        preview_comparison_request: None,
        preview_comparison: None,
    };

    let project_report = project_render_report_from_pipeline_report(
        &report,
        &paths,
        "2026-06-27T10:00:00Z",
        &ProjectRenderReportEvidence {
            video_stream: true,
            audio_stream: true,
            audio_required: true,
            expected_duration_seconds: Some(2.5),
            duration_tolerance_seconds: Some(0.05),
        },
    )
    .expect("pipeline report should convert to project report");

    assert_eq!(project_report.id, "render-final-1");
    assert_eq!(project_report.status, RenderReportStatus::Completed);
    assert_eq!(
        project_report.output_path,
        "renders/render-final-1/output.webm"
    );
    assert_eq!(project_report.duration_seconds, 2.5);
    assert_eq!(project_report.quality, Some(RenderQuality::Final));
    assert_eq!(project_report.requested_width, Some(1920));
    assert_eq!(project_report.requested_height, Some(1080));
    assert_eq!(project_report.actual_width, Some(1920));
    assert_eq!(project_report.actual_height, Some(1080));
    assert!(project_report.streams.video);
    assert!(project_report.streams.audio);
    assert_eq!(
        project_report.checks,
        BTreeMap::from([
            ("duration".to_string(), RenderReportCheckStatus::Passed),
            (
                "captionAlignment".to_string(),
                RenderReportCheckStatus::Skipped
            ),
            (
                "overlayTiming".to_string(),
                RenderReportCheckStatus::Skipped
            ),
            (
                "visualFrameEvidence".to_string(),
                RenderReportCheckStatus::Skipped
            ),
            ("artifactPaths".to_string(), RenderReportCheckStatus::Passed),
            ("streams".to_string(), RenderReportCheckStatus::Passed),
            ("logPath".to_string(), RenderReportCheckStatus::Passed),
        ])
    );
    assert_eq!(project_report.log_path, "renders/render-final-1/render.log");
    assert_eq!(project_report.created_at, "2026-06-27T10:00:00Z");
}

#[test]
fn project_render_report_conversion_preserves_preview_comparison_evidence() {
    let paths = ProjectWebmRenderPaths::new("render-final-1");
    let mut report = successful_render_report();
    report.preview_comparison = Some(RenderPreviewComparison {
        status: "failed".to_string(),
        compared_frames: vec![RenderPreviewComparisonFrame {
            timeline_seconds: 1.25,
            preview_frame: "visual-qa/preview-001.png".to_string(),
            rendered_frame: "renders/render-final-1/frames/frame-000030.png".to_string(),
            diff_frame: Some("visual-qa/diff-001.png".to_string()),
            mismatch_ratio: 0.08,
            passed: false,
        }],
    });

    let project_report = project_render_report_from_pipeline_report(
        &report,
        &paths,
        "2026-06-27T10:00:00Z",
        &successful_render_evidence(),
    )
    .expect("pipeline report should convert with preview comparison evidence");

    let comparison = project_report
        .preview_comparison
        .expect("preview comparison should persist to project report");
    assert_eq!(comparison.status, "failed");
    assert_eq!(comparison.compared_frames[0].timeline_seconds, 1.25);
    assert_eq!(
        comparison.compared_frames[0].preview_frame,
        "visual-qa/preview-001.png"
    );
    assert!(project_report
        .artifacts
        .iter()
        .any(|artifact| artifact == "visual-qa/preview-001.png"));
    assert!(project_report
        .artifacts
        .iter()
        .any(|artifact| artifact == "renders/render-final-1/frames/frame-000030.png"));
    assert!(project_report
        .artifacts
        .iter()
        .any(|artifact| artifact == "visual-qa/diff-001.png"));
}

#[test]
fn project_render_report_conversion_preserves_pending_preview_comparison_request() {
    let paths = ProjectWebmRenderPaths::new("render-final-1");
    let mut report = successful_render_report();
    report.preview_comparison_request = Some(RenderPreviewComparisonRequest {
        status: "pending".to_string(),
        project_dir: "/tmp/project".to_string(),
        project_report_id: "render-final-1".to_string(),
        render_report_path: "renders/render-final-1/pipeline-report.json".to_string(),
        rendered_video: "renders/render-final-1/output.webm".to_string(),
        duration_seconds: 4.0,
        frame_time_seconds: 1.25,
        rendered_frames: vec!["renders/render-final-1/frames/frame-000001.png".to_string()],
        fail_on_mismatch: true,
    });

    let project_report = project_render_report_from_pipeline_report(
        &report,
        &paths,
        "2026-06-27T10:00:00Z",
        &successful_render_evidence(),
    )
    .expect("pipeline report should convert with pending preview comparison request");

    let request = project_report
        .preview_comparison_request
        .expect("pending preview comparison request should persist to project report");
    assert_eq!(request.status, "pending");
    assert_eq!(request.project_report_id, "render-final-1");
    assert_eq!(request.duration_seconds, 4.0);
    assert_eq!(
        request.rendered_frames,
        vec!["renders/render-final-1/frames/frame-000001.png".to_string()]
    );
    assert_eq!(
        request.render_report_path,
        "renders/render-final-1/pipeline-report.json"
    );
}

#[test]
fn project_render_report_marks_missing_artifacts_failed_from_evidence() {
    let paths = ProjectWebmRenderPaths::new("render-final-1");
    let mut report = successful_render_report();
    report.artifacts = vec![
        "renders/render-final-1/output.webm".to_string(),
        "renders/render-final-1/pipeline-report.json".to_string(),
    ];

    let project_report = project_render_report_from_pipeline_report(
        &report,
        &paths,
        "2026-06-27T10:00:00Z",
        &successful_render_evidence(),
    )
    .expect("pipeline report should convert even when artifact evidence fails");

    assert_eq!(project_report.status, RenderReportStatus::Failed);
    assert_eq!(
        project_report.checks.get("artifactPaths"),
        Some(&RenderReportCheckStatus::Failed)
    );
    assert_eq!(
        project_report.checks.get("logPath"),
        Some(&RenderReportCheckStatus::Failed)
    );
}

#[test]
fn project_render_report_marks_visual_checks_passed_when_graphics_are_recorded() {
    let paths = ProjectWebmRenderPaths::new("render-final-1");
    let mut report = successful_render_report();
    report.artifacts.push("preview.png".to_string());
    report.graphics.push(RenderGraphicsReport {
        layer_id: "caption-hook".to_string(),
        renderer: "rust".to_string(),
        quality_profile: None,
        template_id: None,
        motion_preset_id: Some("snap-pop-v1".to_string()),
        visual_qa_status: Some("not-run".to_string()),
        cache_status: None,
        sampled_frames: vec!["preview.png".to_string()],
        qa_metrics: BTreeMap::new(),
    });

    let project_report = project_render_report_from_pipeline_report(
        &report,
        &paths,
        "2026-06-27T10:00:00Z",
        &successful_render_evidence(),
    )
    .expect("pipeline report should convert with visual evidence");

    assert_eq!(project_report.status, RenderReportStatus::Completed);
    assert_eq!(
        project_report.checks.get("captionAlignment"),
        Some(&RenderReportCheckStatus::Passed)
    );
    assert_eq!(
        project_report.checks.get("overlayTiming"),
        Some(&RenderReportCheckStatus::Passed)
    );
    assert_eq!(
        project_report.checks.get("visualFrameEvidence"),
        Some(&RenderReportCheckStatus::Passed)
    );
}

#[test]
fn project_render_report_marks_duration_mismatch_failed_from_evidence() {
    let paths = ProjectWebmRenderPaths::new("render-final-1");
    let report = successful_render_report();

    let project_report = project_render_report_from_pipeline_report(
        &report,
        &paths,
        "2026-06-27T10:00:00Z",
        &ProjectRenderReportEvidence {
            expected_duration_seconds: Some(4.0),
            ..successful_render_evidence()
        },
    )
    .expect("pipeline report should convert even when duration evidence fails");

    assert_eq!(project_report.status, RenderReportStatus::Failed);
    assert_eq!(
        project_report.checks.get("duration"),
        Some(&RenderReportCheckStatus::Failed)
    );
}

#[test]
fn project_render_report_marks_missing_required_audio_failed_from_evidence() {
    let paths = ProjectWebmRenderPaths::new("render-final-1");
    let report = successful_render_report();

    let project_report = project_render_report_from_pipeline_report(
        &report,
        &paths,
        "2026-06-27T10:00:00Z",
        &ProjectRenderReportEvidence {
            audio_stream: false,
            audio_required: true,
            ..successful_render_evidence()
        },
    )
    .expect("pipeline report should convert even when stream evidence fails");

    assert_eq!(project_report.status, RenderReportStatus::Failed);
    assert!(project_report.streams.video);
    assert!(!project_report.streams.audio);
    assert_eq!(
        project_report.checks.get("streams"),
        Some(&RenderReportCheckStatus::Failed)
    );
}

fn successful_render_report() -> RenderReport {
    RenderReport {
        job_id: "render-final-1".to_string(),
        summary: RenderReportSummary {
            status: "succeeded".to_string(),
            duration_seconds: Some(2.5),
            output_path: Some("renders/render-final-1/output.webm".to_string()),
            ..RenderReportSummary::default()
        },
        command: CommandSpec::new("gstreamer-ges").arg("--quality=finalWebm"),
        stdout: String::new(),
        stderr: String::new(),
        streams: None,
        errors: Vec::new(),
        artifacts: vec![
            "renders/render-final-1/output.webm".to_string(),
            "renders/render-final-1/pipeline-report.json".to_string(),
            "renders/render-final-1/pipeline-report.md".to_string(),
            "renders/render-final-1/render.log".to_string(),
        ],
        graphics: Vec::new(),
        performance: None,
        preview_comparison_request: None,
        preview_comparison: None,
    }
}

fn successful_render_evidence() -> ProjectRenderReportEvidence {
    ProjectRenderReportEvidence {
        video_stream: true,
        audio_stream: true,
        audio_required: true,
        expected_duration_seconds: Some(2.5),
        duration_tolerance_seconds: Some(0.05),
    }
}

fn visual_text_item(
    id: &str,
    kind: TimelineItemKind,
    text: &str,
    start_seconds: f64,
    duration_seconds: f64,
    label: &str,
) -> TimelineItem {
    let mut properties = BTreeMap::new();
    properties.insert(
        "visualTreatment".to_string(),
        json!("transparent editorial overlay with strong hierarchy"),
    );
    properties.insert(
        "motion".to_string(),
        json!("quick scale in, hold, and soft fade out"),
    );
    properties.insert(
        "safeZone".to_string(),
        json!("keep text inside 10% margins"),
    );
    properties.insert("avoid".to_string(), json!("full-width opaque black slabs"));

    TimelineItem {
        id: id.to_string(),
        kind,
        start_seconds,
        duration_seconds,
        source: TimelineSource::Text {
            text: text.to_string(),
        },
        label: label.to_string(),
        properties,
    }
}

fn template_overlay_item() -> TimelineItem {
    let mut properties = BTreeMap::new();
    properties.insert("templateId".to_string(), json!("kinetic-lower-third-v1"));
    properties.insert(
        "templateFields".to_string(),
        json!({
            "headline": "Olha API",
            "subline": "Founder",
        }),
    );
    properties.insert(
        "visualTreatment".to_string(),
        json!("compact lower-third block with translucent backing"),
    );
    properties.insert(
        "motion".to_string(),
        json!("slide-and-fade in over 8 frames"),
    );
    properties.insert(
        "safeZone".to_string(),
        json!("keep essential text inside 10% margins"),
    );
    properties.insert("avoid".to_string(), json!("default-font template look"));
    properties.insert("previewVariant".to_string(), json!("lower-third"));

    TimelineItem {
        id: "template-lower-third".to_string(),
        kind: TimelineItemKind::Overlay,
        start_seconds: 4.0,
        duration_seconds: 2.0,
        source: TimelineSource::Generated {
            artifact_id: "template:kinetic-lower-third-v1:template-lower-third".to_string(),
        },
        label: "Kinetic lower third".to_string(),
        properties,
    }
}

fn export_job_summary(job_id: &str, updated_at: &str) -> JobSummary {
    video_creater_lib::workflows::temporal_job_summary(
        video_creater_lib::workflows::TemporalWorkflowKind::ExportMedia,
        "project-export-fixture",
        job_id,
        JobStatus::Queued,
        updated_at,
    )
}

fn platform_delivery_program() -> &'static str {
    if cfg!(target_os = "macos") {
        "avfoundation-native"
    } else {
        "gstreamer-ges"
    }
}

#[test]
fn workflow_dispatched_export_renders_its_already_recorded_job() {
    use video_creater_lib::project::action::ProjectAction;
    use video_creater_lib::workflows::{
        temporal_export_media_start_request_with_options, TemporalWorkflowKind,
    };

    start_render_process_runtime().expect("initialize curated render runtime");
    let Some(availability) = mp4_export_profile_availability_report()
        .into_iter()
        .find(|candidate| candidate.profile == ExportProfile::Webm)
    else {
        panic!("WebM profile should be reported");
    };
    if !availability.available {
        assert!(availability.unavailable_reason.is_some());
        return;
    }
    let dir = tempfile::tempdir().expect("temp project dir");
    fs::create_dir_all(dir.path().join("media")).expect("media dir");
    generate_fixture_source_with_gstreamer(
        &dir.path().join("media/input.webm"),
        320,
        180,
        24.0,
        2.0,
        Duration::from_secs(60),
    )
    .expect("generate fixture source");
    let mut project = sample_project();
    project.render_settings.width = 320;
    project.render_settings.height = 180;
    project.render_settings.fps = 24.0;
    project.timeline.duration_seconds = 1.0;
    project.media[0].relative_path = "media/input.webm".to_string();
    project.media[0].duration_seconds = 2.0;
    let item = &mut project.timeline.tracks[0].items[0];
    item.duration_seconds = 1.0;
    item.properties.insert("sourceIn".to_string(), json!(0.0));
    item.properties.insert("sourceOut".to_string(), json!(1.0));
    save_split_project(dir.path(), &project).expect("save split project");

    let job_id = "export-webm-workflow";
    let options = ExportRenderOptions::new(ExportProfile::Webm, RenderQuality::Draft, 320, 180)
        .expect("export options");
    let start_request = temporal_export_media_start_request_with_options(
        &project.id,
        &dir.path().to_string_lossy(),
        job_id,
        options,
        "exports/workflow.webm",
    );
    let mut queued = video_creater_lib::workflows::temporal_job_summary(
        TemporalWorkflowKind::ExportMedia,
        &project.id,
        job_id,
        JobStatus::Queued,
        "2026-09-13T00:00:00Z",
    );
    if let Some(workflow) = queued.workflow.as_mut() {
        workflow.activity_types = start_request.activity_types.clone();
    }
    queued.start_request = Some(start_request);
    video_creater_lib::project::split::apply_project_actions_to_split_project(
        dir.path(),
        vec![ProjectAction::RecordJob {
            job: Box::new(queued),
        }],
    )
    .expect("record queued workflow job");

    let mut activity_job = video_creater_lib::workflows::temporal_job_summary(
        TemporalWorkflowKind::RenderDraft,
        &project.id,
        job_id,
        JobStatus::Queued,
        "2026-09-13T00:00:01Z",
    );
    if let Some(workflow) = activity_job.workflow.as_mut() {
        workflow.run_id = Some("temporal-run-1".to_string());
    }
    let result = render_media_to_split_project_folder(
        dir.path(),
        &project.id,
        options,
        activity_job,
        "2026-09-13T00:00:01Z",
        Some("temporal-run-1".to_string()),
        None,
    )
    .expect("workflow render uses the recorded job");

    let job = result
        .project
        .jobs
        .iter()
        .find(|job| job.id == job_id)
        .expect("recorded job");
    assert_eq!(job.kind, "export_media");
    assert_eq!(job.status, JobStatus::Completed);
    assert!(job.start_request.is_some());
    assert_eq!(
        result
            .project
            .jobs
            .iter()
            .filter(|job| job.id == job_id)
            .count(),
        1
    );
}
