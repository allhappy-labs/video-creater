use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use video_creater_lib::edit::render_plan::RenderQuality;
use video_creater_lib::precompose::{
    prepare_project_for_render, prepare_project_for_render_cancellable,
};
use video_creater_lib::project::export_options::ExportRenderOptions;
use video_creater_lib::project::export_profiles::ExportProfile;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::import::import_media_files;
use video_creater_lib::project::model::{
    JobStatus, JobSummary, TimelineItem, TimelineItemKind, TimelineSource, TimelineTrack, TrackKind,
};
use video_creater_lib::project::split::save_split_project;
use video_creater_lib::render_pipeline::cancel::{
    register_render_attempt, request_render_cancellation, RenderAttemptKey,
    RenderCancellationOutcome,
};
use video_creater_lib::render_pipeline::project_export::render_media_to_split_project_folder;
use video_creater_lib::render_runtime::start_render_process_runtime;

fn main() {
    if let Err(error) = start_render_process_runtime() {
        eprintln!("render runtime startup failed: {error}");
        std::process::exit(1);
    }
    if let Err(error) = run() {
        eprintln!("compatibility evidence failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let output = env::args()
        .skip(1)
        .find(|argument| argument != "--")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("output/compatibility-decoder-evidence"));
    if output.exists() {
        fs::remove_dir_all(&output).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(output.join("fixtures")).map_err(|error| error.to_string())?;
    let output = output.canonicalize().map_err(|error| error.to_string())?;
    let source = output.join("fixtures/source-vp9-opus.webm");
    generate_webm(&source)?;
    let imported = import_media_files(&output, sample_project(), std::slice::from_ref(&source))
        .map_err(|error| error.to_string())?;
    let imported_media = imported
        .imported
        .first()
        .cloned()
        .ok_or("compatibility import returned no media")?;
    if imported_media.duration_seconds <= 0.0
        || imported_media.width != Some(320)
        || imported_media.height != Some(180)
        || imported_media.fps.is_none()
    {
        return Err(format!(
            "compatibility import metadata was incomplete: {imported_media:?}"
        ));
    }
    let mut project = imported.project;
    project.media.retain(|media| media.id == imported_media.id);
    let visual = &mut project.timeline.tracks[0].items[0];
    visual.source = TimelineSource::Media {
        media_id: imported_media.id.clone(),
    };
    visual.start_seconds = 0.0;
    visual.duration_seconds = 2.0;
    visual.properties.insert("sourceIn".into(), json!(0.0));
    visual.properties.insert("sourceOut".into(), json!(2.0));
    project.timeline.duration_seconds = 2.0;
    project.timeline.tracks.push(TimelineTrack {
        transitions: Vec::new(),
        id: "compat-audio".into(),
        name: "Compatibility audio".into(),
        kind: TrackKind::Audio,
        locked: false,
        sync_locked: true,
        enabled: true,
        items: vec![TimelineItem {
            id: "compat-audio-item".into(),
            kind: TimelineItemKind::AudioClip,
            start_seconds: 0.0,
            duration_seconds: 2.0,
            source: TimelineSource::Media {
                media_id: imported_media.id.clone(),
            },
            label: "VP9 Opus audio".into(),
            properties: BTreeMap::from([
                ("sourceIn".into(), json!(0.0)),
                ("sourceOut".into(), json!(2.0)),
            ]),
        }],
    });
    project.timelines[0].timeline = project.timeline.clone();
    save_split_project(&output, &project).map_err(|error| error.to_string())?;
    let first_preparation = prepare_project_for_render(&output, &project)
        .map_err(|errors| format!("first compatibility preparation failed: {errors:?}"))?;
    let first_report = first_preparation
        .reports
        .iter()
        .find(|report| report.stage == "compatibilityDecode")
        .ok_or("first preparation omitted compatibility report")?;
    if first_report.cache_hit {
        return Err("first compatibility preparation unexpectedly hit cache".into());
    }
    let second_preparation = prepare_project_for_render(&output, &project)
        .map_err(|errors| format!("cached compatibility preparation failed: {errors:?}"))?;
    let second_report = second_preparation
        .reports
        .iter()
        .find(|report| report.stage == "compatibilityDecode")
        .ok_or("cached preparation omitted compatibility report")?;
    if !second_report.cache_hit {
        return Err("second compatibility preparation did not hit cache".into());
    }
    let cancellation_key = RenderAttemptKey::new(
        fs::canonicalize(&output).map_err(|error| error.to_string())?,
        &project.id,
        "compatibility-cancel-evidence",
        "evidence-attempt",
    )
    .map_err(|error| error.to_string())?;
    let cancellation =
        register_render_attempt(cancellation_key.clone()).map_err(|error| error.to_string())?;
    let cancellation_token = cancellation.token();
    if request_render_cancellation(&cancellation_key) != RenderCancellationOutcome::Requested
        || prepare_project_for_render_cancellable(&output, &project, Some(&cancellation_token))
            .is_ok()
    {
        return Err("compatibility preparation did not honor cancellation".into());
    }
    let result = render_media_to_split_project_folder(
        &output,
        &project.id,
        ExportRenderOptions::new(
            ExportProfile::Mp4H264,
            RenderQuality::Final,
            project.render_settings.width,
            project.render_settings.height,
        )
        .map_err(|error| error.to_string())?,
        JobSummary {
            id: "compatibility-native-h264".into(),
            kind: "exportMedia".into(),
            status: JobStatus::Queued,
            updated_at: "2026-07-12T00:00:00Z".into(),
            workflow: None,
            start_request: None,
            provider_request: None,
            failure_reason: None,
            export_settings: None,
        },
        "2026-07-12T00:00:00Z",
        None,
        None,
    )
    .map_err(|errors| format!("{errors:?}"))?;
    let compatibility_intermediate = result
        .render_report
        .artifacts
        .iter()
        .find(|artifact| artifact.ends_with("intermediate.mov"))
        .cloned()
        .ok_or("render report omitted the compatibility intermediate")?;
    if !output.join(&compatibility_intermediate).is_file() {
        return Err("compatibility intermediate is missing".into());
    }
    let report: Value = json!({
        "schemaVersion":1,
        "status":"passed",
        "source":source.strip_prefix(&output).map_err(|error| error.to_string())?,
        "importMetadata": {
            "durationSeconds": imported_media.duration_seconds,
            "width": imported_media.width,
            "height": imported_media.height,
            "fps": imported_media.fps,
        },
        "compatibilityIntermediate": compatibility_intermediate,
        "cache": {"firstHit": first_report.cache_hit, "secondHit": second_report.cache_hit},
        "cancellation": "passed",
        "output":result.output_path,
        "renderBackend":result.render_report.command.program,
        "streams":result.render_report.streams,
        "artifacts":result.render_report.artifacts
    });
    fs::write(
        output.join("compatibility-evidence.json"),
        serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    println!("{}", output.join("compatibility-evidence.json").display());
    Ok(())
}

fn generate_webm(path: &Path) -> Result<(), String> {
    let location = format!("location={}", path.display());
    let status = Command::new("gst-launch-1.0")
        .args([
            "-q",
            "webmmux",
            "name=mux",
            "!",
            "filesink",
            &location,
            "videotestsrc",
            "num-buffers=48",
            "pattern=ball",
            "!",
            "video/x-raw,width=320,height=180,framerate=24/1",
            "!",
            "vp9enc",
            "deadline=1",
            "!",
            "mux.video_0",
            "audiotestsrc",
            "num-buffers=96",
            "wave=sine",
            "!",
            "audio/x-raw,rate=48000",
            "!",
            "opusenc",
            "!",
            "mux.audio_0",
        ])
        .status()
        .map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("fixture generation exited {status}"))
    }
}
