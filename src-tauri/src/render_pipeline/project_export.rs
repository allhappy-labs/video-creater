use crate::edit::render_plan::{
    RenderClip, RenderOutputProfile, RenderPlan, RenderQuality, RenderQualityProfile,
    TemplateRenderLayer,
};
use crate::frame_compositor::PreparedEffectStack;
use crate::graphics::assets::AssetRegistry;
use crate::graphics::ir::{
    Color, Dimensions, GraphicNode, GraphicRole, GraphicsLayer, Rect, RectNode,
};
use crate::graphics::manifest::GraphicsArtifactManifest;
use crate::graphics::renderer::{render_graphics_preview_range_cancellable, GraphicsRenderOptions};
use crate::graphics::templates::template_layer_to_graphics_ir;
use crate::precompose::compatibility::extract_compatibility_frames_cancellable_with_publish_validator;
use crate::precompose::prepare_project_for_render_cancellable;
use crate::project::action::ProjectAction;
use crate::project::export_destination::ExportOutputRequest;
use crate::project::export_options::ExportRenderOptions;
use crate::project::export_profiles::ExportProfile;
use crate::project::model::{
    GeneratedAssetStatus, JobStatus, JobSummary, MediaAsset, MediaKind, ProjectExportArtifact,
    ProjectRenderPreviewComparison, ProjectRenderPreviewComparisonFrame,
    ProjectRenderPreviewComparisonRequest, ProjectRenderReport, RenderReportCheckStatus,
    RenderReportStatus, RenderReportStreams, Timeline, TimelineItem, TimelineItemKind,
    TimelineSource, TimelineTrack, TrackKind, VideoProject,
};
use crate::project::mutation::{
    acquire_split_project_artifact_lease, acquire_split_project_mutation_lease,
    SplitProjectArtifactLease, SplitProjectMutationLease,
};
use crate::project::nested_opacity::{
    apply_nested_opacity_curves, fade_opacity_curve_for_item, opacity_curve_for_item,
    AbsoluteOpacityCurve,
};
use crate::project::reverse::{is_reversed, truncated_tail_source_property, SourceWindow};
use crate::project::split::{
    apply_project_actions_to_split_project, load_split_project, validate_split_project_write_path,
    ProjectActionWriteResult,
};
use crate::settings::storage::{acquire_storage_mutation_lease, StorageMutationLease};
use std::collections::BTreeMap;
#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::avfoundation_backend::{
    avfoundation_profile, AvFoundationCapabilities, AvFoundationExportProfile,
    AvFoundationRenderBackend,
};
use super::backend::RenderBackend;
use super::cancel::{
    is_render_attempt_active, register_render_attempt, RenderAttemptGuard, RenderAttemptKey,
    RenderCancellationToken,
};
use super::error::{PipelineError, PipelineErrorCode, PipelineResult};
use super::gstreamer_backend::{probe_media_with_gstreamer, GstreamerGesRenderBackend};
use super::output_profile::gstreamer_output_profile_target;
use super::probe::{
    normalized_audio_codec_name, normalized_video_codec_name, validate_rendered_media,
    ExpectedMedia, MediaProbe,
};
use super::process::{CommandSpec, ProcessOutput, ProcessRunner, SystemProcessRunner};
use super::proposal::{caption_to_graphics_layer, overlay_to_graphics_layer};
use super::report::{
    write_json_report, write_markdown_report, RenderGraphicsReport, RenderPreviewComparisonRequest,
    RenderReport, RenderReportStreams as PipelineRenderReportStreams, RenderReportSummary,
};
use super::transition_plan::{carry_transitions_through_expansion, ClipKey, TransitionHandlePlan};
use serde_json::{json, Map, Value};
use video_creater_compatibility_protocol::CompatibilityFrameRequest;

// The Rust-sampler capture (and its graphics submodule) replaces the native one-frame render
// everywhere but macOS.
#[cfg(not(target_os = "macos"))]
mod canonical_capture;
mod named_export;

pub use named_export::{render_media_export_to_split_project_folder, MediaExportRequest};
mod admission;
pub use admission::{
    admit_media_render, read_media_render_attempt, recover_media_render_attempt,
    MediaRenderAdmission, MediaRenderAttempt, MediaRenderInput,
};

const PROJECT_RENDER_TIMEOUT: Duration = Duration::from_secs(3600);
const MIN_RENDER_STORAGE_RESERVE_BYTES: u64 = 256 * 1024 * 1024;
const RENDER_STORAGE_OVERRIDE_ENV: &str = "VIDEO_CREATER_RENDER_TEST_AVAILABLE_BYTES";

fn acquire_render_storage_mutation_lease() -> PipelineResult<StorageMutationLease> {
    acquire_storage_mutation_lease().map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "render.storageMutationLease",
            "The render writer could not acquire exclusive storage ownership.",
            "Retry after the active storage operation finishes.",
        )
        .with_detail("error", error)]
    })
}

fn acquire_render_project_mutation_lease(
    project_dir: &Path,
) -> PipelineResult<SplitProjectMutationLease> {
    acquire_split_project_mutation_lease(project_dir).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "render.projectMutationLease",
            "The render writer could not acquire canonical project ownership.",
            "Retry after the active project mutation finishes.",
        )
        .with_detail("error", error)]
    })
}

/// Ownership of the project's derived files for a whole render.
///
/// A render shares `renders/`, the precompose intermediates and their caches with every other
/// render, preparation and filmstrip pass, and those trees publish entries by directory rename,
/// which cannot happen twice at once. Renders therefore keep this lease from start to finish.
/// Canonical project writes need the mutation lease only, so they do not wait for it.
pub fn acquire_render_project_artifact_lease(
    project_dir: &Path,
) -> PipelineResult<SplitProjectArtifactLease> {
    acquire_split_project_artifact_lease(project_dir).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "render.projectArtifactLease",
            "The render writer could not acquire the project's render artifact ownership.",
            "Retry after the active render, preparation or filmstrip pass finishes.",
        )
        .with_detail("error", error)]
    })
}

fn validate_render_project_write_path(project_dir: &Path, path: &Path) -> PipelineResult<()> {
    validate_split_project_write_path(project_dir, path).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "projectDir",
            "The render output path crosses a symbolic link.",
            "Replace symlinked project artifact folders with contained directories, then retry.",
        )
        .with_detail("path", path.display().to_string())
        .with_detail("error", error.to_string())]
    })
}

fn validate_rendered_frame_sample_destination(
    project_dir: &Path,
    path: &Path,
) -> Result<(), String> {
    validate_render_project_write_path(project_dir, path).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ")
    })
}

fn estimated_render_storage_bytes(
    width: u32,
    height: u32,
    fps: f64,
    duration_seconds: f64,
    output_profile: RenderOutputProfile,
) -> u64 {
    let encoded_bytes_per_pixel_frame = match output_profile {
        RenderOutputProfile::QuicktimeInterchange => 0.35,
        RenderOutputProfile::Mp4Modern => 0.018,
        RenderOutputProfile::Mp4Primary
        | RenderOutputProfile::WebPreview
        | RenderOutputProfile::WebDelivery => 0.025,
    };
    let encoded_estimate = (f64::from(width)
        * f64::from(height)
        * fps.max(1.0)
        * duration_seconds.max(1.0)
        * encoded_bytes_per_pixel_frame)
        .ceil();
    let scratch_frames = u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(4)
        .saturating_mul(8);
    MIN_RENDER_STORAGE_RESERVE_BYTES
        .saturating_add(encoded_estimate.min(u64::MAX as f64) as u64)
        .saturating_add(scratch_frames)
}

pub fn validate_render_storage_budget(
    project_dir: &Path,
    available_bytes: u64,
    required_bytes: u64,
) -> PipelineResult<()> {
    if available_bytes >= required_bytes {
        return Ok(());
    }
    Err(vec![PipelineError::new(
        PipelineErrorCode::RenderStorageInsufficient,
        "render.storage",
        "The project volume does not have enough free space for this render.",
        "Free project-volume storage or choose a shorter/lower-resolution render, then retry.",
    )
    .with_detail("projectDir", project_dir.display().to_string())
    .with_detail("availableBytes", available_bytes.to_string())
    .with_detail("requiredBytes", required_bytes.to_string())])
}

fn preflight_render_storage(
    project_dir: &Path,
    width: u32,
    height: u32,
    fps: f64,
    duration_seconds: f64,
    output_profile: RenderOutputProfile,
) -> PipelineResult<()> {
    let required_bytes =
        estimated_render_storage_bytes(width, height, fps, duration_seconds, output_profile);
    let available_bytes = render_available_space_bytes(project_dir)?;
    validate_render_storage_budget(project_dir, available_bytes, required_bytes)
}

fn render_available_space_bytes(project_dir: &Path) -> PipelineResult<u64> {
    if let Some(value) = std::env::var_os(RENDER_STORAGE_OVERRIDE_ENV) {
        return value
            .to_string_lossy()
            .parse::<u64>()
            .map_err(|error| {
                vec![PipelineError::new(
                    PipelineErrorCode::PipelineInputInvalid,
                    "render.storageOverride",
                    "The render storage test override is invalid.",
                    "Set VIDEO_CREATER_RENDER_TEST_AVAILABLE_BYTES to a non-negative integer byte count.",
                )
                .with_detail("error", error.to_string())]
            });
    }

    #[cfg(unix)]
    {
        let path = CString::new(project_dir.as_os_str().as_bytes()).map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                "render.storage",
                "The project path cannot be checked for free storage.",
                "Choose a project folder without embedded NUL bytes.",
            )
            .with_detail("error", error.to_string())]
        })?;
        let mut stats = std::mem::MaybeUninit::<libc::statfs>::uninit();
        let status = unsafe { libc::statfs(path.as_ptr(), stats.as_mut_ptr()) };
        if status != 0 {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderBackendUnavailable,
                "render.storage",
                "Free project-volume storage could not be determined.",
                "Verify that the project volume is mounted and readable, then retry.",
            )
            .with_detail("projectDir", project_dir.display().to_string())
            .with_detail("error", std::io::Error::last_os_error().to_string())]);
        }
        let stats = unsafe { stats.assume_init() };
        let available = (stats.f_bavail as u128).saturating_mul(stats.f_bsize as u128);
        Ok(available.min(u128::from(u64::MAX)) as u64)
    }

    #[cfg(not(unix))]
    {
        let _ = project_dir;
        Ok(u64::MAX)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectWebmRenderPaths {
    pub render_dir: String,
    pub output_path: String,
    pub json_report_path: String,
    pub markdown_report_path: String,
    pub log_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMediaRenderPaths {
    pub render_dir: String,
    pub output_path: String,
    pub json_report_path: String,
    pub markdown_report_path: String,
    pub log_path: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWebmRenderResult {
    pub project: VideoProject,
    pub render_report: RenderReport,
    pub project_render_report: ProjectRenderReport,
    pub output_path: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMediaRenderResult {
    pub project: VideoProject,
    pub render_report: RenderReport,
    pub project_render_report: ProjectRenderReport,
    pub output_path: String,
    /// The saved export when the render was given an output name and folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_artifact: Option<ProjectExportArtifact>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreparedPreviewFrameResult {
    pub project: VideoProject,
    pub playhead_seconds: f64,
    pub preview_frame: String,
    pub source_output: String,
    pub evidence_report: String,
    pub render_report: RenderReport,
    pub project_render_report: ProjectRenderReport,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectRenderReportEvidence {
    pub video_stream: bool,
    pub audio_stream: bool,
    pub audio_required: bool,
    pub expected_duration_seconds: Option<f64>,
    pub duration_tolerance_seconds: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InterruptedRenderRecoveryResult {
    pub project: VideoProject,
    pub recovered_job_ids: Vec<String>,
    pub recovery_logs: Vec<String>,
    pub quarantined_artifacts: Vec<String>,
}

pub fn recover_interrupted_local_render_jobs(
    project_dir: &Path,
    updated_at: &str,
) -> PipelineResult<InterruptedRenderRecoveryResult> {
    let lease = acquire_render_storage_mutation_lease()?;
    recover_interrupted_local_render_jobs_with_lease(project_dir, updated_at, &lease)
}

pub fn recover_interrupted_local_render_jobs_with_lease(
    project_dir: &Path,
    updated_at: &str,
    _lease: &StorageMutationLease,
) -> PipelineResult<InterruptedRenderRecoveryResult> {
    // Recovery quarantines render artifacts, so it owns them for its whole run; its canonical
    // writes take the mutation lease for themselves.
    let _artifacts = acquire_render_project_artifact_lease(project_dir)?;
    let _project_lease = acquire_render_project_mutation_lease(project_dir)?;
    let project = load_split_project(project_dir).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "projectDir",
            "Split project could not be loaded for interrupted-render recovery.",
            "Repair or restore the split project before retrying render recovery.",
        )
        .with_detail("error", error.to_string())]
    })?;
    let interrupted = project
        .jobs
        .iter()
        .filter(|job| {
            matches!(
                job.kind.as_str(),
                "render_draft" | "export_media" | "exportMedia" | "captureCanonicalPreviewFrame"
            ) && render_job_needs_recovery(project_dir, &project.id, job)
        })
        .map(|job| job.id.clone())
        .collect::<Vec<_>>();
    if interrupted.is_empty() {
        return Ok(InterruptedRenderRecoveryResult {
            project,
            recovered_job_ids: Vec::new(),
            recovery_logs: Vec::new(),
            quarantined_artifacts: Vec::new(),
        });
    }

    let mut actions = Vec::with_capacity(interrupted.len());
    let mut recovery_logs = Vec::with_capacity(interrupted.len());
    let mut quarantined_artifacts = Vec::new();
    for job_id in &interrupted {
        let render_dir_relative = format!("renders/{}", safe_path_segment(job_id));
        let render_dir = project_dir.join(&render_dir_relative);
        validate_render_project_write_path(project_dir, &render_dir)?;
        std::fs::create_dir_all(&render_dir).map_err(|error| {
            io_pipeline_error(
                "renders.recovery",
                "Create render recovery directory failed.",
                error,
            )
        })?;
        for extension in ["webm", "mp4", "mov"] {
            let output = render_dir.join(format!("output.{extension}"));
            validate_render_project_write_path(project_dir, &output)?;
            if !output.is_file() {
                continue;
            }
            let quarantined = render_dir.join(format!("output.{extension}.interrupted"));
            validate_render_project_write_path(project_dir, &quarantined)?;
            if quarantined.exists() {
                std::fs::remove_file(&quarantined).map_err(|error| {
                    io_pipeline_error(
                        "renders.recovery.quarantine",
                        "Remove stale interrupted render quarantine failed.",
                        error,
                    )
                })?;
            }
            std::fs::rename(&output, &quarantined).map_err(|error| {
                io_pipeline_error(
                    "renders.recovery.quarantine",
                    "Quarantine interrupted render output failed.",
                    error,
                )
            })?;
            quarantined_artifacts.push(
                quarantined
                    .strip_prefix(project_dir)
                    .unwrap_or(&quarantined)
                    .display()
                    .to_string(),
            );
        }
        let recovery_log_relative = format!("{render_dir_relative}/recovery.log");
        validate_render_project_write_path(project_dir, &project_dir.join(&recovery_log_relative))?;
        std::fs::write(
            project_dir.join(&recovery_log_relative),
            format!(
                "Stopped when the app closed.\n\
                 Render job {job_id} had not finished when the local app process stopped.\n\
                 Status was changed to failed and any unvalidated output was quarantined.\n\
                 Retry the render from the render review panel; the canonical project timeline was not changed.\n"
            ),
        )
        .map_err(|error| {
            io_pipeline_error(
                "renders.recovery.log",
                "Write interrupted render recovery log failed.",
                error,
            )
        })?;
        recovery_logs.push(recovery_log_relative);
        actions.push(ProjectAction::UpdateJobStatus {
            job_id: job_id.clone(),
            status: JobStatus::Failed,
            updated_at: updated_at.to_string(),
            run_id: None,
        });
    }
    let write = apply_project_actions_to_split_project(project_dir, actions).map_err(|error| {
        render_project_action_error(
            "interrupted",
            "Persisting interrupted render recovery failed.",
            error,
        )
    })?;
    Ok(InterruptedRenderRecoveryResult {
        project: write.project,
        recovered_job_ids: interrupted,
        recovery_logs,
        quarantined_artifacts,
    })
}

/// Whether an unfinished render, export or frame capture has no runner left in this process. Jobs
/// with any other run id, or queued without one while they wait for their start, belong to Temporal.
fn render_job_needs_recovery(project_dir: &Path, project_id: &str, job: &JobSummary) -> bool {
    let run_id = job
        .workflow
        .as_ref()
        .and_then(|workflow| workflow.run_id.as_deref());
    if !matches!(
        job.status,
        JobStatus::Queued | JobStatus::Running | JobStatus::Progress
    ) {
        return false;
    }
    match run_id {
        Some(attempt_id) if attempt_id.starts_with("render-attempt/") => {
            if admission::attempt_is_pinned(project_dir, &job.id, attempt_id) {
                return false;
            }
            let project_root =
                std::fs::canonicalize(project_dir).unwrap_or_else(|_| project_dir.to_path_buf());
            RenderAttemptKey::new(project_root, project_id, &job.id, attempt_id)
                .map_or(true, |key| !is_render_attempt_active(&key))
        }
        // The project package export records no attempt, but it holds the project lease while it
        // runs, so recovery never sees it live.
        None => job.status != JobStatus::Queued,
        Some(_) => false,
    }
}

pub fn load_project_render_pipeline_report(
    project_dir: &Path,
    job_id: &str,
) -> PipelineResult<RenderReport> {
    let report_path = project_dir
        .join("renders")
        .join(safe_path_segment(job_id))
        .join("pipeline-report.json");
    let source = std::fs::read_to_string(&report_path).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderArtifactMissing,
            "renders.pipelineReport",
            "The persisted render pipeline report could not be read.",
            "Retry the render to regenerate its failure report and log.",
        )
        .with_detail("path", report_path.display().to_string())
        .with_detail("error", error.to_string())]
    })?;
    let report = serde_json::from_str::<RenderReport>(&source).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderProbeInvalidJson,
            "renders.pipelineReport",
            "The persisted render pipeline report is invalid.",
            "Inspect the report artifact, then retry the render to replace it.",
        )
        .with_detail("path", report_path.display().to_string())
        .with_detail("error", error.to_string())]
    })?;
    if report.job_id != job_id {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderProbeValidationFailed,
            "renders.pipelineReport.jobId",
            "The persisted render pipeline report belongs to a different job.",
            "Do not reuse render report artifacts across job directories.",
        )
        .with_detail("expectedJobId", job_id.to_string())
        .with_detail("actualJobId", report.job_id)]);
    }
    Ok(report)
}

#[derive(Debug, Clone, PartialEq)]
struct ProjectGraphicsLayer {
    layer: GraphicsLayer,
    source_range: Option<(f64, f64)>,
    template_id: Option<String>,
    motion_preset_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct RenderedProjectGraphics {
    artifact_dir: PathBuf,
    manifest: GraphicsArtifactManifest,
    timeline_start_seconds: f64,
    template_id: Option<String>,
    motion_preset_id: Option<String>,
}

impl ProjectWebmRenderPaths {
    pub fn new(job_id: &str) -> Self {
        let safe_job_id = safe_path_segment(job_id);
        let render_dir = format!("renders/{safe_job_id}");

        Self {
            output_path: format!("{render_dir}/output.webm"),
            // `report.json` is reserved for the split-project render-report
            // sidecar. The backend's full pipeline evidence must live beside
            // it under a different name, otherwise the completion action
            // reloads a pipeline report as a project report.
            json_report_path: format!("{render_dir}/pipeline-report.json"),
            markdown_report_path: format!("{render_dir}/pipeline-report.md"),
            log_path: format!("{render_dir}/render.log"),
            render_dir,
        }
    }

    pub fn absolute_output_path(&self, project_dir: &Path) -> PathBuf {
        project_dir.join(&self.output_path)
    }

    pub fn absolute_render_dir(&self, project_dir: &Path) -> PathBuf {
        project_dir.join(&self.render_dir)
    }

    pub fn absolute_json_report_path(&self, project_dir: &Path) -> PathBuf {
        project_dir.join(&self.json_report_path)
    }

    pub fn absolute_markdown_report_path(&self, project_dir: &Path) -> PathBuf {
        project_dir.join(&self.markdown_report_path)
    }

    pub fn absolute_log_path(&self, project_dir: &Path) -> PathBuf {
        project_dir.join(&self.log_path)
    }
}

impl ProjectMediaRenderPaths {
    fn webm(job_id: &str) -> Self {
        Self::with_extension(job_id, "webm")
    }

    pub fn new(job_id: &str, profile: ExportProfile) -> PipelineResult<Self> {
        let target = gstreamer_output_profile_target(profile).ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                "exportProfile",
                "Export profile is not supported by the GStreamer project renderer.",
                "Choose MP4 H.264, MP4 H.265, or ProRes MOV for native project media rendering.",
            )
            .with_detail("profile", format!("{profile:?}"))]
        })?;
        Ok(Self::with_extension(job_id, target.extension))
    }

    fn with_extension(job_id: &str, extension: &str) -> Self {
        let safe_job_id = safe_path_segment(job_id);
        let render_dir = format!("renders/{safe_job_id}");

        Self {
            output_path: format!("{render_dir}/output.{extension}"),
            json_report_path: format!("{render_dir}/pipeline-report.json"),
            markdown_report_path: format!("{render_dir}/pipeline-report.md"),
            log_path: format!("{render_dir}/render.log"),
            render_dir,
        }
    }

    pub fn absolute_output_path(&self, project_dir: &Path) -> PathBuf {
        project_dir.join(&self.output_path)
    }

    pub fn absolute_render_dir(&self, project_dir: &Path) -> PathBuf {
        project_dir.join(&self.render_dir)
    }

    pub fn absolute_json_report_path(&self, project_dir: &Path) -> PathBuf {
        project_dir.join(&self.json_report_path)
    }

    pub fn absolute_markdown_report_path(&self, project_dir: &Path) -> PathBuf {
        project_dir.join(&self.markdown_report_path)
    }

    pub fn absolute_log_path(&self, project_dir: &Path) -> PathBuf {
        project_dir.join(&self.log_path)
    }
}

