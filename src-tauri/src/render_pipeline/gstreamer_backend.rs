#[cfg(feature = "ges-render")]
use crate::edit::render_plan::RenderOutputProfile;
use crate::edit::render_plan::{ExportEncodeTier, RenderClip, RenderPlan, RenderQuality};
use crate::graphics::manifest::GraphicsArtifactManifest;
#[cfg(feature = "ges-render")]
use crate::project::export_profiles::ExportProfile;
#[cfg(feature = "ges-render")]
use crate::project::model::TransitionKind;
use crate::render_pipeline::cancel::RenderCancellationToken;
#[cfg(feature = "ges-render")]
use crate::render_runtime::render_runtime_environment;
use std::path::{Path, PathBuf};

use super::backend::RenderBackend;
use super::error::{PipelineError, PipelineErrorCode, PipelineResult};
#[cfg(feature = "ges-render")]
use super::gstreamer_transition_elements::{
    add_transition_solid, set_default_visual_box_height, visual_box_for_render_clip,
    visual_natural_width,
};
#[cfg(feature = "ges-render")]
use super::gstreamer_transitions::{
    audio_gain_control_points, held_control_value, transition_alpha_control_points,
    GesClipTransitions, GesTransitionLayout, WipeMask,
};
#[cfg(feature = "ges-render")]
use super::output_profile::{gstreamer_output_profile_target, GstreamerOutputProfileTarget};
#[cfg(feature = "ges-render")]
use super::plugin_policy::{policy_error_for_factory, GstFactoryInfo};
use super::probe::MediaProbe;
#[cfg(feature = "ges-render")]
use super::probe::{AudioProbe, VideoProbe};
use super::process::{CommandSpec, ProcessOutput, ProcessRunner};
#[cfg(feature = "ges-render")]
use super::quality::effective_quality_settings_for_tier;
#[cfg(feature = "ges-render")]
use super::transition_plan::{TRANSITION_HEAD_SECONDS_PROPERTY, TRANSITION_TAIL_SECONDS_PROPERTY};
use std::time::Duration;
#[cfg(feature = "ges-render")]
use std::time::Instant;

#[cfg(feature = "ges-render")]
use ges::gio;
#[cfg(feature = "ges-render")]
use ges::prelude::*;
#[cfg(feature = "ges-render")]
use gst::glib;
#[cfg(feature = "ges-render")]
use gst::glib::translate::FromGlibPtrFull;
#[cfg(feature = "ges-render")]
use gstreamer as gst;
#[cfg(feature = "ges-render")]
use gstreamer_editing_services as ges;
#[cfg(feature = "ges-render")]
use gstreamer_pbutils as gst_pbutils;
#[cfg(feature = "ges-render")]
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

const BACKEND_NAME: &str = "gstreamer-ges";
const REQUIRED_FEATURE: &str = "ges-render";
#[cfg(feature = "ges-render")]
const GST_RENDER_CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(200);
#[cfg(feature = "ges-render")]
const REQUIRED_COMMON_INPUT_DECODE_FACTORIES: &[&str] =
    &["decodebin", "qtdemux", "h264parse", "aacparse"];
#[cfg(feature = "ges-render")]
const REQUIRED_MACOS_INPUT_DECODE_FACTORIES: &[&str] = &["vtdec", "atdec"];
#[cfg(all(feature = "ges-render", not(target_os = "macos")))]
const REQUIRED_LINUX_INPUT_DECODE_FACTORIES: &[&str] = &["avdec_h264", "avdec_aac"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GstreamerGesRenderBackend;

#[cfg(feature = "ges-render")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebmEncodingProfileSummary {
    pub video_format: String,
    pub encoder_factory: String,
    pub target_bitrate_bps: u32,
    pub deadline: i64,
    pub cpu_used: i32,
}

#[cfg(feature = "ges-render")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodingProfileSummary {
    pub container_factory: String,
    pub video_factory: String,
    pub video_bitrate_kbps: u32,
    pub realtime: bool,
    pub audio_factory: Option<String>,
    pub parser_factories: Vec<String>,
}

#[cfg(feature = "ges-render")]
fn required_input_decode_factories_for_platform(is_macos: bool) -> Vec<&'static str> {
    let mut factories = REQUIRED_COMMON_INPUT_DECODE_FACTORIES.to_vec();
    if is_macos {
        factories.extend(REQUIRED_MACOS_INPUT_DECODE_FACTORIES);
    }
    factories
}

#[cfg(all(feature = "ges-render", target_os = "macos"))]
fn required_input_decode_factories() -> Vec<&'static str> {
    required_input_decode_factories_for_platform(true)
}

#[cfg(all(feature = "ges-render", not(target_os = "macos")))]
fn required_input_decode_factories() -> Vec<&'static str> {
    let mut factories = required_input_decode_factories_for_platform(false);
    factories.extend(REQUIRED_LINUX_INPUT_DECODE_FACTORIES);
    factories
}

#[cfg(feature = "ges-render")]
pub fn required_input_decode_factories_for_platform_for_test(is_macos: bool) -> Vec<&'static str> {
    required_input_decode_factories_for_platform(is_macos)
}

#[cfg(feature = "ges-render")]
pub fn required_input_decode_factories_for_test() -> Vec<&'static str> {
    required_input_decode_factories()
}

impl GstreamerGesRenderBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn backend_name(&self) -> &'static str {
        BACKEND_NAME
    }

    pub fn required_feature(&self) -> &'static str {
        REQUIRED_FEATURE
    }

    pub fn render_cancellable(
        &self,
        runner: &dyn ProcessRunner,
        plan: &RenderPlan,
        graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
        timeout: Duration,
        cancellation: Option<&RenderCancellationToken>,
    ) -> PipelineResult<ProcessOutput> {
        #[cfg(feature = "ges-render")]
        {
            let _ = runner;
            render_with_ges(plan, graphics, timeout, cancellation)
        }

        #[cfg(not(feature = "ges-render"))]
        {
            let _ = (runner, plan, graphics, timeout, cancellation);
            Err(vec![ges_feature_disabled_error()])
        }
    }
}

impl RenderBackend for GstreamerGesRenderBackend {
    fn build_command(
        &self,
        plan: &RenderPlan,
        graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
    ) -> PipelineResult<CommandSpec> {
        validate_render_plan(plan)?;
        validate_graphics_inputs(plan, graphics)?;

        let mut spec = CommandSpec::new(BACKEND_NAME)
            .arg(format!("--input={}", plan.input_path))
            .arg(format!("--output={}", plan.output_path))
            .arg(format!("--size={}x{}", plan.width, plan.height))
            .arg(format!("--fps={:.3}", plan.fps))
            .arg(format!(
                "--quality={}",
                quality_command_value(&plan.quality)
            ));
        if plan.encode_tier == ExportEncodeTier::Master {
            spec = spec.arg("--encode-tier=master");
        }

        let mut cursor_seconds = 0.0;
        for (index, clip) in plan.clips.iter().enumerate() {
            let source_path = render_clip_source_path(plan, clip);
            let duration_seconds = clip
                .properties
                .get("timelineDurationSeconds")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(clip.source_out - clip.source_in);
            let timeline_start_seconds = clip.timeline_start_seconds.unwrap_or(cursor_seconds);
            cursor_seconds = timeline_start_seconds + duration_seconds;
            spec = spec
                .arg(format!("--clip-source={source_path}"))
                .arg(format!("--clip-start={timeline_start_seconds:.3}"))
                .arg(format!(
                    "--clip={:.3}..{:.3}",
                    clip.source_in, clip.source_out
                ));
            if !clip.properties.is_empty() {
                let properties = serde_json::to_string(&clip.properties).map_err(|error| {
                    vec![PipelineError::new(
                        PipelineErrorCode::RenderPlanInvalidClip,
                        format!("renderPlan.clips[{index}].properties"),
                        "Render clip properties could not be serialized.",
                        "Keep render clip properties JSON-compatible and retry.",
                    )
                    .with_detail("serdeError", error.to_string())]
                })?;
                spec = spec.arg(format!("--clip-properties={properties}"));
            }
        }

        for (manifest, artifact_dir, timeline_start_seconds) in graphics {
            spec = spec.arg(format!(
                "--overlay={}@{:.3}+{:.3}",
                first_graphics_frame_path(manifest, artifact_dir).display(),
                timeline_start_seconds,
                manifest.duration_seconds
            ));
        }

        Ok(spec)
    }

    fn render(
        &self,
        runner: &dyn ProcessRunner,
        plan: &RenderPlan,
        graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
        timeout: Duration,
    ) -> PipelineResult<ProcessOutput> {
        self.render_cancellable(runner, plan, graphics, timeout, None)
    }
}

fn quality_command_value(quality: &RenderQuality) -> &'static str {
    match quality {
        RenderQuality::Draft => "draftWebm",
        RenderQuality::Final => "finalWebm",
    }
}

pub fn probe_media_with_gstreamer(
    path: &Path,
    timeout: Duration,
    error_path: &str,
) -> PipelineResult<(MediaProbe, ProcessOutput)> {
    #[cfg(feature = "ges-render")]
    {
        discover_media_with_gstreamer(path, timeout, error_path)
    }

    #[cfg(not(feature = "ges-render"))]
    {
        let _ = (path, timeout, error_path);
        Err(vec![ges_feature_disabled_error()])
    }
}

pub fn generate_fixture_source_with_gstreamer(
    path: &Path,
    width: u32,
    height: u32,
    fps: f64,
    duration_seconds: f64,
    timeout: Duration,
) -> PipelineResult<ProcessOutput> {
    #[cfg(feature = "ges-render")]
    {
        generate_fixture_source(path, width, height, fps, duration_seconds, timeout)
    }

    #[cfg(not(feature = "ges-render"))]
    {
        let _ = (path, width, height, fps, duration_seconds, timeout);
        Err(vec![ges_feature_disabled_error()])
    }
}

#[cfg(feature = "ges-render")]
#[doc(hidden)]
pub fn webm_encoding_profile_summary_for_test(
    plan: &RenderPlan,
) -> PipelineResult<WebmEncodingProfileSummary> {
    let _guard = gstreamer_runtime_guard()?;
    init_gstreamer()?;
    summarize_webm_encoding_profile(&webm_encoding_profile(plan))
}

#[cfg(feature = "ges-render")]
#[doc(hidden)]
pub fn encoding_profile_summary_for_test(
    plan: &RenderPlan,
    profile: ExportProfile,
) -> PipelineResult<EncodingProfileSummary> {
    let _guard = gstreamer_runtime_guard()?;
    init_gstreamer()?;
    let target = gstreamer_output_profile_target(profile).ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "gstreamer.ges.encodingProfile.profile",
            "Export profile is not supported by the GStreamer/GES backend.",
            "Choose an MP4 or ProRes export profile supported by GStreamer.",
        )
        .with_detail("profile", format!("{profile:?}"))]
    })?;
    summarize_encoding_profile(&encoding_profile(plan, &target), &target, plan)
}

/// Property names of a freshly created `factory` element, or `None` when the
/// runtime has no such factory. Used to verify encoder settings before use.
#[cfg(feature = "ges-render")]
#[doc(hidden)]
pub fn encoder_property_names_for_test(factory: &str) -> PipelineResult<Option<Vec<String>>> {
    let _guard = gstreamer_runtime_guard()?;
    init_gstreamer()?;
    let Ok(element) = gst::ElementFactory::make(factory).build() else {
        return Ok(None);
    };
    Ok(Some(
        element
            .list_properties()
            .iter()
            .map(|property| property.name().to_string())
            .collect(),
    ))
}

/// Builds the GES timeline for `plan` without rendering and returns a
/// deterministic summary of its layers, clips, effects and control envelopes.
/// Source URIs under `strip_prefix` are reported relative to `<dir>`.
#[cfg(feature = "ges-render")]
#[doc(hidden)]
pub fn ges_timeline_summary_for_test(
    plan: &RenderPlan,
    graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
    strip_prefix: &str,
) -> PipelineResult<String> {
    let _guard = gstreamer_runtime_guard()?;
    validate_render_plan(plan)?;
    validate_graphics_inputs(plan, graphics)?;
    init_gstreamer()?;
    let timeline = build_ges_timeline(plan, graphics)?;
    Ok(super::gstreamer_timeline_summary::summarize_ges_timeline(
        &timeline,
        strip_prefix,
    ))
}

#[cfg_attr(feature = "ges-render", allow(dead_code))]
pub(crate) fn ges_feature_disabled_error() -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::RenderBackendUnavailable,
        "gstreamer.ges",
        "GStreamer/GES render backend is not enabled in this build.",
        "Rebuild with `rtk cargo test --manifest-path src-tauri/Cargo.toml --features ges-render` after installing GStreamer Editing Services.",
    )
    .with_detail("backend", BACKEND_NAME)
    .with_detail("feature", REQUIRED_FEATURE)
}

fn validate_render_plan(plan: &RenderPlan) -> PipelineResult<()> {
    super::reverse_guard::reject_reversed_plan_clips(plan)?;
    if plan.clips.is_empty() {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            "renderPlan.clips",
            "Render plan has no source clips.",
            "Add at least one validated source clip before rendering.",
        )
        .with_detail("inputPath", plan.input_path.clone())
        .with_detail("outputPath", plan.output_path.clone())]);
    }

    if plan.clips.iter().any(|clip| {
        !clip.source_in.is_finite()
            || !clip.source_out.is_finite()
            || clip.source_in < 0.0
            || clip.source_out <= clip.source_in
            || clip
                .timeline_start_seconds
                .is_some_and(|start| !start.is_finite() || start < 0.0)
    }) {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            "renderPlan.clips",
            "Render plan contains an invalid source clip range.",
            "Ensure every clip has finite non-negative sourceIn, timelineStartSeconds, and sourceOut greater than sourceIn.",
        )
        .with_detail("inputPath", plan.input_path.clone())
        .with_detail("outputPath", plan.output_path.clone())]);
    }

    Ok(())
}

fn render_clip_source_path<'a>(plan: &'a RenderPlan, clip: &'a RenderClip) -> &'a str {
    clip.source_path.as_deref().unwrap_or(&plan.input_path)
}

fn validate_graphics_inputs(
    plan: &RenderPlan,
    graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
) -> PipelineResult<()> {
    let mut errors = Vec::new();
    let render_duration_seconds = render_plan_duration_seconds(plan);

    for (index, (manifest, _, timeline_start_seconds)) in graphics.iter().enumerate() {
        let path = format!("renderPlan.graphics[{index}]");
        if manifest.frame_count == 0 {
            errors.push(PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidOverlay,
                format!("{path}.frameCount"),
                "Graphics overlay has no rendered frames.",
                "Regenerate graphics artifacts before rendering with GStreamer.",
            ));
        }
        if manifest.frame_count > 600 {
            errors.push(PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidOverlay,
                format!("{path}.frameCount"),
                "Graphics overlay frame sequence exceeds the GStreamer/GES v1 budget.",
                "Render no more than 600 graphics frames for one overlay.",
            ));
        }
        if !manifest.alpha {
            errors.push(PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidOverlay,
                format!("{path}.alpha"),
                "Graphics overlay must preserve alpha for GStreamer composition.",
                "Render transparent PNG graphics artifacts before attaching the overlay.",
            ));
        }
        if manifest.dimensions.width != plan.width || manifest.dimensions.height != plan.height {
            errors.push(PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidOverlay,
                format!("{path}.dimensions"),
                "Graphics overlay dimensions do not match the render plan.",
                "Render graphics at the same width and height as the target video.",
            ));
        }
        if (manifest.fps - plan.fps).abs() > 0.001 {
            errors.push(PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidOverlay,
                format!("{path}.fps"),
                "Graphics overlay FPS does not match the render plan.",
                "Render graphics at the same FPS as the target video.",
            ));
        }
        let overlay_end = timeline_start_seconds + manifest.duration_seconds;
        if !timeline_start_seconds.is_finite()
            || !manifest.duration_seconds.is_finite()
            || *timeline_start_seconds < 0.0
            || manifest.duration_seconds <= 0.0
            || overlay_end - render_duration_seconds > 1e-6
        {
            errors.push(PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidOverlay,
                format!("{path}.durationSeconds"),
                "Graphics overlay timing falls outside the render duration.",
                "Keep overlay start and duration inside the selected EDL duration.",
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn first_graphics_frame_path(manifest: &GraphicsArtifactManifest, artifact_dir: &Path) -> PathBuf {
    graphics_frame_path(manifest, artifact_dir, 0)
}

fn graphics_frame_path(
    manifest: &GraphicsArtifactManifest,
    artifact_dir: &Path,
    frame_index: u32,
) -> PathBuf {
    let six = format!("{frame_index:06}");
    let five = format!("{frame_index:05}");
    let four = format!("{frame_index:04}");
    let raw = frame_index.to_string();
    let frame = manifest
        .frames_pattern
        .replace("%06d", &six)
        .replace("%05d", &five)
        .replace("%04d", &four)
        .replace("%d", &raw);
    artifact_dir.join(frame)
}

/// Loads a `GESUriClipAsset` on a private GLib main context.
///
/// `ges_uri_clip_asset_request_sync` runs a nested `GMainLoop` on the process-wide default context,
/// which GTK and WebKitGTK own in the desktop app. On the main thread that loop dispatched webview IPC
/// in the middle of a render: a synchronous command then blocked forever on the storage lease the
/// render held. On any other thread it waits for the main thread to release the context.
///
/// GES and `GstDiscoverer` attach their sources to the thread-default context, so the request runs
/// with a dedicated context pushed as thread default and iterates only that. The context is shared
/// by every thread because GES keeps one discoverer per thread, bound to the context it started on.
/// The mutex keeps one request at a time on it, so the callback always runs on the requesting thread.
#[cfg(feature = "ges-render")]
fn request_uri_clip_asset(uri: &str) -> Result<ges::UriClipAsset, glib::Error> {
    static ASSET_CONTEXT: OnceLock<Mutex<glib::MainContext>> = OnceLock::new();
    let context = ASSET_CONTEXT
        .get_or_init(|| Mutex::new(glib::MainContext::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let asset = context
        .with_thread_default(|| {
            let loaded = std::rc::Rc::new(std::cell::RefCell::new(None));
            ges::Asset::request_async::<ges::UriClip, _>(Some(uri), None::<&gio::Cancellable>, {
                let loaded = std::rc::Rc::clone(&loaded);
                move |result| *loaded.borrow_mut() = Some(result)
            });
            loop {
                if let Some(result) = loaded.borrow_mut().take() {
                    return result;
                }
                context.iteration(true);
            }
        })
        .map_err(|error| {
            glib::Error::new(
                gst::CoreError::Failed,
                &format!("GES asset main context could not be acquired: {error}"),
            )
        })??;
    asset.downcast::<ges::UriClipAsset>().map_err(|asset| {
        glib::Error::new(
            gst::CoreError::Failed,
            &format!("GES loaded a {} for a URI clip", asset.type_().name()),
        )
    })
}

#[cfg(feature = "ges-render")]
fn gstreamer_runtime_guard() -> PipelineResult<MutexGuard<'static, ()>> {
    static GSTREAMER_RUNTIME_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    GSTREAMER_RUNTIME_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "gstreamer.runtimeLock",
                "GStreamer runtime lock is poisoned.",
                "Restart the render process before running another GStreamer job.",
            )
            .with_detail("lockError", error.to_string())]
        })
}

