use serde_json::json;
use std::fs;
use video_creater_lib::edit::render_plan::RenderQualityProfile;
use video_creater_lib::frame_compositor::{composite_rgba8_srgb, BlendMode};
use video_creater_lib::precompose::{
    decode_video_frames_rgba, package_png_frames_as_mov, prepare_project_for_render,
    FrameDecodeSpec, PngSequenceSpec,
};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{MediaAsset, MediaKind, TimelineSource};
use video_creater_lib::render_pipeline::gstreamer_backend::GstreamerGesRenderBackend;
use video_creater_lib::render_pipeline::process::SystemProcessRunner;
use video_creater_lib::render_pipeline::project_export::build_project_webm_render_plan;
use video_creater_lib::render_runtime::start_render_process_runtime;

#[test]
fn expression_lottie_prepares_once_then_reuses_verified_cache() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let temporary = tempfile::tempdir().expect("temporary project");
    let media_dir = temporary.path().join("media");
    fs::create_dir(&media_dir).expect("media directory");
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/time-seeded-random.json");
    fs::copy(&fixture, media_dir.join("motion.json")).expect("copy Lottie fixture");

    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "lottie-1".to_string(),
        name: Some("Expression motion".to_string()),
        relative_path: "media/motion.json".to_string(),
        kind: MediaKind::Lottie,
        duration_seconds: 2.0,
        width: Some(64),
        height: Some(64),
        fps: Some(2.0),
        folder_id: None,
    });
    let item = &mut project.timeline.tracks[0].items[0];
    item.source = TimelineSource::Media {
        media_id: "lottie-1".to_string(),
    };
    item.duration_seconds = 1.5;
    item.properties.insert("sourceIn".to_string(), json!(0.25));
    item.properties.insert("sourceOut".to_string(), json!(1.75));
    item.properties.insert("looping".to_string(), json!(false));

    let worker = std::path::PathBuf::from(env!("CARGO_BIN_EXE_video-creater-precompose-worker"));
    // This integration test is the only test in its process that resolves the
    // worker override, so changing the process environment is contained.
    unsafe { std::env::set_var("VIDEO_CREATER_PRECOMPOSE_WORKER", &worker) };
    let first = prepare_project_for_render(temporary.path(), &project).expect("first preparation");
    let second =
        prepare_project_for_render(temporary.path(), &project).expect("cached preparation");
    unsafe { std::env::remove_var("VIDEO_CREATER_PRECOMPOSE_WORKER") };

    assert_eq!(first.reports.len(), 1);
    assert!(!first.reports[0].cache_hit);
    assert!(second.reports[0].cache_hit);
    assert_eq!(first.reports[0].fingerprint, second.reports[0].fingerprint);
    assert!(temporary
        .path()
        .join(&first.reports[0].intermediate)
        .is_file());
    assert!(temporary
        .path()
        .join(&first.reports[0].worker_manifest)
        .is_file());
    assert_eq!(
        project.timeline.tracks[0].items[0].source,
        TimelineSource::Media {
            media_id: "lottie-1".to_string()
        },
        "canonical project must remain unchanged"
    );

    let plan = build_project_webm_render_plan(
        temporary.path(),
        &first.project,
        "prepared-plan",
        RenderQualityProfile::DraftWebm,
    )
    .expect("prepared Lottie should be an ordinary GES source");
    assert_eq!(plan.clips.len(), 1);
    assert!(plan.clips[0]
        .source_path
        .as_deref()
        .is_some_and(|path| path.ends_with("intermediate.mov")));
    assert_eq!(plan.clips[0].source_in, 0.0);
    assert_eq!(plan.clips[0].source_out, 1.5);
}