pub fn extract_rendered_frame_samples(
    runner: &impl ProcessRunner,
    project_dir: &Path,
    render_dir: &str,
    output_path: &str,
    timeline_seconds: &[f64],
    timeout: Duration,
) -> PipelineResult<Vec<String>> {
    let lease = acquire_render_storage_mutation_lease()?;
    let _artifacts = acquire_render_project_artifact_lease(project_dir)?;
    extract_rendered_frame_samples_with_lease(
        runner,
        project_dir,
        render_dir,
        output_path,
        timeline_seconds,
        timeout,
        None,
        &lease,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "frame sampling needs the render context and the held project lease explicitly"
)]
fn extract_rendered_frame_samples_with_lease(
    _runner: &impl ProcessRunner,
    project_dir: &Path,
    render_dir: &str,
    output_path: &str,
    timeline_seconds: &[f64],
    timeout: Duration,
    cancellation: Option<&RenderCancellationToken>,
    _lease: &StorageMutationLease,
) -> PipelineResult<Vec<String>> {
    // No project lease: frame sampling decodes the finished output and writes only
    // `renders/<jobId>/frames`. Its project read takes the lease for that read alone.
    if timeline_seconds.is_empty() {
        return Ok(Vec::new());
    }
    ensure_render_not_cancelled(cancellation, "renderedFrames.cancelled")?;

    let output_abs_path = project_dir.join(output_path);
    let project = load_split_project(project_dir).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "renderedFrames.project",
            "The split project could not be loaded for bundled frame extraction.",
            "Save the project and retry the render review.",
        )
        .with_detail("error", error.to_string())]
    })?;
    let frames_dir = format!("{render_dir}/frames");
    let frames_abs_dir = project_dir.join(&frames_dir);
    validate_render_project_write_path(project_dir, &frames_abs_dir)?;
    std::fs::create_dir_all(&frames_abs_dir).map_err(|error| {
        io_pipeline_error(
            "renderedFrames.dir",
            "Create rendered frame sample directory failed.",
            error,
        )
    })?;

    let mut frame_paths = Vec::with_capacity(timeline_seconds.len());
    let mut requests = Vec::with_capacity(timeline_seconds.len());
    for (index, seconds) in timeline_seconds.iter().enumerate() {
        if !seconds.is_finite() || *seconds < 0.0 {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                "renderedFrames.timelineSeconds",
                "Rendered frame sample time must be finite and non-negative.",
                "Choose timeline sample times inside the rendered media duration.",
            )
            .with_detail("timelineSeconds", seconds.to_string())]);
        }

        let frame_path = format!("{frames_dir}/frame-{:06}.png", index + 1);
        let frame_abs_path = project_dir.join(&frame_path);
        requests.push(CompatibilityFrameRequest {
            time_seconds: *seconds,
            output_path: frame_abs_path.to_string_lossy().into_owned(),
        });
        frame_paths.push(frame_path);
    }

    run_cancellable_render_stage(cancellation, "renderedFrames.cancelled", || {
        extract_compatibility_frames_cancellable_with_publish_validator(
            &output_abs_path,
            &frames_abs_dir,
            project.render_settings.width,
            project.render_settings.height,
            requests,
            timeout,
            cancellation,
            |path| validate_rendered_frame_sample_destination(project_dir, path),
        )
        .map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "renderedFrames.compatibilityWorker",
                "The bundled compatibility worker could not extract render-review frames.",
                "Inspect the retained render log and retry the review.",
            )
            .with_detail("error", error)]
        })
    })?;

    Ok(frame_paths)
}

pub fn rendered_frame_sample_times_for_graphics_starts(
    timeline_start_seconds: &[f64],
    duration_seconds: Option<f64>,
) -> Vec<f64> {
    let max_duration_millis = duration_seconds
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .map(seconds_to_millis);
    let mut sample_millis = timeline_start_seconds
        .iter()
        .filter(|seconds| seconds.is_finite() && **seconds >= 0.0)
        .map(|seconds| seconds_to_millis(*seconds))
        .filter(|millis| {
            max_duration_millis
                .map(|duration_millis| *millis < duration_millis)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    sample_millis.sort_unstable();
    sample_millis.dedup();
    sample_millis
        .into_iter()
        .map(|millis| millis as f64 / 1000.0)
        .collect()
}

pub fn build_preview_render_comparison_request(
    project_dir: &Path,
    job_id: &str,
    render_report_path: &str,
    output_path: &str,
    duration_seconds: f64,
    frame_times_seconds: &[f64],
    rendered_frames: &[String],
) -> RenderPreviewComparisonRequest {
    let frame_time_seconds = frame_times_seconds
        .iter()
        .copied()
        .find(|seconds| seconds.is_finite() && *seconds >= 0.0)
        .unwrap_or(0.0);
    let project_dir_display = project_dir.display().to_string();

    RenderPreviewComparisonRequest {
        status: "pending".to_string(),
        project_dir: project_dir_display,
        project_report_id: job_id.to_string(),
        render_report_path: render_report_path.to_string(),
        rendered_video: output_path.to_string(),
        duration_seconds,
        frame_time_seconds,
        rendered_frames: rendered_frames.to_vec(),
        fail_on_mismatch: true,
    }
}

struct RegisteredProjectRenderAttempt {
    guard: RenderAttemptGuard,
    attempt_id: String,
    project_id: String,
}

fn register_project_render_attempt(
    project_dir: &Path,
    project_id: &str,
    job_id: &str,
    run_id: Option<String>,
) -> PipelineResult<RegisteredProjectRenderAttempt> {
    let attempt_id = run_id.unwrap_or_else(|| format!("render-attempt/{}", uuid::Uuid::new_v4()));
    let project_root = std::fs::canonicalize(project_dir).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "projectDir",
            "Split project root could not be canonicalized for render identity.",
            "Repair the project path and retry the render.",
        )
        .with_detail("error", error.to_string())]
    })?;
    let key =
        RenderAttemptKey::new(project_root, project_id, job_id, &attempt_id).map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                "render.attempt",
                "Render attempt identity is invalid.",
                "Start the render with a non-empty project, job, and attempt identity.",
            )
            .with_detail("error", error.to_string())]
        })?;
    let guard = register_render_attempt(key).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "render.attempt",
            "This exact render attempt is already active.",
            "Wait for the active attempt to finish or start a new attempt.",
        )
        .with_detail("error", error.to_string())]
    })?;
    Ok(RegisteredProjectRenderAttempt {
        guard,
        attempt_id,
        project_id: project_id.to_string(),
    })
}

pub fn render_webm_to_split_project_folder(
    project_dir: &Path,
    project_id: &str,
    profile: RenderQualityProfile,
    job: JobSummary,
    updated_at: &str,
    run_id: Option<String>,
    range_seconds: Option<(f64, f64)>,
) -> PipelineResult<ProjectWebmRenderResult> {
    let attempt = register_project_render_attempt(project_dir, project_id, &job.id, run_id)?;
    let lease = acquire_render_storage_mutation_lease()?;
    let _artifacts = acquire_render_project_artifact_lease(project_dir)?;
    render_webm_to_split_project_folder_for_timeline_with_lease(WebmRenderRequest {
        project_dir,
        profile,
        job,
        updated_at,
        attempt,
        range_seconds,
        timeline_id: None,
        lease: &lease,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "timeline render entry point exposes each render input explicitly"
)]
pub fn render_webm_to_split_project_folder_for_timeline(
    project_dir: &Path,
    project_id: &str,
    profile: RenderQualityProfile,
    job: JobSummary,
    updated_at: &str,
    run_id: Option<String>,
    range_seconds: Option<(f64, f64)>,
    timeline_id: Option<&str>,
) -> PipelineResult<ProjectWebmRenderResult> {
    let attempt = register_project_render_attempt(project_dir, project_id, &job.id, run_id)?;
    let lease = acquire_render_storage_mutation_lease()?;
    let _artifacts = acquire_render_project_artifact_lease(project_dir)?;
    render_webm_to_split_project_folder_for_timeline_with_lease(WebmRenderRequest {
        project_dir,
        profile,
        job,
        updated_at,
        attempt,
        range_seconds,
        timeline_id,
        lease: &lease,
    })
}

struct WebmRenderRequest<'a> {
    project_dir: &'a Path,
    profile: RenderQualityProfile,
    job: JobSummary,
    updated_at: &'a str,
    attempt: RegisteredProjectRenderAttempt,
    range_seconds: Option<(f64, f64)>,
    timeline_id: Option<&'a str>,
    lease: &'a StorageMutationLease,
}

fn render_webm_to_split_project_folder_for_timeline_with_lease(
    request: WebmRenderRequest<'_>,
) -> PipelineResult<ProjectWebmRenderResult> {
    let result = render_project_media_to_split_project_folder(ProjectMediaRenderRequest {
        admitted_project: None,
        admitted_identity: None,
        project_dir: request.project_dir,
        quality_profile: request.profile,
        options: None,
        output_profile: RenderOutputProfile::default(),
        paths: ProjectMediaRenderPaths::webm(&request.job.id),
        job: request.job,
        updated_at: request.updated_at,
        attempt: Some(request.attempt),
        range_seconds: request.range_seconds,
        timeline_id: request.timeline_id,
        render_label: "WebM",
        lease: request.lease,
        output: None,
        completes_job: true,
    })?;

    Ok(ProjectWebmRenderResult {
        project: result.project,
        render_report: result.render_report,
        project_render_report: result.project_render_report,
        output_path: result.output_path,
    })
}

pub fn render_media_to_split_project_folder(
    project_dir: &Path,
    project_id: &str,
    options: ExportRenderOptions,
    job: JobSummary,
    updated_at: &str,
    run_id: Option<String>,
    range_seconds: Option<(f64, f64)>,
) -> PipelineResult<ProjectMediaRenderResult> {
    render_media_step_to_split_project_folder(MediaRenderStep {
        project_dir,
        project_id,
        options,
        job,
        updated_at,
        run_id,
        range_seconds,
        completes_job: true,
    })
}

/// Renders an export workflow's media without completing its job: the workflow's later
/// `WriteExportArtifact` step completes the job once the export file exists.
pub fn render_export_workflow_media_to_split_project_folder(
    project_dir: &Path,
    project_id: &str,
    options: ExportRenderOptions,
    job: JobSummary,
    updated_at: &str,
    run_id: Option<String>,
) -> PipelineResult<ProjectMediaRenderResult> {
    render_media_step_to_split_project_folder(MediaRenderStep {
        project_dir,
        project_id,
        options,
        job,
        updated_at,
        run_id,
        range_seconds: None,
        completes_job: false,
    })
}

/// One render of a project's media: the whole export, or an export workflow's render step.
struct MediaRenderStep<'a> {
    project_dir: &'a Path,
    project_id: &'a str,
    options: ExportRenderOptions,
    job: JobSummary,
    updated_at: &'a str,
    run_id: Option<String>,
    range_seconds: Option<(f64, f64)>,
    /// False when a later workflow step completes the job.
    completes_job: bool,
}

fn render_media_step_to_split_project_folder(
    step: MediaRenderStep<'_>,
) -> PipelineResult<ProjectMediaRenderResult> {
    let MediaRenderStep {
        project_dir,
        project_id,
        options,
        job,
        updated_at,
        run_id,
        range_seconds,
        completes_job,
    } = step;
    let attempt = register_project_render_attempt(project_dir, project_id, &job.id, run_id)?;
    let lease = acquire_render_storage_mutation_lease()?;
    let _artifacts = acquire_render_project_artifact_lease(project_dir)?;
    render_media_to_split_project_folder_for_timeline_with_lease(MediaRenderRequest {
        project_dir,
        options,
        job,
        updated_at,
        attempt,
        range_seconds,
        timeline_id: None,
        lease: &lease,
        output: None,
        completes_job,
    })
}

/// Captures the exact split project's prepared compositor output at one playhead.
///
/// macOS: one-frame native render; other platforms: the Rust canonical frame sampler.
/// On macOS the capture uses a one-frame ProRes render through the same project
/// preparation, visual-layer, and AVFoundation path as final export. Elsewhere it samples
/// the prepared project with `precompose::render_canonical_frame_rgba` (see
/// `canonical_capture`). Either way the PreviewPanel gets a native frame artifact without
/// depending on a localhost browser or an injected Tauri bridge.
pub fn render_prepared_preview_frame_to_split_project_folder(
    project_dir: &Path,
    playhead_seconds: f64,
    job_id: &str,
    updated_at: &str,
) -> PipelineResult<PreparedPreviewFrameResult> {
    let lease = acquire_render_storage_mutation_lease()?;
    render_prepared_preview_frame_to_split_project_folder_with_lease(
        project_dir,
        playhead_seconds,
        job_id,
        updated_at,
        &lease,
    )
}

/// [`render_prepared_preview_frame_to_split_project_folder`] for callers that already hold
/// the storage lease. macOS: one-frame native render; other platforms: the Rust canonical
/// frame sampler.
pub fn render_prepared_preview_frame_to_split_project_folder_with_lease(
    project_dir: &Path,
    playhead_seconds: f64,
    job_id: &str,
    updated_at: &str,
    lease: &StorageMutationLease,
) -> PipelineResult<PreparedPreviewFrameResult> {
    let _artifacts = acquire_render_project_artifact_lease(project_dir)?;
    if !playhead_seconds.is_finite() || playhead_seconds < 0.0 {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "playheadSeconds",
            "Canonical preview playhead must be finite and non-negative.",
            "Choose a playhead inside the split project's primary timeline.",
        )]);
    }
    let source_project = load_split_project(project_dir).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "projectDir",
            "Split project could not be loaded for canonical preview capture.",
            "Save the project as a split project folder and retry the capture.",
        )
        .with_detail("error", error.to_string())]
    })?;
    let duration_seconds = source_project.timeline.duration_seconds;
    if playhead_seconds >= duration_seconds {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "playheadSeconds",
            "Canonical preview playhead must be before the end of the timeline.",
            "Choose a playhead inside the split project's primary timeline.",
        )
        .with_detail("durationSeconds", duration_seconds.to_string())]);
    }
    let fps = source_project.render_settings.fps;
    if !fps.is_finite() || fps <= 0.0 {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "renderSettings.fps",
            "Canonical preview capture requires a positive frame rate.",
            "Set a valid project frame rate and retry the capture.",
        )]);
    }
    let frame_end_seconds = (playhead_seconds + 1.0 / fps).min(duration_seconds);
    // The render attempt id is recorded as the job's workflow run id, which needs workflow
    // metadata on the job.
    let mut job = crate::workflows::temporal_job_summary(
        crate::workflows::TemporalWorkflowKind::RenderDraft,
        &source_project.id,
        job_id,
        JobStatus::Queued,
        updated_at,
    );
    job.kind = "captureCanonicalPreviewFrame".to_string();
    capture_prepared_preview_frame(
        project_dir,
        &source_project,
        playhead_seconds,
        frame_end_seconds,
        job,
        updated_at,
        lease,
    )
}

#[cfg(not(target_os = "macos"))]
use canonical_capture::capture_canonical_frame_with_sampler as capture_prepared_preview_frame;

/// macOS canonical capture: a one-frame native ProRes render, then frame extraction.
#[cfg(target_os = "macos")]
fn capture_prepared_preview_frame(
    project_dir: &Path,
    source_project: &VideoProject,
    playhead_seconds: f64,
    frame_end_seconds: f64,
    job: JobSummary,
    updated_at: &str,
    lease: &StorageMutationLease,
) -> PipelineResult<PreparedPreviewFrameResult> {
    let job_id = job.id.clone();
    let job_id = job_id.as_str();
    let attempt = register_project_render_attempt(project_dir, &source_project.id, job_id, None)?;
    let result =
        render_media_to_split_project_folder_for_timeline_with_lease(MediaRenderRequest {
            project_dir,
            options: ExportRenderOptions {
                profile: ExportProfile::ProResMov,
                quality: RenderQuality::Final,
                width: source_project.render_settings.width,
                height: source_project.render_settings.height,
                fps: None,
                encode_tier: crate::edit::render_plan::ExportEncodeTier::Standard,
            },
            job,
            updated_at,
            attempt,
            range_seconds: Some((playhead_seconds, frame_end_seconds)),
            timeline_id: None,
            lease,
            output: None,
            completes_job: true,
        })?;
    if result.render_report.command.program != "avfoundation-native" {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "canonicalPreview.backend",
            "Canonical preview capture did not use the native AVFoundation compositor.",
            "Install the native exporter and retry; canonical preview evidence may not silently fall back.",
        )
        .with_detail("program", result.render_report.command.program.clone())]);
    }
    let render_dir = format!("renders/{}", safe_path_segment(job_id));
    let extracted = extract_rendered_frame_samples_with_lease(
        &SystemProcessRunner,
        project_dir,
        &render_dir,
        &result.output_path,
        &[0.0],
        Duration::from_secs(60),
        None,
        lease,
    )?;
    let extracted_frame = extracted.first().ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "canonicalPreview.frame",
            "Canonical preview render did not produce an extracted frame.",
            "Inspect the retained native render log and retry the capture.",
        )]
    })?;
    let preview_frame = format!("{render_dir}/preview-qa/preview-frames/preview-0001.png");
    let preview_frame_path = project_dir.join(&preview_frame);
    validate_render_project_write_path(project_dir, &preview_frame_path)?;
    std::fs::create_dir_all(preview_frame_path.parent().unwrap_or(project_dir)).map_err(
        |error| {
            io_pipeline_error(
                "canonicalPreview.dir",
                "Create preview evidence directory failed.",
                error,
            )
        },
    )?;
    std::fs::copy(project_dir.join(extracted_frame), &preview_frame_path).map_err(|error| {
        io_pipeline_error(
            "canonicalPreview.frame",
            "Retain canonical preview frame failed.",
            error,
        )
    })?;
    let evidence_report = format!("{render_dir}/preview-qa/canonical-preview-frame.json");
    validate_render_project_write_path(project_dir, &project_dir.join(&evidence_report))?;
    let evidence = json!({
        "schemaVersion": 1,
        "status": "captured",
        "source": "exact-split-project",
        "backend": "avfoundation-native",
        "playheadSeconds": playhead_seconds,
        "frameDurationSeconds": frame_end_seconds - playhead_seconds,
        "previewFrame": preview_frame,
        "sourceOutput": result.output_path,
        "renderReport": format!("{render_dir}/report.json"),
        "policy": { "failOnMismatch": true }
    });
    std::fs::write(
        project_dir.join(&evidence_report),
        serde_json::to_vec_pretty(&evidence).map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "canonicalPreview.report",
                format!("Canonical preview evidence could not be encoded: {error}"),
                "Inspect the capture inputs and retry.",
            )]
        })?,
    )
    .map_err(|error| {
        io_pipeline_error(
            "canonicalPreview.report",
            "Write canonical preview evidence failed.",
            error,
        )
    })?;

    Ok(PreparedPreviewFrameResult {
        project: result.project,
        playhead_seconds,
        preview_frame,
        source_output: result.output_path,
        evidence_report,
        render_report: result.render_report,
        project_render_report: result.project_render_report,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "timeline render entry point exposes each render input explicitly"
)]
pub fn render_media_to_split_project_folder_for_timeline(
    project_dir: &Path,
    project_id: &str,
    options: ExportRenderOptions,
    job: JobSummary,
    updated_at: &str,
    run_id: Option<String>,
    range_seconds: Option<(f64, f64)>,
    timeline_id: Option<&str>,
) -> PipelineResult<ProjectMediaRenderResult> {
    render_media_export_to_split_project_folder(MediaExportRequest {
        project_dir,
        project_id,
        options,
        job,
        updated_at,
        run_id,
        range_seconds,
        timeline_id,
        output: None,
    })
}

struct MediaRenderRequest<'a> {
    project_dir: &'a Path,
    options: ExportRenderOptions,
    job: JobSummary,
    updated_at: &'a str,
    attempt: RegisteredProjectRenderAttempt,
    range_seconds: Option<(f64, f64)>,
    timeline_id: Option<&'a str>,
    lease: &'a StorageMutationLease,
    output: Option<ExportOutputRequest>,
    /// False when a later workflow step completes the job.
    completes_job: bool,
}

fn render_media_to_split_project_folder_for_timeline_with_lease(
    request: MediaRenderRequest<'_>,
) -> PipelineResult<ProjectMediaRenderResult> {
    let output_profile = render_output_profile_for_export(request.options.profile)?;
    let paths = ProjectMediaRenderPaths::new(&request.job.id, request.options.profile)?;
    let quality_profile = match request.options.quality {
        RenderQuality::Draft => RenderQualityProfile::DraftWebm,
        RenderQuality::Final => RenderQualityProfile::FinalWebm,
    };

    render_project_media_to_split_project_folder(ProjectMediaRenderRequest {
        admitted_project: None,
        admitted_identity: None,
        project_dir: request.project_dir,
        quality_profile,
        options: Some(request.options),
        output_profile,
        paths,
        job: request.job,
        updated_at: request.updated_at,
        attempt: Some(request.attempt),
        range_seconds: request.range_seconds,
        timeline_id: request.timeline_id,
        render_label: "media",
        lease: request.lease,
        output: request.output,
        completes_job: request.completes_job,
    })
}

fn persist_media_render_actions(
    project_dir: &Path,
    actions: Vec<ProjectAction>,
    lease: &SplitProjectMutationLease,
    preserve_revision: bool,
) -> Result<ProjectActionWriteResult, crate::project::split::SplitProjectError> {
    if preserve_revision {
        crate::project::split::apply_project_bookkeeping_actions_to_split_project_with_lease(
            project_dir,
            actions,
            lease,
        )
    } else {
        apply_project_actions_to_split_project(project_dir, actions)
    }
}

struct ProjectMediaRenderRequest<'a> {
    /// An immutable input captured by durable admission. Legacy callers capture it at start.
    admitted_project: Option<VideoProject>,
    admitted_identity: Option<admission::PackageIdentity>,
    project_dir: &'a Path,
    quality_profile: RenderQualityProfile,
    options: Option<ExportRenderOptions>,
    output_profile: RenderOutputProfile,
    paths: ProjectMediaRenderPaths,
    job: JobSummary,
    updated_at: &'a str,
    attempt: Option<RegisteredProjectRenderAttempt>,
    range_seconds: Option<(f64, f64)>,
    timeline_id: Option<&'a str>,
    render_label: &'static str,
    lease: &'a StorageMutationLease,
    output: Option<ExportOutputRequest>,
    completes_job: bool,
}

