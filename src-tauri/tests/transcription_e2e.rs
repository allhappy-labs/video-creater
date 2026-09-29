use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;
#[cfg(target_os = "macos")]
use std::{env, fs};

#[cfg(target_os = "macos")]
use serde_json::{json, Value};
use video_creater_lib::project::model::MediaKind;
#[cfg(target_os = "macos")]
use video_creater_lib::project::{
    model::{MediaAsset, VideoProject},
    split::{load_split_project, save_split_project},
};
#[cfg(target_os = "macos")]
use video_creater_lib::transcription::{
    fluidaudio::FluidAudioCoreMlRuntime,
    model::{
        parakeet_v3_catalog_entry, ModelInstallStatus, FLUID_AUDIO_COREML_RUNTIME_ID,
        FLUID_AUDIO_COREML_RUNTIME_MODEL_DIR_NAME,
    },
    runtime::TranscriptionRuntimeRegistry,
    store::TranscriptionModelStore,
};
#[cfg(target_os = "macos")]
use video_creater_lib::workflows::{
    temporal_transcribe_probe_media_activity_value_with_store_and_registry,
    temporal_transcribe_run_activity_value_with_runtime_registry_and_store,
    temporal_transcribe_store_activity_value,
};

const MODEL_ROOT_ENV: &str = "VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT";
const DOWNLOAD_MODEL_ENV: &str = "VIDEO_CREATER_TEST_DOWNLOAD_MODEL";
const WAV_ENV: &str = "VIDEO_CREATER_TEST_TRANSCRIPTION_WAV";
const MEDIA_ENV: &str = "VIDEO_CREATER_TEST_TRANSCRIPTION_MEDIA";
const HELPER_ENV: &str = "VIDEO_CREATER_TEST_FLUIDAUDIO_HELPER";
const EDISON_SPEECH_FIXTURE_RELATIVE_PATH: &str =
    "tests/fixtures/media/edison-speech-1920s-30s.mp4";

#[test]
fn real_fluidaudio_temporal_transcription_e2e() {
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("skipping real FluidAudio transcription E2E: macOS is required");
    }

    #[cfg(target_os = "macos")]
    {
        run_real_fluidaudio_temporal_transcription_e2e();
    }
}

#[test]
fn transcription_e2e_env_prefers_wav_fixture_over_generic_media() {
    let vars = E2eEnvVars {
        model_root: Some(PathBuf::from("/tmp/model")),
        wav: Some(PathBuf::from("/tmp/audio.wav")),
        media: Some(PathBuf::from("/tmp/video.mov")),
        helper: None,
        download_model: false,
    };

    let E2eEnvState::Requested(config) = vars.into_state() else {
        panic!("expected requested env config");
    };

    assert_eq!(config.media_path, PathBuf::from("/tmp/audio.wav"));
    assert_eq!(config.media_kind, MediaKind::Audio);
}

#[test]
fn transcription_e2e_checked_in_speech_fixture_is_available() {
    let fixture = checked_in_speech_fixture_path();

    assert!(
        fixture.is_file(),
        "fixture is available: {}",
        fixture.display()
    );
    assert_eq!(media_kind_for_path(&fixture), MediaKind::Video);
}

#[test]
fn transcription_e2e_env_absent_is_not_requested() {
    let vars = E2eEnvVars {
        model_root: None,
        wav: None,
        media: None,
        helper: None,
        download_model: false,
    };

    assert!(matches!(vars.into_state(), E2eEnvState::NotRequested));
}

#[test]
fn transcription_e2e_env_partial_config_is_requested_error() {
    let vars = E2eEnvVars {
        model_root: Some(PathBuf::from("/tmp/model")),
        wav: None,
        media: None,
        helper: None,
        download_model: false,
    };

    let E2eEnvState::Misconfigured(message) = vars.into_state() else {
        panic!("expected requested-but-misconfigured env");
    };

    assert!(message.contains(WAV_ENV));
    assert!(message.contains(MEDIA_ENV));
}

#[test]
fn transcription_e2e_env_helper_alone_is_requested_error() {
    let vars = E2eEnvVars {
        model_root: None,
        wav: None,
        media: None,
        helper: Some(PathBuf::from("/tmp/helper")),
        download_model: false,
    };

    let E2eEnvState::Misconfigured(message) = vars.into_state() else {
        panic!("expected helper-only env to be requested but misconfigured");
    };

    assert!(message.contains(MODEL_ROOT_ENV));
}