#[test]
fn cube_lut_prepares_video_pixels_then_reuses_cache() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let temporary = tempfile::tempdir().expect("temporary project");
    let media_dir = temporary.path().join("media");
    let frames_dir = temporary.path().join("source-frames");
    let looks_dir = temporary.path().join("looks");
    fs::create_dir(&media_dir).expect("media directory");
    fs::create_dir(&frames_dir).expect("frame directory");
    fs::create_dir(&looks_dir).expect("looks directory");
    for index in 0..2 {
        image::RgbaImage::from_pixel(32, 32, image::Rgba([64, 128, 192, 77]))
            .save(frames_dir.join(format!("frame-{index:06}.png")))
            .expect("source PNG");
    }
    package_png_frames_as_mov(
        &frames_dir,
        2,
        PngSequenceSpec {
            width: 32,
            height: 32,
            fps_numerator: 2,
            fps_denominator: 1,
        },
        &media_dir.join("source.mov"),
        std::time::Duration::from_secs(10),
    )
    .expect("source MOV");
    fs::write(
        looks_dir.join("invert.cube"),
        "LUT_3D_SIZE 2\n1 1 1\n0 1 1\n1 0 1\n0 0 1\n1 1 0\n0 1 0\n1 0 0\n0 0 0\n",
    )
    .expect("invert LUT");

    let mut project = sample_project();
    project.render_settings.width = 32;
    project.render_settings.height = 32;
    project.render_settings.fps = 2.0;
    project.media[0] = MediaAsset {
        id: "video-lut".to_string(),
        name: Some("LUT source".to_string()),
        relative_path: "media/source.mov".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 1.0,
        width: Some(32),
        height: Some(32),
        fps: Some(2.0),
        folder_id: None,
    };
    let item = &mut project.timeline.tracks[0].items[0];
    item.source = TimelineSource::Media {
        media_id: "video-lut".to_string(),
    };
    item.duration_seconds = 1.0;
    item.properties.insert("sourceIn".to_string(), json!(0.0));
    item.properties.insert("sourceOut".to_string(), json!(1.0));
    item.properties.insert(
        "colorGrade".to_string(),
        json!({"lut": {"path": "looks/invert.cube", "strength": 1.0}}),
    );

    let first = prepare_project_for_render(temporary.path(), &project).expect("first LUT bake");
    let second = prepare_project_for_render(temporary.path(), &project).expect("cached LUT bake");
    assert_eq!(first.reports.len(), 1);
    assert_eq!(first.reports[0].stage, "lut");
    assert!(!first.reports[0].cache_hit);
    assert!(second.reports[0].cache_hit);
    assert_eq!(first.reports[0].fingerprint, second.reports[0].fingerprint);
    let prepared_item = &first.project.timeline.tracks[0].items[0];
    assert!(!prepared_item.properties.contains_key("colorGrade"));
    assert_eq!(
        project.timeline.tracks[0].items[0].properties["colorGrade"]["lut"]["path"],
        json!("looks/invert.cube"),
        "canonical project must remain unchanged"
    );
    fs::write(
        temporary.path().join(&first.reports[0].intermediate),
        b"corrupt intermediate",
    )
    .expect("corrupt cached intermediate");
    let rebuilt =
        prepare_project_for_render(temporary.path(), &project).expect("rebuild corrupt LUT cache");
    assert!(!rebuilt.reports[0].cache_hit);

    let decoded = decode_first_rgba_frame(
        &temporary.path().join(&first.reports[0].intermediate),
        32,
        0.0,
        "decode LUT intermediate",
    );
    assert_eq!(&decoded[..4], &[191, 127, 63, 77]);

    let plan = build_project_webm_render_plan(
        temporary.path(),
        &rebuilt.project,
        "lut-ges",
        RenderQualityProfile::FinalWebm,
    )
    .expect("prepared LUT GES plan");
    fs::create_dir_all(
        std::path::Path::new(&plan.output_path)
            .parent()
            .expect("output parent"),
    )
    .expect("render directory");
    GstreamerGesRenderBackend::new()
        .render_cancellable(
            &SystemProcessRunner,
            &plan,
            &[],
            std::time::Duration::from_secs(20),
            None,
        )
        .expect("render prepared LUT through GES");
    let final_frame = decode_first_rgba_frame(
        std::path::Path::new(&plan.output_path),
        32,
        0.25,
        "decode LUT GES output",
    );
    // Final WebM is opaque, so the prepared alpha-77 LUT pixel is composited over black.
    for (actual, expected) in final_frame[..3].iter().zip([58_i16, 38, 19]) {
        assert!(
            (i16::from(*actual) - expected).abs() <= 12,
            "actual RGB={:?}",
            &final_frame[..3]
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn cube_lut_decodes_h264_with_reviewed_videotoolbox_path() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let temporary = tempfile::tempdir().expect("temporary project");
    let media_dir = temporary.path().join("media");
    let looks_dir = temporary.path().join("looks");
    fs::create_dir(&media_dir).expect("media directory");
    fs::create_dir(&looks_dir).expect("looks directory");
    let source = media_dir.join("source.mp4");
    let generated = std::process::Command::new("gst-launch-1.0")
        .args([
            "-q",
            "videotestsrc",
            "num-buffers=2",
            "pattern=red",
            "!",
            "video/x-raw,width=32,height=32,framerate=2/1",
            "!",
            "videoconvert",
            "!",
            "vtenc_h264",
            "allow-frame-reordering=false",
            "!",
            "h264parse",
            "!",
            "qtmux",
            "!",
            "filesink",
        ])
        .arg(format!("location={}", source.display()))
        .output()
        .expect("run GStreamer fixture generator");
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    fs::write(
        looks_dir.join("identity.cube"),
        "LUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n",
    )
    .expect("identity LUT");

    let mut project = sample_project();
    project.render_settings.width = 32;
    project.render_settings.height = 32;
    project.render_settings.fps = 2.0;
    project.media[0] = MediaAsset {
        id: "h264-lut".to_string(),
        name: Some("H264 LUT source".to_string()),
        relative_path: "media/source.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 1.0,
        width: Some(32),
        height: Some(32),
        fps: Some(2.0),
        folder_id: None,
    };
    let item = &mut project.timeline.tracks[0].items[0];
    item.source = TimelineSource::Media {
        media_id: "h264-lut".to_string(),
    };
    item.duration_seconds = 1.0;
    item.properties.insert("sourceIn".to_string(), json!(0.0));
    item.properties.insert("sourceOut".to_string(), json!(1.0));
    item.properties.insert(
        "colorGrade".to_string(),
        json!({"lut": {"path": "looks/identity.cube", "strength": 1.0}}),
    );

    let prepared = prepare_project_for_render(temporary.path(), &project)
        .expect("reviewed H264 decoder should prepare LUT frames");
    assert_eq!(prepared.reports.len(), 1);
    assert!(!prepared.reports[0].cache_hit);
    assert!(temporary
        .path()
        .join(&prepared.reports[0].intermediate)
        .is_file());
}

#[test]
fn multiply_blend_flattens_the_minimal_dependent_stack_and_reuses_cache() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let temporary = tempfile::tempdir().expect("temporary project");
    let media_dir = temporary.path().join("media");
    fs::create_dir(&media_dir).expect("media directory");
    for (name, color) in [
        ("bottom", [200, 100, 50, 255]),
        ("top", [255, 255, 255, 255]),
    ] {
        let frames = temporary.path().join(format!("{name}-frames"));
        fs::create_dir(&frames).expect("frame directory");
        for index in 0..4 {
            image::RgbaImage::from_pixel(64, 64, image::Rgba(color))
                .save(frames.join(format!("frame-{index:06}.png")))
                .expect("source PNG");
        }
        package_png_frames_as_mov(
            &frames,
            4,
            PngSequenceSpec {
                width: 64,
                height: 64,
                fps_numerator: 4,
                fps_denominator: 1,
            },
            &media_dir.join(format!("{name}.mov")),
            std::time::Duration::from_secs(10),
        )
        .expect("source MOV");
    }

    let mut project = sample_project();
    project.render_settings.width = 64;
    project.render_settings.height = 64;
    project.render_settings.fps = 4.0;
    project.timeline.duration_seconds = 1.0;
    project.media = ["bottom", "top"]
        .into_iter()
        .map(|id| MediaAsset {
            id: id.to_string(),
            name: Some(id.to_string()),
            relative_path: format!("media/{id}.mov"),
            kind: MediaKind::Video,
            duration_seconds: 1.0,
            width: Some(64),
            height: Some(64),
            fps: Some(4.0),
            folder_id: None,
        })
        .collect();
    let mut bottom_track = project.timeline.tracks[0].clone();
    bottom_track.id = "bottom-track".to_string();
    bottom_track.items[0].id = "bottom-item".to_string();
    bottom_track.items[0].source = TimelineSource::Media {
        media_id: "bottom".to_string(),
    };
    bottom_track.items[0].start_seconds = 0.0;
    bottom_track.items[0].duration_seconds = 1.0;
    bottom_track.items[0].properties = std::collections::BTreeMap::from([
        ("sourceIn".to_string(), json!(0.0)),
        ("sourceOut".to_string(), json!(1.0)),
    ]);
    let mut top_track = bottom_track.clone();
    top_track.id = "top-track".to_string();
    top_track.items[0].id = "top-item".to_string();
    top_track.items[0].source = TimelineSource::Media {
        media_id: "top".to_string(),
    };
    top_track.items[0]
        .properties
        .insert("blendMode".to_string(), json!("multiply"));
    project.timeline.tracks = vec![bottom_track, top_track];

    let first = prepare_project_for_render(temporary.path(), &project).expect("first flatten");
    let second = prepare_project_for_render(temporary.path(), &project).expect("cached flatten");
    assert_eq!(first.reports.len(), 1);
    assert_eq!(first.reports[0].stage, "flattenedComposite");
    assert!(!first.reports[0].cache_hit);
    assert!(second.reports[0].cache_hit);
    assert_eq!(project.timeline.tracks[1].items[0].id, "top-item");
    assert_eq!(first.project.timeline.tracks.len(), 1);
    assert_eq!(
        first.project.timeline.tracks[0].items[0].properties["blendMode"],
        json!("over")
    );
    assert_eq!(first.project.timeline.tracks[0].items.len(), 1);

    let decoded = decode_first_rgba_frame(
        &temporary.path().join(&first.reports[0].intermediate),
        64,
        0.0,
        "decode flattened intermediate",
    );
    assert_eq!(&decoded[..4], &[200, 100, 50, 255]);

    let plan = build_project_webm_render_plan(
        temporary.path(),
        &first.project,
        "flattened-ges",
        RenderQualityProfile::FinalWebm,
    )
    .expect("flattened GES plan");
    assert_eq!(plan.clips.len(), 1, "plan={plan:?}");
    assert!(plan.clips[0]
        .source_path
        .as_deref()
        .is_some_and(|path| path.ends_with("intermediate.mov")));
    fs::create_dir_all(
        std::path::Path::new(&plan.output_path)
            .parent()
            .expect("output parent"),
    )
    .expect("render directory");
    GstreamerGesRenderBackend::new()
        .render_cancellable(
            &SystemProcessRunner,
            &plan,
            &[],
            std::time::Duration::from_secs(20),
            None,
        )
        .expect("render flattened intermediate through GES");
    let final_frame = decode_first_rgba_frame(
        std::path::Path::new(&plan.output_path),
        64,
        0.25,
        "decode flattened GES output",
    );
    let actual_rgb = &final_frame[..3];
    for (actual, expected) in actual_rgb.iter().zip([200_i16, 100, 50]) {
        assert!(
            (i16::from(*actual) - expected).abs() <= 12,
            "actual RGB={actual_rgb:?}"
        );
    }
}