fn render_project_media_to_split_project_folder(
    request: ProjectMediaRenderRequest<'_>,
) -> PipelineResult<ProjectMediaRenderResult> {
    let ProjectMediaRenderRequest {
        admitted_project,
        admitted_identity,
        project_dir,
        quality_profile,
        options,
        output_profile,
        paths,
        job,
        updated_at,
        attempt,
        range_seconds,
        timeline_id,
        render_label,
        lease,
        output,
        completes_job,
    } = request;
    // The project lease covers the start phase only: reading a consistent project, resolving the
    // render attempt and recording the job as running. Encoding then runs with no project lease
    // held, so an edit queued behind this render lands while it encodes. The render keeps working
    // from `project`, the snapshot taken here, so a mid-render edit cannot change its output, and
    // the result writes below take the lease again for as long as each write needs it.
    let was_admitted = admitted_project.is_some();
    // Canonical result frames are internal evidence, including the macOS native
    // one-frame render. Their metadata must not invalidate the originating edit.
    let preserve_revision = was_admitted || job.kind == "captureCanonicalPreviewFrame";
    let (project, attempt, job_id, attempt_id) = {
        let _project_lease = acquire_render_project_mutation_lease(project_dir)?;
        if let Some(identity) = admitted_identity.as_ref() {
            let attempt_id = attempt
                .as_ref()
                .map(|attempt| attempt.attempt_id.as_str())
                .ok_or_else(|| render_cancelled_error("render.admission.attempt"))?;
            identity
                .active_job(project_dir, &job.id, attempt_id)
                .map_err(|_| render_cancelled_error("render.admission.identity"))?;
        }
        let project = admitted_project
            .map(Ok)
            .unwrap_or_else(|| load_split_project(project_dir))
            .map_err(|error| {
                vec![PipelineError::new(
                    PipelineErrorCode::PipelineInputInvalid,
                    "projectDir",
                    format!("Split project could not be loaded for {render_label} render."),
                    "Save the project as a split project folder and retry the render.",
                )
                .with_detail("projectDir", project_dir.display().to_string())
                .with_detail("error", error.to_string())]
            })?;
        let attempt = match attempt {
            Some(attempt) => {
                if project.id != attempt.project_id {
                    return Err(vec![PipelineError::new(
                        PipelineErrorCode::PipelineInputInvalid,
                        "render.attempt.projectId",
                        "Render attempt project identity does not match the canonical project.",
                        "Reload the project and start a new render attempt.",
                    )]);
                }
                attempt
            }
            None => register_project_render_attempt(project_dir, &project.id, &job.id, None)?,
        };
        let cancellation = attempt.guard.token();
        if cancellation.is_cancelled() {
            let _ = cancellation.begin_cancelled_completion();
            let attempt_id = attempt.attempt_id.clone();
            let job_id = job.id.clone();
            let mut actions = if was_admitted {
                Vec::new()
            } else {
                render_job_start_actions(&project, job, updated_at, &attempt_id)
            };
            actions.push(ProjectAction::UpdateJobStatus {
                job_id,
                status: JobStatus::Cancelled,
                updated_at: updated_at.to_string(),
                run_id: Some(attempt_id),
            });
            let terminal_write = persist_media_render_actions(
                project_dir,
                actions,
                &_project_lease,
                preserve_revision,
            );
            cancellation.mark_terminal();
            terminal_write.map_err(|error| {
                render_project_action_error(
                    render_label,
                    "Persisting cancellation before render start failed.",
                    error,
                )
            })?;
            return Err(render_cancelled_error("render.start.cancelled"));
        }
        let project = match timeline_id {
            Some(timeline_id) => project.projected_for_timeline(timeline_id).ok_or_else(|| {
                vec![PipelineError::new(
                    PipelineErrorCode::PipelineInputInvalid,
                    "timelineId",
                    format!("Requested timeline `{timeline_id}` was not found."),
                    "Choose an existing project timeline before rendering.",
                )]
            })?,
            None => project,
        };
        let absolute_render_dir = paths.absolute_render_dir(project_dir);
        validate_render_project_write_path(project_dir, &absolute_render_dir)?;
        for artifact_path in [
            paths.absolute_output_path(project_dir),
            paths.absolute_json_report_path(project_dir),
            paths.absolute_markdown_report_path(project_dir),
            paths.absolute_log_path(project_dir),
        ] {
            validate_render_project_write_path(project_dir, &artifact_path)?;
        }
        std::fs::create_dir_all(&absolute_render_dir).map_err(|error| {
            io_pipeline_error("renders.dir", "Create render directory failed.", error)
        })?;
        let job_id = job.id.clone();
        let attempt_id = attempt.attempt_id.clone();
        persist_media_render_actions(
            project_dir,
            if was_admitted {
                vec![ProjectAction::UpdateJobStatus {
                    job_id: job.id.clone(),
                    status: JobStatus::Running,
                    updated_at: updated_at.to_string(),
                    run_id: Some(attempt_id.clone()),
                }]
            } else {
                render_job_start_actions(&project, job.clone(), updated_at, &attempt_id)
            },
            &_project_lease,
            preserve_revision,
        )
        .map_err(|error| {
            render_project_action_error(render_label, "Persisting render job start failed.", error)
        })?;
        (project, attempt, job_id, attempt_id)
    };
    let cancellation = attempt.guard.token();

    let mut result = render_project_media_after_job_started(StartedProjectMediaRender {
        admitted_identity: admitted_identity.as_ref(),
        project_dir,
        project: &project,
        quality_profile,
        options,
        output_profile,
        paths: paths.clone(),
        job,
        updated_at,
        run_id: Some(attempt_id.clone()),
        range_seconds,
        render_label,
        cancellation: &cancellation,
        lease,
        output,
        completes_job,
    });

    if let Err(errors) = &mut result {
        // Revalidate before cleanup, terminal writes or diagnostic artifacts. An old worker
        // cannot alter a replacement package, a newer attempt or a canonical terminal job.
        let _project_lease = acquire_render_project_mutation_lease(project_dir)?;
        if let Some(identity) = admitted_identity.as_ref() {
            if identity
                .active_job(project_dir, &job_id, &attempt_id)
                .is_err()
            {
                cancellation.mark_terminal();
                return result;
            }
        }
        let mut terminal_status = claim_error_terminal(&cancellation);
        let cancelled = terminal_status == JobStatus::Cancelled;
        let mut cleanup_failed = false;
        if cancelled {
            if let Err(mut cleanup_errors) =
                clean_cancelled_render_attempt(project_dir, &paths, &job_id, &attempt_id, errors)
            {
                cleanup_failed = true;
                terminal_status = JobStatus::Failed;
                errors.append(&mut cleanup_errors);
            }
        }
        let terminal_write = persist_media_render_actions(
            project_dir,
            vec![ProjectAction::UpdateJobStatus {
                job_id: job_id.clone(),
                status: terminal_status.clone(),
                updated_at: updated_at.to_string(),
                run_id: Some(attempt_id),
            }],
            &_project_lease,
            preserve_revision,
        );
        if terminal_status == JobStatus::Failed && !cleanup_failed && terminal_write.is_ok() {
            let _ = write_failed_project_media_render_report(
                project_dir,
                &paths,
                &job_id,
                output_profile,
                errors,
            );
        }
    }
    cancellation.mark_terminal();

    result
}

fn claim_error_terminal(cancellation: &RenderCancellationToken) -> JobStatus {
    if cancellation.begin_cancelled_completion() {
        JobStatus::Cancelled
    } else if cancellation.begin_completion() {
        JobStatus::Failed
    } else if cancellation.is_cancelled() {
        JobStatus::Cancelled
    } else {
        JobStatus::Failed
    }
}

fn clean_cancelled_render_attempt(
    project_dir: &Path,
    paths: &ProjectMediaRenderPaths,
    job_id: &str,
    attempt_id: &str,
    errors: &[PipelineError],
) -> PipelineResult<()> {
    let render_dir = paths.absolute_render_dir(project_dir);
    let diagnostics_dir = project_dir
        .join("renders")
        .join(".cancelled")
        .join(safe_path_segment(job_id))
        .join(safe_path_segment(attempt_id));
    validate_render_project_write_path(project_dir, &diagnostics_dir).map_err(|errors| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "renders.cancelled.diagnostics",
            "Cancelled render diagnostics path is not writable.",
            "Remove the path obstruction and retry cleanup before treating the attempt as cancelled.",
        )
        .with_detail(
            "cause",
            errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        )]
    })?;
    std::fs::create_dir_all(&diagnostics_dir).map_err(|error| {
        io_pipeline_error(
            "renders.cancelled.diagnostics",
            "Create cancelled render diagnostics directory failed.",
            error,
        )
    })?;
    if render_dir.exists() {
        let quarantined_artifacts = diagnostics_dir.join("artifacts");
        if quarantined_artifacts.exists() {
            std::fs::remove_dir_all(&quarantined_artifacts).map_err(|error| {
                io_pipeline_error(
                    "renders.cancelled.cleanup",
                    "Clear prior cancelled render quarantine failed.",
                    error,
                )
            })?;
        }
        std::fs::rename(&render_dir, &quarantined_artifacts).map_err(|error| {
            io_pipeline_error(
                "renders.cancelled.cleanup",
                "Quarantine cancelled render attempt artifacts failed.",
                error,
            )
        })?;
    }
    let diagnostic = errors
        .iter()
        .map(|error| format!("[{:?}] {}: {}", error.code, error.path, error.message))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(diagnostics_dir.join("render.log"), diagnostic).map_err(|error| {
        io_pipeline_error(
            "renders.cancelled.diagnostics",
            "Write cancelled render diagnostics failed.",
            error,
        )
    })?;
    Ok(())
}

fn write_failed_project_media_render_report(
    project_dir: &Path,
    paths: &ProjectMediaRenderPaths,
    job_id: &str,
    output_profile: RenderOutputProfile,
    errors: &[PipelineError],
) -> PipelineResult<()> {
    let json_report_path = format!("{}/pipeline-report.json", paths.render_dir);
    let markdown_report_path = format!("{}/pipeline-report.md", paths.render_dir);
    let artifacts = vec![
        json_report_path.clone(),
        markdown_report_path.clone(),
        paths.log_path.clone(),
    ];
    let report = RenderReport {
        job_id: job_id.to_string(),
        summary: RenderReportSummary {
            status: "failed".to_string(),
            duration_seconds: None,
            output_path: Some(paths.output_path.clone()),
            ..RenderReportSummary::default()
        },
        command: CommandSpec::new("gstreamer-ges")
            .arg(format!("--output={}", paths.output_path))
            .arg(format!("--profile={output_profile:?}")),
        stdout: String::new(),
        stderr: String::new(),
        streams: None,
        errors: errors.to_vec(),
        artifacts,
        graphics: Vec::new(),
        performance: None,
        preview_comparison_request: None,
        preview_comparison: None,
    };
    let log = errors
        .iter()
        .map(|error| {
            format!(
                "[{:?}] {}: {}\nfix: {}",
                error.code, error.path, error.message, error.fix
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    std::fs::write(paths.absolute_log_path(project_dir), log).map_err(|error| {
        io_pipeline_error("renders.log", "Write failed render log failed.", error)
    })?;
    write_json_report(&project_dir.join(json_report_path), &report)?;
    write_markdown_report(&project_dir.join(markdown_report_path), &report)?;

    Ok(())
}

struct StartedProjectMediaRender<'a> {
    admitted_identity: Option<&'a admission::PackageIdentity>,
    project_dir: &'a Path,
    project: &'a VideoProject,
    quality_profile: RenderQualityProfile,
    options: Option<ExportRenderOptions>,
    output_profile: RenderOutputProfile,
    paths: ProjectMediaRenderPaths,
    job: JobSummary,
    updated_at: &'a str,
    run_id: Option<String>,
    range_seconds: Option<(f64, f64)>,
    render_label: &'static str,
    cancellation: &'a RenderCancellationToken,
    lease: &'a StorageMutationLease,
    output: Option<ExportOutputRequest>,
    completes_job: bool,
}

fn render_project_media_after_job_started(
    render: StartedProjectMediaRender<'_>,
) -> PipelineResult<ProjectMediaRenderResult> {
    let StartedProjectMediaRender {
        admitted_identity,
        project_dir,
        project,
        quality_profile,
        options,
        output_profile,
        paths,
        job,
        updated_at,
        run_id,
        range_seconds,
        render_label,
        cancellation,
        lease,
        output,
        completes_job,
    } = render;
    validate_render_range(range_seconds)?;
    // A frame-rate override renders from a copy of the project at that rate, so
    // the storage preflight, preparation, graphics layers and plan all agree.
    let frame_rate_project;
    let project = match options.and_then(|options| options.fps) {
        Some(fps) => {
            let mut overridden = project.clone();
            overridden.render_settings.fps = fps;
            frame_rate_project = overridden;
            &frame_rate_project
        }
        None => project,
    };
    let render_duration_seconds = range_seconds
        .map(|(start, end)| end - start)
        .unwrap_or(project.timeline.duration_seconds);
    let requested_width = options
        .as_ref()
        .map(|options| options.width)
        .unwrap_or(project.render_settings.width);
    let requested_height = options
        .as_ref()
        .map(|options| options.height)
        .unwrap_or(project.render_settings.height);
    preflight_render_storage(
        project_dir,
        requested_width,
        requested_height,
        project.render_settings.fps,
        render_duration_seconds,
        output_profile,
    )?;
    let expanded_project = expand_project_nested_timelines_for_render(project)?;
    let mut prepared_render =
        prepare_project_for_render_cancellable(project_dir, &expanded_project, Some(cancellation))?;
    attach_prepared_frame_sequences(project_dir, &mut prepared_render)?;
    let project = &prepared_render.project;
    let export_profile = export_profile_for_output_profile(output_profile);
    let quality = crate::edit::render_plan::render_quality_from_legacy(quality_profile);
    let options = match options {
        Some(options) => options,
        None => ExportRenderOptions::for_project_defaults(
            export_profile,
            quality,
            project.render_settings.width,
            project.render_settings.height,
        )
        .map_err(export_options_error)?,
    };
    let plan = build_project_render_plan_with_range(
        project_dir,
        project,
        paths.absolute_output_path(project_dir),
        options,
        output_profile,
        range_seconds,
    )?;

    let graphics_layers = build_project_graphics_render_layers(
        project,
        plan.width,
        plan.height,
        plan.fps,
        range_seconds,
    )?;
    let rendered_graphics = render_project_graphics_layers(
        &graphics_layers,
        project_dir,
        paths.absolute_render_dir(project_dir).join("graphics"),
        Some(cancellation),
    )?;
    ensure_render_not_cancelled(Some(cancellation), "graphics.cancelled")?;
    let graphics_inputs = project_graphics_backend_inputs(&rendered_graphics);
    let output_path = paths.absolute_output_path(project_dir);
    let avfoundation = AvFoundationRenderBackend::new();
    let avfoundation_capabilities = avfoundation.capabilities().ok();
    let avfoundation_profile = select_project_export_avfoundation_profile(
        export_profile,
        quality,
        avfoundation_capabilities.as_ref(),
    )?;
    let use_avfoundation = select_render_backend(
        output_profile,
        avfoundation_profile,
        &avfoundation,
        &job.id,
        &plan,
        &graphics_inputs,
    ) == ProjectRenderBackend::AvFoundation;
    let (command, render_output, probe, probe_output, render_backend_label, probe_label) =
        if use_avfoundation {
            let command = avfoundation.build_command(&plan, &graphics_inputs)?;
            let run = avfoundation.render_cancellable(
                &job.id,
                &plan,
                &graphics_inputs,
                PROJECT_RENDER_TIMEOUT,
                Some(cancellation),
            )?;
            let probe_output = ProcessOutput {
                status_code: Some(0),
                stdout: serde_json::to_string_pretty(&run.probe).unwrap_or_default(),
                stderr: String::new(),
            };
            (
                command,
                run.process_output,
                run.probe,
                probe_output,
                "render avfoundation native",
                "final avfoundation probe",
            )
        } else {
            let backend = GstreamerGesRenderBackend::new();
            let command = backend.build_command(&plan, &graphics_inputs)?;
            let render_output = backend.render_cancellable(
                &SystemProcessRunner,
                &plan,
                &graphics_inputs,
                PROJECT_RENDER_TIMEOUT,
                Some(cancellation),
            )?;
            ensure_render_not_cancelled(Some(cancellation), "render.probe.cancelled")?;
            let (probe, probe_output) =
                probe_media_with_gstreamer(&output_path, PROJECT_RENDER_TIMEOUT, "projectMedia")?;
            ensure_render_not_cancelled(Some(cancellation), "render.probe.cancelled")?;
            let probe_label = "final gstreamer discoverer";
            (
                command,
                render_output,
                probe,
                probe_output,
                "render gstreamer ges",
                probe_label,
            )
        };
    if use_avfoundation && output_path != plan.output_path {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "avfoundation.outputPath",
            "AVFoundation output escaped the Rust-owned render artifact path.",
            "Keep the native exporter output path equal to the canonical render plan output.",
        )]);
    }
    if !output_path.is_file() {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "render.outputPath",
            "Render backend completed without writing the expected output artifact.",
            "Inspect the backend protocol and render log, then retry.",
        )
        .with_detail("path", output_path.display().to_string())]);
    }
    let audio_required = timeline_has_audio(project);
    let expected_media = expected_media_for_render_plan(&plan, audio_required);
    validate_rendered_media(&output_path, &probe, &expected_media)?;
    ensure_render_not_cancelled(Some(cancellation), "render.validation.cancelled")?;
    let log = labeled_output(&[
        (
            render_backend_label,
            &render_output.stdout,
            &render_output.stderr,
        ),
        (probe_label, &probe_output.stdout, &probe_output.stderr),
    ]);
    std::fs::write(paths.absolute_log_path(project_dir), log)
        .map_err(|error| io_pipeline_error("renders.log", "Write render log failed.", error))?;
    let mut artifacts = vec![
        paths.output_path.clone(),
        paths.json_report_path.clone(),
        paths.markdown_report_path.clone(),
        paths.log_path.clone(),
    ];
    artifacts.extend(project_graphics_artifact_paths(
        &rendered_graphics,
        project_dir,
    ));
    for precompose in &prepared_render.reports {
        append_unique_artifact(&mut artifacts, &precompose.worker_manifest);
        append_unique_artifact(&mut artifacts, &precompose.intermediate);
    }
    let mut rendered_frame_sample_times = rendered_frame_sample_times_for_graphics_starts(
        &rendered_graphics
            .iter()
            .map(|graphics| graphics.timeline_start_seconds)
            .collect::<Vec<_>>(),
        probe.duration_seconds,
    );
    if rendered_frame_sample_times.is_empty() {
        let duration = probe.duration_seconds.unwrap_or_default();
        rendered_frame_sample_times.push(if duration > 0.1 {
            (duration / 2.0).min(duration - 0.1)
        } else {
            0.0
        });
    }
    let rendered_frame_samples = extract_rendered_frame_samples_with_lease(
        &SystemProcessRunner,
        project_dir,
        &paths.render_dir,
        &paths.output_path,
        &rendered_frame_sample_times,
        PROJECT_RENDER_TIMEOUT,
        Some(cancellation),
        lease,
    )?;
    // Copying to another filesystem can take minutes. Keep storage/artifact ownership,
    // but stage bytes while edits and cancellation are still allowed.
    let prepared_export = output
        .as_ref()
        .map(|output| {
            named_export::prepare_rendered_export(
                project_dir,
                options,
                output,
                &paths.absolute_output_path(project_dir),
                &job.id,
                updated_at,
                cancellation,
            )
        })
        .transpose()?;
    if !cancellation.begin_completion() {
        return Err(render_cancelled_error("render.completion"));
    }
    let rendered_frame_samples_by_time = rendered_frame_sample_times
        .iter()
        .zip(rendered_frame_samples.iter())
        .map(|(seconds, path)| (seconds_to_millis(*seconds), path.clone()))
        .collect::<BTreeMap<_, _>>();
    let preview_comparison_duration_seconds = probe
        .duration_seconds
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .unwrap_or(1.0);
    let preview_comparison_request = Some(build_preview_render_comparison_request(
        project_dir,
        &job.id,
        &paths.json_report_path,
        &paths.output_path,
        preview_comparison_duration_seconds,
        &rendered_frame_sample_times,
        &rendered_frame_samples,
    ));
    artifacts.extend(rendered_frame_samples);
    let render_report = RenderReport {
        job_id: job.id.clone(),
        summary: RenderReportSummary {
            status: "succeeded".to_string(),
            duration_seconds: probe.duration_seconds,
            output_path: Some(paths.output_path.clone()),
            quality: Some(plan.quality),
            requested_width: Some(plan.width),
            requested_height: Some(plan.height),
            actual_width: probe.video.as_ref().and_then(|video| video.width),
            actual_height: probe.video.as_ref().and_then(|video| video.height),
            container: expected_media.container.clone(),
            video_codec: rendered_video_codec(&probe, &expected_media),
            audio_codec: rendered_audio_codec(&probe, &expected_media),
        },
        command,
        stdout: render_output.stdout,
        stderr: render_output.stderr,
        streams: Some(PipelineRenderReportStreams {
            video: probe.video.is_some(),
            audio: probe.audio.is_some(),
        }),
        errors: Vec::new(),
        artifacts,
        graphics: rendered_graphics
            .iter()
            .map(|graphics| {
                let rendered_frame = rendered_frame_samples_by_time
                    .get(&seconds_to_millis(graphics.timeline_start_seconds))
                    .cloned();
                render_project_graphics_report(graphics, rendered_frame)
            })
            .collect(),
        performance: None,
        preview_comparison_request,
        preview_comparison: None,
    };
    // Completion merges into the latest project, while retaining the admitted input.
    // Hold mutation ownership across identity/attempt validation and report publication.
    let _completion_lease = acquire_render_project_mutation_lease(project_dir)?;
    if let Some(identity) = admitted_identity {
        let attempt_id = run_id
            .as_deref()
            .ok_or_else(|| render_cancelled_error("render.completion.attempt"))?;
        identity
            .active_job(project_dir, &job.id, attempt_id)
            .map_err(|_| render_cancelled_error("render.completion.identity"))?;
    }
    write_json_report(
        &paths.absolute_json_report_path(project_dir),
        &render_report,
    )?;
    write_markdown_report(
        &paths.absolute_markdown_report_path(project_dir),
        &render_report,
    )?;
    let project_render_report = project_media_render_report_from_pipeline_report(
        &render_report,
        &paths,
        updated_at,
        &ProjectRenderReportEvidence {
            video_stream: probe.video.is_some(),
            audio_stream: probe.audio.is_some(),
            audio_required,
            expected_duration_seconds: Some(render_plan_duration_seconds(&plan)),
            duration_tolerance_seconds: Some(render_duration_tolerance_seconds(plan.fps)),
        },
    )?;
    let saved_export = prepared_export
        .map(|prepared| prepared.publish())
        .transpose()?;
    let mut completion_actions = Vec::with_capacity(3);
    if completes_job {
        completion_actions.push(ProjectAction::UpdateJobStatus {
            job_id: job.id.clone(),
            status: JobStatus::Completed,
            updated_at: updated_at.to_string(),
            run_id,
        });
    }
    completion_actions.push(ProjectAction::AttachRenderReport {
        report: project_render_report.clone(),
    });
    if let Some(saved) = &saved_export {
        completion_actions.push(ProjectAction::RecordExportArtifact {
            artifact: saved.artifact.clone(),
        });
    }
    let write = persist_media_render_actions(
        project_dir,
        completion_actions,
        &_completion_lease,
        admitted_identity.is_some() || job.kind == "captureCanonicalPreviewFrame",
    )
    .map_err(|error| {
        if let Some(saved) = &saved_export {
            saved.rollback();
        }
        render_project_action_error(
            render_label,
            "Persisting render completion actions failed.",
            error,
        )
    })?;

    Ok(media_result_from_write(
        write,
        render_report,
        project_render_report,
        paths.output_path,
        saved_export.map(|saved| saved.artifact),
    ))
}

