use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use video_creater_lib::generation::fal::{
    build_fal_queue_submission, fetch_fal_queue_status_with_client,
    submit_fal_queue_submission_with_client, FalQueueCancelStatus, FalQueueStatusKind,
    FalQueueSubmitError, FAL_FLUX_SCHNELL_MODEL_ID, FAL_PROVIDER, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID,
};
use video_creater_lib::generation::replicate::{
    build_replicate_prediction_submission, fetch_replicate_prediction_with_client,
    submit_replicate_prediction_submission_with_client, ReplicatePredictionStatusKind,
    REPLICATE_FLUX_SCHNELL_MODEL_ID, REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_FAST_MODEL_ID,
};
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetReferences, GeneratedAssetSettings, GeneratedAssetStatus,
    GenerationModel, JobProviderRequest, JobStatus, MediaKind, VideoProject,
};
use video_creater_lib::project::split::{
    apply_project_actions_to_split_project, load_split_project, save_split_project,
};
use video_creater_lib::provider_credentials::{
    resolve_provider_credential, ProviderCredentialSource,
};
use video_creater_lib::workflows::{
    temporal_generate_media_cancel_provider_activity_with_client, temporal_job_summary,
    TemporalWorkflowKind,
};

const LIVE_SPEND_ENV_VAR: &str = "VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND";
const PROJECT_ID: &str = "provider-cancel-e2e-project";
const ASSET_ID: &str = "provider-cancel-e2e-generated";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Config {
    provider: String,
    model: String,
    prompt: String,
    out_dir: PathBuf,
    max_status_polls: usize,
    poll_interval_ms: u64,
    continue_existing: bool,
}

