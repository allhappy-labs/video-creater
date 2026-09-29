use super::PrecomposeReport;
use crate::process_supervisor::{
    run_to_completion, SupervisedCommand, SupervisionError, SupervisionPolicy,
};
use crate::project::model::{MediaAsset, MediaKind, Timeline, TimelineSource, VideoProject};
use crate::project::source_probe::probe_source_metadata;
use crate::render_pipeline::cancel::RenderCancellationToken;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use video_creater_compatibility_protocol::{
    CompatibilityFrameRequest, CompatibilityFrameResult, CompatibilityOperation,
    CompatibilityProfile, CompatibilityRequest, CompatibilityResult, MediaProbe, WorkerBudgets,
    WorkerEvent, COMPATIBILITY_PROTOCOL_NAME, COMPATIBILITY_PROTOCOL_VERSION,
};

pub(crate) fn extract_compatibility_frames(
    source: &Path,
    output_root: &Path,
    width: u32,
    height: u32,
    frames: Vec<CompatibilityFrameRequest>,
) -> Result<Vec<CompatibilityFrameResult>, String> {
    extract_compatibility_frames_with_publish_validator(
        source,
        output_root,
        width,
        height,
        frames,
        |_| Ok(()),
    )
}

pub(crate) fn extract_compatibility_frames_with_publish_validator(
    source: &Path,
    output_root: &Path,
    width: u32,
    height: u32,
    frames: Vec<CompatibilityFrameRequest>,
    validate_publish_path: impl FnMut(&Path) -> Result<(), String>,
) -> Result<Vec<CompatibilityFrameResult>, String> {
    extract_compatibility_frames_cancellable_with_publish_validator(
        source,
        output_root,
        width,
        height,
        frames,
        Duration::from_secs(300),
        None,
        validate_publish_path,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "compatibility extraction threads geometry, cancellation, and publish validation explicitly"
)]
pub(crate) fn extract_compatibility_frames_cancellable_with_publish_validator(
    source: &Path,
    output_root: &Path,
    width: u32,
    height: u32,
    frames: Vec<CompatibilityFrameRequest>,
    timeout: Duration,
    cancellation: Option<&RenderCancellationToken>,
    mut validate_publish_path: impl FnMut(&Path) -> Result<(), String>,
) -> Result<Vec<CompatibilityFrameResult>, String> {
    let outputs = prepare_frame_outputs(frames)?;
    let worker_frames = outputs
        .iter()
        .map(|output| CompatibilityFrameRequest {
            time_seconds: output.time_seconds,
            output_path: output.worker_path.to_string_lossy().into_owned(),
        })
        .collect();
    let worker_result = run_worker_operation(
        source,
        output_root,
        CompatibilityOperation::ExtractFrames {
            source_path: source.to_string_lossy().into_owned(),
            output_root: output_root.to_string_lossy().into_owned(),
            width,
            height,
            frames: worker_frames,
        },
        cancellation,
        timeout,
    );
    let worker_frames = match worker_result {
        Ok(CompatibilityResult::ExtractFrames { frames }) => frames,
        Ok(_) => {
            cleanup_failed_frame_outputs(&outputs);
            return Err("compatibility worker returned the wrong filmstrip result".into());
        }
        Err(errors) => {
            cleanup_failed_frame_outputs(&outputs);
            return Err(errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("; "));
        }
    };
    if worker_frames.len() != outputs.len() {
        cleanup_failed_frame_outputs(&outputs);
        return Err("compatibility worker returned an incomplete frame result".into());
    }

    let mut published = Vec::with_capacity(outputs.len());
    for (worker_frame, output) in worker_frames.into_iter().zip(&outputs) {
        let returned_path = fs::canonicalize(&worker_frame.output_path)
            .map_err(|error| format!("compatibility worker frame path is unreadable: {error}"));
        let expected_path = fs::canonicalize(&output.worker_path)
            .map_err(|error| format!("compatibility worker output is unreadable: {error}"));
        if returned_path.as_ref().ok() != expected_path.as_ref().ok()
            || returned_path.is_err()
            || expected_path.is_err()
        {
            cleanup_failed_frame_outputs(&outputs);
            return Err("compatibility worker returned an unexpected frame path".into());
        }
        let published_bytes = if output.worker_path == output.requested_path {
            Ok(worker_frame.bytes)
        } else {
            publish_compatibility_png_with_validator(
                &output.worker_path,
                &output.requested_path,
                &mut validate_publish_path,
            )
        };
        let bytes = match published_bytes {
            Ok(bytes) => bytes,
            Err(error) => {
                cleanup_failed_frame_outputs(&outputs);
                return Err(error);
            }
        };
        if output.worker_path != output.requested_path {
            let _ = fs::remove_file(&output.worker_path);
        }
        published.push(CompatibilityFrameResult {
            time_seconds: worker_frame.time_seconds,
            output_path: output.requested_path.to_string_lossy().into_owned(),
            bytes,
        });
    }
    Ok(published)
}