/// The backend that renders a project media export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectRenderBackend {
    AvFoundation,
    GstreamerGes,
}

/// Chooses the native AVFoundation exporter for delivery profiles it supports
/// (`avfoundation_profile` is `Some`) when it accepts the plan. WebM exports
/// and plans AVFoundation rejects, such as plans with clip transitions, render
/// with GStreamer/GES.
pub fn select_render_backend(
    output_profile: RenderOutputProfile,
    avfoundation_profile: Option<AvFoundationExportProfile>,
    avfoundation: &AvFoundationRenderBackend,
    request_id: &str,
    plan: &RenderPlan,
    graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
) -> ProjectRenderBackend {
    let native = !matches!(
        output_profile,
        RenderOutputProfile::WebPreview | RenderOutputProfile::WebDelivery
    ) && avfoundation_profile.is_some()
        && avfoundation
            .build_request(request_id, plan, graphics)
            .is_ok();
    if native {
        ProjectRenderBackend::AvFoundation
    } else {
        ProjectRenderBackend::GstreamerGes
    }
}

fn select_project_export_avfoundation_profile(
    profile: ExportProfile,
    quality: RenderQuality,
    capabilities: Option<&AvFoundationCapabilities>,
) -> PipelineResult<Option<AvFoundationExportProfile>> {
    if matches!(profile, ExportProfile::Webm | ExportProfile::PalmierProject) {
        return Ok(None);
    }

    let native_profile = avfoundation_profile(profile, quality);
    let supported =
        capabilities.is_some_and(|report| report.supports_profile_quality(native_profile, quality));
    if native_profile == AvFoundationExportProfile::ProResProxy && !supported {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderBackendUnavailable,
            "renderPlan.quality",
            "Draft ProRes requires native ProRes Proxy capability, but this exporter does not report it.",
            "Choose Final ProRes quality or rebuild the native exporter with ProRes Proxy support.",
        )
        .with_detail("profile", "proResProxy")
        .with_detail("quality", "draft")]);
    }

    Ok(supported.then_some(native_profile))
}

/// Marks a render job running. Workflow-dispatched exports were already recorded (with their
/// start request) when they were queued, so only jobs that do not exist yet are recorded here.
fn render_job_start_actions(
    project: &VideoProject,
    job: JobSummary,
    updated_at: &str,
    attempt_id: &str,
) -> Vec<ProjectAction> {
    let job_id = job.id.clone();
    let mut actions = Vec::with_capacity(2);
    if !project.jobs.iter().any(|existing| existing.id == job_id) {
        actions.push(ProjectAction::RecordJob { job: Box::new(job) });
    }
    actions.push(ProjectAction::UpdateJobStatus {
        job_id,
        status: JobStatus::Running,
        updated_at: updated_at.to_string(),
        run_id: Some(attempt_id.to_string()),
    });
    actions
}

fn render_project_action_error(
    render_label: &str,
    message: impl Into<String>,
    error: impl std::fmt::Display,
) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        "project.actions",
        format!("Persisting {render_label} render project actions failed."),
        "Inspect the render report and split project validation errors, then retry.",
    )
    .with_detail("stage", message.into())
    .with_detail("error", error.to_string())]
}

fn validate_render_range(range_seconds: Option<(f64, f64)>) -> PipelineResult<()> {
    if let Some((start_seconds, end_seconds)) = range_seconds {
        if !start_seconds.is_finite() || !end_seconds.is_finite() || end_seconds <= start_seconds {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                "timeline.range",
                "Selected timeline range is invalid.",
                "Select a finite timeline range with an end time after its start time.",
            )]);
        }
    }

    Ok(())
}

pub fn build_project_webm_render_plan(
    project_dir: &Path,
    project: &VideoProject,
    job_id: &str,
    profile: RenderQualityProfile,
) -> PipelineResult<RenderPlan> {
    let paths = ProjectWebmRenderPaths::new(job_id);
    let options = ExportRenderOptions::for_project_defaults(
        ExportProfile::Webm,
        crate::edit::render_plan::render_quality_from_legacy(profile),
        project.render_settings.width,
        project.render_settings.height,
    )
    .map_err(export_options_error)?;
    build_project_render_plan_with_range(
        project_dir,
        project,
        paths.absolute_output_path(project_dir),
        options,
        RenderOutputProfile::default(),
        None,
    )
}

pub fn build_project_webm_render_plan_for_range(
    project_dir: &Path,
    project: &VideoProject,
    job_id: &str,
    profile: RenderQualityProfile,
    start_seconds: f64,
    end_seconds: f64,
) -> PipelineResult<RenderPlan> {
    if !start_seconds.is_finite() || !end_seconds.is_finite() || end_seconds <= start_seconds {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "timeline.range",
            "Selected timeline range is invalid.",
            "Select a finite timeline range with an end time after its start time.",
        )]);
    }

    let paths = ProjectWebmRenderPaths::new(job_id);
    let options = ExportRenderOptions::for_project_defaults(
        ExportProfile::Webm,
        crate::edit::render_plan::render_quality_from_legacy(profile),
        project.render_settings.width,
        project.render_settings.height,
    )
    .map_err(export_options_error)?;
    build_project_render_plan_with_range(
        project_dir,
        project,
        paths.absolute_output_path(project_dir),
        options,
        RenderOutputProfile::default(),
        Some((start_seconds, end_seconds)),
    )
}

pub fn build_project_media_render_plan(
    project_dir: &Path,
    project: &VideoProject,
    job_id: &str,
    quality: RenderQuality,
    export_profile: ExportProfile,
) -> PipelineResult<RenderPlan> {
    let options = ExportRenderOptions::for_project_defaults(
        export_profile,
        quality,
        project.render_settings.width,
        project.render_settings.height,
    )
    .map_err(export_options_error)?;
    build_project_media_render_plan_with_options(project_dir, project, job_id, options)
}

pub fn build_project_media_render_plan_with_options(
    project_dir: &Path,
    project: &VideoProject,
    job_id: &str,
    options: ExportRenderOptions,
) -> PipelineResult<RenderPlan> {
    let paths = ProjectMediaRenderPaths::new(job_id, options.profile)?;
    build_project_render_plan_with_range(
        project_dir,
        project,
        paths.absolute_output_path(project_dir),
        options,
        render_output_profile_for_export(options.profile)?,
        None,
    )
}

pub fn build_project_provider_input_render_plan_for_range(
    project_dir: &Path,
    project: &VideoProject,
    output_path: PathBuf,
    start_seconds: f64,
    end_seconds: f64,
) -> PipelineResult<RenderPlan> {
    if !start_seconds.is_finite() || !end_seconds.is_finite() || end_seconds <= start_seconds {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "timeline.range",
            "Selected timeline range is invalid.",
            "Select a finite timeline range with an end time after its start time.",
        )]);
    }

    let options = ExportRenderOptions::for_project_defaults(
        ExportProfile::Mp4H264,
        RenderQuality::Draft,
        project.render_settings.width,
        project.render_settings.height,
    )
    .map_err(export_options_error)?;
    build_project_render_plan_with_range(
        project_dir,
        project,
        output_path,
        options,
        RenderOutputProfile::Mp4Primary,
        Some((start_seconds, end_seconds)),
    )
}

fn build_project_render_plan_with_range(
    project_dir: &Path,
    project: &VideoProject,
    output_path: PathBuf,
    options: ExportRenderOptions,
    output_profile: RenderOutputProfile,
    range_seconds: Option<(f64, f64)>,
) -> PipelineResult<RenderPlan> {
    let mut expanded_project = expand_project_nested_timelines_for_render(project)?;
    let transition_handles = TransitionHandlePlan::extend_project(&mut expanded_project);
    let project = &expanded_project;
    validate_audio_denoise_render_support(project)?;

    let mut video_items = project
        .timeline
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| track.kind == TrackKind::Video && track.enabled)
        .flat_map(|(track_index, track)| {
            track.items.iter().filter_map(move |item| {
                matches!(
                    item.kind,
                    TimelineItemKind::VideoClip
                        | TimelineItemKind::ImageClip
                        | TimelineItemKind::LottieClip
                        | TimelineItemKind::GeneratedClip
                )
                .then_some((track_index as u32, item))
            })
        })
        .collect::<Vec<_>>();
    video_items.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.start_seconds.total_cmp(&right.1.start_seconds))
    });
    let first_item = video_items.first().map(|(_, item)| *item).ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "timeline.tracks.video.items",
            "Project has no source-backed video clips to render.",
            "Add video clips with sourceIn and sourceOut ranges before rendering.",
        )]
    })?;
    let mut clips = Vec::with_capacity(video_items.len());
    let mut clip_keys: Vec<ClipKey> = Vec::with_capacity(video_items.len());
    let mut first_included_item = None;
    for (index, (track_index, item)) in video_items.iter().enumerate() {
        let item_start = item.start_seconds;
        let item_end = item.start_seconds + item.duration_seconds;
        let range_overlap = if let Some((range_start, range_end)) = range_seconds {
            let overlap_start = item_start.max(range_start);
            let overlap_end = item_end.min(range_end);
            if overlap_end <= overlap_start {
                continue;
            }
            Some((range_start, overlap_start, overlap_end))
        } else {
            None
        };
        let media = render_media_for_item(project, item, index)?;
        let speed = numeric_property(item, "speed").unwrap_or(1.0);
        if !speed.is_finite() || !(0.1..=8.0).contains(&speed) {
            return Err(vec![clip_error(
                index,
                "properties.speed",
                "Video clip speed must be between 0.1 and 8.",
                "Use a finite visual speed from 0.1 through 8.",
            )]);
        }
        let still_image = item.kind == TimelineItemKind::ImageClip;
        let source_in = if still_image {
            numeric_property(item, "sourceIn").unwrap_or(0.0)
        } else {
            numeric_property(item, "sourceIn").ok_or_else(|| {
                vec![clip_error(
                    index,
                    "properties.sourceIn",
                    "Video clip is missing numeric sourceIn.",
                    "Set sourceIn on every generated-edit video clip before rendering.",
                )]
            })?
        };
        let default_still_source_out = source_in + item.duration_seconds * speed;
        let source_out = if still_image {
            numeric_property(item, "sourceOut")
                .filter(|source_out| *source_out > source_in)
                .unwrap_or(default_still_source_out)
        } else {
            numeric_property(item, "sourceOut").ok_or_else(|| {
                vec![clip_error(
                    index,
                    "properties.sourceOut",
                    "Video clip is missing numeric sourceOut.",
                    "Set sourceOut on every generated-edit video clip before rendering.",
                )]
            })?
        };
        if !source_in.is_finite()
            || !source_out.is_finite()
            || source_in < 0.0
            || source_out <= source_in
            || (!still_image && source_out > media.duration_seconds)
        {
            return Err(vec![clip_error(
                index,
                "properties.sourceOut",
                "Video clip source range is outside the source media duration.",
                "Keep sourceIn/sourceOut finite, non-negative, ordered, and inside the media duration.",
            )]);
        }
        let window = SourceWindow {
            source_in,
            source_out,
            speed,
            reverse: is_reversed(item),
        };
        let (timeline_start, render_timeline_duration, (render_source_in, render_source_out)) =
            if let Some((range_start, overlap_start, overlap_end)) = range_overlap {
                (
                    overlap_start - range_start,
                    overlap_end - overlap_start,
                    window.source_range_for(overlap_start - item_start, overlap_end - item_start),
                )
            } else {
                (
                    item.start_seconds,
                    item.duration_seconds,
                    (source_in, source_out),
                )
            };
        if window.range_exceeds_window((render_source_in, render_source_out)) {
            return Err(vec![clip_error(
                index,
                "properties.sourceOut",
                "Selected render range extends outside the clip source range.",
                "Keep clip sourceIn/sourceOut aligned with timeline duration before saving the range.",
            )]);
        }
        if first_included_item.is_none() {
            first_included_item = Some(*item);
        }
        let mut properties = render_clip_properties_with_source_dimensions(item, media);
        properties.insert(
            "timelineDurationSeconds".to_string(),
            json!(round_seconds(render_timeline_duration)),
        );
        let (clip_start, clip_end) =
            range_overlap.map_or((item_start, item_end), |overlap| (overlap.1, overlap.2));
        transition_handles.annotate_clip(
            &mut properties,
            *track_index as usize,
            &item.id,
            clip_start,
            clip_end,
        );
        clip_keys.push((*track_index as usize, item.id.clone()));
        clips.push(RenderClip {
            source_path: Some(project_dir.join(&media.relative_path).display().to_string()),
            timeline_start_seconds: Some(round_seconds(timeline_start)),
            properties,
            timeline_track_index: *track_index,
            source_in: round_seconds(render_source_in),
            source_out: round_seconds(render_source_out),
        });
    }
    if clips.is_empty() {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "timeline.range",
            "Selected timeline range has no source-backed video clips to render.",
            "Select a range that overlaps at least one enabled source-backed video clip.",
        )]);
    }
    let first_media = render_media_for_item(project, first_included_item.unwrap_or(first_item), 0)?;
    let mut audio_clips = Vec::new();
    let mut audio_clip_keys: Vec<ClipKey> = Vec::new();
    for (track_index, track) in project.timeline.tracks.iter().enumerate() {
        if track.kind != TrackKind::Audio || !track.enabled {
            continue;
        }
        for item in track
            .items
            .iter()
            .filter(|item| item.kind == TimelineItemKind::AudioClip)
        {
            let item_start = item.start_seconds;
            let item_end = item.start_seconds + item.duration_seconds;
            let Some((timeline_range_start, overlap_start, overlap_end)) = range_seconds
                .map_or_else(
                    || Some((0.0, item_start, item_end)),
                    |(range_start, range_end)| {
                        let overlap_start = item_start.max(range_start);
                        let overlap_end = item_end.min(range_end);
                        (overlap_end > overlap_start).then_some((
                            range_start,
                            overlap_start,
                            overlap_end,
                        ))
                    },
                )
            else {
                continue;
            };
            let media = render_audio_media_for_item(project, item, audio_clips.len())?;
            let source_in = numeric_property(item, "sourceIn").unwrap_or(0.0);
            let source_out = numeric_property(item, "sourceOut").unwrap_or(media.duration_seconds);
            let speed = numeric_property(item, "speed").unwrap_or(1.0);
            if !speed.is_finite() || !(0.1..=8.0).contains(&speed) {
                return Err(vec![clip_error(
                    audio_clips.len(),
                    "properties.speed",
                    "Audio clip speed must be between 0.1 and 8.",
                    "Use a finite audio speed from 0.1 through 8.",
                )]);
            }
            if !source_in.is_finite()
                || !source_out.is_finite()
                || source_in < 0.0
                || source_out <= source_in
                || source_out > media.duration_seconds
            {
                return Err(vec![clip_error(
                    audio_clips.len(),
                    "properties.sourceOut",
                    "Audio clip source range is outside the source media duration.",
                    "Keep audio sourceIn/sourceOut finite, non-negative, ordered, and inside the media duration.",
                )]);
            }
            let window = SourceWindow {
                source_in,
                source_out,
                speed,
                reverse: is_reversed(item),
            };
            let render_range =
                window.source_range_for(overlap_start - item_start, overlap_end - item_start);
            let (render_source_in, render_source_out) = render_range;
            if window.range_exceeds_window(render_range) {
                return Err(vec![clip_error(
                    audio_clips.len(),
                    "properties.sourceOut",
                    "Selected render range extends outside the audio clip source range.",
                    "Keep audio clip sourceIn/sourceOut aligned with timeline duration before saving the range.",
                )]);
            }
            let mut properties = render_clip_properties(item);
            properties.insert(
                "timelineDurationSeconds".to_string(),
                serde_json::json!(round_seconds(overlap_end - overlap_start)),
            );
            transition_handles.annotate_clip(
                &mut properties,
                track_index,
                &item.id,
                overlap_start,
                overlap_end,
            );
            audio_clip_keys.push((track_index, item.id.clone()));
            audio_clips.push(RenderClip {
                source_path: Some(project_dir.join(&media.relative_path).display().to_string()),
                timeline_start_seconds: Some(round_seconds(overlap_start - timeline_range_start)),
                properties,
                timeline_track_index: track_index as u32,
                source_in: round_seconds(render_source_in),
                source_out: round_seconds(render_source_out),
            });
        }
    }

    let (transitions, audio_transitions) = transition_handles.render_transitions(
        &clip_keys,
        &audio_clip_keys,
        range_seconds.map_or(0.0, |(range_start, _)| range_start),
    );

    Ok(RenderPlan {
        input_path: project_dir
            .join(&first_media.relative_path)
            .display()
            .to_string(),
        output_path: output_path.display().to_string(),
        width: options.width,
        height: options.height,
        fps: options.effective_fps(project.render_settings.fps),
        quality: options.quality,
        output_profile,
        encode_tier: options.encode_tier,
        clips,
        audio_clips,
        transitions,
        audio_transitions,
    })
}

pub fn expand_project_nested_timelines_for_render(
    project: &VideoProject,
) -> PipelineResult<VideoProject> {
    let active_timeline_id = project.active_timeline_id.as_deref().unwrap_or("main");
    let mut timelines_by_id = project
        .timelines
        .iter()
        .map(|entry| (entry.id.as_str(), &entry.timeline))
        .collect::<BTreeMap<_, _>>();
    timelines_by_id.insert(active_timeline_id, &project.timeline);

    let mut expanded = project.clone();
    expanded.timeline = expand_timeline_for_render(
        &project.timeline,
        RenderExpansionContext {
            timelines_by_id: &timelines_by_id,
            ancestors: vec![active_timeline_id],
            offset_seconds: 0.0,
            end_seconds: f64::INFINITY,
            parent_enabled: true,
            parent_opacity_curves: Vec::new(),
            parent_motion_wrappers: Vec::new(),
            parent_effect_stack: PreparedEffectStack::default(),
            parent_volume_db: 0.0,
            parent_transform: NestedCanvasTransform::identity(),
            playback_speed: 1.0,
            fps: project.render_settings.fps,
            namespace: "root".to_string(),
        },
    )?;
    Ok(expanded)
}

struct RenderExpansionContext<'a> {
    timelines_by_id: &'a BTreeMap<&'a str, &'a Timeline>,
    ancestors: Vec<&'a str>,
    offset_seconds: f64,
    end_seconds: f64,
    parent_enabled: bool,
    parent_opacity_curves: Vec<AbsoluteOpacityCurve>,
    parent_motion_wrappers: Vec<TimelineItem>,
    parent_effect_stack: PreparedEffectStack,
    parent_volume_db: f64,
    parent_transform: NestedCanvasTransform,
    playback_speed: f64,
    fps: f64,
    namespace: String,
}

