mod audio_denoise;
mod audio_retime;
#[cfg(feature = "ges-render")]
mod audio_retime_pipeline;
mod cache;
mod canonical_frame;
pub(crate) mod compatibility;
mod flatten;
mod flatten_transitions;
mod frame_source;
mod intermediate;
mod planner;
mod reverse;
mod reverse_audio;
#[cfg(feature = "ges-render")]
mod reverse_pipeline;
mod worker_client;

pub(crate) use audio_denoise::decode_to_wav;
pub use canonical_frame::{
    render_canonical_frame_rgba, sample_canonical_frame, CanonicalFrameSample,
    CanonicalLayerSample, CanonicalSolidSample,
};
pub use frame_source::{decode_video_frames_rgba, FrameDecodeSpec};
pub use intermediate::{package_png_frames_as_mov, PngSequenceSpec};
pub use planner::{plan_precomposition, PreparationReason, PreparationScope, PreparationTask};
pub use worker_client::{run_precompose_worker, run_precompose_worker_cancellable, WorkerRun};

use crate::frame_compositor::PreparedEffectStack;
use crate::project::model::{MediaAsset, MediaKind, TimelineSource, VideoProject};
use crate::render_pipeline::cancel::RenderCancellationToken;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use cache::{
    cache_entry_dir, fingerprint, sha256_file, validate_cache_entry, CacheLock, CacheManifest,
    LottieFingerprint, LutFingerprint, StagingCleanup,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::time::Duration;
use uuid::Uuid;
use video_creater_precompose_protocol::{
    AlphaMode, ColorSpace, ExpressionPolicy, FrameRate, LottieRenderSpec, LottieRuntimeInputs,
    LottieSource, LottieSourceFormat, PrecomposeOperation, PrecomposeRequest, WorkerBudgets,
    WorkerOutput, PRECOMPOSE_PROTOCOL_NAME, PRECOMPOSE_PROTOCOL_VERSION,
};

const DOTLOTTIE_REVISION: &str = "2ce5e48f5786c3e60301d66db4cfdff56c896895";
const THORVG_REVISION: &str = "73045df5398690eb1c6c0946e0b2301032337776";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrecomposeReport {
    pub stage: String,
    pub item_id: String,
    pub media_id: String,
    pub prepared_media_id: String,
    pub fingerprint: String,
    pub cache_hit: bool,
    pub expressions_enabled: bool,
    pub compositor_backend: Option<String>,
    pub compositor_fallback: Option<String>,
    pub worker_manifest: String,
    pub intermediate: String,
}

impl PrecomposeReport {
    /// Whether the stage publishes a `frames/` PNG sequence next to its
    /// intermediate. Audio and compatibility stages do not.
    pub fn has_frame_sequence(&self) -> bool {
        !matches!(
            self.stage.as_str(),
            "audioDenoise" | "audioRetime" | "audioReverse" | "compatibilityDecode"
        )
    }
}

#[derive(Debug, Clone)]
pub struct PreparedProject {
    pub project: VideoProject,
    pub reports: Vec<PrecomposeReport>,
}

pub fn prepare_project_for_render(
    project_dir: &Path,
    project: &VideoProject,
) -> PipelineResult<PreparedProject> {
    prepare_project_for_render_cancellable(project_dir, project, None)
}

pub fn prepare_project_for_render_cancellable(
    project_dir: &Path,
    project: &VideoProject,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<PreparedProject> {
    // Fail before any step decodes a reversed clip that cannot be reversed.
    reverse::reject_unreversible_items(project)?;
    let tasks = plan_precomposition(project);
    let mut prepared = project.clone();
    let mut reports = Vec::new();
    reports.extend(compatibility::prepare_compatibility_media(
        project_dir,
        &mut prepared,
        cancellation,
    )?);
    // Reversed audio first: denoise and retime then work on the reversed WAV.
    reports.extend(reverse_audio::prepare_reversed_audio(
        project_dir,
        &mut prepared,
        cancellation,
    )?);
    reports.extend(audio_denoise::prepare_audio_denoise(
        project_dir,
        &mut prepared,
        cancellation,
    )?);
    // After denoise, which keeps the clip speed: retimed audio plays a
    // pitch-preserved intermediate at speed 1.
    reports.extend(audio_retime::prepare_retimed_audio(
        project_dir,
        &mut prepared,
        cancellation,
    )?);
    let handles = PreparedSourceHandles::plan(&prepared);
    for task in tasks {
        ensure_not_cancelled(cancellation)?;
        let task_handles = handles.for_item(task.track_index, &task.item_id);
        if task.reasons.contains(&PreparationReason::Reverse) {
            reports.push(reverse::prepare_reversed_video(
                project_dir,
                &mut prepared,
                &task,
                task_handles,
                cancellation,
            )?);
        }
        if task.reasons.contains(&PreparationReason::Lottie) {
            reports.push(prepare_lottie_task(
                project_dir,
                &mut prepared,
                &task,
                task_handles,
                cancellation,
            )?);
        }
        if task.reasons.contains(&PreparationReason::Lut) {
            reports.push(prepare_lut_task(
                project_dir,
                &mut prepared,
                &task,
                task_handles,
                cancellation,
            )?);
        }
    }
    // Every reversed clip now plays a reversed intermediate.
    crate::render_pipeline::reverse_guard::reject_unprepared_reversed_items(&prepared)?;
    reports.extend(flatten::prepare_flattened_composites(
        project_dir,
        &mut prepared,
        cancellation,
    )?);
    Ok(PreparedProject {
        project: prepared,
        reports,
    })
}

/// The timeline span `(start, duration)` covered by the prepared media of a
/// prepared clip, whose frames start at `start`. Prepared intermediates play
/// at speed 1 and may include transition handles before `sourceIn`, so the
/// span starts `sourceIn` seconds before the clip and lasts the media duration.
pub fn prepared_media_timeline_span(
    project: &VideoProject,
    item: &crate::project::model::TimelineItem,
) -> (f64, f64) {
    let TimelineSource::Media { media_id } = &item.source else {
        return (item.start_seconds, item.duration_seconds);
    };
    let source_in = numeric_property(&item.properties, "sourceIn")
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(0.0);
    let duration = project
        .media
        .iter()
        .find(|media| media.id == *media_id)
        .map(|media| media.duration_seconds)
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .unwrap_or(source_in + item.duration_seconds);
    (item.start_seconds - source_in, duration)
}

/// Transition handles, in timeline seconds, a prepared source must include
/// beyond its clip: `head` before the start and `tail` after the end. A
/// prepared intermediate covers `head + duration + tail` and its clip reads
/// from `sourceIn = head`, so the render plan and preview still find the
/// handles their transitions need.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct PreparedSourceHandles {
    pub(crate) head_seconds: f64,
    pub(crate) tail_seconds: f64,
}

#[derive(Debug, Default)]
struct PreparedSourceHandlePlan(std::collections::BTreeMap<(usize, String), PreparedSourceHandles>);

impl PreparedSourceHandles {
    fn plan(project: &VideoProject) -> PreparedSourceHandlePlan {
        let mut plan = PreparedSourceHandlePlan::default();
        for transition in crate::render_pipeline::transition_plan::plan_clip_transitions(project) {
            if transition.audio {
                continue;
            }
            let half = transition.duration_seconds / 2.0;
            plan.0
                .entry((transition.track_index, transition.left_item_id))
                .or_default()
                .tail_seconds = half;
            plan.0
                .entry((transition.track_index, transition.right_item_id))
                .or_default()
                .head_seconds = half;
        }
        plan
    }

    /// The timeline span a prepared source covers for a clip of `duration` seconds.
    fn span_seconds(&self, duration_seconds: f64) -> f64 {
        self.head_seconds + duration_seconds + self.tail_seconds
    }
}

impl PreparedSourceHandlePlan {
    fn for_item(&self, track_index: usize, item_id: &str) -> PreparedSourceHandles {
        self.0
            .get(&(track_index, item_id.to_string()))
            .copied()
            .unwrap_or_default()
    }
}

fn prepare_lottie_task(
    project_dir: &Path,
    project: &mut VideoProject,
    task: &PreparationTask,
    handles: PreparedSourceHandles,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<PrecomposeReport> {
    let media = project
        .media
        .iter()
        .find(|media| media.id == task.media_id)
        .cloned()
        .ok_or_else(|| vec![precompose_item_error(task, "Lottie media was not found.")])?;
    let item = project
        .timeline
        .tracks
        .get(task.track_index)
        .and_then(|track| track.items.get(task.item_index))
        .cloned()
        .ok_or_else(|| {
            vec![precompose_item_error(
                task,
                "Lottie timeline item was not found.",
            )]
        })?;
    let source_in = numeric_property(&item.properties, "sourceIn").unwrap_or(0.0);
    let source_out = numeric_property(&item.properties, "sourceOut")
        .unwrap_or(source_in + item.duration_seconds);
    let speed = numeric_property(&item.properties, "speed").unwrap_or(1.0);
    if !source_in.is_finite()
        || !source_out.is_finite()
        || source_in < 0.0
        || source_out <= source_in
        || !speed.is_finite()
        || !(0.1..=8.0).contains(&speed)
    {
        return Err(vec![precompose_item_error(
            task,
            "Lottie source range or playback speed is invalid.",
        )]);
    }
    let width = media.width.unwrap_or(project.render_settings.width);
    let height = media.height.unwrap_or(project.render_settings.height);
    let fps = project.render_settings.fps;
    let (fps_numerator, fps_denominator) = fps_rational(fps).ok_or_else(|| {
        vec![precompose_item_error(
            task,
            "Lottie output frame rate is invalid.",
        )]
    })?;
    let span_seconds = handles.span_seconds(item.duration_seconds);
    let frame_count = (span_seconds * fps).ceil().max(1.0) as u32;
    let source_start_micros =
        seconds_to_micros((source_in - handles.head_seconds * speed).max(0.0))?;
    let timeline_duration_micros = seconds_to_micros(span_seconds)?;
    let playback_rate_micros = (speed * 1_000_000.0).round() as u32;
    let sampled_source_duration_micros = u128::from(timeline_duration_micros)
        .saturating_mul(u128::from(playback_rate_micros))
        / 1_000_000;
    let source_stop_micros = u128::from(source_start_micros)
        .saturating_add(sampled_source_duration_micros)
        .min(u128::from(u64::MAX)) as u64;
    let animation_id = item
        .properties
        .get("animationId")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let looping = item
        .properties
        .get("looping")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let runtime_inputs = item.properties.get("lottieInputs").map_or_else(
        || Ok(LottieRuntimeInputs::default()),
        |value| {
            serde_json::from_value(value.clone()).map_err(|error| {
                vec![precompose_item_error(
                    task,
                    &format!("Lottie inputs are invalid: {error}"),
                )]
            })
        },
    )?;
    let source_path = project_dir.join(&media.relative_path);
    let source = fs::read(&source_path).map_err(|error| {
        vec![precompose_item_error(
            task,
            &format!("Lottie source could not be read: {error}"),
        )]
    })?;
    let source_sha256 = format!("{:x}", Sha256::digest(&source));
    let fingerprint = fingerprint(&LottieFingerprint {
        schema_version: 1,
        source_sha256: &source_sha256,
        animation_id,
        runtime_inputs: &runtime_inputs,
        width,
        height,
        fps_numerator,
        fps_denominator,
        source_start_micros,
        source_stop_micros,
        playback_rate_micros,
        timeline_duration_micros,
        frame_count,
        looping,
        expressions_enabled: true,
        pixel_contract: "rgba8-srgb-straight-v1",
        rng_policy: "expression-code-frame-xorshift64star-v1",
        dotlottie_revision: DOTLOTTIE_REVISION,
        thorvg_revision: THORVG_REVISION,
    })?;
    let cache_dir = cache_entry_dir(project_dir, &fingerprint);
    fs::create_dir_all(cache_dir.parent().unwrap_or(project_dir)).map_err(|error| {
        vec![precompose_item_error(
            task,
            &format!("Precompose cache directory could not be created: {error}"),
        )]
    })?;
    let _lock = CacheLock::acquire(project_dir, &fingerprint, Duration::from_secs(30))?;
    let mut cache_hit = true;
    let manifest = if let Some(manifest) = validate_cache_entry(&cache_dir, &fingerprint) {
        manifest
    } else {
        cache_hit = false;
        if cache_dir.exists() {
            fs::remove_dir_all(&cache_dir).map_err(|error| {
                vec![precompose_item_error(
                    task,
                    &format!("Corrupt precompose cache entry could not be removed: {error}"),
                )]
            })?;
        }
        let staging =
            cache_dir.with_file_name(format!(".{}.staging-{}", fingerprint, Uuid::new_v4()));
        let mut staging_cleanup = StagingCleanup::new(staging.clone());
        let request = PrecomposeRequest {
            protocol: PRECOMPOSE_PROTOCOL_NAME.to_string(),
            schema_version: PRECOMPOSE_PROTOCOL_VERSION,
            request_id: format!("precompose-{}", &fingerprint[..16]),
            cache_key: fingerprint.clone(),
            operation: PrecomposeOperation::BakeLottieRgba,
            source: LottieSource {
                path: source_path.to_string_lossy().into_owned(),
                sha256: source_sha256,
                format: if source_path.extension().and_then(|value| value.to_str())
                    == Some("lottie")
                {
                    LottieSourceFormat::DotLottie
                } else {
                    LottieSourceFormat::LottieJson
                },
                animation_id: animation_id.map(str::to_string),
            },
            render: LottieRenderSpec {
                width,
                height,
                fps: FrameRate {
                    numerator: fps_numerator,
                    denominator: fps_denominator,
                },
                first_frame: 0,
                frame_count,
                source_start_micros,
                playback_rate_micros,
                looping,
                alpha_mode: AlphaMode::Straight,
                color_space: ColorSpace::Srgb,
            },
            inputs: runtime_inputs,
            expressions: ExpressionPolicy { enabled: true },
            budgets: worker_budgets(frame_count, width, height),
            output: WorkerOutput {
                staging_dir: staging.to_string_lossy().into_owned(),
            },
        };
        let worker = match run_precompose_worker_cancellable(&request, || {
            cancellation.is_some_and(RenderCancellationToken::is_cancelled)
        }) {
            Ok(worker) => worker,
            Err(error) => {
                let _ = fs::remove_dir_all(&staging);
                return Err(error);
            }
        };
        let intermediate = staging.join("intermediate.mov");
        if let Err(errors) = package_png_frames_as_mov(
            &staging.join("frames"),
            frame_count,
            PngSequenceSpec {
                width,
                height,
                fps_numerator,
                fps_denominator,
            },
            &intermediate,
            Duration::from_secs(60),
        ) {
            let _ = fs::remove_dir_all(&staging);
            return Err(errors);
        }
        ensure_not_cancelled(cancellation)?;
        let cache_manifest = CacheManifest {
            schema_version: 1,
            fingerprint: fingerprint.clone(),
            worker_manifest: "manifest.json".to_string(),
            intermediate: "intermediate.mov".to_string(),
            intermediate_sha256: sha256_file(&intermediate).map_err(|error| {
                vec![precompose_item_error(
                    task,
                    &format!("Prepared intermediate could not be hashed: {error}"),
                )]
            })?,
            frame_count,
            width,
            height,
            fps_numerator,
            fps_denominator,
        };
        let cache_manifest_bytes = serde_json::to_vec_pretty(&cache_manifest).map_err(|error| {
            vec![precompose_item_error(
                task,
                &format!("Precompose cache manifest could not be serialized: {error}"),
            )]
        })?;
        fs::write(staging.join("cache-manifest.json"), cache_manifest_bytes).map_err(|error| {
            vec![precompose_item_error(
                task,
                &format!("Precompose cache manifest could not be written: {error}"),
            )]
        })?;
        ensure_not_cancelled(cancellation)?;
        fs::rename(&staging, &cache_dir).map_err(|error| {
            vec![precompose_item_error(
                task,
                &format!("Precompose cache entry could not be published atomically: {error}"),
            )]
        })?;
        staging_cleanup.disarm();
        let _ = worker;
        cache_manifest
    };

    let intermediate_path = cache_dir.join(&manifest.intermediate);
    let relative_intermediate = project_relative_path(project_dir, &intermediate_path)?;
    let prepared_media_id = format!("precompose-{fingerprint}");
    if !project
        .media
        .iter()
        .any(|media| media.id == prepared_media_id)
    {
        project.media.push(MediaAsset {
            id: prepared_media_id.clone(),
            name: Some(format!("Prepared {}", item.label)),
            relative_path: relative_intermediate.clone(),
            kind: MediaKind::Video,
            duration_seconds: span_seconds,
            width: Some(width),
            height: Some(height),
            fps: Some(fps),
            folder_id: media.folder_id.clone(),
        });
    }
    let prepared_item = project
        .timeline
        .tracks
        .get_mut(task.track_index)
        .and_then(|track| track.items.get_mut(task.item_index))
        .ok_or_else(|| {
            vec![precompose_item_error(
                task,
                "Prepared timeline item was lost.",
            )]
        })?;
    prepared_item.source = TimelineSource::Media {
        media_id: prepared_media_id.clone(),
    };
    prepared_item
        .properties
        .insert("sourceIn".to_string(), json!(handles.head_seconds));
    prepared_item.properties.insert(
        "sourceOut".to_string(),
        json!(handles.head_seconds + item.duration_seconds),
    );
    prepared_item
        .properties
        .insert("speed".to_string(), json!(1.0));

    Ok(PrecomposeReport {
        stage: "lottie".to_string(),
        item_id: task.item_id.clone(),
        media_id: task.media_id.clone(),
        prepared_media_id,
        fingerprint,
        cache_hit,
        expressions_enabled: true,
        compositor_backend: None,
        compositor_fallback: None,
        worker_manifest: project_relative_path(
            project_dir,
            &cache_dir.join(&manifest.worker_manifest),
        )?,
        intermediate: relative_intermediate,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LutStageManifest {
    schema_version: u32,
    fingerprint: String,
    frame_count: u32,
    width: u32,
    height: u32,
    fps_numerator: u32,
    fps_denominator: u32,
    source_sha256: String,
    lut_sha256: String,
    lut_strength_micros: u32,
    pre_lut_effect_stack: PreparedEffectStack,
    frame_rgba_sha256: Vec<String>,
    frame_png_sha256: Vec<String>,
    pixel_contract: &'static str,
    interpolation: &'static str,
}

fn prepare_lut_task(
    project_dir: &Path,
    project: &mut VideoProject,
    task: &PreparationTask,
    handles: PreparedSourceHandles,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<PrecomposeReport> {
    let item = project
        .timeline
        .tracks
        .get(task.track_index)
        .and_then(|track| track.items.get(task.item_index))
        .cloned()
        .ok_or_else(|| {
            vec![precompose_item_error(
                task,
                "LUT timeline item was not found.",
            )]
        })?;
    let media_id = match &item.source {
        TimelineSource::Media { media_id } => media_id.clone(),
        _ => {
            return Err(vec![precompose_item_error(
                task,
                "LUT preparation requires a media-backed visual item.",
            )])
        }
    };
    let media = project
        .media
        .iter()
        .find(|media| media.id == media_id)
        .cloned()
        .ok_or_else(|| {
            vec![precompose_item_error(
                task,
                "LUT source media was not found.",
            )]
        })?;
    if !matches!(media.kind, MediaKind::Video) {
        return Err(vec![precompose_item_error(
            task,
            "LUT preparation currently requires video or a prepared Lottie intermediate.",
        )]);
    }
    let legacy_lut = item
        .properties
        .get("colorGrade")
        .and_then(serde_json::Value::as_object)
        .and_then(|grade| grade.get("lut"))
        .and_then(serde_json::Value::as_object);
    let catalog_lut = item
        .properties
        .get("effects")
        .and_then(serde_json::Value::as_array)
        .and_then(|effects| {
            effects.iter().find(|effect| {
                effect.get("effectType").and_then(serde_json::Value::as_str) == Some("color.lut")
                    && effect.get("enabled").and_then(serde_json::Value::as_bool) != Some(false)
            })
        })
        .and_then(|effect| effect.get("params"))
        .and_then(serde_json::Value::as_object);
    let lut = legacy_lut
        .or(catalog_lut)
        .ok_or_else(|| vec![precompose_item_error(task, "LUT metadata was not found.")])?;
    let lut_relative = lut
        .get("path")
        .and_then(serde_json::Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| vec![precompose_item_error(task, "LUT path is missing.")])?;
    let lut_strength = legacy_lut
        .and_then(|lut| lut.get("strength"))
        .or_else(|| catalog_lut.and_then(|lut| lut.get("intensity")))
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(1.0);
    if !lut_strength.is_finite() || !(0.0..=1.0).contains(&lut_strength) {
        return Err(vec![precompose_item_error(
            task,
            "LUT strength must be between zero and one.",
        )]);
    }
    let lut_path = safe_project_relative_path(project_dir, lut_relative, task)?;
    validate_lut_path(project_dir, &lut_path, task)?;
    let lut_bytes = fs::read(&lut_path).map_err(|error| {
        vec![precompose_item_error(
            task,
            &format!("LUT source could not be read: {error}"),
        )]
    })?;
    if lut_bytes.len() > 16 * 1024 * 1024 {
        return Err(vec![precompose_item_error(
            task,
            "LUT source exceeds the 16 MiB preparation limit.",
        )]);
    }
    let lut_text = std::str::from_utf8(&lut_bytes).map_err(|_| {
        vec![precompose_item_error(
            task,
            "LUT source must be UTF-8 text.",
        )]
    })?;
    let cube = crate::frame_compositor::CubeLut::parse(lut_text).map_err(|error| {
        vec![precompose_item_error(
            task,
            &format!("LUT source is invalid: {error}"),
        )]
    })?;
    let lut_effect_stack = PreparedEffectStack::from_item_properties_for_duration(
        &item.properties,
        item.duration_seconds,
    )
    .map_err(|error| {
        vec![precompose_item_error(
            task,
            &format!("Pre-LUT effect stack is invalid: {error}"),
        )]
    })?;
    let pre_lut_effect_stack = lut_effect_stack.before_lut();

    let source_in = numeric_property(&item.properties, "sourceIn").unwrap_or(0.0);
    let source_out = numeric_property(&item.properties, "sourceOut")
        .unwrap_or(source_in + item.duration_seconds);
    let speed = numeric_property(&item.properties, "speed").unwrap_or(1.0);
    if !source_in.is_finite()
        || !source_out.is_finite()
        || source_in < 0.0
        || source_out <= source_in
        || !speed.is_finite()
        || !(0.1..=8.0).contains(&speed)
    {
        return Err(vec![precompose_item_error(
            task,
            "LUT source range or playback speed is invalid.",
        )]);
    }
    let width = media.width.unwrap_or(project.render_settings.width);
    let height = media.height.unwrap_or(project.render_settings.height);
    let fps = project.render_settings.fps;
    let (fps_numerator, fps_denominator) = fps_rational(fps).ok_or_else(|| {
        vec![precompose_item_error(
            task,
            "LUT output frame rate is invalid.",
        )]
    })?;
    let span_seconds = handles.span_seconds(item.duration_seconds);
    let frame_count = (span_seconds * fps).ceil().max(1.0) as u32;
    let source_start_micros =
        seconds_to_micros((source_in - handles.head_seconds * speed).max(0.0))?;
    let timeline_duration_micros = seconds_to_micros(span_seconds)?;
    let playback_rate_micros = (speed * 1_000_000.0).round() as u32;
    let sampled_source_duration_micros = u128::from(timeline_duration_micros)
        .saturating_mul(u128::from(playback_rate_micros))
        / 1_000_000;
    let source_stop_micros = u128::from(source_start_micros)
        .saturating_add(sampled_source_duration_micros)
        .min(u128::from(u64::MAX)) as u64;
    let declared_source_out_micros = seconds_to_micros(source_out + handles.tail_seconds * speed)?;
    let transition_head_micros = seconds_to_micros(handles.head_seconds)?;
    // Effects and LUT intensity sample canonical clip time, holding their edge
    // values in transition handles.
    let clip_seconds = |frame_index: u32| {
        (frame_index as f64 / fps - handles.head_seconds).clamp(0.0, item.duration_seconds)
    };
    let media_duration_micros = seconds_to_micros(media.duration_seconds)?;
    if source_stop_micros > declared_source_out_micros.saturating_add(1)
        || source_stop_micros > media_duration_micros.saturating_add(1)
    {
        return Err(vec![precompose_item_error(
            task,
            "LUT sampling would read beyond sourceOut or the media duration.",
        )]);
    }
    let source_path = safe_project_relative_path(project_dir, &media.relative_path, task)?;
    let source_bytes = fs::read(&source_path).map_err(|error| {
        vec![precompose_item_error(
            task,
            &format!("LUT input media could not be read: {error}"),
        )]
    })?;
    let source_sha256 = format!("{:x}", Sha256::digest(&source_bytes));
    let lut_sha256 = format!("{:x}", Sha256::digest(cube.to_canonical_cube().as_bytes()));
    let lut_strength_micros = (lut_strength * 1_000_000.0).round() as u32;
    let fingerprint = fingerprint(&LutFingerprint {
        schema_version: 1,
        source_sha256: &source_sha256,
        lut_sha256: &lut_sha256,
        lut_strength_micros,
        lut_effect_stack: &lut_effect_stack,
        pre_lut_effect_stack: &pre_lut_effect_stack,
        width,
        height,
        fps_numerator,
        fps_denominator,
        source_start_micros,
        source_stop_micros,
        playback_rate_micros,
        timeline_duration_micros,
        frame_count,
        transition_head_micros,
        pixel_contract: "rgba8-srgb-straight-v1",
        interpolation: "trilinear-rgb-srgb-v1",
        sampling_policy: "presentation-interval-hold-v1",
        compositor_revision: "cpu-frame-compositor-v1",
    })?;
    let cache_dir = cache_entry_dir(project_dir, &fingerprint);
    fs::create_dir_all(cache_dir.parent().unwrap_or(project_dir)).map_err(|error| {
        vec![precompose_item_error(
            task,
            &format!("Precompose cache directory could not be created: {error}"),
        )]
    })?;
    let _lock = CacheLock::acquire(project_dir, &fingerprint, Duration::from_secs(30))?;
    let mut cache_hit = true;
    let manifest = if let Some(manifest) = validate_cache_entry(&cache_dir, &fingerprint) {
        manifest
    } else {
        cache_hit = false;
        if cache_dir.exists() {
            fs::remove_dir_all(&cache_dir).map_err(|error| {
                vec![precompose_item_error(
                    task,
                    &format!("Corrupt LUT cache entry could not be removed: {error}"),
                )]
            })?;
        }
        let staging =
            cache_dir.with_file_name(format!(".{}.staging-{}", fingerprint, Uuid::new_v4()));
        let mut staging_cleanup = StagingCleanup::new(staging.clone());
        let frames_dir = staging.join("frames");
        fs::create_dir_all(&frames_dir).map_err(|error| {
            vec![precompose_item_error(
                task,
                &format!("LUT frame staging directory could not be created: {error}"),
            )]
        })?;
        let mut frame_rgba_sha256 = Vec::with_capacity(frame_count as usize);
        let mut frame_png_sha256 = Vec::with_capacity(frame_count as usize);
        let decode_result = decode_video_frames_rgba(
            &source_path,
            FrameDecodeSpec {
                width,
                height,
                fps_numerator,
                fps_denominator,
                source_start_micros,
                source_stop_micros,
                playback_rate_micros,
                frame_count,
            },
            Duration::from_secs(120),
            |frame_index, rgba| {
                ensure_not_cancelled(cancellation)?;
                let effected = pre_lut_effect_stack
                    .apply_rgba8_srgb_at_seconds(
                        rgba,
                        width,
                        height,
                        frame_index,
                        clip_seconds(frame_index),
                    )
                    .map_err(|error| {
                        vec![precompose_item_error(
                            task,
                            &format!("Pre-LUT effect execution failed: {error}"),
                        )]
                    })?;
                let mut image =
                    image::RgbaImage::from_raw(width, height, effected).ok_or_else(|| {
                        vec![precompose_item_error(
                            task,
                            "Decoded LUT frame has invalid dimensions.",
                        )]
                    })?;
                let sampled_lut_strength = lut_effect_stack
                    .sample(clip_seconds(frame_index))
                    .effects
                    .iter()
                    .find(|effect| effect.effect_type == "color.lut")
                    .and_then(|effect| effect.params.get("intensity"))
                    .copied()
                    .unwrap_or(lut_strength);
                for pixel in image.pixels_mut() {
                    pixel.0 = cube.apply_rgba8_srgb_strength(pixel.0, sampled_lut_strength as f32);
                }
                frame_rgba_sha256.push(format!("{:x}", Sha256::digest(image.as_raw())));
                let frame_path = frames_dir.join(format!("frame-{frame_index:06}.png"));
                image.save(&frame_path).map_err(|error| {
                    vec![precompose_item_error(
                        task,
                        &format!("LUT frame could not be encoded: {error}"),
                    )]
                })?;
                frame_png_sha256.push(sha256_file(&frame_path).map_err(|error| {
                    vec![precompose_item_error(
                        task,
                        &format!("LUT PNG frame could not be hashed: {error}"),
                    )]
                })?);
                Ok(())
            },
        );
        if let Err(errors) = decode_result {
            let _ = fs::remove_dir_all(&staging);
            return Err(errors);
        }
        let intermediate = staging.join("intermediate.mov");
        if let Err(errors) = package_png_frames_as_mov(
            &frames_dir,
            frame_count,
            PngSequenceSpec {
                width,
                height,
                fps_numerator,
                fps_denominator,
            },
            &intermediate,
            Duration::from_secs(60),
        ) {
            let _ = fs::remove_dir_all(&staging);
            return Err(errors);
        }
        ensure_not_cancelled(cancellation)?;
        let stage_manifest = LutStageManifest {
            schema_version: 1,
            fingerprint: fingerprint.clone(),
            frame_count,
            width,
            height,
            fps_numerator,
            fps_denominator,
            source_sha256,
            lut_sha256,
            lut_strength_micros,
            pre_lut_effect_stack: pre_lut_effect_stack.clone(),
            frame_rgba_sha256,
            frame_png_sha256,
            pixel_contract: "rgba8-srgb-straight-v1",
            interpolation: "trilinear-rgb-srgb-v1",
        };
        fs::write(
            staging.join("manifest.json"),
            serde_json::to_vec_pretty(&stage_manifest).map_err(|error| {
                vec![precompose_item_error(
                    task,
                    &format!("LUT stage manifest could not be serialized: {error}"),
                )]
            })?,
        )
        .map_err(|error| {
            vec![precompose_item_error(
                task,
                &format!("LUT stage manifest could not be written: {error}"),
            )]
        })?;
        let cache_manifest = CacheManifest {
            schema_version: 1,
            fingerprint: fingerprint.clone(),
            worker_manifest: "manifest.json".to_string(),
            intermediate: "intermediate.mov".to_string(),
            intermediate_sha256: sha256_file(&intermediate).map_err(|error| {
                vec![precompose_item_error(
                    task,
                    &format!("LUT intermediate could not be hashed: {error}"),
                )]
            })?,
            frame_count,
            width,
            height,
            fps_numerator,
            fps_denominator,
        };
        fs::write(
            staging.join("cache-manifest.json"),
            serde_json::to_vec_pretty(&cache_manifest).map_err(|error| {
                vec![precompose_item_error(
                    task,
                    &format!("LUT cache manifest could not be serialized: {error}"),
                )]
            })?,
        )
        .map_err(|error| {
            vec![precompose_item_error(
                task,
                &format!("LUT cache manifest could not be written: {error}"),
            )]
        })?;
        ensure_not_cancelled(cancellation)?;
        fs::rename(&staging, &cache_dir).map_err(|error| {
            vec![precompose_item_error(
                task,
                &format!("LUT cache entry could not be published atomically: {error}"),
            )]
        })?;
        staging_cleanup.disarm();
        cache_manifest
    };

    let intermediate_path = cache_dir.join(&manifest.intermediate);
    let relative_intermediate = project_relative_path(project_dir, &intermediate_path)?;
    let prepared_media_id = format!("precompose-{fingerprint}");
    if !project
        .media
        .iter()
        .any(|media| media.id == prepared_media_id)
    {
        project.media.push(MediaAsset {
            id: prepared_media_id.clone(),
            name: Some(format!("Prepared LUT {}", item.label)),
            relative_path: relative_intermediate.clone(),
            kind: MediaKind::Video,
            duration_seconds: span_seconds,
            width: Some(width),
            height: Some(height),
            fps: Some(fps),
            folder_id: media.folder_id.clone(),
        });
    }
    let prepared_item = project
        .timeline
        .tracks
        .get_mut(task.track_index)
        .and_then(|track| track.items.get_mut(task.item_index))
        .ok_or_else(|| vec![precompose_item_error(task, "Prepared LUT item was lost.")])?;
    prepared_item.source = TimelineSource::Media {
        media_id: prepared_media_id.clone(),
    };
    prepared_item
        .properties
        .insert("sourceIn".to_string(), json!(handles.head_seconds));
    prepared_item.properties.insert(
        "sourceOut".to_string(),
        json!(handles.head_seconds + item.duration_seconds),
    );
    prepared_item
        .properties
        .insert("speed".to_string(), json!(1.0));
    prepared_item.properties.remove("colorGrade");
    let mut baked_lut_instance_ids = Vec::new();
    if let Some(effects) = prepared_item
        .properties
        .get_mut("effects")
        .and_then(serde_json::Value::as_array_mut)
    {
        let lut_rank = crate::effects::canonical_effect_rank("color.lut");
        effects.retain(|effect| {
            let effect_type = effect.get("effectType").and_then(serde_json::Value::as_str);
            if effect_type == Some("color.lut") {
                if let Some(instance_id) = effect
                    .get("effectInstanceId")
                    .and_then(serde_json::Value::as_str)
                {
                    baked_lut_instance_ids.push(instance_id.to_string());
                }
            }
            effect_type.is_none_or(|id| crate::effects::canonical_effect_rank(id) > lut_rank)
        });
    }
    if let Some(instances) = prepared_item
        .properties
        .get_mut("effectParameterKeyframes")
        .and_then(serde_json::Value::as_object_mut)
    {
        for instance_id in baked_lut_instance_ids {
            instances.remove(&instance_id);
        }
    }

    Ok(PrecomposeReport {
        stage: "lut".to_string(),
        item_id: task.item_id.clone(),
        media_id,
        prepared_media_id,
        fingerprint,
        cache_hit,
        expressions_enabled: false,
        compositor_backend: None,
        compositor_fallback: None,
        worker_manifest: project_relative_path(project_dir, &cache_dir.join("manifest.json"))?,
        intermediate: relative_intermediate,
    })
}

fn ensure_not_cancelled(cancellation: Option<&RenderCancellationToken>) -> PipelineResult<()> {
    if cancellation.is_some_and(RenderCancellationToken::is_cancelled) {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "precompose.cancelled",
            "Render preparation was cancelled.",
            "Restart the render when preparation should continue.",
        )]);
    }
    Ok(())
}

fn safe_project_relative_path(
    project_dir: &Path,
    relative: &str,
    task: &PreparationTask,
) -> PipelineResult<std::path::PathBuf> {
    let relative = Path::new(relative);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        })
    {
        return Err(vec![precompose_item_error(
            task,
            "Prepared-source path must stay inside the project directory.",
        )]);
    }
    Ok(project_dir.join(relative))
}

fn validate_lut_path(
    project_dir: &Path,
    path: &Path,
    task: &PreparationTask,
) -> PipelineResult<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        vec![precompose_item_error(
            task,
            &format!("LUT path could not be inspected: {error}"),
        )]
    })?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("cube"))
    {
        return Err(vec![precompose_item_error(
            task,
            "LUT path must name a regular non-symlink .cube file.",
        )]);
    }
    let canonical_project = fs::canonicalize(project_dir).map_err(|error| {
        vec![precompose_item_error(
            task,
            &format!("Project directory could not be canonicalized: {error}"),
        )]
    })?;
    let canonical_lut = fs::canonicalize(path).map_err(|error| {
        vec![precompose_item_error(
            task,
            &format!("LUT path could not be canonicalized: {error}"),
        )]
    })?;
    if !canonical_lut.starts_with(&canonical_project) {
        return Err(vec![precompose_item_error(
            task,
            "LUT path escaped the canonical project directory.",
        )]);
    }
    Ok(())
}