#[cfg(feature = "ges-render")]
fn discover_media_with_gstreamer(
    path: &Path,
    timeout: Duration,
    error_path: &str,
) -> PipelineResult<(MediaProbe, ProcessOutput)> {
    let _guard = gstreamer_runtime_guard()?;
    require_allowed_factories(&required_input_decode_factories())?;
    init_gstreamer()?;
    let uri = filename_to_uri(path, error_path)?;
    let timeout = gst::ClockTime::try_from_seconds_f64(timeout.as_secs_f64()).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendTimeout,
            format!("gstreamer.discoverer.{error_path}.timeout"),
            "GStreamer media discovery timeout could not be represented as clock time.",
            "Use a finite positive media discovery timeout.",
        )
        .with_detail("clockError", error.to_string())]
    })?;
    let discoverer = gst_pbutils::Discoverer::new(timeout).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendUnavailable,
            format!("gstreamer.discoverer.{error_path}"),
            "GStreamer Discoverer could not be created.",
            "Update or reinstall Video Creater to restore its bundled media-inspection runtime.",
        )
        .with_detail("gstreamerError", error.to_string())]
    })?;
    let info = discoverer.discover_uri(uri.as_str()).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            format!("gstreamer.discoverer.{error_path}"),
            "GStreamer could not discover media metadata.",
            "Choose media readable by GStreamer and inspect missing plugin diagnostics.",
        )
        .with_detail("path", path.display().to_string())
        .with_detail("gstreamerError", error.to_string())]
    })?;

    if info.result() != gst_pbutils::DiscovererResult::Ok {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderProbeValidationFailed,
            format!("gstreamer.discoverer.{error_path}.result"),
            "GStreamer media discovery did not complete successfully.",
            "Choose media supported by Video Creater. If supported media keeps failing, update or reinstall the app.",
        )
        .with_detail("path", path.display().to_string())
        .with_detail("result", format!("{:?}", info.result()))
        .with_detail(
            "missingPlugins",
            info.missing_elements_installer_details()
                .iter()
                .map(|detail| detail.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        )]);
    }

    let probe = media_probe_from_discoverer(path, &info);
    let stdout = serde_json::to_string_pretty(&probe).unwrap_or_else(|_| "{}".to_string());
    Ok((
        probe,
        ProcessOutput {
            status_code: Some(0),
            stdout,
            stderr: String::new(),
        },
    ))
}

#[cfg(all(feature = "ges-render", target_os = "macos"))]
const FIXTURE_H264_ENCODER: &str = "vtenc_h264";
#[cfg(all(feature = "ges-render", target_os = "macos"))]
const FIXTURE_H264_ENCODER_LAUNCH: &str = "vtenc_h264 allow-frame-reordering=false realtime=true";
#[cfg(all(feature = "ges-render", target_os = "macos"))]
const FIXTURE_AAC_ENCODER: &str = "atenc";
#[cfg(all(feature = "ges-render", not(target_os = "macos")))]
const FIXTURE_H264_ENCODER: &str = "openh264enc";
#[cfg(all(feature = "ges-render", not(target_os = "macos")))]
const FIXTURE_H264_ENCODER_LAUNCH: &str = "openh264enc";
#[cfg(all(feature = "ges-render", not(target_os = "macos")))]
const FIXTURE_AAC_ENCODER: &str = "avenc_aac";

#[cfg(feature = "ges-render")]
fn generate_fixture_source(
    path: &Path,
    width: u32,
    height: u32,
    fps: f64,
    duration_seconds: f64,
    timeout: Duration,
) -> PipelineResult<ProcessOutput> {
    let _guard = gstreamer_runtime_guard()?;
    init_gstreamer()?;
    let mp4 = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"));
    let factories = if mp4 {
        vec![
            "filesink",
            "videotestsrc",
            "audiotestsrc",
            "mp4mux",
            FIXTURE_H264_ENCODER,
            FIXTURE_AAC_ENCODER,
            "aacparse",
            "h264parse",
            "videoconvert",
            "audioconvert",
            "audioresample",
        ]
    } else {
        vec![
            "filesink",
            "videotestsrc",
            "audiotestsrc",
            "webmmux",
            "vp8enc",
            "opusenc",
            "videoconvert",
            "audioconvert",
            "audioresample",
        ]
    };
    require_allowed_factories(&factories)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                "gstreamer.fixture.outputPath",
                "Create GStreamer fixture output directory failed.",
                "Choose a writable output path for the combined E2E fixture.",
            )
            .with_detail("path", parent.display().to_string())
            .with_detail("ioError", error.to_string())]
        })?;
    }

    let fps_fraction = fps_fraction(fps);
    let frame_count = (duration_seconds.max(0.1) * fps.max(1.0)).ceil() as u32;
    let audio_buffers = (duration_seconds.max(0.1) * 48_000.0 / 1024.0).ceil() as u32;
    let location = quote_for_gst_parse_launch(path);
    let description = if mp4 {
        format!(
            "mp4mux name=mux ! filesink location={location} \
             videotestsrc num-buffers={frame_count} pattern=ball ! video/x-raw,width={width},height={height},framerate={}/{} ! videoconvert ! {FIXTURE_H264_ENCODER_LAUNCH} ! h264parse ! queue ! mux. \
             audiotestsrc samplesperbuffer=1024 num-buffers={audio_buffers} wave=sine freq=440 ! audio/x-raw,rate=48000,channels=1 ! audioconvert ! audioresample ! {FIXTURE_AAC_ENCODER} ! aacparse ! queue ! mux.",
            fps_fraction.numer(),
            fps_fraction.denom()
        )
    } else {
        format!(
            "webmmux name=mux ! filesink location={location} \
             videotestsrc num-buffers={frame_count} pattern=ball ! video/x-raw,width={width},height={height},framerate={}/{} ! videoconvert ! vp8enc deadline=1 ! queue ! mux. \
             audiotestsrc samplesperbuffer=1024 num-buffers={audio_buffers} wave=sine freq=440 ! audio/x-raw,rate=48000,channels=1 ! audioconvert ! audioresample ! opusenc ! queue ! mux.",
            fps_fraction.numer(),
            fps_fraction.denom()
        )
    };
    let element = gst::parse::launch(&description).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "gstreamer.fixture.pipeline",
            "GStreamer fixture pipeline could not be parsed.",
            "Inspect the fixture pipeline element names and caps.",
        )
        .with_detail("gstreamerError", error.to_string())]
    })?;
    let pipeline = element.downcast::<gst::Pipeline>().map_err(|_| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "gstreamer.fixture.pipeline",
            "GStreamer fixture launch did not produce a pipeline.",
            "Inspect the fixture pipeline description.",
        )]
    })?;
    pipeline.set_state(gst::State::Playing).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "gstreamer.fixture.state",
            "GStreamer fixture pipeline could not start.",
            "Inspect fixture source and encoder availability.",
        )
        .with_detail("gstreamerError", error.to_string())]
    })?;
    let result = wait_for_gst_pipeline(&pipeline, timeout, "gstreamer.fixture");
    let _ = pipeline.set_state(gst::State::Null);
    result?;

    Ok(ProcessOutput {
        status_code: Some(0),
        stdout: format!(
            "Generated GStreamer fixture source\noutput={}\nsize={}x{}\nfps={:.3}\nduration={:.3}",
            path.display(),
            width,
            height,
            fps,
            duration_seconds
        ),
        stderr: String::new(),
    })
}

#[cfg(feature = "ges-render")]
fn render_with_ges(
    plan: &RenderPlan,
    graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
    timeout: Duration,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<ProcessOutput> {
    let _guard = gstreamer_runtime_guard()?;
    validate_render_plan(plan)?;
    validate_graphics_inputs(plan, graphics)?;
    let output_target = render_output_target(plan)?;
    let mut required_factories = required_input_decode_factories();
    required_factories.extend([
        "videoconvert",
        "videoscale",
        "videorate",
        "videoflip",
        "volume",
        "audioconvert",
        "audiomixer",
        "audiorate",
        "audioresample",
        "autoaudiosink",
        "autovideosink",
        "compositor",
    ]);
    if plan.clips.iter().any(|clip| {
        clip.properties
            .get("keyframes")
            .and_then(serde_json::Value::as_object)
            .is_some_and(|keyframes| keyframes.contains_key("rotationDegrees"))
    }) {
        required_factories.push("rotate");
    }
    if plan.transitions.iter().any(|transition| {
        matches!(
            transition.kind,
            TransitionKind::DipToBlack | TransitionKind::DipToWhite
        )
    }) {
        required_factories.push("videotestsrc");
    }
    if plan
        .transitions
        .iter()
        .any(|transition| transition.kind == TransitionKind::Wipe)
    {
        required_factories.push("videocrop");
    }
    match &output_target {
        RenderOutputTarget::Webm => {
            let encoder_settings = webm_encoder_settings(plan);
            required_factories.extend(["webmmux", encoder_settings.encoder_factory, "opusenc"]);
        }
        RenderOutputTarget::Export(target) => {
            required_factories.extend(target.required_factories());
        }
    }
    require_allowed_factories(&required_factories)?;
    init_gstreamer()?;

    if let Some(parent) = Path::new(&plan.output_path).parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                "renderPlan.outputPath",
                "Create GStreamer output directory failed.",
                "Choose a writable output path and retry the render.",
            )
            .with_detail("path", parent.display().to_string())
            .with_detail("ioError", error.to_string())]
        })?;
    }

    let timeline = build_ges_timeline(plan, graphics)?;
    let output_uri = filename_to_uri(Path::new(&plan.output_path), "renderPlan.outputPath")?;
    render_built_timeline(
        plan,
        graphics,
        &timeline,
        output_uri.as_str(),
        &output_target,
        timeout,
        cancellation,
    )
}

/// Builds and commits the GES timeline for `plan` without rendering it.
#[cfg(feature = "ges-render")]
fn build_ges_timeline(
    plan: &RenderPlan,
    graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
) -> PipelineResult<ges::Timeline> {
    let layout = GesTransitionLayout::new(plan)?;
    let timeline = ges::Timeline::new_audio_video();
    let video_restriction_caps = gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .field("width", plan.width as i32)
        .field("height", plan.height as i32)
        .field("framerate", fps_fraction(plan.fps))
        .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
        .build();
    for track in timeline.tracks() {
        if track.track_type().contains(ges::TrackType::VIDEO) {
            track.set_restriction_caps(&video_restriction_caps);
        }
    }
    // In GES a lower layer priority is closer to the viewer. Create visual
    // layers from top to bottom so the canonical project convention (larger
    // track index paints above smaller) is preserved.
    let mut graphics_layers = std::collections::BTreeMap::new();
    for index in (0..graphics.len()).rev() {
        graphics_layers.insert(index, timeline.append_layer());
    }
    // Each track gets one layer per transition stacking depth, incoming clips
    // above outgoing ones, and dips add a solid layer beneath the track's clips.
    let mut source_layers = std::collections::BTreeMap::new();
    let mut solid_layers = std::collections::BTreeMap::new();
    let source_track_indices = plan
        .clips
        .iter()
        .map(|clip| clip.timeline_track_index)
        .collect::<std::collections::BTreeSet<_>>();
    for track_index in source_track_indices.into_iter().rev() {
        for depth in (0..=layout.max_video_depth(plan, track_index)).rev() {
            source_layers.insert((track_index, depth), timeline.append_layer());
        }
        if layout.has_solid(track_index) {
            solid_layers.insert(track_index, timeline.append_layer());
        }
    }

    let mut cursor = gst::ClockTime::ZERO;
    for (index, clip) in plan.clips.iter().enumerate() {
        let source_path = render_clip_source_path(plan, clip);
        let source_uri = filename_to_uri(Path::new(source_path), "renderPlan.clips.sourcePath")?;
        let source_asset = request_uri_clip_asset(source_uri.as_str()).map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.clips[{index}].sourceAsset"),
                "GStreamer/GES could not load the source media asset.",
                "Choose source videos readable by GStreamer and retry the render.",
            )
            .with_detail("inputPath", source_path.to_string())
            .with_detail("gstreamerError", error.to_string())]
        })?;
        let duration_seconds = render_clip_timeline_duration_seconds(clip, index)?;
        let timeline_start = match clip.timeline_start_seconds {
            Some(start_seconds) => seconds_to_clock_time(start_seconds, "renderPlan.clips.start")?,
            None => cursor,
        };
        let inpoint = seconds_to_clock_time(clip.source_in, "renderPlan.clips.inpoint")?;
        let duration = seconds_to_clock_time(duration_seconds, "renderPlan.clips.duration")?;
        let source_layer = source_layers
            .get(&(clip.timeline_track_index, layout.clips[index].depth))
            .ok_or_else(|| {
                vec![PipelineError::new(
                    PipelineErrorCode::RenderBackendFailed,
                    format!("gstreamer.ges.clips[{index}].layer"),
                    "GStreamer/GES source layer was not prepared.",
                    "Rebuild the canonical visual layer map and retry the render.",
                )]
            })?;
        let ges_clip = source_layer
            .add_asset(
                &source_asset,
                Some(timeline_start),
                Some(inpoint),
                Some(duration),
                ges::TrackType::VIDEO,
            )
            .map_err(|error| {
                vec![PipelineError::new(
                    PipelineErrorCode::RenderBackendFailed,
                    format!("gstreamer.ges.clips[{index}]"),
                    "GStreamer/GES could not add a source clip to the timeline.",
                    "Inspect the selected EDL clip range and source media compatibility.",
                )
                .with_detail("gstreamerError", error.to_string())]
            })?;
        apply_visual_clip_properties(
            &ges_clip,
            clip,
            plan,
            index,
            &layout.clips[index],
            &source_asset,
        )?;
        cursor = timeline_start.checked_add(duration).ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                "renderPlan.clips",
                "Render plan duration exceeds supported GStreamer clock time.",
                "Shorten the render plan duration.",
            )]
        })?;
    }

    for (index, solid) in layout.solids.iter().enumerate() {
        let layer = solid_layers.get(&solid.track_index).ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.transitions.solids[{index}].layer"),
                "GStreamer/GES transition solid layer was not prepared.",
                "Rebuild the canonical visual layer map and retry the render.",
            )]
        })?;
        add_transition_solid(layer, solid, index)?;
    }

    let mut audio_layers = std::collections::BTreeMap::new();
    for (index, clip) in plan.audio_clips.iter().enumerate() {
        let source_path = render_clip_source_path(plan, clip);
        let source_uri =
            filename_to_uri(Path::new(source_path), "renderPlan.audioClips.sourcePath")?;
        let source_asset = request_uri_clip_asset(source_uri.as_str()).map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.audioClips[{index}].sourceAsset"),
                "GStreamer/GES could not load the timeline audio asset.",
                "Choose audio media readable by GStreamer and retry the render.",
            )
            .with_detail("inputPath", source_path.to_string())
            .with_detail("gstreamerError", error.to_string())]
        })?;
        let start = seconds_to_clock_time(
            clip.timeline_start_seconds.unwrap_or(0.0),
            "renderPlan.audioClips.start",
        )?;
        reject_unprepared_audio_speed(clip, index)?;
        let inpoint = seconds_to_clock_time(clip.source_in, "renderPlan.audioClips.inpoint")?;
        let duration = seconds_to_clock_time(
            render_clip_number_property(clip, "timelineDurationSeconds")
                .unwrap_or(clip.source_out - clip.source_in),
            "renderPlan.audioClips.duration",
        )?;
        let audio_layer = audio_layers
            .entry((clip.timeline_track_index, layout.audio_clips[index].depth))
            .or_insert_with(|| timeline.append_layer());
        let ges_clip = audio_layer
            .add_asset(
                &source_asset,
                Some(start),
                Some(inpoint),
                Some(duration),
                ges::TrackType::AUDIO,
            )
            .map_err(|error| {
                vec![PipelineError::new(
                    PipelineErrorCode::RenderBackendFailed,
                    format!("gstreamer.ges.audioClips[{index}]"),
                    "GStreamer/GES could not add a timeline audio clip.",
                    "Inspect the selected audio range and source media compatibility.",
                )
                .with_detail("gstreamerError", error.to_string())]
            })?;
        apply_audio_clip_properties(&ges_clip, clip, index, &layout.audio_clips[index])?;
    }

    if !graphics.is_empty() {
        for (index, (manifest, artifact_dir, timeline_start_seconds)) in graphics.iter().enumerate()
        {
            let overlay_layer = graphics_layers.get(&index).ok_or_else(|| {
                vec![PipelineError::new(
                    PipelineErrorCode::RenderBackendFailed,
                    format!("gstreamer.ges.graphics[{index}].layer"),
                    "GStreamer/GES graphics layer was not prepared.",
                    "Rebuild the canonical graphics layer map and retry the render.",
                )]
            })?;
            let frame_duration_seconds = manifest.duration_seconds / manifest.frame_count as f64;
            for frame_index in 0..manifest.frame_count {
                let frame_path = graphics_frame_path(manifest, artifact_dir, frame_index);
                let frame_uri = filename_to_uri(&frame_path, "renderPlan.graphics.frame")?;
                let asset = request_uri_clip_asset(frame_uri.as_str()).map_err(|error| {
                    vec![PipelineError::new(
                        PipelineErrorCode::RenderBackendFailed,
                        format!("gstreamer.ges.graphics[{index}].frames[{frame_index}]"),
                        "GStreamer/GES could not load a graphics overlay PNG frame.",
                        "Regenerate graphics artifacts and ensure every frame exists.",
                    )
                    .with_detail("framePath", frame_path.display().to_string())
                    .with_detail("gstreamerError", error.to_string())]
                })?;
                let frame_start_seconds =
                    *timeline_start_seconds + frame_duration_seconds * frame_index as f64;
                let start =
                    seconds_to_clock_time(frame_start_seconds, "renderPlan.graphics.frameStart")?;
                let duration = seconds_to_clock_time(
                    frame_duration_seconds,
                    "renderPlan.graphics.frameDuration",
                )?;
                asset.set_duration(duration.nseconds());
                overlay_layer
                    .add_asset(
                        &asset,
                        Some(start),
                        Some(gst::ClockTime::ZERO),
                        Some(duration),
                        ges::TrackType::VIDEO,
                    )
                    .map_err(|error| {
                        vec![PipelineError::new(
                            PipelineErrorCode::RenderBackendFailed,
                            format!("gstreamer.ges.graphics[{index}].frames[{frame_index}]"),
                            "GStreamer/GES could not add a graphics overlay frame to the timeline.",
                            "Inspect overlay timing, dimensions, and PNG frame compatibility.",
                        )
                        .with_detail("framePath", frame_path.display().to_string())
                        .with_detail("gstreamerError", error.to_string())]
                    })?;
            }
        }
    }

    if !timeline.commit_sync() {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "gstreamer.ges.timeline",
            "GStreamer/GES could not commit the render timeline.",
            "Inspect the timeline clips and graphics layers, then retry the render.",
        )]);
    }
    Ok(timeline)
}

