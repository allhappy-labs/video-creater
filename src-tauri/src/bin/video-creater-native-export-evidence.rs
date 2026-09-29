use serde_json::json;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(not(target_os = "macos"))]
use std::process::ExitCode;
use std::time::Duration;
use video_creater_lib::edit::render_plan::RenderQuality;
use video_creater_lib::project::export_options::ExportRenderOptions;
use video_creater_lib::project::export_profiles::{
    mp4_export_profile_availability_report, ExportProfile,
};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    JobStatus, JobSummary, MediaAsset, MediaKind, TimelineItem, TimelineItemKind, TimelineSource,
    TrackKind, VideoProject,
};
use video_creater_lib::project::split::save_split_project;
use video_creater_lib::render_pipeline::gstreamer_backend::generate_fixture_source_with_gstreamer;
use video_creater_lib::render_pipeline::project_export::render_media_to_split_project_folder;
use video_creater_lib::render_runtime::start_render_process_runtime;

const UPDATED_AT: &str = "2026-07-10T00:00:00Z";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvidenceProfile {
    Mp4H264,
    Mp4H265,
    ProResMov,
}

impl EvidenceProfile {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "mp4H264" => Some(Self::Mp4H264),
            "mp4H265" => Some(Self::Mp4H265),
            "proResMov" => Some(Self::ProResMov),
            _ => None,
        }
    }

    fn export_profile(self) -> ExportProfile {
        match self {
            Self::Mp4H264 => ExportProfile::Mp4H264,
            Self::Mp4H265 => ExportProfile::Mp4H265,
            Self::ProResMov => ExportProfile::ProResMov,
        }
    }

    fn job_id(self) -> &'static str {
        match self {
            Self::Mp4H264 => "export-h264",
            Self::Mp4H265 => "export-h265",
            Self::ProResMov => "export-prores",
        }
    }

    fn profile_id(self) -> &'static str {
        match self {
            Self::Mp4H264 => "mp4H264",
            Self::Mp4H265 => "mp4H265",
            Self::ProResMov => "proResMov",
        }
    }

    fn all() -> [Self; 3] {
        [Self::Mp4H264, Self::Mp4H265, Self::ProResMov]
    }
}

struct EvidenceArguments {
    project_dir: PathBuf,
    profile: Option<EvidenceProfile>,
    assemble: bool,
}

#[cfg(target_os = "macos")]
fn main() {
    if let Err(error) = start_render_process_runtime() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    let exit_code = gstreamer::macos_main(run_exit_code);
    std::process::exit(exit_code);
}

#[cfg(not(target_os = "macos"))]
fn main() -> ExitCode {
    if let Err(error) = start_render_process_runtime() {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    ExitCode::from(u8::try_from(run_exit_code()).unwrap_or(1))
}

fn run_exit_code() -> i32 {
    match run() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("native export evidence failed: {error}");
            1
        }
    }
}

fn run() -> Result<(), String> {
    let arguments = parse_arguments(env::args().skip(1))?;
    if arguments.assemble {
        return write_evidence_manifest(&arguments.project_dir);
    }
    let profile = arguments
        .profile
        .ok_or_else(|| "provide --profile mp4H264|mp4H265|proResMov or --assemble".to_string())?;
    let evidence_dir = arguments.project_dir;
    let availability = mp4_export_profile_availability_report()
        .into_iter()
        .find(|candidate| candidate.profile == profile.export_profile())
        .ok_or_else(|| format!("{} availability was not reported", profile.profile_id()))?;
    if !availability.available {
        return Err(format!(
            "{} is unavailable in the curated native runtime: {}",
            profile.profile_id(),
            availability
                .unavailable_reason
                .unwrap_or_else(|| "unknown availability failure".to_string())
        ));
    }
    let execution_dir = tempfile::tempdir()
        .map_err(|error| format!("could not create isolated export fixture: {error}"))?;
    let project_dir = execution_dir.path();
    fs::create_dir_all(project_dir.join("media"))
        .map_err(|error| format!("could not create evidence media directory: {error}"))?;
    let source_path = project_dir.join("media/input.mp4");
    generate_fixture_source_with_gstreamer(
        &source_path,
        320,
        180,
        24.0,
        4.0,
        Duration::from_secs(60),
    )
    .map_err(|errors| format!("could not generate GStreamer fixture: {errors:?}"))?;

    let project = build_evidence_project();
    save_split_project(project_dir, &project)
        .map_err(|error| format!("could not save evidence project: {error}"))?;

    let result = render_media_to_split_project_folder(
        project_dir,
        &project.id,
        ExportRenderOptions::new(profile.export_profile(), RenderQuality::Final, 320, 180)
            .map_err(|error| error.to_string())?,
        evidence_job(profile.job_id()),
        UPDATED_AT,
        None,
        None,
    )
    .map_err(|errors| format!("{} render failed: {errors:?}", profile.profile_id()))?;
    let ffprobe_path = format!("renders/{}/ffprobe.json", profile.job_id());
    write_ffprobe_json(
        &project_dir.join(&result.output_path),
        &project_dir.join(&ffprobe_path),
    )?;
    retain_profile_artifacts(project_dir, &evidence_dir, profile)?;
    println!("retained {} export evidence", profile.profile_id());
    Ok(())
}

