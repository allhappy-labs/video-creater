//! Remote workflow entrypoints resolve catalog identities before calling shared services.
use super::*;
use crate::project::model::{JobSummary, TemporalWorkflowStartRequest};
use crate::project::mutation::acquire_split_project_mutation_lease;
use crate::project::nle_export::{
    export_project_timeline_to_nle_xml, nle_xml_export_artifact, NleXmlFormat,
};
use crate::workflows::*;
use serde::de::DeserializeOwned;

fn decode<T: DeserializeOwned>(payload: &Value, field: &str) -> Result<T, ServiceError> {
    serde_json::from_value(
        payload
            .get(field)
            .cloned()
            .ok_or_else(|| ServiceError::invalid_input(format!("{field} is required")))?,
    )
    .map_err(|_| ServiceError::invalid_input(format!("{field} is invalid")))
}
fn text<'a>(payload: &'a Value, field: &str) -> Result<&'a str, ServiceError> {
    required_string(payload, field).map_err(ServiceError::invalid_input)
}
fn value<T: serde::Serialize>(result: T) -> Result<Value, ServiceError> {
    serde_json::to_value(result).map_err(|error| ServiceError::internal(error.to_string()))
}
fn internal(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::internal(error.to_string())
}

impl HostDispatcher {
    pub(super) fn dispatch_workflow(
        &self,
        request: &RpcEnvelope,
    ) -> Option<Result<Value, ServiceError>> {
        let operation = request.operation.as_str();
        if operation.starts_with("remote_build_temporal_") {
            return Some(self.workflow_builder(request));
        }
        Some(match operation {
            "list_generation_model_catalog" => {
                crate::codex::tools::list_models_payload(None).map_err(internal)
            }
            "get_temporal_worker_environment_report" => value(temporal_worker_environment_report()),
            "list_shader_background_templates" => self.shader_templates(request),
            "search_project_media" => self.search_media(request),
            "export_nle_xml_to_split_project_folder" => self.export_xml(request),
            "export_palmier_project_package_to_split_project_folder" => self.export_bundle(request),
            "run_generate_media_in_process" => self.run_generation(request),
            "remote_start_temporal_workflow" => self.start_workflow(request),
            "reconcile_temporal_jobs_in_split_project_folder" => self.reconcile_workflows(request),
            _ => return None,
        })
    }

    pub(super) fn checked_project(
        &self,
        request: &RpcEnvelope,
        write: bool,
    ) -> Result<(std::path::PathBuf, VideoProject), ServiceError> {
        let path = self
            .resolve_project(request)
            .map_err(|_| ServiceError::not_found("project"))?;
        let context =
            self.project_request_context(request, &path, AuthorizationScope::ProjectRead)?;
        let project =
            ProjectService::new(std::sync::Arc::new(NoopEventSink)).load(&context, &path)?;
        if write {
            let expected = request
                .expected_revision
                .ok_or_else(|| ServiceError::invalid_input("expected revision is required"))?;
            if expected != project.content_revision {
                return Err(ServiceError::revision_conflict(
                    expected,
                    project.content_revision,
                ));
            }
        }
        Ok((path, project))
    }

