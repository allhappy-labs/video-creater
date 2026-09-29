#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

#[cfg(target_os = "macos")]
use serde_json::json;
#[cfg(target_os = "macos")]
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
#[cfg(target_os = "macos")]
use video_creater_lib::edit::render_plan::RenderQuality;
#[cfg(target_os = "macos")]
use video_creater_lib::project::export_options::ExportRenderOptions;
#[cfg(target_os = "macos")]
use video_creater_lib::project::export_profiles::ExportProfile;
#[cfg(target_os = "macos")]
use video_creater_lib::project::fixtures::sample_project;
#[cfg(target_os = "macos")]
use video_creater_lib::project::model::{
    JobStatus, JobSummary, MediaAsset, MediaKind, ProjectTimeline, TimelineItem, TimelineItemKind,
    TimelineSource,
};
#[cfg(target_os = "macos")]
use video_creater_lib::project::split::save_split_project;
#[cfg(target_os = "macos")]
use video_creater_lib::render_pipeline::gstreamer_backend::generate_fixture_source_with_gstreamer;
#[cfg(target_os = "macos")]
use video_creater_lib::render_pipeline::process::SystemProcessRunner;
#[cfg(target_os = "macos")]
use video_creater_lib::render_pipeline::project_export::{
    extract_rendered_frame_samples, render_media_to_split_project_folder,
    render_prepared_preview_frame_to_split_project_folder,
};
#[cfg(target_os = "macos")]
use video_creater_lib::render_runtime::start_render_process_runtime;

#[cfg(target_os = "macos")]
fn main() {
    if std::env::var_os("VIDEO_CREATER_HEADLESS_RUST_SUITE").is_some() {
        println!("project_export_nested_effect_appkit: deferred to dedicated native lane");
        return;
    }
    let exit_code = gstreamer::macos_main(|| match run_fixture() {
        Ok(()) => {
            println!("project_export_nested_effect_appkit: passed");
            0
        }
        Err(error) => {
            eprintln!("project_export_nested_effect_appkit: {error}");
            1
        }
    });
    std::process::exit(exit_code);
}

#[cfg(target_os = "macos")]
fn write_control_dotlottie(path: &Path) -> Result<(), String> {
    let file = fs::File::create(path).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let manifest = json!({
        "version": "2", "animations": [{"id":"main","themes":["dark"]}],
        "themes": [{"id":"dark"}], "stateMachines": [{"id":"button"}]
    });
    let animation = json!({
        "v":"5.7.4","fr":2,"ip":0,"op":4,"w":64,"h":64,"nm":"controls","ddd":0,"assets":[],
        "markers":[{"cm":"idle","tm":0,"dr":1},{"cm":"active","tm":2,"dr":1}],
        "slots":{"brand":{"p":{"a":0,"k":[1.0,0.0,0.0]}}},
        "layers":[{"ddd":0,"ind":1,"ty":4,"nm":"badge","sr":1,
            "ks":{"o":{"a":0,"k":100},"r":{"a":0,"k":0},"p":{"a":1,"k":[{"t":0,"s":[16,32,0],"e":[48,32,0]},{"t":2,"s":[48,32,0]}]},"a":{"a":0,"k":[0,0,0]},"s":{"a":0,"k":[100,100,100]}},
            "shapes":[{"ty":"rc","d":1,"s":{"a":0,"k":[20,20]},"p":{"a":0,"k":[0,0]},"r":{"a":0,"k":0,"nm":"Rectangle"}},
                {"ty":"fl","c":{"a":0,"k":[1.0,0.0,0.0],"sid":"brand"},"o":{"a":0,"k":100},"r":1,"nm":"Fill"}],"ip":0,"op":4,"st":0,"bm":0}]
    });
    let theme = json!({"rules":[{"type":"Color","id":"brand","value":[0.0,0.0,1.0]}]});
    let machine = json!({
        "initial":"idle","inputs":[{"type":"Boolean","name":"enabled","value":false},{"type":"Event","name":"activate"}],
        "states":[
            {"type":"PlaybackState","name":"idle","animation":"main","segment":"idle","transitions":[{"type":"Transition","toState":"active","guards":[{"type":"Boolean","inputName":"enabled","conditionType":"Equal","compareTo":true},{"type":"Event","inputName":"activate"}]}]},
            {"type":"PlaybackState","name":"active","animation":"main","segment":"active","transitions":[]}
        ]
    });
    for (name, value) in [
        ("manifest.json", manifest),
        ("a/main.json", animation),
        ("t/dark.json", theme),
        ("s/button.json", machine),
    ] {
        archive
            .start_file(name, options)
            .map_err(|error| error.to_string())?;
        archive
            .write_all(
                serde_json::to_string(&value)
                    .map_err(|error| error.to_string())?
                    .as_bytes(),
            )
            .map_err(|error| error.to_string())?;
    }
    archive.finish().map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("project_export_nested_effect_appkit: skipped outside macOS");
}