#[test]
fn transcription_e2e_env_blank_values_are_requested_errors() {
    let cases = [
        (
            MODEL_ROOT_ENV,
            E2eEnvVars {
                model_root: Some(PathBuf::new()),
                wav: Some(PathBuf::from("/tmp/audio.wav")),
                media: None,
                helper: None,
                download_model: false,
            },
        ),
        (
            WAV_ENV,
            E2eEnvVars {
                model_root: Some(PathBuf::from("/tmp/model")),
                wav: Some(PathBuf::new()),
                media: None,
                helper: None,
                download_model: false,
            },
        ),
        (
            MEDIA_ENV,
            E2eEnvVars {
                model_root: Some(PathBuf::from("/tmp/model")),
                wav: None,
                media: Some(PathBuf::new()),
                helper: None,
                download_model: false,
            },
        ),
        (
            HELPER_ENV,
            E2eEnvVars {
                model_root: Some(PathBuf::from("/tmp/model")),
                wav: Some(PathBuf::from("/tmp/audio.wav")),
                media: None,
                helper: Some(PathBuf::new()),
                download_model: false,
            },
        ),
    ];

    for (env_name, vars) in cases {
        let E2eEnvState::Misconfigured(message) = vars.into_state() else {
            panic!("expected blank {env_name} to be misconfigured");
        };

        assert!(
            message.contains(env_name),
            "misconfiguration message should name {env_name}: {message}"
        );
    }
}

#[test]
fn transcription_e2e_download_mode_does_not_require_model_root() {
    let vars = E2eEnvVars {
        model_root: None,
        wav: Some(PathBuf::from("/tmp/audio.wav")),
        media: None,
        helper: None,
        download_model: true,
    };

    let E2eEnvState::Requested(config) = vars.into_state() else {
        panic!("expected requested download config");
    };

    assert!(config.download_model);
    assert_eq!(config.media_path, PathBuf::from("/tmp/audio.wav"));
}

#[test]
fn transcription_e2e_download_mode_still_requires_media_fixture() {
    let vars = E2eEnvVars {
        model_root: None,
        wav: None,
        media: None,
        helper: Some(PathBuf::from("/tmp/helper")),
        download_model: true,
    };

    let E2eEnvState::Misconfigured(message) = vars.into_state() else {
        panic!("expected download mode without media to be misconfigured");
    };

    assert!(message.contains(WAV_ENV));
    assert!(message.contains(MEDIA_ENV));
}

#[test]
fn native_runtime_selection_uses_helper_for_verified_model_path() {
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("skipping native runtime helper selection test: macOS is required");
    }

    #[cfg(target_os = "macos")]
    {
        let project_dir = tempfile::tempdir().expect("project dir");
        let model_store_root = tempfile::tempdir().expect("temp global model root");
        let model_source = tempfile::tempdir().expect("model source");
        let helper_dir = tempfile::tempdir().expect("helper dir");
        write_minimal_coreml_source(model_source.path());
        let helper_path = helper_dir
            .path()
            .join("video-creater-fluidaudio-transcribe");
        write_fake_fluidaudio_helper(&helper_path);

        let source_media_path = project_dir.path().join("media/helper-selection.wav");
        fs::create_dir_all(source_media_path.parent().expect("media parent")).expect("media dir");
        fs::write(&source_media_path, b"fake wav").expect("write media fixture");
        let project = e2e_project(&source_media_path, MediaKind::Audio);
        save_split_project(project_dir.path(), &project).expect("save split project");

        let entry = parakeet_v3_catalog_entry();
        let store = TranscriptionModelStore::new(model_store_root.path().to_path_buf());
        let status = store
            .import_model(entry.id, model_source.path())
            .expect("import fake verified Core ML model");
        assert_eq!(status.install_status, ModelInstallStatus::Ready);
        store.set_active_model(entry.id).expect("set active model");

        let registry = TranscriptionRuntimeRegistry::new(vec![Box::new(
            FluidAudioCoreMlRuntime::new(helper_path),
        )]);
        let job_id = "helper-selection-e2e";
        let probe_output = temporal_transcribe_probe_media_activity_value_with_store_and_registry(
            json!({
                "projectId": project.id,
                "projectDir": project_dir.path().display().to_string(),
                "mediaId": "media-real-e2e",
                "jobId": job_id,
                "languageMode": "en"
            }),
            &store,
            &registry,
        )
        .expect("probe media with verified fake model and helper");
        assert_eq!(probe_output["runtimeId"], FLUID_AUDIO_COREML_RUNTIME_ID);

        let artifact_dir = project_dir
            .path()
            .join(format!("workflow-artifacts/{job_id}"));
        let run_output = temporal_transcribe_run_activity_value_with_runtime_registry_and_store(
            probe_output,
            &registry,
            &store,
            &artifact_dir,
        )
        .expect("run helper-backed native transcription");

        assert_eq!(run_output["runtimeId"], FLUID_AUDIO_COREML_RUNTIME_ID);
        assert_eq!(run_output["tokenCount"], 2);
        assert_eq!(
            run_output["nativeOutput"]["engine"],
            FLUID_AUDIO_COREML_RUNTIME_ID
        );
        assert_eq!(
            run_output["nativeOutput"]["rawArtifactPath"],
            artifact_dir
                .join(format!("{job_id}-transcript.json"))
                .display()
                .to_string()
        );
        assert!(
            !run_output
                .to_string()
                .contains("Core ML Parakeet decode loop is awaiting model IO wiring"),
            "verified helper-backed path must not use the legacy stub"
        );
    }
}