#[cfg(feature = "ges-render")]
fn render_built_timeline(
    plan: &RenderPlan,
    graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
    timeline: &ges::Timeline,
    output_uri: &str,
    output_target: &RenderOutputTarget,
    timeout: Duration,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<ProcessOutput> {
    let profile = match output_target {
        RenderOutputTarget::Webm => webm_encoding_profile(plan),
        RenderOutputTarget::Export(target) => encoding_profile(plan, target),
    };
    let pipeline = ges::Pipeline::new();
    let encoder_effort = EncoderEffort::for_plan(plan);
    let encoder_policy_errors = Arc::new(Mutex::new(Vec::<PipelineError>::new()));
    let encoder_policy_errors_for_signal = Arc::clone(&encoder_policy_errors);
    pipeline.connect_deep_element_added(move |_, _, element| {
        if let Some(error) = auto_selected_encoder_policy_error(element) {
            encoder_policy_errors_for_signal
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(error);
        }
        configure_platform_encoder(element, encoder_effort);
        if element
            .factory()
            .is_some_and(|factory| factory.name() == "compositor")
        {
            for pad in element.sink_pads() {
                set_compositor_pad_over(&pad);
            }
            element.connect_pad_added(|_, pad| set_compositor_pad_over(pad));
        }
    });
    pipeline.set_timeline(timeline).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "gstreamer.ges.pipeline.timeline",
            "GStreamer/GES could not attach the timeline to the render pipeline.",
            "Inspect the GES timeline construction and retry the render.",
        )
        .with_detail("gstreamerError", error.to_string())]
    })?;
    pipeline
        .set_render_settings(output_uri, &profile)
        .map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "gstreamer.ges.pipeline.renderSettings",
                "GStreamer/GES could not configure render settings.",
                "Use an output path and approved GStreamer plugins for the selected target profile.",
            )
            .with_detail("gstreamerError", error.to_string())]
        })?;
    pipeline
        .set_mode(ges::PipelineFlags::RENDER)
        .map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "gstreamer.ges.pipeline.mode",
                "GStreamer/GES could not enter render mode.",
                "Inspect GES pipeline setup and retry the render.",
            )
            .with_detail("gstreamerError", error.to_string())]
        })?;

    pipeline.set_state(gst::State::Playing).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "gstreamer.ges.pipeline.state",
            "GStreamer/GES could not start rendering.",
            "Inspect selected plugins, source media, and render settings.",
        )
        .with_detail("gstreamerError", error.to_string())]
    })?;

    let render_result = wait_for_pipeline(&pipeline, timeout, cancellation);
    let _ = pipeline.set_state(gst::State::Null);
    let encoder_policy_errors = std::mem::take(
        &mut *encoder_policy_errors
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
    );
    if !encoder_policy_errors.is_empty() {
        let _ = std::fs::remove_file(&plan.output_path);
        return Err(encoder_policy_errors);
    }
    render_result?;

    Ok(ProcessOutput {
        status_code: Some(0),
        stdout: format!(
            "Rendered with GStreamer/GES\ninput={}\noutput={}\nclips={}\noverlays={}",
            plan.input_path,
            plan.output_path,
            plan.clips.len(),
            graphics.len()
        ),
        stderr: String::new(),
    })
}

#[cfg(feature = "ges-render")]
fn set_compositor_pad_over(pad: &gst::Pad) {
    write_compositor_pad_over(pad);
    pad.add_probe(gst::PadProbeType::BUFFER, |pad, _| {
        write_compositor_pad_over(pad);
        gst::PadProbeReturn::Remove
    });
}

#[cfg(feature = "ges-render")]
fn write_compositor_pad_over(pad: &gst::Pad) {
    let Some(property) = pad.find_property("operator") else {
        return;
    };
    let Some(enum_class) = glib::EnumClass::with_type(property.value_type()) else {
        return;
    };
    let Some(enum_value) = enum_class.value(1) else {
        return;
    };
    let value = enum_value.to_value(&enum_class);
    pad.set_property_from_value(property.name(), &value);
}

#[cfg(feature = "ges-render")]
fn apply_visual_clip_properties(
    ges_clip: &ges::Clip,
    clip: &RenderClip,
    plan: &RenderPlan,
    index: usize,
    transitions: &GesClipTransitions,
    source_asset: &ges::UriClipAsset,
) -> PipelineResult<()> {
    let mut transitions = *transitions;
    let head = transitions.head_seconds;
    let speed = visual_speed_for_render_clip(clip, index)?;
    let timing = GesSourceTiming {
        source_in: clip.source_in,
        speed: speed.unwrap_or(1.0),
    };
    let opacity = visual_opacity_for_render_clip(clip, index)?;
    if let Some(opacity) = opacity {
        ges_clip
            .set_child_property("alpha", opacity)
            .map_err(|error| {
                vec![PipelineError::new(
                    PipelineErrorCode::RenderBackendFailed,
                    format!("gstreamer.ges.clips[{index}].properties.opacity"),
                    "GStreamer/GES could not apply visual clip opacity.",
                    "Use a GStreamer runtime with the GES video alpha child property available.",
                )
                .with_detail("gstreamerError", error.to_string())]
            })?;
    }
    let geometry = visual_geometry_for_render_clip(clip, plan, index)?;
    // A newly added GES effect is applied before the existing ones, so the
    // wipe mask goes first: it then crops the finished frame, above any speed
    // effect, in clip-local time.
    let wipe_effect = if transitions.wipe().is_some() {
        let (box_left, _) = visual_box_for_render_clip(ges_clip, geometry, plan);
        transitions.wipe_box_left = box_left;
        Some(add_visual_effect(
            ges_clip,
            "videocrop right=0",
            index,
            "transition.wipe",
            "videocrop",
        )?)
    } else {
        None
    };
    // The speed effect is attached before source envelopes are bound so GES
    // does not re-clamp them when the time effect changes the source range.
    if let Some(speed) = speed {
        let effect = add_visual_effect(
            ges_clip,
            &format!("videorate rate={speed:.6}"),
            index,
            "properties.speed",
            "videorate",
        )?;
        super::gstreamer_video_speed::correct_speed_effect_timing(&effect, index)?;
    }
    let fade = visual_fade_envelope_for_render_clip(clip, index)?;
    let opacity_keyframes = visual_opacity_keyframes_for_render_clip(clip, index)?;
    let timeline_duration = render_clip_timeline_duration_seconds(clip, index)?;
    if transitions.is_transitioned() {
        if fade.is_some() || !opacity_keyframes.is_empty() || transitions.affects_alpha() {
            let points = transition_alpha_control_points(
                opacity.unwrap_or(1.0),
                timeline_duration,
                fade,
                &opacity_keyframes,
                &transitions,
                plan.fps,
            );
            apply_visual_track_control_points(
                ges_clip, "alpha", &points, index, "opacity", timing,
            )?;
        }
    } else if fade.is_some() || !opacity_keyframes.is_empty() {
        apply_visual_alpha_envelope(
            ges_clip,
            opacity.unwrap_or(1.0),
            timeline_duration,
            fade,
            &opacity_keyframes,
            index,
            timing,
        )?;
    }
    visual_blend_operator_for_render_clip(clip, index)?;
    if let Some(geometry) = geometry {
        for (property_name, value) in [
            ("posx", geometry.x),
            ("posy", geometry.y),
            ("width", geometry.width),
            ("height", geometry.height),
        ] {
            ges_clip
                .set_child_property(property_name, value)
                .map_err(|error| {
                    vec![PipelineError::new(
                        PipelineErrorCode::RenderBackendFailed,
                        format!("gstreamer.ges.clips[{index}].properties.transform"),
                        "GStreamer/GES could not apply visual clip canvas geometry.",
                        "Use a GStreamer runtime with GES video source geometry child properties available.",
                    )
                    .with_detail("property", property_name)
                    .with_detail("gstreamerError", error.to_string())]
            })?;
        }
    }
    let mut box_left_points = Vec::new();
    let mut box_width_points = Vec::new();
    for (keyframe_property, child_property, base) in [
        (
            "positionX",
            "posx",
            geometry.map_or(0.0, |geometry| f64::from(geometry.x)),
        ),
        (
            "positionY",
            "posy",
            geometry.map_or(0.0, |geometry| f64::from(geometry.y)),
        ),
    ] {
        let position_keyframes =
            visual_position_keyframes_for_render_clip(clip, index, keyframe_property)?;
        if !position_keyframes.is_empty() {
            let points = position_keyframes
                .into_iter()
                .map(|keyframe| GesTimedControlPoint {
                    seconds: keyframe.seconds + head,
                    value: base + keyframe.value,
                })
                .collect::<Vec<_>>();
            apply_visual_track_control_points(
                ges_clip,
                child_property,
                &points,
                index,
                keyframe_property,
                timing,
            )?;
            if child_property == "posx" {
                box_left_points = points;
            }
        }
    }
    let scale_keyframes = visual_scale_keyframes_for_render_clip(clip, index)?;
    if !scale_keyframes.is_empty() {
        let base_width =
            geometry.map_or(f64::from(plan.width), |geometry| f64::from(geometry.width));
        let base_height = geometry.map_or(f64::from(plan.height), |geometry| {
            f64::from(geometry.height)
        });
        for (child_property, base) in [("width", base_width), ("height", base_height)] {
            let points = scale_keyframes
                .iter()
                .map(|keyframe| GesTimedControlPoint {
                    seconds: keyframe.seconds + head,
                    value: (base * keyframe.value).round(),
                })
                .collect::<Vec<_>>();
            if child_property == "width" && wipe_effect.is_some() {
                // The wipe mask binds the width, composed with these points.
                box_width_points = points;
                continue;
            }
            apply_visual_track_control_points(
                ges_clip,
                child_property,
                &points,
                index,
                "scale",
                timing,
            )?;
        }
    }
    let rotation_keyframes = visual_rotation_keyframes_for_render_clip(clip, index)?;
    if !rotation_keyframes.is_empty() {
        let effect = ges::Effect::new("rotate angle=0.0").map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.clips[{index}].properties.keyframes.rotationDegrees"),
                "GStreamer/GES could not create the visual rotation effect.",
                "Use a GStreamer runtime with the reviewed rotate effect available.",
            )
            .with_detail("gstreamerError", error.to_string())]
        })?;
        ges_clip.add(&effect).map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.clips[{index}].properties.keyframes.rotationDegrees"),
                "GStreamer/GES could not attach the visual rotation effect.",
                "Inspect the clip effect stack and rotate effect availability.",
            )
            .with_detail("gstreamerError", error.to_string())]
        })?;
        let points = rotation_keyframes
            .into_iter()
            .map(|keyframe| GesTimedControlPoint {
                seconds: keyframe.seconds + head,
                value: keyframe.value.to_radians(),
            })
            .collect::<Vec<_>>();
        apply_visual_effect_control_points(&effect, "angle", &points, index, "rotationDegrees")?;
    }
    if let Some(method) = visual_flip_method_for_render_clip(clip, index)? {
        let effect =
            ges::Effect::new(&format!("videoflip video-direction={method}")).map_err(|error| {
                vec![PipelineError::new(
                    PipelineErrorCode::RenderBackendFailed,
                    format!("gstreamer.ges.clips[{index}].properties.transform"),
                    "GStreamer/GES could not create the visual flip effect.",
                    "Use a GStreamer runtime with the approved videoflip effect available.",
                )
                .with_detail("gstreamerError", error.to_string())]
            })?;
        ges_clip.add(&effect).map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.clips[{index}].properties.transform"),
                "GStreamer/GES could not attach the visual flip effect.",
                "Inspect the clip effect stack and GStreamer videoflip availability.",
            )
            .with_detail("gstreamerError", error.to_string())]
        })?;
    }
    let static_crop = visual_crop_for_render_clip(clip, index)?;
    let (source_width, source_height) = if clip
        .properties
        .get("keyframes")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|keyframes| {
            ["cropTop", "cropRight", "cropBottom", "cropLeft"]
                .iter()
                .any(|property| keyframes.contains_key(*property))
        }) {
        visual_source_dimensions_for_render_clip(clip, index)?
    } else {
        (0.0, 0.0)
    };
    let shift = |points: Vec<GesTimedControlPoint>| {
        points
            .into_iter()
            .map(|point| GesTimedControlPoint {
                seconds: point.seconds + head,
                ..point
            })
            .collect::<Vec<_>>()
    };
    let crop_keyframes = [
        (
            "cropTop",
            "top",
            shift(visual_crop_keyframes_for_render_clip(
                clip,
                index,
                "cropTop",
                source_height,
            )?),
        ),
        (
            "cropRight",
            "right",
            shift(visual_crop_keyframes_for_render_clip(
                clip,
                index,
                "cropRight",
                source_width,
            )?),
        ),
        (
            "cropBottom",
            "bottom",
            shift(visual_crop_keyframes_for_render_clip(
                clip,
                index,
                "cropBottom",
                source_height,
            )?),
        ),
        (
            "cropLeft",
            "left",
            shift(visual_crop_keyframes_for_render_clip(
                clip,
                index,
                "cropLeft",
                source_width,
            )?),
        ),
    ];
    if static_crop.is_some()
        || crop_keyframes
            .iter()
            .any(|(_, _, points)| !points.is_empty())
    {
        let crop = static_crop.unwrap_or(GesCropInsets {
            top: 0,
            right: 0,
            bottom: 0,
            left: 0,
        });
        let effect = ges::Effect::new(&format!(
            "videocrop top={} right={} bottom={} left={}",
            crop.top, crop.right, crop.bottom, crop.left
        ))
        .map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.clips[{index}].properties.crop"),
                "GStreamer/GES could not create the visual crop effect.",
                "Use a GStreamer runtime with the approved videocrop effect available.",
            )
            .with_detail("gstreamerError", error.to_string())]
        })?;
        ges_clip.add(&effect).map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.clips[{index}].properties.crop"),
                "GStreamer/GES could not attach the visual crop effect.",
                "Inspect the clip effect stack and GStreamer videocrop availability.",
            )
            .with_detail("gstreamerError", error.to_string())]
        })?;
        for (property_label, effect_property, points) in &crop_keyframes {
            if !points.is_empty() {
                apply_visual_effect_control_points(
                    &effect,
                    effect_property,
                    points,
                    index,
                    property_label,
                )?;
            }
        }
    }
    if let Some(grade) = visual_color_grade_for_render_clip(clip, index)? {
        let effect = ges::Effect::new(&format!(
            "videobalance brightness={:.6} contrast={:.6} saturation={:.6}",
            grade.brightness, grade.contrast, grade.saturation
        ))
        .map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.clips[{index}].properties.colorGrade"),
                "GStreamer/GES could not create the basic visual color-grade effect.",
                "Use a GStreamer runtime with the approved videobalance effect available.",
            )
            .with_detail("gstreamerError", error.to_string())]
        })?;
        ges_clip.add(&effect).map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.clips[{index}].properties.colorGrade"),
                "GStreamer/GES could not attach the basic visual color-grade effect.",
                "Inspect the clip effect stack and GStreamer videobalance availability.",
            )
            .with_detail("gstreamerError", error.to_string())]
        })?;
    }
    if let Some(effect) = wipe_effect {
        let (base_left, base_width) = visual_box_for_render_clip(ges_clip, geometry, plan);
        let natural_width = visual_natural_width(clip, source_asset, index)?;
        let crop = static_crop.unwrap_or(GesCropInsets {
            top: 0,
            right: 0,
            bottom: 0,
            left: 0,
        });
        let crop_points = |label: &str| {
            crop_keyframes
                .iter()
                .find(|(property, _, _)| *property == label)
                .map(|(_, _, points)| points.clone())
                .unwrap_or_default()
        };
        let (crop_left, crop_right) = (crop_points("cropLeft"), crop_points("cropRight"));
        let box_left = |local: f64| held_control_value(&box_left_points, local, base_left);
        let box_width = |local: f64| held_control_value(&box_width_points, local, base_width);
        let frame_width = |local: f64| {
            natural_width
                - held_control_value(&crop_left, local, f64::from(crop.left))
                - held_control_value(&crop_right, local, f64::from(crop.right))
        };
        let mut times = transitions.visual_event_seconds(plan.fps);
        times.extend(
            box_left_points
                .iter()
                .chain(&box_width_points)
                .chain(&crop_left)
                .chain(&crop_right)
                .map(|point| point.seconds),
        );
        let (widths, crops) = WipeMask {
            transitions: &transitions,
            box_left: &box_left,
            box_width: &box_width,
            frame_width: &frame_width,
        }
        .points(times);
        if geometry.is_none() && box_width_points.is_empty() {
            set_default_visual_box_height(ges_clip, plan, index)?;
        }
        apply_visual_track_control_points(
            ges_clip,
            "width",
            &widths,
            index,
            "transition.wipe",
            timing,
        )?;
        apply_visual_effect_control_points(&effect, "right", &crops, index, "transition.wipe")?;
    }
    Ok(())
}