    fn workflow_builder(&self, request: &RpcEnvelope) -> Result<Value, ServiceError> {
        let (_path, project) = self.checked_project(request, false)?;
        let payload = &request.payload;
        let locator = request
            .project_id
            .as_deref()
            .ok_or_else(|| ServiceError::invalid_input("project ID is required"))?;
        let operation = request.operation.as_str();
        if operation == "remote_build_temporal_job_summary" {
            let kind = match text(payload, "kind")? {
                "generate_media" => TemporalWorkflowKind::GenerateMedia,
                "transcribe_media" => TemporalWorkflowKind::TranscribeMedia,
                "export_media" => TemporalWorkflowKind::ExportMedia,
                "export_nle_xml" => TemporalWorkflowKind::ExportNleXml,
                "render_draft" => TemporalWorkflowKind::RenderDraft,
                "codex_edit" => TemporalWorkflowKind::CodexEdit,
                _ => return Err(ServiceError::invalid_input("workflow kind is invalid")),
            };
            return value(temporal_job_summary(
                kind,
                &project.id,
                text(payload, "jobId")?,
                decode(payload, "status")?,
                text(payload, "updatedAt")?,
            ));
        }
        if matches!(
            operation,
            "remote_build_temporal_start_result_action"
                | "remote_build_temporal_generate_media_failure_actions"
        ) {
            let supplied: JobSummary = decode(payload, "job")?;
            let job = project
                .jobs
                .iter()
                .find(|job| job.id == supplied.id)
                .ok_or_else(|| ServiceError::not_found("job"))?;
            if operation == "remote_build_temporal_start_result_action" {
                // A synchronous operation may have completed while the client waited for admission.
                if matches!(
                    job.status,
                    JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
                ) {
                    return value(ProjectAction::UpdateJobStatus {
                        job_id: job.id.clone(),
                        status: job.status.clone(),
                        updated_at: job.updated_at.clone(),
                        run_id: job
                            .workflow
                            .as_ref()
                            .and_then(|workflow| workflow.run_id.clone()),
                    });
                }
                return value(
                    temporal_start_result_action(
                        job,
                        text(payload, "runId")?,
                        text(payload, "updatedAt")?,
                    )
                    .map_err(internal)?,
                );
            }
            return value(
                temporal_generate_media_failure_actions(
                    job,
                    text(payload, "assetId")?,
                    payload.get("runId").and_then(Value::as_str),
                    text(payload, "updatedAt")?,
                )
                .map_err(internal)?,
            );
        }
        let job_id = text(payload, "jobId")?;
        match operation {
            "remote_build_temporal_transcribe_media_start_request" => {
                let media_id = text(payload, "mediaId")?;
                if !project.media.iter().any(|media| media.id == media_id) {
                    return Err(ServiceError::not_found("media"));
                }
                let language = text(payload, "languageMode")?;
                if language.len() > 64 {
                    return Err(ServiceError::invalid_input("language is invalid"));
                }
                value(temporal_transcribe_media_start_request(
                    &project.id,
                    locator,
                    media_id,
                    job_id,
                    language,
                ))
            }
            "remote_build_temporal_generate_media_start_request" => {
                let brief: TemporalGenerateMediaBrief = serde_json::from_value(payload.clone())
                    .map_err(|_| ServiceError::invalid_input("generation brief is invalid"))?;
                value(temporal_generate_media_start_request(
                    &project.id,
                    locator,
                    text(payload, "assetId")?,
                    job_id,
                    payload
                        .get("mockMode")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    Some(brief),
                ))
            }
            "remote_build_temporal_export_media_start_request" => {
                let options = ExportRenderOptions::new(
                    decode(payload, "profile")?,
                    decode(payload, "quality")?,
                    decode(payload, "width")?,
                    decode(payload, "height")?,
                )
                .map_err(internal)?
                .with_fps(payload.get("fps").and_then(Value::as_f64))
                .map_err(internal)?
                .with_encode_tier(
                    payload
                        .get("encodeTier")
                        .filter(|v| !v.is_null())
                        .map(|v| serde_json::from_value(v.clone()))
                        .transpose()
                        .map_err(internal)?
                        .unwrap_or_default(),
                )
                .map_err(internal)?;
                let output: Option<ExportOutputRequest> = payload
                    .get("output")
                    .filter(|v| !v.is_null())
                    .map(|v| serde_json::from_value(v.clone()))
                    .transpose()
                    .map_err(internal)?;
                checked_destination(output.as_ref())?;
                value(temporal_export_media_start_request_with_output(
                    &project.id,
                    locator,
                    job_id,
                    options,
                    checked_export_path(payload)?,
                    output.as_ref(),
                ))
            }
            "remote_build_temporal_export_project_bundle_start_request" => {
                value(temporal_export_project_bundle_start_request(
                    &project.id,
                    locator,
                    job_id,
                    checked_export_path(payload)?,
                    true,
                ))
            }
            "remote_build_temporal_export_nle_xml_start_request" => {
                value(temporal_export_nle_xml_start_request(
                    &project.id,
                    locator,
                    job_id,
                    decode(payload, "format")?,
                    checked_export_path(payload)?,
                ))
            }
            "remote_build_temporal_codex_edit_start_request" => {
                value(temporal_codex_edit_start_request(
                    &project.id,
                    locator,
                    locator,
                    job_id,
                    decode(payload, "request")?,
                ))
            }
            _ => Err(ServiceError::not_found("workflow builder")),
        }
    }