#[cfg(target_os = "macos")]
fn run_real_fluidaudio_temporal_transcription_e2e() {
    let config = match E2eEnvVars::from_process_env().into_state() {
        E2eEnvState::NotRequested => {
            eprintln!(
                "skipping real FluidAudio transcription E2E: set {DOWNLOAD_MODEL_ENV}=1 or {MODEL_ROOT_ENV}, plus either {WAV_ENV} or {MEDIA_ENV}"
            );
            return;
        }
        E2eEnvState::Misconfigured(message) => {
            panic!("real FluidAudio transcription E2E is misconfigured: {message}");
        }
        E2eEnvState::Requested(config) => config,
    };

    if let Some(model_root) = &config.model_root {
        assert!(
            model_root.is_dir(),
            "{MODEL_ROOT_ENV} must point to a model directory: {}",
            model_root.display()
        );
    }
    assert!(
        config.media_path.is_file(),
        "transcription E2E media fixture must be a file: {}",
        config.media_path.display()
    );

    let helper_path = config
        .helper_path
        .clone()
        .unwrap_or_else(FluidAudioCoreMlRuntime::default_helper_path);
    assert!(
        is_executable_file(&helper_path),
        "{HELPER_ENV} or the default FluidAudio helper must point to an executable file: {}",
        helper_path.display()
    );

    let project_dir = tempfile::tempdir().expect("project dir");
    let model_store_root = tempfile::tempdir().expect("temp global model root");
    let source_media_path = copy_media_fixture(project_dir.path(), &config.media_path);
    let project = e2e_project(&source_media_path, config.media_kind);
    save_split_project(project_dir.path(), &project).expect("save split project");

    let entry = parakeet_v3_catalog_entry();
    let store = TranscriptionModelStore::new(model_store_root.path().to_path_buf());
    let status = if config.download_model {
        store
            .download(entry.id)
            .expect("download FluidAudio/Core ML model")
    } else {
        let model_source = importable_model_source_dir(
            config
                .model_root
                .as_ref()
                .expect("model root required without download mode"),
        );
        store
            .import_model(entry.id, &model_source)
            .expect("import local FluidAudio/Core ML model")
    };
    assert_eq!(status.install_status, ModelInstallStatus::Ready);
    assert_eq!(status.model_id, entry.id);
    let active_status = store.set_active_model(entry.id).expect("set active model");
    assert_eq!(active_status.install_status, ModelInstallStatus::Ready);

    let registry = TranscriptionRuntimeRegistry::new(vec![Box::new(FluidAudioCoreMlRuntime::new(
        helper_path,
    ))]);
    let job_id = "real-fluidaudio-e2e";
    let probe_output = temporal_transcribe_probe_media_activity_value_with_store_and_registry(
        json!({
            "projectId": project.id,
            "projectDir": project_dir.path().display().to_string(),
            "mediaId": "media-real-e2e",
            "jobId": job_id,
            "languageMode": "en"
        }),
        &store,
        &registry,
    )
    .expect("probe media with installed global model");
    assert_eq!(probe_output["status"], "ready");
    assert_eq!(probe_output["modelId"], entry.id);
    assert_eq!(probe_output["runtimeId"], FLUID_AUDIO_COREML_RUNTIME_ID);

    let artifact_dir = project_dir
        .path()
        .join(format!("workflow-artifacts/{job_id}"));
    let run_output = temporal_transcribe_run_activity_value_with_runtime_registry_and_store(
        probe_output,
        &registry,
        &store,
        &artifact_dir,
    )
    .expect("run real FluidAudio transcription");
    assert_eq!(run_output["runtimeId"], FLUID_AUDIO_COREML_RUNTIME_ID);
    assert!(
        run_output["tokenCount"].as_u64().unwrap_or_default() > 0,
        "real transcription should produce timed tokens"
    );

    let store_output =
        temporal_transcribe_store_activity_value(run_output).expect("store transcript");
    assert_eq!(store_output["projectId"], "transcription-e2e-project");
    assert_eq!(store_output["mediaId"], "media-real-e2e");
    assert!(
        store_output["wordCount"].as_u64().unwrap_or_default() > 0,
        "store activity reports stored words"
    );

    let saved_project = load_split_project(project_dir.path()).expect("load stored project");
    let transcript = saved_project
        .transcripts
        .iter()
        .find(|transcript| transcript.media_id == "media-real-e2e")
        .expect("stored transcript for media");
    assert!(!transcript.words.is_empty(), "stored transcript has words");
    assert!(
        transcript.words.iter().all(|word| {
            word.start_seconds.is_finite()
                && word.end_seconds.is_finite()
                && word.end_seconds >= word.start_seconds
        }),
        "stored transcript words have valid timings"
    );

    let raw_artifact_path = transcript
        .raw_artifact_path
        .as_ref()
        .expect("transcript raw artifact path");
    let artifact_path = project_dir.path().join(raw_artifact_path);
    assert!(artifact_path.is_file(), "raw transcript artifact exists");
    let artifact_json: Value = serde_json::from_str(
        &fs::read_to_string(&artifact_path).expect("read transcript artifact"),
    )
    .expect("parse transcript artifact");
    let tokens = artifact_json["tokens"]
        .as_array()
        .expect("artifact tokens array");
    assert!(!tokens.is_empty(), "artifact has tokens");
    assert!(
        tokens.iter().all(|token| {
            token.get("start").and_then(Value::as_f64).is_some()
                && token.get("end").and_then(Value::as_f64).is_some()
        }),
        "artifact tokens include start and end timings"
    );
}