#[derive(Debug, Clone)]
struct SubmittedRequest {
    request_id: String,
    status_url: String,
    response_url: String,
    cancel_url: String,
    initial_status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CredentialEvidence {
    configured: bool,
    source: &'static str,
}

fn main() {
    match parse_args(std::env::args().skip(1)).and_then(run) {
        Ok(path) => println!("{}", path.display()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn parse_args<I>(args: I) -> Result<Config, String>
where
    I: IntoIterator<Item = String>,
{
    let mut provider = FAL_PROVIDER.to_string();
    let mut model = FAL_FLUX_SCHNELL_MODEL_ID.to_string();
    let mut prompt = "a tiny blue product cube on a white background".to_string();
    let mut out_dir = PathBuf::from("output/provider-app-live-e2e/fal-cancellation");
    let mut max_status_polls = 30usize;
    let mut poll_interval_ms = 500u64;
    let mut continue_existing = false;
    let mut args = args.into_iter().filter(|arg| arg != "--");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--provider" => provider = require_value(&arg, args.next())?,
            "--model" => model = require_value(&arg, args.next())?,
            "--prompt" => prompt = require_value(&arg, args.next())?,
            "--out-dir" => out_dir = PathBuf::from(require_value(&arg, args.next())?),
            "--max-status-polls" => {
                max_status_polls = positive_usize(&arg, &require_value(&arg, args.next())?)?
            }
            "--poll-interval-ms" => {
                poll_interval_ms = positive_u64(&arg, &require_value(&arg, args.next())?)?
            }
            "--continue-existing" => {
                continue_existing = parse_bool(&arg, &require_value(&arg, args.next())?)?
            }
            "--live" => {}
            "--help" | "-h" => {
                println!(
                    "Usage: video-creater-provider-cancel-e2e --live --provider fal.ai|replicate \
                     --model MODEL --out-dir OUTPUT_DIR"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    if !matches!(provider.as_str(), FAL_PROVIDER | REPLICATE_PROVIDER) {
        return Err(format!("unsupported cancellation provider: {provider}"));
    }
    if provider == FAL_PROVIDER
        && !matches!(
            model.as_str(),
            FAL_FLUX_SCHNELL_MODEL_ID | FAL_WAN_TEXT_TO_VIDEO_MODEL_ID
        )
    {
        return Err(format!(
            "fal cancellation proof requires {FAL_FLUX_SCHNELL_MODEL_ID} or {FAL_WAN_TEXT_TO_VIDEO_MODEL_ID}"
        ));
    }
    if provider == REPLICATE_PROVIDER
        && !matches!(
            model.as_str(),
            REPLICATE_FLUX_SCHNELL_MODEL_ID | REPLICATE_SEEDANCE_20_FAST_MODEL_ID
        )
    {
        return Err(format!(
            "Replicate cancellation proof requires {REPLICATE_FLUX_SCHNELL_MODEL_ID} or {REPLICATE_SEEDANCE_20_FAST_MODEL_ID}"
        ));
    }
    if prompt.trim().is_empty() {
        return Err("prompt must be non-empty".to_string());
    }
    if std::env::var(LIVE_SPEND_ENV_VAR).ok().as_deref() != Some("1") {
        return Err(format!(
            "live provider cancellation E2E requires explicit spend opt-in: {LIVE_SPEND_ENV_VAR}=1"
        ));
    }
    Ok(Config {
        provider,
        model,
        prompt,
        out_dir,
        max_status_polls,
        poll_interval_ms,
        continue_existing,
    })
}

fn require_value(flag: &str, value: Option<String>) -> Result<String, String> {
    value
        .filter(|value| !value.trim().is_empty() && !value.starts_with("--"))
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn positive_usize(flag: &str, value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{flag} must be a positive integer"))
}

fn positive_u64(flag: &str, value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{flag} must be a positive integer"))
}

fn parse_bool(flag: &str, value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("{flag} must be true or false")),
    }
}

fn run(config: Config) -> Result<PathBuf, String> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "Cargo manifest has no repository parent".to_string())?;
    let out_dir = absolute_from(repo_root, &config.out_dir);
    if out_dir.exists() && !config.continue_existing {
        fs::remove_dir_all(&out_dir).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&out_dir).map_err(|error| error.to_string())?;
    let project_dir = out_dir.join("project");
    let now = Utc::now().to_rfc3339();
    let credential = resolve_provider_credential(&config.provider).map_err(|error| {
        format!(
            "{} credential is not configured through the app credential resolver: {error}",
            config.provider
        )
    })?;
    let credential_source = credential_source_label(credential.source());
    let asset = if config.continue_existing {
        load_split_project(&project_dir)
            .map_err(|error| format!("cannot continue cancellation evidence: {error}"))?
            .generated_assets
            .into_iter()
            .find(|asset| asset.id == ASSET_ID)
            .ok_or_else(|| "existing cancellation evidence is missing its asset".to_string())?
    } else {
        let asset = cancellation_asset(&config, &now);
        let mut project = VideoProject::new_empty(
            PROJECT_ID.to_string(),
            "Provider cancellation E2E".to_string(),
            now.clone(),
        );
        project.generated_assets.push(asset.clone());
        project.jobs.push(temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            PROJECT_ID,
            ASSET_ID,
            JobStatus::Running,
            &now,
        ));
        save_split_project(&project_dir, &project).map_err(|error| error.to_string())?;
        asset
    };

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| error.to_string())?;
    let (submitted, provider_request) = if config.continue_existing {
        let request = load_split_project(&project_dir)
            .map_err(|error| error.to_string())?
            .jobs
            .into_iter()
            .find(|job| job.id == ASSET_ID)
            .and_then(|job| job.provider_request)
            .ok_or_else(|| {
                "existing cancellation evidence is missing provider request metadata".to_string()
            })?;
        (
            SubmittedRequest {
                request_id: request.request_id.clone(),
                status_url: request.status_url.clone(),
                response_url: request.response_url.clone(),
                cancel_url: request.cancel_url.clone(),
                initial_status: "queued",
            },
            request,
        )
    } else {
        let submitted = submit(&client, &config, &asset, credential.secret())?;
        let request = JobProviderRequest {
            provider: config.provider.clone(),
            request_id: submitted.request_id.clone(),
            status_url: submitted.status_url.clone(),
            response_url: submitted.response_url.clone(),
            cancel_url: submitted.cancel_url.clone(),
            submitted_at: now.clone(),
        };
        apply_project_actions_to_split_project(
            &project_dir,
            vec![ProjectAction::UpdateJobProviderRequest {
                job_id: ASSET_ID.to_string(),
                provider_request: request.clone(),
            }],
        )
        .map_err(|error| error.to_string())?;
        (submitted, request)
    };

    let cancelled = temporal_generate_media_cancel_provider_activity_with_client(
        &client,
        &submitted.cancel_url,
        credential.secret(),
    )
    .map_err(|error| format!("production provider cancellation failed: {error}"))?;
    let (terminal_observation, status_polls) =
        observe_terminal_cancellation(&client, &config, &submitted, credential.secret())?;
    let cancelled_at = Utc::now().to_rfc3339();
    apply_project_actions_to_split_project(
        &project_dir,
        vec![
            ProjectAction::UpdateJobStatus {
                job_id: ASSET_ID.to_string(),
                status: JobStatus::Cancelled,
                updated_at: cancelled_at,
                run_id: None,
            },
            ProjectAction::UpdateGeneratedAssetStatus {
                asset_id: ASSET_ID.to_string(),
                status: GeneratedAssetStatus::Cancelled,
            },
        ],
    )
    .map_err(|error| error.to_string())?;

    let reloaded = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let persisted_asset = reloaded
        .generated_assets
        .iter()
        .find(|asset| asset.id == ASSET_ID)
        .ok_or_else(|| "cancelled generated asset was not persisted".to_string())?;
    let persisted_job = reloaded
        .jobs
        .iter()
        .find(|job| job.id == ASSET_ID)
        .ok_or_else(|| "cancelled generation job was not persisted".to_string())?;
    let request_id_preserved = persisted_job
        .provider_request
        .as_ref()
        .is_some_and(|request| request == &provider_request);
    let completed_output_absent = persisted_asset.outputs.is_empty()
        && reloaded.media.iter().all(|media| {
            !matches!(media.kind, MediaKind::Generated)
                && !media.relative_path.starts_with("generated/")
        });
    if persisted_asset.status != GeneratedAssetStatus::Cancelled
        || persisted_job.status != JobStatus::Cancelled
        || !request_id_preserved
        || !completed_output_absent
    {
        return Err("canonical project did not durably retain a clean cancelled state".to_string());
    }

    let report = json!({
        "schemaVersion": 3,
        "status": "passed",
        "providerMode": "live-cancellation",
        "provider": config.provider,
        "model": config.model,
        "scenario": "provider-cancellation",
        "credential": CredentialEvidence {
            configured: true,
            source: credential_source,
        },
        "providerRequest": provider_request,
        "lifecycle": {
            "observed": [submitted.initial_status, "cancellation-requested", terminal_observation],
            "providerCancelStatus": cancel_status_label(cancelled.status),
            "terminalObservation": terminal_observation,
            "statusPolls": status_polls,
            "requestIdPreserved": request_id_preserved,
            "providerSide": true
        },
        "canonical": {
            "projectDir": project_dir,
            "generatedAssetStatus": "cancelled",
            "jobStatus": "cancelled",
            "outputCount": persisted_asset.outputs.len(),
            "generatedMediaCount": reloaded.media.iter().filter(|media| matches!(media.kind, MediaKind::Generated)).count(),
            "completedOutputAbsent": completed_output_absent
        },
        "artifacts": ["project/video-creater.project.json", "project/jobs/index.json", "report.json"]
    });
    let serialized = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?;
    if serialized.contains(credential.secret()) {
        return Err(
            "provider cancellation report attempted to serialize credential material".to_string(),
        );
    }
    let report_path = out_dir.join("report.json");
    fs::write(&report_path, serialized).map_err(|error| error.to_string())?;
    Ok(report_path)
}

fn cancellation_asset(config: &Config, now: &str) -> GeneratedAsset {
    let video_model = matches!(
        config.model.as_str(),
        FAL_WAN_TEXT_TO_VIDEO_MODEL_ID | REPLICATE_SEEDANCE_20_FAST_MODEL_ID
    );
    GeneratedAsset {
        schema_version: 1,
        id: ASSET_ID.to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Running,
        name: Some("Provider cancellation E2E still".to_string()),
        target_folder_id: None,
        placement_intent: None,
        prompt: config.prompt.clone(),
        model: GenerationModel {
            provider: config.provider.clone(),
            id: config.model.clone(),
        },
        references: GeneratedAssetReferences::default(),
        settings: GeneratedAssetSettings {
            width: Some(if video_model { 1280 } else { 512 }),
            height: Some(if video_model { 720 } else { 512 }),
            duration_seconds: video_model.then_some(5.0),
            aspect_ratio: video_model.then(|| "16:9".to_string()),
            resolution: video_model.then(|| "720p".to_string()),
            num_images: Some(1),
            generate_audio: video_model.then_some(false),
            ..GeneratedAssetSettings::default()
        },
        outputs: Vec::new(),
        created_at: now.to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    }
}

fn submit(
    client: &reqwest::blocking::Client,
    config: &Config,
    asset: &GeneratedAsset,
    credential: &str,
) -> Result<SubmittedRequest, String> {
    match config.provider.as_str() {
        FAL_PROVIDER => {
            let submission =
                build_fal_queue_submission(asset).map_err(|error| error.to_string())?;
            let response = submit_fal_queue_submission_with_client(client, &submission, credential)
                .map_err(|error| error.to_string())?;
            Ok(SubmittedRequest {
                request_id: response.request_id,
                status_url: response.status_url,
                response_url: response.response_url,
                cancel_url: response.cancel_url,
                initial_status: "queued",
            })
        }
        REPLICATE_PROVIDER => {
            let submission =
                build_replicate_prediction_submission(asset).map_err(|error| error.to_string())?;
            let response =
                submit_replicate_prediction_submission_with_client(client, &submission, credential)
                    .map_err(|error| error.to_string())?;
            Ok(SubmittedRequest {
                request_id: response.id,
                status_url: response.urls.get.clone(),
                response_url: response.urls.get,
                cancel_url: response.urls.cancel,
                initial_status: replicate_status_label(response.status),
            })
        }
        _ => unreachable!(),
    }
}

fn observe_terminal_cancellation(
    client: &reqwest::blocking::Client,
    config: &Config,
    submitted: &SubmittedRequest,
    credential: &str,
) -> Result<(&'static str, usize), String> {
    for poll in 1..=config.max_status_polls {
        let observation = match config.provider.as_str() {
            FAL_PROVIDER => match fetch_fal_queue_status_with_client(
                client,
                &submitted.status_url,
                credential,
                false,
            ) {
                Ok(status) if status.status == FalQueueStatusKind::Completed => {
                    return Err(
                        "fal request completed before cancellation became terminal".to_string()
                    )
                }
                Ok(_) => None,
                Err(error) if fal_missing_after_cancel(&error) => Some("not-found"),
                Err(error) => return Err(format!("fal cancellation status read failed: {error}")),
            },
            REPLICATE_PROVIDER => {
                let status = fetch_replicate_prediction_with_client(
                    client,
                    &submitted.status_url,
                    credential,
                )
                .map_err(|error| format!("Replicate cancellation status read failed: {error}"))?;
                match status.status {
                    ReplicatePredictionStatusKind::Canceled => Some("cancelled"),
                    ReplicatePredictionStatusKind::Succeeded
                    | ReplicatePredictionStatusKind::Failed => {
                        return Err(format!(
                            "Replicate request reached {:?} instead of cancelled",
                            status.status
                        ))
                    }
                    _ => None,
                }
            }
            _ => unreachable!(),
        };
        if let Some(observation) = observation {
            return Ok((observation, poll));
        }
        thread::sleep(Duration::from_millis(config.poll_interval_ms));
    }
    Err(format!(
        "provider cancellation did not reach cancelled/not-found after {} polls",
        config.max_status_polls
    ))
}

fn fal_missing_after_cancel(error: &FalQueueSubmitError) -> bool {
    matches!(
        error,
        FalQueueSubmitError::HttpStatus {
            status: 404 | 410 | 422
        } | FalQueueSubmitError::HttpStatusWithMessage {
            status: 404 | 410 | 422,
            ..
        }
    )
}

fn cancel_status_label(status: FalQueueCancelStatus) -> &'static str {
    match status {
        FalQueueCancelStatus::CancellationRequested => "cancellation-requested",
        FalQueueCancelStatus::AlreadyCompleted => "already-completed",
        FalQueueCancelStatus::NotFound => "not-found",
    }
}