    fn search_media(&self, request: &RpcEnvelope) -> Result<Value, ServiceError> {
        let (path, project) = self.checked_project(request, false)?;
        let scope = crate::search::SearchScope::parse(
            request
                .payload
                .get("scope")
                .and_then(Value::as_str)
                .unwrap_or("both"),
        )
        .map_err(internal)?;
        let limit = request
            .payload
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(20)
            .min(100) as usize;
        crate::search::query_project_search_for_project_dir(
            &path,
            &project,
            crate::search::ProjectSearchQuery {
                query: text(&request.payload, "query")?.into(),
                limit,
                scope,
                media_id: request
                    .payload
                    .get("mediaId")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            },
        )
        .map_err(internal)
    }

    fn shader_templates(&self, request: &RpcEnvelope) -> Result<Value, ServiceError> {
        use crate::gpu_graphics::templates::{
            builtin_shader_background_templates, project_shader_background_templates,
        };
        let project_path = request
            .project_id
            .as_ref()
            .map(|_| self.resolve_project(request).map_err(internal))
            .transpose()?;
        let templates = match request.project_id.as_ref() {
            Some(_) => {
                let (path, _) = self.checked_project(request, false)?;
                project_shader_background_templates(&path).map_err(internal)?
            }
            None => builtin_shader_background_templates().map_err(internal)?,
        };
        value(templates.into_iter().map(|template| {
            let config_path = remote_template_path(&template.config_path, project_path.as_deref())?;
            let shader_path = remote_template_path(&template.shader_path, project_path.as_deref())?;
            let config = template.config;
            Ok(json!({"id":config.id,"version":config.version,"name":config.title,"sourceKind":config.source_kind,"category":config.category,"durationSeconds":config.default_duration_seconds,"shaderProfileId":config.shader_profile_id,"configPath":config_path,"shaderPath":shader_path,"utilityRefs":config.utility_refs,"sourceUrl":config.source_url,"license":config.license,"placement":config.placement,"renderContract":config.render_contract,"preview":config.preview,"visualTreatment":config.visual_treatment,"motion":config.motion,"safeZone":config.safe_zone,"avoid":config.avoid,"agentSummary":config.agent_summary}))
        }).collect::<Result<Vec<_>, ServiceError>>()?)
    }

    fn export_xml(&self, request: &RpcEnvelope) -> Result<Value, ServiceError> {
        let path = self.resolve_project(request).map_err(internal)?;
        let lease = acquire_split_project_mutation_lease(&path).map_err(internal)?;
        let (_, canonical) = self.checked_project(request, true)?;
        let payload = &request.payload;
        let project = match payload.get("timelineId").and_then(Value::as_str) {
            Some(id) => canonical
                .projected_for_timeline(id)
                .ok_or_else(|| ServiceError::not_found("timeline"))?,
            None => canonical.clone(),
        };
        let format: NleXmlFormat = decode(payload, "format")?;
        let job_id = text(payload, "jobId")?;
        let updated_at = text(payload, "updatedAt")?;
        let mut export = export_project_timeline_to_nle_xml(&project, format).map_err(internal)?;
        export.filename = format!("{}-{}", job_id, export.filename);
        let artifact = nle_xml_export_artifact(&export, format, job_id, updated_at);
        let job = temporal_job_summary(
            TemporalWorkflowKind::ExportNleXml,
            &canonical.id,
            job_id,
            JobStatus::Queued,
            updated_at,
        );
        let actions = vec![
            ProjectAction::RecordJob { job: Box::new(job) },
            ProjectAction::UpdateJobStatus {
                job_id: job_id.into(),
                status: JobStatus::Completed,
                updated_at: updated_at.into(),
                run_id: None,
            },
            ProjectAction::RecordExportArtifact { artifact },
        ];
        let mut validated = canonical.clone();
        for action in &actions {
            crate::project::action::apply_project_action(&mut validated, action.clone())
                .map_err(|error| ServiceError::invalid_input(error.to_string()))?;
        }
        // Publish a unique artifact only after every project action validates. A failed
        // project commit removes this request's artifact without touching existing files.
        let directory = path.join("exports");
        std::fs::create_dir_all(&directory).map_err(internal)?;
        if directory.canonicalize().map_err(internal)? != directory {
            return Err(ServiceError::invalid_input(
                "export directory must stay inside the project",
            ));
        }
        let output = directory.join(&export.filename);
        let mut staged = tempfile::NamedTempFile::new_in(&directory).map_err(internal)?;
        use std::io::Write;
        staged.write_all(export.xml.as_bytes()).map_err(internal)?;
        staged.as_file().sync_all().map_err(internal)?;
        staged.persist_noclobber(&output).map_err(internal)?;
        let write = match crate::project::split::apply_project_actions_to_split_project_with_lease(
            &path, actions, &lease,
        ) {
            Ok(write) => write,
            Err(error) => {
                let _ = std::fs::remove_file(&output);
                return Err(internal(error));
            }
        };
        let job = write
            .project
            .jobs
            .iter()
            .find(|job| job.id == job_id)
            .cloned();
        value(
            json!({"project": write.project, "exportPath": format!("exports/{}",export.filename), "job": job}),
        )
    }

