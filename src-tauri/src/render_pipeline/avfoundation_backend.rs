use crate::edit::render_plan::{RenderClip, RenderOutputProfile, RenderPlan, RenderQuality};
use crate::graphics::manifest::{GraphicsArtifactManifest, GraphicsPlaybackMode};
use crate::project::export_profiles::ExportProfile;
use crate::project::source_probe::{probe_source_metadata, SourceMediaType};
use crate::render_pipeline::backend::RenderBackend;
use crate::render_pipeline::cancel::RenderCancellationToken;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use crate::render_pipeline::probe::{AudioProbe, MediaProbe, VideoProbe};
use crate::render_pipeline::process::{CommandSpec, ProcessOutput, ProcessRunner};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const AVFOUNDATION_EXPORT_PROTOCOL: &str = "video-creater.avfoundation-export";
pub const AVFOUNDATION_EXPORT_PROTOCOL_VERSION: u32 = 5;
const BACKEND_NAME: &str = "avfoundation-native";
const EXPORTER_ENV_VAR: &str = "VIDEO_CREATER_AVFOUNDATION_EXPORTER";
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const MAX_STDOUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 128 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AvFoundationRenderBackend;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvFoundationExportRequest {
    pub protocol: String,
    pub schema_version: u32,
    pub request_id: String,
    pub output_path: String,
    pub profile: AvFoundationExportProfile,
    pub quality: RenderQuality,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub video_clips: Vec<AvFoundationVideoClip>,
    pub audio_clips: Vec<AvFoundationAudioClip>,
    pub overlays: Vec<AvFoundationOverlay>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AvFoundationExportProfile {
    H264,
    Hevc,
    ProResProxy,
    ProRes422,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvFoundationVideoClip {
    pub source_path: String,
    pub source_start_seconds: f64,
    pub source_end_seconds: f64,
    pub timeline_start_seconds: f64,
    pub timeline_duration_seconds: f64,
    pub track_index: u32,
    pub opacity: f64,
    pub fade_in_seconds: f64,
    pub fade_out_seconds: f64,
    pub transform: AvFoundationCanvasTransform,
    pub crop: AvFoundationCropInsets,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepared_frames: Option<AvFoundationPreparedFrames>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvFoundationPreparedFrames {
    pub frame_paths: Vec<String>,
    pub frame_duration_seconds: f64,
    pub alpha: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvFoundationAudioClip {
    pub source_path: String,
    pub source_start_seconds: f64,
    pub source_end_seconds: f64,
    pub timeline_start_seconds: f64,
    pub timeline_duration_seconds: f64,
    pub track_index: u32,
    pub volume_db: f64,
    pub fade_in_seconds: f64,
    pub fade_out_seconds: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volume_keyframes: Vec<AvFoundationAudioVolumeKeyframe>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvFoundationAudioVolumeKeyframe {
    pub at_seconds: f64,
    pub value_db: f64,
    pub easing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvFoundationCanvasTransform {
    pub center_x: f64,
    pub center_y: f64,
    pub width: f64,
    pub height: f64,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
}

impl Default for AvFoundationCanvasTransform {
    fn default() -> Self {
        Self {
            center_x: 0.5,
            center_y: 0.5,
            width: 1.0,
            height: 1.0,
            flip_horizontal: false,
            flip_vertical: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvFoundationCropInsets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Default for AvFoundationCropInsets {
    fn default() -> Self {
        Self {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvFoundationOverlay {
    pub frame_paths: Vec<String>,
    pub timeline_start_seconds: f64,
    pub duration_seconds: f64,
    pub frame_duration_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AvFoundationWorkerEvent {
    Progress {
        protocol: String,
        schema_version: u32,
        request_id: Option<String>,
        phase: String,
        completed: u32,
        total: u32,
        message: String,
    },
    Completed {
        protocol: String,
        schema_version: u32,
        request_id: Option<String>,
        result: AvFoundationExportResult,
    },
    Failed {
        protocol: String,
        schema_version: u32,
        request_id: Option<String>,
        error: AvFoundationWorkerError,
    },
}

impl AvFoundationWorkerEvent {
    fn is_final(&self) -> bool {
        matches!(self, Self::Completed { .. } | Self::Failed { .. })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvFoundationExportResult {
    pub output_path: String,
    pub duration_seconds: f64,
    pub size_bytes: u64,
    pub video_codec: String,
    pub audio_codec: Option<String>,
    pub width: u32,
    pub height: u32,
    pub fps: Option<f64>,
    pub video_clip_count: usize,
    pub audio_clip_count: usize,
    pub overlay_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvFoundationWorkerError {
    pub code: String,
    pub message: String,
    pub field: Option<String>,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvFoundationCapabilities {
    pub protocol: String,
    pub schema_version: u32,
    pub backend: String,
    pub profiles: std::collections::BTreeMap<String, bool>,
}

impl AvFoundationCapabilities {
    pub fn supports_profile(&self, profile: AvFoundationExportProfile) -> bool {
        let key = match profile {
            AvFoundationExportProfile::H264 => "h264",
            AvFoundationExportProfile::Hevc => "hevc",
            AvFoundationExportProfile::ProResProxy => "proResProxy",
            AvFoundationExportProfile::ProRes422 => "proRes422",
        };
        self.profiles.get(key).copied().unwrap_or(false)
    }

    pub fn supports_profile_quality(
        &self,
        profile: AvFoundationExportProfile,
        quality: RenderQuality,
    ) -> bool {
        let key = match (profile, quality) {
            (AvFoundationExportProfile::H264, RenderQuality::Draft) => "h264Draft",
            (AvFoundationExportProfile::H264, RenderQuality::Final) => "h264Final",
            (AvFoundationExportProfile::Hevc, RenderQuality::Draft) => "hevcDraft",
            (AvFoundationExportProfile::Hevc, RenderQuality::Final) => "hevcFinal",
            (AvFoundationExportProfile::ProResProxy, _) => "proResProxy",
            (AvFoundationExportProfile::ProRes422, _) => "proRes422",
        };
        self.profiles.get(key).copied().unwrap_or(false)
    }
}

#[derive(Debug)]
pub struct AvFoundationRenderRun {
    pub process_output: ProcessOutput,
    pub result: AvFoundationExportResult,
    pub probe: MediaProbe,
}

#[derive(Debug)]
struct BoundedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

impl AvFoundationRenderBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn backend_name(&self) -> &'static str {
        BACKEND_NAME
    }

    pub fn is_available(&self) -> bool {
        self.capabilities().is_ok()
    }

    pub fn supports_profile(&self, profile: AvFoundationExportProfile) -> bool {
        self.capabilities()
            .ok()
            .map(|capabilities| capabilities.supports_profile(profile))
            .unwrap_or(false)
    }

    pub fn capabilities(&self) -> PipelineResult<AvFoundationCapabilities> {
        let exporter_path = avfoundation_exporter_path()?;
        let output = Command::new(&exporter_path)
            .arg("--capabilities")
            .env_clear()
            .output()
            .map_err(|error| {
                vec![avfoundation_error(
                    "avfoundation.exporter.capabilities",
                    "Bundled AVFoundation exporter capabilities could not be queried.",
                    "Reinstall or rebuild the signed native exporter.",
                )
                .with_detail("exporterPath", exporter_path.display().to_string())
                .with_detail("error", error.to_string())]
            })?;
        if !output.status.success() {
            return Err(vec![avfoundation_error(
                "avfoundation.exporter.capabilities",
                "Bundled AVFoundation exporter rejected its capability probe.",
                "Inspect the exporter signature and system codec availability.",
            )
            .with_detail("statusCode", output.status.code().unwrap_or(-1).to_string())
            .with_detail(
                "stderr",
                String::from_utf8_lossy(&output.stderr).into_owned(),
            )]);
        }
        let capabilities = serde_json::from_slice::<AvFoundationCapabilities>(&output.stdout)
            .map_err(|error| {
                vec![avfoundation_error(
                    "avfoundation.exporter.capabilities",
                    "Bundled AVFoundation exporter emitted invalid capability JSON.",
                    "Keep the capability probe on the versioned native exporter protocol.",
                )
                .with_detail("error", error.to_string())]
            })?;
        if capabilities.protocol != AVFOUNDATION_EXPORT_PROTOCOL
            || capabilities.schema_version != AVFOUNDATION_EXPORT_PROTOCOL_VERSION
            || capabilities.backend != BACKEND_NAME
        {
            return Err(vec![avfoundation_error(
                "avfoundation.exporter.capabilities",
                "Bundled AVFoundation exporter capability protocol is incompatible.",
                "Rebuild the app and exporter from the same source revision.",
            )]);
        }
        Ok(capabilities)
    }

    pub fn build_request(
        &self,
        request_id: &str,
        plan: &RenderPlan,
        graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
    ) -> PipelineResult<AvFoundationExportRequest> {
        validate_plan_header(plan)?;
        super::reverse_guard::reject_reversed_plan_clips(plan)?;
        reject_transitions(plan)?;
        let profile = avfoundation_profile_for_output(plan.output_profile, plan.quality)?;
        let video_clips = plan
            .clips
            .iter()
            .enumerate()
            .map(|(index, clip)| avfoundation_video_clip(plan, clip, index))
            .collect::<PipelineResult<Vec<_>>>()?;
        let audio_clips = plan
            .audio_clips
            .iter()
            .enumerate()
            .map(|(index, clip)| avfoundation_audio_clip(plan, clip, index))
            .collect::<PipelineResult<Vec<_>>>()?;
        let overlays = graphics
            .iter()
            .enumerate()
            .map(|(index, (manifest, artifact_dir, start))| {
                avfoundation_overlay(manifest, artifact_dir, *start, index)
            })
            .collect::<PipelineResult<Vec<_>>>()?;
        let request = AvFoundationExportRequest {
            protocol: AVFOUNDATION_EXPORT_PROTOCOL.to_string(),
            schema_version: AVFOUNDATION_EXPORT_PROTOCOL_VERSION,
            request_id: request_id.to_string(),
            output_path: plan.output_path.clone(),
            profile,
            quality: plan.quality,
            width: plan.width,
            height: plan.height,
            fps: plan.fps,
            video_clips,
            audio_clips,
            overlays,
        };
        validate_request(&request)?;
        Ok(request)
    }

    pub fn render_cancellable(
        &self,
        request_id: &str,
        plan: &RenderPlan,
        graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
        timeout: Duration,
        cancellation: Option<&RenderCancellationToken>,
    ) -> PipelineResult<AvFoundationRenderRun> {
        let request = self.build_request(request_id, plan, graphics)?;
        run_avfoundation_exporter(&request, timeout, || {
            cancellation.is_some_and(RenderCancellationToken::is_cancelled)
        })
    }
}

impl RenderBackend for AvFoundationRenderBackend {
    fn build_command(
        &self,
        plan: &RenderPlan,
        graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
    ) -> PipelineResult<CommandSpec> {
        let _ = self.build_request("render-report", plan, graphics)?;
        Ok(CommandSpec::new(BACKEND_NAME)
            .arg(format!("--output={}", plan.output_path))
            .arg(format!("--profile={:?}", plan.output_profile))
            .arg(format!("--clips={}", plan.clips.len()))
            .arg(format!("--audio-clips={}", plan.audio_clips.len()))
            .arg(format!("--overlays={}", graphics.len())))
    }

    fn render(
        &self,
        _runner: &dyn ProcessRunner,
        plan: &RenderPlan,
        graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
        timeout: Duration,
    ) -> PipelineResult<ProcessOutput> {
        self.render_cancellable("render", plan, graphics, timeout, None)
            .map(|run| run.process_output)
    }
}

fn validate_plan_header(plan: &RenderPlan) -> PipelineResult<()> {
    let mut errors = Vec::new();
    if plan.clips.is_empty() {
        errors.push(avfoundation_error(
            "renderPlan.clips",
            "AVFoundation export requires at least one video clip.",
            "Add a source-backed video clip before exporting.",
        ));
    }
    if plan.width == 0 || plan.height == 0 || plan.width > 16_384 || plan.height > 16_384 {
        errors.push(avfoundation_error(
            "renderPlan.dimensions",
            "AVFoundation export dimensions are outside the supported range.",
            "Use positive render dimensions no larger than 16384 pixels per axis.",
        ));
    }
    if !plan.fps.is_finite() || !(0.0..=240.0).contains(&plan.fps) || plan.fps == 0.0 {
        errors.push(avfoundation_error(
            "renderPlan.fps",
            "AVFoundation export frame rate is invalid.",
            "Use a finite frame rate greater than zero and no more than 240 fps.",
        ));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn avfoundation_profile(
    profile: ExportProfile,
    quality: RenderQuality,
) -> AvFoundationExportProfile {
    match profile {
        ExportProfile::Mp4H264 => AvFoundationExportProfile::H264,
        ExportProfile::Mp4H265 => AvFoundationExportProfile::Hevc,
        ExportProfile::ProResMov if quality == RenderQuality::Draft => {
            AvFoundationExportProfile::ProResProxy
        }
        ExportProfile::ProResMov => AvFoundationExportProfile::ProRes422,
        ExportProfile::Webm | ExportProfile::PalmierProject => {
            unreachable!("only AVFoundation delivery profiles can select an AVFoundation profile")
        }
    }
}

fn avfoundation_profile_for_output(
    profile: RenderOutputProfile,
    quality: RenderQuality,
) -> PipelineResult<AvFoundationExportProfile> {
    match profile {
        RenderOutputProfile::Mp4Primary => {
            Ok(avfoundation_profile(ExportProfile::Mp4H264, quality))
        }
        RenderOutputProfile::Mp4Modern => Ok(avfoundation_profile(ExportProfile::Mp4H265, quality)),
        RenderOutputProfile::QuicktimeInterchange => {
            Ok(avfoundation_profile(ExportProfile::ProResMov, quality))
        }
        RenderOutputProfile::WebPreview | RenderOutputProfile::WebDelivery => {
            Err(vec![avfoundation_error(
                "renderPlan.outputProfile",
                "AVFoundation backend supports H.264, HEVC, and ProRes delivery profiles only.",
                "Use the GStreamer backend for WebM preview or delivery exports.",
            )])
        }
    }
}

fn avfoundation_video_clip(
    plan: &RenderPlan,
    clip: &RenderClip,
    index: usize,
) -> PipelineResult<AvFoundationVideoClip> {
    reject_unsupported_video_properties(clip, index)?;
    let source_path = clip
        .source_path
        .clone()
        .unwrap_or_else(|| plan.input_path.clone());
    let prepared_frames = prepared_frames(clip, index)?;
    if prepared_frames.is_none() {
        require_avfoundation_video_source(&source_path, index)?;
    }
    let timeline_duration_seconds = timeline_duration(clip, index, "videoClips")?;
    let timeline_start_seconds = clip.timeline_start_seconds.unwrap_or(0.0);
    let opacity = optional_number(clip, "opacity", 1.0, index, "videoClips")?;
    if !(0.0..=1.0).contains(&opacity) {
        return Err(vec![avfoundation_error(
            format!("renderPlan.videoClips[{index}].properties.opacity"),
            "AVFoundation clip opacity must be between zero and one.",
            "Use a normalized opacity value.",
        )]);
    }
    let fade_in_seconds = optional_number(clip, "fadeInSeconds", 0.0, index, "videoClips")?;
    let fade_out_seconds = optional_number(clip, "fadeOutSeconds", 0.0, index, "videoClips")?;
    if fade_in_seconds < 0.0
        || fade_out_seconds < 0.0
        || fade_in_seconds + fade_out_seconds > timeline_duration_seconds
    {
        return Err(vec![avfoundation_error(
            format!("renderPlan.videoClips[{index}].properties.fades"),
            "AVFoundation clip fades must fit inside the clip duration.",
            "Reduce fade-in or fade-out duration and retry.",
        )]);
    }
    let transform = canvas_transform(clip, index)?;
    let crop = crop_insets(clip, index)?;
    validate_clip_range(
        &source_path,
        clip.source_in,
        clip.source_out,
        timeline_start_seconds,
        timeline_duration_seconds,
        index,
        "videoClips",
    )?;
    Ok(AvFoundationVideoClip {
        source_path,
        source_start_seconds: clip.source_in,
        source_end_seconds: clip.source_out,
        timeline_start_seconds,
        timeline_duration_seconds,
        track_index: clip.timeline_track_index,
        opacity,
        fade_in_seconds,
        fade_out_seconds,
        transform,
        crop,
        prepared_frames,
    })
}

fn prepared_frames(
    clip: &RenderClip,
    index: usize,
) -> PipelineResult<Option<AvFoundationPreparedFrames>> {
    let Some(value) = clip.properties.get("preparedFrames") else {
        return Ok(None);
    };
    let object = value.as_object().ok_or_else(|| {
        vec![avfoundation_error(
            format!("renderPlan.videoClips[{index}].properties.preparedFrames"),
            "Prepared frame metadata must be an object.",
            "Regenerate the canonical prepared visual artifact.",
        )]
    })?;
    let frame_paths = object
        .get("framePaths")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let frame_duration_seconds = object
        .get("frameDurationSeconds")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let alpha = object.get("alpha").and_then(Value::as_bool).unwrap_or(true);
    if frame_paths.is_empty()
        || frame_paths
            .iter()
            .any(|path| !Path::new(path).is_absolute())
        || !frame_duration_seconds.is_finite()
        || frame_duration_seconds <= 0.0
    {
        return Err(vec![avfoundation_error(
            format!("renderPlan.videoClips[{index}].properties.preparedFrames"),
            "Prepared frame paths/timing are invalid.",
            "Use a non-empty absolute PNG frame sequence with positive timing.",
        )]);
    }
    Ok(Some(AvFoundationPreparedFrames {
        frame_paths,
        frame_duration_seconds,
        alpha,
    }))
}

fn avfoundation_audio_clip(
    plan: &RenderPlan,
    clip: &RenderClip,
    index: usize,
) -> PipelineResult<AvFoundationAudioClip> {
    reject_unsupported_audio_properties(clip, index)?;
    let source_path = clip
        .source_path
        .clone()
        .unwrap_or_else(|| plan.input_path.clone());
    require_avfoundation_audio_source(&source_path, index)?;
    let timeline_duration_seconds = timeline_duration(clip, index, "audioClips")?;
    let timeline_start_seconds = clip.timeline_start_seconds.unwrap_or(0.0);
    let volume_db = optional_number(clip, "volumeDb", 0.0, index, "audioClips")?;
    if !(-60.0..=24.0).contains(&volume_db) {
        return Err(vec![avfoundation_error(
            format!("renderPlan.audioClips[{index}].properties.volumeDb"),
            "AVFoundation audio gain must be between -60 dB and 24 dB.",
            "Use a supported finite audio gain.",
        )]);
    }
    let fade_in_seconds = optional_number(clip, "fadeInSeconds", 0.0, index, "audioClips")?;
    let fade_out_seconds = optional_number(clip, "fadeOutSeconds", 0.0, index, "audioClips")?;
    if fade_in_seconds < 0.0
        || fade_out_seconds < 0.0
        || fade_in_seconds + fade_out_seconds > timeline_duration_seconds
    {
        return Err(vec![avfoundation_error(
            format!("renderPlan.audioClips[{index}].properties.fades"),
            "AVFoundation audio fades must fit inside the clip duration.",
            "Reduce fade-in or fade-out duration and retry.",
        )]);
    }
    validate_clip_range(
        &source_path,
        clip.source_in,
        clip.source_out,
        timeline_start_seconds,
        timeline_duration_seconds,
        index,
        "audioClips",
    )?;
    let volume_keyframes = audio_volume_keyframes(clip, timeline_duration_seconds, index)?;
    Ok(AvFoundationAudioClip {
        source_path,
        source_start_seconds: clip.source_in,
        source_end_seconds: clip.source_out,
        timeline_start_seconds,
        timeline_duration_seconds,
        track_index: clip.timeline_track_index,
        volume_db,
        fade_in_seconds,
        fade_out_seconds,
        volume_keyframes,
    })
}

/// AVFoundation v1 composes clips with hard cuts only. Plans with transitions
/// extend clips into overlapping handles, which the native exporter would
/// render as a wrong cut, so they fail here and export selection uses GES.
fn reject_transitions(plan: &RenderPlan) -> PipelineResult<()> {
    let (path, count) = if !plan.transitions.is_empty() {
        ("renderPlan.transitions", plan.transitions.len())
    } else if !plan.audio_transitions.is_empty() {
        ("renderPlan.audioTransitions", plan.audio_transitions.len())
    } else {
        return Ok(());
    };
    Err(vec![avfoundation_error(
        path,
        "AVFoundation export does not support clip transitions.",
        "Render this export with the GStreamer backend, which export selection uses for projects with transitions.",
    )
    .with_detail("transitionCount", count.to_string())])
}

fn reject_unsupported_video_properties(clip: &RenderClip, index: usize) -> PipelineResult<()> {
    const UNSUPPORTED: &[&str] = &[
        "colorGrade",
        "effects",
        "keyframes",
        "rotationDegrees",
        "positionX",
        "positionY",
        "scale",
        "scaleX",
        "scaleY",
    ];
    if let Some(property) = UNSUPPORTED.iter().find(|property| {
        clip.properties
            .get(**property)
            .is_some_and(|value| !value.is_null() && !empty_json(value))
    }) {
        return Err(vec![avfoundation_error(
            format!("renderPlan.videoClips[{index}].properties.{property}"),
            format!("AVFoundation v1 does not support `{property}` on an unprepared clip."),
            "Bake this finishing operation into the prepared project or use the reviewed GStreamer backend.",
        )]);
    }
    if let Some(mode) = clip.properties.get("blendMode").and_then(Value::as_str) {
        if !matches!(mode, "over" | "normal" | "source") {
            return Err(vec![avfoundation_error(
                format!("renderPlan.videoClips[{index}].properties.blendMode"),
                format!(
                    "AVFoundation v1 does not support `{mode}` blending on an unprepared clip."
                ),
                "Prepare the richer blend into a canonical intermediate before native export.",
            )]);
        }
    }
    Ok(())
}

fn reject_unsupported_audio_properties(clip: &RenderClip, index: usize) -> PipelineResult<()> {
    if clip
        .properties
        .get("speed")
        .and_then(serde_json::Value::as_f64)
        .is_some_and(|speed| (speed - 1.0).abs() > 0.000_001)
    {
        return Err(vec![avfoundation_error(
            format!("renderPlan.audioClips[{index}].properties.speed"),
            "AVFoundation export does not retime audio clips.",
            "Prepare the retimed audio first or render with the reviewed GStreamer backend.",
        )]);
    }
    for property in ["effects"] {
        if clip
            .properties
            .get(property)
            .is_some_and(|value| !value.is_null() && !empty_json(value))
        {
            return Err(vec![avfoundation_error(
                format!("renderPlan.audioClips[{index}].properties.{property}"),
                format!("AVFoundation v1 does not support audio `{property}` on an unprepared clip."),
                "Bake the audio operation into a prepared asset or use the reviewed GStreamer backend.",
            )]);
        }
    }
    Ok(())
}

fn audio_volume_keyframes(
    clip: &RenderClip,
    duration_seconds: f64,
    index: usize,
) -> PipelineResult<Vec<AvFoundationAudioVolumeKeyframe>> {
    let Some(keyframes) = clip.properties.get("keyframes") else {
        return Ok(Vec::new());
    };
    let lanes = keyframes.as_object().ok_or_else(|| {
        vec![avfoundation_error(
            format!("renderPlan.audioClips[{index}].properties.keyframes"),
            "Audio keyframes must be an object of named lanes.",
            "Use a canonical volumeDb keyframe lane.",
        )]
    })?;
    if let Some((property, _)) = lanes
        .iter()
        .find(|(property, value)| property.as_str() != "volumeDb" && !empty_json(value))
    {
        return Err(vec![avfoundation_error(
            format!("renderPlan.audioClips[{index}].properties.keyframes.{property}"),
            format!("AVFoundation does not support audio `{property}` automation."),
            "Use the canonical volumeDb automation lane.",
        )]);
    }
    let Some(lane) = lanes.get("volumeDb") else {
        return Ok(Vec::new());
    };
    let points = lane.as_array().ok_or_else(|| {
        vec![avfoundation_error(
            format!("renderPlan.audioClips[{index}].properties.keyframes.volumeDb"),
            "Audio volume automation must be an array.",
            "Use ordered canonical keyframes with atSeconds, value, and easing.",
        )]
    })?;
    if points.len() > 10_000 {
        return Err(vec![avfoundation_error(
            format!("renderPlan.audioClips[{index}].properties.keyframes.volumeDb"),
            "Audio volume automation exceeds the 10,000-point safety limit.",
            "Simplify the automation lane and retry.",
        )]);
    }
    let mut parsed = Vec::with_capacity(points.len());
    let mut previous = None;
    for (point_index, point) in points.iter().enumerate() {
        let object = point.as_object().ok_or_else(|| {
            vec![avfoundation_error(
                format!(
                    "renderPlan.audioClips[{index}].properties.keyframes.volumeDb[{point_index}]"
                ),
                "Audio volume keyframe must be an object.",
                "Use atSeconds, value, and easing fields.",
            )]
        })?;
        let at_seconds = object.get("atSeconds").and_then(Value::as_f64);
        let value_db = object.get("value").and_then(Value::as_f64);
        let easing = object
            .get("easing")
            .and_then(Value::as_str)
            .unwrap_or("linear");
        let canonical_easing = match easing {
            "linear" | "hold" | "easeIn" | "easeOut" | "easeInOut" => easing,
            "smooth" => "easeInOut",
            _ => "",
        };
        let valid = at_seconds.is_some_and(|value| {
            value.is_finite()
                && value >= 0.0
                && value <= duration_seconds
                && previous.is_none_or(|previous| value > previous)
        }) && value_db
            .is_some_and(|value| value.is_finite() && (-60.0..=24.0).contains(&value))
            && !canonical_easing.is_empty();
        if !valid {
            return Err(vec![avfoundation_error(
                format!("renderPlan.audioClips[{index}].properties.keyframes.volumeDb[{point_index}]"),
                "Audio volume keyframe is invalid or out of order.",
                "Keep times inside the clip, gains between -60 and 24 dB, and use a supported easing.",
            )]);
        }
        let at_seconds = at_seconds.expect("validated audio keyframe time");
        previous = Some(at_seconds);
        parsed.push(AvFoundationAudioVolumeKeyframe {
            at_seconds,
            value_db: value_db.expect("validated audio keyframe gain"),
            easing: canonical_easing.to_string(),
        });
    }
    Ok(parsed)
}

fn empty_json(value: &Value) -> bool {
    matches!(value, Value::Array(values) if values.is_empty())
        || matches!(value, Value::Object(values) if values.is_empty())
}

fn timeline_duration(clip: &RenderClip, index: usize, label: &str) -> PipelineResult<f64> {
    let duration = optional_number(
        clip,
        "timelineDurationSeconds",
        clip.source_out - clip.source_in,
        index,
        label,
    )?;
    if duration <= 0.0 {
        return Err(vec![avfoundation_error(
            format!("renderPlan.{label}[{index}].properties.timelineDurationSeconds"),
            "AVFoundation clip timeline duration must be positive.",
            "Use a positive timeline duration after trim and speed mapping.",
        )]);
    }
    Ok(duration)
}

fn optional_number(
    clip: &RenderClip,
    property: &str,
    fallback: f64,
    index: usize,
    label: &str,
) -> PipelineResult<f64> {
    let Some(value) = clip.properties.get(property) else {
        return Ok(fallback);
    };
    value
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| {
            vec![avfoundation_error(
                format!("renderPlan.{label}[{index}].properties.{property}"),
                format!("AVFoundation `{property}` must be a finite number."),
                "Use a finite numeric value and retry.",
            )]
        })
}

fn canvas_transform(
    clip: &RenderClip,
    index: usize,
) -> PipelineResult<AvFoundationCanvasTransform> {
    let Some(value) = clip.properties.get("transform") else {
        return Ok(AvFoundationCanvasTransform::default());
    };
    let object = value.as_object().ok_or_else(|| {
        vec![avfoundation_error(
            format!("renderPlan.videoClips[{index}].properties.transform"),
            "AVFoundation canvas transform must be an object.",
            "Use normalized center, dimensions, and optional flip flags.",
        )]
    })?;
    let number = |key: &str, fallback: f64, minimum: f64| -> PipelineResult<f64> {
        let value = object.get(key).map_or(Some(fallback), Value::as_f64);
        value
            .filter(|value| value.is_finite() && (minimum..=1.0).contains(value))
            .ok_or_else(|| {
                vec![avfoundation_error(
                    format!("renderPlan.videoClips[{index}].properties.transform.{key}"),
                    "AVFoundation transform values must use normalized canvas coordinates.",
                    "Use center values from 0 through 1 and positive dimensions through 1.",
                )]
            })
    };
    let bool_value = |key: &str| -> PipelineResult<bool> {
        object.get(key).map_or(Ok(false), |value| {
            value.as_bool().ok_or_else(|| {
                vec![avfoundation_error(
                    format!("renderPlan.videoClips[{index}].properties.transform.{key}"),
                    "AVFoundation flip flags must be boolean.",
                    "Use true or false for visual flip controls.",
                )]
            })
        })
    };
    Ok(AvFoundationCanvasTransform {
        center_x: number("centerX", 0.5, 0.0)?,
        center_y: number("centerY", 0.5, 0.0)?,
        width: number("width", 1.0, f64::MIN_POSITIVE)?,
        height: number("height", 1.0, f64::MIN_POSITIVE)?,
        flip_horizontal: bool_value("flipHorizontal")?,
        flip_vertical: bool_value("flipVertical")?,
    })
}

fn crop_insets(clip: &RenderClip, index: usize) -> PipelineResult<AvFoundationCropInsets> {
    let crop = AvFoundationCropInsets {
        top: optional_number(clip, "cropTop", 0.0, index, "videoClips")?,
        right: optional_number(clip, "cropRight", 0.0, index, "videoClips")?,
        bottom: optional_number(clip, "cropBottom", 0.0, index, "videoClips")?,
        left: optional_number(clip, "cropLeft", 0.0, index, "videoClips")?,
    };
    if [crop.top, crop.right, crop.bottom, crop.left]
        .iter()
        .any(|value| !(0.0..1.0).contains(value))
        || crop.left + crop.right >= 1.0
        || crop.top + crop.bottom >= 1.0
    {
        return Err(vec![avfoundation_error(
            format!("renderPlan.videoClips[{index}].properties.crop"),
            "AVFoundation crop insets must leave visible normalized source content.",
            "Keep every crop inset from zero inclusive to one exclusive and each opposing pair below one.",
        )]);
    }
    Ok(crop)
}

fn validate_clip_range(
    source_path: &str,
    source_start: f64,
    source_end: f64,
    timeline_start: f64,
    timeline_duration: f64,
    index: usize,
    label: &str,
) -> PipelineResult<()> {
    if source_path.trim().is_empty()
        || !source_start.is_finite()
        || source_start < 0.0
        || !source_end.is_finite()
        || source_end <= source_start
        || !timeline_start.is_finite()
        || timeline_start < 0.0
        || !timeline_duration.is_finite()
        || timeline_duration <= 0.0
    {
        return Err(vec![avfoundation_error(
            format!("renderPlan.{label}[{index}]"),
            "AVFoundation clip source or timeline range is invalid.",
            "Use a non-empty source path and finite positive source/timeline ranges.",
        )]);
    }
    Ok(())
}

fn require_avfoundation_video_source(source_path: &str, index: usize) -> PipelineResult<()> {
    let extension = Path::new(source_path)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    if matches!(
        extension.as_deref(),
        Some(
            "mp4"
                | "mov"
                | "m4v"
                | "png"
                | "jpg"
                | "jpeg"
                | "webp"
                | "heic"
                | "heif"
                | "tif"
                | "tiff"
                | "bmp"
        )
    ) {
        return Ok(());
    }
    if probe_source_metadata(Path::new(source_path)).is_ok_and(|metadata| {
        matches!(
            metadata.media_type,
            SourceMediaType::Video | SourceMediaType::Image
        )
    }) {
        return Ok(());
    }
    Err(vec![avfoundation_error(
        format!("renderPlan.videoClips[{index}].sourcePath"),
        "AVFoundation cannot decode this visual source container.",
        "Prepare a system-supported movie or still image, or use the reviewed compatibility backend for this render.",
    )
    .with_detail("sourcePath", source_path.to_string())])
}

fn require_avfoundation_audio_source(source_path: &str, index: usize) -> PipelineResult<()> {
    let extension = Path::new(source_path)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    if matches!(
        extension.as_deref(),
        Some("mp4" | "mov" | "m4v" | "m4a" | "wav" | "aif" | "aiff" | "caf")
    ) {
        return Ok(());
    }
    if probe_source_metadata(Path::new(source_path)).is_ok_and(|metadata| {
        matches!(
            metadata.media_type,
            SourceMediaType::Audio | SourceMediaType::Video
        )
    }) {
        return Ok(());
    }
    Err(vec![avfoundation_error(
        format!("renderPlan.audioClips[{index}].sourcePath"),
        "AVFoundation v1 cannot decode this audio source container.",
        "Prepare a system-supported audio source or use the reviewed GStreamer backend for this render.",
    )
    .with_detail("sourcePath", source_path.to_string())])
}

fn avfoundation_overlay(
    manifest: &GraphicsArtifactManifest,
    artifact_dir: &Path,
    timeline_start_seconds: f64,
    index: usize,
) -> PipelineResult<AvFoundationOverlay> {
    if !manifest.alpha || manifest.frame_count == 0 {
        return Err(vec![avfoundation_error(
            format!("renderPlan.overlays[{index}]"),
            "AVFoundation overlay must contain at least one alpha frame.",
            "Regenerate the graphics artifact as a transparent frame sequence.",
        )]);
    }
    let relative_frames = if manifest.playback.expected_frames.is_empty() {
        (0..manifest.frame_count)
            .map(|frame| frame_path_from_pattern(&manifest.frames_pattern, frame))
            .collect::<Vec<_>>()
    } else {
        manifest.playback.expected_frames.clone()
    };
    let frame_paths = relative_frames
        .iter()
        .map(|path| artifact_dir.join(path).display().to_string())
        .collect::<Vec<_>>();
    let frame_duration_seconds = match manifest.playback.mode {
        GraphicsPlaybackMode::Sequence => manifest.playback.frame_duration_seconds,
        GraphicsPlaybackMode::StaticHold => manifest.duration_seconds,
    };
    if !timeline_start_seconds.is_finite()
        || timeline_start_seconds < 0.0
        || !manifest.duration_seconds.is_finite()
        || manifest.duration_seconds <= 0.0
        || !frame_duration_seconds.is_finite()
        || frame_duration_seconds <= 0.0
    {
        return Err(vec![avfoundation_error(
            format!("renderPlan.overlays[{index}]"),
            "AVFoundation overlay timing is invalid.",
            "Use finite positive overlay timing inside the render duration.",
        )]);
    }
    Ok(AvFoundationOverlay {
        frame_paths,
        timeline_start_seconds,
        duration_seconds: manifest.duration_seconds,
        frame_duration_seconds,
    })
}

fn frame_path_from_pattern(pattern: &str, frame_index: u32) -> PathBuf {
    let six = format!("{frame_index:06}");
    let five = format!("{frame_index:05}");
    let four = format!("{frame_index:04}");
    let raw = frame_index.to_string();
    PathBuf::from(
        pattern
            .replace("%06d", &six)
            .replace("%05d", &five)
            .replace("%04d", &four)
            .replace("%d", &raw),
    )
}

fn validate_request(request: &AvFoundationExportRequest) -> PipelineResult<()> {
    if request.protocol != AVFOUNDATION_EXPORT_PROTOCOL
        || request.schema_version != AVFOUNDATION_EXPORT_PROTOCOL_VERSION
        || request.request_id.trim().is_empty()
        || request.output_path.trim().is_empty()
        || request.video_clips.is_empty()
    {
        return Err(vec![avfoundation_error(
            "avfoundation.request",
            "AVFoundation export request failed protocol validation.",
            "Use the current protocol version, non-empty IDs/paths, and at least one video clip.",
        )]);
    }
    Ok(())
}

fn run_avfoundation_exporter(
    request: &AvFoundationExportRequest,
    timeout: Duration,
    is_cancelled: impl Fn() -> bool,
) -> PipelineResult<AvFoundationRenderRun> {
    let exporter_path = avfoundation_exporter_path()?;
    let mut command = Command::new(&exporter_path);
    command
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);
    if let Some(tmpdir) = std::env::var_os("TMPDIR") {
        command.env("TMPDIR", tmpdir);
    }
    let mut child = command.spawn().map_err(|error| {
        vec![avfoundation_error(
            "avfoundation.exporter",
            "Bundled AVFoundation exporter could not be started.",
            "Bundle video-creater-avfoundation-exporter beside the app or set VIDEO_CREATER_AVFOUNDATION_EXPORTER.",
        )
        .with_detail("exporterPath", exporter_path.display().to_string())
        .with_detail("error", error.to_string())]
    })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| exporter_pipe_error("stdin"))?;
    serde_json::to_writer(&mut stdin, request).map_err(|error| {
        vec![avfoundation_error(
            "avfoundation.exporter.request",
            "AVFoundation export request could not be serialized.",
            "Validate the native export protocol request and retry.",
        )
        .with_detail("error", error.to_string())]
    })?;
    stdin.write_all(b"\n").map_err(|error| {
        vec![avfoundation_error(
            "avfoundation.exporter.stdin",
            "AVFoundation export request could not be written to the exporter.",
            "Retry with a healthy bundled exporter.",
        )
        .with_detail("error", error.to_string())]
    })?;
    drop(stdin);

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| exporter_pipe_error("stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| exporter_pipe_error("stderr"))?;
    let stdout_reader = thread::spawn(move || read_bounded(stdout, MAX_STDOUT_BYTES));
    let stderr_reader = thread::spawn(move || read_bounded(stderr, MAX_STDERR_BYTES));
    let started = Instant::now();
    let status = loop {
        if is_cancelled() {
            terminate_exporter(&mut child);
            let _ = child.wait();
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "avfoundation.exporter.cancelled",
                "AVFoundation export was cancelled.",
                "Restart the render when export should continue.",
            )]);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                terminate_exporter(&mut child);
                let _ = child.wait();
                return Err(vec![PipelineError::new(
                    PipelineErrorCode::RenderBackendTimeout,
                    "avfoundation.exporter.timeout",
                    "AVFoundation exporter exceeded the render timeout.",
                    "Reduce render complexity or inspect exporter diagnostics.",
                )
                .with_detail("timeoutMs", timeout.as_millis().to_string())]);
            }
            Err(error) => {
                terminate_exporter(&mut child);
                return Err(vec![avfoundation_error(
                    "avfoundation.exporter.status",
                    "AVFoundation exporter status could not be read.",
                    "Retry with a healthy bundled exporter.",
                )
                .with_detail("error", error.to_string())]);
            }
        }
    };
    let stdout = stdout_reader.join().unwrap_or(BoundedOutput {
        bytes: Vec::new(),
        truncated: true,
    });
    let stderr = stderr_reader.join().unwrap_or(BoundedOutput {
        bytes: Vec::new(),
        truncated: true,
    });
    if stdout.truncated {
        return Err(vec![avfoundation_error(
            "avfoundation.exporter.stdoutBudget",
            "AVFoundation exporter exceeded the stdout protocol budget.",
            "Keep exporter stdout restricted to bounded NDJSON protocol records.",
        )]);
    }
    let stdout_text = String::from_utf8_lossy(&stdout.bytes).into_owned();
    let mut stderr_text = String::from_utf8_lossy(&stderr.bytes).into_owned();
    if stderr.truncated {
        stderr_text.push_str("\n[stderr truncated]");
    }
    let events = stdout_text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(line_index, line)| {
            serde_json::from_str::<AvFoundationWorkerEvent>(line).map_err(|error| {
                vec![avfoundation_error(
                    "avfoundation.exporter.stdout",
                    "AVFoundation exporter emitted an invalid protocol event.",
                    "Keep exporter stdout restricted to versioned NDJSON records.",
                )
                .with_detail("line", (line_index + 1).to_string())
                .with_detail("error", error.to_string())]
            })
        })
        .collect::<PipelineResult<Vec<_>>>()?;
    let final_events = events
        .iter()
        .filter(|event| event.is_final())
        .collect::<Vec<_>>();
    if final_events.len() != 1 {
        return Err(vec![avfoundation_error(
            "avfoundation.exporter.events",
            "AVFoundation exporter did not emit exactly one final protocol event.",
            "Inspect the bundled exporter and retry.",
        )
        .with_detail("finalEventCount", final_events.len().to_string())
        .with_detail("stderr", stderr_text)]);
    }
    match final_events[0] {
        AvFoundationWorkerEvent::Completed { result, .. } if status.success() => {
            if result.output_path != request.output_path {
                return Err(vec![avfoundation_error(
                    "avfoundation.exporter.outputPath",
                    "AVFoundation exporter completed with an unexpected output path.",
                    "Keep exporter output inside the Rust-owned render artifact path.",
                )
                .with_detail("expected", request.output_path.clone())
                .with_detail("actual", result.output_path.clone())]);
            }
            let process_output = ProcessOutput {
                status_code: status.code(),
                stdout: stdout_text,
                stderr: stderr_text,
            };
            let probe = MediaProbe {
                container_name: Some(
                    match request.profile {
                        AvFoundationExportProfile::H264 | AvFoundationExportProfile::Hevc => "mp4",
                        AvFoundationExportProfile::ProResProxy
                        | AvFoundationExportProfile::ProRes422 => "mov",
                    }
                    .to_string(),
                ),
                duration_seconds: Some(result.duration_seconds),
                size_bytes: Some(result.size_bytes),
                video: Some(VideoProbe {
                    codec_name: Some(normalized_video_codec(&result.video_codec).to_string()),
                    width: Some(result.width),
                    height: Some(result.height),
                    fps: result.fps,
                }),
                audio: result.audio_codec.as_deref().map(|codec| AudioProbe {
                    codec_name: Some(normalized_audio_codec(codec).to_string()),
                }),
            };
            Ok(AvFoundationRenderRun {
                process_output,
                result: result.clone(),
                probe,
            })
        }
        AvFoundationWorkerEvent::Failed { error, .. } => Err(vec![avfoundation_error(
            error
                .field
                .as_deref()
                .unwrap_or("avfoundation.exporter.render"),
            error.message.clone(),
            "Inspect the native export request and source media before retrying.",
        )
        .with_detail("workerErrorCode", error.code.clone())
        .with_detail("retryable", error.retryable.to_string())
        .with_detail("stderr", stderr_text)]),
        _ => Err(vec![avfoundation_error(
            "avfoundation.exporter.statusCode",
            "AVFoundation exporter completion disagreed with its process status.",
            "Inspect the bundled exporter and protocol output.",
        )
        .with_detail("statusCode", status.code().unwrap_or(-1).to_string())]),
    }
}

fn normalized_video_codec(codec: &str) -> &str {
    match codec.trim() {
        "avc1" | "avc3" => "h264",
        "hvc1" | "hev1" => "hevc",
        "apcn" | "apch" | "apcs" | "apco" | "ap4h" | "ap4x" => "prores",
        other => other,
    }
}

fn normalized_audio_codec(codec: &str) -> &str {
    match codec.trim() {
        "aac " | "mp4a" => "aac",
        "lpcm" => "pcm",
        other => other,
    }
}

fn avfoundation_exporter_path() -> PipelineResult<PathBuf> {
    if let Some(path) = std::env::var_os(EXPORTER_ENV_VAR) {
        return Ok(PathBuf::from(path));
    }
    let current = std::env::current_exe().map_err(|error| {
        vec![avfoundation_error(
            "avfoundation.exporter.path",
            "Current executable path could not be resolved.",
            "Set VIDEO_CREATER_AVFOUNDATION_EXPORTER to the bundled exporter path.",
        )
        .with_detail("error", error.to_string())]
    })?;
    let mut binary_dir = current.parent().unwrap_or_else(|| Path::new("."));
    if binary_dir.file_name().and_then(|value| value.to_str()) == Some("deps") {
        binary_dir = binary_dir.parent().unwrap_or(binary_dir);
    }
    Ok(binary_dir.join("video-creater-avfoundation-exporter"))
}

fn read_bounded(mut reader: impl Read, limit: usize) -> BoundedOutput {
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    let mut buffer = [0_u8; 8 * 1024];
    let mut truncated = false;
    while let Ok(read) = reader.read(&mut buffer) {
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(bytes.len());
        let retained = remaining.min(read);
        bytes.extend_from_slice(&buffer[..retained]);
        truncated |= retained < read;
    }
    BoundedOutput { bytes, truncated }
}

fn terminate_exporter(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        if let Ok(pid) = i32::try_from(child.id()) {
            let _ = unsafe { kill(-pid, 9) };
        }
    }
    let _ = child.kill();
}

fn exporter_pipe_error(pipe: &str) -> Vec<PipelineError> {
    vec![avfoundation_error(
        format!("avfoundation.exporter.{pipe}"),
        "AVFoundation exporter pipe could not be opened.",
        "Retry with a healthy bundled exporter.",
    )]
}

fn avfoundation_error(
    path: impl Into<String>,
    message: impl Into<String>,
    fix: impl Into<String>,
) -> PipelineError {
    PipelineError::new(PipelineErrorCode::RenderBackendFailed, path, message, fix)
        .with_detail("backend", BACKEND_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::render_plan::{RenderOutputProfile, RenderQuality};
    use crate::graphics::ir::Dimensions;
    use crate::graphics::manifest::{GraphicsPlaybackManifest, GraphicsPlaybackMode};
    #[cfg(target_os = "macos")]
    use crate::render_pipeline::cancel::{
        register_render_attempt, request_render_cancellation, RenderAttemptKey,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    fn plan() -> RenderPlan {
        RenderPlan {
            input_path: "/tmp/input.mov".to_string(),
            output_path: "/tmp/output.mp4".to_string(),
            width: 1920,
            height: 1080,
            fps: 30.0,
            quality: RenderQuality::Final,
            output_profile: RenderOutputProfile::Mp4Primary,
            encode_tier: crate::edit::render_plan::ExportEncodeTier::Standard,
            clips: vec![RenderClip {
                source_path: Some("/tmp/input.mov".to_string()),
                timeline_start_seconds: Some(1.0),
                properties: BTreeMap::from([
                    ("timelineDurationSeconds".to_string(), json!(2.0)),
                    ("opacity".to_string(), json!(0.8)),
                    ("fadeInSeconds".to_string(), json!(0.25)),
                    ("fadeOutSeconds".to_string(), json!(0.25)),
                    (
                        "transform".to_string(),
                        json!({
                            "centerX": 0.5,
                            "centerY": 0.5,
                            "width": 0.8,
                            "height": 0.8,
                            "flipHorizontal": false,
                            "flipVertical": false
                        }),
                    ),
                ]),
                timeline_track_index: 1,
                source_in: 0.5,
                source_out: 2.5,
            }],
            audio_clips: vec![RenderClip {
                source_path: Some("/tmp/input.mov".to_string()),
                timeline_start_seconds: Some(1.0),
                properties: BTreeMap::from([
                    ("timelineDurationSeconds".to_string(), json!(2.0)),
                    ("volumeDb".to_string(), json!(-3.0)),
                    ("fadeInSeconds".to_string(), json!(0.25)),
                    ("fadeOutSeconds".to_string(), json!(0.5)),
                    (
                        "keyframes".to_string(),
                        json!({"volumeDb":[
                            {"atSeconds":0.0,"value":-12.0,"easing":"easeInOut"},
                            {"atSeconds":2.0,"value":0.0,"easing":"linear"}
                        ]}),
                    ),
                ]),
                timeline_track_index: 2,
                source_in: 0.5,
                source_out: 2.5,
            }],
            transitions: Vec::new(),
            audio_transitions: Vec::new(),
        }
    }

    #[test]
    fn builds_versioned_native_request_from_canonical_render_plan() {
        let graphics = GraphicsArtifactManifest {
            schema_version: 1,
            artifact_id: "title".to_string(),
            kind: "rgbaFrameSequence".to_string(),
            dimensions: Dimensions {
                width: 1920,
                height: 1080,
            },
            fps: 30.0,
            duration_seconds: 1.0,
            alpha: true,
            frame_count: 2,
            frames_pattern: "frames/frame-%06d.png".to_string(),
            playback: GraphicsPlaybackManifest {
                mode: GraphicsPlaybackMode::Sequence,
                start_number: 0,
                frame_duration_seconds: 1.0 / 30.0,
                expected_frames: vec![
                    PathBuf::from("frames/frame-000000.png"),
                    PathBuf::from("frames/frame-000001.png"),
                ],
            },
            preview_path: PathBuf::from("preview.png"),
            source_layer_ids: vec!["title".to_string()],
            checksums: BTreeMap::new(),
        };
        let request = AvFoundationRenderBackend::new()
            .build_request(
                "job-1",
                &plan(),
                &[(graphics, PathBuf::from("/tmp/graphics/title"), 1.5)],
            )
            .expect("native request");

        assert_eq!(request.protocol, AVFOUNDATION_EXPORT_PROTOCOL);
        assert_eq!(request.schema_version, AVFOUNDATION_EXPORT_PROTOCOL_VERSION);
        assert_eq!(request.profile, AvFoundationExportProfile::H264);
        assert_eq!(request.video_clips[0].timeline_start_seconds, 1.0);
        assert_eq!(request.video_clips[0].opacity, 0.8);
        assert_eq!(request.audio_clips[0].volume_db, -3.0);
        assert_eq!(request.audio_clips[0].fade_in_seconds, 0.25);
        assert_eq!(request.audio_clips[0].fade_out_seconds, 0.5);
        assert_eq!(request.audio_clips[0].volume_keyframes.len(), 2);
        assert_eq!(request.audio_clips[0].volume_keyframes[0].value_db, -12.0);
        assert_eq!(
            request.audio_clips[0].volume_keyframes[0].easing,
            "easeInOut"
        );
        assert_eq!(request.overlays[0].frame_paths.len(), 2);
        assert_eq!(request.overlays[0].timeline_start_seconds, 1.5);
    }

    #[test]
    fn rejects_audio_fades_that_exceed_the_clip_duration() {
        let mut plan = plan();
        plan.audio_clips[0]
            .properties
            .insert("fadeInSeconds".to_string(), json!(1.25));
        plan.audio_clips[0]
            .properties
            .insert("fadeOutSeconds".to_string(), json!(1.0));

        let error = AvFoundationRenderBackend::new()
            .build_request("job-1", &plan, &[])
            .expect_err("invalid audio fades must fail");

        assert!(error[0].path.contains("audioClips[0].properties.fades"));
    }

    #[test]
    fn rejects_invalid_or_non_volume_audio_automation() {
        let mut plan = plan();
        plan.audio_clips[0].properties.insert(
            "keyframes".to_string(),
            json!({"opacity":[{"atSeconds":0.0,"value":1.0}]}),
        );
        let error = AvFoundationRenderBackend::new()
            .build_request("job-audio-keyframes", &plan, &[])
            .expect_err("unsupported audio automation must fail");
        assert!(error[0].path.contains("keyframes.opacity"));

        plan.audio_clips[0].properties.insert(
            "keyframes".to_string(),
            json!({"volumeDb":[
                {"atSeconds":1.0,"value":-3.0},
                {"atSeconds":0.5,"value":-6.0}
            ]}),
        );
        let error = AvFoundationRenderBackend::new()
            .build_request("job-audio-keyframes", &plan, &[])
            .expect_err("out-of-order audio automation must fail");
        assert!(error[0].path.contains("volumeDb[1]"));
    }

    #[test]
    fn rejects_retimed_audio_clips() {
        let mut plan = plan();
        plan.audio_clips[0]
            .properties
            .insert("speed".to_string(), json!(2.0));
        let error = AvFoundationRenderBackend::new()
            .build_request("job-audio-speed", &plan, &[])
            .expect_err("retimed audio must fail closed");
        assert_eq!(error[0].path, "renderPlan.audioClips[0].properties.speed");
        assert_eq!(
            error[0].message,
            "AVFoundation export does not retime audio clips."
        );
    }

    #[test]
    fn rejects_plans_with_video_or_audio_transitions() {
        use crate::edit::render_plan::RenderTransition;
        use crate::project::model::TransitionKind;

        let transition = RenderTransition {
            kind: TransitionKind::Crossfade,
            start_seconds: 1.5,
            duration_seconds: 1.0,
            left_clip_index: 0,
            right_clip_index: 0,
        };
        for audio in [false, true] {
            let mut plan = plan();
            if audio {
                plan.audio_transitions.push(transition.clone());
            } else {
                plan.transitions.push(transition.clone());
            }
            let error = AvFoundationRenderBackend::new()
                .build_request("job-transitions", &plan, &[])
                .expect_err("transitions must fail closed");
            let expected = if audio {
                "renderPlan.audioTransitions"
            } else {
                "renderPlan.transitions"
            };
            assert_eq!(error[0].path, expected);
            assert!(error[0]
                .message
                .contains("does not support clip transitions"));
        }
    }

    #[test]
    fn rejects_unprepared_effects_instead_of_silently_dropping_them() {
        let mut plan = plan();
        plan.clips[0].properties.insert(
            "effects".to_string(),
            json!([{"effectType": "stylize.grain"}]),
        );
        let error = AvFoundationRenderBackend::new()
            .build_request("job-1", &plan, &[])
            .expect_err("unprepared effect must fail");
        assert!(error[0].path.contains("effects"));
        assert!(error[0].message.contains("does not support"));
    }

    #[test]
    fn accepts_system_supported_still_image_sources() {
        let mut plan = plan();
        plan.clips[0].source_path = Some("/tmp/input.png".to_string());

        let request = AvFoundationRenderBackend::new()
            .build_request("job-image", &plan, &[])
            .expect("still image request");

        assert_eq!(request.video_clips[0].source_path, "/tmp/input.png");
        assert!(request.video_clips[0].prepared_frames.is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn accepts_non_allowlisted_visual_container_when_system_probe_decodes_it() {
        let directory = tempfile::tempdir().expect("source directory");
        let source_path = directory.path().join("system-decodable.visual");
        image::save_buffer_with_format(
            &source_path,
            &[0x22, 0x88, 0xcc, 0xff],
            1,
            1,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .expect("write content-detected source");

        require_avfoundation_video_source(source_path.to_str().expect("utf-8 source path"), 0)
            .expect("system source probe should decide decodability, not the extension allowlist");
    }

    #[test]
    fn maps_delivery_profiles_to_system_export_presets() {
        let backend = AvFoundationRenderBackend::new();
        let mut plan = plan();
        for (output_profile, expected) in [
            (
                RenderOutputProfile::Mp4Primary,
                AvFoundationExportProfile::H264,
            ),
            (
                RenderOutputProfile::Mp4Modern,
                AvFoundationExportProfile::Hevc,
            ),
            (
                RenderOutputProfile::QuicktimeInterchange,
                AvFoundationExportProfile::ProRes422,
            ),
        ] {
            plan.output_profile = output_profile;
            assert_eq!(
                backend.build_request("job", &plan, &[]).unwrap().profile,
                expected
            );
        }

        plan.output_profile = RenderOutputProfile::Mp4Primary;
        plan.quality = RenderQuality::Draft;
        let draft = backend.build_request("job-draft", &plan, &[]).unwrap();
        plan.quality = RenderQuality::Final;
        let final_request = backend.build_request("job-final", &plan, &[]).unwrap();
        assert_eq!(draft.profile, AvFoundationExportProfile::H264);
        assert_eq!(final_request.profile, AvFoundationExportProfile::H264);
        assert_eq!(draft.quality, RenderQuality::Draft);
        assert_eq!(final_request.quality, RenderQuality::Final);
    }

    #[test]
    fn native_capabilities_are_quality_aware_for_delivery_codecs() {
        let capabilities = AvFoundationCapabilities {
            protocol: AVFOUNDATION_EXPORT_PROTOCOL.to_string(),
            schema_version: AVFOUNDATION_EXPORT_PROTOCOL_VERSION,
            backend: BACKEND_NAME.to_string(),
            profiles: std::collections::BTreeMap::from([
                ("h264Draft".to_string(), false),
                ("h264Final".to_string(), true),
                ("hevcDraft".to_string(), true),
                ("hevcFinal".to_string(), true),
            ]),
        };

        assert!(!capabilities
            .supports_profile_quality(AvFoundationExportProfile::H264, RenderQuality::Draft));
        assert!(capabilities
            .supports_profile_quality(AvFoundationExportProfile::H264, RenderQuality::Final));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cancelled_native_export_terminates_the_sidecar_and_returns_actionable_status() {
        let key = RenderAttemptKey::new(
            PathBuf::from("/tmp/native-cancel-test"),
            "project",
            "native-cancel-test",
            "attempt",
        )
        .expect("key");
        let guard = register_render_attempt(key.clone()).expect("register");
        let token = guard.token();
        assert_eq!(
            request_render_cancellation(&key),
            crate::render_pipeline::cancel::RenderCancellationOutcome::Requested
        );

        let errors = AvFoundationRenderBackend::new()
            .render_cancellable(
                "native-cancel-test",
                &plan(),
                &[],
                Duration::from_secs(60),
                Some(&token),
            )
            .expect_err("pre-cancelled native render must stop the sidecar");

        assert_eq!(errors[0].path, "avfoundation.exporter.cancelled");
        assert!(errors[0].message.contains("cancelled"));
    }
}
