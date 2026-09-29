#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::path::Path;
#[cfg(target_os = "macos")]
use video_creater_lib::edit::render_plan::RenderQuality;
#[cfg(target_os = "macos")]
use video_creater_lib::project::export_options::ExportRenderOptions;
#[cfg(target_os = "macos")]
use video_creater_lib::project::export_profiles::{
    mp4_export_profile_availability_report, ExportProfile,
};
#[cfg(target_os = "macos")]
use video_creater_lib::project::fixtures::sample_project;
#[cfg(target_os = "macos")]
use video_creater_lib::project::model::{JobStatus, JobSummary};
#[cfg(target_os = "macos")]
use video_creater_lib::project::split::save_split_project;
#[cfg(target_os = "macos")]
use video_creater_lib::render_pipeline::project_export::render_media_to_split_project_folder;
#[cfg(target_os = "macos")]
use video_creater_lib::render_runtime::start_render_process_runtime;

#[cfg(target_os = "macos")]
fn main() {
    if std::env::var_os("VIDEO_CREATER_HEADLESS_RUST_SUITE").is_some() {
        println!("project_export_prores_appkit: deferred to dedicated native lane");
        return;
    }
    let exit_code = gstreamer::macos_main(|| match run_fixture() {
        Ok(()) => {
            println!("project_export_prores_appkit: passed");
            0
        }
        Err(error) => {
            eprintln!("project_export_prores_appkit: {error}");
            1
        }
    });
    std::process::exit(exit_code);
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("project_export_prores_appkit: skipped outside macOS");
}

#[cfg(target_os = "macos")]
fn run_fixture() -> Result<(), String> {
    start_render_process_runtime().map_err(|error| error.to_string())?;
    let availability = mp4_export_profile_availability_report()
        .into_iter()
        .find(|candidate| candidate.profile == ExportProfile::ProResMov)
        .ok_or_else(|| "ProRes availability was not reported".to_string())?;
    if !availability.available {
        return Err(format!(
            "ProRes must be available in the curated native runtime: {}",
            availability
                .unavailable_reason
                .unwrap_or_else(|| "unknown availability failure".to_string())
        ));
    }

    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    fs::create_dir_all(dir.path().join("media")).map_err(|error| error.to_string())?;
    let source_path = dir.path().join("media/input.mp4");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/media/edison-speech-1920s-30s.mp4"),
        &source_path,
    )
    .map_err(|error| format!("copy retained system-decodable fixture: {error}"))?;

    let mut project = sample_project();
    project.media[0].relative_path = "media/input.mp4".to_string();
    project.media[0].duration_seconds = 4.0;
    project.media[0].width = Some(320);
    project.media[0].height = Some(180);
    project.media[0].fps = Some(24.0);
    let item = &mut project.timeline.tracks[0].items[0];
    item.properties
        .insert("sourceIn".to_string(), serde_json::json!(0.0));
    item.properties
        .insert("sourceOut".to_string(), serde_json::json!(3.0));
    save_split_project(dir.path(), &project).map_err(|error| error.to_string())?;

    let result = render_media_to_split_project_folder(
        dir.path(),
        &project.id,
        ExportRenderOptions::new(ExportProfile::ProResMov, RenderQuality::Final, 320, 180)
            .expect("explicit export options"),
        JobSummary {
            id: "export-prores-appkit-e2e".to_string(),
            kind: "exportMedia".to_string(),
            status: JobStatus::Queued,
            updated_at: "2026-07-10T00:00:00Z".to_string(),
            workflow: None,
            start_request: None,
            provider_request: None,
            failure_reason: None,
            export_settings: None,
        },
        "2026-07-10T00:00:00Z",
        None,
        None,
    )
    .map_err(|errors| format!("ProRes render failed: {errors:?}"))?;

    if result.output_path != "renders/export-prores-appkit-e2e/output.mov" {
        return Err(format!("unexpected output path: {}", result.output_path));
    }
    let output_path = dir.path().join(&result.output_path);
    if !output_path.is_file() {
        return Err(format!("missing ProRes output: {}", output_path.display()));
    }
    if !result
        .render_report
        .streams
        .as_ref()
        .is_some_and(|streams| streams.video)
    {
        return Err("render report did not record a video stream".to_string());
    }
    if availability
        .required_runtime
        .iter()
        .any(|runtime| runtime == "system:avfoundation")
        && result.render_report.command.program != "avfoundation-native"
    {
        return Err(format!(
            "expected AVFoundation render command, got {}",
            result.render_report.command.program
        ));
    }
    if !result
        .render_report
        .stdout
        .contains("\"videoCodec\":\"apcn\"")
    {
        return Err(format!(
            "native exporter did not report the ProRes 422 apcn codec: {}",
            result.render_report.stdout
        ));
    }

    Ok(())
}