#[cfg(feature = "ges-render")]
fn render_clip_timeline_duration_seconds(clip: &RenderClip, index: usize) -> PipelineResult<f64> {
    let duration = render_clip_number_property(clip, "timelineDurationSeconds")
        .unwrap_or(clip.source_out - clip.source_in);
    if !duration.is_finite() || duration <= 0.0 {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.timelineDurationSeconds"),
            "Visual clip timeline duration must be finite and positive.",
            "Keep the timeline duration positive after changing clip speed.",
        )]);
    }
    Ok(duration)
}

#[cfg(feature = "ges-render")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct GesVisualFadeEnvelope {
    pub(super) opacity: f64,
    pub(super) duration_seconds: f64,
    pub(super) fade_in_seconds: f64,
    pub(super) fade_out_seconds: f64,
}

#[cfg(feature = "ges-render")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct GesTimedControlPoint {
    pub(super) seconds: f64,
    pub(super) value: f64,
}

#[cfg(feature = "ges-render")]
fn apply_visual_alpha_envelope(
    ges_clip: &ges::Clip,
    opacity: f64,
    duration_seconds: f64,
    fade: Option<GesVisualFadeEnvelope>,
    opacity_keyframes: &[GesTimedControlPoint],
    index: usize,
    timing: GesSourceTiming,
) -> PipelineResult<()> {
    let points = visual_alpha_control_points(opacity, duration_seconds, fade, opacity_keyframes);
    apply_visual_track_control_points(ges_clip, "alpha", &points, index, "opacity", timing)
}

/// How clip-local seconds map to the control-point timestamps of a GES
/// source's child properties.
///
/// GES binds source child properties (alpha, geometry, volume) in the
/// source's internal time, which starts at the clip's in-point (verified on
/// GES 1.24.2: a point at `inpoint + t` applies `t` seconds into the clip).
/// With a `videorate rate=s` time effect the source reads `inpoint + t * s`
/// (`gstreamer_video_speed` corrects `videorate`, which otherwise read
/// `inpoint / s + t * s`), and control points follow the source frame. Effect
/// properties use clip-local time instead.
#[cfg(feature = "ges-render")]
#[derive(Debug, Clone, Copy, PartialEq)]
struct GesSourceTiming {
    source_in: f64,
    speed: f64,
}

#[cfg(feature = "ges-render")]
impl GesSourceTiming {
    fn internal_seconds(&self, local_seconds: f64) -> f64 {
        self.source_in + local_seconds * self.speed
    }
}

/// Attaches a GES effect parsed from `description` to `ges_clip`.
#[cfg(feature = "ges-render")]
fn add_visual_effect(
    ges_clip: &ges::Clip,
    description: &str,
    index: usize,
    property_label: &str,
    factory: &str,
) -> PipelineResult<ges::Effect> {
    let path = format!("gstreamer.ges.clips[{index}].{property_label}");
    let effect = ges::Effect::new(description).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            path.clone(),
            format!("GStreamer/GES could not create the {factory} effect."),
            format!("Use a GStreamer runtime with the approved {factory} effect available."),
        )
        .with_detail("gstreamerError", error.to_string())]
    })?;
    ges_clip.add(&effect).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            path,
            format!("GStreamer/GES could not attach the {factory} effect."),
            format!("Inspect the clip effect stack and GStreamer {factory} availability."),
        )
        .with_detail("gstreamerError", error.to_string())]
    })?;
    Ok(effect)
}

#[cfg(feature = "ges-render")]
fn apply_visual_track_control_points(
    ges_clip: &ges::Clip,
    child_property: &str,
    points: &[GesTimedControlPoint],
    index: usize,
    property_label: &str,
    timing: GesSourceTiming,
) -> PipelineResult<()> {
    bind_source_control_points(
        ges_clip,
        child_property,
        points,
        &format!("gstreamer.ges.clips[{index}].properties.{property_label}"),
        timing,
    )
}

/// Binds `points` (clip-local seconds) to a child property of the clip's
/// source track element, converting them to the source's internal time.
#[cfg(feature = "ges-render")]
fn bind_source_control_points(
    ges_clip: &ges::Clip,
    child_property: &str,
    points: &[GesTimedControlPoint],
    error_path: &str,
    timing: GesSourceTiming,
) -> PipelineResult<()> {
    let source = linear_control_source();
    let add_point = |seconds: f64, value: f64| unsafe {
        gst_timed_value_control_source_set(
            source.as_ptr() as *mut _,
            (seconds * 1_000_000_000.0).round() as u64,
            value,
        )
    };
    for &GesTimedControlPoint { seconds, value } in points {
        if add_point(timing.internal_seconds(seconds), value) == 0 {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                error_path,
                "GStreamer could not add a visual control point.",
                "Use a GStreamer runtime with controller support available.",
            )]);
        }
    }
    let track_element = ges_clip
        .children(false)
        .into_iter()
        // Effects are track elements too; the envelope belongs to the core source.
        .find_map(|child| child.dynamic_cast::<ges::Source>().ok())
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                error_path,
                "GStreamer/GES did not expose a video track element for visual controls.",
                "Use a GStreamer runtime with GES video source track elements available.",
            )]
        })?;
    // These control points are already expressed in the child property's native
    // units (pixels for geometry, unit values for alpha). A normalized `direct`
    // binding would map values outside 0..=1 to the property's maximum, which can
    // make animated dimensions enormous and crash the downstream video scaler.
    if !track_element.set_control_source(&source, child_property, "direct-absolute") {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            error_path,
            "GStreamer/GES could not bind the visual control envelope.",
            "Use a GStreamer runtime with GES track-element control binding support.",
        )]);
    }
    Ok(())
}

#[cfg(feature = "ges-render")]
fn apply_visual_effect_control_points(
    effect: &ges::Effect,
    property: &str,
    points: &[GesTimedControlPoint],
    index: usize,
    property_label: &str,
) -> PipelineResult<()> {
    let source = linear_control_source();
    for &GesTimedControlPoint { seconds, value } in points {
        let added = unsafe {
            gst_timed_value_control_source_set(
                source.as_ptr() as *mut _,
                (seconds * 1_000_000_000.0).round() as u64,
                value,
            )
        };
        if added == 0 {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.clips[{index}].properties.{property_label}"),
                "GStreamer could not add a visual effect control point.",
                "Use a GStreamer runtime with controller support available.",
            )]);
        }
    }
    // Effect envelopes likewise contain native property values (for example,
    // radians or crop pixels), not normalized controller values.
    if !effect.set_control_source(&source, property, "direct-absolute") {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            format!("gstreamer.ges.clips[{index}].properties.{property_label}"),
            "GStreamer/GES could not bind the visual effect control envelope.",
            "Use a GStreamer runtime with effect control binding support.",
        )]);
    }
    Ok(())
}

#[cfg(feature = "ges-render")]
fn visual_alpha_control_points(
    opacity: f64,
    duration_seconds: f64,
    fade: Option<GesVisualFadeEnvelope>,
    opacity_keyframes: &[GesTimedControlPoint],
) -> Vec<GesTimedControlPoint> {
    let mut event_times = vec![0.0, duration_seconds];
    if let Some(fade) = fade {
        event_times.push(fade.fade_in_seconds.min(duration_seconds));
        event_times.push((duration_seconds - fade.fade_out_seconds).max(fade.fade_in_seconds));
    }
    event_times.extend(opacity_keyframes.iter().map(|keyframe| keyframe.seconds));
    event_times.sort_by(f64::total_cmp);
    event_times.dedup_by(|left, right| (*left - *right).abs() <= f64::EPSILON);
    event_times
        .into_iter()
        .map(|seconds| GesTimedControlPoint {
            seconds,
            value: (interpolated_control_value(opacity_keyframes, seconds, opacity)
                * visual_fade_multiplier(fade, seconds))
            .clamp(0.0, 1.0),
        })
        .collect()
}

#[cfg(feature = "ges-render")]
pub(super) fn interpolated_control_value(
    keyframes: &[GesTimedControlPoint],
    seconds: f64,
    fallback: f64,
) -> f64 {
    let Some(first) = keyframes.first() else {
        return fallback;
    };
    if seconds < first.seconds {
        return fallback;
    }
    for pair in keyframes.windows(2) {
        let (left, right) = (pair[0], pair[1]);
        if seconds <= right.seconds {
            let progress = (seconds - left.seconds) / (right.seconds - left.seconds);
            return left.value + (right.value - left.value) * progress;
        }
    }
    keyframes
        .last()
        .map(|keyframe| keyframe.value)
        .unwrap_or(fallback)
}

#[cfg(feature = "ges-render")]
pub(super) fn visual_fade_multiplier(fade: Option<GesVisualFadeEnvelope>, seconds: f64) -> f64 {
    let Some(fade) = fade else {
        return 1.0;
    };
    let fade_in = if fade.fade_in_seconds > 0.0 && seconds < fade.fade_in_seconds {
        seconds / fade.fade_in_seconds
    } else {
        1.0
    };
    let fade_out_start = fade.duration_seconds - fade.fade_out_seconds;
    let fade_out = if fade.fade_out_seconds > 0.0 && seconds > fade_out_start {
        (fade.duration_seconds - seconds) / fade.fade_out_seconds
    } else {
        1.0
    };
    (fade_in * fade_out).clamp(0.0, 1.0)
}

/// A timed-value control source that interpolates linearly between points.
///
/// `GstInterpolationControlSource` defaults to `mode=none`, which holds each
/// value until the next point (verified on GStreamer 1.24.2), so envelopes
/// would step instead of ramping.
#[cfg(feature = "ges-render")]
fn linear_control_source() -> gst::ControlSource {
    let source = unsafe {
        // The controller crate currently requires a newer Rust version than this
        // app. GStreamer exposes this stable GObject constructor in libgstcontroller.
        let object = gst::glib::Object::from_glib_full(gst_interpolation_control_source_new());
        object.unsafe_cast::<gst::ControlSource>()
    };
    source.set_property_from_str("mode", "linear");
    source
}

#[cfg(feature = "ges-render")]
#[link(name = "gstcontroller-1.0")]
unsafe extern "C" {
    fn gst_interpolation_control_source_new() -> *mut gst::glib::gobject_ffi::GObject;
    fn gst_timed_value_control_source_set(
        source: *mut gst::glib::gobject_ffi::GObject,
        timestamp: u64,
        value: f64,
    ) -> i32;
}

#[cfg(feature = "ges-render")]
fn apply_audio_clip_properties(
    ges_clip: &ges::Clip,
    clip: &RenderClip,
    index: usize,
    transitions: &GesClipTransitions,
) -> PipelineResult<()> {
    let gain = audio_linear_gain_for_render_clip(clip, index)?;
    let fade = audio_fade_envelope_for_render_clip(clip, index)?;
    if fade.is_some() || transitions.is_transitioned() {
        let timeline_duration = render_clip_number_property(clip, "timelineDurationSeconds")
            .unwrap_or(clip.source_out - clip.source_in);
        let points =
            audio_gain_control_points(gain.unwrap_or(1.0), timeline_duration, fade, transitions);
        // GES audio sources have no speed effect, so their internal time is
        // `sourceIn + t`.
        return bind_source_control_points(
            ges_clip,
            "volume",
            &points,
            &format!("gstreamer.ges.audioClips[{index}].properties.volume"),
            GesSourceTiming {
                source_in: clip.source_in,
                speed: 1.0,
            },
        );
    }
    let Some(linear_gain) = gain else {
        return Ok(());
    };
    ges_clip
        .set_child_property("volume", linear_gain)
        .map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.audioClips[{index}].properties.volumeDb"),
                "GStreamer/GES could not apply audio clip gain.",
                "Use a GStreamer runtime with the GES audio volume child property available.",
            )
            .with_detail("gstreamerError", error.to_string())]
        })
}

/// Retimed audio renders from a pitch-preserved intermediate that render
/// preparation (`precompose::audio_retime`) substitutes at speed 1; GES has no
/// pitch-preserving audio time effect to apply the speed itself.
#[cfg(feature = "ges-render")]
fn reject_unprepared_audio_speed(clip: &RenderClip, index: usize) -> PipelineResult<()> {
    match render_clip_number_property(clip, "speed") {
        Some(speed) if (speed - 1.0).abs() > 0.000_001 => Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.audioClips[{index}].properties.speed"),
            "Retimed audio clips must be prepared before the GStreamer render.",
            "Render through project export, which prepares retimed audio with pitch preserved.",
        )]),
        _ => Ok(()),
    }
}

#[cfg(feature = "ges-render")]
fn render_clip_number_property(clip: &RenderClip, key: &str) -> Option<f64> {
    clip.properties
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .filter(|value| value.is_finite())
}

#[cfg(feature = "ges-render")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GesCanvasGeometry {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: i32,
    pub(super) height: i32,
}

#[cfg(feature = "ges-render")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GesCropInsets {
    top: i32,
    right: i32,
    bottom: i32,
    left: i32,
}

#[cfg(feature = "ges-render")]
#[derive(Debug, Clone, Copy, PartialEq)]
struct GesBasicColorGrade {
    brightness: f64,
    contrast: f64,
    saturation: f64,
}

#[cfg(feature = "ges-render")]
fn visual_geometry_for_render_clip(
    clip: &RenderClip,
    plan: &RenderPlan,
    index: usize,
) -> PipelineResult<Option<GesCanvasGeometry>> {
    let Some(transform) = clip
        .properties
        .get("transform")
        .and_then(serde_json::Value::as_object)
    else {
        return Ok(None);
    };
    let normalized = |key: &str, fallback: f64, minimum: f64| -> Result<f64, PipelineError> {
        let value = transform
            .get(key)
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(fallback);
        if !value.is_finite() || value < minimum || value > 1.0 {
            return Err(PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!("renderPlan.clips[{index}].properties.transform.{key}"),
                "Visual canvas transform values must be normalized.",
                "Use finite center values from 0.0 through 1.0 and positive width/height up to 1.0.",
            ));
        }
        Ok(value)
    };
    let center_x = normalized("centerX", 0.5, 0.0).map_err(|error| vec![error])?;
    let center_y = normalized("centerY", 0.5, 0.0).map_err(|error| vec![error])?;
    let width = normalized("width", 1.0, f64::MIN_POSITIVE).map_err(|error| vec![error])?;
    let height = normalized("height", 1.0, f64::MIN_POSITIVE).map_err(|error| vec![error])?;
    let width_pixels = (width * f64::from(plan.width)).round() as i32;
    let height_pixels = (height * f64::from(plan.height)).round() as i32;
    Ok(Some(GesCanvasGeometry {
        x: (center_x * f64::from(plan.width)).round() as i32 - width_pixels / 2,
        y: (center_y * f64::from(plan.height)).round() as i32 - height_pixels / 2,
        width: width_pixels,
        height: height_pixels,
    }))
}

