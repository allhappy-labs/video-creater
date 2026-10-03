//! Resolve timeline template references through the same Rust renderers used by export.
//! Only the transient prepared project is rewritten; authored items remain editable.

use super::cache::{
    cache_entry_dir, fingerprint, sha256_file, validate_cache_entry, CacheLock, CacheManifest,
    StagingCleanup,
};
use super::{
    ensure_not_cancelled, fps_rational, package_png_frames_as_mov, project_relative_path,
    PngSequenceSpec, PrecomposeReport,
};
use crate::gpu_graphics::ir::{GpuGraphicRole, GpuGraphicsLayer, ShaderLanguage, ShaderPass};
use crate::gpu_graphics::profile::shadertoy_fragment_source;
use crate::gpu_graphics::renderer::{render_gpu_graphics_layer, GpuRenderOptions};
use crate::gpu_graphics::templates::project_shader_background_templates;
use crate::graphics::assets::AssetRegistry;
use crate::graphics::ir::{Dimensions, GraphicsLayer};
use crate::graphics::renderer::{render_graphics_preview_cancellable, GraphicsRenderOptions};
use crate::project::model::{
    MediaAsset, MediaKind, TimelineItem, TimelineItemKind, TimelineSource, TrackKind, VideoProject,
};
use crate::render_pipeline::cancel::RenderCancellationToken;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use crate::render_pipeline::project_export::build_project_graphics_layers;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::Duration;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(tag = "renderer", content = "layer")]
enum TemplateLayer {
    Shader(Box<GpuGraphicsLayer>),
    Motion(Box<GraphicsLayer>),
}