pub(crate) fn publish_compatibility_png_with_validator(
    worker_path: &Path,
    requested_path: &Path,
    validate_publish_path: &mut impl FnMut(&Path) -> Result<(), String>,
) -> Result<u64, String> {
    let image = image::open(worker_path)
        .map_err(|error| format!("compatibility BMP decode failed: {error}"))?;
    validate_publish_path(requested_path)?;
    image
        .save_with_format(requested_path, image::ImageFormat::Png)
        .map_err(|error| format!("compatibility PNG publication failed: {error}"))?;
    fs::metadata(requested_path)
        .map(|metadata| metadata.len())
        .map_err(|error| format!("compatibility PNG metadata failed: {error}"))
}

#[derive(Debug)]
struct CompatibilityFrameOutput {
    time_seconds: f64,
    requested_path: PathBuf,
    worker_path: PathBuf,
}

fn prepare_frame_outputs(
    frames: Vec<CompatibilityFrameRequest>,
) -> Result<Vec<CompatibilityFrameOutput>, String> {
    frames
        .into_iter()
        .map(|frame| {
            let requested_path = PathBuf::from(frame.output_path);
            let extension = requested_path
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase);
            let worker_path = match extension.as_deref() {
                Some("bmp") => requested_path.clone(),
                Some("png") => {
                    let parent = requested_path
                        .parent()
                        .ok_or("frame output has no parent")?;
                    parent.join(format!(".compatibility-frame-{}.bmp", uuid::Uuid::new_v4()))
                }
                _ => return Err("compatibility frame output must be .bmp or .png".into()),
            };
            Ok(CompatibilityFrameOutput {
                time_seconds: frame.time_seconds,
                requested_path,
                worker_path,
            })
        })
        .collect()
}

fn cleanup_failed_frame_outputs(outputs: &[CompatibilityFrameOutput]) {
    for output in outputs {
        let _ = fs::remove_file(&output.worker_path);
        let _ = fs::remove_file(&output.requested_path);
    }
}

const CACHE_VERSION: &str = "compatibility-h264-pcm-v2";