fn replicate_status_label(status: ReplicatePredictionStatusKind) -> &'static str {
    match status {
        ReplicatePredictionStatusKind::Starting => "queued",
        ReplicatePredictionStatusKind::Processing => "running",
        ReplicatePredictionStatusKind::Succeeded => "completed",
        ReplicatePredictionStatusKind::Failed => "failed",
        ReplicatePredictionStatusKind::Canceled => "cancelled",
    }
}

fn credential_source_label(source: ProviderCredentialSource) -> &'static str {
    match source {
        ProviderCredentialSource::Keychain => "keychain",
        ProviderCredentialSource::Missing => "missing",
        ProviderCredentialSource::Unavailable => "unavailable",
    }
}

fn absolute_from(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_asset_is_low_cost_and_has_no_outputs() {
        let asset = cancellation_asset(
            &Config {
                provider: FAL_PROVIDER.to_string(),
                model: FAL_FLUX_SCHNELL_MODEL_ID.to_string(),
                prompt: "fixture".to_string(),
                out_dir: PathBuf::from("fixture"),
                max_status_polls: 1,
                poll_interval_ms: 1,
                continue_existing: false,
            },
            "2026-07-12T00:00:00Z",
        );
        assert_eq!(asset.settings.width, Some(512));
        assert_eq!(asset.settings.height, Some(512));
        assert_eq!(asset.settings.num_images, Some(1));
        assert!(asset.outputs.is_empty());
    }

    #[test]
    fn recognizes_only_terminal_missing_statuses_after_fal_cancel() {
        assert!(fal_missing_after_cancel(&FalQueueSubmitError::HttpStatus {
            status: 404
        }));
        assert!(fal_missing_after_cancel(
            &FalQueueSubmitError::HttpStatusWithMessage {
                status: 422,
                message: "gone".to_string(),
            }
        ));
        assert!(!fal_missing_after_cancel(
            &FalQueueSubmitError::HttpStatus { status: 500 }
        ));
    }
}
