use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;
use video_creater_lib::edit::render_plan::RenderQuality;
use video_creater_lib::project::action::{ProjectAction, ProjectActionReplaceGeneratedOutput};
use video_creater_lib::project::export_options::ExportRenderOptions;
use video_creater_lib::project::export_profiles::ExportProfile;
use video_creater_lib::project::import::import_media_files;
use video_creater_lib::project::model::{
    GeneratedAssetStatus, JobStatus, TimelineItem, TimelineItemKind, TimelineSource, TrackKind,
};
use video_creater_lib::project::split::{
    apply_project_action_to_split_project, apply_project_actions_to_split_project,
    load_split_project, migrate_single_file_project, save_split_project,
};
use video_creater_lib::provider_credentials::{
    resolve_provider_credential, ProviderCredentialSource,
};
use video_creater_lib::render_pipeline::project_export::render_media_to_split_project_folder;
use video_creater_lib::render_runtime::start_render_process_runtime;
use video_creater_lib::workflows::{
    reconcile_interrupted_generation_jobs_on_project_open,
    resume_interrupted_generate_media_job_with_client_and_credential_cancellable,
    temporal_job_summary, TemporalGenerateMediaProviderRunOptions, TemporalWorkflowKind,
};

const LIVE_SPEND_ENV_VAR: &str = "VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND";
const ASSET_ID: &str = "provider-e2e-generated";
const INSERT_ITEM_ID: &str = "provider-app-e2e-insert";
const INSERT_AUDIO_ITEM_ID: &str = "provider-app-e2e-insert-audio";
const REPLACE_ITEM_ID: &str = "provider-app-e2e-replace";
const BACKGROUND_ITEM_ID: &str = "provider-app-e2e-background";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Config {
    provider: String,
    model: String,
    scenario: String,
    prompt: String,
    out_dir: PathBuf,
    provider_runner: Option<PathBuf>,
    max_status_polls: usize,
    poll_interval_ms: u64,
    recover: bool,
    continue_existing: bool,
    output_index: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderRunResult {
    ok: bool,
    scenario: String,
    provider: String,
    model: String,
    project_dir: PathBuf,
    artifact_path: PathBuf,
    output_count: usize,
    job_status: String,
    provider_request: Value,
    #[serde(default)]
    lifecycle_history: Vec<LifecycleObservation>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LifecycleObservation {
    status: String,
    job_status: String,
    asset_status: String,
    observed_at: String,
    source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CredentialEvidence {
    configured: bool,
    source: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalEvidence {
    project_dir: String,
    asset_id: String,
    generated_media_id: String,
    output_relative_path: String,
    media_kind: &'static str,
    persisted: bool,
    reloaded: bool,
    inserted: bool,
    embedded_audio_inserted: bool,
    replaced: bool,
    insert_item_id: String,
    replacement_item_id: Option<String>,
    output_cardinality: usize,
    selected_output_index: usize,
    selection_method: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryEvidence {
    supported: bool,
    attempted: bool,
    resume_candidate: bool,
    request_id_preserved: bool,
    completed: bool,
    output_reloaded: bool,
    method: &'static str,
}

fn main() {
    if let Err(error) = start_render_process_runtime() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    match parse_args(std::env::args().skip(1)).and_then(run) {
        Ok(report_path) => println!("{}", report_path.display()),
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
    let mut provider = "replicate".to_string();
    let mut model = "black-forest-labs/flux-schnell".to_string();
    let mut scenario = "text-to-image".to_string();
    let mut prompt = "small product still on a clean studio background".to_string();
    let mut out_dir = PathBuf::from("output/provider-app-live-e2e/replicate-text-to-image");
    let mut provider_runner = None;
    let mut max_status_polls = 60usize;
    let mut poll_interval_ms = 5_000u64;
    let mut recover = true;
    let mut continue_existing = false;
    let mut output_index = 0usize;
    let mut args = args.into_iter().filter(|arg| arg != "--");

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--provider" => provider = require_value(&arg, args.next())?,
            "--model" => model = require_value(&arg, args.next())?,
            "--scenario" => scenario = require_value(&arg, args.next())?,
            "--prompt" => prompt = require_value(&arg, args.next())?,
            "--out-dir" => out_dir = PathBuf::from(require_value(&arg, args.next())?),
            "--provider-runner" => {
                provider_runner = Some(PathBuf::from(require_value(&arg, args.next())?))
            }
            "--max-status-polls" => {
                max_status_polls = positive_usize(&arg, &require_value(&arg, args.next())?)?
            }
            "--poll-interval-ms" => {
                poll_interval_ms = positive_u64(&arg, &require_value(&arg, args.next())?)?
            }
            "--recover" => recover = parse_bool(&arg, &require_value(&arg, args.next())?)?,
            "--continue-existing" => {
                continue_existing = parse_bool(&arg, &require_value(&arg, args.next())?)?
            }
            "--output-index" => {
                output_index = non_negative_usize(&arg, &require_value(&arg, args.next())?)?
            }
            "--live" => {}
            "--help" | "-h" => {
                println!(
                    "Usage: video-creater-provider-app-e2e --live --provider fal.ai|replicate \
                     --model MODEL --scenario SCENARIO --out-dir OUTPUT_DIR [--recover true|false]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    if !matches!(provider.as_str(), "fal.ai" | "replicate") {
        return Err(format!(
            "provider app E2E currently supports fal.ai and replicate; got {provider}"
        ));
    }
    if model.trim().is_empty() || scenario.trim().is_empty() || prompt.trim().is_empty() {
        return Err("model, scenario, and prompt must be non-empty".to_string());
    }
    if std::env::var(LIVE_SPEND_ENV_VAR).ok().as_deref() != Some("1") {
        return Err(format!(
            "live provider app E2E requires explicit spend opt-in: {LIVE_SPEND_ENV_VAR}=1"
        ));
    }

    Ok(Config {
        provider,
        model,
        scenario,
        prompt,
        out_dir,
        provider_runner,
        max_status_polls,
        poll_interval_ms,
        recover,
        continue_existing,
        output_index,
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

fn non_negative_usize(flag: &str, value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("{flag} must be a non-negative integer"))
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

    let credential = resolve_provider_credential(&config.provider).map_err(|error| {
        format!(
            "{} credential is not configured through the app credential resolver: {error}",
            config.provider
        )
    })?;
    let credential_source = credential_source_label(credential.source());
    let credential_env_var = provider_runner_credential_env_var(credential.provider());

    let provider_run = if config.continue_existing {
        let provider_run_path = out_dir.join("provider-run.json");
        let provider_run: ProviderRunResult =
            serde_json::from_slice(&fs::read(&provider_run_path).map_err(|error| {
                format!(
                    "cannot continue existing provider evidence without {}: {error}",
                    provider_run_path.display()
                )
            })?)
            .map_err(|error| format!("existing provider run is invalid: {error}"))?;
        ensure_current_split_project(&project_dir)?;
        provider_run
    } else {
        let child_output = run_provider_child(
            repo_root,
            &config,
            &project_dir,
            credential_env_var,
            credential.secret(),
        )?;
        let provider_run: ProviderRunResult = serde_json::from_slice(&child_output.stdout)
            .map_err(|error| {
                format!("provider runner did not emit its typed JSON result: {error}")
            })?;
        fs::write(
            out_dir.join("provider-run.json"),
            serde_json::to_vec_pretty(
                &serde_json::from_slice::<Value>(&child_output.stdout).map_err(|error| {
                    format!("provider runner JSON could not be retained: {error}")
                })?,
            )
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        provider_run
    };
    validate_provider_run(&config, &provider_run, &project_dir)?;
    let (lifecycle_observations, lifecycle_coverage, persisted_terminal_status) =
        source_backed_lifecycle(&provider_run, &project_dir)?;
    let observed_lifecycle = lifecycle_observations
        .iter()
        .map(|observation| observation.status.clone())
        .collect::<Vec<_>>();

    let recovery = if config.recover {
        prove_recovery(
            &config,
            &project_dir,
            credential.secret(),
            config.max_status_polls,
            config.poll_interval_ms,
        )?
    } else {
        RecoveryEvidence {
            supported: true,
            attempted: false,
            resume_candidate: false,
            request_id_preserved: false,
            completed: false,
            output_reloaded: false,
            method: "persisted-provider-get-only",
        }
    };

    let canonical = prepare_canonical_timeline(&project_dir, &out_dir, config.output_index)?;
    let now = Utc::now().to_rfc3339();
    let render_job_id = format!(
        "provider-app-e2e-final-render-{}",
        Utc::now().timestamp_millis()
    );
    let render_run_id = format!("{render_job_id}-run");
    let render_job = temporal_job_summary(
        TemporalWorkflowKind::ExportMedia,
        "provider-app-e2e-project",
        &render_job_id,
        JobStatus::Queued,
        &now,
    );
    let render = render_media_to_split_project_folder(
        &project_dir,
        "provider-app-e2e-project",
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Final, 1920, 1080)
            .map_err(|error| error.to_string())?,
        render_job,
        &now,
        Some(render_run_id),
        None,
    )
    .map_err(|errors| {
        errors
            .into_iter()
            .map(|error| format!("{error:?}"))
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let rendered_path = project_dir.join(&render.output_path);
    fs::copy(&rendered_path, out_dir.join("final-output.mp4"))
        .map_err(|error| error.to_string())?;
    fs::copy(
        project_dir.join(&render.project_render_report.log_path),
        out_dir.join("render.log"),
    )
    .map_err(|error| error.to_string())?;

    let report = json!({
        "schemaVersion": 2,
        "status": "passed",
        "providerMode": "live",
        "provider": config.provider,
        "model": config.model,
        "scenario": config.scenario,
        "credential": CredentialEvidence {
            configured: true,
            source: credential_source,
        },
        "providerRequest": provider_run.provider_request,
        "providerRun": {
            "jobStatus": provider_run.job_status,
            "outputCount": provider_run.output_count,
            "artifactPath": provider_run.artifact_path,
            "providerReportPath": "provider-run.json"
        },
        "lifecycle": {
            "observed": observed_lifecycle,
            "observations": lifecycle_observations,
            "coverage": lifecycle_coverage,
            "persistedTerminalStatus": persisted_terminal_status,
            "recovery": &recovery
        },
        "canonical": canonical,
        "render": {
            "durationSeconds": render.project_render_report.duration_seconds,
            "streams": render.project_render_report.streams,
            "checks": render.project_render_report.checks,
            "artifacts": ["provider-run.json", "final-output.mp4", "render.log", "report.json"],
            "outputPath": "final-output.mp4",
            "logPath": "render.log"
        }
    });
    let serialized = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?;
    if serialized.contains(credential.secret()) {
        return Err(
            "provider app E2E report attempted to serialize credential material".to_string(),
        );
    }
    let report_path = out_dir.join("report.json");
    fs::write(&report_path, serialized).map_err(|error| error.to_string())?;
    Ok(report_path)
}

fn source_backed_lifecycle(
    provider_run: &ProviderRunResult,
    project_dir: &Path,
) -> Result<(Vec<LifecycleObservation>, &'static str, String), String> {
    let project = load_split_project(project_dir).map_err(|error| error.to_string())?;
    let job = project
        .jobs
        .iter()
        .find(|candidate| candidate.id == ASSET_ID)
        .ok_or_else(|| "provider lifecycle project is missing its generation job".to_string())?;
    let asset = project
        .generated_assets
        .iter()
        .find(|candidate| candidate.id == ASSET_ID)
        .ok_or_else(|| "provider lifecycle project is missing its generated asset".to_string())?;
    let terminal = format!("{:?}", job.status).to_ascii_lowercase();
    let terminal_asset = format!("{:?}", asset.status).to_ascii_lowercase();
    if terminal != "completed" || terminal_asset != "completed" {
        return Err(format!(
            "provider lifecycle terminal state is not completed: job={terminal}, asset={terminal_asset}"
        ));
    }

    let observations = if provider_run.lifecycle_history.is_empty() {
        vec![LifecycleObservation {
            status: terminal.clone(),
            job_status: terminal.clone(),
            asset_status: terminal_asset,
            observed_at: job.updated_at.clone(),
            source: "persisted-split-project-terminal".to_string(),
        }]
    } else {
        provider_run.lifecycle_history.clone()
    };
    for observation in &observations {
        if !matches!(
            observation.source.as_str(),
            "persisted-split-project" | "persisted-split-project-terminal"
        ) || observation.status != observation.job_status
            || observation.asset_status != observation.status
            || observation.observed_at.trim().is_empty()
        {
            return Err(
                "provider lifecycle history is not a source-backed matching job/asset transition"
                    .to_string(),
            );
        }
    }
    let statuses = observations
        .iter()
        .map(|observation| observation.status.as_str())
        .collect::<Vec<_>>();
    let coverage = if statuses == ["queued", "running", "completed"] {
        if observations
            .iter()
            .any(|observation| observation.source != "persisted-split-project")
        {
            return Err(
                "full provider lifecycle history must come from persisted transition snapshots"
                    .to_string(),
            );
        }
        "full-transition"
    } else if statuses == ["completed"]
        && observations[0].source == "persisted-split-project-terminal"
    {
        "terminal-only"
    } else {
        return Err(format!(
            "provider lifecycle history has unsupported observed sequence: {}",
            statuses.join(" -> ")
        ));
    };
    Ok((observations, coverage, terminal))
}

fn run_provider_child(
    repo_root: &Path,
    config: &Config,
    project_dir: &Path,
    credential_env_var: &str,
    credential: &str,
) -> Result<Output, String> {
    let max_status_polls = config.max_status_polls.to_string();
    let poll_interval_ms = config.poll_interval_ms.to_string();
    let mut command = provider_command(repo_root, config)?;
    command
        .current_dir(repo_root)
        .env(credential_env_var, credential)
        .args([
            "--provider",
            &config.provider,
            "--model",
            &config.model,
            "--scenario",
            &config.scenario,
            "--prompt",
            &config.prompt,
            "--out-dir",
        ])
        .arg(project_dir)
        .args([
            "--max-status-polls",
            &max_status_polls,
            "--poll-interval-ms",
            &poll_interval_ms,
        ]);
    let output = command
        .output()
        .map_err(|error| format!("failed to launch production provider runner: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "production provider runner failed (status {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output)
}

fn provider_runner_credential_env_var(provider: &str) -> &'static str {
    match provider {
        "replicate" => "REPLICATE_API_TOKEN",
        "openai" => "OPENAI_API_KEY",
        "xai" => "XAI_API_KEY",
        "google" => "GEMINI_API_KEY",
        "elevenlabs" => "ELEVENLABS_API_KEY",
        "minimax" => "MINIMAX_API_KEY",
        _ => "FAL_KEY",
    }
}

fn provider_command(repo_root: &Path, config: &Config) -> Result<Command, String> {
    if let Some(path) = &config.provider_runner {
        if !path.is_file() {
            return Err(format!(
                "provider runner does not exist: {}",
                path.display()
            ));
        }
        return Ok(Command::new(path));
    }
    let manifest = repo_root.join("src-tauri/Cargo.toml");
    let mut command = Command::new("cargo");
    command.args([
        "run",
        "--quiet",
        "--manifest-path",
        manifest
            .to_str()
            .ok_or_else(|| "Cargo manifest path is not UTF-8".to_string())?,
        "--bin",
        "video-creater-provider-e2e",
        "--",
    ]);
    Ok(command)
}

fn validate_provider_run(
    config: &Config,
    result: &ProviderRunResult,
    project_dir: &Path,
) -> Result<(), String> {
    if !result.ok
        || result.provider != config.provider
        || result.model != config.model
        || result.scenario != config.scenario
        || result.job_status != "Completed"
        || result.output_count < 1
    {
        return Err(
            "provider runner result did not prove the requested completed generation".into(),
        );
    }
    if result.project_dir != project_dir {
        return Err(format!(
            "provider runner project path mismatch: expected {}, got {}",
            project_dir.display(),
            result.project_dir.display()
        ));
    }
    if !result.artifact_path.is_file() || !result.artifact_path.starts_with(project_dir) {
        return Err("provider artifact is missing or escaped the evidence project".to_string());
    }
    Ok(())
}

fn ensure_current_split_project(project_dir: &Path) -> Result<(), String> {
    if load_split_project(project_dir).is_ok() {
        return Ok(());
    }
    migrate_single_file_project(project_dir)
        .map(|_| ())
        .map_err(|error| format!("existing provider project could not be migrated: {error}"))
}

fn prove_recovery(
    config: &Config,
    project_dir: &Path,
    credential: &str,
    max_status_polls: usize,
    poll_interval_ms: u64,
) -> Result<RecoveryEvidence, String> {
    let mut project = load_split_project(project_dir).map_err(|error| error.to_string())?;
    let asset_index = project
        .generated_assets
        .iter()
        .position(|asset| asset.id == ASSET_ID)
        .ok_or_else(|| "completed provider project is missing its generated asset".to_string())?;
    let job_index = project
        .jobs
        .iter()
        .position(|job| job.id == ASSET_ID)
        .ok_or_else(|| "completed provider project is missing its generation job".to_string())?;
    let request_id = project.jobs[job_index]
        .provider_request
        .as_ref()
        .map(|request| request.request_id.clone())
        .ok_or_else(|| {
            "completed provider job is missing persisted request metadata".to_string()
        })?;
    let outputs = project.generated_assets[asset_index].outputs.clone();
    let output_ids = outputs
        .iter()
        .map(|output| output.media_id.as_str())
        .collect::<Vec<_>>();
    project
        .media
        .retain(|media| !output_ids.contains(&media.id.as_str()));
    for output in &outputs {
        let _ = fs::remove_file(project_dir.join(&output.relative_path));
    }
    project.generated_assets[asset_index].status = GeneratedAssetStatus::Running;
    project.generated_assets[asset_index].outputs.clear();
    project.jobs[job_index].status = JobStatus::Running;
    // Recovery resumes what the desktop process ran itself; Temporal runs stay with their worker.
    if let Some(workflow) = project.jobs[job_index].workflow.as_mut() {
        workflow.run_id = Some(format!("in-process/{}", workflow.workflow_id));
    }
    save_split_project(project_dir, &project).map_err(|error| error.to_string())?;

    let now = Utc::now().to_rfc3339();
    let reconciled = reconcile_interrupted_generation_jobs_on_project_open(project_dir, &now)
        .map_err(|error| error.to_string())?;
    let resume_candidate = reconciled
        .resume_candidates
        .iter()
        .any(|candidate| candidate.job_id == ASSET_ID && candidate.provider == config.provider);
    if !resume_candidate {
        return Err("project-open recovery did not identify the persisted provider request".into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|error| error.to_string())?;
    resume_interrupted_generate_media_job_with_client_and_credential_cancellable(
        &client,
        project_dir,
        ASSET_ID,
        &now,
        credential,
        TemporalGenerateMediaProviderRunOptions {
            max_status_polls,
            poll_interval_millis: poll_interval_ms,
        },
        None,
    )
    .map_err(|error| format!("persisted provider recovery failed: {error}"))?;
    let reloaded = load_split_project(project_dir).map_err(|error| error.to_string())?;
    let asset = reloaded
        .generated_assets
        .iter()
        .find(|asset| asset.id == ASSET_ID)
        .ok_or_else(|| "recovered project lost its generated asset".to_string())?;
    let job = reloaded
        .jobs
        .iter()
        .find(|job| job.id == ASSET_ID)
        .ok_or_else(|| "recovered project lost its generation job".to_string())?;
    let request_id_preserved = job
        .provider_request
        .as_ref()
        .is_some_and(|request| request.request_id == request_id);
    let output_reloaded = !asset.outputs.is_empty()
        && asset.outputs.iter().all(|output| {
            reloaded
                .media
                .iter()
                .any(|media| media.id == output.media_id)
                && project_dir.join(&output.relative_path).is_file()
        });
    if asset.status != GeneratedAssetStatus::Completed
        || job.status != JobStatus::Completed
        || !request_id_preserved
        || !output_reloaded
    {
        return Err("provider recovery did not durably restore the completed output".to_string());
    }
    Ok(RecoveryEvidence {
        supported: true,
        attempted: true,
        resume_candidate,
        request_id_preserved,
        completed: true,
        output_reloaded,
        method: "persisted-provider-get-only",
    })
}

fn prepare_canonical_timeline(
    project_dir: &Path,
    evidence_dir: &Path,
    output_index: usize,
) -> Result<CanonicalEvidence, String> {
    let project = load_split_project(project_dir).map_err(|error| error.to_string())?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == ASSET_ID && asset.status == GeneratedAssetStatus::Completed)
        .ok_or_else(|| {
            "canonical project does not contain the completed generated asset".to_string()
        })?;
    let output_cardinality = asset.outputs.len();
    let output = asset
        .outputs
        .get(output_index)
        .cloned()
        .ok_or_else(|| {
            format!(
                "selected generated output index {output_index} is outside cardinality {output_cardinality}"
            )
        })?;
    if !project_dir.join(&output.relative_path).is_file() {
        return Err("canonical generated output file is missing".to_string());
    }
    let media_type = media_type_for_path(&output.relative_path);
    let expects_embedded_audio =
        media_type == "video" && asset.settings.generate_audio.unwrap_or(false);
    let duration = if output.duration_seconds.is_finite() && output.duration_seconds > 0.0 {
        output.duration_seconds.min(30.0)
    } else {
        3.0
    };

    let existing_item_ids = project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter().map(|item| item.id.clone()))
        .collect::<Vec<_>>();
    if !existing_item_ids.is_empty() {
        apply_project_action_to_split_project(
            project_dir,
            ProjectAction::RemoveItems {
                item_ids: existing_item_ids,
            },
        )
        .map_err(|error| error.to_string())?;
    }

    let project = load_split_project(project_dir).map_err(|error| error.to_string())?;
    let visual_track_id = track_id(&project, TrackKind::Video)?;
    let audio_track_id = track_id(&project, TrackKind::Audio)?;
    let generated_item_kind = match media_type {
        "image" => TimelineItemKind::ImageClip,
        "video" => TimelineItemKind::VideoClip,
        "audio" => TimelineItemKind::AudioClip,
        _ => unreachable!(),
    };
    let generated_track_id = if media_type == "audio" {
        audio_track_id.clone()
    } else {
        visual_track_id.clone()
    };

    let mut actions = vec![ProjectAction::AddItems {
        target_track_id: generated_track_id,
        items: vec![timeline_media_item(
            INSERT_ITEM_ID,
            generated_item_kind,
            &output.media_id,
            0.0,
            duration,
        )],
    }];
    if expects_embedded_audio {
        actions.push(ProjectAction::AddItems {
            target_track_id: audio_track_id,
            items: vec![timeline_media_item(
                INSERT_AUDIO_ITEM_ID,
                TimelineItemKind::AudioClip,
                &output.media_id,
                0.0,
                duration,
            )],
        });
    }
    let mut replacement_item_id = None;
    if media_type != "audio" {
        let placeholder_path = evidence_dir.join("replacement-placeholder.png");
        write_placeholder_png(&placeholder_path)?;
        let imported = import_media_files(project_dir, project, &[placeholder_path])
            .map_err(|error| error.to_string())?;
        let placeholder_media_id = imported.imported[0].id.clone();
        actions.push(ProjectAction::AddItems {
            target_track_id: visual_track_id.clone(),
            items: vec![timeline_media_item(
                REPLACE_ITEM_ID,
                TimelineItemKind::ImageClip,
                &placeholder_media_id,
                duration,
                duration,
            )],
        });
        actions.push(ProjectAction::ReplaceTimelineItemWithGeneratedOutput {
            replacement: ProjectActionReplaceGeneratedOutput {
                item_id: REPLACE_ITEM_ID.to_string(),
                media_id: output.media_id.clone(),
            },
        });
        replacement_item_id = Some(REPLACE_ITEM_ID.to_string());
    } else {
        let background_path = evidence_dir.join("audio-background.png");
        write_placeholder_png(&background_path)?;
        let imported = import_media_files(project_dir, project, &[background_path])
            .map_err(|error| error.to_string())?;
        let background_media_id = imported.imported[0].id.clone();
        actions.push(ProjectAction::AddItems {
            target_track_id: visual_track_id,
            items: vec![timeline_media_item(
                BACKGROUND_ITEM_ID,
                TimelineItemKind::ImageClip,
                &background_media_id,
                0.0,
                duration,
            )],
        });
    }
    apply_project_actions_to_split_project(project_dir, actions)
        .map_err(|error| error.to_string())?;

    let reloaded = load_split_project(project_dir).map_err(|error| error.to_string())?;
    let inserted = reloaded.timeline.tracks.iter().any(|track| {
        track.items.iter().any(|item| {
            item.id == INSERT_ITEM_ID
                && matches!(&item.source, TimelineSource::Media { media_id } if media_id == &output.media_id)
        })
    });
    let embedded_audio_inserted = reloaded.timeline.tracks.iter().any(|track| {
        track.kind == TrackKind::Audio
            && track.items.iter().any(|item| {
                item.id == INSERT_AUDIO_ITEM_ID
                    && matches!(&item.source, TimelineSource::Media { media_id } if media_id == &output.media_id)
            })
    });
    let replaced = replacement_item_id.as_ref().is_none_or(|item_id| {
        reloaded.timeline.tracks.iter().any(|track| {
            track.items.iter().any(|item| {
                item.id == *item_id
                    && matches!(&item.source, TimelineSource::Media { media_id } if media_id == &output.media_id)
            })
        })
    });
    if !inserted || !replaced || (expects_embedded_audio && !embedded_audio_inserted) {
        return Err("generated output did not survive canonical insert/replace reload".to_string());
    }

    Ok(CanonicalEvidence {
        project_dir: project_dir.display().to_string(),
        asset_id: ASSET_ID.to_string(),
        generated_media_id: output.media_id,
        output_relative_path: output.relative_path,
        media_kind: media_type,
        persisted: true,
        reloaded: true,
        inserted,
        embedded_audio_inserted,
        replaced,
        insert_item_id: INSERT_ITEM_ID.to_string(),
        replacement_item_id,
        output_cardinality,
        selected_output_index: output_index,
        selection_method: "explicit-zero-based-index",
    })
}

fn timeline_media_item(
    id: &str,
    kind: TimelineItemKind,
    media_id: &str,
    start_seconds: f64,
    duration_seconds: f64,
) -> TimelineItem {
    let properties = if kind == TimelineItemKind::ImageClip {
        BTreeMap::new()
    } else {
        BTreeMap::from([
            ("sourceIn".to_string(), json!(0.0)),
            ("sourceOut".to_string(), json!(duration_seconds)),
        ])
    };
    TimelineItem {
        id: id.to_string(),
        kind,
        start_seconds,
        duration_seconds,
        source: TimelineSource::Media {
            media_id: media_id.to_string(),
        },
        label: "Provider app E2E generated output".to_string(),
        properties,
    }
}

fn track_id(
    project: &video_creater_lib::project::model::VideoProject,
    kind: TrackKind,
) -> Result<String, String> {
    project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == kind)
        .map(|track| track.id.clone())
        .ok_or_else(|| format!("canonical project is missing its {kind:?} track"))
}

fn media_type_for_path(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" | "jpg" | "jpeg" | "webp" => "image",
        "mp3" | "wav" | "m4a" | "aac" | "flac" | "ogg" => "audio",
        _ => "video",
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

fn write_placeholder_png(path: &Path) -> Result<(), String> {
    let file = fs::File::create(path).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(file, 64, 64);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    let mut pixels = vec![0u8; 64 * 64 * 4];
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        let x = index % 64;
        let y = index / 64;
        pixel.copy_from_slice(&[24 + (x * 2) as u8, 48 + y as u8, 96, 255]);
    }
    writer
        .write_image_data(&pixels)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_spend_opt_in<T>(run: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let before = std::env::var_os(LIVE_SPEND_ENV_VAR);
        std::env::set_var(LIVE_SPEND_ENV_VAR, "1");
        let output = run();
        if let Some(value) = before {
            std::env::set_var(LIVE_SPEND_ENV_VAR, value);
        } else {
            std::env::remove_var(LIVE_SPEND_ENV_VAR);
        }
        output
    }

    #[test]
    fn parses_extensible_live_provider_config() {
        let config = with_spend_opt_in(|| {
            parse_args(
                [
                    "--live",
                    "--provider",
                    "fal.ai",
                    "--model",
                    "fal-ai/flux/schnell",
                    "--scenario",
                    "text-to-image",
                    "--out-dir",
                    "output/custom",
                    "--recover",
                    "false",
                    "--output-index",
                    "1",
                ]
                .into_iter()
                .map(str::to_string),
            )
            .expect("live config")
        });
        assert_eq!(config.provider, "fal.ai");
        assert_eq!(config.model, "fal-ai/flux/schnell");
        assert_eq!(config.out_dir, PathBuf::from("output/custom"));
        assert!(!config.recover);
        assert_eq!(config.output_index, 1);
    }

    #[test]
    fn rejects_unsupported_provider_before_credential_lookup() {
        let error = with_spend_opt_in(|| {
            parse_args(["--provider", "openai"].into_iter().map(str::to_string))
                .expect_err("unsupported provider")
        });
        assert!(error.contains("currently supports fal.ai and replicate"));
    }

    #[test]
    fn classifies_generated_output_media_for_timeline_materialization() {
        assert_eq!(media_type_for_path("generated/a/output.webp"), "image");
        assert_eq!(media_type_for_path("generated/a/output.m4a"), "audio");
        assert_eq!(media_type_for_path("generated/a/output.mp4"), "video");
    }

    #[test]
    fn still_timeline_items_keep_zero_source_range_while_video_uses_duration() {
        let still = timeline_media_item(
            "still",
            TimelineItemKind::ImageClip,
            "still-media",
            0.0,
            4.0,
        );
        let video = timeline_media_item(
            "video",
            TimelineItemKind::VideoClip,
            "video-media",
            0.0,
            4.0,
        );
        assert!(!still.properties.contains_key("sourceOut"));
        assert_eq!(video.properties["sourceOut"], json!(4.0));
    }
}
