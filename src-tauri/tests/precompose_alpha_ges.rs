use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::time::Duration;
use video_creater_lib::edit::render_plan::RenderQualityProfile;
use video_creater_lib::precompose::{package_png_frames_as_mov, PngSequenceSpec};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    MediaAsset, MediaKind, TimelineItem, TimelineItemKind, TimelineSource, TimelineTrack, TrackKind,
};
use video_creater_lib::render_pipeline::gstreamer_backend::GstreamerGesRenderBackend;
use video_creater_lib::render_pipeline::process::SystemProcessRunner;
use video_creater_lib::render_pipeline::project_export::build_project_webm_render_plan;
use video_creater_lib::render_runtime::start_render_process_runtime;

#[cfg(target_os = "macos")]
fn main() {
    if std::env::var_os("VIDEO_CREATER_HEADLESS_RUST_SUITE").is_some() {
        println!("precompose_alpha_ges: deferred to dedicated native lane");
        return;
    }
    let exit_code = gstreamer::macos_main(|| {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run_fixture)) {
            Ok(()) => {
                println!("precompose_alpha_ges: passed");
                0
            }
            Err(_) => {
                eprintln!("precompose_alpha_ges: failed");
                1
            }
        }
    });
    std::process::exit(exit_code);
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("precompose_alpha_ges: skipped outside macOS");
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn run_fixture() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let temporary = tempfile::tempdir().expect("temporary project");
    let media_dir = temporary.path().join("media");
    let background_frames = temporary.path().join("background-frames");
    let overlay_frames = temporary.path().join("overlay-frames");
    let top_frames = temporary.path().join("top-frames");
    fs::create_dir(&media_dir).expect("media directory");
    fs::create_dir(&background_frames).expect("background frames");
    fs::create_dir(&overlay_frames).expect("overlay frames");
    fs::create_dir(&top_frames).expect("top frames");
    let alpha_values = [64_u8, 96, 160, 224];
    for (index, alpha) in alpha_values.into_iter().enumerate() {
        image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 32, 128, 255]))
            .save(background_frames.join(format!("frame-{index:06}.png")))
            .expect("background PNG");
        let mut overlay = image::RgbaImage::from_pixel(64, 64, image::Rgba([255, 0, 255, 0]));
        for y in 16..48 {
            for x in 16..48 {
                overlay.put_pixel(x, y, image::Rgba([255, 255, 255, alpha]));
            }
        }
        overlay
            .save(overlay_frames.join(format!("frame-{index:06}.png")))
            .expect("overlay PNG");
        let mut top = image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 255, 255, 0]));
        for y in 0..16 {
            for x in 0..16 {
                top.put_pixel(x, y, image::Rgba([0, 255, 0, 255]));
            }
        }
        top.save(top_frames.join(format!("frame-{index:06}.png")))
            .expect("top PNG");
    }
    package_png_frames_as_mov(
        &background_frames,
        4,
        PngSequenceSpec {
            width: 64,
            height: 64,
            fps_numerator: 4,
            fps_denominator: 1,
        },
        &media_dir.join("background.mov"),
        Duration::from_secs(10),
    )
    .expect("background MOV");
    package_png_frames_as_mov(
        &overlay_frames,
        4,
        PngSequenceSpec {
            width: 64,
            height: 64,
            fps_numerator: 4,
            fps_denominator: 1,
        },
        &media_dir.join("overlay.mov"),
        Duration::from_secs(10),
    )
    .expect("overlay MOV");
    package_png_frames_as_mov(
        &top_frames,
        4,
        PngSequenceSpec {
            width: 64,
            height: 64,
            fps_numerator: 4,
            fps_denominator: 1,
        },
        &media_dir.join("top.mov"),
        Duration::from_secs(10),
    )
    .expect("top MOV");

    let mut project = sample_project();
    project.render_settings.width = 64;
    project.render_settings.height = 64;
    project.render_settings.fps = 4.0;
    project.timeline.duration_seconds = 1.0;
    project.media = vec![
        MediaAsset {
            id: "background".to_string(),
            name: Some("Background".to_string()),
            relative_path: "media/background.mov".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 1.0,
            width: Some(64),
            height: Some(64),
            fps: Some(4.0),
            folder_id: None,
        },
        MediaAsset {
            id: "top-overlay".to_string(),
            name: Some("Top overlay".to_string()),
            relative_path: "media/top.mov".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 1.0,
            width: Some(64),
            height: Some(64),
            fps: Some(4.0),
            folder_id: None,
        },
        MediaAsset {
            id: "prepared-overlay".to_string(),
            name: Some("Prepared overlay".to_string()),
            relative_path: "media/overlay.mov".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 1.0,
            width: Some(64),
            height: Some(64),
            fps: Some(4.0),
            folder_id: None,
        },
    ];
    project.timeline.tracks = vec![
        video_track("background-track", "background", None),
        video_track("overlay-track", "prepared-overlay", Some("over")),
        video_track("top-track", "top-overlay", Some("over")),
    ];
    project.timelines.clear();
    project.active_timeline_id = None;
    let plan = build_project_webm_render_plan(
        temporary.path(),
        &project,
        "precompose-alpha-ges",
        RenderQualityProfile::FinalWebm,
    )
    .expect("build GES plan");
    let output = std::path::Path::new(&plan.output_path);
    fs::create_dir_all(output.parent().expect("output parent")).expect("render directory");
    GstreamerGesRenderBackend::new()
        .render_cancellable(
            &SystemProcessRunner,
            &plan,
            &[],
            Duration::from_secs(20),
            None,
        )
        .expect("GES alpha render");
    let decoded = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&plan.output_path)
        .args(["-frames:v", "4", "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .output()
        .expect("decode GES output");
    assert!(
        decoded.status.success(),
        "{}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    let frame_bytes = 64 * 64 * 4;
    assert_eq!(decoded.stdout.len(), frame_bytes * 4);
    for frame_index in [0_usize, 2, 3] {
        let frame = &decoded.stdout[frame_index * frame_bytes..(frame_index + 1) * frame_bytes];
        let center = region_mean(frame, 64, 24..40, 24..40);
        let outside = region_mean(frame, 64, 52..60, 52..60);
        let top = region_mean(frame, 64, 2..10, 2..10);
        let alpha = f64::from(alpha_values[frame_index]) / 255.0;
        for (channel, background) in [0.0, 32.0, 128.0].into_iter().enumerate() {
            let expected = 255.0 * alpha + background * (1.0 - alpha);
            assert!(
                (center[channel] - expected).abs() <= 20.0,
                "frame={frame_index} center={center:?} outside={outside:?}"
            );
            assert!(
                (outside[channel] - background).abs() <= 12.0,
                "frame={frame_index} center={center:?} outside={outside:?}"
            );
        }
        assert!(center[3] >= 250.0 && outside[3] >= 250.0);
        assert!(
            top[0] <= 12.0 && top[1] >= 243.0 && top[2] <= 12.0 && top[3] >= 250.0,
            "frame={frame_index} top={top:?}"
        );
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn video_track(id: &str, media_id: &str, blend_mode: Option<&str>) -> TimelineTrack {
    let mut properties = BTreeMap::from([
        ("sourceIn".to_string(), json!(0.0)),
        ("sourceOut".to_string(), json!(1.0)),
    ]);
    if let Some(blend_mode) = blend_mode {
        properties.insert("blendMode".to_string(), json!(blend_mode));
    }
    TimelineTrack {
        transitions: Vec::new(),
        id: id.to_string(),
        name: id.to_string(),
        kind: TrackKind::Video,
        locked: false,
        sync_locked: false,
        enabled: true,
        items: vec![TimelineItem {
            id: format!("{id}-item"),
            kind: TimelineItemKind::VideoClip,
            start_seconds: 0.0,
            duration_seconds: 1.0,
            source: TimelineSource::Media {
                media_id: media_id.to_string(),
            },
            label: id.to_string(),
            properties,
        }],
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn region_mean(
    rgba: &[u8],
    width: usize,
    x_range: std::ops::Range<usize>,
    y_range: std::ops::Range<usize>,
) -> [f64; 4] {
    let mut totals = [0_u64; 4];
    let mut count = 0_u64;
    for y in y_range {
        for x in x_range.clone() {
            let offset = (y * width + x) * 4;
            for channel in 0..4 {
                totals[channel] += u64::from(rgba[offset + channel]);
            }
            count += 1;
        }
    }
    totals.map(|total| total as f64 / count as f64)
}