#[cfg(feature = "ges-render")]
fn visual_flip_method_for_render_clip(
    clip: &RenderClip,
    index: usize,
) -> PipelineResult<Option<&'static str>> {
    let Some(transform) = clip
        .properties
        .get("transform")
        .and_then(serde_json::Value::as_object)
    else {
        return Ok(None);
    };
    let bool_property = |key: &str| -> Result<bool, PipelineError> {
        match transform.get(key) {
            None => Ok(false),
            Some(serde_json::Value::Bool(value)) => Ok(*value),
            Some(_) => Err(PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!("renderPlan.clips[{index}].properties.transform.{key}"),
                "Visual flip flags must be boolean values.",
                "Use true or false for visual flip controls.",
            )),
        }
    };
    let horizontal = bool_property("flipHorizontal").map_err(|error| vec![error])?;
    let vertical = bool_property("flipVertical").map_err(|error| vec![error])?;
    // `videoflip video-direction` takes GstVideoOrientationMethod nicks.
    Ok(match (horizontal, vertical) {
        (false, false) => None,
        (true, false) => Some("horiz"),
        (false, true) => Some("vert"),
        (true, true) => Some("180"),
    })
}

#[cfg(feature = "ges-render")]
fn visual_crop_for_render_clip(
    clip: &RenderClip,
    index: usize,
) -> PipelineResult<Option<GesCropInsets>> {
    let crop_value = |key: &str| -> Result<f64, PipelineError> {
        let value = render_clip_number_property(clip, key).unwrap_or(0.0);
        if !(0.0..1.0).contains(&value) {
            return Err(PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!("renderPlan.clips[{index}].properties.{key}"),
                "Visual crop values must be normalized from zero inclusive to one exclusive.",
                "Use finite crop inset fractions that leave visible source content.",
            ));
        }
        Ok(value)
    };
    let top = crop_value("cropTop").map_err(|error| vec![error])?;
    let right = crop_value("cropRight").map_err(|error| vec![error])?;
    let bottom = crop_value("cropBottom").map_err(|error| vec![error])?;
    let left = crop_value("cropLeft").map_err(|error| vec![error])?;
    if left == 0.0 && right == 0.0 && top == 0.0 && bottom == 0.0 {
        return Ok(None);
    }
    if left + right >= 1.0 || top + bottom >= 1.0 {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.crop"),
            "Visual crop insets must leave visible source content.",
            "Keep each horizontal and vertical crop pair below one.",
        )]);
    }
    let (width, height) = visual_source_dimensions_for_render_clip(clip, index)?;
    Ok(Some(GesCropInsets {
        top: (top * height).round() as i32,
        right: (right * width).round() as i32,
        bottom: (bottom * height).round() as i32,
        left: (left * width).round() as i32,
    }))
}

#[cfg(feature = "ges-render")]
fn visual_source_dimensions_for_render_clip(
    clip: &RenderClip,
    index: usize,
) -> PipelineResult<(f64, f64)> {
    let width = render_clip_number_property(clip, "sourceWidth").ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.sourceWidth"),
            "Visual crop requires source media dimensions.",
            "Probe the source media before exporting a cropped visual clip.",
        )]
    })?;
    let height = render_clip_number_property(clip, "sourceHeight").ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.sourceHeight"),
            "Visual crop requires source media dimensions.",
            "Probe the source media before exporting a cropped visual clip.",
        )]
    })?;
    if width < 1.0 || height < 1.0 {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.sourceDimensions"),
            "Visual crop requires positive source media dimensions.",
            "Probe the source media before exporting a cropped visual clip.",
        )]);
    }
    Ok((width, height))
}

#[cfg(feature = "ges-render")]
fn visual_opacity_for_render_clip(clip: &RenderClip, index: usize) -> PipelineResult<Option<f64>> {
    let Some(opacity) = render_clip_number_property(clip, "opacity") else {
        return Ok(None);
    };
    if !(0.0..=1.0).contains(&opacity) {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.opacity"),
            "Visual clip opacity must be between zero and one.",
            "Use a finite opacity value from 0.0 through 1.0.",
        )]);
    }
    Ok(Some(opacity))
}

#[cfg(feature = "ges-render")]
fn visual_opacity_keyframes_for_render_clip(
    clip: &RenderClip,
    index: usize,
) -> PipelineResult<Vec<GesTimedControlPoint>> {
    let Some(keyframes) = clip
        .properties
        .get("keyframes")
        .and_then(serde_json::Value::as_object)
        .and_then(|keyframes| keyframes.get("opacity"))
    else {
        return Ok(Vec::new());
    };
    let Some(keyframes) = keyframes.as_array() else {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.keyframes.opacity"),
            "Visual opacity keyframes must be an array.",
            "Use ordered keyframes with atSeconds and value fields.",
        )]);
    };
    let duration_seconds = render_clip_timeline_duration_seconds(clip, index)?;
    let mut points = Vec::with_capacity(keyframes.len());
    let mut previous_seconds = None;
    for (keyframe_index, keyframe) in keyframes.iter().enumerate() {
        let Some(keyframe) = keyframe.as_object() else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!("renderPlan.clips[{index}].properties.keyframes.opacity[{keyframe_index}]"),
                "Visual opacity keyframes must be objects.",
                "Use ordered keyframes with atSeconds and value fields.",
            )]);
        };
        let at_seconds = keyframe
            .get("atSeconds")
            .and_then(serde_json::Value::as_f64);
        let value = keyframe.get("value").and_then(serde_json::Value::as_f64);
        let Some((seconds, value)) = at_seconds.zip(value) else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!("renderPlan.clips[{index}].properties.keyframes.opacity[{keyframe_index}]"),
                "Visual opacity keyframes require finite atSeconds and value fields.",
                "Set each keyframe inside the clip duration with an opacity from 0.0 through 1.0.",
            )]);
        };
        if !seconds.is_finite()
            || !value.is_finite()
            || !(0.0..=duration_seconds).contains(&seconds)
            || !(0.0..=1.0).contains(&value)
            || previous_seconds.is_some_and(|previous| seconds <= previous)
        {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!("renderPlan.clips[{index}].properties.keyframes.opacity[{keyframe_index}]"),
                "Visual opacity keyframes must be ordered and inside the clip duration.",
                "Use strictly increasing keyframe times and opacity values from 0.0 through 1.0.",
            )]);
        }
        previous_seconds = Some(seconds);
        points.push(GesTimedControlPoint { seconds, value });
    }
    Ok(points)
}

#[cfg(feature = "ges-render")]
fn visual_position_keyframes_for_render_clip(
    clip: &RenderClip,
    index: usize,
    property: &str,
) -> PipelineResult<Vec<GesTimedControlPoint>> {
    let Some(keyframes) = clip
        .properties
        .get("keyframes")
        .and_then(serde_json::Value::as_object)
        .and_then(|keyframes| keyframes.get(property))
    else {
        return Ok(Vec::new());
    };
    let Some(keyframes) = keyframes.as_array() else {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.keyframes.{property}"),
            "Visual position keyframes must be an array.",
            "Use ordered keyframes with atSeconds and value fields.",
        )]);
    };
    let duration_seconds = render_clip_timeline_duration_seconds(clip, index)?;
    let mut points = Vec::with_capacity(keyframes.len());
    let mut previous_seconds = None;
    for (keyframe_index, keyframe) in keyframes.iter().enumerate() {
        let Some(keyframe) = keyframe.as_object() else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!(
                    "renderPlan.clips[{index}].properties.keyframes.{property}[{keyframe_index}]"
                ),
                "Visual position keyframes must be objects.",
                "Use ordered keyframes with atSeconds and value fields.",
            )]);
        };
        let at_seconds = keyframe
            .get("atSeconds")
            .and_then(serde_json::Value::as_f64);
        let value = keyframe.get("value").and_then(serde_json::Value::as_f64);
        let Some((seconds, value)) = at_seconds.zip(value) else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!(
                    "renderPlan.clips[{index}].properties.keyframes.{property}[{keyframe_index}]"
                ),
                "Visual position keyframes require finite atSeconds and value fields.",
                "Set each keyframe inside the clip duration with a finite pixel offset.",
            )]);
        };
        if !seconds.is_finite()
            || !value.is_finite()
            || !(0.0..=duration_seconds).contains(&seconds)
            || !(-10_000.0..=10_000.0).contains(&value)
            || previous_seconds.is_some_and(|previous| seconds <= previous)
        {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!("renderPlan.clips[{index}].properties.keyframes.{property}[{keyframe_index}]"),
                "Visual position keyframes must be ordered and inside supported bounds.",
                "Use strictly increasing keyframe times and finite pixel offsets from -10000 through 10000.",
            )]);
        }
        previous_seconds = Some(seconds);
        points.push(GesTimedControlPoint { seconds, value });
    }
    Ok(points)
}

#[cfg(feature = "ges-render")]
fn visual_scale_keyframes_for_render_clip(
    clip: &RenderClip,
    index: usize,
) -> PipelineResult<Vec<GesTimedControlPoint>> {
    let Some(keyframes) = clip
        .properties
        .get("keyframes")
        .and_then(serde_json::Value::as_object)
        .and_then(|keyframes| keyframes.get("scale"))
    else {
        return Ok(Vec::new());
    };
    let Some(keyframes) = keyframes.as_array() else {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.keyframes.scale"),
            "Visual scale keyframes must be an array.",
            "Use ordered keyframes with atSeconds and value fields.",
        )]);
    };
    let duration_seconds = render_clip_timeline_duration_seconds(clip, index)?;
    let mut points = Vec::with_capacity(keyframes.len());
    let mut previous_seconds = None;
    for (keyframe_index, keyframe) in keyframes.iter().enumerate() {
        let Some(keyframe) = keyframe.as_object() else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!("renderPlan.clips[{index}].properties.keyframes.scale[{keyframe_index}]"),
                "Visual scale keyframes must be objects.",
                "Use ordered keyframes with atSeconds and value fields.",
            )]);
        };
        let at_seconds = keyframe
            .get("atSeconds")
            .and_then(serde_json::Value::as_f64);
        let value = keyframe.get("value").and_then(serde_json::Value::as_f64);
        let Some((seconds, value)) = at_seconds.zip(value) else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!("renderPlan.clips[{index}].properties.keyframes.scale[{keyframe_index}]"),
                "Visual scale keyframes require finite atSeconds and value fields.",
                "Set each keyframe inside the clip duration with a positive scale.",
            )]);
        };
        if !seconds.is_finite()
            || !value.is_finite()
            || !(0.0..=duration_seconds).contains(&seconds)
            || !(0.01..=100.0).contains(&value)
            || previous_seconds.is_some_and(|previous| seconds <= previous)
        {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!("renderPlan.clips[{index}].properties.keyframes.scale[{keyframe_index}]"),
                "Visual scale keyframes must be ordered and inside supported bounds.",
                "Use strictly increasing keyframe times and scales from 0.01 through 100.",
            )]);
        }
        previous_seconds = Some(seconds);
        points.push(GesTimedControlPoint { seconds, value });
    }
    Ok(points)
}

#[cfg(feature = "ges-render")]
fn visual_rotation_keyframes_for_render_clip(
    clip: &RenderClip,
    index: usize,
) -> PipelineResult<Vec<GesTimedControlPoint>> {
    let Some(keyframes) = clip
        .properties
        .get("keyframes")
        .and_then(serde_json::Value::as_object)
        .and_then(|keyframes| keyframes.get("rotationDegrees"))
    else {
        return Ok(Vec::new());
    };
    let Some(keyframes) = keyframes.as_array() else {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.keyframes.rotationDegrees"),
            "Visual rotation keyframes must be an array.",
            "Use ordered keyframes with atSeconds and value fields.",
        )]);
    };
    let duration_seconds = render_clip_timeline_duration_seconds(clip, index)?;
    let mut points = Vec::with_capacity(keyframes.len());
    let mut previous_seconds = None;
    for (keyframe_index, keyframe) in keyframes.iter().enumerate() {
        let Some(keyframe) = keyframe.as_object() else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!("renderPlan.clips[{index}].properties.keyframes.rotationDegrees[{keyframe_index}]"),
                "Visual rotation keyframes must be objects.",
                "Use ordered keyframes with atSeconds and value fields.",
            )]);
        };
        let at_seconds = keyframe
            .get("atSeconds")
            .and_then(serde_json::Value::as_f64);
        let value = keyframe.get("value").and_then(serde_json::Value::as_f64);
        let Some((seconds, value)) = at_seconds.zip(value) else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!("renderPlan.clips[{index}].properties.keyframes.rotationDegrees[{keyframe_index}]"),
                "Visual rotation keyframes require finite atSeconds and value fields.",
                "Set each keyframe inside the clip duration with a degree value from -360 through 360.",
            )]);
        };
        if !seconds.is_finite()
            || !value.is_finite()
            || !(0.0..=duration_seconds).contains(&seconds)
            || !(-360.0..=360.0).contains(&value)
            || previous_seconds.is_some_and(|previous| seconds <= previous)
        {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!("renderPlan.clips[{index}].properties.keyframes.rotationDegrees[{keyframe_index}]"),
                "Visual rotation keyframes must be ordered and inside supported bounds.",
                "Use strictly increasing keyframe times and degrees from -360 through 360.",
            )]);
        }
        previous_seconds = Some(seconds);
        points.push(GesTimedControlPoint { seconds, value });
    }
    Ok(points)
}

#[cfg(feature = "ges-render")]
fn visual_crop_keyframes_for_render_clip(
    clip: &RenderClip,
    index: usize,
    property: &str,
    source_dimension: f64,
) -> PipelineResult<Vec<GesTimedControlPoint>> {
    let Some(keyframes) = clip
        .properties
        .get("keyframes")
        .and_then(serde_json::Value::as_object)
        .and_then(|keyframes| keyframes.get(property))
    else {
        return Ok(Vec::new());
    };
    let Some(keyframes) = keyframes.as_array() else {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.keyframes.{property}"),
            "Visual crop keyframes must be an array.",
            "Use ordered keyframes with atSeconds and value fields.",
        )]);
    };
    let duration_seconds = render_clip_timeline_duration_seconds(clip, index)?;
    let mut points = Vec::with_capacity(keyframes.len());
    let mut previous_seconds = None;
    for (keyframe_index, keyframe) in keyframes.iter().enumerate() {
        let Some(keyframe) = keyframe.as_object() else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!(
                    "renderPlan.clips[{index}].properties.keyframes.{property}[{keyframe_index}]"
                ),
                "Visual crop keyframes must be objects.",
                "Use ordered keyframes with atSeconds and value fields.",
            )]);
        };
        let at_seconds = keyframe
            .get("atSeconds")
            .and_then(serde_json::Value::as_f64);
        let value = keyframe.get("value").and_then(serde_json::Value::as_f64);
        let Some((seconds, value)) = at_seconds.zip(value) else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!(
                    "renderPlan.clips[{index}].properties.keyframes.{property}[{keyframe_index}]"
                ),
                "Visual crop keyframes require finite atSeconds and value fields.",
                "Set each keyframe inside the clip duration with a normalized crop value.",
            )]);
        };
        if !seconds.is_finite()
            || !value.is_finite()
            || !(0.0..=duration_seconds).contains(&seconds)
            || !(0.0..1.0).contains(&value)
            || previous_seconds.is_some_and(|previous| seconds <= previous)
        {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!("renderPlan.clips[{index}].properties.keyframes.{property}[{keyframe_index}]"),
                "Visual crop keyframes must be ordered and inside supported bounds.",
                "Use strictly increasing keyframe times and crop values from zero inclusive to one exclusive.",
            )]);
        }
        previous_seconds = Some(seconds);
        points.push(GesTimedControlPoint {
            seconds,
            value: (value * source_dimension).round(),
        });
    }
    Ok(points)
}

#[cfg(feature = "ges-render")]
fn visual_fade_envelope_for_render_clip(
    clip: &RenderClip,
    index: usize,
) -> PipelineResult<Option<GesVisualFadeEnvelope>> {
    let fade_in_seconds = render_clip_number_property(clip, "fadeInSeconds").unwrap_or(0.0);
    let fade_out_seconds = render_clip_number_property(clip, "fadeOutSeconds").unwrap_or(0.0);
    // Fades belong to the canonical clip, without transition handles.
    let duration_seconds = (render_clip_timeline_duration_seconds(clip, index)?
        - render_clip_handle_seconds(clip))
    .max(0.0);
    if !fade_in_seconds.is_finite()
        || !fade_out_seconds.is_finite()
        || fade_in_seconds < 0.0
        || fade_out_seconds < 0.0
        || fade_in_seconds + fade_out_seconds > duration_seconds
    {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.fade"),
            "Visual fades must be non-negative and fit inside the clip duration.",
            "Keep fade-in plus fade-out no longer than the selected clip.",
        )]);
    }
    if fade_in_seconds == 0.0 && fade_out_seconds == 0.0 {
        return Ok(None);
    }
    Ok(Some(GesVisualFadeEnvelope {
        opacity: visual_opacity_for_render_clip(clip, index)?.unwrap_or(1.0),
        duration_seconds,
        fade_in_seconds,
        fade_out_seconds,
    }))
}

