use super::*;
use crate::project::model::{TimelineItem, TimelineItemKind, TimelineSource, TrackKind};
use std::collections::BTreeMap;

fn template_project(shader: bool) -> VideoProject {
    let mut project = VideoProject::new_empty("p".into(), "Templates".into(), "now".into());
    project.render_settings.width = 96;
    project.render_settings.height = 64;
    project.render_settings.fps = 8.0;
    project.timeline.duration_seconds = 1.0;
    project.timeline.tracks.truncate(1);
    let track = &mut project.timeline.tracks[0];
    track.kind = if shader {
        TrackKind::HyperframeScene
    } else {
        TrackKind::Overlay
    };
    track.items = vec![TimelineItem {
        id: "visual".into(),
        kind: if shader {
            TimelineItemKind::HyperframeScene
        } else {
            TimelineItemKind::Overlay
        },
        start_seconds: 0.0,
        duration_seconds: 1.0,
        source: TimelineSource::Generated {
            artifact_id: "unbacked-template".into(),
        },
        label: "Visual".into(),
        properties: BTreeMap::from([
            (
                if shader {
                    "shaderBackgroundTemplateId"
                } else {
                    "templateId"
                }
                .into(),
                json!(if shader {
                    "shadertoy-octagrams-v1"
                } else {
                    "kinetic-lower-third-v1"
                }),
            ),
            (
                "templateFields".into(),
                json!({"headline":"Preview", "subline":"Animated"}),
            ),
            (
                "visualTreatment".into(),
                json!("deliberate shaped material"),
            ),
            ("motion".into(), json!("animated reveal and exit")),
            ("safeZone".into(), json!("10% margins")),
            ("avoid".into(), json!("static cards")),
        ]),
    }];
    project
}

fn assert_animated_template(shader: bool) {
    crate::render_runtime::start_render_process_runtime().expect("render runtime");
    let root = tempfile::tempdir().unwrap();
    let project = template_project(shader);
    let original = project.clone();
    let prepared = prepare_project_for_render(root.path(), &project).expect("prepare templates");
    assert_eq!(
        project, original,
        "preparation must not mutate canonical state"
    );
    let report = prepared
        .reports
        .first()
        .expect("template frame sequence report");
    assert!(report.has_frame_sequence());
    let track = &prepared.project.timeline.tracks[0];
    assert_eq!(track.kind, TrackKind::Video);
    let item = &track.items[0];
    assert_eq!(item.kind, TimelineItemKind::VideoClip);
    assert!(matches!(item.source, TimelineSource::Media { .. }));
    let directory = root
        .path()
        .join(&report.intermediate)
        .parent()
        .unwrap()
        .to_path_buf();
    let first = image::open(directory.join("frames/frame-000000.png"))
        .unwrap()
        .into_rgba8();
    let middle = image::open(directory.join("frames/frame-000004.png"))
        .unwrap()
        .into_rgba8();
    assert_ne!(
        first.as_raw(),
        middle.as_raw(),
        "template frames must animate"
    );
    assert_eq!(first.dimensions(), (96, 64));
    let graphics = crate::render_pipeline::project_export::build_project_graphics_layers(
        &prepared.project,
        96,
        64,
        8.0,
    )
    .expect("prepared template export layers");
    assert!(
        graphics.is_empty(),
        "prepared templates must not be expanded twice"
    );
    let plan = crate::render_pipeline::project_export::build_project_webm_render_plan(
        root.path(),
        &prepared.project,
        "template-export",
        crate::edit::render_plan::RenderQualityProfile::FinalWebm,
    )
    .expect("media-backed template export plan");
    assert_eq!(plan.clips.len(), 1);
    assert!(plan.clips[0]
        .source_path
        .as_deref()
        .unwrap_or(&plan.input_path)
        .ends_with("intermediate.mov"));
    let again = prepare_project_for_render(root.path(), &project).expect("cached templates");
    assert!(again.reports[0].cache_hit);
    assert_eq!(again.reports[0].fingerprint, report.fingerprint);
    std::fs::write(directory.join("frames/frame-000004.png"), b"damaged frame").unwrap();
    let rebuilt = prepare_project_for_render(root.path(), &project).expect("rebuild damaged cache");
    assert!(!rebuilt.reports[0].cache_hit);
    assert!(image::open(directory.join("frames/frame-000004.png")).is_ok());
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "PNG-in-QuickTime packaging requires the AppKit-hosted GStreamer test path on macOS"
)]
fn shader_background_prepares_real_animated_cached_frames() {
    assert_animated_template(true);
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "PNG-in-QuickTime packaging requires the AppKit-hosted GStreamer test path on macOS"
)]
fn motion_template_prepares_same_animated_renderer_as_export() {
    assert_animated_template(false);
}