pub(super) fn prepare_graphics_templates(
    project_dir: &Path,
    project: &mut VideoProject,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<Vec<PrecomposeReport>> {
    let mut reports = Vec::new();
    let mut template_tracks = BTreeSet::new();
    for track_index in 0..project.timeline.tracks.len() {
        if !project.timeline.tracks[track_index].enabled {
            continue;
        }
        for item_index in 0..project.timeline.tracks[track_index].items.len() {
            let item = project.timeline.tracks[track_index].items[item_index].clone();
            let shader = item.kind == TimelineItemKind::HyperframeScene
                && item
                    .properties
                    .get("shaderBackgroundTemplateId")
                    .and_then(serde_json::Value::as_str)
                    .is_some();
            let motion = item.kind == TimelineItemKind::Overlay
                && item
                    .properties
                    .get("templateId")
                    .and_then(serde_json::Value::as_str)
                    .is_some();
            if !shader && !motion {
                continue;
            }
            // Already prepared items have a media source and never enter this path again.
            ensure_not_cancelled(cancellation)?;
            let layer = template_layer(project_dir, project, &item, shader)?;
            let key = fingerprint(
                &json!({"schemaVersion": 1, "revision": "timeline-template-raster-v1", "layer": layer}),
            )?;
            let directory = cache_entry_dir(project_dir, &key);
            let _lock = CacheLock::acquire(project_dir, &key, Duration::from_secs(60))?;
            let mut cached = validate_cache_entry(&directory, &key);
            let cache_hit = cached.is_some();
            if cached.is_none() {
                let staging = directory.with_extension(format!("staging-{}", Uuid::new_v4()));
                let mut cleanup = StagingCleanup::new(staging.clone());
                let manifest = match &layer {
                    TemplateLayer::Shader(layer) => render_gpu_graphics_layer(
                        layer,
                        GpuRenderOptions {
                            output_dir: staging.clone(),
                        },
                    )
                    .map_err(crate::gpu_graphics::error::gpu_errors_to_pipeline_errors)?,
                    TemplateLayer::Motion(layer) => render_graphics_preview_cancellable(
                        layer,
                        &AssetRegistry::new(project_dir.to_path_buf()),
                        GraphicsRenderOptions {
                            output_dir: staging.clone(),
                        },
                        || cancellation.is_some_and(RenderCancellationToken::is_cancelled),
                    )
                    .map_err(PipelineError::from_graphics_errors)?,
                };
                ensure_not_cancelled(cancellation)?;
                let (numerator, denominator) = fps_rational(manifest.fps)
                    .ok_or_else(|| template_error(&item, "Template frame rate is invalid."))?;
                let mut png_hashes = Vec::new();
                let mut rgba_hashes = Vec::new();
                for index in 0..manifest.frame_count {
                    ensure_not_cancelled(cancellation)?;
                    let frame = staging.join(format!("frames/frame-{index:06}.png"));
                    png_hashes.push(
                        sha256_file(&frame)
                            .map_err(|error| template_error(&item, &error.to_string()))?,
                    );
                    let rgba = image::open(&frame)
                        .map_err(|error| template_error(&item, &error.to_string()))?
                        .into_rgba8();
                    rgba_hashes.push(format!("{:x}", Sha256::digest(rgba.as_raw())));
                }
                let intermediate = staging.join("intermediate.mov");
                package_png_frames_as_mov(
                    &staging.join("frames"),
                    manifest.frame_count,
                    PngSequenceSpec {
                        width: manifest.dimensions.width,
                        height: manifest.dimensions.height,
                        fps_numerator: numerator,
                        fps_denominator: denominator,
                    },
                    &intermediate,
                    Duration::from_secs(120),
                )?;
                let stage = json!({"schemaVersion":1, "fingerprint":key, "frameCount":manifest.frame_count,
                    "width":manifest.dimensions.width, "height":manifest.dimensions.height,
                    "fpsNumerator":numerator, "fpsDenominator":denominator,
                    "framePngSha256":png_hashes, "frameRgbaSha256":rgba_hashes, "graphics":manifest});
                let cache = CacheManifest {
                    schema_version: 1,
                    fingerprint: key.clone(),
                    worker_manifest: "stage-manifest.json".into(),
                    intermediate: "intermediate.mov".into(),
                    intermediate_sha256: sha256_file(&intermediate)
                        .map_err(|error| template_error(&item, &error.to_string()))?,
                    frame_count: manifest.frame_count,
                    width: manifest.dimensions.width,
                    height: manifest.dimensions.height,
                    fps_numerator: numerator,
                    fps_denominator: denominator,
                };
                fs::write(
                    staging.join("stage-manifest.json"),
                    serde_json::to_vec_pretty(&stage).unwrap(),
                )
                .map_err(|error| template_error(&item, &error.to_string()))?;
                fs::write(
                    staging.join("cache-manifest.json"),
                    serde_json::to_vec_pretty(&cache).unwrap(),
                )
                .map_err(|error| template_error(&item, &error.to_string()))?;
                ensure_not_cancelled(cancellation)?;
                if directory.exists() {
                    fs::remove_dir_all(&directory)
                        .map_err(|error| template_error(&item, &error.to_string()))?;
                }
                fs::create_dir_all(directory.parent().unwrap())
                    .map_err(|error| template_error(&item, &error.to_string()))?;
                fs::rename(&staging, &directory)
                    .map_err(|error| template_error(&item, &error.to_string()))?;
                cleanup.disarm();
                cached = Some(cache);
            }
            let cached = cached.expect("prepared template cache");
            let relative_intermediate =
                project_relative_path(project_dir, &directory.join(&cached.intermediate))?;
            let prepared_media_id = format!("prepared-template-{}", &key[..24]);
            if !project
                .media
                .iter()
                .any(|asset| asset.id == prepared_media_id)
            {
                project.media.push(MediaAsset {
                    id: prepared_media_id.clone(),
                    name: Some(item.label.clone()),
                    relative_path: relative_intermediate.clone(),
                    kind: MediaKind::Video,
                    duration_seconds: f64::from(cached.frame_count) / project.render_settings.fps,
                    width: Some(cached.width),
                    height: Some(cached.height),
                    fps: Some(project.render_settings.fps),
                    folder_id: None,
                });
            }
            let prepared_track = &mut project.timeline.tracks[track_index];
            template_tracks.insert(prepared_track.id.clone());
            prepared_track.kind = TrackKind::Video;
            let prepared_item = &mut prepared_track.items[item_index];
            prepared_item.kind = TimelineItemKind::VideoClip;
            prepared_item.source = TimelineSource::Media {
                media_id: prepared_media_id.clone(),
            };
            // Template previews must use the rasterized media rather than the DOM thumbnail.
            prepared_item.properties.remove("templateId");
            prepared_item.properties.remove("previewVariant");
            prepared_item.properties.insert("sourceIn".into(), json!(0));
            prepared_item
                .properties
                .insert("sourceOut".into(), json!(item.duration_seconds));
            prepared_item.properties.insert("speed".into(), json!(1));
            reports.push(PrecomposeReport {
                stage: if shader {
                    "shaderBackground"
                } else {
                    "graphicsTemplate"
                }
                .into(),
                item_id: item.id.clone(),
                media_id: match &item.source {
                    TimelineSource::Generated { artifact_id } => artifact_id.clone(),
                    TimelineSource::Media { media_id } => media_id.clone(),
                    _ => item.id.clone(),
                },
                prepared_media_id,
                fingerprint: key,
                cache_hit,
                expressions_enabled: false,
                compositor_backend: Some(if shader { "wgpu" } else { "rust" }.into()),
                compositor_fallback: None,
                worker_manifest: project_relative_path(
                    project_dir,
                    &directory.join(&cached.worker_manifest),
                )?,
                intermediate: relative_intermediate,
            });
        }
    }
    // Render media follows stored track order, while authored graphics draw above
    // primary media. Keep that order after converting graphics into media tracks.
    project
        .timeline
        .tracks
        .sort_by_key(|track| template_tracks.contains(&track.id));
    Ok(reports)
}

fn template_layer(
    project_dir: &Path,
    project: &VideoProject,
    item: &TimelineItem,
    shader: bool,
) -> PipelineResult<TemplateLayer> {
    if shader {
        let id = item.properties["shaderBackgroundTemplateId"]
            .as_str()
            .unwrap();
        let template = project_shader_background_templates(project_dir)
            .map_err(|error| template_error(item, &error.to_string()))?
            .into_iter()
            .find(|template| template.config.id == id)
            .ok_or_else(|| template_error(item, "Shader background template was not found."))?;
        let config = template.config;
        Ok(TemplateLayer::Shader(Box::new(GpuGraphicsLayer {
            schema_version: 1,
            id: item.id.clone(),
            role: GpuGraphicRole::ShaderBackground,
            timeline_start: 0.0,
            duration_seconds: item.duration_seconds,
            dimensions: Dimensions {
                width: project.render_settings.width,
                height: project.render_settings.height,
            },
            fps: project.render_settings.fps,
            alpha: config.render_contract.alpha,
            source_beat: item.label.clone(),
            visual_treatment: config.visual_treatment,
            motion: config.motion,
            safe_zone: config.safe_zone,
            avoid: config.avoid,
            quality_profile: Some(config.shader_profile_id),
            background: Some(ShaderPass {
                shader_language: ShaderLanguage::Glsl,
                fragment_source: shadertoy_fragment_source(&template.fragment_body),
                uniforms: BTreeMap::new(),
            }),
            scene: None,
        })))
    } else {
        let mut single = project.clone();
        single.timeline.tracks.truncate(1);
        single.timeline.tracks[0].enabled = true;
        single.timeline.tracks[0].items = vec![item.clone()];
        single.timeline.tracks[0].items[0].start_seconds = 0.0;
        single.timelines.clear();
        let mut layers = build_project_graphics_layers(
            &single,
            project.render_settings.width,
            project.render_settings.height,
            project.render_settings.fps,
        )?;
        layers
            .pop()
            .map(|layer| TemplateLayer::Motion(Box::new(layer)))
            .ok_or_else(|| template_error(item, "Motion template produced no graphics layer."))
    }
}

fn template_error(item: &TimelineItem, message: &str) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        "precompose.graphicsTemplate",
        message,
        "Inspect the template and render runtime, then retry preview preparation.",
    )
    .with_detail("itemId", item.id.clone())]
}