/// Transition handle seconds a render clip includes before and after its
/// canonical range.
#[cfg(feature = "ges-render")]
fn render_clip_handle_seconds(clip: &RenderClip) -> f64 {
    [
        TRANSITION_HEAD_SECONDS_PROPERTY,
        TRANSITION_TAIL_SECONDS_PROPERTY,
    ]
    .iter()
    .filter_map(|key| render_clip_number_property(clip, key))
    .filter(|seconds| *seconds > 0.0)
    .sum()
}

#[cfg(feature = "ges-render")]
fn audio_fade_envelope_for_render_clip(
    clip: &RenderClip,
    index: usize,
) -> PipelineResult<Option<GesVisualFadeEnvelope>> {
    let fade_in_seconds = render_clip_number_property(clip, "fadeInSeconds").unwrap_or(0.0);
    let fade_out_seconds = render_clip_number_property(clip, "fadeOutSeconds").unwrap_or(0.0);
    let timeline_duration = render_clip_number_property(clip, "timelineDurationSeconds")
        .unwrap_or(clip.source_out - clip.source_in);
    let duration_seconds = (timeline_duration - render_clip_handle_seconds(clip)).max(0.0);
    if fade_in_seconds < 0.0
        || fade_out_seconds < 0.0
        || fade_in_seconds + fade_out_seconds > duration_seconds + 1e-9
    {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.audioClips[{index}].properties.fade"),
            "Audio fades must be non-negative and fit inside the clip duration.",
            "Keep fade-in plus fade-out no longer than the selected audio clip.",
        )]);
    }
    if fade_in_seconds == 0.0 && fade_out_seconds == 0.0 {
        return Ok(None);
    }
    Ok(Some(GesVisualFadeEnvelope {
        opacity: 1.0,
        duration_seconds,
        fade_in_seconds,
        fade_out_seconds,
    }))
}

#[cfg(feature = "ges-render")]
fn visual_speed_for_render_clip(clip: &RenderClip, index: usize) -> PipelineResult<Option<f64>> {
    let Some(speed) = render_clip_number_property(clip, "speed") else {
        return Ok(None);
    };
    if !speed.is_finite() || !(0.1..=8.0).contains(&speed) {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.speed"),
            "Visual clip speed must be between 0.1 and 8.",
            "Use a finite visual speed from 0.1 through 8.",
        )]);
    }
    Ok((speed != 1.0).then_some(speed))
}

#[cfg(feature = "ges-render")]
fn visual_blend_operator_for_render_clip(
    clip: &RenderClip,
    index: usize,
) -> PipelineResult<Option<i32>> {
    let Some(value) = clip.properties.get("blendMode") else {
        return Ok(None);
    };
    let Some(blend_mode) = value.as_str() else {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.blendMode"),
            "Visual blend mode must be a supported string.",
            "Use normal or add from the visual inspector.",
        )]);
    };
    match blend_mode {
        "over" | "normal" => Ok(None),
        _ => Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.blendMode"),
            "Visual blend mode must be prepared by the CPU frame compositor before GES rendering.",
            "Run the project through the reusable precompose stage before building the GES plan.",
        )]),
    }
}

#[cfg(feature = "ges-render")]
fn visual_color_grade_for_render_clip(
    clip: &RenderClip,
    index: usize,
) -> PipelineResult<Option<GesBasicColorGrade>> {
    let Some(grade) = clip
        .properties
        .get("colorGrade")
        .and_then(serde_json::Value::as_object)
    else {
        return Ok(None);
    };
    let value = |key: &str, fallback: f64, minimum: f64, maximum: f64| {
        let value = grade
            .get(key)
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(fallback);
        if !value.is_finite() || !(minimum..=maximum).contains(&value) {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidClip,
                format!("renderPlan.clips[{index}].properties.colorGrade.{key}"),
                "Basic visual color-grade values are outside their supported range.",
                "Use exposure from -3 through 3, contrast from 0.5 through 1.5, and saturation from 0 through 2.",
            )]);
        }
        Ok(value)
    };
    let exposure = value("exposure", 0.0, -3.0, 3.0)?;
    let contrast = value("contrast", 1.0, 0.5, 1.5)?;
    let saturation = value("saturation", 1.0, 0.0, 2.0)?;
    if exposure == 0.0 && contrast == 1.0 && saturation == 1.0 {
        return Ok(None);
    }

    // videobalance brightness is bounded to [-1, 1], while the canonical
    // exposure control is expressed in stops. This monotonic mapping keeps
    // neutral at zero and approaches the native element's safe limits.
    let brightness = if exposure >= 0.0 {
        1.0 - 2_f64.powf(-exposure)
    } else {
        2_f64.powf(exposure) - 1.0
    };
    Ok(Some(GesBasicColorGrade {
        brightness,
        contrast,
        saturation,
    }))
}

#[cfg(feature = "ges-render")]
fn audio_linear_gain_for_render_clip(
    clip: &RenderClip,
    index: usize,
) -> PipelineResult<Option<f64>> {
    let Some(volume_db) = render_clip_number_property(clip, "volumeDb") else {
        return Ok(None);
    };
    if !(-60.0..=24.0).contains(&volume_db) {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.audioClips[{index}].properties.volumeDb"),
            "Audio clip gain must be between -60 dB and 24 dB.",
            "Use a finite gain value from -60 dB through 24 dB.",
        )]);
    }
    Ok(Some(10_f64.powf(volume_db / 20.0)))
}

#[cfg(feature = "ges-render")]
fn media_probe_from_discoverer(path: &Path, info: &gst_pbutils::DiscovererInfo) -> MediaProbe {
    let video = info
        .video_streams()
        .into_iter()
        .find(|stream| !stream.is_image())
        .or_else(|| info.video_streams().into_iter().next())
        .map(|stream| {
            let framerate = stream.framerate();
            VideoProbe {
                codec_name: codec_name_from_caps(stream.caps()),
                width: nonzero_u32(stream.width()),
                height: nonzero_u32(stream.height()),
                fps: fraction_to_fps(framerate),
            }
        });
    let audio = info
        .audio_streams()
        .into_iter()
        .next()
        .map(|stream| AudioProbe {
            codec_name: codec_name_from_caps(stream.caps()),
        });

    MediaProbe {
        container_name: info
            .stream_info()
            .and_then(|stream| codec_name_from_caps(stream.caps())),
        duration_seconds: info.duration().map(|duration| duration.seconds_f64()),
        size_bytes: std::fs::metadata(path).ok().map(|metadata| metadata.len()),
        video,
        audio,
    }
}

#[cfg(feature = "ges-render")]
fn codec_name_from_caps(caps: Option<gst::Caps>) -> Option<String> {
    caps.and_then(|caps| {
        caps.structure(0).map(|structure| {
            // GStreamer describes AAC and MP3 with the same audio/mpeg media type; the
            // mpegversion field distinguishes them, and delivery validation depends on it.
            if structure.name() == "audio/mpeg" {
                match structure.get::<i32>("mpegversion") {
                    Ok(2 | 4) => return "aac".to_string(),
                    Ok(1) => return "mp3".to_string(),
                    _ => {}
                }
            }
            structure.name().to_string()
        })
    })
}

#[cfg(feature = "ges-render")]
fn nonzero_u32(value: u32) -> Option<u32> {
    (value > 0).then_some(value)
}

#[cfg(feature = "ges-render")]
fn fraction_to_fps(fraction: gst::Fraction) -> Option<f64> {
    let denominator = fraction.denom();
    if denominator == 0 {
        return None;
    }
    let fps = fraction.numer() as f64 / denominator as f64;
    (fps.is_finite() && fps > 0.0).then_some(fps)
}

#[cfg(feature = "ges-render")]
enum RenderOutputTarget {
    Webm,
    Export(GstreamerOutputProfileTarget),
}

#[cfg(feature = "ges-render")]
fn render_output_target(plan: &RenderPlan) -> PipelineResult<RenderOutputTarget> {
    match plan.output_profile {
        RenderOutputProfile::WebPreview | RenderOutputProfile::WebDelivery => {
            validate_webm_output_path(&plan.output_path)?;
            Ok(RenderOutputTarget::Webm)
        }
        RenderOutputProfile::Mp4Primary => export_output_target(plan, ExportProfile::Mp4H264),
        RenderOutputProfile::Mp4Modern => export_output_target(plan, ExportProfile::Mp4H265),
        RenderOutputProfile::QuicktimeInterchange => {
            export_output_target(plan, ExportProfile::ProResMov)
        }
    }
}

#[cfg(feature = "ges-render")]
fn export_output_target(
    plan: &RenderPlan,
    profile: ExportProfile,
) -> PipelineResult<RenderOutputTarget> {
    let target = gstreamer_output_profile_target(profile)
        .ok_or_else(|| unsupported_export_profile_error(profile))?;
    validate_output_path_for_target(&plan.output_path, &target)?;
    Ok(RenderOutputTarget::Export(target))
}

#[cfg(feature = "ges-render")]
fn unsupported_export_profile_error(profile: ExportProfile) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        "gstreamer.ges.outputProfile",
        "Export profile is not supported by the GStreamer/GES backend.",
        "Choose an MP4 or ProRes export profile supported by GStreamer.",
    )
    .with_detail("profile", format!("{profile:?}"))]
}

#[cfg(feature = "ges-render")]
fn validate_webm_output_path(output_path: &str) -> PipelineResult<()> {
    let actual_extension = output_extension(output_path);
    if actual_extension.as_deref() == Some("webm") {
        return Ok(());
    }

    Err(vec![PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        "renderPlan.outputPath",
        "GStreamer/GES output path extension does not match the selected target profile.",
        "Use an output path with the expected extension for the selected render profile.",
    )
    .with_detail("outputPath", output_path.to_string())
    .with_detail("expectedExtension", "webm")
    .with_detail("actualExtension", actual_extension.unwrap_or_default())
    .with_detail(
        "profile",
        format!("{:?}", RenderOutputProfile::WebPreview),
    )])
}

#[cfg(feature = "ges-render")]
fn validate_output_path_for_target(
    output_path: &str,
    target: &GstreamerOutputProfileTarget,
) -> PipelineResult<()> {
    let actual_extension = output_extension(output_path);
    if actual_extension
        .as_deref()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(target.extension))
    {
        return Ok(());
    }

    Err(vec![PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        "renderPlan.outputPath",
        "GStreamer/GES output path extension does not match the selected target profile.",
        "Use an output path with the expected extension for the selected export profile.",
    )
    .with_detail("outputPath", output_path.to_string())
    .with_detail("expectedExtension", target.extension.to_string())
    .with_detail("actualExtension", actual_extension.unwrap_or_default())
    .with_detail("profile", format!("{:?}", target.profile))])
}

#[cfg(feature = "ges-render")]
fn output_extension(output_path: &str) -> Option<String> {
    Path::new(output_path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
}

#[cfg(feature = "ges-render")]
pub(crate) fn require_allowed_factories(factory_names: &[&str]) -> PipelineResult<()> {
    init_gstreamer()?;
    let mut errors = Vec::new();
    for factory_name in factory_names {
        match gst::ElementFactory::find(factory_name) {
            Some(factory) => {
                let plugin = factory.plugin();
                let info = GstFactoryInfo::new(*factory_name)
                    .plugin_name(
                        factory
                            .plugin_name()
                            .map(|name| name.to_string())
                            .unwrap_or_default(),
                    )
                    .package(
                        plugin
                            .as_ref()
                            .map(|plugin| plugin.package().to_string())
                            .unwrap_or_default(),
                    )
                    .license(
                        plugin
                            .as_ref()
                            .map(|plugin| plugin.license().to_string())
                            .unwrap_or_default(),
                    );
                if let Some(error) = policy_error_for_factory(&info) {
                    errors.push(error);
                }
            }
            None => errors.push(
                PipelineError::new(
                    PipelineErrorCode::RenderBackendUnavailable,
                    format!("gstreamer.plugins.{factory_name}"),
                    "Required GStreamer element factory is not available.",
                    "Update or reinstall Video Creater to restore its bundled render runtime.",
                )
                .with_detail("factory", (*factory_name).to_string()),
            ),
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(feature = "ges-render")]
fn init_gstreamer() -> PipelineResult<()> {
    render_runtime_environment().map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendUnavailable,
            "gstreamer.runtime",
            error.message,
            "Restore the bundled render runtime or configure a reviewed development runtime.",
        )
        .with_detail("code", error.code)
        .with_detail("detail", error.detail)]
    })?;
    Ok(())
}

#[cfg(feature = "ges-render")]
pub fn configure_gstreamer_runtime_environment() -> Result<(), String> {
    render_runtime_environment()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn render_plan_duration_seconds(plan: &RenderPlan) -> f64 {
    let mut cursor_seconds = 0.0;
    let mut duration_seconds: f64 = 0.0;
    for clip in &plan.clips {
        let clip_duration_seconds = clip
            .properties
            .get("timelineDurationSeconds")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(clip.source_out - clip.source_in);
        let start_seconds = clip.timeline_start_seconds.unwrap_or(cursor_seconds);
        let end_seconds = start_seconds + clip_duration_seconds;
        cursor_seconds = end_seconds;
        duration_seconds = duration_seconds.max(end_seconds);
    }
    duration_seconds
}

#[cfg(feature = "ges-render")]
fn filename_to_uri(path: &Path, error_path: &str) -> PipelineResult<glib::GString> {
    glib::filename_to_uri(path, None).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            error_path,
            "Could not convert a render path to a file URI for GStreamer.",
            "Use an absolute or current working directory path supported by GLib file URIs.",
        )
        .with_detail("path", path.display().to_string())
        .with_detail("glibError", error.to_string())]
    })
}

#[cfg(feature = "ges-render")]
fn seconds_to_clock_time(seconds: f64, path: &'static str) -> PipelineResult<gst::ClockTime> {
    gst::ClockTime::try_from_seconds_f64(seconds).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            path,
            "Could not convert render timing to GStreamer clock time.",
            "Use finite, non-negative render timings within supported duration limits.",
        )
        .with_detail("seconds", seconds.to_string())
        .with_detail("clockError", error.to_string())]
    })
}

#[cfg(feature = "ges-render")]
fn webm_encoding_profile(plan: &RenderPlan) -> gst_pbutils::EncodingContainerProfile {
    let encoder_settings = webm_encoder_settings(plan);
    let container_caps = gst::Caps::builder("video/webm").build();
    let video_caps = gst::Caps::builder(encoder_settings.video_format).build();
    let audio_caps = gst::Caps::builder("audio/x-opus").build();
    let framerate = fps_fraction(plan.fps);
    let video_restriction = gst::Caps::builder("video/x-raw")
        .field("width", plan.width as i32)
        .field("height", plan.height as i32)
        .field("framerate", framerate)
        .build();
    let video_profile = gst_pbutils::EncodingVideoProfile::builder(&video_caps)
        .restriction(&video_restriction)
        .element_properties(webm_video_element_properties(&encoder_settings))
        .build();
    let audio_profile = gst_pbutils::EncodingAudioProfile::builder(&audio_caps).build();

    gst_pbutils::EncodingContainerProfile::builder(&container_caps)
        .name(encoder_settings.profile_name)
        .add_profile(audio_profile)
        .add_profile(video_profile)
        .build()
}

#[cfg(feature = "ges-render")]
fn encoding_profile(
    plan: &RenderPlan,
    target: &GstreamerOutputProfileTarget,
) -> gst_pbutils::EncodingContainerProfile {
    let quality = plan_quality_settings(plan);
    let container_caps = profile_caps(target.container_caps);
    let video_caps = profile_caps(target.video_caps);
    let framerate = fps_fraction(plan.fps);
    let video_restriction = gst::Caps::builder("video/x-raw")
        .field("width", plan.width as i32)
        .field("height", plan.height as i32)
        .field("framerate", framerate)
        .build();
    let mut video_profile = gst_pbutils::EncodingVideoProfile::builder(&video_caps)
        .restriction(&video_restriction)
        .element_properties(quality_video_element_properties(
            target.video_factory,
            &quality,
        ));
    if let Some(factory) = pinned_encoder_factory(target.video_factory) {
        video_profile = video_profile.preset_name(factory);
    }
    let video_profile = video_profile.build();
    let mut container_profile = gst_pbutils::EncodingContainerProfile::builder(&container_caps)
        .name(ges_profile_name(target.profile))
        .add_profile(video_profile);

    if let Some(audio_caps) = target.audio_caps {
        let audio_caps = profile_caps(audio_caps);
        let mut audio_profile = gst_pbutils::EncodingAudioProfile::builder(&audio_caps)
            .element_properties_if_some(target.audio_factory.map(factory_element_properties));
        if let Some(factory) = target.audio_factory.and_then(pinned_encoder_factory) {
            audio_profile = audio_profile.preset_name(factory);
        }
        let audio_profile = audio_profile.build();
        container_profile = container_profile.add_profile(audio_profile);
    }

    container_profile.build()
}

