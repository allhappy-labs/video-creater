use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;
use video_creater_lib::provider_credentials::{
    delete_provider_credential_with_store, list_provider_credential_statuses_with_store,
    provider_credential_status_with_store, set_provider_credential_with_store, CredentialStore,
    CredentialStoreError, ProviderCredentialSource, ProviderCredentialStatus,
};
use video_creater_lib::settings::agent::{
    probe_claude_cli, probe_codex_app_server, probe_mcp_server, probe_proposal_validator,
};
use video_creater_lib::settings::health::{build_settings_health_snapshot, SettingsHealthState};
use video_creater_lib::settings::operations::{
    SettingsOperationKind, SettingsOperationRegistry, SettingsOperationState,
    SettingsOperationTransition,
};
use video_creater_lib::settings::providers::{
    aggregate_provider_health, ProviderAccountValidation, ProviderModelDependency,
    ProviderValidationState,
};
use video_creater_lib::settings::render_system::{
    get_render_system_health, native_delivery_capability,
};
use video_creater_lib::settings::skills::{
    mandatory_skill_definition, mandatory_skill_prompt_bundle, repair_bundled_skills,
    sha256_checksum, verify_bundled_skills, SkillLoadState, SkillRepairConfirmation,
    SkillRepairOutcome, MANDATORY_SKILLS,
};
use video_creater_lib::settings::storage::{
    collect_storage_inventory, preview_storage_cleanup, run_storage_cleanup, StorageCleanupTarget,
};
use video_creater_lib::speech_models::ProductionSpeechModelStore;
use video_creater_lib::transcription::acquisition::ModelDownloadProgressSnapshot;
use video_creater_lib::transcription::model::{parakeet_v3_catalog_entry, ModelInstallStatus};
use video_creater_lib::transcription::store::{
    StartDownload, TranscriptionModelStore, MODEL_MANIFEST_FILE_NAME,
};

const DEFAULT_REPORT_PATH: &str = "output/settings-readiness/integration/report.json";
const REPORT_SCHEMA: &str = "video-creater.settings-readiness";
const REPORT_VERSION: u32 = 1;

