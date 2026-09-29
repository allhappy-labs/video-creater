//! Plans without transitions, and backend selection for plans with them.

use super::transition_fixtures::*;
use std::collections::BTreeMap;
use std::path::Path;
use video_creater_lib::edit::render_plan::{
    RenderOutputProfile, RenderPlan, RenderQuality, RenderQualityProfile,
};
use video_creater_lib::project::export_profiles::ExportProfile;
use video_creater_lib::project::model::*;
use video_creater_lib::render_pipeline::avfoundation_backend::{
    AvFoundationExportProfile, AvFoundationRenderBackend,
};
use video_creater_lib::render_pipeline::project_export::{
    build_project_media_render_plan, build_project_webm_render_plan,
    build_project_webm_render_plan_for_range, expand_project_nested_timelines_for_render,
    select_render_backend, ProjectRenderBackend,
};

fn render_json(dir: &Path, plan: &RenderPlan) -> String {
    serde_json::to_string_pretty(plan)
        .expect("plan serializes")
        .replace(&dir.display().to_string(), "<dir>")
}

/// A project without transitions that exercises speed, image, audio, a second
/// video track and a nested sequence.
fn project_without_transitions() -> VideoProject {
    let mut project = base_project();
    project.timeline.tracks[0].items = vec![
        video("clip-a", 0.0, 4.0, 2.0),
        clip(
            "clip-b",
            TimelineItemKind::VideoClip,
            "media-2",
            4.0,
            2.0,
            6.0,
            2.0,
        ),
    ];
    let mut upper = TimelineTrack::empty("track-video-2", "Video 2", TrackKind::Video);
    upper.items = vec![clip(
        "still-a",
        TimelineItemKind::ImageClip,
        "still",
        1.0,
        2.0,
        0.0,
        1.0,
    )];
    project.timeline.tracks.push(upper);
    project.timeline.tracks[4].items = vec![
        clip(
            "music-a",
            TimelineItemKind::AudioClip,
            "music",
            0.0,
            3.0,
            1.0,
            1.0,
        ),
        clip(
            "music-b",
            TimelineItemKind::AudioClip,
            "music",
            3.0,
            3.0,
            10.0,
            1.0,
        ),
    ];
    let mut nested = Timeline::default_editor_timeline();
    nested.duration_seconds = 4.0;
    nested.tracks[0].items = vec![
        video("nested-a", 0.0, 2.0, 1.0),
        video("nested-b", 2.0, 2.0, 8.0),
    ];
    project.timelines.push(ProjectTimeline {
        id: "nested".to_string(),
        name: "Nested".to_string(),
        timeline: nested,
    });
    let mut wrapper = TimelineTrack::empty("track-video-3", "Video 3", TrackKind::Video);
    wrapper.items = vec![TimelineItem {
        id: "wrapper".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 6.0,
        duration_seconds: 4.0,
        source: TimelineSource::Timeline {
            timeline_id: "nested".to_string(),
        },
        label: "Nested".to_string(),
        properties: BTreeMap::new(),
    }];
    project.timeline.tracks.push(wrapper);
    project.timeline.duration_seconds = 10.0;
    project
}

#[test]
fn plans_and_expanded_projects_without_transitions_are_unchanged() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let project = project_without_transitions();
    let full = build_project_webm_render_plan(
        dir.path(),
        &project,
        "full",
        RenderQualityProfile::DraftWebm,
    )
    .expect("full plan");
    let range = build_project_webm_render_plan_for_range(
        dir.path(),
        &project,
        "range",
        RenderQualityProfile::DraftWebm,
        1.5,
        7.0,
    )
    .expect("range plan");
    let mp4 = build_project_media_render_plan(
        dir.path(),
        &project,
        "mp4",
        RenderQuality::Final,
        ExportProfile::Mp4H264,
    )
    .expect("mp4 plan");
    let expanded = expand_project_nested_timelines_for_render(&project).expect("expand nested");
    let actual = format!(
        "{}\n---\n{}\n---\n{}\n---\n{}\n",
        render_json(dir.path(), &full),
        render_json(dir.path(), &range),
        render_json(dir.path(), &mp4),
        serde_json::to_string_pretty(&expanded.timeline).expect("timeline serializes"),
    );

    // Captured from the render plan builder before transitions existed.
    assert_eq!(actual, include_str!("plans_without_transitions.golden.txt"));
}

#[test]
fn avfoundation_rejects_transition_plans_and_export_selection_falls_back_to_ges() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let mp4_plan = |project: &VideoProject| {
        build_project_media_render_plan(
            dir.path(),
            project,
            "export",
            RenderQuality::Final,
            ExportProfile::Mp4H264,
        )
        .expect("mp4 plan")
    };
    let avfoundation = AvFoundationRenderBackend::new();
    let select = |plan: &RenderPlan| {
        select_render_backend(
            plan.output_profile,
            Some(AvFoundationExportProfile::H264),
            &avfoundation,
            "export",
            plan,
            &[],
        )
    };

    let mut hard_cut = crossfade_project();
    hard_cut.timeline.tracks[0].transitions.clear();
    let hard_cut_plan = mp4_plan(&hard_cut);
    assert_eq!(
        hard_cut_plan.output_profile,
        RenderOutputProfile::Mp4Primary
    );
    avfoundation
        .build_request("export", &hard_cut_plan, &[])
        .expect("AVFoundation accepts hard cuts");
    assert_eq!(select(&hard_cut_plan), ProjectRenderBackend::AvFoundation);

    let transition_plan = mp4_plan(&crossfade_project());
    let error = avfoundation
        .build_request("export", &transition_plan, &[])
        .expect_err("AVFoundation rejects transitions");
    assert_eq!(error[0].path, "renderPlan.transitions");
    assert_eq!(
        error[0].message,
        "AVFoundation export does not support clip transitions."
    );
    assert_eq!(select(&transition_plan), ProjectRenderBackend::GstreamerGes);

    let mut audio_only = crossfade_project();
    audio_only.timeline.tracks[0].transitions.clear();
    audio_only.timeline.tracks[AUDIO_TRACK_INDEX].items = vec![
        clip(
            "music-a",
            TimelineItemKind::AudioClip,
            "music",
            0.0,
            3.0,
            1.0,
            1.0,
        ),
        clip(
            "music-b",
            TimelineItemKind::AudioClip,
            "music",
            3.0,
            3.0,
            10.0,
            1.0,
        ),
    ];
    add_transition(
        &mut audio_only,
        "track-audio",
        "music-fade",
        ("music-a", "music-b"),
        TransitionKind::Crossfade,
        1.0,
    );
    let audio_plan = mp4_plan(&audio_only);
    assert!(audio_plan.transitions.is_empty());
    assert_eq!(select(&audio_plan), ProjectRenderBackend::GstreamerGes);

    // WebM and profiles without native support never pick AVFoundation.
    assert_eq!(
        select_render_backend(
            RenderOutputProfile::WebDelivery,
            Some(AvFoundationExportProfile::H264),
            &avfoundation,
            "export",
            &hard_cut_plan,
            &[],
        ),
        ProjectRenderBackend::GstreamerGes
    );
    assert_eq!(
        select_render_backend(
            hard_cut_plan.output_profile,
            None,
            &avfoundation,
            "export",
            &hard_cut_plan,
            &[],
        ),
        ProjectRenderBackend::GstreamerGes
    );
}