    fn export_bundle(&self, request: &RpcEnvelope) -> Result<Value, ServiceError> {
        let (path, project) = self.checked_project(request, true)?;
        let payload = &request.payload;
        let job_id = text(payload, "jobId")?;
        let updated_at = text(payload, "updatedAt")?;
        let output = checked_export_path(payload)?;
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::ExportMedia,
            &project.id,
            job_id,
            JobStatus::Running,
            updated_at,
        );
        let start = temporal_export_project_bundle_start_request(
            &project.id,
            request
                .project_id
                .as_deref()
                .ok_or_else(|| ServiceError::invalid_input("project ID is required"))?,
            job_id,
            output,
            true,
        );
        job.workflow
            .as_mut()
            .expect("canonical workflow metadata")
            .activity_types = start.activity_types.clone();
        job.start_request = Some(start);
        let context =
            self.project_request_context(request, &path, AuthorizationScope::ProjectWrite)?;
        ProjectService::new(std::sync::Arc::new(NoopEventSink)).apply_actions(
            &context,
            &path,
            vec![ProjectAction::RecordJob { job: Box::new(job) }],
        )?;
        let result = temporal_export_project_bundle_write_activity_value(
            json!({"projectId": project.id,"projectDir":path,"jobId":job_id,"profile":"palmierProject","outputPath":output,"createdAt":updated_at}),
        );
        if let Err(error) = result {
            let _ = apply_project_actions_to_split_project(
                &path,
                vec![ProjectAction::UpdateJobStatus {
                    job_id: job_id.into(),
                    status: JobStatus::Failed,
                    updated_at: updated_at.into(),
                    run_id: None,
                }],
            );
            return Err(internal(error));
        }
        let saved = load_split_project(&path).map_err(internal)?;
        let job = saved.jobs.iter().find(|job| job.id == job_id).cloned();
        value(json!({"project":saved,"exportPath":output,"job":job}))
    }

    fn run_generation(&self, request: &RpcEnvelope) -> Result<Value, ServiceError> {
        let (path, project) = self.checked_project(request, true)?;
        let supplied: TemporalWorkflowStartRequest = decode(&request.payload, "startRequest")?;
        let input = temporal_generate_media_workflow_input(&supplied).map_err(internal)?;
        if input.project_id != project.id {
            return Err(ServiceError::invalid_input(
                "generation project does not match",
            ));
        }
        let job = project
            .jobs
            .iter()
            .find(|job| job.id == input.job_id)
            .ok_or_else(|| ServiceError::not_found("job"))?;
        let asset = project
            .generated_assets
            .iter()
            .find(|asset| asset.id == input.asset_id)
            .ok_or_else(|| ServiceError::not_found("generated asset"))?;
        let mut start = job.start_request.clone().ok_or_else(|| {
            ServiceError::invalid_input("recorded generation request is required")
        })?;
        if start != supplied {
            return Err(ServiceError::invalid_input(
                "generation request differs from the recorded job",
            ));
        }
        if matches!(
            job.status,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
        ) || asset.status == GeneratedAssetStatus::Cancelled
        {
            return value(project);
        }
        start.input["projectDir"] = json!(path);
        let updated_at = text(&request.payload, "updatedAt")?;
        let run_id = format!("in-process/{}", start.workflow_id);
        let guard =
            crate::generation::cancel::register_generation_cancellation(&project.id, &input.job_id)
                .map_err(internal)?;
        let cancellation = guard.token();
        let watched_path = path.clone();
        let watched_id = input.job_id.clone();
        let _watch = crate::generation::orphan_watch::watch_generation_job(
            &project.id,
            &input.job_id,
            std::time::Duration::from_secs(5),
            move || {
                load_split_project(&watched_path)
                    .map(|project| project.jobs.iter().any(|job| job.id == watched_id))
                    .map_err(|error| error.to_string())
            },
        );
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(internal)?;
        let context =
            self.project_request_context(request, &path, AuthorizationScope::ProjectWrite)?;
        ProjectService::new(std::sync::Arc::new(NoopEventSink)).apply_actions(
            &context,
            &path,
            vec![
                temporal_start_result_action(job, &run_id, updated_at).map_err(internal)?,
                ProjectAction::UpdateGeneratedAssetStatus {
                    asset_id: input.asset_id.clone(),
                    status: GeneratedAssetStatus::Running,
                },
            ],
        )?;
        let credential = if input.mock_mode {
            String::new()
        } else {
            match resolve_provider_credential(&asset.model.provider) {
                Ok(credential) => credential.into_secret(),
                Err(error) => {
                    let _ = temporal_generate_media_attach_generated_asset_failure_to_project_dir(
                        &start,
                        Some(&run_id),
                        updated_at,
                    );
                    return Err(internal(error));
                }
            }
        };
        let result = run_generate_media_in_process_with_client_and_credential_cancellable(
            &client,
            &start,
            updated_at,
            Some(&run_id),
            TemporalGenerateMediaProviderRunOptions::default(),
            &credential,
            Some(&cancellation),
        );
        if let Err(error) = result {
            if !matches!(error, TemporalWorkflowInputError::GenerationCancelled) {
                return Err(internal(error));
            }
        }
        value(load_split_project(&path).map_err(internal)?)
    }

    fn start_workflow(&self, request: &RpcEnvelope) -> Result<Value, ServiceError> {
        let (path, project) = self.checked_project(request, true)?;
        let supplied: JobSummary = decode(&request.payload, "job")?;
        let mut job = project
            .jobs
            .iter()
            .find(|job| job.id == supplied.id)
            .cloned()
            .ok_or_else(|| ServiceError::not_found("job"))?;
        if job.start_request != supplied.start_request || job.kind != supplied.kind {
            return Err(ServiceError::invalid_input(
                "workflow differs from the recorded job",
            ));
        }
        if matches!(
            job.status,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
        ) {
            return Err(ServiceError::invalid_input(
                "finished jobs cannot be started again",
            ));
        }
        temporal_workflow_client_start_plan(&job)
            .map_err(|error| ServiceError::invalid_input(error.to_string()))?;
        let start = job
            .start_request
            .as_mut()
            .ok_or_else(|| ServiceError::invalid_input("workflow start request is required"))?;
        if start.input.get("projectId").and_then(Value::as_str) != Some(project.id.as_str()) {
            return Err(ServiceError::invalid_input(
                "workflow project does not match",
            ));
        }
        if start.input.get("jobId").and_then(Value::as_str) != Some(job.id.as_str()) {
            return Err(ServiceError::invalid_input("workflow job does not match"));
        }
        let kind = [
            TemporalWorkflowKind::GenerateMedia,
            TemporalWorkflowKind::TranscribeMedia,
            TemporalWorkflowKind::ExportMedia,
            TemporalWorkflowKind::ExportNleXml,
            TemporalWorkflowKind::RenderDraft,
            TemporalWorkflowKind::CodexEdit,
        ]
        .into_iter()
        .find(|kind| kind.job_kind() == job.kind)
        .ok_or_else(|| ServiceError::invalid_input("workflow kind is unsupported"))?;
        let spec = temporal_workflow_spec(kind, &project.id, &job.id);
        let activity_types = if kind == TemporalWorkflowKind::ExportMedia {
            let profile = decode(&start.input, "profile")?;
            export_media_activity_types(profile)
                .iter()
                .map(|activity| (*activity).to_string())
                .collect()
        } else {
            spec.activity_types
        };
        if start.workflow_type != spec.workflow_type
            || start.workflow_id != spec.workflow_id
            || start.task_queue != spec.task_queue
            || start.activity_types != activity_types
            || start
                .search_attributes
                .get("projectId")
                .and_then(Value::as_str)
                != Some(project.id.as_str())
            || start.search_attributes.get("jobId").and_then(Value::as_str) != Some(job.id.as_str())
            || start
                .search_attributes
                .get("workflowKind")
                .and_then(Value::as_str)
                != Some(kind.job_kind())
        {
            return Err(ServiceError::invalid_input(
                "workflow does not match the canonical project job specification",
            ));
        }
        if start.workflow_type == TemporalWorkflowKind::ExportMedia.workflow_type()
            || start.workflow_type == TemporalWorkflowKind::ExportNleXml.workflow_type()
        {
            checked_export_path(&start.input)?;
            let destination: Option<ExportOutputRequest> = start
                .input
                .get("destination")
                .filter(|value| !value.is_null())
                .map(|value| serde_json::from_value(value.clone()))
                .transpose()
                .map_err(|error| ServiceError::invalid_input(error.to_string()))?;
            checked_destination(destination.as_ref())?;
        }
        if start.workflow_type == TemporalWorkflowKind::TranscribeMedia.workflow_type()
            && !project.media.iter().any(|media| {
                Some(media.id.as_str()) == start.input.get("mediaId").and_then(Value::as_str)
            })
        {
            return Err(ServiceError::not_found("media"));
        }
        start.input["projectDir"] = json!(path);
        if start.input.get("projectRoot").is_some() {
            start.input["projectRoot"] = json!(path);
        }
        start_temporal(&job)
    }
}