fn expand_timeline_for_render<'a>(
    timeline: &Timeline,
    context: RenderExpansionContext<'a>,
) -> PipelineResult<Timeline> {
    let RenderExpansionContext {
        timelines_by_id,
        ancestors,
        offset_seconds,
        end_seconds,
        parent_enabled,
        parent_opacity_curves,
        parent_motion_wrappers,
        parent_effect_stack,
        parent_volume_db,
        parent_transform,
        playback_speed,
        fps,
        namespace,
    } = context;
    let mut tracks = Vec::new();
    let mut all_nested_tracks = Vec::new();
    for (track_index, track) in timeline.tracks.iter().enumerate() {
        let effective_enabled = parent_enabled && track.enabled;
        let mut own_items = Vec::new();
        let mut nested_tracks = Vec::new();
        for item in &track.items {
            let item_start = offset_seconds + item.start_seconds / playback_speed;
            let item_end = (item_start + item.duration_seconds / playback_speed).min(end_seconds);
            if item_end <= item_start {
                continue;
            }
            let mut expanded_item = item.clone();
            if namespace != "root" {
                expanded_item.id = format!("{namespace}:{}", item.id);
            }
            expanded_item.start_seconds = item_start;
            expanded_item.duration_seconds = item_end - item_start;

            let TimelineSource::Timeline { timeline_id } = &item.source else {
                let item_speed = numeric_property(item, "speed").unwrap_or(1.0);
                let composed_speed = item_speed * playback_speed;
                if !composed_speed.is_finite() || !(0.1..=8.0).contains(&composed_speed) {
                    return Err(vec![PipelineError::new(
                        PipelineErrorCode::PipelineInputInvalid,
                        "timeline.source.timelineId.speed",
                        format!(
                            "Nested playback speed for `{}` is outside 0.1 through 8.",
                            item.id
                        ),
                        "Reduce the nested wrapper or child clip speed.",
                    )]);
                }
                if (composed_speed - 1.0).abs() < f64::EPSILON {
                    expanded_item.properties.remove("speed");
                } else {
                    expanded_item
                        .properties
                        .insert("speed".to_string(), serde_json::json!(composed_speed));
                }
                apply_nested_opacity_curves(
                    &mut expanded_item,
                    &parent_opacity_curves,
                    item_start,
                    item_end,
                    fps,
                )
                .map_err(nested_opacity_pipeline_error)?;
                apply_nested_motion_wrappers(
                    &mut expanded_item,
                    &parent_motion_wrappers,
                    item_start,
                    item_end,
                    fps,
                )?;
                compose_nested_transform(&mut expanded_item, parent_transform)?;
                compose_nested_volume_db(&mut expanded_item, parent_volume_db)?;
                if matches!(
                    expanded_item.kind,
                    TimelineItemKind::VideoClip
                        | TimelineItemKind::ImageClip
                        | TimelineItemKind::LottieClip
                        | TimelineItemKind::GeneratedClip
                ) {
                    let child_stack = PreparedEffectStack::from_item_properties_for_duration(
                        &expanded_item.properties,
                        expanded_item.duration_seconds,
                    )
                    .map_err(|error| {
                        vec![PipelineError::new(
                            PipelineErrorCode::PipelineInputInvalid,
                            "timeline.source.timelineId.effects",
                            format!(
                                "Nested child `{}` effect stack is incompatible: {error}",
                                item.id
                            ),
                            "Use only canonical visual effects on nested visual clips.",
                        )]
                    })?;
                    let prepared = child_stack.compose_child_then_wrapper(&parent_effect_stack);
                    if !prepared.effects.is_empty() {
                        expanded_item.properties.insert(
                            "effects".to_string(),
                            serde_json::to_value(&prepared.effects)
                                .expect("prepared effects serialize"),
                        );
                        expanded_item.properties.insert(
                            "preparedEffectStack".to_string(),
                            serde_json::to_value(&prepared).expect("prepared stack serializes"),
                        );
                        expanded_item.properties.insert(
                            "preparedEffectStackFingerprint".to_string(),
                            serde_json::json!(prepared.canonical_fingerprint()),
                        );
                        if let Some(grade) = expanded_item
                            .properties
                            .get_mut("colorGrade")
                            .and_then(serde_json::Value::as_object_mut)
                        {
                            grade.retain(|key, _| key == "lut");
                            if grade.is_empty() {
                                expanded_item.properties.remove("colorGrade");
                            }
                        }
                    }
                }
                if item_end < item_start + item.duration_seconds {
                    let speed = numeric_property(&expanded_item, "speed").unwrap_or(1.0);
                    if let Some((key, seconds)) = truncated_tail_source_property(
                        item,
                        speed,
                        item.duration_seconds / playback_speed,
                        expanded_item.duration_seconds,
                    ) {
                        expanded_item
                            .properties
                            .insert(key.to_string(), serde_json::json!(seconds));
                    }
                }
                own_items.push(expanded_item);
                continue;
            };

            if ancestors.contains(&timeline_id.as_str()) {
                return Err(vec![PipelineError::new(
                    PipelineErrorCode::PipelineInputInvalid,
                    "timeline.source.timelineId",
                    format!("Nested timeline cycle includes `{timeline_id}`."),
                    "Remove the nested timeline reference that closes the cycle.",
                )]);
            }
            let nested_timeline = timelines_by_id.get(timeline_id.as_str()).ok_or_else(|| {
                vec![PipelineError::new(
                    PipelineErrorCode::PipelineInputInvalid,
                    "timeline.source.timelineId",
                    format!("Nested timeline `{timeline_id}` was not found."),
                    "Choose an existing project timeline before rendering.",
                )]
            })?;
            let wrapper = nested_timeline_wrapper_properties(item)?;
            let nested_effect_stack = wrapper
                .effect_stack
                .compose_child_then_wrapper(&parent_effect_stack);
            let mut timed_wrapper = item.clone();
            timed_wrapper.duration_seconds = item.duration_seconds / playback_speed;
            scale_nested_wrapper_animation_timing(&mut timed_wrapper, playback_speed);
            let wrapper_opacity_curve =
                opacity_curve_for_item(&timed_wrapper, item_start, timed_wrapper.duration_seconds)
                    .map_err(nested_opacity_pipeline_error)?;
            let mut nested_opacity_curves = parent_opacity_curves.clone();
            nested_opacity_curves.push(wrapper_opacity_curve);
            if let Some(fade_curve) = fade_opacity_curve_for_item(
                &timed_wrapper,
                item_start,
                timed_wrapper.duration_seconds,
            )
            .map_err(nested_opacity_pipeline_error)?
            {
                nested_opacity_curves.push(fade_curve);
            }
            let mut nested_ancestors = ancestors.clone();
            nested_ancestors.push(timeline_id);
            timed_wrapper.start_seconds = item_start;
            let mut nested_motion_wrappers = parent_motion_wrappers.clone();
            nested_motion_wrappers.push(timed_wrapper.clone());
            nested_tracks.extend(
                expand_timeline_for_render(
                    nested_timeline,
                    RenderExpansionContext {
                        timelines_by_id,
                        ancestors: nested_ancestors,
                        offset_seconds: item_start,
                        end_seconds: item_end,
                        parent_enabled: effective_enabled,
                        parent_opacity_curves: nested_opacity_curves,
                        parent_motion_wrappers: nested_motion_wrappers,
                        parent_effect_stack: nested_effect_stack,
                        parent_volume_db: parent_volume_db + wrapper.volume_db,
                        parent_transform: parent_transform.compose(wrapper.transform),
                        playback_speed: playback_speed * wrapper.speed,
                        fps,
                        namespace: format!("{namespace}:{track_index}:{}", item.id),
                    },
                )?
                .tracks,
            );
        }
        tracks.push(TimelineTrack {
            transitions: carry_transitions_through_expansion(
                track,
                &own_items,
                &namespace,
                playback_speed,
            ),
            id: format!("{namespace}:{}:{track_index}", track.id),
            name: track.name.clone(),
            kind: track.kind.clone(),
            locked: track.locked,
            sync_locked: track.sync_locked,
            enabled: effective_enabled,
            items: own_items,
        });
        all_nested_tracks.extend(nested_tracks);
    }
    tracks.extend(all_nested_tracks);
    Ok(Timeline {
        duration_seconds: (timeline.duration_seconds + offset_seconds).min(end_seconds),
        tracks,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct NestedCanvasTransform {
    center_x: f64,
    center_y: f64,
    width: f64,
    height: f64,
    flip_horizontal: bool,
    flip_vertical: bool,
}

impl NestedCanvasTransform {
    fn identity() -> Self {
        Self {
            center_x: 0.5,
            center_y: 0.5,
            width: 1.0,
            height: 1.0,
            flip_horizontal: false,
            flip_vertical: false,
        }
    }

    /// Applies `child` in this transform's normalized coordinate space.
    fn compose(self, child: Self) -> Self {
        let child_center_x = if self.flip_horizontal {
            1.0 - child.center_x
        } else {
            child.center_x
        };
        let child_center_y = if self.flip_vertical {
            1.0 - child.center_y
        } else {
            child.center_y
        };
        Self {
            center_x: self.center_x + (child_center_x - 0.5) * self.width,
            center_y: self.center_y + (child_center_y - 0.5) * self.height,
            width: self.width * child.width,
            height: self.height * child.height,
            flip_horizontal: self.flip_horizontal != child.flip_horizontal,
            flip_vertical: self.flip_vertical != child.flip_vertical,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct NestedTimelineWrapperProperties {
    volume_db: f64,
    transform: NestedCanvasTransform,
    speed: f64,
    effect_stack: PreparedEffectStack,
}

fn nested_timeline_wrapper_properties(
    item: &TimelineItem,
) -> PipelineResult<NestedTimelineWrapperProperties> {
    if !item.properties.keys().all(|key| {
        matches!(
            key.as_str(),
            "opacity"
                | "keyframes"
                | "transform"
                | "volumeDb"
                | "fadeInSeconds"
                | "fadeOutSeconds"
                | "speed"
                | "positionX"
                | "positionY"
                | "scale"
                | "scaleX"
                | "scaleY"
                | "rotationDegrees"
                | "cropTop"
                | "cropRight"
                | "cropBottom"
                | "cropLeft"
                | "effects"
                | "effectParameterKeyframes"
                | "colorGrade"
        )
    }) {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "timeline.source.timelineId",
            format!(
                "Nested timeline wrapper `{}` has properties whose composition is not supported.",
                item.id
            ),
            "Only canonical visual effects, speed, opacity, fades, audio gain, and normalized canvas transform are supported on nested wrappers.",
        )]);
    }
    validate_nested_wrapper_motion_keyframes(item)?;
    let volume_db = match item.properties.get("volumeDb") {
        None => 0.0,
        Some(value) => value
            .as_f64()
            .filter(|volume_db| volume_db.is_finite() && (-60.0..=24.0).contains(volume_db))
            .ok_or_else(|| {
                vec![PipelineError::new(
                    PipelineErrorCode::PipelineInputInvalid,
                    "timeline.source.timelineId",
                    format!(
                        "Nested timeline wrapper `{}` has invalid audio gain.",
                        item.id
                    ),
                    "Use a finite wrapper audio gain from -60 dB through 24 dB.",
                )]
            })?,
    };
    Ok(NestedTimelineWrapperProperties {
        volume_db,
        transform: nested_canvas_transform(item, "timeline.source.timelineId")?,
        speed: match item.properties.get("speed") {
            None => 1.0,
            Some(value) => value
                .as_f64()
                .filter(|speed| speed.is_finite() && (0.1..=8.0).contains(speed))
                .ok_or_else(|| {
                    vec![PipelineError::new(
                        PipelineErrorCode::PipelineInputInvalid,
                        "timeline.source.timelineId.speed",
                        format!("Nested timeline wrapper `{}` has invalid speed.", item.id),
                        "Use a finite wrapper speed from 0.1 through 8.",
                    )]
                })?,
        },
        effect_stack: PreparedEffectStack::from_item_properties_for_duration(
            &item.properties,
            item.duration_seconds,
        )
        .map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                "timeline.source.timelineId.effects",
                format!("Nested wrapper `{}` effect stack is incompatible: {error}", item.id),
                "Use only canonical visual effects on nested wrappers; audio effects must remain on audio clips.",
            )]
        })?,
    })
}

fn scale_nested_wrapper_animation_timing(item: &mut TimelineItem, speed: f64) {
    for key in ["fadeInSeconds", "fadeOutSeconds"] {
        if let Some(value) = item.properties.get(key).and_then(serde_json::Value::as_f64) {
            item.properties
                .insert(key.to_string(), serde_json::json!(value / speed));
        }
    }
    if let Some(opacity) = item
        .properties
        .get_mut("keyframes")
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|keyframes| keyframes.get_mut("opacity"))
        .and_then(serde_json::Value::as_array_mut)
    {
        for keyframe in opacity {
            if let Some(object) = keyframe.as_object_mut() {
                if let Some(at) = object.get("atSeconds").and_then(serde_json::Value::as_f64) {
                    object.insert("atSeconds".to_string(), serde_json::json!(at / speed));
                }
            }
        }
    }
    if let Some(instances) = item
        .properties
        .get_mut("effectParameterKeyframes")
        .and_then(serde_json::Value::as_object_mut)
    {
        for parameters in instances
            .values_mut()
            .filter_map(serde_json::Value::as_object_mut)
        {
            for lane in parameters
                .values_mut()
                .filter_map(serde_json::Value::as_array_mut)
            {
                for keyframe in lane {
                    if let Some(object) = keyframe.as_object_mut() {
                        if let Some(at) =
                            object.get("atSeconds").and_then(serde_json::Value::as_f64)
                        {
                            object.insert("atSeconds".to_string(), serde_json::json!(at / speed));
                        }
                    }
                }
            }
        }
    }
}

const NESTED_MOTION_PROPERTIES: &[&str] = &[
    "positionX",
    "positionY",
    "scale",
    "scaleX",
    "scaleY",
    "rotationDegrees",
    "cropTop",
    "cropRight",
    "cropBottom",
    "cropLeft",
];

fn validate_nested_wrapper_motion_keyframes(item: &TimelineItem) -> PipelineResult<()> {
    let Some(keyframes) = item.properties.get("keyframes") else {
        return Ok(());
    };
    let keyframes = keyframes.as_object().ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "timeline.source.timelineId.keyframes",
            format!("Nested wrapper `{}` has malformed keyframes.", item.id),
            "Use numeric opacity or transform keyframe arrays.",
        )]
    })?;
    if let Some(property) = keyframes
        .keys()
        .find(|key| key.as_str() != "opacity" && !NESTED_MOTION_PROPERTIES.contains(&key.as_str()))
    {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "timeline.source.timelineId.keyframes",
            format!(
                "Nested wrapper `{}` has unsupported `{property}` keyframes.",
                item.id
            ),
            "Use opacity, position, scale, rotation, or crop keyframes.",
        )]);
    }
    Ok(())
}

fn apply_nested_motion_wrappers(
    item: &mut TimelineItem,
    wrappers: &[TimelineItem],
    absolute_start: f64,
    absolute_end: f64,
    fps: f64,
) -> PipelineResult<()> {
    if wrappers.is_empty() || item.kind == TimelineItemKind::AudioClip {
        return Ok(());
    }
    let has_motion = wrappers.iter().any(|wrapper| {
        NESTED_MOTION_PROPERTIES.iter().any(|property| {
            wrapper.properties.contains_key(*property)
                || wrapper
                    .properties
                    .get("keyframes")
                    .and_then(serde_json::Value::as_object)
                    .is_some_and(|keyframes| keyframes.contains_key(*property))
        })
    });
    if !has_motion {
        return Ok(());
    }
    let frame_start = (absolute_start * fps).ceil() as i64;
    let frame_end = (absolute_end * fps).floor() as i64;
    let mut keyframes = item
        .properties
        .get("keyframes")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    for property in [
        "positionX",
        "positionY",
        "rotationDegrees",
        "scaleX",
        "scaleY",
        "cropTop",
        "cropRight",
        "cropBottom",
        "cropLeft",
    ] {
        let mut points = Vec::new();
        for frame in frame_start..=frame_end {
            let absolute = (frame as f64 / fps).clamp(absolute_start, absolute_end);
            let local = absolute - absolute_start;
            let child_fallback = if property == "scaleX" || property == "scaleY" {
                numeric_property(item, property)
                    .or_else(|| numeric_property(item, "scale"))
                    .unwrap_or(1.0)
            } else {
                numeric_property(item, property).unwrap_or(0.0)
            };
            let mut value = sample_item_numeric_curve(item, property, local, child_fallback)?;
            for wrapper in wrappers {
                let wrapper_local = absolute - wrapper.start_seconds;
                let wrapper_fallback = if property == "scaleX" || property == "scaleY" {
                    numeric_property(wrapper, property)
                        .or_else(|| numeric_property(wrapper, "scale"))
                        .unwrap_or(1.0)
                } else {
                    numeric_property(wrapper, property).unwrap_or(0.0)
                };
                let parent =
                    sample_item_numeric_curve(wrapper, property, wrapper_local, wrapper_fallback)?;
                value = if property == "scaleX" || property == "scaleY" {
                    value * parent
                } else if property.starts_with("crop") {
                    (parent + value * (1.0 - parent)).clamp(0.0, 1.0)
                } else {
                    value + parent
                };
            }
            points
                .push(serde_json::json!({"atSeconds": local, "value": value, "easing": "linear"}));
        }
        keyframes.insert(property.to_string(), serde_json::Value::Array(points));
    }
    item.properties.remove("scale");
    item.properties.insert(
        "keyframes".to_string(),
        serde_json::Value::Object(keyframes),
    );
    Ok(())
}

fn sample_item_numeric_curve(
    item: &TimelineItem,
    property: &str,
    seconds: f64,
    fallback: f64,
) -> PipelineResult<f64> {
    let Some(values) = item
        .properties
        .get("keyframes")
        .and_then(serde_json::Value::as_object)
        .and_then(|keyframes| keyframes.get(property))
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(fallback);
    };
    let mut parsed = Vec::new();
    for value in values {
        let object = value
            .as_object()
            .ok_or_else(|| nested_motion_error(item, property))?;
        let at = object
            .get("atSeconds")
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(|| nested_motion_error(item, property))?;
        let number = object
            .get("value")
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(|| nested_motion_error(item, property))?;
        let easing = object
            .get("easing")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("linear");
        parsed.push((at, number, easing));
    }
    parsed.sort_by(|left, right| left.0.total_cmp(&right.0));
    if parsed.is_empty() {
        return Ok(fallback);
    }
    if seconds <= parsed[0].0 {
        return Ok(parsed[0].1);
    }
    for pair in parsed.windows(2) {
        if seconds <= pair[1].0 {
            let progress = ((seconds - pair[0].0) / (pair[1].0 - pair[0].0)).clamp(0.0, 1.0);
            let eased = match pair[0].2 {
                "linear" => progress,
                "hold" => 0.0,
                "easeIn" => progress * progress,
                "easeOut" => 1.0 - (1.0 - progress).powi(2),
                "easeInOut" | "smooth" => progress * progress * (3.0 - 2.0 * progress),
                _ => return Err(nested_motion_error(item, property)),
            };
            return Ok(pair[0].1 + (pair[1].1 - pair[0].1) * eased);
        }
    }
    Ok(parsed.last().expect("non-empty curve").1)
}

fn nested_motion_error(item: &TimelineItem, property: &str) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        "timeline.source.timelineId.keyframes",
        format!(
            "Nested motion curve `{property}` on `{}` is invalid.",
            item.id
        ),
        "Use ordered finite keyframes and linear, hold, easeIn, easeOut, or easeInOut easing.",
    )]
}

fn nested_opacity_pipeline_error(
    error: crate::project::nested_opacity::NestedOpacityError,
) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        "timeline.source.timelineId",
        error.to_string(),
        "Use finite opacity keyframes in order inside the nested item duration.",
    )]
}

fn nested_canvas_transform(
    item: &TimelineItem,
    path: &str,
) -> PipelineResult<NestedCanvasTransform> {
    let Some(transform_value) = item.properties.get("transform") else {
        return Ok(NestedCanvasTransform::identity());
    };
    let Some(transform) = transform_value.as_object() else {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            path,
            format!(
                "Nested timeline wrapper `{}` has an invalid transform.",
                item.id
            ),
            "Use a transform object with normalized center and dimensions.",
        )]);
    };
    let number =
        |key: &str, fallback: f64, minimum: f64| -> PipelineResult<f64> {
            let value = transform
                .get(key)
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(fallback);
            if !value.is_finite() || !(minimum..=1.0).contains(&value) {
                return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                path,
                format!("Nested timeline wrapper `{}` has an invalid transform {key}.", item.id),
                "Use normalized center values from 0 through 1 and positive dimensions through 1.",
            )]);
            }
            Ok(value)
        };
    Ok(NestedCanvasTransform {
        center_x: number("centerX", 0.5, 0.0)?,
        center_y: number("centerY", 0.5, 0.0)?,
        width: number("width", 1.0, f64::MIN_POSITIVE)?,
        height: number("height", 1.0, f64::MIN_POSITIVE)?,
        flip_horizontal: transform
            .get("flipHorizontal")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        flip_vertical: transform
            .get("flipVertical")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

fn compose_nested_volume_db(item: &mut TimelineItem, parent_volume_db: f64) -> PipelineResult<()> {
    if parent_volume_db.abs() <= f64::EPSILON || !matches!(item.kind, TimelineItemKind::AudioClip) {
        return Ok(());
    }
    let child_volume_db = match item.properties.get("volumeDb") {
        None => 0.0,
        Some(value) => value
            .as_f64()
            .filter(|volume_db| volume_db.is_finite() && (-60.0..=24.0).contains(volume_db))
            .ok_or_else(|| {
                vec![PipelineError::new(
                    PipelineErrorCode::PipelineInputInvalid,
                    "timeline.source.timelineId",
                    format!("Nested audio item `{}` has invalid audio gain.", item.id),
                    "Use a finite audio gain from -60 dB through 24 dB.",
                )]
            })?,
    };
    let composed_volume_db = child_volume_db + parent_volume_db;
    if !(-60.0..=24.0).contains(&composed_volume_db) {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "timeline.source.timelineId",
            format!(
                "Nested audio item `{}` has a composed gain outside the supported range.",
                item.id
            ),
            "Reduce the nested wrapper or child gain so their sum stays from -60 dB through 24 dB.",
        )]);
    }
    item.properties.insert(
        "volumeDb".to_string(),
        serde_json::json!(composed_volume_db),
    );
    Ok(())
}

fn compose_nested_transform(
    item: &mut TimelineItem,
    parent_transform: NestedCanvasTransform,
) -> PipelineResult<()> {
    if parent_transform == NestedCanvasTransform::identity()
        || matches!(item.kind, TimelineItemKind::AudioClip)
    {
        return Ok(());
    }
    let child = nested_canvas_transform(item, "timeline.source.timelineId")?;
    let composed = parent_transform.compose(child);
    item.properties.insert(
        "transform".to_string(),
        serde_json::json!({
            "centerX": composed.center_x,
            "centerY": composed.center_y,
            "width": composed.width,
            "height": composed.height,
            "flipHorizontal": composed.flip_horizontal,
            "flipVertical": composed.flip_vertical,
        }),
    );
    Ok(())
}

fn render_output_profile_for_export(profile: ExportProfile) -> PipelineResult<RenderOutputProfile> {
    match profile {
        ExportProfile::Webm => Ok(RenderOutputProfile::WebPreview),
        ExportProfile::Mp4H264 => Ok(RenderOutputProfile::Mp4Primary),
        ExportProfile::Mp4H265 => Ok(RenderOutputProfile::Mp4Modern),
        ExportProfile::ProResMov => Ok(RenderOutputProfile::QuicktimeInterchange),
        ExportProfile::PalmierProject => Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "exportProfile",
            "Export profile is not supported by the GStreamer project renderer.",
            "Choose MP4 H.264, MP4 H.265, or ProRes MOV for native project media rendering.",
        )
        .with_detail("profile", format!("{profile:?}"))]),
    }
}