#[test]
fn every_precomposed_blend_class_has_a_real_multilayer_pixel_fixture() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let temporary = tempfile::tempdir().expect("temporary project");
    let media_dir = temporary.path().join("media");
    fs::create_dir(&media_dir).expect("media directory");
    let bottom = [100, 80, 60, 255];
    let top = [180, 100, 220, 128];
    for (name, color) in [("bottom", bottom), ("top", top)] {
        let frames = temporary.path().join(format!("{name}-frames"));
        fs::create_dir(&frames).expect("frame directory");
        for index in 0..2 {
            image::RgbaImage::from_pixel(32, 32, image::Rgba(color))
                .save(frames.join(format!("frame-{index:06}.png")))
                .expect("source PNG");
        }
        package_png_frames_as_mov(
            &frames,
            2,
            PngSequenceSpec {
                width: 32,
                height: 32,
                fps_numerator: 2,
                fps_denominator: 1,
            },
            &media_dir.join(format!("{name}.mov")),
            std::time::Duration::from_secs(10),
        )
        .expect("source MOV");
    }

    let mut base = sample_project();
    base.render_settings.width = 32;
    base.render_settings.height = 32;
    base.render_settings.fps = 2.0;
    base.timeline.duration_seconds = 1.0;
    base.media = ["bottom", "top"]
        .into_iter()
        .map(|id| MediaAsset {
            id: id.to_string(),
            name: Some(id.to_string()),
            relative_path: format!("media/{id}.mov"),
            kind: MediaKind::Video,
            duration_seconds: 1.0,
            width: Some(32),
            height: Some(32),
            fps: Some(2.0),
            folder_id: None,
        })
        .collect();
    let mut bottom_track = base.timeline.tracks[0].clone();
    bottom_track.id = "bottom-track".to_string();
    bottom_track.items[0].id = "bottom-item".to_string();
    bottom_track.items[0].source = TimelineSource::Media {
        media_id: "bottom".to_string(),
    };
    bottom_track.items[0].start_seconds = 0.0;
    bottom_track.items[0].duration_seconds = 1.0;
    bottom_track.items[0].properties = std::collections::BTreeMap::from([
        ("sourceIn".to_string(), json!(0.0)),
        ("sourceOut".to_string(), json!(1.0)),
    ]);
    let mut top_track = bottom_track.clone();
    top_track.id = "top-track".to_string();
    top_track.items[0].id = "top-item".to_string();
    top_track.items[0].source = TimelineSource::Media {
        media_id: "top".to_string(),
    };
    base.timeline.tracks = vec![bottom_track, top_track];

    for (label, mode) in [
        ("source", BlendMode::Source),
        ("add", BlendMode::Add),
        ("screen", BlendMode::Screen),
        ("overlay", BlendMode::Overlay),
    ] {
        let mut project = base.clone();
        project.timeline.tracks[1].items[0]
            .properties
            .insert("blendMode".to_string(), json!(label));
        let prepared = prepare_project_for_render(temporary.path(), &project)
            .unwrap_or_else(|errors| panic!("{label} preparation failed: {errors:?}"));
        assert_eq!(prepared.reports.len(), 1, "mode={label}");
        assert_eq!(prepared.reports[0].stage, "flattenedComposite");
        let decoded = decode_first_rgba_frame(
            &temporary.path().join(&prepared.reports[0].intermediate),
            32,
            0.0,
            "decode flattened intermediate",
        );
        let expected = composite_rgba8_srgb(bottom, top, mode);
        assert_eq!(&decoded[..4], &expected, "mode={label}");
    }

    let mut animated = base;
    animated.timeline.tracks[1].items[0]
        .properties
        .extend(std::collections::BTreeMap::from([
            ("blendMode".to_string(), json!("multiply")),
            (
                "transform".to_string(),
                json!({
                    "centerX": 0.5,
                    "centerY": 0.5,
                    "width": 0.75,
                    "height": 0.75,
                    "flipHorizontal": true
                }),
            ),
            ("fadeInSeconds".to_string(), json!(0.25)),
            ("fadeOutSeconds".to_string(), json!(0.25)),
            (
                "keyframes".to_string(),
                json!({
                    "positionX": [
                        {"atSeconds": 0.0, "value": -2.0, "easing": "easeInOut"},
                        {"atSeconds": 1.0, "value": 2.0, "easing": "linear"}
                    ],
                    "scale": [
                        {"atSeconds": 0.0, "value": 0.8, "easing": "easeOut"},
                        {"atSeconds": 1.0, "value": 1.0, "easing": "linear"}
                    ],
                    "rotationDegrees": [
                        {"atSeconds": 0.0, "value": 0.0, "easing": "linear"},
                        {"atSeconds": 1.0, "value": 45.0, "easing": "linear"}
                    ],
                    "cropLeft": [
                        {"atSeconds": 0.0, "value": 0.0, "easing": "linear"},
                        {"atSeconds": 1.0, "value": 0.2, "easing": "linear"}
                    ],
                    "opacity": [
                        {"atSeconds": 0.0, "value": 0.5, "easing": "linear"},
                        {"atSeconds": 1.0, "value": 1.0, "easing": "linear"}
                    ]
                }),
            ),
        ]));
    let animated_prepared = prepare_project_for_render(temporary.path(), &animated)
        .expect("animated richer blend preparation");
    let animated_cached = prepare_project_for_render(temporary.path(), &animated)
        .expect("animated richer blend cache hit");
    assert!(!animated_prepared.reports[0].cache_hit);
    assert!(animated_cached.reports[0].cache_hit);
    let compositor_backend = animated_prepared.reports[0]
        .compositor_backend
        .as_deref()
        .expect("flattened composite backend report");
    assert!(
        matches!(
            compositor_backend,
            "wgpu-verified-v1" | "cpu-v1" | "cpu-fallback-v1"
        ),
        "unexpected compositor backend: {compositor_backend}"
    );
    if compositor_backend == "wgpu-verified-v1" {
        assert!(animated_prepared.reports[0].compositor_fallback.is_none());
    } else {
        assert!(
            animated_prepared.reports[0].compositor_fallback.is_some(),
            "GPU-preferred builds must explain a CPU fallback"
        );
    }
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(
            temporary
                .path()
                .join(&animated_prepared.reports[0].worker_manifest),
        )
        .expect("animated frame manifest"),
    )
    .expect("animated frame manifest JSON");
    let hashes = manifest["frameRgbaSha256"]
        .as_array()
        .expect("animated RGBA hashes");
    assert_eq!(manifest["blendBackend"]["backend"], compositor_backend);
    assert_eq!(
        manifest["blendBackend"]["fallbackReason"].is_null(),
        compositor_backend == "wgpu-verified-v1"
    );
    assert_ne!(
        hashes[0], hashes[1],
        "animated frame program must change pixels"
    );
}

/// Decodes the top-left RGBA pixels of one frame through the curated render
/// runtime, so pixel assertions do not depend on a host FFmpeg install.
fn decode_first_rgba_frame(
    path: &std::path::Path,
    size: u32,
    at_seconds: f64,
    context: &str,
) -> Vec<u8> {
    let start_micros = (at_seconds * 1_000_000.0).round() as u64;
    let mut decoded = None;
    decode_video_frames_rgba(
        path,
        FrameDecodeSpec {
            width: size,
            height: size,
            fps_numerator: 2,
            fps_denominator: 1,
            source_start_micros: start_micros,
            source_stop_micros: start_micros + 500_000,
            playback_rate_micros: 1_000_000,
            frame_count: 1,
        },
        std::time::Duration::from_secs(20),
        |_, frame| {
            decoded = Some(frame.to_vec());
            Ok(())
        },
    )
    .unwrap_or_else(|error| panic!("{context}: {error:?}"));
    decoded.unwrap_or_else(|| panic!("{context}: no decoded frame"))
}