#[cfg(target_os = "macos")]
fn run_fixture() -> Result<(), String> {
    start_render_process_runtime().map_err(|error| error.to_string())?;
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../output/preview-render-native-evidence");
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(dir.join("media")).map_err(|error| error.to_string())?;
    fs::create_dir_all(dir.join("looks")).map_err(|error| error.to_string())?;
    write_control_dotlottie(&dir.join("media/controls.lottie"))?;
    fs::write(
        dir.join("looks/native.cube"),
        "LUT_3D_SIZE 2\n0 0 0\n0 0 1\n0 1 0\n0 1 1\n1 0 0\n1 0 1\n1 1 0\n1 1 1\n",
    )
    .map_err(|error| error.to_string())?;
    generate_fixture_source_with_gstreamer(
        &dir.join("media/input.webm"),
        320,
        180,
        24.0,
        4.0,
        Duration::from_secs(60),
    )
    .map_err(|errors| format!("fixture generation failed: {errors:?}"))?;

    let mut project = sample_project();
    project.render_settings.width = 320;
    project.render_settings.height = 180;
    project.render_settings.fps = 24.0;
    project.media[0].relative_path = "media/input.webm".to_string();
    project.media[0].duration_seconds = 4.0;
    project.media[0].width = Some(320);
    project.media[0].height = Some(180);
    project.media[0].fps = Some(24.0);
    project.media.push(MediaAsset {
        id: "lottie-controls".to_string(),
        name: Some("Theme and state controls".to_string()),
        relative_path: "media/controls.lottie".to_string(),
        kind: MediaKind::Lottie,
        duration_seconds: 2.0,
        width: Some(64),
        height: Some(64),
        fps: Some(2.0),
        folder_id: None,
    });
    let mut child = project.timeline.clone();
    child.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    child.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(3.0));
    child.tracks[0].items[0].properties.insert(
        "colorGrade".to_string(),
        json!({ "lut": { "path": "looks/native.cube", "strength": 0.7 } }),
    );
    child.tracks[0].items[0].properties.extend(BTreeMap::from([
        ("transform".to_string(), json!({ "centerX": 0.48, "centerY": 0.52, "width": 0.92, "height": 0.9, "rotationDegrees": 2.0 })),
        ("cropLeft".to_string(), json!(0.03)),
        ("cropRight".to_string(), json!(0.02)),
        ("fadeInSeconds".to_string(), json!(0.2)),
        ("fadeOutSeconds".to_string(), json!(0.25)),
        ("keyframes".to_string(), json!({
            "opacity": [
                { "atSeconds": 0.0, "value": 0.7, "easing": "easeOut" },
                { "atSeconds": 1.5, "value": 1.0, "easing": "easeInOut" },
                { "atSeconds": 3.0, "value": 0.75, "easing": "easeIn" }
            ],
            "positionX": [
                { "atSeconds": 0.0, "value": -0.05, "easing": "easeOut" },
                { "atSeconds": 3.0, "value": 0.05, "easing": "easeIn" }
            ],
            "positionY": [
                { "atSeconds": 0.0, "value": -0.03, "easing": "easeOut" },
                { "atSeconds": 3.0, "value": 0.03, "easing": "easeIn" }
            ],
            "scale": [
                { "atSeconds": 0.0, "value": 0.9, "easing": "easeOut" },
                { "atSeconds": 1.5, "value": 1.0, "easing": "easeInOut" },
                { "atSeconds": 3.0, "value": 0.92, "easing": "easeIn" }
            ],
            "rotationDegrees": [
                { "atSeconds": 0.0, "value": -2.0, "easing": "easeOut" },
                { "atSeconds": 1.5, "value": 3.0, "easing": "easeInOut" },
                { "atSeconds": 3.0, "value": 0.0, "easing": "easeIn" }
            ],
            "cropTop": [
                { "atSeconds": 0.0, "value": 0.0, "easing": "easeOut" },
                { "atSeconds": 1.5, "value": 0.04, "easing": "easeInOut" },
                { "atSeconds": 3.0, "value": 0.0, "easing": "easeIn" }
            ],
            "cropRight": [
                { "atSeconds": 0.0, "value": 0.02, "easing": "easeOut" },
                { "atSeconds": 1.5, "value": 0.06, "easing": "easeInOut" },
                { "atSeconds": 3.0, "value": 0.02, "easing": "easeIn" }
            ]
        })),
    ]));
    let mut upper_track = child.tracks[0].clone();
    upper_track.id = "track-richer-blend".to_string();
    upper_track.name = "Richer blend".to_string();
    upper_track.items[0].id = "richer-blend-item".to_string();
    upper_track.items[0]
        .properties
        .insert("blendMode".to_string(), json!("multiply"));
    child.tracks.push(upper_track);
    for (id, mode, opacity) in [("screen", "screen", 0.42), ("overlay", "overlay", 0.33)] {
        let mut track = child.tracks[0].clone();
        track.id = format!("track-{id}-blend");
        track.name = format!("{id} blend");
        track.items[0].id = format!("{id}-blend-item");
        track.items[0]
            .properties
            .insert("blendMode".to_string(), json!(mode));
        track.items[0]
            .properties
            .insert("opacity".to_string(), json!(opacity));
        child.tracks.push(track);
    }
    let mut lottie_track = child.tracks[0].clone();
    lottie_track.id = "track-lottie-controls".to_string();
    lottie_track.name = "Lottie theme and state".to_string();
    lottie_track.items = vec![TimelineItem {
        id: "lottie-theme-state-item".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 0.0,
        duration_seconds: 3.0,
        source: TimelineSource::Media {
            media_id: "lottie-controls".to_string(),
        },
        label: "Themed state badge".to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(0.0)),
            ("sourceOut".to_string(), json!(2.0)),
            ("animationId".to_string(), json!("main")),
            ("looping".to_string(), json!(true)),
            ("blendMode".to_string(), json!("overlay")),
            (
                "transform".to_string(),
                json!({ "centerX": 0.78, "centerY": 0.25, "width": 0.3, "height": 0.3 }),
            ),
            (
                "lottieInputs".to_string(),
                json!({
                    "themeId": "dark",
                    "slots": [],
                    "stateMachine": {
                        "id": "button",
                        "inputs": [{ "type": "boolean", "name": "enabled", "value": true }],
                        "events": ["activate"]
                    }
                }),
            ),
        ]),
    }];
    child.tracks.push(lottie_track);
    project.timelines.push(ProjectTimeline {
        id: "nested-effect-cut".to_string(),
        name: "Nested effect cut".to_string(),
        timeline: child,
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.start_seconds = 0.0;
    wrapper.duration_seconds = 3.0;
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "nested-effect-cut".to_string(),
    };
    wrapper.properties = BTreeMap::from([(
        "effects".to_string(),
        json!([
            { "effectType": "color.contrast", "enabled": true, "params": { "amount": 1.35 } }
        ]),
    )]);
    let visual_properties = BTreeMap::from([
        (
            "visualTreatment".to_string(),
            json!("compact translucent editorial label with cyan accent stroke"),
        ),
        (
            "motion".to_string(),
            json!("quick scale in, hold, and soft fade out"),
        ),
        (
            "safeZone".to_string(),
            json!("keep text inside 10% margins"),
        ),
        (
            "avoid".to_string(),
            json!("full-width opaque black slabs and static centered boxes"),
        ),
    ]);
    project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == "track-captions")
        .ok_or_else(|| "missing caption track".to_string())?
        .items
        .push(TimelineItem {
            id: "caption-fidelity".to_string(),
            kind: TimelineItemKind::Caption,
            start_seconds: 0.5,
            duration_seconds: 2.0,
            source: TimelineSource::Text {
                text: "Canonical preview".to_string(),
            },
            label: "Canonical preview".to_string(),
            properties: visual_properties.clone(),
        });
    project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == "track-overlays")
        .ok_or_else(|| "missing overlay track".to_string())?
        .items
        .push(TimelineItem {
            id: "overlay-fidelity".to_string(),
            kind: TimelineItemKind::Overlay,
            start_seconds: 1.0,
            duration_seconds: 1.5,
            source: TimelineSource::Text {
                text: "NATIVE".to_string(),
            },
            label: "Native badge".to_string(),
            properties: visual_properties,
        });
    save_split_project(&dir, &project).map_err(|error| error.to_string())?;

    let mut h264_output = None;
    for (profile, job_id, suffix) in [
        (
            ExportProfile::Mp4H264,
            "export-nested-effect-h264-appkit",
            "output.mp4",
        ),
        (
            ExportProfile::ProResMov,
            "export-nested-effect-prores-appkit",
            "output.mov",
        ),
    ] {
        let result = render_media_to_split_project_folder(
            &dir,
            &project.id,
            ExportRenderOptions::new(profile, RenderQuality::Final, 320, 180)
                .expect("explicit export options"),
            JobSummary {
                id: job_id.to_string(),
                kind: "exportMedia".to_string(),
                status: JobStatus::Queued,
                updated_at: "2026-07-11T00:00:00Z".to_string(),
                workflow: None,
                start_request: None,
                provider_request: None,
                failure_reason: None,
                export_settings: None,
            },
            "2026-07-11T00:00:00Z",
            None,
            None,
        )
        .map_err(|errors| format!("{profile:?} nested effect native render failed: {errors:?}"))?;
        let output = dir.join(&result.output_path);
        if !output.is_file() || !result.output_path.ends_with(suffix) {
            return Err(format!(
                "missing native {profile:?} output: {}",
                output.display()
            ));
        }
        if result.render_report.command.program != "avfoundation-native" {
            return Err(format!(
                "{profile:?} silently fell back to {}",
                result.render_report.command.program
            ));
        }
        if !result
            .render_report
            .streams
            .as_ref()
            .is_some_and(|streams| streams.video)
        {
            return Err(format!("{profile:?} native output has no video stream"));
        }
        let duration = result
            .render_report
            .summary
            .duration_seconds
            .unwrap_or_default();
        if !(2.9..=3.1).contains(&duration) {
            return Err(format!("{profile:?} duration drifted: {duration}"));
        }
        if !result
            .render_report
            .artifacts
            .iter()
            .any(|artifact| artifact.contains("precompose"))
        {
            return Err(format!(
                "{profile:?} missing prepared artifacts: {:?}",
                result.render_report.artifacts
            ));
        }
        if profile == ExportProfile::Mp4H264 {
            h264_output = Some(result.output_path.clone());
        }
    }

    let playhead_seconds = 1.5;
    let capture = render_prepared_preview_frame_to_split_project_folder(
        &dir,
        playhead_seconds,
        "canonical-preview-capture",
        "2026-07-11T00:00:00Z",
    )
    .map_err(|errors| format!("canonical preview capture failed: {errors:?}"))?;
    let final_output = h264_output.ok_or_else(|| "missing retained H264 output".to_string())?;
    let rendered_frames = extract_rendered_frame_samples(
        &SystemProcessRunner,
        &dir,
        "renders/export-nested-effect-h264-appkit/preview-qa",
        &final_output,
        &[playhead_seconds],
        Duration::from_secs(60),
    )
    .map_err(|errors| format!("final frame extraction failed: {errors:?}"))?;
    let rendered_frame = rendered_frames
        .first()
        .ok_or_else(|| "missing rendered frame".to_string())?;
    let comparison_path =
        dir.join("renders/export-nested-effect-h264-appkit/preview-qa/preview-comparison.json");
    let diff_dir = dir.join("renders/export-nested-effect-h264-appkit/preview-qa/diffs");
    let status = Command::new("node")
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."))
        .args([
            "scripts/compare-preview-render-frames.mjs",
            "--out",
            &comparison_path.display().to_string(),
            "--diff-dir",
            &diff_dir.display().to_string(),
            "--threshold",
            "0.08",
            "--channel-threshold",
            "18",
            "--fail-on-mismatch",
            "--frame",
            &format!(
                "{playhead_seconds}:{}:{}",
                dir.join(&capture.preview_frame).display(),
                dir.join(rendered_frame).display()
            ),
        ])
        .status()
        .map_err(|error| format!("comparison policy failed to start: {error}"))?;
    if !status.success() {
        return Err(format!(
            "preview/render mismatch policy failed; inspect {}",
            comparison_path.display()
        ));
    }
    if !comparison_path.is_file() || !dir.join(&capture.evidence_report).is_file() {
        return Err("retained preview/render evidence is incomplete".to_string());
    }
    Ok(())
}