struct E2eEnvVars {
    model_root: Option<PathBuf>,
    wav: Option<PathBuf>,
    media: Option<PathBuf>,
    helper: Option<PathBuf>,
    download_model: bool,
}

impl E2eEnvVars {
    #[cfg(target_os = "macos")]
    fn from_process_env() -> Self {
        Self {
            model_root: env_path(MODEL_ROOT_ENV),
            wav: env_path(WAV_ENV),
            media: env_path(MEDIA_ENV),
            helper: env_path(HELPER_ENV),
            download_model: env_flag(DOWNLOAD_MODEL_ENV),
        }
    }

    fn into_state(self) -> E2eEnvState {
        let requested = self.model_root.is_some()
            || self.wav.is_some()
            || self.media.is_some()
            || self.helper.is_some()
            || self.download_model;
        if !requested {
            return E2eEnvState::NotRequested;
        }

        if let Some(message) = blank_env_error(MODEL_ROOT_ENV, &self.model_root)
            .or_else(|| blank_env_error(WAV_ENV, &self.wav))
            .or_else(|| blank_env_error(MEDIA_ENV, &self.media))
            .or_else(|| blank_env_error(HELPER_ENV, &self.helper))
        {
            return E2eEnvState::Misconfigured(message);
        }

        if !self.download_model && self.model_root.is_none() {
            return E2eEnvState::Misconfigured(format!(
                "set {MODEL_ROOT_ENV} or {DOWNLOAD_MODEL_ENV}=1"
            ));
        }

        if let Some(media_path) = self.wav {
            return E2eEnvState::Requested(E2eConfig {
                model_root: self.model_root,
                media_path,
                media_kind: MediaKind::Audio,
                helper_path: self.helper,
                download_model: self.download_model,
            });
        }

        if let Some(media_path) = self.media {
            return E2eEnvState::Requested(E2eConfig {
                model_root: self.model_root,
                media_kind: media_kind_for_path(&media_path),
                media_path,
                helper_path: self.helper,
                download_model: self.download_model,
            });
        }

        E2eEnvState::Misconfigured(format!(
            "either {WAV_ENV} or {MEDIA_ENV} is required when transcription E2E env is set"
        ))
    }
}

enum E2eEnvState {
    NotRequested,
    Misconfigured(String),
    Requested(E2eConfig),
}

struct E2eConfig {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    model_root: Option<PathBuf>,
    media_path: PathBuf,
    media_kind: MediaKind,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    helper_path: Option<PathBuf>,
    download_model: bool,
}

#[cfg(target_os = "macos")]
fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name).map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn env_flag(name: &str) -> bool {
    env::var(name).ok().as_deref() == Some("1")
}