pub fn prepare_compatibility_media(
    project_dir: &Path,
    project: &mut VideoProject,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<Vec<PrecomposeReport>> {
    let candidates = project
        .media
        .iter()
        .filter(|media| {
            matches!(media.kind, MediaKind::Video | MediaKind::Audio)
                && needs_compatibility(&project_dir.join(&media.relative_path))
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut reports = Vec::new();
    for media in candidates {
        ensure_not_cancelled(cancellation)?;
        let source = project_dir.join(&media.relative_path);
        let bytes = fs::read(&source).map_err(|error| {
            compatibility_error(
                &media.id,
                format!("Compatibility source could not be read: {error}"),
            )
        })?;
        let fingerprint = format!(
            "{:x}",
            Sha256::digest([CACHE_VERSION.as_bytes(), bytes.as_slice()].concat())
        );
        let cache_dir = project_dir
            .join(".video-creater/cache/compatibility")
            .join(&fingerprint);
        fs::create_dir_all(&cache_dir)
            .map_err(|error| compatibility_error(&media.id, error.to_string()))?;
        let output = cache_dir.join("intermediate.mov");
        let cache_hit = output.is_file() && probe_source_metadata(&output).is_ok();
        let result = if cache_hit {
            None
        } else {
            Some(run_worker(&source, &cache_dir, &output, cancellation)?)
        };
        let prepared_id = format!("prepared-compat-{}-{}", media.id, &fingerprint[..16]);
        let probe = result.as_ref().and_then(|result| match result {
            CompatibilityResult::Transcode { probe, .. } => Some(probe),
            _ => None,
        });
        let relative_path = output
            .strip_prefix(project_dir)
            .map_err(|_| {
                compatibility_error(&media.id, "Compatibility output escaped the project cache.")
            })?
            .to_string_lossy()
            .into_owned();
        let prepared_media = MediaAsset {
            id: prepared_id.clone(),
            name: media.name.clone(),
            relative_path: relative_path.clone(),
            kind: media.kind.clone(),
            duration_seconds: probe
                .map(|value| value.duration_seconds)
                .filter(|value| *value > 0.0)
                .unwrap_or(media.duration_seconds),
            width: probe
                .and_then(|value| value.video.as_ref().map(|video| video.width))
                .or(media.width),
            height: probe
                .and_then(|value| value.video.as_ref().map(|video| video.height))
                .or(media.height),
            fps: probe
                .and_then(|value| value.video.as_ref().map(|video| video.fps))
                .or(media.fps),
            folder_id: media.folder_id.clone(),
        };
        project.media.retain(|entry| entry.id != prepared_id);
        project.media.push(prepared_media);
        rewrite_timeline(&mut project.timeline, &media.id, &prepared_id);
        for entry in &mut project.timelines {
            rewrite_timeline(&mut entry.timeline, &media.id, &prepared_id);
        }
        reports.push(PrecomposeReport {
            stage: "compatibilityDecode".into(),
            item_id: media.id.clone(),
            media_id: media.id.clone(),
            prepared_media_id: prepared_id,
            fingerprint,
            cache_hit,
            expressions_enabled: false,
            compositor_backend: Some("gstreamer-compatibility-sidecar".into()),
            compositor_fallback: None,
            worker_manifest: format!(
                "{}@{}",
                COMPATIBILITY_PROTOCOL_NAME, COMPATIBILITY_PROTOCOL_VERSION
            ),
            intermediate: relative_path,
        });
    }
    Ok(reports)
}

fn needs_compatibility(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("webm" | "mkv" | "avi" | "flv" | "ogv" | "ogg")
    ) && probe_source_metadata(path).is_err()
}
fn rewrite_timeline(timeline: &mut Timeline, source_id: &str, prepared_id: &str) {
    for item in timeline
        .tracks
        .iter_mut()
        .flat_map(|track| track.items.iter_mut())
    {
        if matches!(&item.source, TimelineSource::Media { media_id } if media_id == source_id) {
            item.source = TimelineSource::Media {
                media_id: prepared_id.to_string(),
            };
        }
    }
}

fn run_worker(
    source: &Path,
    output_root: &Path,
    output: &Path,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<CompatibilityResult> {
    run_worker_operation(
        source,
        output_root,
        CompatibilityOperation::Transcode {
            source_path: source.to_string_lossy().into_owned(),
            output_root: output_root.to_string_lossy().into_owned(),
            output_path: output.to_string_lossy().into_owned(),
            profile: CompatibilityProfile::H264PcmMov,
        },
        cancellation,
        Duration::from_secs(300),
    )
}

pub(crate) fn probe_compatibility_source(source: &Path) -> Option<MediaProbe> {
    probe_compatibility_source_within(source, Duration::from_secs(300)).ok()
}

pub(crate) fn probe_compatibility_source_within(
    source: &Path,
    timeout: Duration,
) -> PipelineResult<MediaProbe> {
    let registry_root = std::env::temp_dir();
    match run_worker_operation(
        source,
        &registry_root,
        CompatibilityOperation::Probe {
            source_path: source.to_string_lossy().into_owned(),
        },
        None,
        timeout,
    )? {
        CompatibilityResult::Probe { probe } => Ok(probe),
        _ => Err(compatibility_error(
            "worker",
            "Compatibility worker returned a non-probe result.",
        )),
    }
}

fn run_worker_operation(
    source: &Path,
    registry_root: &Path,
    operation: CompatibilityOperation,
    cancellation: Option<&RenderCancellationToken>,
    timeout: Duration,
) -> PipelineResult<CompatibilityResult> {
    let worker = worker_path()?;
    let env = compatibility_worker_environment(&worker, registry_root);
    let request = CompatibilityRequest {
        protocol: COMPATIBILITY_PROTOCOL_NAME.into(),
        schema_version: COMPATIBILITY_PROTOCOL_VERSION,
        request_id: format!(
            "compat-{}",
            &format!("{:x}", Sha256::digest(source.to_string_lossy().as_bytes()))[..16]
        ),
        operation,
        budgets: WorkerBudgets {
            timeout_millis: 300_000,
            max_output_bytes: 50 * 1024 * 1024 * 1024,
        },
    };
    let mut stdin = serde_json::to_vec(&request)
        .map_err(|error| compatibility_error("worker", error.to_string()))?;
    stdin.push(b'\n');
    let output = run_to_completion(
        SupervisedCommand {
            program: worker,
            args: Vec::new(),
            cwd: None,
            clear_env: true,
            env,
            env_remove: Vec::new(),
            stdin,
            stdout_limit: 1024 * 1024,
            stderr_limit: 64 * 1024,
        },
        SupervisionPolicy {
            deadline: timeout,
            ..SupervisionPolicy::default()
        },
        cancellation.map(|token| token as &dyn crate::process_supervisor::CancellationSignal),
    )
    .map_err(|error| match error {
        SupervisionError::Cancelled { stderr, .. } => compatibility_error(
            "worker",
            format!("Compatibility decode was cancelled. {stderr}"),
        ),
        SupervisionError::Deadline { stderr, .. } => compatibility_error(
            "worker",
            format!("Compatibility worker exceeded its parent-side deadline. {stderr}"),
        ),
        error => compatibility_error("worker", error.to_string()),
    })?;
    let line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    if line.trim().is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        let stderr = if stderr.is_empty() {
            "no stderr output".to_string()
        } else {
            stderr.to_string()
        };
        return Err(compatibility_error(
            "worker",
            format!(
                "Compatibility worker exited with {}: {stderr}",
                output.status
            ),
        ));
    }
    let event: WorkerEvent = serde_json::from_str(&line).map_err(|error| {
        compatibility_error("worker", format!("Invalid compatibility response: {error}"))
    })?;
    match event {
        WorkerEvent::Completed { result, .. } => Ok(result),
        WorkerEvent::Failed { error, .. } => Err(compatibility_error(&error.stage, error.message)),
        WorkerEvent::Progress { .. } => Err(compatibility_error(
            "worker",
            "Compatibility worker ended without completion.",
        )),
    }
}

/// Environment for the compatibility worker process. The worker starts from a cleared
/// environment so it can only load the reviewed plugin set of the bundled runtime.
pub(crate) fn compatibility_worker_environment(
    worker: &Path,
    registry_root: &Path,
) -> BTreeMap<String, String> {
    let mut env = BTreeMap::from([
        ("RUST_BACKTRACE".to_string(), "0".to_string()),
        ("ORC_CODE".to_string(), "backup".to_string()),
    ]);
    if let Some(tmp) = std::env::var_os("TMPDIR") {
        env.insert("TMPDIR".to_string(), tmp.to_string_lossy().into_owned());
    }
    let Some(runtime) = compatibility_runtime(worker) else {
        if !cfg!(target_os = "macos") {
            // Fail closed: never let the worker fall back to distribution plugin directories.
            env.insert("GST_PLUGIN_SYSTEM_PATH_1_0".to_string(), String::new());
            env.insert("GST_PLUGIN_PATH_1_0".to_string(), String::new());
        }
        return env;
    };
    if let Some(library_path) = runtime.library_path {
        env.insert(
            "DYLD_LIBRARY_PATH".to_string(),
            library_path.to_string_lossy().into_owned(),
        );
    }
    env.insert(
        "GST_PLUGIN_PATH_1_0".to_string(),
        runtime.plugin_path.to_string_lossy().into_owned(),
    );
    env.insert("GST_PLUGIN_SYSTEM_PATH_1_0".to_string(), String::new());
    if let Some(scanner) = runtime.scanner_path {
        env.insert(
            "GST_PLUGIN_SCANNER".to_string(),
            scanner.to_string_lossy().into_owned(),
        );
    }
    env.insert("GST_REGISTRY_FORK".to_string(), "no".to_string());
    let registry = runtime.registry_path.unwrap_or_else(|| {
        registry_root.join(format!(
            "video-creater-gst-registry-{}.bin",
            std::process::id()
        ))
    });
    env.insert(
        "GST_REGISTRY_1_0".to_string(),
        registry.to_string_lossy().into_owned(),
    );
    env
}

struct CompatibilityRuntime {
    plugin_path: PathBuf,
    library_path: Option<PathBuf>,
    scanner_path: Option<PathBuf>,
    registry_path: Option<PathBuf>,
}

fn compatibility_runtime(worker: &Path) -> Option<CompatibilityRuntime> {
    if let Some(root) = std::env::var_os("VIDEO_CREATER_COMPATIBILITY_RUNTIME").map(PathBuf::from) {
        return Some(macos_style_runtime(root));
    }
    #[cfg(target_os = "macos")]
    {
        worker
            .parent()?
            .parent()
            .map(|contents| contents.join("Resources/compatibility-runtime"))
            .filter(|resources| resources.is_dir())
            .or_else(|| {
                #[cfg(debug_assertions)]
                {
                    let workspace_runtime = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("resources/compatibility-runtime");
                    workspace_runtime.is_dir().then_some(workspace_runtime)
                }
                #[cfg(not(debug_assertions))]
                None
            })
            .map(macos_style_runtime)
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Linux shares the render runtime: system GStreamer core with the license-filtered
        // plugin directory and the bundled LGPL FFmpeg plugin. Its registry is shared too.
        let _ = worker;
        crate::render_runtime::helper_render_runtime_environment().map(|environment| {
            CompatibilityRuntime {
                plugin_path: environment.plugin_path,
                library_path: None,
                scanner_path: Some(environment.scanner_path),
                registry_path: Some(environment.registry_path.with_file_name(format!(
                    "compatibility-{}",
                    file_name_or_default(&environment.registry_path)
                ))),
            }
        })
    }
}

#[cfg(not(target_os = "macos"))]
fn file_name_or_default(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "registry.bin".to_string())
}

fn macos_style_runtime(root: PathBuf) -> CompatibilityRuntime {
    CompatibilityRuntime {
        plugin_path: root.join("plugins"),
        library_path: cfg!(target_os = "macos").then(|| root.join("lib")),
        scanner_path: None,
        registry_path: None,
    }
}

pub(crate) fn worker_path() -> PipelineResult<PathBuf> {
    if let Some(path) = std::env::var_os("VIDEO_CREATER_COMPATIBILITY_DECODER") {
        return Ok(path.into());
    }
    #[cfg(debug_assertions)]
    {
        let reviewed_workspace_worker = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "binaries/video-creater-compatibility-decoder-{}",
            crate::render_runtime::HOST_TARGET_TRIPLE
        ));
        if reviewed_workspace_worker
            .metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
        {
            return Ok(reviewed_workspace_worker);
        }
    }
    let current = std::env::current_exe()
        .map_err(|error| compatibility_error("worker", error.to_string()))?;
    let mut dir = current.parent().unwrap_or(Path::new("."));
    if dir.file_name().and_then(|v| v.to_str()) == Some("deps") {
        dir = dir.parent().unwrap_or(dir);
    }
    Ok(dir.join("video-creater-compatibility-decoder"))
}
fn ensure_not_cancelled(token: Option<&RenderCancellationToken>) -> PipelineResult<()> {
    if token.is_some_and(RenderCancellationToken::is_cancelled) {
        Err(compatibility_error(
            "cancelled",
            "Compatibility decode was cancelled.",
        ))
    } else {
        Ok(())
    }
}
fn compatibility_error(path: &str, message: impl Into<String>) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        format!("compatibility.{path}"),
        message,
        "Use the bundled reviewed compatibility runtime or choose system-decodable media.",
    )]
}