#[test]
fn cancelled_shader_preparation_writes_no_cache() {
    let root = tempfile::tempdir().unwrap();
    let project = template_project(true);
    let key = crate::render_pipeline::cancel::RenderAttemptKey::new(
        root.path().to_path_buf(),
        "p",
        "j",
        "a",
    )
    .unwrap();
    let guard = crate::render_pipeline::cancel::register_render_attempt(key.clone()).unwrap();
    let cancelled = guard.token();
    crate::render_pipeline::cancel::request_render_cancellation(&key);
    assert!(
        prepare_project_for_render_cancellable(root.path(), &project, Some(&cancelled)).is_err()
    );
    assert!(!root.path().join("cache").exists());
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "PNG-in-QuickTime packaging requires the AppKit-hosted GStreamer test path on macOS"
)]
fn prepared_templates_remain_above_primary_media_in_export_order() {
    crate::render_runtime::start_render_process_runtime().unwrap();
    let root = tempfile::tempdir().unwrap();
    image::RgbaImage::from_pixel(96, 64, image::Rgba([255, 0, 0, 255]))
        .save(root.path().join("primary.png"))
        .unwrap();
    let mut project = template_project(true);
    project.media.push(MediaAsset {
        id: "primary".into(),
        name: None,
        relative_path: "primary.png".into(),
        kind: MediaKind::Image,
        duration_seconds: 1.0,
        width: Some(96),
        height: Some(64),
        fps: None,
        folder_id: None,
    });
    let mut primary = project.timeline.tracks[0].clone();
    primary.id = "primary-track".into();
    primary.kind = TrackKind::Video;
    primary.items[0].id = "primary-item".into();
    primary.items[0].kind = TimelineItemKind::ImageClip;
    primary.items[0].source = TimelineSource::Media {
        media_id: "primary".into(),
    };
    primary.items[0].properties.clear();
    // The editor stores graphics tracks before video tracks when dropped above them.
    project.timeline.tracks.push(primary);
    let prepared = prepare_project_for_render(root.path(), &project).unwrap();
    assert_eq!(prepared.project.timeline.tracks[0].id, "primary-track");
    assert_eq!(prepared.project.timeline.tracks[1].items[0].id, "visual");
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "PNG-in-QuickTime packaging requires the AppKit-hosted GStreamer test path on macOS"
)]
fn every_builtin_shader_and_motion_template_has_animated_prepared_frames() {
    crate::render_runtime::start_render_process_runtime().unwrap();
    let root = tempfile::tempdir().unwrap();
    let shaders = crate::gpu_graphics::templates::builtin_shader_background_templates().unwrap();
    let motion_ids = [
        "kinetic-lower-third-v1",
        "punchy-caption-v1",
        "metric-callout-v1",
        "chapter-card-v1",
        "tracking-highlight-v1",
        "holographic-logo-cutout-v1",
        "gradient-background-loop-v1",
    ];
    for (shader, id) in shaders
        .iter()
        .map(|template| (true, template.config.id.as_str()))
        .chain(motion_ids.iter().map(|id| (false, *id)))
    {
        let mut project = template_project(shader);
        let item = &mut project.timeline.tracks[0].items[0];
        item.properties.insert(
            if shader {
                "shaderBackgroundTemplateId"
            } else {
                "templateId"
            }
            .into(),
            json!(id),
        );
        item.properties.insert(
            "templateFields".into(),
            json!({"headline":"98", "subline":"Animated", "logoAssetId":"builtin:v-photo-light"}),
        );
        let prepared = prepare_project_for_render(root.path(), &project)
            .unwrap_or_else(|error| panic!("{id}: {error:?}"));
        let report = &prepared.reports[0];
        let directory = root
            .path()
            .join(&report.intermediate)
            .parent()
            .unwrap()
            .to_path_buf();
        let first = image::open(directory.join("frames/frame-000000.png"))
            .unwrap()
            .into_rgba8();
        // Looping templates can repeat their first frame at the midpoint.
        let changes = (1..8).any(|index| {
            let frame = image::open(directory.join(format!("frames/frame-{index:06}.png")))
                .unwrap()
                .into_rgba8();
            first.as_raw() != frame.as_raw()
        });
        assert!(changes, "{id} must animate");
    }
}