const REQUIRED_CHECK_IDS: [&str; 9] = [
    "settings-health-snapshot",
    "operation-journal-recovery",
    "transcription-model-lifecycle",
    "speech-readiness-separation",
    "render-system-rollup",
    "agent-component-independence",
    "skill-scoped-repair",
    "storage-cleanup-safety",
    "provider-secret-redaction",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct EvidenceCheck {
    id: String,
    required: bool,
    status: CheckStatus,
    duration_ms: u64,
    diagnostic_code: String,
    detail: String,
    artifact_paths: Vec<String>,
    provenance: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum CheckStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct EvidenceReport {
    schema: String,
    version: u32,
    generated_at: String,
    passed: bool,
    checks: Vec<EvidenceCheck>,
    notes: Vec<String>,
}

fn build_report(checks: Vec<EvidenceCheck>) -> EvidenceReport {
    let mut report = EvidenceReport {
        schema: REPORT_SCHEMA.to_string(),
        version: REPORT_VERSION,
        generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        passed: false,
        checks,
        notes: vec![
            "Local fixture evidence exercises native Rust service contracts only.".to_string(),
            "Packaged WebView dialogs, Finder, Keychain prompts, signing, notarization, and real packaged renders are outside this report.".to_string(),
        ],
    };
    report.passed = required_checks_passed(&report);
    report
}

fn required_checks_passed(report: &EvidenceReport) -> bool {
    REQUIRED_CHECK_IDS.iter().all(|required_id| {
        let matches = report
            .checks
            .iter()
            .filter(|check| check.id == *required_id && check.required)
            .collect::<Vec<_>>();
        matches.len() == 1 && matches[0].status == CheckStatus::Passed
    }) && report
        .checks
        .iter()
        .filter(|check| check.required)
        .all(|check| check.status == CheckStatus::Passed)
}

fn assert_secret_free(value: &Value, canaries: &[&str]) -> Result<(), String> {
    match value {
        Value::Object(entries) => {
            for (key, child) in entries {
                let normalized = key
                    .chars()
                    .filter(|character| character.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_ascii_lowercase();
                if matches!(
                    normalized.as_str(),
                    "secret"
                        | "token"
                        | "accesstoken"
                        | "refreshtoken"
                        | "apikey"
                        | "credential"
                        | "credentialvalue"
                        | "authorization"
                        | "authheader"
                ) {
                    return Err(format!("secret-shaped report key: {key}"));
                }
                assert_secret_free(child, canaries)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                assert_secret_free(child, canaries)?;
            }
        }
        Value::String(text) => {
            let lowercase = text.to_ascii_lowercase();
            if lowercase.contains("authorization:") || lowercase.contains("bearer ") {
                return Err("authentication header text appeared in report".to_string());
            }
            for canary in canaries {
                if !canary.is_empty() && text.contains(canary) {
                    return Err("fixture secret canary appeared in report".to_string());
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn assert_no_unsafe_host_content(value: &Value) -> Result<(), String> {
    match value {
        Value::Object(entries) => {
            for child in entries.values() {
                assert_no_unsafe_host_content(child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                assert_no_unsafe_host_content(child)?;
            }
        }
        Value::String(text) => {
            let lowercase = text.to_ascii_lowercase();
            if lowercase.contains("/users/")
                || lowercase.contains("/home/")
                || lowercase.contains("/private/tmp/")
                || (cfg!(target_os = "linux") && lowercase.contains("/tmp/"))
                || lowercase.contains("/var/folders/")
                || contains_windows_drive_path(text)
            {
                return Err("absolute host path appeared in evidence".to_string());
            }
            if lowercase.contains("authorization:")
                || lowercase.contains("bearer ")
                || lowercase.contains("environment dump")
                || lowercase.contains("request headers")
                || lowercase.contains("response headers")
            {
                return Err(
                    "authentication, header, or environment dump appeared in evidence".to_string(),
                );
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn contains_windows_drive_path(text: &str) -> bool {
    text.as_bytes().windows(3).any(|window| {
        window[0].is_ascii_alphabetic()
            && window[1] == b':'
            && (window[2] == b'\\' || window[2] == b'/')
    })
}

fn assert_persisted_artifact_safe(value: &Value) -> Result<(), String> {
    match value {
        Value::Object(entries) => {
            for (key, child) in entries {
                let normalized = key
                    .chars()
                    .filter(|character| character.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_ascii_lowercase();
                if normalized.starts_with("stderr")
                    || normalized.starts_with("stdout")
                    || normalized.contains("command")
                    || normalized.contains("executable")
                    || normalized.contains("provenance")
                    || normalized.starts_with("environment")
                    || normalized.starts_with("envdump")
                    || normalized.contains("authorization")
                    || normalized.contains("authheader")
                    || normalized == "headers"
                    || normalized.ends_with("headers")
                {
                    return Err(format!("unsafe persisted artifact key: {key}"));
                }
                assert_persisted_artifact_safe(child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                assert_persisted_artifact_safe(child)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    assert_no_unsafe_host_content(value)
}

fn scan_persisted_artifacts(
    report: &EvidenceReport,
    report_directory: &Path,
    canaries: &[&str],
) -> Result<(), String> {
    for artifact_path in report.checks.iter().flat_map(|check| &check.artifact_paths) {
        let relative = Path::new(artifact_path);
        if relative.is_absolute()
            || !relative.starts_with("artifacts")
            || relative
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(format!(
                "reported artifact path is not a safe relative evidence path: {artifact_path}"
            ));
        }
        let bytes = fs::read(report_directory.join(relative))
            .map_err(|error| format!("reported artifact is not readable: {error}"))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("reported artifact is not valid JSON: {error}"))?;
        assert_secret_free(&value, canaries)
            .map_err(|error| format!("reported artifact failed secret scan: {error}"))?;
        assert_persisted_artifact_safe(&value)
            .map_err(|error| format!("reported artifact failed safety scan: {error}"))?;
    }
    Ok(())
}

fn write_report_atomically(path: &Path, report: &EvidenceReport) -> Result<(), String> {
    write_json_atomically(path, report)
}

fn write_json_atomically<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("report path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create report directory: {error}"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid report file name: {}", path.display()))?;
    let temporary = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("could not serialize JSON artifact: {error}"))?;
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("could not create temporary report: {error}"))?;
        file.write_all(&bytes)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.flush())
            .map_err(|error| format!("could not write temporary report: {error}"))?;
        fs::rename(&temporary, path)
            .map_err(|error| format!("could not publish report atomically: {error}"))
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

fn persist_artifact<T: Serialize>(
    artifact_root: &Path,
    file_name: &str,
    value: &T,
) -> Result<String, String> {
    let path = artifact_root.join(file_name);
    write_json_atomically(&path, value)?;
    Ok(Path::new("artifacts")
        .join(file_name)
        .to_string_lossy()
        .into_owned())
}

fn copy_artifact(artifact_root: &Path, file_name: &str, source: &Path) -> Result<String, String> {
    let value: Value = serde_json::from_slice(
        &fs::read(source).map_err(|error| format!("could not read artifact source: {error}"))?,
    )
    .map_err(|error| format!("artifact source was not JSON: {error}"))?;
    persist_artifact(artifact_root, file_name, &value)
}

fn parse_output_path() -> Result<PathBuf, String> {
    let mut arguments = std::env::args_os().skip(1);
    let mut output = PathBuf::from(DEFAULT_REPORT_PATH);
    while let Some(argument) = arguments.next() {
        if argument == "--" {
            continue;
        }
        if argument == "--output" {
            output = arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "--output requires a path".to_string())?;
        } else {
            return Err(format!("unknown argument: {}", argument.to_string_lossy()));
        }
    }
    Ok(output)
}

type CheckEvidence = (String, Vec<String>, Vec<String>);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct FixtureFileSnapshot {
    bytes: Vec<u8>,
    sha256: String,
    length: u64,
    read_only: bool,
    unix_mode: Option<u32>,
}

fn snapshot_fixture_tree(root: &Path) -> Result<BTreeMap<PathBuf, FixtureFileSnapshot>, String> {
    fn walk(
        root: &Path,
        directory: &Path,
        snapshot: &mut BTreeMap<PathBuf, FixtureFileSnapshot>,
    ) -> Result<(), String> {
        let mut entries = fs::read_dir(directory)
            .map_err(|error| format!("could not read fixture tree: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("could not read fixture entry: {error}"))?;
        entries.sort_by_key(std::fs::DirEntry::path);
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("could not inspect fixture path: {error}"))?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "skill fixture audit refuses symlinks: {}",
                    path.display()
                ));
            }
            if metadata.is_dir() {
                walk(root, &path, snapshot)?;
                continue;
            }
            if !metadata.is_file() {
                return Err(format!(
                    "skill fixture audit found a non-file: {}",
                    path.display()
                ));
            }
            let bytes =
                fs::read(&path).map_err(|error| format!("could not read fixture file: {error}"))?;
            #[cfg(unix)]
            let unix_mode = {
                use std::os::unix::fs::PermissionsExt;
                Some(metadata.permissions().mode())
            };
            #[cfg(not(unix))]
            let unix_mode = None;
            let relative = path
                .strip_prefix(root)
                .map_err(|error| format!("fixture path escaped root: {error}"))?
                .to_path_buf();
            snapshot.insert(
                relative,
                FixtureFileSnapshot {
                    sha256: sha256_checksum(&bytes),
                    length: metadata.len(),
                    read_only: metadata.permissions().readonly(),
                    unix_mode,
                    bytes,
                },
            );
        }
        Ok(())
    }

    let mut snapshot = BTreeMap::new();
    walk(root, root, &mut snapshot)?;
    Ok(snapshot)
}

fn verify_scoped_skill_mutation(
    before: &BTreeMap<PathBuf, FixtureFileSnapshot>,
    after: &BTreeMap<PathBuf, FixtureFileSnapshot>,
    affected_path: &Path,
) -> Result<(), String> {
    let removed = before
        .keys()
        .filter(|path| !after.contains_key(*path))
        .collect::<Vec<_>>();
    if !removed.is_empty() {
        return Err(format!("skill repair removed fixture files: {removed:?}"));
    }
    let changed = before
        .iter()
        .filter_map(|(path, prior)| {
            after
                .get(path)
                .filter(|current| *current != prior)
                .map(|_| path.as_path())
        })
        .collect::<Vec<_>>();
    if changed != [affected_path] {
        return Err(format!(
            "skill repair changed files outside its scope: {changed:?}"
        ));
    }
    Ok(())
}

fn materialize_bundled_skills(project_root: &Path) -> Result<(), String> {
    fs::create_dir_all(project_root).map_err(|error| error.to_string())?;
    fs::write(
        project_root.join("AGENTS.md"),
        b"# Disposable Settings fixture\n",
    )
    .map_err(|error| error.to_string())?;
    for definition in MANDATORY_SKILLS {
        let path = project_root.join(definition.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(path, definition.bundled_content).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn run_check(
    id: &'static str,
    check: impl FnOnce() -> Result<CheckEvidence, String>,
) -> EvidenceCheck {
    let started = Instant::now();
    match check() {
        Ok((detail, artifact_paths, provenance)) => EvidenceCheck {
            id: id.to_string(),
            required: true,
            status: CheckStatus::Passed,
            duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            diagnostic_code: format!("settings.e2e.{id}.passed"),
            detail,
            artifact_paths,
            provenance,
        },
        Err(detail) => EvidenceCheck {
            id: id.to_string(),
            required: true,
            status: CheckStatus::Failed,
            duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            diagnostic_code: format!("settings.e2e.{id}.failed"),
            detail,
            artifact_paths: Vec::new(),
            provenance: vec!["local-fixture".to_string()],
        },
    }
}

fn check_health_snapshot(artifact_root: &Path) -> Result<CheckEvidence, String> {
    let root = tempfile::tempdir().map_err(|error| error.to_string())?;
    let model_root = root.path().join("models");
    let cache_root = root.path().join("cache");
    let project_root = root.path().join("project");
    fs::create_dir_all(&model_root).map_err(|error| error.to_string())?;
    fs::create_dir_all(&cache_root).map_err(|error| error.to_string())?;
    fs::create_dir_all(&project_root).map_err(|error| error.to_string())?;
    materialize_bundled_skills(&project_root)?;
    let model_state = std::sync::Mutex::new(TranscriptionModelStore::new(model_root.clone()));
    let provider_health = aggregate_provider_health(
        &[ProviderCredentialStatus {
            provider: "openai".to_string(),
            display_name: "OpenAI".to_string(),
            configured: false,
            source: ProviderCredentialSource::Missing,
        }],
        &[],
        &[],
        None,
    )
    .map_err(|error| error.to_string())?;
    let inventory = collect_storage_inventory(&model_root, &cache_root, Some(&project_root));
    let snapshot = build_settings_health_snapshot(
        &model_state,
        Some(&project_root),
        &provider_health,
        Some(&inventory),
        None,
    )
    .map_err(|error| error.to_string())?;
    let serialized = serde_json::to_value(&snapshot)
        .map_err(|error| format!("snapshot serialization failed: {error}"))?;
    let category_ids = snapshot
        .categories
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if category_ids
        != vec![
            "agent",
            "general",
            "models",
            "providers",
            "skills",
            "storage",
        ]
    {
        return Err(format!(
            "production snapshot categories were incomplete: {category_ids:?}"
        ));
    }
    if snapshot.categories["models"].state != SettingsHealthState::ActionRequired
        || snapshot.categories["skills"].state != SettingsHealthState::Ready
        || snapshot.categories["providers"].state != SettingsHealthState::Checking
        || snapshot.categories["storage"].items.len() != 6
    {
        return Err(
            "production snapshot did not preserve truthful input-derived states".to_string(),
        );
    }
    if serialized["categories"]["models"]["items"][0]["diagnosticCode"]
        != serde_json::json!("models.modelMissing")
    {
        return Err("serialized production snapshot lost missing-model diagnostics".to_string());
    }
    let projection = serde_json::json!({
        "generatedAt": snapshot.generated_at,
        "overallState": snapshot.overall,
        "categoryCount": snapshot.categories.len(),
        "categories": snapshot.categories.values().map(|category| serde_json::json!({
            "id": category.id,
            "state": category.state,
            "itemCount": category.items.len(),
            "items": category.items.iter().map(|item| serde_json::json!({
                "id": item.id,
                "state": item.state,
                "diagnosticCode": item.diagnostic_code,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    });
    let artifact = persist_artifact(artifact_root, "health-snapshot.json", &projection)?;
    Ok((
        "Serialized the production Settings health snapshot assembled from disposable model, skill, provider, render, agent, and storage inputs."
            .to_string(),
        vec![artifact],
        vec![
            "native-service:build_settings_health_snapshot".to_string(),
            "disposable-roots:tempfile".to_string(),
        ],
    ))
}

fn check_operation_journal_recovery(artifact_root: &Path) -> Result<CheckEvidence, String> {
    let root = tempfile::tempdir().map_err(|error| error.to_string())?;
    let journal = root.path().join("settings-operations.json");
    let registry = SettingsOperationRegistry::load(journal.clone(), |_| false)
        .map_err(|error| error.to_string())?;
    let operation = registry
        .start(
            SettingsOperationKind::HealthCheck,
            "settings-e2e-restart",
            Some(2),
            Some("steps".to_string()),
        )
        .map_err(|error| error.to_string())?;
    let mut running = SettingsOperationTransition::to(SettingsOperationState::Running);
    running.phase = Some("probing".to_string());
    running.completed_units = Some(1);
    running.message = Some("Fixture probe running.".to_string());
    registry
        .update(&operation.id, running)
        .map_err(|error| error.to_string())?;
    drop(registry);

    let restarted = SettingsOperationRegistry::load(journal.clone(), |_| false)
        .map_err(|error| error.to_string())?;
    let interrupted = restarted
        .get(&operation.id)
        .map_err(|error| error.to_string())?;
    if interrupted.state != SettingsOperationState::Failed
        || interrupted.phase != "interrupted"
        || interrupted.error.as_ref().map(|error| error.code.as_str())
            != Some("settings.operation.interrupted")
    {
        return Err("reloaded operation did not reconcile to interrupted failure".to_string());
    }
    let (retry, created) = restarted
        .start_or_get_active(
            SettingsOperationKind::HealthCheck,
            "settings-e2e-restart",
            Some(2),
            Some("steps".to_string()),
        )
        .map_err(|error| error.to_string())?;
    if !created || retry.id == operation.id || retry.state != SettingsOperationState::Queued {
        return Err("interrupted operation could not be retried truthfully".to_string());
    }
    let artifact = copy_artifact(artifact_root, "operation-journal.json", &journal)?;
    Ok((
        "A retained running operation reloaded as interrupted/failed and a distinct retry was queued."
            .to_string(),
        vec![artifact],
        vec!["disposable-root:tempfile".to_string()],
    ))
}

fn materialize_transcription_source(root: &Path) -> Result<(), String> {
    for required in parakeet_v3_catalog_entry().required_files {
        let path = root.join(required.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(&path, b"video-creater-local-model-fixture")
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn check_transcription_model_lifecycle(artifact_root: &Path) -> Result<CheckEvidence, String> {
    let root = tempfile::tempdir().map_err(|error| error.to_string())?;
    let source = root.path().join("local-source");
    let model_root = root.path().join("model-store");
    fs::create_dir_all(&source).map_err(|error| error.to_string())?;
    materialize_transcription_source(&source)?;
    let store = TranscriptionModelStore::new(model_root.clone());
    let operation_journal = root.path().join("model-operations.json");
    let operations = SettingsOperationRegistry::load(operation_journal.clone(), |_| false)
        .map_err(|error| error.to_string())?;
    let entry = parakeet_v3_catalog_entry();
    let operation = operations
        .start(
            SettingsOperationKind::ModelDownload,
            entry.id,
            Some(entry.required_files.len() as u64),
            Some("files".to_string()),
        )
        .map_err(|error| error.to_string())?;
    let mut running = SettingsOperationTransition::to(SettingsOperationState::Running);
    running.phase = Some("downloading".to_string());
    running.completed_units = Some(1);
    running.message = Some("Copied one local fixture file.".to_string());
    operations
        .update(&operation.id, running)
        .map_err(|error| error.to_string())?;
    let StartDownload::Started { token, .. } = store
        .mark_download_started(entry.id, entry.required_files.len() as u32)
        .map_err(|error| error.to_string())?
    else {
        return Err("initial local fixture download was not reserved".to_string());
    };
    if !store
        .update_download_progress_snapshot_if_owner(
            entry.id,
            token,
            &ModelDownloadProgressSnapshot {
                downloaded_files: 1,
                total_files: entry.required_files.len() as u32,
                downloaded_bytes: 32,
                total_bytes: 64,
            },
        )
        .map_err(|error| error.to_string())?
    {
        return Err("owner-scoped progress update was rejected".to_string());
    }
    let progress = store.status(entry.id).map_err(|error| error.to_string())?;
    if progress.install_status != ModelInstallStatus::Downloading || progress.downloaded_files != 1
    {
        return Err("download progress was not observable through model status".to_string());
    }
    let cancelled = store
        .cancel_download(entry.id)
        .map_err(|error| error.to_string())?;
    operations
        .request_cancel(&operation.id)
        .map_err(|error| error.to_string())?;
    let mut terminal_cancel = SettingsOperationTransition::to(SettingsOperationState::Cancelled);
    terminal_cancel.phase = Some("cancelled".to_string());
    terminal_cancel.message = Some("Local fixture download cancelled.".to_string());
    operations
        .update(&operation.id, terminal_cancel)
        .map_err(|error| error.to_string())?;
    if cancelled.install_status != ModelInstallStatus::Failed
        || cancelled.last_error_code.as_deref() != Some("model.download.cancelled")
    {
        return Err("cancel did not yield a typed retryable failure".to_string());
    }
    let retry = store.download_from_local_source_for_test(entry.id, &source);
    let (retry_operation, retry_created) = operations
        .start_or_get_active(
            SettingsOperationKind::ModelDownload,
            entry.id,
            Some(entry.required_files.len() as u64),
            Some("files".to_string()),
        )
        .map_err(|error| error.to_string())?;
    if !retry_created || retry_operation.id == operation.id {
        return Err("model operation journal did not create a distinct retry".to_string());
    }
    let mut retry_running = SettingsOperationTransition::to(SettingsOperationState::Running);
    retry_running.phase = Some("verifying".to_string());
    retry_running.completed_units = Some(entry.required_files.len() as u64);
    retry_running.message = Some("Local fixture files copied.".to_string());
    operations
        .update(&retry_operation.id, retry_running)
        .map_err(|error| error.to_string())?;
    #[cfg(not(target_os = "linux"))]
    {
        let ready = retry.map_err(|error| error.to_string())?;
        finish_ready_local_fixture_retry(
            artifact_root,
            &store,
            &operations,
            &operation_journal,
            &retry_operation.id,
            ready,
        )
    }
    #[cfg(target_os = "linux")]
    {
        finish_rejected_local_fixture_retry(
            artifact_root,
            &store,
            &operations,
            &operation_journal,
            &retry_operation.id,
            retry,
        )
    }
}

/// Core ML catalog entries are unpinned, so deterministic local fixture bytes install, verify,
/// publish a manifest, and become the active Ready model.
#[cfg(not(target_os = "linux"))]
fn finish_ready_local_fixture_retry(
    artifact_root: &Path,
    store: &TranscriptionModelStore,
    operations: &SettingsOperationRegistry,
    operation_journal: &Path,
    retry_operation_id: &str,
    ready: video_creater_lib::transcription::store::TranscriptionModelStatus,
) -> Result<CheckEvidence, String> {
    let entry = parakeet_v3_catalog_entry();
    if ready.install_status != ModelInstallStatus::Ready || !ready.is_active {
        return Err("local-source retry did not reach active Ready state".to_string());
    }
    let verified = store.verify(entry.id).map_err(|error| error.to_string())?;
    if verified.install_status != ModelInstallStatus::Ready || verified.verified_at.is_none() {
        return Err("downloaded local fixture did not verify".to_string());
    }
    let mut retry_succeeded = SettingsOperationTransition::to(SettingsOperationState::Succeeded);
    retry_succeeded.phase = Some("ready".to_string());
    retry_succeeded.completed_units = Some(entry.required_files.len() as u64);
    retry_succeeded.message = Some("Local fixture model verified.".to_string());
    operations
        .update(retry_operation_id, retry_succeeded)
        .map_err(|error| error.to_string())?;
    let events = operations
        .list_recent()
        .map_err(|error| error.to_string())?;
    if events.len() != 2
        || !events
            .iter()
            .any(|event| event.state == SettingsOperationState::Cancelled)
        || !events
            .iter()
            .any(|event| event.state == SettingsOperationState::Succeeded)
    {
        return Err("model operation progress/events were not journaled".to_string());
    }
    let manifest = store
        .model_dir(entry.id)
        .map_err(|error| error.to_string())?
        .join(MODEL_MANIFEST_FILE_NAME);
    if !manifest.is_file() {
        return Err("verified model manifest was not published".to_string());
    }
    let manifest_artifact = copy_artifact(artifact_root, "model-manifest.json", &manifest)?;
    let operations_artifact = copy_artifact(
        artifact_root,
        "model-operation-journal.json",
        operation_journal,
    )?;
    Ok((
        "Local-only model progress, cancel, retry, verify, manifest publication, and active readiness passed."
            .to_string(),
        vec![manifest_artifact, operations_artifact],
        vec![
            "source:deterministic-local-files".to_string(),
            "network:false".to_string(),
            "progressSnapshots:1".to_string(),
            format!("operationEvents:{}", events.len()),
        ],
    ))
}

/// The Linux sherpa-onnx catalog pins every artifact's size and SHA-256, so deterministic local
/// fixture bytes must fail closed: no Ready state, no published manifest, and a journaled failure.
#[cfg(target_os = "linux")]
fn finish_rejected_local_fixture_retry(
    artifact_root: &Path,
    store: &TranscriptionModelStore,
    operations: &SettingsOperationRegistry,
    operation_journal: &Path,
    retry_operation_id: &str,
    retry: Result<
        video_creater_lib::transcription::store::TranscriptionModelStatus,
        video_creater_lib::transcription::store::ModelStoreError,
    >,
) -> Result<CheckEvidence, String> {
    let entry = parakeet_v3_catalog_entry();
    if !entry
        .required_files
        .iter()
        .all(|file| file.size_bytes.is_some() && file.sha256.is_some())
    {
        return Err("Linux transcription catalog entry is not fully pinned".to_string());
    }
    let rejection_code = match retry {
        Err(error) => error.stable_code().to_string(),
        Ok(status) if status.install_status != ModelInstallStatus::Ready => status
            .last_error_code
            .unwrap_or_else(|| "model.download.failed".to_string()),
        Ok(_) => {
            return Err(
                "unpinned local fixture bytes reached Ready against pinned catalog".to_string(),
            )
        }
    };
    let status = store.status(entry.id).map_err(|error| error.to_string())?;
    if status.install_status == ModelInstallStatus::Ready
        || status.install_status == ModelInstallStatus::Downloading
    {
        return Err("rejected local fixture retry left the model Ready or downloading".to_string());
    }
    let verified = store.verify(entry.id).map_err(|error| error.to_string())?;
    if verified.install_status == ModelInstallStatus::Ready || verified.verified_at.is_some() {
        return Err("unpinned local fixture bytes verified against pinned catalog".to_string());
    }
    let manifest = store
        .model_dir(entry.id)
        .map_err(|error| error.to_string())?
        .join(MODEL_MANIFEST_FILE_NAME);
    if manifest.exists() {
        return Err("rejected local fixture retry published a model manifest".to_string());
    }
    let mut retry_failed = SettingsOperationTransition::to(SettingsOperationState::Failed);
    retry_failed.phase = Some("verifying".to_string());
    retry_failed.message = Some("Local fixture files failed pinned integrity checks.".to_string());
    retry_failed.error = Some(
        video_creater_lib::settings::operations::SettingsOperationError {
            code: rejection_code.clone(),
            message: "Local fixture files do not match the pinned model artifacts.".to_string(),
            recovery_action: None,
            detail: None,
        },
    );
    operations
        .update(retry_operation_id, retry_failed)
        .map_err(|error| error.to_string())?;
    let events = operations
        .list_recent()
        .map_err(|error| error.to_string())?;
    if events.len() != 2
        || !events
            .iter()
            .any(|event| event.state == SettingsOperationState::Cancelled)
        || !events
            .iter()
            .any(|event| event.state == SettingsOperationState::Failed)
    {
        return Err("model operation progress/events were not journaled".to_string());
    }
    let operations_artifact = copy_artifact(
        artifact_root,
        "model-operation-journal.json",
        operation_journal,
    )?;
    Ok((
        "Local-only model progress, cancel, retry, and pinned-integrity rejection of unpinned fixture bytes passed."
            .to_string(),
        vec![operations_artifact],
        vec![
            "source:deterministic-local-files".to_string(),
            "network:false".to_string(),
            "progressSnapshots:1".to_string(),
            "catalog:pinned-sha256".to_string(),
            format!("rejectionCode:{rejection_code}"),
            format!("operationEvents:{}", events.len()),
        ],
    ))
}

fn check_speech_readiness_separation(artifact_root: &Path) -> Result<CheckEvidence, String> {
    let root = tempfile::tempdir().map_err(|error| error.to_string())?;
    let source = root.path().join("local-source");
    fs::create_dir_all(&source).map_err(|error| error.to_string())?;
    materialize_transcription_source(&source)?;
    let transcription = TranscriptionModelStore::new(root.path().join("models"));
    let entry = parakeet_v3_catalog_entry();
    #[cfg(not(target_os = "linux"))]
    let transcription_status = transcription
        .download_from_local_source_for_test(entry.id, &source)
        .map_err(|error| error.to_string())?;
    // The pinned Linux catalog refuses fixture bytes, so only identity and path separation and
    // the independent not-ready speech state can be exercised without the real model.
    #[cfg(target_os = "linux")]
    let transcription_status =
        match transcription.download_from_local_source_for_test(entry.id, &source) {
            Ok(status) if status.install_status != ModelInstallStatus::Ready => status,
            Ok(_) => {
                return Err(
                    "unpinned local fixture bytes reached Ready against pinned catalog".to_string(),
                )
            }
            Err(_) => transcription
                .status(entry.id)
                .map_err(|error| error.to_string())?,
        };
    let speech = ProductionSpeechModelStore::new(root.path().join("models"));
    let speech_status = speech.status();
    #[cfg(not(target_os = "linux"))]
    if transcription_status.install_status != ModelInstallStatus::Ready || speech_status.ready {
        return Err("transcription readiness incorrectly implied speech readiness".to_string());
    }
    #[cfg(target_os = "linux")]
    if transcription_status.install_status == ModelInstallStatus::Ready || speech_status.ready {
        return Err(
            "transcription and speech readiness were not reported independently".to_string(),
        );
    }
    if transcription_status.runtime_id == speech_status.runtime_id
        || transcription_status.local_path == speech_status.root_path
    {
        return Err("transcription and speech readiness share an identity or path".to_string());
    }
    let artifact = persist_artifact(
        artifact_root,
        "speech-readiness.json",
        &serde_json::json!({
            "transcription": {
                "modelId": transcription_status.model_id,
                "runtimeId": transcription_status.runtime_id,
                "ready": transcription_status.install_status == ModelInstallStatus::Ready,
            },
            "productionSpeech": {
                "modelSetId": speech_status.model_set_id,
                "runtimeId": speech_status.runtime_id,
                "ready": speech_status.ready,
            }
        }),
    )?;
    #[cfg(not(target_os = "linux"))]
    let detail =
        "Ready transcription remained independent from missing production speech analysis models.";
    #[cfg(target_os = "linux")]
    let detail = "Pinned-integrity-rejected transcription and missing production speech analysis models kept distinct identities, paths, and readiness.";
    Ok((
        detail.to_string(),
        vec![artifact],
        vec![
            "disposable-root:tempfile".to_string(),
            "network:false".to_string(),
        ],
    ))
}

#[cfg(target_os = "macos")]
const NATIVE_DELIVERY_NAME: &str = "AVFoundation";
#[cfg(not(target_os = "macos"))]
const NATIVE_DELIVERY_NAME: &str = "Linux codec";

fn check_render_rollup(artifact_root: &Path) -> Result<CheckEvidence, String> {
    let health = get_render_system_health();
    let item = |id: &str| health.items.iter().find(|item| item.id == id);
    let gstreamer = item("render.gstreamerGes")
        .ok_or_else(|| "render rollup omitted GStreamer/GES".to_string())?;
    let plugin_policy = item("render.pluginPolicy")
        .ok_or_else(|| "render rollup omitted plugin policy".to_string())?;
    // Final delivery is AVFoundation on macOS and the reviewed GStreamer codec set on Linux.
    let (native_delivery_id, _) = native_delivery_capability();
    let native_delivery = item(native_delivery_id)
        .ok_or_else(|| format!("render rollup omitted {NATIVE_DELIVERY_NAME} delivery"))?;
    let _compatibility = item("render.compatibilityDecoder")
        .ok_or_else(|| "render rollup omitted compatibility decoder".to_string())?;
    let expected_composition_ready = gstreamer.state == SettingsHealthState::Ready
        && plugin_policy.state == SettingsHealthState::Ready;
    let expected_state = if expected_composition_ready {
        SettingsHealthState::Ready
    } else if gstreamer.state == SettingsHealthState::Checking
        || plugin_policy.state == SettingsHealthState::Checking
    {
        SettingsHealthState::Checking
    } else {
        SettingsHealthState::ActionRequired
    };
    if health.composition_ready != expected_composition_ready || health.state != expected_state {
        return Err("render overall severity did not follow composition readiness".to_string());
    }
    if health.native_delivery_degraded != (native_delivery.state != SettingsHealthState::Ready) {
        return Err(format!(
            "{NATIVE_DELIVERY_NAME} degradation was not reported independently"
        ));
    }
    let artifact = persist_artifact(
        artifact_root,
        "render-rollup.json",
        &serde_json::json!({
            "overallState": format!("{:?}", health.state),
            "compositionReady": health.composition_ready,
            "nativeDeliveryDegraded": health.native_delivery_degraded,
            "compatibilityDegraded": health.compatibility_degraded,
            "components": health.items.iter().map(|item| serde_json::json!({
                "id": item.id,
                "state": format!("{:?}", item.state),
                "diagnosticCode": item.diagnostic_code,
            })).collect::<Vec<_>>(),
        }),
    )?;
    Ok((
        format!(
            "GStreamer/GES composition determines render readiness; {NATIVE_DELIVERY_NAME} delivery remains an independent degradation."
        ),
        vec![artifact],
        vec![
            format!("compositionReady:{}", health.composition_ready),
            format!("nativeDeliveryDegraded:{}", health.native_delivery_degraded),
            "boundary:health-rollup-not-real-render".to_string(),
        ],
    ))
}

fn check_agent_independence(artifact_root: &Path) -> Result<CheckEvidence, String> {
    let codex = probe_codex_app_server();
    // Claude is the preferred backend, so the independence evidence has to show its row too:
    // the point of this check is that no component's state decides another's, and a
    // Codex-only record could not show that.
    let claude = probe_claude_cli(None);
    let mcp = probe_mcp_server();
    let validator = probe_proposal_validator();
    let ids = [&codex.id, &claude.id, &mcp.id, &validator.id];
    if ids
        != [
            "agent.codex",
            "agent.claude",
            "agent.mcpServer",
            "agent.proposalValidator",
        ]
    {
        return Err("agent component probes did not retain independent IDs".to_string());
    }
    if validator.state != SettingsHealthState::Ready
        || validator.diagnostic_code.as_deref() != Some("agent.proposalValidator.ready")
    {
        return Err("pure proposal validator fixture was not Ready".to_string());
    }
    let artifact = persist_artifact(
        artifact_root,
        "agent-components.json",
        &serde_json::json!({
            "components": [
                {"id": codex.id, "state": format!("{:?}", codex.state), "diagnosticCode": codex.diagnostic_code},
                {"id": claude.id, "state": format!("{:?}", claude.state), "diagnosticCode": claude.diagnostic_code},
                {"id": mcp.id, "state": format!("{:?}", mcp.state), "diagnosticCode": mcp.diagnostic_code},
                {"id": validator.id, "state": format!("{:?}", validator.state), "diagnosticCode": validator.diagnostic_code},
            ],
            "boundedTimeoutSeconds": 5,
            "rawProcessOutputIncluded": false,
        }),
    )?;
    Ok((
        "Bounded app-server and MCP probes plus the pure EDL-first validator returned independent component health."
            .to_string(),
        vec![artifact],
        vec![
            format!("codexState:{:?}", codex.state),
            format!("claudeState:{:?}", claude.state),
            format!("mcpState:{:?}", mcp.state),
            "validatorState:Ready".to_string(),
            "processTimeoutSeconds:5".to_string(),
            "processOutput:omitted".to_string(),
        ],
    ))
}

fn check_skill_scoped_repair(artifact_root: &Path) -> Result<CheckEvidence, String> {
    let root = tempfile::tempdir().map_err(|error| error.to_string())?;
    materialize_bundled_skills(root.path())?;
    let affected_id = "video-creater-graphics";
    let affected_relative = Path::new(".agents/skills/video-creater-graphics/SKILL.md");
    let affected_path = root.path().join(affected_relative);
    let corrupted_content = b"damaged local fixture\n";
    fs::write(&affected_path, corrupted_content).map_err(|error| error.to_string())?;
    let sentinel_path = root.path().join(".agents/skills/custom-sentinel/SKILL.md");
    fs::create_dir_all(sentinel_path.parent().expect("sentinel parent"))
        .map_err(|error| error.to_string())?;
    fs::write(&sentinel_path, b"custom skill sentinel\n").map_err(|error| error.to_string())?;
    let notes_path = root.path().join("custom-fixture-note.txt");
    fs::write(&notes_path, b"unaffected custom file\n").map_err(|error| error.to_string())?;
    let before_tree = snapshot_fixture_tree(root.path())?;
    let before_verification = verify_bundled_skills(root.path(), &mandatory_skill_prompt_bundle());
    let differing = before_verification
        .iter()
        .filter(|item| item.load_state == SkillLoadState::Differs)
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>();
    if differing != [affected_id] {
        return Err("fixture verification did not isolate one changed skill".to_string());
    }
    let preview = match repair_bundled_skills(root.path(), &[affected_id], None)
        .map_err(|error| error.to_string())?
    {
        SkillRepairOutcome::ConfirmationRequired(preview) => preview,
        SkillRepairOutcome::Repaired(_) => {
            return Err("mismatched skill repair skipped confirmation".to_string())
        }
    };
    if preview.affected_paths != vec![affected_path.clone()] {
        return Err("repair preview affected more than the requested skill".to_string());
    }
    let repaired = match repair_bundled_skills(
        root.path(),
        &[affected_id],
        Some(&SkillRepairConfirmation {
            affected_paths: preview.affected_paths,
        }),
    )
    .map_err(|error| error.to_string())?
    {
        SkillRepairOutcome::Repaired(report) => report,
        SkillRepairOutcome::ConfirmationRequired(_) => {
            return Err("confirmed scoped repair did not run".to_string())
        }
    };
    if repaired.repaired_paths != vec![affected_path.clone()]
        || repaired
            .verification
            .iter()
            .any(|item| item.load_state != SkillLoadState::MatchesBundled)
    {
        return Err("scoped skill repair did not restore exact bundled checksums".to_string());
    }
    let after_tree = snapshot_fixture_tree(root.path())?;
    verify_scoped_skill_mutation(&before_tree, &after_tree, affected_relative)?;
    let affected_after = after_tree
        .get(affected_relative)
        .ok_or_else(|| "repaired skill disappeared from fixture tree".to_string())?;
    let bundled_content = mandatory_skill_definition(affected_id)
        .ok_or_else(|| "affected skill definition disappeared".to_string())?
        .bundled_content
        .as_bytes();
    if affected_after.bytes != bundled_content {
        return Err("affected skill did not change from corrupted to bundled bytes".to_string());
    }
    let new_files = after_tree
        .keys()
        .filter(|path| !before_tree.contains_key(*path))
        .collect::<Vec<_>>();
    if new_files.len() != 1 {
        return Err(format!(
            "skill repair created unexpected collateral files: {new_files:?}"
        ));
    }
    let backup = new_files[0];
    let expected_backup_parent = affected_relative.parent();
    let backup_name = backup.file_name().and_then(|name| name.to_str());
    if backup.parent() != expected_backup_parent
        || !backup_name.is_some_and(|name| name.starts_with("SKILL.md.backup-"))
        || after_tree
            .get(backup)
            .is_none_or(|snapshot| snapshot.bytes != corrupted_content)
    {
        return Err("skill repair backup was not the exact corrupted target".to_string());
    }
    for unchanged in [
        Path::new("AGENTS.md"),
        Path::new(".agents/skills/video-creater-video-pipeline/SKILL.md"),
        Path::new(".agents/skills/video-creater-visuals/SKILL.md"),
        Path::new(".agents/skills/custom-sentinel/SKILL.md"),
        Path::new("custom-fixture-note.txt"),
    ] {
        if before_tree.get(unchanged) != after_tree.get(unchanged) {
            return Err(format!(
                "skill repair changed unaffected fixture bytes or metadata: {}",
                unchanged.display()
            ));
        }
    }
    let artifact = persist_artifact(
        artifact_root,
        "skill-repair.json",
        &serde_json::json!({
            "affectedSkillId": affected_id,
            "repairedCount": repaired.repaired_paths.len(),
            "independentFilesAudited": before_tree.len(),
            "collateralPreexistingChanges": 0,
            "expectedBackupCount": 1,
            "customSentinelPreserved": true,
            "allMandatorySkillsMatchBundled": repaired.verification.iter().all(|item| item.load_state == SkillLoadState::MatchesBundled),
            "verification": repaired.verification.iter().map(|item| serde_json::json!({
                "id": item.id,
                "loadState": item.load_state,
                "checksum": item.checksum,
                "bundledChecksum": item.bundled_checksum,
            })).collect::<Vec<_>>(),
        }),
    )?;
    Ok((
        "Checksum verification identified and repaired only the selected graphics skill."
            .to_string(),
        vec![artifact],
        vec!["disposable-project-root:tempfile".to_string()],
    ))
}

fn check_storage_cleanup_safety(artifact_root: &Path) -> Result<CheckEvidence, String> {
    let root = tempfile::tempdir().map_err(|error| error.to_string())?;
    let models = root.path().join("models");
    let cache = root.path().join("cache");
    let project = root.path().join("project");
    let selected = project.join("renders/render-draft-1");
    let protected_media = project.join("media/source.mov");
    fs::create_dir_all(&models).map_err(|error| error.to_string())?;
    fs::create_dir_all(&cache).map_err(|error| error.to_string())?;
    fs::create_dir_all(&selected).map_err(|error| error.to_string())?;
    fs::create_dir_all(protected_media.parent().expect("media parent"))
        .map_err(|error| error.to_string())?;
    fs::write(&protected_media, b"protected-user-media").map_err(|error| error.to_string())?;
    fs::write(selected.join("output.mp4"), b"disposable-draft")
        .map_err(|error| error.to_string())?;
    let inventory = collect_storage_inventory(&models, &cache, Some(&project));
    if inventory.len() != 6
        || inventory
            .iter()
            .find(|item| item.id == "storage.projectMedia")
            .is_none_or(|item| item.removable)
    {
        return Err("storage inventory did not retain protected media scope".to_string());
    }
    let traversal = StorageCleanupTarget::ProjectRenderArtifacts {
        artifact_ids: vec!["../media".to_string()],
    };
    let protected_error = preview_storage_cleanup(&traversal, &cache, Some(&project))
        .expect_err("parent traversal cleanup target must be refused");
    if protected_error.code() != "settings.storage.cleanupInvalidArtifactId" {
        return Err("non-allowlisted cleanup target returned the wrong refusal".to_string());
    }
    let target = StorageCleanupTarget::ProjectRenderArtifacts {
        artifact_ids: vec!["render-draft-1".to_string()],
    };
    let preview = preview_storage_cleanup(&target, &cache, Some(&project))
        .map_err(|error| error.to_string())?;
    fs::write(selected.join("changed-after-preview.bin"), b"stale")
        .map_err(|error| error.to_string())?;
    let stale = run_storage_cleanup(
        &target,
        &preview.confirmation_token,
        &cache,
        Some(&project),
        |_| Ok(()),
    )
    .expect_err("stale cleanup confirmation must be refused");
    if stale.code() != "settings.storage.cleanupConfirmationMismatch"
        || !selected.is_dir()
        || !protected_media.is_file()
    {
        return Err("stale cleanup refusal did not preserve project data".to_string());
    }
    let artifact = persist_artifact(
        artifact_root,
        "storage-safety.json",
        &serde_json::json!({
            "inventoryItemCount": inventory.len(),
            "protectedTargetRefusal": protected_error.code(),
            "stalePreviewRefusal": stale.code(),
            "renderArtifactPreserved": selected.is_dir(),
            "projectMediaPreserved": protected_media.is_file(),
        }),
    )?;
    Ok((
        "Inventory completed; traversal and stale-preview cleanup were refused without deleting render or media data."
            .to_string(),
        vec![artifact],
        vec!["disposable-project-root:tempfile".to_string()],
    ))
}

#[derive(Default)]
struct IsolatedAcceptanceKeychain {
    items: std::sync::Mutex<BTreeMap<String, Vec<u8>>>,
}

impl CredentialStore for IsolatedAcceptanceKeychain {
    fn get(&self, account: &str) -> Result<Option<Vec<u8>>, CredentialStoreError> {
        self.items
            .lock()
            .map_err(|_| CredentialStoreError::Unavailable("acceptance store poisoned".to_string()))
            .map(|items| items.get(account).cloned())
    }

    fn set(&self, account: &str, credential: &[u8]) -> Result<(), CredentialStoreError> {
        self.items
            .lock()
            .map_err(|_| {
                CredentialStoreError::Unavailable("acceptance store poisoned".to_string())
            })?
            .insert(account.to_string(), credential.to_vec());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), CredentialStoreError> {
        self.items
            .lock()
            .map_err(|_| {
                CredentialStoreError::Unavailable("acceptance store poisoned".to_string())
            })?
            .remove(account);
        Ok(())
    }
}

fn check_provider_secret_redaction(artifact_root: &Path) -> Result<CheckEvidence, String> {
    const KEYCHAIN_CANARY: &str = "fixture-keychain-secret-61a0";
    const SECOND_KEYCHAIN_CANARY: &str = "fixture-keychain-token-bb20";
    let keychain = IsolatedAcceptanceKeychain::default();
    set_provider_credential_with_store(&keychain, "openai", KEYCHAIN_CANARY)
        .map_err(|error| error.to_string())?;
    set_provider_credential_with_store(&keychain, "xai", SECOND_KEYCHAIN_CANARY)
        .map_err(|error| error.to_string())?;
    let credentials = list_provider_credential_statuses_with_store(&keychain);
    for provider in ["openai", "xai"] {
        let status = provider_credential_status_with_store(&keychain, provider)
            .map_err(|error| error.to_string())?;
        if !status.configured || status.source != ProviderCredentialSource::Keychain {
            return Err(format!(
                "isolated Keychain status was not configured: {provider}"
            ));
        }
    }
    let validations = vec![
        ProviderAccountValidation {
            provider: "openai".to_string(),
            validation_state: ProviderValidationState::Available,
            account_label: Some("Fixture account".to_string()),
            balance_label: None,
            last_checked_at: Some("2026-07-17T00:00:00Z".to_string()),
            diagnostic_code: None,
        },
        ProviderAccountValidation {
            provider: "xai".to_string(),
            validation_state: ProviderValidationState::BalanceUnavailable,
            account_label: None,
            balance_label: None,
            last_checked_at: Some("2026-07-17T00:00:00Z".to_string()),
            diagnostic_code: Some("providers.balanceUnavailable".to_string()),
        },
    ];
    let dependencies = vec![
        ProviderModelDependency::new("openai", "openai:gpt-image-2"),
        ProviderModelDependency::new("xai", "xai:grok-imagine"),
    ];
    let health = aggregate_provider_health(&credentials, &validations, &dependencies, None)
        .map_err(|error| error.to_string())?;
    let serialized = serde_json::to_value(&health).map_err(|error| error.to_string())?;
    assert_secret_free(&serialized, &[KEYCHAIN_CANARY, SECOND_KEYCHAIN_CANARY])?;
    let serialized_text = serialized.to_string();
    if serialized_text.contains("envVar") || serialized_text.contains("auth") {
        return Err("provider health exposed credential metadata or auth headers".to_string());
    }
    for provider in ["openai", "xai"] {
        let deleted = delete_provider_credential_with_store(&keychain, provider)
            .map_err(|error| error.to_string())?;
        if deleted.configured || deleted.source != ProviderCredentialSource::Missing {
            return Err(format!(
                "isolated Keychain item was not deleted: {provider}"
            ));
        }
    }
    let artifact = persist_artifact(artifact_root, "provider-health.json", &serialized)?;
    Ok((
        "Provider health preserved source/readiness metadata while omitting credential values and headers."
            .to_string(),
        vec![artifact],
        vec![
            "credentialFixtures:isolated-acceptance-keychain".to_string(),
            "keychainMutation:true".to_string(),
            "credentialRoundTrip:save-list-status-delete".to_string(),
            "environmentMutation:false".to_string(),
        ],
    ))
}

fn run_all_checks(artifact_root: &Path) -> Vec<EvidenceCheck> {
    vec![
        run_check(REQUIRED_CHECK_IDS[0], || {
            check_health_snapshot(artifact_root)
        }),
        run_check(REQUIRED_CHECK_IDS[1], || {
            check_operation_journal_recovery(artifact_root)
        }),
        run_check(REQUIRED_CHECK_IDS[2], || {
            check_transcription_model_lifecycle(artifact_root)
        }),
        run_check(REQUIRED_CHECK_IDS[3], || {
            check_speech_readiness_separation(artifact_root)
        }),
        run_check(REQUIRED_CHECK_IDS[4], || check_render_rollup(artifact_root)),
        run_check(REQUIRED_CHECK_IDS[5], || {
            check_agent_independence(artifact_root)
        }),
        run_check(REQUIRED_CHECK_IDS[6], || {
            check_skill_scoped_repair(artifact_root)
        }),
        run_check(REQUIRED_CHECK_IDS[7], || {
            check_storage_cleanup_safety(artifact_root)
        }),
        run_check(REQUIRED_CHECK_IDS[8], || {
            check_provider_secret_redaction(artifact_root)
        }),
    ]
}

fn main() {
    let output = match parse_output_path() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let artifact_root = output
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("artifacts");
    let report = build_report(run_all_checks(&artifact_root));
    let value = match serde_json::to_value(&report) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("Settings evidence report serialization failed: {error}");
            std::process::exit(2);
        }
    };
    if let Err(error) = assert_secret_free(
        &value,
        &[
            "fixture-keychain-secret-61a0",
            "fixture-environment-token-bb20",
        ],
    ) {
        eprintln!("Settings evidence report secret scan failed: {error}");
        std::process::exit(2);
    }
    if let Err(error) = assert_no_unsafe_host_content(&value) {
        eprintln!("Settings evidence report safety scan failed: {error}");
        std::process::exit(2);
    }
    if let Err(error) = scan_persisted_artifacts(
        &report,
        output.parent().unwrap_or_else(|| Path::new(".")),
        &[
            "fixture-keychain-secret-61a0",
            "fixture-environment-token-bb20",
        ],
    ) {
        eprintln!("Settings evidence artifact secret scan failed: {error}");
        std::process::exit(2);
    }
    if let Err(error) = write_report_atomically(&output, &report) {
        eprintln!("Settings evidence report write failed: {error}");
        std::process::exit(2);
    }
    println!("Settings readiness report: {}", output.display());
    if !report.passed {
        for check in report
            .checks
            .iter()
            .filter(|check| check.required && check.status == CheckStatus::Failed)
        {
            eprintln!("{}: {}", check.id, check.detail);
        }
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(id: &str, required: bool, status: CheckStatus) -> EvidenceCheck {
        EvidenceCheck {
            id: id.to_string(),
            required,
            status,
            duration_ms: 1,
            diagnostic_code: "fixture.check".to_string(),
            detail: "Fixture evidence.".to_string(),
            artifact_paths: Vec::new(),
            provenance: vec!["local-fixture".to_string()],
        }
    }

    #[test]
    fn report_aggregation_requires_every_stable_check_id_once() {
        let checks = REQUIRED_CHECK_IDS
            .iter()
            .map(|id| check(id, true, CheckStatus::Passed))
            .collect();

        let report = build_report(checks);

        assert!(report.passed);
        assert_eq!(report.schema, REPORT_SCHEMA);
        assert_eq!(report.version, REPORT_VERSION);
        assert_eq!(report.checks.len(), REQUIRED_CHECK_IDS.len());
    }

    #[test]
    fn required_failure_sets_nonzero_policy_even_when_optional_checks_pass() {
        let report = build_report(vec![
            check("required", true, CheckStatus::Failed),
            check("optional", false, CheckStatus::Passed),
        ]);

        assert!(!required_checks_passed(&report));
        assert!(!report.passed);
    }

    #[test]
    fn recursive_secret_scan_rejects_secret_keys_headers_and_canaries() {
        let canary = "fixture-secret-canary-7c92";
        for value in [
            serde_json::json!({"nested": [{"credentialValue": "redacted"}]}),
            serde_json::json!({"authorization": "Bearer redacted"}),
            serde_json::json!({"safe": format!("prefix-{canary}-suffix")}),
        ] {
            assert!(assert_secret_free(&value, &[canary]).is_err());
        }
        assert!(assert_secret_free(
            &serde_json::json!({"credentialSource": "keychain", "configured": true}),
            &[canary],
        )
        .is_ok());
    }

    #[test]
    fn atomic_report_write_creates_parent_and_leaves_no_temporary_sibling() {
        let root = tempfile::tempdir().expect("report root");
        let path = root.path().join("nested/report.json");
        let report = build_report(
            REQUIRED_CHECK_IDS
                .iter()
                .map(|id| check(id, true, CheckStatus::Passed))
                .collect(),
        );

        write_report_atomically(&path, &report).expect("atomic report write");

        let persisted: EvidenceReport =
            serde_json::from_slice(&std::fs::read(&path).expect("persisted report"))
                .expect("valid report JSON");
        assert_eq!(persisted, report);
        let siblings = std::fs::read_dir(path.parent().expect("parent"))
            .expect("read parent")
            .collect::<Result<Vec<_>, _>>()
            .expect("read entries");
        assert_eq!(siblings.len(), 1);
    }

    #[test]
    fn native_health_snapshot_check_serializes_all_categories() {
        let root = tempfile::tempdir().expect("artifact root");
        check_health_snapshot(root.path()).expect("health snapshot evidence");
    }

    #[test]
    fn native_operation_check_recovers_and_retries_interruption() {
        let root = tempfile::tempdir().expect("artifact root");
        check_operation_journal_recovery(root.path()).expect("operation recovery evidence");
    }

    #[test]
    fn native_transcription_check_exercises_local_lifecycle_and_events() {
        let root = tempfile::tempdir().expect("artifact root");
        check_transcription_model_lifecycle(root.path()).expect("transcription lifecycle evidence");
    }

    #[test]
    fn native_speech_check_keeps_readiness_separate() {
        let root = tempfile::tempdir().expect("artifact root");
        check_speech_readiness_separation(root.path()).expect("speech readiness evidence");
    }

    #[test]
    fn native_render_check_enforces_rollup_invariants() {
        let root = tempfile::tempdir().expect("artifact root");
        check_render_rollup(root.path()).expect("render rollup evidence");
    }

    #[test]
    fn native_agent_check_keeps_component_health_independent() {
        let root = tempfile::tempdir().expect("artifact root");
        check_agent_independence(root.path()).expect("agent component evidence");
    }

    #[test]
    fn native_skill_check_repairs_only_the_affected_skill() {
        let root = tempfile::tempdir().expect("artifact root");
        check_skill_scoped_repair(root.path()).expect("skill repair evidence");
    }

    #[test]
    fn native_storage_check_refuses_unsafe_and_stale_cleanup() {
        let root = tempfile::tempdir().expect("artifact root");
        check_storage_cleanup_safety(root.path()).expect("storage safety evidence");
    }

    #[test]
    fn native_provider_check_serializes_without_secrets() {
        let root = tempfile::tempdir().expect("artifact root");
        let (_detail, artifacts, provenance) =
            check_provider_secret_redaction(root.path()).expect("provider redaction evidence");

        assert!(provenance.contains(&"keychainMutation:true".to_string()));
        assert!(provenance.contains(&"credentialRoundTrip:save-list-status-delete".to_string()));
        let artifact_name = Path::new(&artifacts[0])
            .file_name()
            .expect("provider health artifact name");
        let artifact = std::fs::read_to_string(root.path().join(artifact_name))
            .expect("provider health artifact");
        assert!(!artifact.contains("fixture-keychain-secret-61a0"));
    }

    #[test]
    fn every_reported_artifact_remains_readable_after_checks_return() {
        let root = tempfile::tempdir().expect("artifact root");
        let checks = run_all_checks(&root.path().join("artifacts"));

        for artifact in checks.iter().flat_map(|check| &check.artifact_paths) {
            assert!(
                root.path().join(artifact).is_file(),
                "reported artifact must remain readable: {artifact}"
            );
        }
    }

    #[test]
    fn scoped_skill_mutation_audit_rejects_collateral_custom_file_changes() {
        let root = tempfile::tempdir().expect("skill audit root");
        materialize_bundled_skills(root.path()).expect("bundled skills");
        let affected = root
            .path()
            .join(".agents/skills/video-creater-graphics/SKILL.md");
        std::fs::write(&affected, b"corrupted fixture\n").expect("corrupt target");
        let sentinel = root.path().join(".agents/skills/custom/SKILL.md");
        std::fs::create_dir_all(sentinel.parent().expect("sentinel parent"))
            .expect("sentinel parent");
        std::fs::write(&sentinel, b"custom sentinel\n").expect("sentinel");
        let before = snapshot_fixture_tree(root.path()).expect("before snapshot");
        std::fs::write(
            &affected,
            mandatory_skill_definition("video-creater-graphics")
                .unwrap()
                .bundled_content,
        )
        .expect("repair target");
        std::fs::write(&sentinel, b"collateral mutation\n").expect("mutate sentinel");
        let after = snapshot_fixture_tree(root.path()).expect("after snapshot");

        assert!(verify_scoped_skill_mutation(
            &before,
            &after,
            Path::new(".agents/skills/video-creater-graphics/SKILL.md"),
        )
        .is_err());
    }

    #[test]
    fn persisted_artifact_scan_rejects_nested_secret_content() {
        let root = tempfile::tempdir().expect("artifact scan root");
        let artifact = root.path().join("artifacts/provider.json");
        std::fs::create_dir_all(artifact.parent().expect("artifact parent"))
            .expect("artifact parent");
        std::fs::write(
            &artifact,
            br#"{"nested":{"authorization":"Bearer fixture"}}"#,
        )
        .expect("secret artifact");
        let report = report_with_artifact("provider.json");

        assert!(scan_persisted_artifacts(&report, root.path(), &[]).is_err());
    }

    #[test]
    fn persisted_artifact_scan_rejects_raw_process_output_and_command_provenance() {
        let root = tempfile::tempdir().expect("artifact scan root");
        let artifact = root.path().join("artifacts/health.json");
        std::fs::create_dir_all(artifact.parent().expect("artifact parent"))
            .expect("artifact parent");
        std::fs::write(
            &artifact,
            br#"{"stderr":"raw helper failure","provenance":{"command":"/Users/example/bin/helper --probe","executable":"/Users/example/bin/helper"}}"#,
        )
        .expect("unsafe artifact");
        let report = report_with_artifact("health.json");

        assert!(scan_persisted_artifacts(&report, root.path(), &[]).is_err());
    }

    #[test]
    fn persisted_artifact_scan_rejects_absolute_host_paths() {
        let root = tempfile::tempdir().expect("artifact scan root");
        let artifact = root.path().join("artifacts/storage.json");
        std::fs::create_dir_all(artifact.parent().expect("artifact parent"))
            .expect("artifact parent");
        std::fs::write(
            &artifact,
            br#"{"detail":"fixture escaped through /home/example/project and C:\\Users\\example\\token"}"#,
        )
        .expect("unsafe artifact");
        let report = report_with_artifact("storage.json");

        assert!(scan_persisted_artifacts(&report, root.path(), &[]).is_err());
    }

    #[test]
    fn check_artifact_references_are_relative_to_the_report_directory() {
        let root = tempfile::tempdir().expect("artifact root");
        let checks = run_all_checks(&root.path().join("artifacts"));

        assert!(checks
            .iter()
            .flat_map(|check| &check.artifact_paths)
            .all(|path| !Path::new(path).is_absolute() && path.starts_with("artifacts/")));
    }

    fn report_with_artifact(file_name: &str) -> EvidenceReport {
        build_report(vec![EvidenceCheck {
            id: REQUIRED_CHECK_IDS[0].to_string(),
            required: true,
            status: CheckStatus::Passed,
            duration_ms: 0,
            diagnostic_code: "fixture".to_string(),
            detail: "fixture".to_string(),
            artifact_paths: vec![format!("artifacts/{file_name}")],
            provenance: Vec::new(),
        }])
    }
}