fn export_profile_for_output_profile(profile: RenderOutputProfile) -> ExportProfile {
    match profile {
        RenderOutputProfile::WebPreview | RenderOutputProfile::WebDelivery => ExportProfile::Webm,
        RenderOutputProfile::Mp4Primary => ExportProfile::Mp4H264,
        RenderOutputProfile::Mp4Modern => ExportProfile::Mp4H265,
        RenderOutputProfile::QuicktimeInterchange => ExportProfile::ProResMov,
    }
}

fn export_options_error(
    error: crate::project::export_options::ExportRenderOptionsError,
) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        "project.renderSettings",
        "Project render settings are not valid export dimensions.",
        "Use positive, even dimensions no larger than 16,384 pixels.",
    )
    .with_detail("error", error.to_string())]
}

pub fn build_project_graphics_layers(
    project: &VideoProject,
    width: u32,
    height: u32,
    fps: f64,
) -> PipelineResult<Vec<GraphicsLayer>> {
    let expanded_project = expand_project_nested_timelines_for_render(project)?;
    Ok(
        build_project_graphics_render_layers(&expanded_project, width, height, fps, None)?
            .into_iter()
            .map(|graphics| graphics.layer)
            .collect(),
    )
}

fn build_project_graphics_render_layers(
    project: &VideoProject,
    width: u32,
    height: u32,
    fps: f64,
    range_seconds: Option<(f64, f64)>,
) -> PipelineResult<Vec<ProjectGraphicsLayer>> {
    let mut graphics_layers = Vec::new();

    for track in &project.timeline.tracks {
        if !track.enabled {
            continue;
        }

        for item in &track.items {
            let source_range = if let Some((range_start, range_end)) = range_seconds {
                let start = item.start_seconds.max(range_start);
                let end = (item.start_seconds + item.duration_seconds).min(range_end);
                if end <= start {
                    continue;
                }
                Some((start - item.start_seconds, end - start))
            } else {
                None
            };

            if matches!(
                item.kind,
                TimelineItemKind::VideoClip
                    | TimelineItemKind::ImageClip
                    | TimelineItemKind::GeneratedClip
            ) {
                if let Some(layer) = project_visual_effect_graphics_layer(item, width, height, fps)
                {
                    graphics_layers.push(sample_graphics_range(
                        ProjectGraphicsLayer {
                            layer,
                            source_range: None,
                            template_id: None,
                            motion_preset_id: None,
                        },
                        source_range,
                        range_seconds.map(|range| range.0),
                    ));
                }
            }

            match item.kind {
                TimelineItemKind::Caption => {
                    let caption = project_caption_value(item)?;
                    graphics_layers.push(sample_graphics_range(
                        ProjectGraphicsLayer {
                            source_range: None,
                            layer: caption_to_graphics_layer(
                                &caption,
                                graphics_layers.len(),
                                width,
                                height,
                                fps,
                            )?,
                            template_id: None,
                            motion_preset_id: Some(
                                string_property(item, "motionPresetId")
                                    .unwrap_or_else(|| "snap-pop-v1".to_string()),
                            ),
                        },
                        source_range,
                        range_seconds.map(|range| range.0),
                    ));
                }
                TimelineItemKind::Overlay => {
                    let overlay = project_overlay_value(item)?;
                    let template_id = string_property(item, "templateId");
                    let motion_preset_id = string_property(item, "motionPresetId")
                        .or_else(|| {
                            template_id
                                .as_deref()
                                .and_then(project_template_motion_preset)
                        })
                        .or_else(|| Some("slide-fade-up-v1".to_string()));
                    graphics_layers.push(sample_graphics_range(
                        ProjectGraphicsLayer {
                            source_range: None,
                            layer: overlay_to_graphics_layer(
                                &overlay,
                                graphics_layers.len(),
                                width,
                                height,
                                fps,
                            )?,
                            template_id,
                            motion_preset_id,
                        },
                        source_range,
                        range_seconds.map(|range| range.0),
                    ));
                }
                TimelineItemKind::HyperframeScene => {
                    let (layer, template_id, motion_preset_id) =
                        project_hyperframe_layer(item, width, height, fps)?;
                    graphics_layers.push(sample_graphics_range(
                        ProjectGraphicsLayer {
                            layer,
                            source_range: None,
                            template_id: Some(template_id),
                            motion_preset_id,
                        },
                        source_range,
                        range_seconds.map(|range| range.0),
                    ));
                }
                _ => {}
            }
        }
    }

    // Keep authored draw order even when clipping maps several active layers
    // to delivery time zero. Subtracting the sample offset recovers their
    // original start order (the range start is common to all layers).
    let authored_start = |graphics: &ProjectGraphicsLayer| {
        graphics.layer.timeline_start - graphics.source_range.map_or(0.0, |range| range.0)
    };
    graphics_layers.sort_by(|left, right| {
        authored_start(left)
            .total_cmp(&authored_start(right))
            .then_with(|| left.layer.id.cmp(&right.layer.id))
    });
    Ok(graphics_layers)
}

fn sample_graphics_range(
    mut graphics: ProjectGraphicsLayer,
    source_range: Option<(f64, f64)>,
    timeline_range_start: Option<f64>,
) -> ProjectGraphicsLayer {
    if let Some((offset, _)) = source_range {
        graphics.layer.timeline_start = round_seconds(
            graphics.layer.timeline_start + offset - timeline_range_start.unwrap_or(0.0),
        );
        graphics.source_range = source_range;
    }
    graphics
}

fn project_visual_effect_graphics_layer(
    item: &TimelineItem,
    width: u32,
    height: u32,
    fps: f64,
) -> Option<GraphicsLayer> {
    let effects = item.properties.get("effects")?.as_array()?;
    let is_enabled = |effect_type: &str| {
        effects.iter().any(|effect| {
            effect.as_object().is_some_and(|effect| {
                effect.get("effectType").and_then(Value::as_str) == Some(effect_type)
                    && effect.get("enabled").and_then(Value::as_bool) != Some(false)
            })
        })
    };
    let grain = is_enabled("stylize.grain");
    let vignette = is_enabled("stylize.vignette");
    if !grain && !vignette {
        return None;
    }
    let box_rect = Rect {
        x: 0.0,
        y: 0.0,
        width: f64::from(width),
        height: f64::from(height),
    };
    let mut nodes = Vec::new();
    if grain {
        nodes.push(GraphicNode::Rect(RectNode {
            id: "stylize-grain".to_string(),
            box_rect,
            fill: Color::Hex("#FFFFFF2E".to_string()),
            animate: None,
        }));
    }
    if vignette {
        nodes.push(GraphicNode::Rect(RectNode {
            id: "stylize-vignette".to_string(),
            box_rect,
            fill: Color::Hex("#00000040".to_string()),
            animate: None,
        }));
    }
    Some(GraphicsLayer {
        schema_version: 1,
        id: format!("timeline-effects-{}", item.id),
        role: GraphicRole::Overlay,
        timeline_start: item.start_seconds,
        duration_seconds: item.duration_seconds,
        dimensions: Dimensions { width, height },
        fps,
        alpha: true,
        source_beat: format!("Visual finishing for {}", item.label),
        visual_treatment: "restrained film grain and edge vignette that preserve the source action"
            .to_string(),
        motion: "static finishing texture with no added movement or frame obstruction".to_string(),
        safe_zone: "apply only as a transparent full-frame treatment; preserve all source content"
            .to_string(),
        avoid: "opaque overlays, text, face obstruction, and unrelated novelty filters".to_string(),
        nodes,
    })
}

fn render_project_graphics_layers(
    graphics_layers: &[ProjectGraphicsLayer],
    project_dir: &Path,
    graphics_dir: PathBuf,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<Vec<RenderedProjectGraphics>> {
    let assets = AssetRegistry::new(project_dir.to_path_buf());
    let mut rendered = Vec::with_capacity(graphics_layers.len());

    for graphics in graphics_layers {
        ensure_render_not_cancelled(cancellation, "graphics.cancelled")?;
        let layer_dir = graphics_dir.join(safe_path_segment(&graphics.layer.id));
        let manifest = run_cancellable_render_stage(cancellation, "graphics.cancelled", || {
            render_graphics_preview_range_cancellable(
                &graphics.layer,
                graphics.source_range,
                &assets,
                GraphicsRenderOptions {
                    output_dir: layer_dir.clone(),
                },
                || cancellation.is_some_and(RenderCancellationToken::is_cancelled),
            )
            .map_err(PipelineError::from_graphics_errors)
        })?;
        rendered.push(RenderedProjectGraphics {
            artifact_dir: layer_dir,
            manifest,
            timeline_start_seconds: graphics.layer.timeline_start,
            template_id: graphics.template_id.clone(),
            motion_preset_id: graphics.motion_preset_id.clone(),
        });
        ensure_render_not_cancelled(cancellation, "graphics.cancelled")?;
    }

    Ok(rendered)
}

fn ensure_render_not_cancelled(
    cancellation: Option<&RenderCancellationToken>,
    path: &str,
) -> PipelineResult<()> {
    if cancellation.is_some_and(RenderCancellationToken::is_cancelled) {
        Err(render_cancelled_error(path))
    } else {
        Ok(())
    }
}

fn run_cancellable_render_stage<T>(
    cancellation: Option<&RenderCancellationToken>,
    path: &str,
    operation: impl FnOnce() -> PipelineResult<T>,
) -> PipelineResult<T> {
    ensure_render_not_cancelled(cancellation, path)?;
    let value = operation()?;
    ensure_render_not_cancelled(cancellation, path)?;
    Ok(value)
}

fn render_cancelled_error(path: &str) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        path,
        "Render attempt was cancelled.",
        "Start a new render attempt when you are ready to retry.",
    )]
}

fn project_graphics_backend_inputs(
    graphics: &[RenderedProjectGraphics],
) -> Vec<(GraphicsArtifactManifest, PathBuf, f64)> {
    graphics
        .iter()
        .map(|graphics| {
            (
                graphics.manifest.clone(),
                graphics.artifact_dir.clone(),
                graphics.timeline_start_seconds,
            )
        })
        .collect()
}

fn render_project_graphics_report(
    graphics: &RenderedProjectGraphics,
    rendered_frame: Option<String>,
) -> RenderGraphicsReport {
    let mut sampled_frames = vec![graphics.manifest.preview_path.display().to_string()];
    if let Some(rendered_frame) = rendered_frame {
        sampled_frames.push(rendered_frame);
    }

    RenderGraphicsReport {
        layer_id: graphics
            .manifest
            .source_layer_ids
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        renderer: "rust".to_string(),
        quality_profile: None,
        template_id: graphics.template_id.clone(),
        motion_preset_id: graphics.motion_preset_id.clone(),
        visual_qa_status: Some("not-run".to_string()),
        cache_status: None,
        sampled_frames,
        qa_metrics: BTreeMap::new(),
    }
}

fn project_graphics_artifact_paths(
    graphics: &[RenderedProjectGraphics],
    project_dir: &Path,
) -> Vec<String> {
    graphics
        .iter()
        .flat_map(|graphics| {
            let mut paths = vec![
                graphics.artifact_dir.join("manifest.json"),
                graphics.artifact_dir.join(&graphics.manifest.preview_path),
            ];
            for frame_index in 0..graphics.manifest.frame_count {
                paths.push(
                    graphics
                        .artifact_dir
                        .join("frames")
                        .join(format!("frame-{frame_index:06}.png")),
                );
            }
            paths
        })
        .map(|path| project_report_path(project_dir, &path))
        .collect()
}

fn project_report_path(project_dir: &Path, path: &Path) -> String {
    path.strip_prefix(project_dir)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn project_template_motion_preset(template_id: &str) -> Option<String> {
    match template_id {
        "kinetic-lower-third-v1" => Some("slide-fade-up-v1".to_string()),
        "punchy-caption-v1" => Some("snap-pop-v1".to_string()),
        "metric-callout-v1" => Some("metric-count-pop-v1".to_string()),
        "chapter-card-v1" => Some("vertical-reveal-v1".to_string()),
        "tracking-highlight-v1" => Some("tracking-draw-v1".to_string()),
        "holographic-logo-cutout-v1" => Some("pulse-emphasis-v2".to_string()),
        _ => None,
    }
}

fn project_caption_value(item: &TimelineItem) -> PipelineResult<Value> {
    let text = visual_item_text(item).ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "timeline.visuals.caption.text",
            "Caption item is missing renderable text.",
            "Set the caption source text or text property before rendering timeline graphics.",
        )
        .with_detail("itemId", item.id.clone())]
    })?;
    let mut value = project_visual_base_value(item, "caption")?;
    value.insert("text".to_string(), json!(text));
    if let Some(placement) = string_property(item, "captionPlacement") {
        value.insert("captionPlacement".to_string(), json!(placement));
    }
    if let Some(style_preset) = string_property(item, "stylePreset") {
        value.insert("stylePreset".to_string(), json!(style_preset));
    }
    if let Some(emphasized_word_indices) = item.properties.get("emphasizedWordIndices") {
        value.insert(
            "emphasizedWordIndices".to_string(),
            emphasized_word_indices.clone(),
        );
    }
    if let Some(caption_word_timings) = item.properties.get("captionWordTimings") {
        value.insert(
            "captionWordTimings".to_string(),
            caption_word_timings.clone(),
        );
    }
    if let Some(caption_word_animations) = item.properties.get("captionWordAnimations") {
        value.insert(
            "captionWordAnimations".to_string(),
            caption_word_animations.clone(),
        );
    }
    if let Some(preset) = string_property(item, "captionWordAnimationPreset") {
        value.insert("captionWordAnimationPreset".to_string(), json!(preset));
    }
    if let Some(stagger) = item.properties.get("captionWordStaggerSeconds") {
        value.insert("captionWordStaggerSeconds".to_string(), stagger.clone());
    }
    if let Some(motion_preset_id) = string_property(item, "motionPresetId") {
        value.insert("motionPresetId".to_string(), json!(motion_preset_id));
    }
    Ok(Value::Object(value))
}

fn project_overlay_value(item: &TimelineItem) -> PipelineResult<Value> {
    let mut value = project_visual_base_value(item, "overlay")?;
    if let Some(template_id) = string_property(item, "templateId") {
        value.insert("templateId".to_string(), json!(template_id));
        if let Some(fields) = item
            .properties
            .get("templateFields")
            .and_then(Value::as_object)
        {
            value.insert("fields".to_string(), Value::Object(fields.clone()));
        }
        if let Some(preview_variant) = string_property(item, "previewVariant") {
            value.insert("previewVariant".to_string(), json!(preview_variant));
        }
    }
    if let Some(text) = visual_item_text(item) {
        value.insert("text".to_string(), json!(text));
    }
    if let Some(motion_preset_id) = string_property(item, "motionPresetId") {
        value.insert("motionPresetId".to_string(), json!(motion_preset_id));
    }
    Ok(Value::Object(value))
}

fn project_hyperframe_layer(
    item: &TimelineItem,
    width: u32,
    height: u32,
    fps: f64,
) -> PipelineResult<(GraphicsLayer, String, Option<String>)> {
    let kind = string_property(item, "kind").unwrap_or_else(|| "template_overlay".to_string());
    if !is_renderable_project_hyperframe_kind(&kind) {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "timeline.visuals.hyperframe.kind",
            "HyperFrame kind is not renderable yet.",
            "Use kind title_card, lower_third, diagram, transition, immersive_scene, or template_overlay with a supported templateId, or bake the HyperFrame to media before WebM rendering.",
        )
        .with_detail("itemId", item.id.clone())
        .with_detail("kind", kind)]);
    }

    let visual = project_visual_base_value(item, "hyperframe")?;
    let template_id = project_hyperframe_template_id(item, &kind)?;
    let motion_preset_id = string_property(item, "motionPresetId")
        .or_else(|| project_template_motion_preset(&template_id));
    let template_layer = TemplateRenderLayer {
        item_id: item.id.clone(),
        template_id: template_id.clone(),
        label: string_property(item, "sourceBeat").unwrap_or_else(|| item.label.clone()),
        timeline_start_seconds: item.start_seconds,
        duration_seconds: item.duration_seconds,
        fields: hyperframe_template_fields(item, &kind, &template_id),
        visual_treatment: string_from_visual_value(&visual, "visualTreatment"),
        motion: string_from_visual_value(&visual, "motion"),
        safe_zone: string_from_visual_value(&visual, "safeZone"),
        avoid: string_from_visual_value(&visual, "avoid"),
        preview_variant: string_property(item, "previewVariant")
            .unwrap_or_else(|| "hyperframe".to_string()),
    };

    let mut layer = template_layer_to_graphics_ir(
        &template_layer,
        crate::graphics::ir::Dimensions { width, height },
        fps,
    )
    .map_err(|errors| {
        PipelineError::from_graphics_errors(errors)
            .into_iter()
            .map(|mut error| {
                error.path = format!("timeline.visuals.hyperframe.{}", error.path);
                error.details.insert("itemId".to_string(), item.id.clone());
                error
            })
            .collect::<Vec<_>>()
    })?;
    if kind == "lower_third" {
        layer.source_beat =
            string_property(item, "sourceBeat").unwrap_or_else(|| item.label.clone());
    } else if kind == "diagram" {
        layer.role = GraphicRole::Diagram;
        layer.source_beat =
            string_property(item, "sourceBeat").unwrap_or_else(|| item.label.clone());
    } else if kind == "transition" {
        layer.role = GraphicRole::Transition;
        layer.source_beat =
            string_property(item, "sourceBeat").unwrap_or_else(|| item.label.clone());
    } else if kind == "immersive_scene" {
        layer.source_beat =
            string_property(item, "sourceBeat").unwrap_or_else(|| item.label.clone());
    }

    Ok((layer, template_id, motion_preset_id))
}

fn is_renderable_project_hyperframe_kind(kind: &str) -> bool {
    matches!(
        kind,
        "template_overlay"
            | "title_card"
            | "lower_third"
            | "diagram"
            | "transition"
            | "immersive_scene"
    )
}

fn project_hyperframe_template_id(item: &TimelineItem, kind: &str) -> PipelineResult<String> {
    if let Some(template_id) = string_property(item, "templateId") {
        return Ok(template_id);
    }

    match kind {
        "title_card" => Ok("chapter-card-v1".to_string()),
        "lower_third" => Ok("kinetic-lower-third-v1".to_string()),
        "diagram" => Ok("metric-callout-v1".to_string()),
        "transition" => Ok("gradient-background-loop-v1".to_string()),
        "immersive_scene" => Ok(project_immersive_scene_template_id(item)),
        _ => Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "timeline.visuals.hyperframe.templateId",
            "HyperFrame scene is missing a renderable templateId.",
            "Set templateId to a supported graphics template such as chapter-card-v1.",
        )
        .with_detail("itemId", item.id.clone())]),
    }
}

fn project_immersive_scene_template_id(item: &TimelineItem) -> String {
    let mut cues = vec![item.label.as_str()];
    for key in [
        "sourceBeat",
        "role",
        "visualTreatment",
        "motion",
        "safeZone",
    ] {
        if let Some(value) = item.properties.get(key).and_then(Value::as_str) {
            cues.push(value);
        }
    }
    if let Some(fields) = item
        .properties
        .get("templateFields")
        .and_then(Value::as_object)
    {
        for value in fields.values().filter_map(Value::as_str) {
            cues.push(value);
        }
    }

    if scene_cues_contain_any(&cues, &["logo", "brand", "wordmark", "identity"]) {
        "holographic-logo-cutout-v1".to_string()
    } else if scene_cues_contain_any(
        &cues,
        &[
            "metric",
            "data",
            "kpi",
            "retention",
            "conversion",
            "revenue",
            "percent",
            "percentage",
            "growth",
            "pricing",
            "price",
            "cost",
            "savings",
            "roi",
            "margin",
            "spend",
            "budget",
            "comparison",
            "compare",
            "versus",
            "before vs after",
            "split comparison",
        ],
    ) {
        "metric-callout-v1".to_string()
    } else if scene_cues_contain_any(
        &cues,
        &[
            "risk",
            "warning",
            "alert",
            "security",
            "compliance",
            "failure",
            "blocked",
            "urgent",
        ],
    ) {
        "punchy-caption-v1".to_string()
    } else if scene_cues_contain_any(
        &cues,
        &[
            "process",
            "roadmap",
            "workflow",
            "steps",
            "step",
            "sequence",
            "milestone",
            "rollout",
            "journey",
            "timeline",
            "schedule",
            "deadline",
            "calendar",
            "due date",
        ],
    ) {
        "chapter-card-v1".to_string()
    } else if scene_cues_contain_any(
        &cues,
        &[
            "callout",
            "detail",
            "tracking",
            "highlight",
            "pointer",
            "look here",
            "watch this",
            "product",
            "feature",
            "spec",
            "leader line",
            "lens",
            "map",
            "route",
            "location",
            "place marker",
            "map pin",
            "arrival",
            "venue",
            "city map",
        ],
    ) {
        "tracking-highlight-v1".to_string()
    } else if scene_cues_contain_any(
        &cues,
        &[
            "quote",
            "quoted",
            "emphasis",
            "emphasize",
            "punch",
            "punchy",
            "caption",
            "key line",
            "hook line",
            "testimonial",
            "interview",
            "speaker",
            "reaction",
            "emotional",
            "emotion",
            "surprised",
            "surprise",
            "wow",
        ],
    ) {
        "punchy-caption-v1".to_string()
    } else if scene_cues_contain_any(
        &cues,
        &[
            "chapter",
            "story",
            "story beat",
            "reset",
            "section",
            "act break",
            "setup",
            "payoff",
            "proof segment",
            "launch",
            "announcement",
            "announce",
            "release",
        ],
    ) {
        "chapter-card-v1".to_string()
    } else {
        "gradient-background-loop-v1".to_string()
    }
}

fn scene_cues_contain_any(cues: &[&str], keywords: &[&str]) -> bool {
    let text = cues.join(" ").to_ascii_lowercase();
    let normalized = normalize_scene_cue_text(&text);
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    keywords.iter().any(|keyword| {
        let keyword = normalize_scene_cue_text(keyword);
        if keyword.contains(' ') {
            normalized.contains(&keyword)
        } else {
            tokens.contains(&keyword.as_str())
        }
    })
}