#[test]
fn shader_templates_fit_landscape_and_portrait_raster_budget() {
    let root = tempfile::tempdir().unwrap();
    for (width, height, expected) in [(3840, 2160, (1920, 1080)), (1080, 1920, (1080, 1920))] {
        let mut project = template_project(true);
        project.render_settings.width = width;
        project.render_settings.height = height;
        let layer = super::graphics_template::template_layer(
            root.path(),
            &project,
            &project.timeline.tracks[0].items[0],
            true,
        )
        .unwrap();
        let super::graphics_template::TemplateLayer::Shader(layer) = layer else {
            panic!("shader")
        };
        assert_eq!((layer.dimensions.width, layer.dimensions.height), expected);
        crate::gpu_graphics::validation::validate_gpu_graphics_layer(&layer)
            .expect("supported working raster");
    }
}

#[test]
#[cfg_attr(target_os = "macos", ignore = "requires AppKit-hosted GStreamer")]
fn high_resolution_shader_projects_publish_valid_scaled_animation() {
    crate::render_runtime::start_render_process_runtime().unwrap();
    let root = tempfile::tempdir().unwrap();
    for (width, height, expected) in [(3840, 2160, (1920, 1080)), (1080, 1920, (1080, 1920))] {
        let mut project = template_project(true);
        project.render_settings.width = width;
        project.render_settings.height = height;
        project.render_settings.fps = 2.0;
        let prepared = prepare_project_for_render(root.path(), &project).unwrap();
        let report = &prepared.reports[0];
        let frames = root
            .path()
            .join(&report.intermediate)
            .parent()
            .unwrap()
            .join("frames");
        let first = image::open(frames.join("frame-000000.png"))
            .unwrap()
            .into_rgba8();
        let last = image::open(frames.join("frame-000001.png"))
            .unwrap()
            .into_rgba8();
        assert_eq!(first.dimensions(), expected);
        assert_ne!(first.as_raw(), last.as_raw());
        assert_eq!(
            (
                project.render_settings.width,
                project.render_settings.height
            ),
            (width, height)
        );
        let plan = crate::render_pipeline::project_export::build_project_webm_render_plan(
            root.path(),
            &prepared.project,
            "scaled-shader",
            crate::edit::render_plan::RenderQualityProfile::FinalWebm,
        )
        .unwrap();
        assert_eq!((plan.width, plan.height), (width, height));
    }
}

#[test]
fn shader_renderer_cancels_between_frames_and_removes_partial_artifacts() {
    let root = tempfile::tempdir().unwrap();
    let project = template_project(true);
    let layer = super::graphics_template::template_layer(
        root.path(),
        &project,
        &project.timeline.tracks[0].items[0],
        true,
    )
    .unwrap();
    let super::graphics_template::TemplateLayer::Shader(layer) = layer else {
        panic!("shader")
    };
    let checks = std::cell::Cell::new(0);
    let output = root.path().join("cancelled");
    let result = crate::gpu_graphics::renderer::render_gpu_graphics_layer_cancellable(
        &layer,
        crate::gpu_graphics::renderer::GpuRenderOptions {
            output_dir: output.clone(),
        },
        || {
            checks.set(checks.get() + 1);
            checks.get() >= 4
        },
    );
    let errors = result.expect_err("GPU frame loop must observe cancellation");
    assert!(errors.iter().any(|error| error.path == "cancelled"));
    assert!(!output.exists(), "cancelled frame cache is removed");
}
