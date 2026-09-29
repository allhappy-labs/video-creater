use super::conversation::CodexConversationEditRequest;
use crate::edit::preset::{EditJobRequest, EditRequestError};
use crate::gpu_graphics::templates::project_agent_shader_background_catalog;
use crate::project::model::VideoProject;
use crate::settings::skills::{mandatory_skill_definition, mandatory_skill_prompt_bundle};
use std::fs;
use std::path::Path;
use thiserror::Error;

mod history;
mod media;
mod project_files;
mod relevance;
mod timeline;
use history::{
    export_artifacts_summary, export_artifacts_summary_for, export_capabilities_summary,
    generated_assets_summary, generated_assets_summary_for, render_reports_summary,
    render_reports_summary_for, workflow_jobs_summary, workflow_jobs_summary_for,
};
use media::{media_library_summary, media_library_summary_for, transcript_excerpt};
use project_files::{
    project_files_summary, template_overrides_summary, template_overrides_summary_for,
};
use relevance::{
    conversation_focus_summary, generated_output_count, select_newest, select_ranked,
    ConversationRelevance,
};
use timeline::{timeline_summary, timeline_summary_for};

const MAX_CONTEXT_FOLDERS: usize = 40;
const MAX_CONTEXT_MEDIA: usize = 80;
const MAX_CONTEXT_TRACKS: usize = 12;
const MAX_CONTEXT_TIMELINE_ITEMS: usize = 120;
const MAX_CONTEXT_GENERATED_ASSETS: usize = 80;
const MAX_CONTEXT_GENERATED_OUTPUTS: usize = 120;
const MAX_CONTEXT_TEMPLATE_OVERRIDES: usize = 80;
const MAX_CONTEXT_RENDER_REPORTS: usize = 40;
const MAX_CONTEXT_JOBS: usize = 40;
const MAX_CONTEXT_EXPORT_ARTIFACTS: usize = 40;
const MAX_CONTEXT_CONVERSATION_TRANSCRIPTS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSkillBundle {
    pub agents_md: String,
    pub video_pipeline: String,
    pub graphics: String,
    pub visuals: String,
    pub shader_background_catalog: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoEditContext {
    pub project_id: String,
    pub project_name: String,
    pub request: EditJobRequest,
    pub media_library_summary: String,
    pub timeline_summary: String,
    pub generated_assets_summary: String,
    pub template_overrides_summary: String,
    pub render_reports_summary: String,
    pub workflow_jobs_summary: String,
    pub export_artifacts_summary: String,
    pub export_capabilities_summary: String,
    pub project_files_summary: String,
    pub media_relative_path: String,
    pub media_duration_seconds: f64,
    pub media_width: Option<u32>,
    pub media_height: Option<u32>,
    pub media_fps: Option<f64>,
    pub transcript_excerpt: String,
}

#[derive(Debug, Error, PartialEq)]
pub enum CodexContextError {
    #[error("edit request is invalid: {0}")]
    InvalidRequest(EditRequestError),
    #[error("media was not found: {0}")]
    MediaNotFound(String),
    #[error("transcript was not found for media: {0}")]
    TranscriptNotFound(String),
    #[error("failed to read {path}: {kind}")]
    ReadSkill {
        path: String,
        kind: std::io::ErrorKind,
    },
    #[error("failed to load shader background template catalog: {0}")]
    ShaderTemplateCatalog(String),
}

pub fn load_project_skill_bundle(
    project_root: impl AsRef<Path>,
) -> Result<ProjectSkillBundle, CodexContextError> {
    let root = project_root.as_ref();
    let video_pipeline = mandatory_skill_definition("video-creater-video-pipeline")
        .expect("fixed video pipeline skill definition");
    let graphics = mandatory_skill_definition("video-creater-graphics")
        .expect("fixed graphics skill definition");
    let visuals = mandatory_skill_definition("video-creater-visuals")
        .expect("fixed visuals skill definition");
    Ok(ProjectSkillBundle {
        agents_md: read_skill(root.join("AGENTS.md"))?,
        video_pipeline: video_pipeline.bundled_content.to_string(),
        graphics: graphics.bundled_content.to_string(),
        visuals: visuals.bundled_content.to_string(),
        shader_background_catalog: project_agent_shader_background_catalog(root)
            .map_err(|error| CodexContextError::ShaderTemplateCatalog(error.to_string()))?,
    })
}

/// Mandatory edit guidance for the packaged headless host. Unlike the desktop
/// development path this does not depend on a source checkout being present.
pub fn bundled_host_skill_bundle() -> ProjectSkillBundle {
    let video_pipeline = mandatory_skill_definition("video-creater-video-pipeline")
        .expect("fixed video pipeline skill definition");
    let graphics = mandatory_skill_definition("video-creater-graphics")
        .expect("fixed graphics skill definition");
    let visuals = mandatory_skill_definition("video-creater-visuals")
        .expect("fixed visuals skill definition");
    ProjectSkillBundle {
        agents_md: include_str!("../../../AGENTS.md").to_string(),
        video_pipeline: video_pipeline.bundled_content.to_string(),
        graphics: graphics.bundled_content.to_string(),
        visuals: visuals.bundled_content.to_string(),
        shader_background_catalog:
            "No project-specific shader background overrides are installed on this host."
                .to_string(),
    }
}

pub fn build_codex_developer_instructions(skills: &ProjectSkillBundle) -> String {
    let mandatory_skills = mandatory_skill_prompt_bundle();
    format!(
        "\
{agents_md}

## Mandatory Loaded Skill Text

{mandatory_skills}

### shader-background-template-catalog
{shader_background_catalog}
",
        agents_md = skills.agents_md,
        mandatory_skills = mandatory_skills,
        shader_background_catalog = skills.shader_background_catalog,
    )
}

pub fn build_video_edit_context(
    project: &VideoProject,
    request: &EditJobRequest,
) -> Result<VideoEditContext, CodexContextError> {
    build_video_edit_context_with_project_dir_optional(project, request, None)
}

pub fn build_video_edit_context_with_project_dir(
    project: &VideoProject,
    request: &EditJobRequest,
    project_dir: &Path,
) -> Result<VideoEditContext, CodexContextError> {
    build_video_edit_context_with_project_dir_optional(project, request, Some(project_dir))
}

fn build_video_edit_context_with_project_dir_optional(
    project: &VideoProject,
    request: &EditJobRequest,
    project_dir: Option<&Path>,
) -> Result<VideoEditContext, CodexContextError> {
    request
        .validate()
        .map_err(CodexContextError::InvalidRequest)?;

    let media = project
        .media
        .iter()
        .find(|media| media.id == request.media_id)
        .ok_or_else(|| CodexContextError::MediaNotFound(request.media_id.clone()))?;
    let transcript = project
        .transcripts
        .iter()
        .find(|transcript| transcript.media_id == request.media_id)
        .ok_or_else(|| CodexContextError::TranscriptNotFound(request.media_id.clone()))?;

    Ok(VideoEditContext {
        project_id: project.id.clone(),
        project_name: project.name.clone(),
        request: request.clone(),
        media_library_summary: media_library_summary(project),
        timeline_summary: timeline_summary(project),
        generated_assets_summary: generated_assets_summary(project),
        template_overrides_summary: template_overrides_summary(project),
        render_reports_summary: render_reports_summary(project),
        workflow_jobs_summary: workflow_jobs_summary(project),
        export_artifacts_summary: export_artifacts_summary(project),
        export_capabilities_summary: export_capabilities_summary(),
        project_files_summary: project_files_summary(project, project_dir),
        media_relative_path: media.relative_path.clone(),
        media_duration_seconds: media.duration_seconds,
        media_width: media.width,
        media_height: media.height,
        media_fps: media.fps,
        transcript_excerpt: transcript_excerpt(media, &transcript.words),
    })
}

/// Model-only context for a preset-free conversation turn. None of these
/// summaries are returned to the client presentation model.
#[derive(Debug, Clone, PartialEq)]
pub struct CodexConversationContext {
    pub project_id: String,
    pub project_name: String,
    pub request: CodexConversationEditRequest,
    pub internal_focus_summary: String,
    pub media_library_summary: String,
    pub timeline_summary: String,
    pub transcript_excerpts_summary: String,
    pub generated_assets_summary: String,
    pub template_overrides_summary: String,
    pub render_reports_summary: String,
    pub workflow_jobs_summary: String,
    pub export_artifacts_summary: String,
    pub export_capabilities_summary: String,
    pub project_files_summary: String,
}

/// Builds the model-only conversation context from an already validated request.
///
/// Budget rule: every collection that fits its existing `MAX_CONTEXT_*` cap is
/// sent complete, in canonical project order. A collection over its cap is
/// ranked and only the ranked prefix is sent, followed by the usual truncation
/// marker. Ranking tiers, in order:
/// 1. objects the focus names explicitly (mentions, primary media, selected items);
/// 2. objects referenced by focused timeline items (selected, inside the focused
///    range, or sourced from focused media);
/// 3. objects referenced by the active timeline;
/// 4. everything else.
///
/// Ties keep canonical order. History collections without focus relations
/// (render reports, jobs, exports) rank newest first. Focus only reorders what
/// survives the cap, so unfocused entries still fill the remaining budget.
pub fn build_codex_conversation_context(
    project: &VideoProject,
    request: &CodexConversationEditRequest,
    project_dir: Option<&Path>,
) -> CodexConversationContext {
    let relevance = ConversationRelevance::new(project, request);
    let mut bounded = Vec::new();

    let folders = select_ranked(
        &project.media_folders,
        MAX_CONTEXT_FOLDERS,
        "folders",
        &mut bounded,
        |folder| relevance.folder_tier(folder),
    );
    let media = select_ranked(
        &project.media,
        MAX_CONTEXT_MEDIA,
        "media assets",
        &mut bounded,
        |media| relevance.media_tier(&media.id),
    );
    let timeline = relevance.timeline_selection(&mut bounded);
    let generated_assets = select_ranked(
        &project.generated_assets,
        MAX_CONTEXT_GENERATED_ASSETS,
        "generated assets",
        &mut bounded,
        |asset| relevance.generated_asset_tier(asset),
    );
    if generated_output_count(project) > MAX_CONTEXT_GENERATED_OUTPUTS
        && !bounded.contains(&"generated assets")
    {
        bounded.push("generated assets");
    }
    let templates = select_ranked(
        &project.template_overrides,
        MAX_CONTEXT_TEMPLATE_OVERRIDES,
        "template overrides",
        &mut bounded,
        |template| relevance.template_tier(&template.template_id),
    );
    let transcript_excerpts = relevance.transcript_excerpts(&mut bounded);
    let render_reports = select_newest(
        &project.render_reports,
        MAX_CONTEXT_RENDER_REPORTS,
        "render reports",
        &mut bounded,
        |report| report.created_at.as_str(),
    );
    let jobs = select_newest(
        &project.jobs,
        MAX_CONTEXT_JOBS,
        "workflow jobs",
        &mut bounded,
        |job| job.updated_at.as_str(),
    );
    let export_artifacts = select_newest(
        &project.export_artifacts,
        MAX_CONTEXT_EXPORT_ARTIFACTS,
        "export artifacts",
        &mut bounded,
        |artifact| artifact.created_at.as_str(),
    );

    CodexConversationContext {
        project_id: project.id.clone(),
        project_name: project.name.clone(),
        request: request.clone(),
        internal_focus_summary: conversation_focus_summary(request, &bounded),
        media_library_summary: media_library_summary_for(project, &folders, &media),
        timeline_summary: timeline_summary_for(project, &timeline),
        transcript_excerpts_summary: transcript_excerpts,
        generated_assets_summary: generated_assets_summary_for(project, &generated_assets),
        template_overrides_summary: template_overrides_summary_for(project, &templates),
        render_reports_summary: render_reports_summary_for(project, &render_reports),
        workflow_jobs_summary: workflow_jobs_summary_for(project, &jobs),
        export_artifacts_summary: export_artifacts_summary_for(project, &export_artifacts),
        export_capabilities_summary: export_capabilities_summary(),
        project_files_summary: project_files_summary(project, project_dir),
    }
}

fn read_skill(path: impl AsRef<Path>) -> Result<String, CodexContextError> {
    let path = path.as_ref();
    fs::read_to_string(path).map_err(|source| CodexContextError::ReadSkill {
        path: path.display().to_string(),
        kind: source.kind(),
    })
}

#[cfg(test)]
mod evidence_tests;
#[cfg(test)]
mod tests;