/// On Linux the plugin set offers several encoders per format, so the encoding profile names
/// the reviewed factory; encodebin only considers encoders whose factory matches the preset name.
#[cfg(feature = "ges-render")]
fn pinned_encoder_factory(factory: &str) -> Option<&str> {
    (!cfg!(target_os = "macos")).then_some(factory)
}

/// Parses target caps such as `audio/mpeg,mpegversion=4`. Field constraints matter where one
/// media type covers several codecs, so encodebin cannot choose a different encoder.
#[cfg(feature = "ges-render")]
fn profile_caps(description: &str) -> gst::Caps {
    description
        .parse::<gst::Caps>()
        .unwrap_or_else(|_| gst::Caps::builder(caps_media_type(description)).build())
}

#[cfg(feature = "ges-render")]
fn caps_media_type(description: &str) -> &str {
    description.split(',').next().unwrap_or(description).trim()
}

#[cfg(feature = "ges-render")]
fn ges_profile_name(profile: ExportProfile) -> &'static str {
    match profile {
        ExportProfile::Webm => "video-creater-legacy-webm",
        ExportProfile::Mp4H264 => "video-creater-delivery-mp4-primary",
        ExportProfile::Mp4H265 => "video-creater-delivery-mp4-modern",
        ExportProfile::ProResMov => "video-creater-interchange-quicktime",
        ExportProfile::PalmierProject => "video-creater-unsupported-project",
    }
}

#[cfg(feature = "ges-render")]
fn factory_element_properties(factory_name: &str) -> gst_pbutils::ElementProperties {
    gst_pbutils::ElementProperties::builder_map()
        .item(gst_pbutils::ElementPropertiesMapItem::builder(factory_name).build())
        .build()
}

#[cfg(feature = "ges-render")]
fn quality_video_element_properties(
    factory_name: &str,
    quality: &super::quality::RenderQualitySettings,
) -> gst_pbutils::ElementProperties {
    let bitrate_kbps = quality.video_bitrate_kbps.unwrap_or(1_200);
    let realtime = quality.speed_hint == "draft-fast";
    let compression_quality = if realtime { 0.35 } else { 0.8 };

    let item = gst_pbutils::ElementPropertiesMapItem::builder(factory_name);
    // Only properties the selected encoder actually has are attached here; enum-valued
    // settings are applied by `configure_platform_encoder` when the encoder is created.
    let item = match factory_name {
        // OpenH264 expresses bitrate in bits per second.
        "openh264enc" => item.field("bitrate", bitrate_kbps.saturating_mul(1000)),
        "vah264enc" | "vah265enc" => item.field("bitrate", bitrate_kbps),
        "avenc_prores_ks" => item,
        _ => item
            .field("bitrate", bitrate_kbps)
            .field("quality", compression_quality)
            .field("realtime", realtime),
    };
    gst_pbutils::ElementProperties::builder_map()
        .item(item.build())
        .build()
}

/// encodebin chooses encoders and muxers by caps and rank. Distribution plugin sets include
/// encoders outside the reviewed policy, so every automatically selected encoder is checked.
#[cfg(feature = "ges-render")]
fn auto_selected_encoder_policy_error(element: &gst::Element) -> Option<PipelineError> {
    if cfg!(target_os = "macos") || element.is::<gst::Bin>() {
        return None;
    }
    let factory = element.factory()?;
    let class = factory.metadata("klass").unwrap_or_default();
    if !class.contains("Encoder") && !class.contains("Muxer") {
        return None;
    }
    let plugin = factory.plugin();
    let info = GstFactoryInfo::new(factory.name().to_string())
        .plugin_name(
            factory
                .plugin_name()
                .map(|name| name.to_string())
                .unwrap_or_default(),
        )
        .package(
            plugin
                .as_ref()
                .map(|plugin| plugin.package().to_string())
                .unwrap_or_default(),
        )
        .license(
            plugin
                .as_ref()
                .map(|plugin| plugin.license().to_string())
                .unwrap_or_default(),
        );
    policy_error_for_factory(&info)
}

#[cfg(feature = "ges-render")]
fn plan_quality_settings(plan: &RenderPlan) -> super::quality::RenderQualitySettings {
    effective_quality_settings_for_tier(
        plan.quality,
        plan.encode_tier,
        plan.width,
        plan.height,
        plan.fps,
    )
}

/// How hard the encoder works, from the plan's quality and encode tier.
///
/// On macOS the native AVFoundation exporter uses presets without a bitrate,
/// so Master equals Final there; that path can't be verified on Linux.
#[cfg(feature = "ges-render")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EncoderEffort {
    Realtime,
    Final,
    /// Carries the Master video bitrate in bits per second.
    Master {
        bitrate_bps: u32,
    },
}

#[cfg(feature = "ges-render")]
impl EncoderEffort {
    fn for_plan(plan: &RenderPlan) -> Self {
        let quality = plan_quality_settings(plan);
        match quality.speed_hint.as_str() {
            "draft-fast" => Self::Realtime,
            "master-quality" => Self::Master {
                bitrate_bps: quality
                    .video_bitrate_kbps
                    .unwrap_or(12_000)
                    .saturating_mul(1000),
            },
            _ => Self::Final,
        }
    }
}

/// Applies enum-valued encoder settings that encoding-profile property maps cannot express.
/// The Master settings were verified against the curated Linux runtime by
/// `linux_master_encoder_properties_exist_in_the_curated_runtime`; every
/// setting is still guarded by `find_property`.
#[cfg(feature = "ges-render")]
fn configure_platform_encoder(element: &gst::Element, effort: EncoderEffort) {
    let Some(factory) = element.factory() else {
        return;
    };
    let realtime = effort == EncoderEffort::Realtime;
    let master = matches!(effort, EncoderEffort::Master { .. });
    let factory_name = factory.name();
    let settings: &[(&str, &str)] = match factory_name.as_str() {
        "openh264enc" => &[
            ("rate-control", "bitrate"),
            ("usage-type", "camera"),
            ("complexity", if realtime { "low" } else { "high" }),
        ],
        "vah264enc" | "vah265enc" if master => &[("rate-control", "vbr"), ("target-usage", "1")],
        "vah264enc" | "vah265enc" => &[("rate-control", "vbr")],
        "vp8enc" if master => &[("auto-alt-ref", "true"), ("lag-in-frames", "25")],
        "avenc_prores_ks" => &[("profile", if realtime { "proxy" } else { "standard" })],
        _ => &[],
    };
    for (property, value) in settings {
        if element.find_property(property).is_some() {
            element.set_property_from_str(property, value);
        }
    }
    if let EncoderEffort::Master { bitrate_bps } = effort {
        if factory_name == "openh264enc" && element.find_property("max-bitrate").is_some() {
            let max_bitrate_bps = u32::try_from(u64::from(bitrate_bps) * 3 / 2).unwrap_or(u32::MAX);
            element.set_property("max-bitrate", max_bitrate_bps);
        }
    }
}

#[cfg(feature = "ges-render")]
#[derive(Debug, Clone, Copy, PartialEq)]
struct WebmEncoderSettings {
    profile_name: &'static str,
    video_format: &'static str,
    encoder_factory: &'static str,
    target_bitrate_bps: u32,
    deadline: i64,
    cpu_used: i32,
    threads: u32,
}

#[cfg(feature = "ges-render")]
fn webm_encoder_settings(plan: &RenderPlan) -> WebmEncoderSettings {
    let quality = plan_quality_settings(plan);
    let target_bitrate_bps = quality
        .video_bitrate_kbps
        .unwrap_or(1_200)
        .saturating_mul(1000);
    let threads = std::thread::available_parallelism()
        .map(|threads| threads.get().min(8) as u32)
        .unwrap_or(4);

    match (plan.quality, plan.encode_tier) {
        (RenderQuality::Final, ExportEncodeTier::Master) => WebmEncoderSettings {
            profile_name: "video-creater-master-web",
            video_format: "video/x-vp8",
            encoder_factory: "vp8enc",
            target_bitrate_bps,
            deadline: 1_000_000,
            cpu_used: 0,
            threads,
        },
        (RenderQuality::Draft, _) => WebmEncoderSettings {
            profile_name: "video-creater-preview-web",
            video_format: "video/x-vp8",
            encoder_factory: "vp8enc",
            target_bitrate_bps,
            deadline: 1,
            cpu_used: 8,
            threads,
        },
        (RenderQuality::Final, ExportEncodeTier::Standard) => WebmEncoderSettings {
            profile_name: "video-creater-delivery-web",
            video_format: "video/x-vp8",
            encoder_factory: "vp8enc",
            target_bitrate_bps,
            deadline: 1_000_000,
            cpu_used: 2,
            threads,
        },
    }
}

#[cfg(feature = "ges-render")]
fn webm_video_element_properties(settings: &WebmEncoderSettings) -> gst_pbutils::ElementProperties {
    gst_pbutils::ElementProperties::builder_map()
        .item(
            gst_pbutils::ElementPropertiesMapItem::builder(settings.encoder_factory)
                .field("target-bitrate", settings.target_bitrate_bps)
                .field("deadline", settings.deadline)
                .field("cpu-used", settings.cpu_used)
                .field("threads", settings.threads)
                .build(),
        )
        .build()
}

#[cfg(feature = "ges-render")]
fn summarize_encoding_profile(
    profile: &gst_pbutils::EncodingContainerProfile,
    target: &GstreamerOutputProfileTarget,
    plan: &RenderPlan,
) -> PipelineResult<EncodingProfileSummary> {
    let quality = plan_quality_settings(plan);
    let video_profile = encoding_profile_by_format(profile, target.video_caps, "video")?;
    let video_factory = profile_element_factory(&video_profile, target.video_factory, "video")?;
    let video_encoder_properties = video_profile
        .element_properties()
        .and_then(|properties| {
            properties
                .map()
                .and_then(|items| {
                    items
                        .into_iter()
                        .find(|item| item.name().as_str() == target.video_factory)
                })
        })
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "gstreamer.ges.encodingProfile.video.elementProperties.map",
                "GStreamer/GES encoding profile is missing the selected video encoder property map.",
                "Attach quality settings under the selected target video factory name.",
            )
            .with_detail("expectedFactory", target.video_factory.to_string())]
        })?;
    let audio_factory = match target.audio_factory {
        Some(expected_factory) => {
            let audio_caps = target.audio_caps.ok_or_else(|| {
                vec![PipelineError::new(
                    PipelineErrorCode::RenderBackendFailed,
                    "gstreamer.ges.encodingProfile.audio.caps",
                    "GStreamer/GES target has an audio factory but no audio caps.",
                    "Inspect GStreamer output profile target construction.",
                )]
            })?;
            let audio_profile = encoding_profile_by_format(profile, audio_caps, "audio")?;
            Some(profile_element_factory(
                &audio_profile,
                expected_factory,
                "audio",
            )?)
        }
        None => None,
    };

    Ok(EncodingProfileSummary {
        container_factory: target.container_factory.to_string(),
        video_factory,
        video_bitrate_kbps: summarized_bitrate_kbps(
            &video_encoder_properties,
            target.video_factory,
            &quality,
        )?,
        realtime: if video_encoder_properties.has_field("realtime") {
            video_encoder_properties
                .get::<bool>("realtime")
                .map_err(|error| {
                    vec![PipelineError::new(
                        PipelineErrorCode::RenderBackendFailed,
                        "gstreamer.ges.encodingProfile.video.elementProperties.realtime",
                        "GStreamer/GES video encoder profile has an invalid realtime property.",
                        "Set realtime as a boolean quality-effort value.",
                    )
                    .with_detail("gstreamerError", error.to_string())]
                })?
        } else {
            quality.speed_hint == "draft-fast"
        },
        audio_factory,
        parser_factories: target
            .parser_factories
            .iter()
            .map(|factory| (*factory).to_string())
            .collect(),
    })
}

#[cfg(feature = "ges-render")]
fn summarized_bitrate_kbps(
    properties: &gst::StructureRef,
    factory: &str,
    quality: &super::quality::RenderQualitySettings,
) -> PipelineResult<u32> {
    if !properties.has_field("bitrate") {
        return Ok(quality.video_bitrate_kbps.unwrap_or(1_200));
    }
    let bitrate = properties.get::<u32>("bitrate").map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "gstreamer.ges.encodingProfile.video.elementProperties.bitrate",
            "GStreamer/GES video encoder profile has an invalid bitrate property.",
            "Set bitrate as an unsigned kilobits-per-second value.",
        )
        .with_detail("gstreamerError", error.to_string())]
    })?;
    Ok(if factory == "openh264enc" {
        bitrate / 1000
    } else {
        bitrate
    })
}

#[cfg(feature = "ges-render")]
fn encoding_profile_by_format(
    profile: &gst_pbutils::EncodingContainerProfile,
    expected_caps: &str,
    component: &'static str,
) -> PipelineResult<gst_pbutils::EncodingProfile> {
    profile
        .profiles()
        .into_iter()
        .find(|profile| {
            profile.format().structure(0).is_some_and(|structure| {
                structure.name().as_str() == caps_media_type(expected_caps)
            })
        })
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.encodingProfile.{component}"),
                "GStreamer/GES encoding profile is missing the expected component caps.",
                "Inspect encoding profile construction for the selected output target.",
            )
            .with_detail("expectedCaps", expected_caps.to_string())]
        })
}

#[cfg(feature = "ges-render")]
fn profile_element_factory(
    profile: &gst_pbutils::EncodingProfile,
    expected_factory: &str,
    component: &'static str,
) -> PipelineResult<String> {
    let properties = profile.element_properties().ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            format!("gstreamer.ges.encodingProfile.{component}.elementProperties"),
            "GStreamer/GES encoding profile is missing encoder element properties.",
            "Attach encoder properties under the selected target factory name.",
        )]
    })?;
    let factory = properties
        .map()
        .and_then(|items| {
            items
                .into_iter()
                .find(|item| item.name().as_str() == expected_factory)
        })
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.encodingProfile.{component}.elementProperties.map"),
                "GStreamer/GES encoding profile is missing the selected target factory map.",
                "Attach encoder properties under the selected target factory name.",
            )
            .with_detail("expectedFactory", expected_factory.to_string())]
        })?;

    Ok(factory.name().as_str().to_string())
}

#[cfg(feature = "ges-render")]
fn summarize_webm_encoding_profile(
    profile: &gst_pbutils::EncodingContainerProfile,
) -> PipelineResult<WebmEncodingProfileSummary> {
    let video_profile = profile
        .profiles()
        .into_iter()
        .find(|profile| {
            profile
                .format()
                .structure(0)
                .is_some_and(|structure| structure.name().as_str().starts_with("video/x-vp"))
        })
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "gstreamer.ges.encodingProfile.video",
                "GStreamer/GES WebM profile is missing a VP8/VP9 video profile.",
                "Inspect WebM encoding profile construction.",
            )]
        })?;
    let video_format = video_profile
        .format()
        .structure(0)
        .map(|structure| structure.name().as_str().to_string())
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "gstreamer.ges.encodingProfile.video.format",
                "GStreamer/GES WebM video profile has no caps structure.",
                "Inspect WebM encoding profile construction.",
            )]
        })?;
    let properties = video_profile.element_properties().ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "gstreamer.ges.encodingProfile.video.elementProperties",
            "GStreamer/GES WebM video profile is missing encoder element properties.",
            "Attach VP8/VP9 encoder properties to the video encoding profile.",
        )]
    })?;
    let encoder_properties = properties
        .map()
        .and_then(|items| {
            items
                .into_iter()
                .find(|item| matches!(item.name().as_str(), "vp8enc" | "vp9enc"))
        })
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "gstreamer.ges.encodingProfile.video.elementProperties.map",
                "GStreamer/GES WebM video profile has no VP8/VP9 encoder property map.",
                "Attach encoder properties under the vp8enc or vp9enc factory name.",
            )]
        })?;

    Ok(WebmEncodingProfileSummary {
        video_format,
        encoder_factory: encoder_properties.name().as_str().to_string(),
        target_bitrate_bps: encoder_properties
            .get::<u32>("target-bitrate")
            .map_err(|error| {
                vec![PipelineError::new(
                    PipelineErrorCode::RenderBackendFailed,
                    "gstreamer.ges.encodingProfile.video.elementProperties.targetBitrate",
                    "GStreamer/GES WebM encoder profile has an invalid target bitrate property.",
                    "Set target-bitrate as an unsigned bits-per-second value.",
                )
                .with_detail("gstreamerError", error.to_string())]
            })?,
        deadline: encoder_properties.get::<i64>("deadline").map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "gstreamer.ges.encodingProfile.video.elementProperties.deadline",
                "GStreamer/GES WebM encoder profile has an invalid deadline property.",
                "Set deadline as a signed microsecond value.",
            )
            .with_detail("gstreamerError", error.to_string())]
        })?,
        cpu_used: encoder_properties.get::<i32>("cpu-used").map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "gstreamer.ges.encodingProfile.video.elementProperties.cpuUsed",
                "GStreamer/GES WebM encoder profile has an invalid cpu-used property.",
                "Set cpu-used as a signed integer value.",
            )
            .with_detail("gstreamerError", error.to_string())]
        })?,
    })
}