fn remote_template_path(
    path: &str,
    project: Option<&std::path::Path>,
) -> Result<String, ServiceError> {
    let path = std::path::Path::new(path);
    if !path.is_absolute() {
        return Ok(path.to_string_lossy().into_owned());
    }
    let root = project
        .ok_or_else(|| ServiceError::invalid_input("template requires a project"))?
        .canonicalize()
        .map_err(internal)?;
    let path = path.canonicalize().map_err(internal)?;
    let relative = path
        .strip_prefix(root)
        .map_err(|_| ServiceError::invalid_input("template must stay inside the project"))?;
    Ok(relative.to_string_lossy().into_owned())
}

fn checked_destination(output: Option<&ExportOutputRequest>) -> Result<(), ServiceError> {
    if output.is_some_and(|output| output.directory.is_some()) {
        return Err(ServiceError::invalid_input(
            "remote exports must use the project exports folder",
        ));
    }
    if let Some(output) = output {
        crate::project::export_destination::validate_export_output_request(output, "mp4")
            .map_err(|error| ServiceError::invalid_input(error.to_string()))?;
    }
    Ok(())
}

fn checked_export_path(payload: &Value) -> Result<&str, ServiceError> {
    let output = text(payload, "outputPath")?;
    let mut parts = std::path::Path::new(output).components();
    if parts.next()
        != Some(std::path::Component::Normal(std::ffi::OsStr::new(
            "exports",
        )))
        || parts.clone().count() == 0
        || !parts.all(|part| matches!(part, std::path::Component::Normal(_)))
    {
        return Err(ServiceError::invalid_input(
            "export output must stay under the project exports folder",
        ));
    }
    Ok(output)
}

#[cfg(feature = "temporal-worker")]
fn start_temporal(job: &JobSummary) -> Result<Value, ServiceError> {
    // RPC dispatch runs in the host's blocking pool, separate from its async HTTP runtime.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(internal)?;
    runtime.block_on(async {
        let (connection_options, client_options) =
            temporalio_client::ClientOptions::load_from_config(
                temporalio_client::envconfig::LoadClientConfigProfileOptions::default(),
            )
            .map_err(internal)?;
        let connection = temporalio_client::Connection::connect(connection_options)
            .await
            .map_err(internal)?;
        let client =
            temporalio_client::Client::new(connection, client_options).map_err(internal)?;
        value(
            temporal_start_workflow_with_client(&client, job)
                .await
                .map_err(internal)?,
        )
    })
}
#[cfg(not(feature = "temporal-worker"))]
fn start_temporal(job: &JobSummary) -> Result<Value, ServiceError> {
    value(temporal_workflow_unavailable_start_result(job).map_err(internal)?)
}