fn build_evidence_project() -> VideoProject {
    let mut project = sample_project();
    project.render_settings.width = 320;
    project.render_settings.height = 180;
    project.render_settings.fps = 24.0;
    project.timeline.duration_seconds = 3.0;
    project.media[0].relative_path = "media/input.mp4".to_string();
    project.media[0].duration_seconds = 4.0;
    project.media[0].width = Some(320);
    project.media[0].height = Some(180);
    project.media[0].fps = Some(24.0);
    project.media.push(MediaAsset {
        id: "native-export-audio-source".to_string(),
        name: Some("Native export source audio".to_string()),
        relative_path: "media/input.mp4".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 4.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });

    let video_item = &mut project.timeline.tracks[0].items[0];
    video_item.duration_seconds = 3.0;
    video_item
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    video_item
        .properties
        .insert("sourceOut".to_string(), json!(3.0));
    let audio_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Audio)
        .expect("sample project must include an audio track");
    audio_track.items.push(TimelineItem {
        id: "native-export-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 3.0,
        source: TimelineSource::Media {
            media_id: "native-export-audio-source".to_string(),
        },
        label: "Native export source audio".to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(0.0)),
            ("sourceOut".to_string(), json!(3.0)),
        ]),
    });
    project
}

fn retain_profile_artifacts(
    execution_dir: &Path,
    evidence_dir: &Path,
    profile: EvidenceProfile,
) -> Result<(), String> {
    fs::create_dir_all(evidence_dir.join("media"))
        .map_err(|error| format!("could not create retained media directory: {error}"))?;
    fs::copy(
        execution_dir.join("media/input.mp4"),
        evidence_dir.join("media/input.mp4"),
    )
    .map_err(|error| format!("could not retain fixture source: {error}"))?;

    let source_dir = execution_dir.join("renders").join(profile.job_id());
    let destination_dir = evidence_dir.join("renders").join(profile.job_id());
    if destination_dir.exists() {
        fs::remove_dir_all(&destination_dir).map_err(|error| {
            format!(
                "could not replace retained {} artifacts: {error}",
                profile.profile_id()
            )
        })?;
    }
    copy_directory(&source_dir, &destination_dir)
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("could not create retained artifact directory: {error}"))?;
    for entry in fs::read_dir(source)
        .map_err(|error| format!("could not read generated artifact directory: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("could not read generated artifact entry: {error}"))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry
            .file_type()
            .map_err(|error| format!("could not inspect generated artifact entry: {error}"))?
            .is_dir()
        {
            copy_directory(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).map_err(|error| {
                format!(
                    "could not retain generated artifact {}: {error}",
                    source_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn write_evidence_manifest(project_dir: &Path) -> Result<(), String> {
    let artifacts = EvidenceProfile::all()
        .into_iter()
        .map(|profile| {
            let job_id = profile.job_id();
            let extension = match profile {
                EvidenceProfile::ProResMov => "mov",
                EvidenceProfile::Mp4H264 | EvidenceProfile::Mp4H265 => "mp4",
            };
            json!({
                "profile": profile.profile_id(),
                "report": format!("renders/{job_id}/pipeline-report.json"),
                "output": format!("renders/{job_id}/output.{extension}"),
                "probe": format!("renders/{job_id}/ffprobe.json"),
            })
        })
        .collect::<Vec<_>>();
    fs::write(
        project_dir.join("native-export-evidence.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({
                "nativeExporterReport": "avfoundation-exporter.json",
                "artifacts": artifacts,
            }))
            .map_err(|error| format!("could not serialize evidence manifest: {error}"))?
        ),
    )
    .map_err(|error| format!("could not write evidence manifest: {error}"))?;
    println!(
        "retained native export evidence at {}",
        project_dir.join("native-export-evidence.json").display()
    );
    Ok(())
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<EvidenceArguments, String> {
    let arguments = arguments
        .filter(|argument| argument != "--")
        .collect::<Vec<_>>();
    let Some(project_dir) = arguments
        .first()
        .filter(|argument| !argument.starts_with('-'))
    else {
        return Err(usage());
    };
    let mut profile = None;
    let mut assemble = false;
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--profile" => {
                let value = arguments.get(index + 1).ok_or_else(usage)?;
                profile = Some(EvidenceProfile::parse(value).ok_or_else(|| {
                    "--profile must be mp4H264, mp4H265, or proResMov".to_string()
                })?);
                index += 2;
            }
            "--assemble" => {
                assemble = true;
                index += 1;
            }
            _ => return Err(usage()),
        }
    }
    if assemble && profile.is_some() {
        return Err("--assemble cannot be combined with --profile".to_string());
    }
    let project_dir = PathBuf::from(project_dir);
    let project_dir = if project_dir.is_absolute() {
        project_dir
    } else {
        env::current_dir()
            .map(|current_dir| current_dir.join(project_dir))
            .map_err(|error| format!("could not resolve evidence directory: {error}"))?
    };
    Ok(EvidenceArguments {
        project_dir,
        profile,
        assemble,
    })
}

fn usage() -> String {
    "usage: video-creater-native-export-evidence <project-evidence-directory> --profile mp4H264|mp4H265|proResMov\n       video-creater-native-export-evidence <project-evidence-directory> --assemble"
        .to_string()
}

#[cfg(test)]
mod evidence_project_tests {
    use super::*;
    use video_creater_lib::project::model::{
        MediaKind, TimelineItemKind, TimelineSource, TrackKind,
    };

    #[test]
    fn evidence_project_routes_source_audio_to_the_export_timeline() {
        let project = build_evidence_project();
        let audio_asset = project
            .media
            .iter()
            .find(|asset| asset.kind == MediaKind::Audio)
            .expect("evidence project must retain the source audio as an audio asset");
        assert_eq!(audio_asset.relative_path, "media/input.mp4");

        let audio_item = project
            .timeline
            .tracks
            .iter()
            .find(|track| track.kind == TrackKind::Audio)
            .and_then(|track| track.items.first())
            .expect("evidence project must route source audio onto an audio track");
        assert_eq!(audio_item.kind, TimelineItemKind::AudioClip);
        assert_eq!(audio_item.duration_seconds, 3.0);
        assert_eq!(audio_item.properties["sourceIn"], json!(0.0));
        assert_eq!(audio_item.properties["sourceOut"], json!(3.0));
        assert!(matches!(
            &audio_item.source,
            TimelineSource::Media { media_id } if media_id == &audio_asset.id
        ));
    }
}

fn evidence_job(id: &str) -> JobSummary {
    JobSummary {
        id: id.to_string(),
        kind: "exportMedia".to_string(),
        status: JobStatus::Queued,
        updated_at: UPDATED_AT.to_string(),
        workflow: None,
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    }
}

fn write_ffprobe_json(output_path: &Path, destination_path: &Path) -> Result<(), String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
        ])
        .arg(output_path)
        .output()
        .map_err(|error| format!("could not start ffprobe: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "ffprobe failed for {}: {}",
            output_path.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    fs::write(destination_path, output.stdout)
        .map_err(|error| format!("could not write ffprobe JSON: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{parse_arguments, EvidenceProfile};

    #[test]
    fn accepts_the_package_manager_argument_separator() {
        let parsed = parse_arguments(
            [
                "--".to_string(),
                "output/evidence".to_string(),
                "--profile".to_string(),
                "mp4H264".to_string(),
            ]
            .into_iter(),
        )
        .expect("separator should be accepted");
        assert!(parsed.project_dir.is_absolute());
        assert!(parsed.project_dir.ends_with("output/evidence"));
        assert_eq!(parsed.profile, Some(EvidenceProfile::Mp4H264));
    }
}