fn normalize_scene_cue_text(value: &str) -> String {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn hyperframe_template_fields(
    item: &TimelineItem,
    kind: &str,
    template_id: &str,
) -> BTreeMap<String, String> {
    let mut fields = visual_template_fields(item);
    if (kind == "title_card" || template_requires_subline(template_id))
        && !fields.contains_key("subline")
    {
        fields.insert(
            "subline".to_string(),
            string_property(item, "sourceBeat").unwrap_or_else(|| item.label.clone()),
        );
    }
    if template_id == "holographic-logo-cutout-v1" && !fields.contains_key("logoAssetId") {
        fields.insert(
            "logoAssetId".to_string(),
            "builtin:v-photo-light".to_string(),
        );
    }
    fields
}

fn template_requires_subline(template_id: &str) -> bool {
    matches!(
        template_id,
        "kinetic-lower-third-v1" | "metric-callout-v1" | "chapter-card-v1"
    )
}

fn project_visual_base_value(
    item: &TimelineItem,
    kind: &str,
) -> PipelineResult<Map<String, Value>> {
    if !item.start_seconds.is_finite()
        || item.start_seconds < 0.0
        || !item.duration_seconds.is_finite()
        || item.duration_seconds <= 0.0
    {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "timeline.visuals.timing",
            "Timeline visual item timing is invalid.",
            "Use finite non-negative startSeconds and positive durationSeconds for visual timeline items.",
        )
        .with_detail("itemId", item.id.clone())]);
    }

    let mut value = Map::new();
    value.insert("id".to_string(), json!(item.id));
    value.insert("kind".to_string(), json!(kind));
    value.insert("startSeconds".to_string(), json!(item.start_seconds));
    value.insert("durationSeconds".to_string(), json!(item.duration_seconds));
    value.insert(
        "sourceBeat".to_string(),
        json!(string_property(item, "sourceBeat").unwrap_or_else(|| item.label.clone())),
    );

    for key in ["visualTreatment", "motion", "safeZone", "avoid"] {
        let Some(metadata) = string_property(item, key) else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("timeline.visuals.{key}"),
                "Timeline visual item is missing required render metadata.",
                "Record visualTreatment, motion, safeZone, and avoid for every caption, overlay, and template layer.",
            )
            .with_detail("itemId", item.id.clone())]);
        };
        value.insert(key.to_string(), json!(metadata));
    }

    Ok(value)
}

fn visual_template_fields(item: &TimelineItem) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    if let Some(object) = item
        .properties
        .get("templateFields")
        .and_then(Value::as_object)
    {
        for (key, value) in object {
            if let Some(value) = value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                fields.insert(key.clone(), value.to_string());
            }
        }
    }

    for (field, key) in [
        ("headline", "headline"),
        ("title", "headline"),
        ("label", "headline"),
        ("text", "headline"),
        ("subline", "subline"),
    ] {
        if !fields.contains_key(key) {
            if let Some(value) = string_property(item, field) {
                fields.insert(key.to_string(), value);
            }
        }
    }

    if !fields.contains_key("headline") {
        if let TimelineSource::Text { text } = &item.source {
            if let Some(text) = non_empty_string(text) {
                fields.insert("headline".to_string(), text);
            }
        }
    }

    fields
}

fn string_from_visual_value(visual: &Map<String, Value>, key: &str) -> String {
    visual
        .get(key)
        .and_then(Value::as_str)
        .expect("project_visual_base_value inserts required visual metadata")
        .to_string()
}

fn visual_item_text(item: &TimelineItem) -> Option<String> {
    match &item.source {
        TimelineSource::Text { text } => non_empty_string(text),
        _ => None,
    }
    .or_else(|| string_property(item, "text"))
    .or_else(|| string_property(item, "headline"))
    .or_else(|| string_property(item, "title"))
    .or_else(|| string_property(item, "label"))
    .or_else(|| {
        item.properties
            .get("templateFields")
            .and_then(Value::as_object)
            .and_then(|fields| fields.get("headline"))
            .and_then(Value::as_str)
            .and_then(non_empty_string)
    })
}

fn string_property(item: &TimelineItem, key: &str) -> Option<String> {
    item.properties
        .get(key)
        .and_then(Value::as_str)
        .and_then(non_empty_string)
}

fn non_empty_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

pub fn project_render_report_from_pipeline_report(
    report: &RenderReport,
    paths: &ProjectWebmRenderPaths,
    created_at: &str,
    evidence: &ProjectRenderReportEvidence,
) -> PipelineResult<ProjectRenderReport> {
    let media_paths = ProjectMediaRenderPaths {
        render_dir: paths.render_dir.clone(),
        output_path: paths.output_path.clone(),
        json_report_path: paths.json_report_path.clone(),
        markdown_report_path: paths.markdown_report_path.clone(),
        log_path: paths.log_path.clone(),
    };

    project_media_render_report_from_pipeline_report(report, &media_paths, created_at, evidence)
}

fn project_media_render_report_from_pipeline_report(
    report: &RenderReport,
    paths: &ProjectMediaRenderPaths,
    created_at: &str,
    evidence: &ProjectRenderReportEvidence,
) -> PipelineResult<ProjectRenderReport> {
    let output_path = report
        .summary
        .output_path
        .as_deref()
        .unwrap_or(&paths.output_path);
    let duration_seconds = report.summary.duration_seconds.ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderProbeValidationFailed,
            "renderReport.summary.durationSeconds",
            "Render report is missing output duration.",
            "Probe the rendered media and record its duration before attaching the report.",
        )]
    })?;
    if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderProbeValidationFailed,
            "renderReport.summary.durationSeconds",
            "Render report duration must be positive.",
            "Probe the rendered media and retry after a non-empty render.",
        )]);
    }

    let duration_check = if let Some(expected_duration_seconds) = evidence.expected_duration_seconds
    {
        let tolerance = evidence.duration_tolerance_seconds.unwrap_or(0.05);
        if (duration_seconds - expected_duration_seconds).abs() <= tolerance {
            RenderReportCheckStatus::Passed
        } else {
            RenderReportCheckStatus::Failed
        }
    } else {
        RenderReportCheckStatus::Passed
    };
    let required_artifacts = [
        output_path,
        paths.json_report_path.as_str(),
        paths.markdown_report_path.as_str(),
        paths.log_path.as_str(),
    ];
    let artifact_paths_check = if report
        .artifacts
        .iter()
        .any(|artifact| artifact.trim().is_empty())
        || required_artifacts
            .iter()
            .any(|required| !report.artifacts.iter().any(|artifact| artifact == required))
    {
        RenderReportCheckStatus::Failed
    } else {
        RenderReportCheckStatus::Passed
    };
    let streams_check =
        if evidence.video_stream && (!evidence.audio_required || evidence.audio_stream) {
            RenderReportCheckStatus::Passed
        } else {
            RenderReportCheckStatus::Failed
        };
    let log_path_check = if paths.log_path.trim().is_empty()
        || !report
            .artifacts
            .iter()
            .any(|artifact| artifact == &paths.log_path)
    {
        RenderReportCheckStatus::Failed
    } else {
        RenderReportCheckStatus::Passed
    };
    let visual_timing_check = if report.graphics.is_empty() {
        RenderReportCheckStatus::Skipped
    } else if report.errors.is_empty()
        && report
            .graphics
            .iter()
            .all(|graphics| graphics_has_frame_evidence(graphics, &report.artifacts))
    {
        RenderReportCheckStatus::Passed
    } else {
        RenderReportCheckStatus::Failed
    };
    let preview_comparison =
        report
            .preview_comparison
            .as_ref()
            .map(|comparison| ProjectRenderPreviewComparison {
                status: comparison.status.clone(),
                compared_frames: comparison
                    .compared_frames
                    .iter()
                    .map(|frame| ProjectRenderPreviewComparisonFrame {
                        timeline_seconds: frame.timeline_seconds,
                        preview_frame: frame.preview_frame.clone(),
                        rendered_frame: frame.rendered_frame.clone(),
                        diff_frame: frame.diff_frame.clone(),
                        mismatch_ratio: frame.mismatch_ratio,
                        passed: frame.passed,
                    })
                    .collect(),
            });
    let mut artifacts = report.artifacts.clone();
    if let Some(comparison) = &preview_comparison {
        for frame in &comparison.compared_frames {
            append_unique_artifact(&mut artifacts, &frame.preview_frame);
            append_unique_artifact(&mut artifacts, &frame.rendered_frame);
            if let Some(diff_frame) = &frame.diff_frame {
                append_unique_artifact(&mut artifacts, diff_frame);
            }
        }
    }

    let failed_evidence = streams_check == RenderReportCheckStatus::Failed
        || duration_check == RenderReportCheckStatus::Failed
        || artifact_paths_check == RenderReportCheckStatus::Failed
        || log_path_check == RenderReportCheckStatus::Failed
        || visual_timing_check == RenderReportCheckStatus::Failed
        || !report.errors.is_empty();

    Ok(ProjectRenderReport {
        schema_version: 1,
        id: report.job_id.clone(),
        status: if failed_evidence {
            RenderReportStatus::Failed
        } else {
            RenderReportStatus::Completed
        },
        output_path: output_path.to_string(),
        duration_seconds,
        quality: report.summary.quality,
        requested_width: report.summary.requested_width,
        requested_height: report.summary.requested_height,
        actual_width: report.summary.actual_width,
        actual_height: report.summary.actual_height,
        streams: RenderReportStreams {
            video: evidence.video_stream,
            audio: evidence.audio_stream,
        },
        checks: BTreeMap::from([
            ("duration".to_string(), duration_check),
            ("captionAlignment".to_string(), visual_timing_check.clone()),
            ("overlayTiming".to_string(), visual_timing_check.clone()),
            ("visualFrameEvidence".to_string(), visual_timing_check),
            ("artifactPaths".to_string(), artifact_paths_check),
            ("streams".to_string(), streams_check),
            ("logPath".to_string(), log_path_check),
        ]),
        artifacts,
        preview_comparison_request: report.preview_comparison_request.as_ref().map(|request| {
            ProjectRenderPreviewComparisonRequest {
                status: request.status.clone(),
                project_dir: request.project_dir.clone(),
                project_report_id: request.project_report_id.clone(),
                render_report_path: request.render_report_path.clone(),
                rendered_video: request.rendered_video.clone(),
                duration_seconds: request.duration_seconds,
                frame_time_seconds: request.frame_time_seconds,
                rendered_frames: request.rendered_frames.clone(),
                fail_on_mismatch: request.fail_on_mismatch,
            }
        }),
        preview_comparison,
        log_path: paths.log_path.clone(),
        created_at: created_at.to_string(),
    })
}

fn append_unique_artifact(artifacts: &mut Vec<String>, artifact: &str) {
    if !artifact.trim().is_empty() && !artifacts.iter().any(|existing| existing == artifact) {
        artifacts.push(artifact.to_string());
    }
}

fn graphics_has_frame_evidence(graphics: &RenderGraphicsReport, artifacts: &[String]) -> bool {
    graphics
        .sampled_frames
        .iter()
        .any(|frame| !frame.trim().is_empty() && artifacts.iter().any(|artifact| artifact == frame))
}

fn render_media_for_item<'a>(
    project: &'a VideoProject,
    item: &TimelineItem,
    index: usize,
) -> PipelineResult<&'a MediaAsset> {
    let media_id = match &item.source {
        TimelineSource::Media { media_id } => media_id.as_str(),
        TimelineSource::Generated { artifact_id } => {
            let generated_asset = project
                .generated_assets
                .iter()
                .find(|asset| asset.id == *artifact_id)
                .ok_or_else(|| {
                    vec![clip_error(
                        index,
                        "source.artifactId",
                        "Generated render asset was not found.",
                        "Keep generated timeline clips linked to a completed generated asset.",
                    )
                    .with_detail("artifactId", artifact_id.clone())]
                })?;
            if generated_asset.status != GeneratedAssetStatus::Completed {
                return Err(vec![clip_error(
                    index,
                    "source.artifactId",
                    "Generated render asset is not completed.",
                    "Wait for generation to complete before rendering this timeline clip.",
                )
                .with_detail("artifactId", artifact_id.clone())]);
            }
            generated_asset
                .outputs
                .first()
                .map(|output| output.media_id.as_str())
                .ok_or_else(|| {
                    vec![clip_error(
                        index,
                        "source.artifactId",
                        "Generated render asset has no outputs.",
                        "Record at least one generated output before rendering this timeline clip.",
                    )
                    .with_detail("artifactId", artifact_id.clone())]
                })?
        }
        TimelineSource::Timeline { timeline_id } => {
            return Err(vec![clip_error(
                index,
                "source.timelineId",
                "Nested timeline rendering is not implemented yet.",
                "Decompose the nested timeline before rendering, or use a media-backed clip.",
            )
            .with_detail("timelineId", timeline_id.clone())]);
        }
        TimelineSource::Text { .. } => {
            return Err(vec![clip_error(
                index,
                "source",
                "Video clip does not reference renderable media.",
                "Use media-backed or completed generated video clips for WebM rendering.",
            )]);
        }
    };

    let media = project
        .media
        .iter()
        .find(|media| media.id == media_id)
        .ok_or_else(|| {
            vec![clip_error(
                index,
                "source.mediaId",
                "Render media asset was not found.",
                "Keep every timeline video clip linked to an imported video or generated output media asset.",
            )
            .with_detail("mediaId", media_id.to_string())]
        })?;

    if media.kind == MediaKind::Lottie {
        return Err(vec![clip_error(
            index,
            "source.mediaId",
            "Lottie media must be baked before WebM rendering.",
            "Bake the Lottie animation to a playable video asset, then render the timeline with that baked output.",
        )
        .with_detail("mediaId", media_id.to_string())
        .with_detail("mediaKind", "lottie")]);
    }

    if !matches!(
        media.kind,
        MediaKind::Video | MediaKind::Image | MediaKind::Generated
    ) {
        return Err(vec![clip_error(
            index,
            "source.mediaId",
            "Render media asset is not a renderable video source.",
            "Use imported video, imported still images, generated video output, or a baked visual asset for WebM rendering.",
        )
        .with_detail("mediaId", media_id.to_string())
        .with_detail("mediaKind", media_kind_label(&media.kind))]);
    }

    Ok(media)
}

fn render_audio_media_for_item<'a>(
    project: &'a VideoProject,
    item: &TimelineItem,
    index: usize,
) -> PipelineResult<&'a MediaAsset> {
    let TimelineSource::Media { media_id } = &item.source else {
        return Err(vec![clip_error(
            index,
            "source",
            "Audio clip does not reference imported audio media.",
            "Use imported audio media for timeline audio clips before rendering.",
        )]);
    };
    let media = project
        .media
        .iter()
        .find(|media| media.id == *media_id)
        .ok_or_else(|| {
            vec![clip_error(
                index,
                "source.mediaId",
                "Audio render media asset was not found.",
                "Keep every timeline audio clip linked to imported audio media.",
            )
            .with_detail("mediaId", media_id.clone())]
        })?;
    if !matches!(
        media.kind,
        MediaKind::Audio | MediaKind::Video | MediaKind::Generated
    ) {
        return Err(vec![clip_error(
            index,
            "source.mediaId",
            "Audio clip must reference audio, video, or generated media.",
            "Choose imported audio or video media, or a completed generated output, that can carry an audio stream.",
        )
        .with_detail("mediaId", media_id.clone())
        .with_detail("mediaKind", media_kind_label(&media.kind))]);
    }
    Ok(media)
}

fn media_result_from_write(
    write: ProjectActionWriteResult,
    render_report: RenderReport,
    project_render_report: ProjectRenderReport,
    output_path: String,
    export_artifact: Option<ProjectExportArtifact>,
) -> ProjectMediaRenderResult {
    ProjectMediaRenderResult {
        project: write.project,
        render_report,
        project_render_report,
        output_path,
        export_artifact,
    }
}

fn timeline_has_audio(project: &VideoProject) -> bool {
    project
        .timeline
        .tracks
        .iter()
        .any(|track| track.kind == TrackKind::Audio && track.enabled && !track.items.is_empty())
}

fn validate_audio_denoise_render_support(project: &VideoProject) -> PipelineResult<()> {
    for track in project
        .timeline
        .tracks
        .iter()
        .filter(|track| track.kind == TrackKind::Audio && track.enabled)
    {
        for (item_index, item) in track.items.iter().enumerate() {
            if item.kind == TimelineItemKind::AudioClip && has_enabled_audio_denoise(item) {
                return Err(vec![PipelineError::new(
                    PipelineErrorCode::PipelineInputInvalid,
                    format!("timeline.tracks.audio.items[{item_index}].effects.audio.denoise"),
                    "audio denoise is not available in the render pipeline yet.",
                    "Disable Denoise on this clip or bake a denoised audio asset before rendering.",
                )
                .with_detail("trackId", track.id.clone())
                .with_detail("itemId", item.id.clone())]);
            }
        }
    }
    Ok(())
}

fn has_enabled_audio_denoise(item: &TimelineItem) -> bool {
    item.properties
        .get("effects")
        .and_then(Value::as_array)
        .is_some_and(|effects| {
            effects.iter().any(|effect| {
                effect_type(effect) == Some("audio.denoise")
                    && effect
                        .get("enabled")
                        .and_then(Value::as_bool)
                        .unwrap_or(true)
            })
        })
}

fn effect_type(effect: &Value) -> Option<&str> {
    effect
        .get("effectType")
        .or_else(|| effect.get("type"))
        .and_then(Value::as_str)
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

fn render_duration_tolerance_seconds(fps: f64) -> f64 {
    if fps.is_finite() && fps > 0.0 {
        // GES timestamps are frame-aligned, while MP4/MOV muxers can retain
        // encoder padding at the tail. Keep validation tight enough to catch
        // an edit-length error while allowing that bounded container variance.
        (2.0 / fps).max(0.1)
    } else {
        0.1
    }
}

pub fn expected_media_for_render_plan(plan: &RenderPlan, audio_required: bool) -> ExpectedMedia {
    let (container, video_codec, audio_codec) = match plan.output_profile {
        RenderOutputProfile::WebPreview => ("webm", "vp8", "opus"),
        RenderOutputProfile::WebDelivery => ("webm", "vp9", "opus"),
        RenderOutputProfile::Mp4Primary => ("mp4", "h264", "aac"),
        RenderOutputProfile::Mp4Modern => ("mp4", "hevc", "aac"),
        RenderOutputProfile::QuicktimeInterchange => ("mov", "prores", "pcm"),
    };
    ExpectedMedia {
        width: Some(plan.width),
        height: Some(plan.height),
        video_required: true,
        audio_required,
        non_empty_required: true,
        expected_duration_seconds: Some(render_plan_duration_seconds(plan)),
        duration_tolerance_seconds: Some(render_duration_tolerance_seconds(plan.fps)),
        container: Some(container.to_string()),
        video_codec: Some(video_codec.to_string()),
        // The codec check only runs when the output carries an audio stream, and
        // the profile's audio codec is the right one whether the stream came from
        // the timeline or from the audio track GES always renders.
        audio_codec: Some(audio_codec.to_string()),
    }
}

/// The video codec the export actually wrote. Containers spell the same codec
/// several ways (`video/x-h265`, `hvc1`, `hevc`), and the probe normalises them
/// to one name, so reporting that name keeps the summary describing the file
/// rather than the plan's wording.
fn rendered_video_codec(probe: &MediaProbe, expected_media: &ExpectedMedia) -> Option<String> {
    probe
        .video
        .as_ref()
        .and_then(|video| video.codec_name.as_deref())
        .map(normalized_video_codec_name)
        .or_else(|| expected_media.video_codec.clone())
}

/// The audio codec the export actually wrote, or `None` when it wrote no audio
/// stream. GES renders an audio/video timeline and every delivery profile muxes
/// an audio track, so a timeline without audio still produces a silent stream
/// that the report has to name; reporting the timeline's expectation instead
/// would describe the file as audio-free while it carries audio.
fn rendered_audio_codec(probe: &MediaProbe, expected_media: &ExpectedMedia) -> Option<String> {
    let audio = probe.audio.as_ref()?;
    audio
        .codec_name
        .as_deref()
        .map(normalized_audio_codec_name)
        .or_else(|| expected_media.audio_codec.clone())
}

fn labeled_output(sections: &[(&str, &str, &str)]) -> String {
    sections
        .iter()
        .flat_map(|(label, stdout, stderr)| {
            [
                format!("== {label} stdout ==\n{stdout}"),
                format!("== {label} stderr ==\n{stderr}"),
            ]
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn io_pipeline_error(
    path: &str,
    message: &str,
    error: impl std::fmt::Display,
) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        path,
        message,
        "Ensure the render folder is writable and retry.",
    )
    .with_detail("ioError", error.to_string())]
}

fn numeric_property(item: &TimelineItem, key: &str) -> Option<f64> {
    item.properties.get(key).and_then(|value| value.as_f64())
}

fn render_clip_properties(item: &TimelineItem) -> BTreeMap<String, serde_json::Value> {
    const RENDER_CLIP_PROPERTY_KEYS: &[&str] = &[
        "opacity",
        "blendMode",
        "timelineDurationSeconds",
        "volumeDb",
        "fadeInSeconds",
        "fadeOutSeconds",
        "speed",
        "centerX",
        "centerY",
        "width",
        "height",
        "flipHorizontal",
        "flipVertical",
        "transform",
        "positionX",
        "positionY",
        "scale",
        "scaleX",
        "scaleY",
        "rotationDegrees",
        "cropTop",
        "cropRight",
        "cropBottom",
        "cropLeft",
        "keyframes",
        "colorGrade",
        "effects",
        "preparedFrames",
        "reverse",
    ];

    RENDER_CLIP_PROPERTY_KEYS
        .iter()
        .filter_map(|key| {
            item.properties
                .get(*key)
                .map(|value| ((*key).to_string(), value.clone()))
        })
        .collect()
}

fn attach_prepared_frame_sequences(
    project_dir: &Path,
    prepared: &mut crate::precompose::PreparedProject,
) -> PipelineResult<()> {
    for report in &prepared.reports {
        if !report.has_frame_sequence() {
            continue;
        }
        let intermediate = project_dir.join(&report.intermediate);
        let Some(cache_dir) = intermediate.parent() else {
            continue;
        };
        let frames_dir = cache_dir.join("frames");
        let mut frame_paths = std::fs::read_dir(&frames_dir)
            .map_err(|error| {
                vec![PipelineError::new(
                    PipelineErrorCode::PipelineInputInvalid,
                    "precompose.preparedFrames",
                    "Prepared visual frame directory could not be read.",
                    "Regenerate the canonical prepared visual artifact.",
                )
                .with_detail("error", error.to_string())]
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
            })
            .collect::<Vec<_>>();
        frame_paths.sort();
        if frame_paths.is_empty() {
            continue;
        }
        let prepared_media_duration = prepared
            .project
            .media
            .iter()
            .find(|media| media.id == report.prepared_media_id)
            .map(|media| media.duration_seconds);
        for item in prepared
            .project
            .timeline
            .tracks
            .iter_mut()
            .flat_map(|track| track.items.iter_mut())
        {
            let matches_media = matches!(&item.source, TimelineSource::Media { media_id } if media_id == &report.prepared_media_id);
            if !matches_media {
                continue;
            }
            // The sequence covers the whole prepared media, including any
            // transition handles the clip reads past `sourceIn`.
            let sequence_duration = prepared_media_duration
                .filter(|duration| duration.is_finite() && *duration > 0.0)
                .unwrap_or(item.duration_seconds);
            item.properties.insert("preparedFrames".to_string(), serde_json::json!({
                "framePaths": frame_paths.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
                "frameDurationSeconds": sequence_duration / frame_paths.len() as f64,
                "alpha": true,
                "fingerprint": report.fingerprint,
            }));
        }
    }
    Ok(())
}

fn render_clip_properties_with_source_dimensions(
    item: &TimelineItem,
    media: &MediaAsset,
) -> BTreeMap<String, serde_json::Value> {
    let mut properties = render_clip_properties(item);
    if let Some(width) = media.width.filter(|width| *width > 0) {
        properties.insert("sourceWidth".to_string(), serde_json::json!(width));
    }
    if let Some(height) = media.height.filter(|height| *height > 0) {
        properties.insert("sourceHeight".to_string(), serde_json::json!(height));
    }
    properties
}

fn round_seconds(value: f64) -> f64 {
    seconds_to_millis(value) as f64 / 1000.0
}

fn seconds_to_millis(value: f64) -> i64 {
    (value * 1000.0).round() as i64
}

fn clip_error(index: usize, path: &str, message: &str, fix: &str) -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        format!("timeline.tracks.video.items[{index}].{path}"),
        message,
        fix,
    )
}