fn worker_budgets(frame_count: u32, width: u32, height: u32) -> WorkerBudgets {
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    WorkerBudgets {
        max_frames: frame_count,
        max_pixels_per_frame: pixels,
        max_source_bytes: 64 * 1024 * 1024,
        max_archive_entries: 512,
        max_expanded_archive_bytes: 256 * 1024 * 1024,
        max_compression_ratio: 200,
        max_wall_time_ms: u64::from(frame_count)
            .saturating_mul(250)
            .clamp(30_000, 3_600_000),
        max_memory_bytes: pixels.saturating_mul(12).max(128 * 1024 * 1024),
        max_output_bytes: pixels
            .saturating_mul(4)
            .saturating_mul(u64::from(frame_count))
            .saturating_add(16 * 1024 * 1024),
    }
}

pub(super) fn numeric_property(
    properties: &std::collections::BTreeMap<String, serde_json::Value>,
    key: &str,
) -> Option<f64> {
    properties.get(key).and_then(serde_json::Value::as_f64)
}

pub(super) fn fps_rational(fps: f64) -> Option<(u32, u32)> {
    if !fps.is_finite() || fps <= 0.0 || fps > 240.0 {
        return None;
    }
    let denominator = 1_000_u32;
    let numerator = (fps * f64::from(denominator)).round() as u32;
    let divisor = gcd(numerator, denominator);
    Some((numerator / divisor, denominator / divisor))
}

fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

pub(super) fn seconds_to_micros(seconds: f64) -> PipelineResult<u64> {
    if !seconds.is_finite() || seconds < 0.0 || seconds > u64::MAX as f64 / 1_000_000.0 {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "precompose.sourceStart",
            "Prepared-source time is outside the deterministic timestamp range.",
            "Use a finite non-negative prepared-source range.",
        )]);
    }
    Ok((seconds * 1_000_000.0).round() as u64)
}

pub(super) fn project_relative_path(project_dir: &Path, path: &Path) -> PipelineResult<String> {
    let relative = path.strip_prefix(project_dir).map_err(|_| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "precompose.cache.path",
            "Prepared artifact escaped the project directory.",
            "Keep precompose cache artifacts inside the project cache folder.",
        )]
    })?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn precompose_item_error(task: &PreparationTask, message: &str) -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        format!("precompose.items.{}", task.item_id),
        message,
        "Inspect the Lottie source, timing, and precompose cache before retrying.",
    )
    .with_detail("itemId", task.item_id.clone())
    .with_detail("mediaId", task.media_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn prepared_media_spans_include_transition_handles() {
        let mut project = crate::project::fixtures::sample_project();
        project.media[0].duration_seconds = 3.0;
        let item = &mut project.timeline.tracks[0].items[0];
        item.start_seconds = 2.0;
        item.duration_seconds = 2.0;
        item.properties.insert("sourceIn".to_string(), json!(0.5));
        let item = item.clone();
        assert_eq!(prepared_media_timeline_span(&project, &item), (1.5, 3.0));

        let mut plain = item.clone();
        plain.properties.insert("sourceIn".to_string(), json!(0.0));
        project.media[0].duration_seconds = 2.0;
        assert_eq!(prepared_media_timeline_span(&project, &plain), (2.0, 2.0));
    }

    #[test]
    fn canonical_frame_rates_reduce_to_stable_rationals() {
        assert_eq!(fps_rational(30.0), Some((30, 1)));
        assert_eq!(fps_rational(29.97), Some((2997, 100)));
    }

    #[test]
    fn catalog_lut_intensity_uses_numeric_parameter_keyframes() {
        let properties = BTreeMap::from([
            (
                "effects".to_string(),
                json!([{
                    "effectInstanceId":"lut-a",
                    "effectType":"color.lut",
                    "enabled":true,
                    "params":{"path":"looks/invert.cube","intensity":0.0}
                }]),
            ),
            (
                "effectParameterKeyframes".to_string(),
                json!({"lut-a":{"intensity":[
                    {"atSeconds":0.0,"value":0.0,"easing":"linear"},
                    {"atSeconds":2.0,"value":1.0,"easing":"linear"}
                ]}}),
            ),
        ]);
        let stack = PreparedEffectStack::from_item_properties_for_duration(&properties, 2.0)
            .expect("animated LUT stack");
        let intensity_at = |seconds| {
            stack
                .sample(seconds)
                .effects
                .iter()
                .find(|effect| effect.effect_type == "color.lut")
                .and_then(|effect| effect.params.get("intensity"))
                .copied()
                .expect("LUT intensity")
        };
        assert_eq!(intensity_at(0.0), 0.0);
        assert_eq!(intensity_at(1.0), 0.5);
        assert_eq!(intensity_at(2.0), 1.0);
    }
}
