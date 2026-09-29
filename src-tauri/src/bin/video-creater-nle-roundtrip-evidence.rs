use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use video_creater_lib::project::model::{
    CaptionRenderMode, MediaAsset, MediaKind, TimelineItem, TimelineItemKind, TimelineSource,
    TimelineTrack, TrackKind, VideoProject,
};
use video_creater_lib::project::nle_export::{export_project_timeline_to_nle_xml, NleXmlFormat};

const FPS: f64 = 24.0;
const SOURCE_DURATION: f64 = 8.0;

#[derive(Debug)]
struct Arguments {
    output: PathBuf,
    video: Option<PathBuf>,
    audio: Option<PathBuf>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedClip {
    id: &'static str,
    track_kind: &'static str,
    media_id: &'static str,
    timeline_in_seconds: f64,
    timeline_out_seconds: f64,
    source_in_seconds: f64,
    source_out_seconds: f64,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("NLE round-trip evidence generation failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = parse_arguments(env::args().skip(1))?;
    let media_dir = arguments.output.join("media");
    fs::create_dir_all(&media_dir)
        .map_err(|error| format!("could not create {}: {error}", media_dir.display()))?;
    let video = prepare_media(
        arguments.video,
        media_dir.join("nle-source-av.mp4"),
        generate_video,
    )?;
    let audio = prepare_media(
        arguments.audio,
        media_dir.join("nle-source-voiceover.wav"),
        generate_audio,
    )?;
    let project = evidence_project(&video, &audio);
    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .map_err(|error| format!("could not export DaVinci FCPXML: {error}"))?;

    let project_path = arguments.output.join("project.json");
    let export_path = arguments.output.join(export.filename);
    let expected_path = arguments.output.join("expected.json");
    write_json(&project_path, &project)?;
    fs::write(&export_path, export.xml)
        .map_err(|error| format!("could not write {}: {error}", export_path.display()))?;
    write_json(
        &expected_path,
        &json!({
            "schemaVersion": 1,
            "projectId": project.id,
            "timelineName": project.name,
            "format": "davinciFcpxml",
            "fps": FPS,
            "width": 640,
            "height": 360,
            "durationSeconds": 5.0,
            "expectedVideoTrackCount": 1,
            "expectedAudioTrackCount": 1,
            "expectedClips": expected_clips(),
            "sourceMedia": { "video": video, "audio": audio },
            "artifacts": { "project": project_path, "interchange": export_path },
            "verification": {
                "import": "Compare the imported Resolve timeline and source ranges to expectedClips.",
                "render": "Verify a 5.0 second 640x360 render with video and audio streams.",
                "roundTrip": "Export the Resolve timeline as FCPXML beside this manifest."
            }
        }),
    )?;
    println!("{}", expected_path.display());
    Ok(())
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let mut result = Arguments {
        output: PathBuf::from("output/nle-roundtrip-evidence"),
        video: None,
        audio: None,
    };
    let mut arguments = arguments.peekable();
    while let Some(flag) = arguments.next() {
        if matches!(flag.as_str(), "--help" | "-h") {
            println!("Usage: video-creater-nle-roundtrip-evidence [--output DIR] [--video FILE] [--audio FILE]");
            std::process::exit(0);
        }
        let value = arguments
            .next()
            .ok_or_else(|| format!("{flag} requires a path"))?;
        match flag.as_str() {
            "--output" => result.output = value.into(),
            "--video" => result.video = Some(value.into()),
            "--audio" => result.audio = Some(value.into()),
            _ => return Err(format!("unknown argument: {flag}")),
        }
    }
    Ok(result)
}

fn prepare_media(
    supplied: Option<PathBuf>,
    generated: PathBuf,
    generate: fn(&Path) -> Result<(), String>,
) -> Result<PathBuf, String> {
    let path = match supplied {
        Some(path) if path.is_file() => path,
        Some(path) => return Err(format!("supplied media does not exist: {}", path.display())),
        None => {
            generate(&generated)?;
            generated
        }
    };
    path.canonicalize()
        .map_err(|error| format!("could not canonicalize {}: {error}", path.display()))
}

fn generate_video(path: &Path) -> Result<(), String> {
    run_ffmpeg(vec![
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        "testsrc2=size=640x360:rate=24:duration=8".into(),
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        "sine=frequency=440:sample_rate=48000:duration=8".into(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "ultrafast".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "aac".into(),
        "-shortest".into(),
        path.as_os_str().to_owned(),
    ])
}

fn generate_audio(path: &Path) -> Result<(), String> {
    run_ffmpeg(vec![
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        "sine=frequency=880:sample_rate=48000:duration=8".into(),
        "-c:a".into(),
        "pcm_s16le".into(),
        path.as_os_str().to_owned(),
    ])
}

fn run_ffmpeg(arguments: Vec<std::ffi::OsString>) -> Result<(), String> {
    let status = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(arguments)
        .status()
        .map_err(|error| format!("could not start ffmpeg: {error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("ffmpeg exited with {status}"))
}

fn evidence_project(video: &Path, audio: &Path) -> VideoProject {
    let mut project = VideoProject::new_empty(
        "nle-roundtrip-proof".into(),
        "Video Creater NLE Roundtrip Proof".into(),
        "2026-07-11T00:00:00Z".into(),
    );
    project.render_settings.width = 640;
    project.render_settings.height = 360;
    project.render_settings.fps = FPS;
    project.render_settings.captions = CaptionRenderMode::Off;
    project.media = vec![
        media("source-av", video, MediaKind::Video, Some((640, 360, FPS))),
        media("source-voiceover", audio, MediaKind::Audio, None),
    ];
    project.timeline.duration_seconds = 5.0;
    project.timeline.tracks = vec![
        TimelineTrack {
            transitions: Vec::new(),
            id: "video-1".into(),
            name: "Video 1".into(),
            kind: TrackKind::Video,
            locked: false,
            sync_locked: true,
            enabled: true,
            items: vec![
                clip(
                    "cut-a",
                    TimelineItemKind::VideoClip,
                    "source-av",
                    0.0,
                    2.0,
                    1.0,
                    3.0,
                ),
                clip(
                    "cut-b",
                    TimelineItemKind::VideoClip,
                    "source-av",
                    2.0,
                    3.0,
                    4.0,
                    7.0,
                ),
            ],
        },
        TimelineTrack {
            transitions: Vec::new(),
            id: "audio-1".into(),
            name: "Audio 1".into(),
            kind: TrackKind::Audio,
            locked: false,
            sync_locked: true,
            enabled: true,
            items: vec![clip(
                "voiceover-cut",
                TimelineItemKind::AudioClip,
                "source-voiceover",
                0.5,
                4.0,
                1.5,
                5.5,
            )],
        },
    ];
    project.timelines[0].timeline = project.timeline.clone();
    project
}

fn media(
    id: &str,
    path: &Path,
    kind: MediaKind,
    dimensions: Option<(u32, u32, f64)>,
) -> MediaAsset {
    MediaAsset {
        id: id.into(),
        name: Some(id.into()),
        relative_path: path.to_string_lossy().into_owned(),
        kind,
        duration_seconds: SOURCE_DURATION,
        width: dimensions.map(|value| value.0),
        height: dimensions.map(|value| value.1),
        fps: dimensions.map(|value| value.2),
        folder_id: None,
    }
}

fn clip(
    id: &str,
    kind: TimelineItemKind,
    media_id: &str,
    start: f64,
    duration: f64,
    source_in: f64,
    source_out: f64,
) -> TimelineItem {
    TimelineItem {
        id: id.into(),
        kind,
        start_seconds: start,
        duration_seconds: duration,
        source: TimelineSource::Media {
            media_id: media_id.into(),
        },
        label: id.into(),
        properties: BTreeMap::from([
            ("sourceIn".into(), json!(source_in)),
            ("sourceOut".into(), json!(source_out)),
        ]),
    }
}

fn expected_clips() -> Vec<ExpectedClip> {
    vec![
        ExpectedClip {
            id: "cut-a",
            track_kind: "video",
            media_id: "source-av",
            timeline_in_seconds: 0.0,
            timeline_out_seconds: 2.0,
            source_in_seconds: 1.0,
            source_out_seconds: 3.0,
        },
        ExpectedClip {
            id: "cut-b",
            track_kind: "video",
            media_id: "source-av",
            timeline_in_seconds: 2.0,
            timeline_out_seconds: 5.0,
            source_in_seconds: 4.0,
            source_out_seconds: 7.0,
        },
        ExpectedClip {
            id: "voiceover-cut",
            track_kind: "audio",
            media_id: "source-voiceover",
            timeline_in_seconds: 0.5,
            timeline_out_seconds: 4.5,
            source_in_seconds: 1.5,
            source_out_seconds: 5.5,
        },
    ]
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("could not serialize {}: {error}", path.display()))?;
    bytes.push(b'\n');
    fs::write(path, bytes).map_err(|error| format!("could not write {}: {error}", path.display()))
}