fn media_kind_label(kind: &MediaKind) -> &'static str {
    match kind {
        MediaKind::Video => "video",
        MediaKind::Audio => "audio",
        MediaKind::Image => "image",
        MediaKind::Lottie => "lottie",
        MediaKind::Generated => "generated",
    }
}

fn safe_path_segment(value: &str) -> String {
    let segment = value
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if segment.is_empty() {
        "render".to_string()
    } else {
        segment
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::fixtures::sample_project;
    use crate::render_pipeline::avfoundation_backend::{
        AvFoundationCapabilities, AvFoundationExportProfile,
    };

    #[test]
    fn ranged_graphics_preserve_original_stacking_order() {
        let mut project = sample_project();
        project.timeline.tracks.truncate(1);
        let track = &mut project.timeline.tracks[0];
        track.kind = TrackKind::Caption;
        track.items = [("z-earlier", 0.0), ("a-later", 1.0)]
            .into_iter()
            .map(|(id, start)| TimelineItem {
                id: id.into(),
                kind: TimelineItemKind::Caption,
                start_seconds: start,
                duration_seconds: 3.0,
                source: TimelineSource::Text { text: id.into() },
                label: id.into(),
                properties: BTreeMap::from([
                    ("visualTreatment".into(), json!("accent emphasis")),
                    ("motion".into(), json!("snap pop")),
                    ("safeZone".into(), json!("10% margins")),
                    ("avoid".into(), json!("opaque slabs")),
                ]),
            })
            .collect();
        let full = build_project_graphics_render_layers(&project, 160, 96, 8.0, None).unwrap();
        let range =
            build_project_graphics_render_layers(&project, 160, 96, 8.0, Some((2.0, 2.125)))
                .unwrap();
        assert_eq!(
            full.iter().map(|g| &g.layer.id).collect::<Vec<_>>(),
            range.iter().map(|g| &g.layer.id).collect::<Vec<_>>()
        );
    }

    #[test]
    #[cfg(feature = "graphics-render")]
    fn highlighted_caption_range_matches_full_animation_frames() {
        let root = tempfile::tempdir().unwrap();
        let mut project = sample_project();
        project.render_settings.width = 160;
        project.render_settings.height = 96;
        project.render_settings.fps = 8.0;
        project.timeline.duration_seconds = 2.0;
        project.timeline.tracks.truncate(1);
        let track = &mut project.timeline.tracks[0];
        track.kind = TrackKind::Caption;
        track.items = vec![TimelineItem {
            id: "timed".into(),
            kind: TimelineItemKind::Caption,
            start_seconds: 0.0,
            duration_seconds: 2.0,
            source: TimelineSource::Text {
                text: "hello world".into(),
            },
            label: "Caption".into(),
            properties: BTreeMap::from([
                ("visualTreatment".into(), json!("accent emphasis")),
                ("motion".into(), json!("snap pop")),
                ("safeZone".into(), json!("10% margins")),
                ("avoid".into(), json!("opaque slabs")),
                ("emphasizedWordIndices".into(), json!([0, 1])),
                (
                    "captionWordTimings".into(),
                    json!([
                        {"wordIndex":0,"startSeconds":0.0,"endSeconds":1.0},
                        {"wordIndex":1,"startSeconds":1.0,"endSeconds":2.0}
                    ]),
                ),
            ]),
        }];
        let full = build_project_graphics_render_layers(&project, 160, 96, 8.0, None).unwrap();
        let full =
            render_project_graphics_layers(&full, root.path(), root.path().join("full"), None)
                .unwrap();
        for (start, end) in [(0.0, 0.125), (1.0, 1.125), (1.875, 2.0), (0.5, 1.5)] {
            let selected =
                build_project_graphics_render_layers(&project, 160, 96, 8.0, Some((start, end)))
                    .expect("ranged timed caption validates");
            let selected = render_project_graphics_layers(
                &selected,
                root.path(),
                root.path().join(format!("range-{start}")),
                None,
            )
            .unwrap();
            assert_eq!(selected[0].manifest.duration_seconds, end - start);
            for index in 0..selected[0].manifest.frame_count {
                let original_index = (start * 8.0) as u32 + index;
                let a = image::open(
                    full[0]
                        .artifact_dir
                        .join(format!("frames/frame-{original_index:06}.png")),
                )
                .unwrap()
                .into_rgba8();
                let b = image::open(
                    selected[0]
                        .artifact_dir
                        .join(format!("frames/frame-{index:06}.png")),
                )
                .unwrap()
                .into_rgba8();
                assert_eq!(a.as_raw(), b.as_raw(), "range {start} frame {index}");
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let folder = root.path().join("capture-project");
            crate::project::split::save_split_project(&folder, &project).unwrap();
            let capture = render_prepared_preview_frame_to_split_project_folder(
                &folder,
                1.0,
                "timed-caption-capture",
                "2026-10-03T12:00:00Z",
            )
            .expect("public capture API accepts emphasized timings");
            let expected = image::open(full[0].artifact_dir.join("frames/frame-000008.png"))
                .unwrap()
                .into_rgba8();
            let actual = image::open(folder.join(capture.preview_frame))
                .unwrap()
                .into_rgba8();
            assert_eq!(
                expected.as_raw(),
                actual.as_raw(),
                "canonical capture preserves caption animation phase"
            );
        }
    }

    #[test]
    fn normal_render_entry_registers_before_waiting_for_project_lease() {
        use crate::project::mutation::acquire_split_project_mutation_lease;
        use crate::project::split::save_split_project;
        use crate::render_pipeline::cancel::{
            request_render_cancellation, RenderCancellationOutcome,
        };
        use crate::workflows::{temporal_job_summary, TemporalWorkflowKind};
        use std::sync::mpsc;
        use std::time::{Duration, Instant};

        let temp = tempfile::tempdir().expect("temp");
        let project_dir = temp.path().join("project");
        let project = sample_project();
        save_split_project(&project_dir, &project).expect("save project");
        let attempt_id = "render-attempt/blocked-start";
        let key = RenderAttemptKey::new(
            std::fs::canonicalize(&project_dir).expect("canonical root"),
            &project.id,
            "blocked-render",
            attempt_id,
        )
        .expect("key");
        let lease = acquire_split_project_mutation_lease(&project_dir).expect("hold project lease");
        let (done_tx, done_rx) = mpsc::channel();
        let render_dir = project_dir.clone();
        let project_id = project.id.clone();
        let render = std::thread::spawn(move || {
            let job = temporal_job_summary(
                TemporalWorkflowKind::RenderDraft,
                &project_id,
                "blocked-render",
                JobStatus::Queued,
                "2026-09-12T00:00:00Z",
            );
            let result = render_webm_to_split_project_folder(
                &render_dir,
                &project_id,
                RenderQualityProfile::DraftWebm,
                job,
                "2026-09-12T00:00:00Z",
                Some(attempt_id.to_string()),
                None,
            );
            done_tx.send(()).expect("send completion");
            result
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        while !is_render_attempt_active(&key) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(is_render_attempt_active(&key));
        assert_eq!(
            request_render_cancellation(&key),
            RenderCancellationOutcome::Requested
        );
        assert!(
            done_rx.try_recv().is_err(),
            "render must still be lease-blocked"
        );
        drop(lease);
        let errors = render.join().expect("render joins").expect_err("cancelled");
        assert_eq!(errors[0].path, "render.start.cancelled");
        assert!(!project_dir.join("renders/blocked-render").exists());
        let persisted = crate::project::split::load_split_project(&project_dir).expect("reload");
        assert_eq!(
            persisted
                .jobs
                .iter()
                .find(|job| job.id == "blocked-render")
                .expect("cancelled job")
                .status,
            JobStatus::Cancelled
        );
    }

    #[test]
    fn project_export_rejects_draft_prores_when_proxy_is_unavailable_but_keeps_final_422() {
        let capabilities = AvFoundationCapabilities {
            protocol: "video-creater.avfoundation-export".to_string(),
            schema_version: 5,
            backend: "avfoundation-native".to_string(),
            profiles: BTreeMap::from([
                ("proResProxy".to_string(), false),
                ("proRes422".to_string(), true),
            ]),
        };

        let draft_error = select_project_export_avfoundation_profile(
            ExportProfile::ProResMov,
            RenderQuality::Draft,
            Some(&capabilities),
        )
        .expect_err("Draft ProRes must not reach Swift without ProRes Proxy support");
        assert_eq!(draft_error[0].path, "renderPlan.quality");
        assert!(draft_error[0].message.contains("ProRes Proxy"));

        assert_eq!(
            select_project_export_avfoundation_profile(
                ExportProfile::ProResMov,
                RenderQuality::Final,
                Some(&capabilities),
            )
            .expect("Final ProRes should select the reported 422 codec"),
            Some(AvFoundationExportProfile::ProRes422)
        );
    }

    #[cfg(unix)]
    #[test]
    fn rendered_frame_sample_publication_rejects_nested_symlink_escape() {
        use crate::precompose::compatibility::publish_compatibility_png_with_validator;
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temporary root");
        let project_dir = temp.path().join("project");
        let frames_dir = project_dir.join("renders/review/frames");
        std::fs::create_dir_all(&frames_dir).expect("create frames directory");
        let external = temp.path().join("external.png");
        std::fs::write(&external, b"outside").expect("write external target");
        let destination = frames_dir.join("frame-000001.png");
        symlink(&external, &destination).expect("nested frame symlink");
        let worker_frame = temp.path().join("worker.bmp");
        image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]))
            .save_with_format(&worker_frame, image::ImageFormat::Bmp)
            .expect("write worker frame");

        let error =
            publish_compatibility_png_with_validator(&worker_frame, &destination, &mut |path| {
                validate_rendered_frame_sample_destination(&project_dir, path)
            })
            .expect_err("frame sample destination symlink must be rejected");

        assert!(error.contains("symbolic link"));
        assert_eq!(
            std::fs::read(&external).expect("read external target"),
            b"outside"
        );
    }

    #[test]
    fn cancellable_stage_rejects_results_when_cancellation_arrives_during_work() {
        use crate::render_pipeline::cancel::{
            register_render_attempt, request_render_cancellation, RenderAttemptKey,
            RenderCancellationOutcome,
        };

        for (index, stage) in ["graphics.cancelled", "renderedFrames.cancelled"]
            .into_iter()
            .enumerate()
        {
            let key = RenderAttemptKey::new(
                PathBuf::from(format!("/tmp/render-stage-{index}")),
                "project",
                "job",
                "attempt",
            )
            .expect("key");
            let guard = register_render_attempt(key.clone()).expect("register");
            let token = guard.token();
            let error = run_cancellable_render_stage(Some(&token), stage, || {
                assert_eq!(
                    request_render_cancellation(&key),
                    RenderCancellationOutcome::Requested
                );
                Ok("unpublished result")
            })
            .expect_err("cancelled stage result must not publish");
            assert_eq!(error[0].path, stage);
        }
    }

    #[test]
    fn cancelled_attempt_removes_late_graphics_review_and_success_artifacts() {
        let temp = tempfile::tempdir().expect("temp");
        let project_dir = temp.path();
        let paths = ProjectMediaRenderPaths::webm("render-cleanup");
        let render_dir = paths.absolute_render_dir(project_dir);
        for relative in [
            "output.webm",
            "pipeline-report.json",
            "pipeline-report.md",
            "render.log",
            "graphics/layer/frames/frame-000001.png",
            "graphics/layer/preview.png",
            "graphics/layer/manifest.json",
            "frames/frame-000001.png",
        ] {
            let path = render_dir.join(relative);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
            std::fs::write(path, b"partial").expect("write partial artifact");
        }

        clean_cancelled_render_attempt(
            project_dir,
            &paths,
            "render-cleanup",
            "render-attempt/late",
            &render_cancelled_error("renderedFrames.cancelled"),
        )
        .expect("cleanup succeeds");

        assert!(!render_dir.exists());
        let diagnostic =
            project_dir.join("renders/.cancelled/render-cleanup/render-attempt-late/render.log");
        assert!(diagnostic.is_file());
        assert!(std::fs::read_to_string(diagnostic)
            .expect("diagnostic")
            .contains("renderedFrames.cancelled"));
    }

    #[test]
    fn cancelled_attempt_cleanup_failure_is_returned_without_a_success_report() {
        let temp = tempfile::tempdir().expect("temp");
        let project_dir = temp.path();
        let paths = ProjectMediaRenderPaths::webm("cleanup-failure");
        let render_dir = paths.absolute_render_dir(project_dir);
        std::fs::create_dir_all(&render_dir).expect("render dir");
        std::fs::write(render_dir.join("output.webm"), b"partial").expect("partial output");
        std::fs::write(project_dir.join("renders/.cancelled"), b"blocking file")
            .expect("block diagnostics directory");

        let errors = clean_cancelled_render_attempt(
            project_dir,
            &paths,
            "cleanup-failure",
            "render-attempt/failure",
            &render_cancelled_error("renderedFrames.cancelled"),
        )
        .expect_err("cleanup obstruction must be visible");

        assert_eq!(errors[0].path, "renders.cancelled.diagnostics");
        assert!(render_dir.join("output.webm").is_file());
        assert!(!render_dir.join("pipeline-report.json").exists());
        assert!(!render_dir.join("pipeline-report.md").exists());
    }

    #[test]
    fn failure_claim_rejects_cancellation_before_failure_publication() {
        use crate::render_pipeline::cancel::{
            register_render_attempt, request_render_cancellation, RenderCancellationOutcome,
        };

        let key = RenderAttemptKey::new(
            PathBuf::from("/tmp/render-failure-publication-race"),
            "project",
            "job",
            "attempt",
        )
        .expect("key");
        let guard = register_render_attempt(key.clone()).expect("register");
        assert_eq!(claim_error_terminal(&guard.token()), JobStatus::Failed);
        assert_eq!(
            request_render_cancellation(&key),
            RenderCancellationOutcome::TooLate,
            "cancellation between classification and failure publication must not win"
        );
    }

    #[test]
    fn canonical_repeated_terminal_updates_preserve_revision_and_attempt_metadata() {
        use crate::project::model::TemporalWorkflowMetadata;
        use crate::project::split::{apply_project_action_to_split_project, save_split_project};

        for (index, status) in [
            JobStatus::Completed,
            JobStatus::Failed,
            JobStatus::Cancelled,
        ]
        .into_iter()
        .enumerate()
        {
            let temp = tempfile::tempdir().expect("temp");
            let mut project = sample_project();
            project.jobs.push(JobSummary {
                id: "render-terminal".into(),
                kind: "render_draft".into(),
                status: status.clone(),
                updated_at: "2026-09-12T00:00:00Z".into(),
                workflow: Some(TemporalWorkflowMetadata {
                    workflow_id: "render-terminal".into(),
                    workflow_type: "render".into(),
                    task_queue: "render".into(),
                    run_id: Some("render-attempt/current".into()),
                    activity_types: vec!["render".into()],
                }),
                start_request: None,
                provider_request: None,
                failure_reason: None,
                export_settings: None,
            });
            save_split_project(temp.path(), &project).expect("save");
            let before = load_split_project(temp.path()).expect("load before");
            let error = apply_project_action_to_split_project(
                temp.path(),
                ProjectAction::UpdateJobStatus {
                    job_id: "render-terminal".into(),
                    status,
                    updated_at: format!("2026-09-12T00:00:0{}Z", index + 1),
                    run_id: Some("render-attempt/current".into()),
                },
            )
            .expect_err("terminal repeat must be rejected before commit");
            assert!(error.to_string().contains("terminal"));
            assert_eq!(load_split_project(temp.path()).expect("load after"), before);
        }
    }
}

#[cfg(test)]
mod admitted_bookkeeping_tests {
    use super::*;
    use crate::project::fixtures::sample_project;
    use crate::project::split::{
        apply_project_actions_to_split_project_if_revision, save_split_project,
    };

    #[test]
    fn admitted_worker_status_preserves_edit_revision_and_completion_merges_newer_edits() {
        let root = tempfile::tempdir().unwrap();
        let initial = save_split_project(root.path(), &sample_project())
            .unwrap()
            .project;
        let job_id = "revision-render";
        let attempt_id = "render-attempt/bookkeeping";
        let mut job = crate::workflows::temporal_job_summary(
            crate::workflows::TemporalWorkflowKind::RenderDraft,
            &initial.id,
            job_id,
            JobStatus::Queued,
            "2026-10-01",
        );
        job.workflow.as_mut().unwrap().run_id = Some(attempt_id.into());
        let admitted = apply_project_actions_to_split_project(
            root.path(),
            vec![ProjectAction::RecordJob { job: Box::new(job) }],
        )
        .unwrap()
        .project;
        let lease = acquire_split_project_mutation_lease(root.path()).unwrap();
        let running = persist_media_render_actions(
            root.path(),
            vec![ProjectAction::UpdateJobStatus {
                job_id: job_id.into(),
                status: JobStatus::Running,
                updated_at: "2026-10-01".into(),
                run_id: Some(attempt_id.into()),
            }],
            &lease,
            true,
        )
        .unwrap()
        .project;
        assert_eq!(
            running.content_revision, admitted.content_revision,
            "worker status must not stale an acknowledged edit revision"
        );
        let edited = apply_project_actions_to_split_project_if_revision(
            root.path(),
            vec![ProjectAction::UpdateProjectSettings {
                name: "Edited while encoding".into(),
                render_settings: admitted.render_settings.clone(),
            }],
            &admitted.id,
            admitted.content_revision,
        )
        .unwrap()
        .project;
        assert_eq!(
            edited.content_revision,
            admitted.content_revision + 1,
            "user edits still advance the revision"
        );
        let completed = persist_media_render_actions(
            root.path(),
            vec![ProjectAction::UpdateJobStatus {
                job_id: job_id.into(),
                status: JobStatus::Completed,
                updated_at: "2026-10-01".into(),
                run_id: Some(attempt_id.into()),
            }],
            &lease,
            true,
        )
        .unwrap()
        .project;
        assert_eq!(completed.content_revision, edited.content_revision);
        assert_eq!(completed.name, edited.name);
        assert_eq!(completed.jobs[0].status, JobStatus::Completed);
        assert_eq!(load_split_project(root.path()).unwrap(), completed);
    }
    #[test]
    fn bookkeeping_rejects_content_actions_and_invalid_batches_without_writing() {
        let root = tempfile::tempdir().unwrap();
        let initial = save_split_project(root.path(), &sample_project())
            .unwrap()
            .project;
        let mut job = crate::workflows::temporal_job_summary(
            crate::workflows::TemporalWorkflowKind::RenderDraft,
            &initial.id,
            "valid-job",
            JobStatus::Queued,
            "2026-10-01",
        );
        job.workflow.as_mut().unwrap().run_id = Some("render-attempt/valid".into());
        let project = apply_project_actions_to_split_project(
            root.path(),
            vec![ProjectAction::RecordJob { job: Box::new(job) }],
        )
        .unwrap()
        .project;
        let lease = acquire_split_project_mutation_lease(root.path()).unwrap();
        let valid = ProjectAction::UpdateJobStatus {
            job_id: "valid-job".into(),
            status: JobStatus::Running,
            updated_at: "2026-10-01".into(),
            run_id: Some("render-attempt/valid".into()),
        };
        for actions in [
            vec![
                valid.clone(),
                ProjectAction::UpdateProjectSettings {
                    name: "Must not persist".into(),
                    render_settings: project.render_settings.clone(),
                },
            ],
            vec![
                valid,
                ProjectAction::UpdateJobStatus {
                    job_id: "missing-job".into(),
                    status: JobStatus::Running,
                    updated_at: "2026-10-01".into(),
                    run_id: Some("render-attempt/missing".into()),
                },
            ],
        ] {
            assert!(persist_media_render_actions(root.path(), actions, &lease, true).is_err());
            assert_eq!(load_split_project(root.path()).unwrap(), project);
        }
    }
}