/// NTSC rates as the export popover offers them (23.976, 29.97, 59.94) and their exact fractions.
#[cfg(feature = "ges-render")]
const NTSC_FRAME_RATES: [(i32, i32); 3] = [(24_000, 1_001), (30_000, 1_001), (60_000, 1_001)];

#[cfg(feature = "ges-render")]
fn fps_fraction(fps: f64) -> gst::Fraction {
    if !fps.is_finite() || fps <= 0.0 {
        return gst::Fraction::new(30, 1);
    }
    if let Some((numerator, denominator)) =
        NTSC_FRAME_RATES
            .into_iter()
            .find(|(numerator, denominator)| {
                (fps - *numerator as f64 / *denominator as f64).abs() < 0.001
            })
    {
        return gst::Fraction::new(numerator, denominator);
    }
    let denominator = 1000;
    let mut numerator = (fps * denominator as f64).round() as i32;
    let mut denominator = denominator;
    let divisor = gcd_i32(numerator.abs(), denominator);
    if divisor > 0 {
        numerator /= divisor;
        denominator /= divisor;
    }
    gst::Fraction::new(numerator, denominator)
}

#[cfg(feature = "ges-render")]
fn gcd_i32(mut left: i32, mut right: i32) -> i32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.abs()
}

#[cfg(feature = "ges-render")]
fn wait_for_pipeline(
    pipeline: &ges::Pipeline,
    timeout: Duration,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<()> {
    wait_for_gst_pipeline_with_cancellation(
        pipeline.upcast_ref::<gst::Pipeline>(),
        timeout,
        "gstreamer",
        cancellation,
    )
}

#[cfg(feature = "ges-render")]
fn wait_for_gst_pipeline(
    pipeline: &gst::Pipeline,
    timeout: Duration,
    error_path: &'static str,
) -> PipelineResult<()> {
    wait_for_gst_pipeline_with_cancellation(pipeline, timeout, error_path, None)
}

#[cfg(feature = "ges-render")]
fn wait_for_gst_pipeline_with_cancellation(
    pipeline: &gst::Pipeline,
    timeout: Duration,
    error_path: &'static str,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<()> {
    let bus = pipeline.bus().ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            format!("{error_path}.bus"),
            "GStreamer render pipeline did not expose a bus.",
            "Inspect GES pipeline setup before rendering.",
        )]
    })?;
    gst::ClockTime::try_from_seconds_f64(timeout.as_secs_f64()).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendTimeout,
            format!("{error_path}.timeout"),
            "Render timeout could not be represented as GStreamer clock time.",
            "Use a finite positive render timeout.",
        )
        .with_detail("clockError", error.to_string())]
    })?;
    let started_at = Instant::now();

    loop {
        if cancellation.is_some_and(RenderCancellationToken::is_cancelled) {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("{error_path}.cancelled"),
                "GStreamer/GES render was cancelled.",
                "Start a new render when you are ready to export again.",
            )]);
        }

        let elapsed = started_at.elapsed();
        if elapsed >= timeout {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderBackendTimeout,
                format!("{error_path}.timeout"),
                "GStreamer/GES render timed out.",
                "Shorten the render or inspect the GStreamer pipeline for a stalled element.",
            )
            .with_detail("timeoutSeconds", timeout.as_secs().to_string())]);
        }

        let remaining = timeout.saturating_sub(elapsed);
        let poll_duration = if cancellation.is_some() {
            remaining.min(GST_RENDER_CANCEL_POLL_INTERVAL)
        } else {
            remaining
        };
        let poll_timeout = gst::ClockTime::try_from_seconds_f64(poll_duration.as_secs_f64())
            .map_err(|error| {
                vec![PipelineError::new(
                    PipelineErrorCode::RenderBackendTimeout,
                    format!("{error_path}.timeout"),
                    "Render timeout could not be represented as GStreamer clock time.",
                    "Use a finite positive render timeout.",
                )
                .with_detail("clockError", error.to_string())]
            })?;

        match bus.timed_pop_filtered(
            poll_timeout,
            &[gst::MessageType::Eos, gst::MessageType::Error],
        ) {
            Some(message) => match message.view() {
                gst::MessageView::Eos(_) => return Ok(()),
                gst::MessageView::Error(error) => {
                    return Err(vec![PipelineError::new(
                        PipelineErrorCode::RenderBackendFailed,
                        format!("{error_path}.bus.error"),
                        "GStreamer/GES render pipeline failed.",
                        "Inspect the GStreamer error, selected plugins, source media, and render settings.",
                    )
                    .with_detail("gstreamerError", error.error().to_string())
                    .with_detail(
                        "debug",
                        error
                            .debug()
                            .map(|debug| debug.to_string())
                            .unwrap_or_default(),
                    )])
                }
                _ => {
                    return Err(vec![PipelineError::new(
                        PipelineErrorCode::RenderBackendFailed,
                        format!("{error_path}.bus.message"),
                        "GStreamer/GES render pipeline returned an unexpected bus message.",
                        "Inspect backend logs and retry the render.",
                    )
                    .with_detail("messageType", format!("{:?}", message.type_()))])
                }
            },
            None if cancellation.is_none() => {
                return Err(vec![PipelineError::new(
                    PipelineErrorCode::RenderBackendTimeout,
                    format!("{error_path}.timeout"),
                    "GStreamer/GES render timed out.",
                    "Shorten the render or inspect the GStreamer pipeline for a stalled element.",
                )
                .with_detail("timeoutSeconds", timeout.as_secs().to_string())]);
            }
            None => {
                if let Some(token) = cancellation {
                    report_gst_pipeline_progress(pipeline, token);
                }
            }
        }
    }
}

/// Reports encode progress as position over duration, scaled to 0..=0.95. Validation and report
/// writing follow the encode, so 1.0 is left for the job's completion.
#[cfg(feature = "ges-render")]
fn report_gst_pipeline_progress(pipeline: &gst::Pipeline, token: &RenderCancellationToken) {
    let position = pipeline.query_position::<gst::ClockTime>();
    let duration = pipeline.query_duration::<gst::ClockTime>();
    if let (Some(position), Some(duration)) = (position, duration) {
        if duration.nseconds() > 0 {
            let fraction = position.nseconds() as f64 / duration.nseconds() as f64;
            token.report_progress(0.95 * fraction.min(1.0));
        }
    }
}

#[cfg(feature = "ges-render")]
fn quote_for_gst_parse_launch(path: &Path) -> String {
    let escaped = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(all(test, feature = "ges-render"))]
mod tests {
    use super::*;
    use crate::render_pipeline::cancel::{
        register_render_attempt, request_render_cancellation, RenderAttemptKey,
        RenderCancellationOutcome,
    };

    #[test]
    fn fps_fraction_snaps_ntsc_rates_to_their_exact_fractions() {
        for (fps, numerator, denominator) in [
            (23.976, 24_000, 1_001),
            (29.97, 30_000, 1_001),
            (59.94, 60_000, 1_001),
            (24_000.0 / 1_001.0, 24_000, 1_001),
            (24.0, 24, 1),
            (25.0, 25, 1),
            (30.0, 30, 1),
            (12.5, 25, 2),
        ] {
            assert_eq!(
                fps_fraction(fps),
                gst::Fraction::new(numerator, denominator),
                "{fps}"
            );
        }
    }

    #[test]
    fn cancellable_gstreamer_wait_returns_cancelled_before_timeout() {
        crate::render_runtime::start_render_runtime().expect("startup render runtime");
        init_gstreamer().expect("gstreamer init");
        let key = RenderAttemptKey::new(
            PathBuf::from("/tmp/render-backend-cancel-test"),
            "project",
            "render-backend-cancel-test",
            "attempt",
        )
        .expect("key");
        let guard = register_render_attempt(key.clone()).expect("register");
        let token = guard.token();
        assert_eq!(
            request_render_cancellation(&key),
            RenderCancellationOutcome::Requested
        );
        let pipeline = gst::Pipeline::new();

        let errors = wait_for_gst_pipeline_with_cancellation(
            &pipeline,
            Duration::from_secs(60),
            "gstreamer.test",
            Some(&token),
        )
        .expect_err("cancelled wait");

        assert_eq!(errors[0].path, "gstreamer.test.cancelled");
    }

    #[test]
    fn canonical_opacity_and_volume_properties_validate_before_ges_mutation() {
        let visual = RenderClip {
            source_path: None,
            timeline_start_seconds: None,
            properties: std::collections::BTreeMap::from([(
                "opacity".to_string(),
                serde_json::json!(0.35),
            )]),
            timeline_track_index: 0,
            source_in: 0.0,
            source_out: 1.0,
        };
        let audio = RenderClip {
            source_path: None,
            timeline_start_seconds: None,
            properties: std::collections::BTreeMap::from([(
                "volumeDb".to_string(),
                serde_json::json!(-6.0),
            )]),
            timeline_track_index: 0,
            source_in: 0.0,
            source_out: 1.0,
        };

        assert_eq!(visual_opacity_for_render_clip(&visual, 0), Ok(Some(0.35)));
        assert_eq!(
            audio_linear_gain_for_render_clip(&audio, 0),
            Ok(Some(10_f64.powf(-6.0 / 20.0)))
        );

        let invalid = RenderClip {
            properties: std::collections::BTreeMap::from([(
                "opacity".to_string(),
                serde_json::json!(1.1),
            )]),
            ..visual
        };
        let error = visual_opacity_for_render_clip(&invalid, 2).expect_err("invalid opacity");
        assert_eq!(error[0].path, "renderPlan.clips[2].properties.opacity");
    }

    #[cfg(feature = "ges-render")]
    #[test]
    fn canonical_over_is_native_and_dependent_blends_require_precomposition() {
        for blend_mode in ["over", "normal"] {
            let visual = RenderClip {
                source_path: None,
                timeline_start_seconds: None,
                properties: std::collections::BTreeMap::from([(
                    "blendMode".to_string(),
                    serde_json::json!(blend_mode),
                )]),
                timeline_track_index: 0,
                source_in: 0.0,
                source_out: 1.0,
            };
            assert_eq!(visual_blend_operator_for_render_clip(&visual, 0), Ok(None));
        }

        for blend_mode in ["source", "add", "multiply", "screen", "overlay"] {
            let invalid = RenderClip {
                source_path: None,
                timeline_start_seconds: None,
                properties: std::collections::BTreeMap::from([(
                    "blendMode".to_string(),
                    serde_json::json!(blend_mode),
                )]),
                timeline_track_index: 0,
                source_in: 0.0,
                source_out: 1.0,
            };
            let error =
                visual_blend_operator_for_render_clip(&invalid, 2).expect_err("prepared blend");
            assert_eq!(error[0].path, "renderPlan.clips[2].properties.blendMode");
        }
    }

    #[cfg(feature = "ges-render")]
    #[test]
    fn canonical_visual_fades_build_a_bounded_alpha_envelope() {
        let visual = RenderClip {
            source_path: None,
            timeline_start_seconds: None,
            properties: std::collections::BTreeMap::from([
                ("opacity".to_string(), serde_json::json!(0.75)),
                ("fadeInSeconds".to_string(), serde_json::json!(0.5)),
                ("fadeOutSeconds".to_string(), serde_json::json!(0.25)),
            ]),
            timeline_track_index: 0,
            source_in: 0.0,
            source_out: 2.0,
        };
        assert_eq!(
            visual_fade_envelope_for_render_clip(&visual, 0),
            Ok(Some(GesVisualFadeEnvelope {
                opacity: 0.75,
                duration_seconds: 2.0,
                fade_in_seconds: 0.5,
                fade_out_seconds: 0.25,
            }))
        );
    }

    #[cfg(feature = "ges-render")]
    #[test]
    fn canonical_opacity_keyframes_compose_with_visual_fades() {
        let visual = RenderClip {
            source_path: None,
            timeline_start_seconds: None,
            properties: std::collections::BTreeMap::from([
                ("opacity".to_string(), serde_json::json!(0.75)),
                ("fadeInSeconds".to_string(), serde_json::json!(0.5)),
                ("fadeOutSeconds".to_string(), serde_json::json!(0.25)),
                (
                    "keyframes".to_string(),
                    serde_json::json!({
                        "opacity": [
                            { "atSeconds": 0.0, "value": 0.2 },
                            { "atSeconds": 1.0, "value": 0.8 },
                            { "atSeconds": 2.0, "value": 0.4 }
                        ]
                    }),
                ),
            ]),
            timeline_track_index: 0,
            source_in: 0.0,
            source_out: 2.0,
        };
        let fade = visual_fade_envelope_for_render_clip(&visual, 0)
            .expect("valid fade")
            .expect("fade configured");
        let keyframes =
            visual_opacity_keyframes_for_render_clip(&visual, 0).expect("valid opacity keyframes");

        assert_eq!(
            visual_alpha_control_points(0.75, 2.0, Some(fade), &keyframes),
            vec![
                GesTimedControlPoint {
                    seconds: 0.0,
                    value: 0.0
                },
                GesTimedControlPoint {
                    seconds: 0.5,
                    value: 0.5
                },
                GesTimedControlPoint {
                    seconds: 1.0,
                    value: 0.8
                },
                GesTimedControlPoint {
                    seconds: 1.75,
                    value: 0.5
                },
                GesTimedControlPoint {
                    seconds: 2.0,
                    value: 0.0
                },
            ]
        );
    }

    #[cfg(feature = "ges-render")]
    #[test]
    fn canonical_visual_flip_flags_map_to_reviewed_videoflip_methods() {
        for (transform, method) in [
            (serde_json::json!({ "flipHorizontal": true }), "horiz"),
            (serde_json::json!({ "flipVertical": true }), "vert"),
            (
                serde_json::json!({ "flipHorizontal": true, "flipVertical": true }),
                "180",
            ),
        ] {
            let clip = RenderClip {
                source_path: None,
                timeline_start_seconds: None,
                properties: std::collections::BTreeMap::from([(
                    "transform".to_string(),
                    transform,
                )]),
                timeline_track_index: 0,
                source_in: 0.0,
                source_out: 1.0,
            };
            assert_eq!(
                visual_flip_method_for_render_clip(&clip, 0),
                Ok(Some(method))
            );
        }
    }

    #[cfg(feature = "ges-render")]
    #[test]
    fn canonical_basic_color_grade_maps_to_approved_videobalance_ranges() {
        let visual = RenderClip {
            source_path: None,
            timeline_start_seconds: None,
            properties: std::collections::BTreeMap::from([(
                "colorGrade".to_string(),
                serde_json::json!({ "exposure": 1.0, "contrast": 1.25, "saturation": 0.8 }),
            )]),
            timeline_track_index: 0,
            source_in: 0.0,
            source_out: 1.0,
        };

        assert_eq!(
            visual_color_grade_for_render_clip(&visual, 0),
            Ok(Some(GesBasicColorGrade {
                brightness: 0.5,
                contrast: 1.25,
                saturation: 0.8,
            }))
        );

        let invalid = RenderClip {
            properties: std::collections::BTreeMap::from([(
                "colorGrade".to_string(),
                serde_json::json!({ "contrast": 2.0 }),
            )]),
            ..visual
        };
        let error = visual_color_grade_for_render_clip(&invalid, 2).expect_err("invalid grade");
        assert_eq!(
            error[0].path,
            "renderPlan.clips[2].properties.colorGrade.contrast"
        );
    }

    #[test]
    fn normalized_canvas_transform_maps_to_ges_geometry() {
        let clip = RenderClip {
            source_path: None,
            timeline_start_seconds: None,
            properties: std::collections::BTreeMap::from([(
                "transform".to_string(),
                serde_json::json!({
                    "centerX": 0.4,
                    "centerY": 0.6,
                    "width": 0.5,
                    "height": 0.25
                }),
            )]),
            timeline_track_index: 0,
            source_in: 0.0,
            source_out: 1.0,
        };
        let plan = RenderPlan {
            input_path: "input.mov".to_string(),
            output_path: "output.webm".to_string(),
            width: 1920,
            height: 1080,
            fps: 30.0,
            quality: RenderQuality::Draft,
            output_profile: RenderOutputProfile::WebPreview,
            encode_tier: crate::edit::render_plan::ExportEncodeTier::Standard,
            clips: vec![clip.clone()],
            audio_clips: Vec::new(),
            transitions: Vec::new(),
            audio_transitions: Vec::new(),
        };

        assert_eq!(
            visual_geometry_for_render_clip(&clip, &plan, 0),
            Ok(Some(GesCanvasGeometry {
                x: 288,
                y: 513,
                width: 960,
                height: 270,
            }))
        );
    }

    #[test]
    fn normalized_crop_maps_to_source_pixel_insets() {
        let clip = RenderClip {
            source_path: None,
            timeline_start_seconds: None,
            properties: std::collections::BTreeMap::from([
                ("cropTop".to_string(), serde_json::json!(0.1)),
                ("cropRight".to_string(), serde_json::json!(0.2)),
                ("cropBottom".to_string(), serde_json::json!(0.15)),
                ("cropLeft".to_string(), serde_json::json!(0.05)),
                ("sourceWidth".to_string(), serde_json::json!(1920)),
                ("sourceHeight".to_string(), serde_json::json!(1080)),
            ]),
            timeline_track_index: 0,
            source_in: 0.0,
            source_out: 1.0,
        };

        assert_eq!(
            visual_crop_for_render_clip(&clip, 0),
            Ok(Some(GesCropInsets {
                top: 108,
                right: 384,
                bottom: 162,
                left: 96,
            }))
        );
    }
}