fn blank_env_error(name: &str, path: &Option<PathBuf>) -> Option<String> {
    path.as_ref()
        .filter(|path| path.as_os_str().is_empty())
        .map(|_| format!("{name} must not be blank when transcription E2E env is set"))
}

#[cfg(target_os = "macos")]
fn importable_model_source_dir(model_root: &Path) -> PathBuf {
    let normalized = model_root.join(FLUID_AUDIO_COREML_RUNTIME_MODEL_DIR_NAME);
    if normalized.is_dir() {
        normalized
    } else {
        model_root.to_path_buf()
    }
}

#[cfg(target_os = "macos")]
fn copy_media_fixture(project_dir: &Path, source: &Path) -> PathBuf {
    let media_dir = project_dir.join("media");
    fs::create_dir_all(&media_dir).expect("media dir");
    let extension = source
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("media");
    let destination = media_dir.join(format!("real-transcription-e2e.{extension}"));
    fs::copy(source, &destination).expect("copy media fixture");
    destination
}

#[cfg(target_os = "macos")]
fn e2e_project(source_media_path: &Path, media_kind: MediaKind) -> VideoProject {
    let mut project = VideoProject::new_empty(
        "transcription-e2e-project".to_string(),
        "Transcription E2E Project".to_string(),
        "2026-06-29T00:00:00Z".to_string(),
    );
    let relative_path = source_media_path
        .strip_prefix(
            source_media_path
                .parent()
                .and_then(Path::parent)
                .expect("project dir"),
        )
        .expect("project relative media path")
        .display()
        .to_string();
    project.media.push(MediaAsset {
        id: "media-real-e2e".to_string(),
        name: None,
        relative_path,
        kind: media_kind,
        duration_seconds: 10.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });

    project
}

fn media_kind_for_path(path: &Path) -> MediaKind {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("wav" | "mp3" | "m4a" | "aac" | "flac" | "ogg") => MediaKind::Audio,
        _ => MediaKind::Video,
    }
}

fn checked_in_speech_fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(EDISON_SPEECH_FIXTURE_RELATIVE_PATH)
}

#[cfg(target_os = "macos")]
fn is_executable_file(path: &Path) -> bool {
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn chmod(path: &Path, mode: u32) {
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions).expect("set permissions");
}

#[cfg(target_os = "macos")]
fn write_fake_fluidaudio_helper(path: &Path) {
    fs::write(
        path,
        format!(
            r#"#!/bin/sh
request="$(cat)"
case "$request" in
  *'"outputArtifactPath"'*) ;;
  *)
    printf '%s\n' '{{"schemaVersion":1,"error":"missing output artifact path"}}'
    exit 1
    ;;
esac
printf '%s\n' '{{"schemaVersion":1,"runtimeId":"{runtime_id}","modelId":"nvidia/parakeet-tdt-0.6b-v3","text":"hello world","durationSeconds":1.0,"processingSeconds":0.01,"words":[{{"text":"hello","startSeconds":0.0,"endSeconds":0.4,"confidence":0.98}},{{"text":"world","startSeconds":0.45,"endSeconds":0.9,"confidence":0.97}}]}}'
"#,
            runtime_id = FLUID_AUDIO_COREML_RUNTIME_ID
        ),
    )
    .expect("write fake helper");
    chmod(path, 0o755);
}

#[cfg(target_os = "macos")]
fn write_minimal_coreml_source(root: &Path) {
    for path in [
        "config.json",
        "parakeet_v3_vocab.json",
        "Encoder.mlmodelc/coremldata.bin",
        "Encoder.mlmodelc/metadata.json",
        "Encoder.mlmodelc/model.mil",
        "Encoder.mlmodelc/weights/weight.bin",
        "Decoder.mlmodelc/coremldata.bin",
        "Decoder.mlmodelc/metadata.json",
        "Decoder.mlmodelc/model.mil",
        "Decoder.mlmodelc/weights/weight.bin",
        "JointDecisionv3.mlmodelc/coremldata.bin",
        "JointDecisionv3.mlmodelc/metadata.json",
        "JointDecisionv3.mlmodelc/model.mil",
        "JointDecisionv3.mlmodelc/weights/weight.bin",
        "Preprocessor.mlmodelc/coremldata.bin",
        "Preprocessor.mlmodelc/metadata.json",
        "Preprocessor.mlmodelc/model.mil",
        "Preprocessor.mlmodelc/weights/weight.bin",
    ] {
        let target = root.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("mkdir");
        }
        fs::write(&target, format!("fixture {path}")).expect("write fixture");
    }
}
