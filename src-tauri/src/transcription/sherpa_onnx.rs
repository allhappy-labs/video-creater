//! Linux transcription runtime backed by the `video-creater-speech` helper (sherpa-onnx +
//! Parakeet TDT 0.6B v3 INT8 ONNX). The helper speaks the same stdin/stdout JSON contract as the
//! macOS FluidAudio helper; see `crates/speech-worker/src/protocol.rs`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::model::{TranscriptionModelArtifactFormat, SHERPA_ONNX_RUNTIME_ID};
use super::runtime::{
    InstalledModel, ModelArtifactFormat, ModelModality, ModelRuntime, RuntimeCapability,
    TranscriptToken, TranscriptionRuntimeEngine, TranscriptionRuntimeError,
    TranscriptionRuntimeJob, TranscriptionRuntimeOutput,
};

const HELPER_SCHEMA_VERSION: u32 = 1;
pub const SPEECH_HELPER_EXECUTABLE_NAME: &str = "video-creater-speech";
pub const SPEECH_HELPER_ENV: &str = "VIDEO_CREATER_SPEECH_HELPER";
pub const SHERPA_ONNX_REQUIRED_MODEL_FILES: [&str; 4] = [
    "encoder.int8.onnx",
    "decoder.int8.onnx",
    "joiner.int8.onnx",
    "tokens.txt",
];

/// Resolves the Linux speech helper: `VIDEO_CREATER_SPEECH_HELPER`, then the helper packaged next
/// to the running executable (deb/AppImage `usr/bin`, `tauri dev` target directory), then the
/// development staging path written by `scripts/build-linux-speech-worker.mjs`.
pub fn default_speech_helper_path() -> PathBuf {
    let override_path = std::env::var_os(SPEECH_HELPER_ENV).map(PathBuf::from);
    let current_exe = std::env::current_exe().ok();
    resolve_speech_helper_path(
        override_path.as_deref(),
        current_exe.as_deref(),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
}

pub fn resolve_speech_helper_path(
    override_path: Option<&Path>,
    current_exe: Option<&Path>,
    manifest_dir: &Path,
) -> PathBuf {
    if let Some(path) = override_path.filter(|path| !path.as_os_str().is_empty()) {
        return path.to_path_buf();
    }
    if let Some(packaged) = current_exe
        .and_then(Path::parent)
        .map(|directory| directory.join(SPEECH_HELPER_EXECUTABLE_NAME))
        .filter(|path| path.is_file())
    {
        return packaged;
    }
    manifest_dir.join("binaries").join(format!(
        "{SPEECH_HELPER_EXECUTABLE_NAME}-x86_64-unknown-linux-gnu"
    ))
}

#[derive(Debug, Clone)]
pub struct SherpaOnnxRuntime {
    helper_path: PathBuf,
}

impl SherpaOnnxRuntime {
    pub fn new(helper_path: PathBuf) -> Self {
        Self { helper_path }
    }

    pub fn default_helper_path() -> PathBuf {
        default_speech_helper_path()
    }

    pub fn helper_path(&self) -> &Path {
        &self.helper_path
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HelperRequest<'a> {
    schema_version: u32,
    model_id: &'a str,
    model_path: String,
    media_path: &'a str,
    language_mode: Option<&'a str>,
    output_artifact_path: Option<&'a str>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HelperResponse {
    schema_version: u32,
    runtime_id: String,
    model_id: String,
    words: Vec<HelperWord>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HelperWord {
    text: String,
    start_seconds: f64,
    end_seconds: f64,
    #[serde(default)]
    confidence: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HelperErrorResponse {
    schema_version: u32,
    error: String,
}

fn onnx_error(message: impl Into<String>) -> TranscriptionRuntimeError {
    TranscriptionRuntimeError::OnnxInference(message.into())
}

/// Validates helper stdout exactly like the FluidAudio runtime does, but for the `sherpa_onnx`
/// runtime id.
pub fn runtime_output_from_speech_helper_json(
    expected_model_id: &str,
    stdout: &[u8],
) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
    if let Ok(response) = serde_json::from_slice::<HelperErrorResponse>(stdout) {
        if response.schema_version != HELPER_SCHEMA_VERSION {
            return Err(onnx_error(format!(
                "speech helper schemaVersion mismatch: expected {HELPER_SCHEMA_VERSION}, found {}",
                response.schema_version
            )));
        }
        return Err(onnx_error(format!(
            "speech helper failed: {}",
            response.error
        )));
    }

    let response: HelperResponse = serde_json::from_slice(stdout)
        .map_err(|error| onnx_error(format!("failed to parse speech helper JSON: {error}")))?;
    if response.schema_version != HELPER_SCHEMA_VERSION {
        return Err(onnx_error(format!(
            "speech helper schemaVersion mismatch: expected {HELPER_SCHEMA_VERSION}, found {}",
            response.schema_version
        )));
    }
    if response.runtime_id != SHERPA_ONNX_RUNTIME_ID {
        return Err(onnx_error(format!(
            "speech helper runtimeId mismatch: expected {SHERPA_ONNX_RUNTIME_ID}, found {}",
            response.runtime_id
        )));
    }
    if response.model_id != expected_model_id {
        return Err(onnx_error(format!(
            "speech helper modelId mismatch: expected {expected_model_id}, found {}",
            response.model_id
        )));
    }

    let tokens = response
        .words
        .into_iter()
        .filter_map(|word| {
            let token = word.text.trim();
            if token.is_empty()
                || !word.start_seconds.is_finite()
                || !word.end_seconds.is_finite()
                || word.end_seconds < word.start_seconds
            {
                return None;
            }
            Some(TranscriptToken {
                token: token.to_string(),
                start: word.start_seconds,
                end: word.end_seconds,
                confidence: word.confidence.filter(|confidence| confidence.is_finite()),
            })
        })
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return Err(onnx_error("speech helper returned no usable words"));
    }

    Ok(TranscriptionRuntimeOutput {
        runtime_id: SHERPA_ONNX_RUNTIME_ID.to_string(),
        model_id: expected_model_id.to_string(),
        tokens,
    })
}

pub fn first_missing_sherpa_onnx_model_file(model_dir: &Path) -> Option<PathBuf> {
    SHERPA_ONNX_REQUIRED_MODEL_FILES
        .iter()
        .map(|name| model_dir.join(name))
        .find(|path| !path.is_file())
}

fn helper_is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

impl ModelRuntime for SherpaOnnxRuntime {
    fn runtime_id(&self) -> &str {
        SHERPA_ONNX_RUNTIME_ID
    }

    fn modality(&self) -> ModelModality {
        ModelModality::Transcription
    }

    fn supports_installed_model(&self, model: &InstalledModel) -> bool {
        model.modality == ModelModality::Transcription
            && model.runtime_family == SHERPA_ONNX_RUNTIME_ID
            && model.artifact_format
                == ModelArtifactFormat::from(TranscriptionModelArtifactFormat::SherpaOnnxTransducer)
    }

    fn probe_installed_model(&self, model: &InstalledModel) -> RuntimeCapability {
        if !cfg!(target_os = "linux") {
            return RuntimeCapability::UnsupportedPlatform;
        }
        if helper_is_executable(&self.helper_path)
            && self.supports_installed_model(model)
            && first_missing_sherpa_onnx_model_file(&model.model_dir).is_none()
        {
            RuntimeCapability::Ready
        } else {
            RuntimeCapability::Unavailable
        }
    }

    fn readiness_diagnostic(
        &self,
        model: &InstalledModel,
        capability: &RuntimeCapability,
    ) -> Option<String> {
        match capability {
            RuntimeCapability::Ready => None,
            RuntimeCapability::UnsupportedPlatform => Some(
                "ONNX transcription requires Linux with the packaged video-creater-speech helper"
                    .to_string(),
            ),
            RuntimeCapability::Unavailable => {
                if let Some(missing) = first_missing_sherpa_onnx_model_file(&model.model_dir) {
                    return Some(format!("missing ONNX model file {}", missing.display()));
                }
                if !helper_is_executable(&self.helper_path) {
                    return Some(format!(
                        "speech helper executable is unavailable at {}",
                        self.helper_path.display()
                    ));
                }
                None
            }
        }
    }
}

impl TranscriptionRuntimeEngine for SherpaOnnxRuntime {
    fn transcribe_installed(
        &self,
        job: &TranscriptionRuntimeJob,
        model: &InstalledModel,
    ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
        if !cfg!(target_os = "linux") {
            return Err(TranscriptionRuntimeError::UnsupportedPlatform);
        }
        if !self.supports_installed_model(model) {
            return Err(onnx_error(format!(
                "ONNX runtime does not support installed model {}",
                model.model_id
            )));
        }
        if let Some(missing) = first_missing_sherpa_onnx_model_file(&model.model_dir) {
            return Err(onnx_error(format!(
                "missing ONNX model file {}",
                missing.display()
            )));
        }
        if !helper_is_executable(&self.helper_path) {
            return Err(onnx_error(format!(
                "speech helper executable is unavailable at {}",
                self.helper_path.display()
            )));
        }

        let request = HelperRequest {
            schema_version: HELPER_SCHEMA_VERSION,
            model_id: &model.model_id,
            model_path: model.model_dir.display().to_string(),
            media_path: &job.source_path,
            language_mode: non_blank(&job.language_mode),
            output_artifact_path: job.output_artifact_path.as_deref().and_then(non_blank),
        };
        let request_json = serde_json::to_vec(&request).map_err(|error| {
            onnx_error(format!(
                "failed to serialize speech helper request: {error}"
            ))
        })?;
        let output = run_helper(&self.helper_path, &request_json)?;
        if !output.status.success() {
            if let Ok(response) = serde_json::from_slice::<HelperErrorResponse>(&output.stdout) {
                return Err(onnx_error(format!(
                    "speech helper failed{}: {}",
                    exit_code_suffix(output.status.code()),
                    response.error
                )));
            }
            return Err(onnx_error(format!(
                "speech helper failed{}: {}",
                exit_code_suffix(output.status.code()),
                super::fluidaudio::concise_stderr(&output.stderr)
            )));
        }
        runtime_output_from_speech_helper_json(&model.model_id, &output.stdout)
    }
}

fn non_blank(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn exit_code_suffix(status_code: Option<i32>) -> String {
    status_code
        .map(|code| format!(" with exit code {code}"))
        .unwrap_or_default()
}

/// Runs a helper with `request` on stdin. The request is written from a separate thread so a
/// helper that fails early (and stops reading) cannot deadlock the caller.
pub fn run_helper(
    helper_path: &Path,
    request: &[u8],
) -> Result<std::process::Output, TranscriptionRuntimeError> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new(helper_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            onnx_error(format!(
                "failed to start speech helper {}: {error}",
                helper_path.display()
            ))
        })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| onnx_error("failed to open speech helper stdin"))?;
    let request = request.to_vec();
    let writer = std::thread::spawn(move || stdin.write_all(&request));
    let output = child
        .wait_with_output()
        .map_err(|error| onnx_error(format!("failed waiting for speech helper: {error}")))?;
    let write_result = writer
        .join()
        .map_err(|_| onnx_error("speech helper request writer panicked"))?;
    if let Err(error) = write_result {
        if output.status.success() {
            return Err(onnx_error(format!(
                "failed to write speech helper request: {error}"
            )));
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed(model_dir: PathBuf) -> InstalledModel {
        InstalledModel {
            model_id: "nvidia/parakeet-tdt-0.6b-v3".to_string(),
            model_dir,
            artifact_format: ModelArtifactFormat::from(
                TranscriptionModelArtifactFormat::SherpaOnnxTransducer,
            ),
            modality: ModelModality::Transcription,
            runtime_family: SHERPA_ONNX_RUNTIME_ID.to_string(),
        }
    }

    #[test]
    fn helper_json_is_validated_against_the_onnx_runtime_identity() {
        let ok = br#"{"durationSeconds":1.0,"modelId":"nvidia/parakeet-tdt-0.6b-v3","processingSeconds":0.1,"runtimeId":"sherpa_onnx","schemaVersion":1,"text":"hello world","words":[{"confidence":0.9,"endSeconds":0.4,"startSeconds":0.1,"text":"hello"},{"confidence":0.8,"endSeconds":0.3,"startSeconds":0.5,"text":"bad"},{"confidence":0.7,"endSeconds":0.9,"startSeconds":0.5,"text":"world"}]}"#;
        let output = runtime_output_from_speech_helper_json("nvidia/parakeet-tdt-0.6b-v3", ok)
            .expect("valid helper output");
        assert_eq!(output.runtime_id, SHERPA_ONNX_RUNTIME_ID);
        assert_eq!(
            output
                .tokens
                .iter()
                .map(|token| token.token.as_str())
                .collect::<Vec<_>>(),
            ["hello", "world"]
        );

        let coreml = String::from_utf8_lossy(ok).replace("sherpa_onnx", "fluid_audio_coreml");
        assert!(runtime_output_from_speech_helper_json(
            "nvidia/parakeet-tdt-0.6b-v3",
            coreml.as_bytes()
        )
        .unwrap_err()
        .to_string()
        .contains("runtimeId mismatch"));
        let failure = runtime_output_from_speech_helper_json(
            "nvidia/parakeet-tdt-0.6b-v3",
            br#"{"error":"Media file not found at /x","schemaVersion":1}"#,
        )
        .unwrap_err();
        assert_eq!(
            failure.to_string(),
            "ONNX inference failed: speech helper failed: Media file not found at /x"
        );
    }

    #[test]
    fn runtime_supports_only_sherpa_onnx_installs_and_reports_missing_files() {
        let directory = tempfile::tempdir().expect("model dir");
        let helper = directory.path().join("video-creater-speech");
        std::fs::write(&helper, b"#!/bin/sh\n").expect("helper");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755))
                .expect("chmod");
        }
        let runtime = SherpaOnnxRuntime::new(helper);
        let model = installed(directory.path().to_path_buf());
        assert!(runtime.supports_installed_model(&model));

        let mut coreml = model.clone();
        coreml.artifact_format =
            ModelArtifactFormat::from(TranscriptionModelArtifactFormat::CoreMlBundle);
        assert!(!runtime.supports_installed_model(&coreml));

        let capability = runtime.probe_installed_model(&model);
        if cfg!(target_os = "linux") {
            assert_eq!(capability, RuntimeCapability::Unavailable);
            assert!(runtime
                .readiness_diagnostic(&model, &capability)
                .expect("diagnostic")
                .contains("encoder.int8.onnx"));
            for name in SHERPA_ONNX_REQUIRED_MODEL_FILES {
                std::fs::write(directory.path().join(name), b"x").expect("model file");
            }
            assert_eq!(
                runtime.probe_installed_model(&model),
                RuntimeCapability::Ready
            );
        } else {
            assert_eq!(capability, RuntimeCapability::UnsupportedPlatform);
        }
    }

    #[test]
    fn helper_path_prefers_override_then_packaged_sibling_then_dev_staging() {
        let directory = tempfile::tempdir().expect("layout");
        let manifest = directory.path().join("src-tauri");
        let exe_dir = directory.path().join("usr/bin");
        std::fs::create_dir_all(&exe_dir).expect("exe dir");
        let exe = exe_dir.join("video-creater");
        assert_eq!(
            resolve_speech_helper_path(Some(Path::new("/opt/helper")), Some(&exe), &manifest),
            PathBuf::from("/opt/helper")
        );
        assert_eq!(
            resolve_speech_helper_path(None, Some(&exe), &manifest),
            manifest.join("binaries/video-creater-speech-x86_64-unknown-linux-gnu")
        );
        std::fs::write(exe_dir.join(SPEECH_HELPER_EXECUTABLE_NAME), b"helper").expect("helper");
        assert_eq!(
            resolve_speech_helper_path(Some(Path::new("")), Some(&exe), &manifest),
            exe_dir.join(SPEECH_HELPER_EXECUTABLE_NAME)
        );
    }

    /// Real end-to-end run through the app code path: downloads and hash-verifies the pinned
    /// Parakeet ONNX model with `TranscriptionModelStore`, selects the runtime from the default
    /// registry, and transcribes a known English clip with the staged helper.
    ///
    /// `VIDEO_CREATER_LINUX_SPEECH_E2E_ROOT=/persistent/dir cargo test --lib -- --ignored
    /// real_parakeet_onnx_transcription_through_the_default_registry`
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "downloads ~670 MB of models and needs the staged video-creater-speech helper"]
    fn real_parakeet_onnx_transcription_through_the_default_registry() {
        use crate::transcription::model::{parakeet_v3_catalog_entry, ModelInstallStatus};
        use crate::transcription::store::TranscriptionModelStore;

        let root = PathBuf::from(
            std::env::var_os("VIDEO_CREATER_LINUX_SPEECH_E2E_ROOT")
                .expect("set VIDEO_CREATER_LINUX_SPEECH_E2E_ROOT to a persistent directory"),
        );
        let store = TranscriptionModelStore::new(root.join("models"));
        let entry = parakeet_v3_catalog_entry();
        let status = match store.verify(entry.id) {
            Ok(status) if status.install_status == ModelInstallStatus::Ready => status,
            _ => store
                .download(entry.id)
                .expect("download pinned Parakeet ONNX model"),
        };
        assert_eq!(status.install_status, ModelInstallStatus::Ready);
        assert_eq!(status.runtime_id, SHERPA_ONNX_RUNTIME_ID);

        let clip = root.join("en.wav");
        if !clip.is_file() {
            let bytes = reqwest::blocking::get(format!(
                "https://huggingface.co/{}/resolve/{}/test_wavs/en.wav",
                crate::transcription::model::SHERPA_ONNX_PARAKEET_V3_REPO_ID,
                crate::transcription::model::SHERPA_ONNX_PARAKEET_V3_REVISION
            ))
            .and_then(reqwest::blocking::Response::error_for_status)
            .and_then(reqwest::blocking::Response::bytes)
            .expect("download English test clip");
            std::fs::write(&clip, &bytes).expect("write clip");
        }

        let installed = InstalledModel {
            model_id: entry.id.to_string(),
            model_dir: store
                .runtime_model_dir(entry.id, SHERPA_ONNX_RUNTIME_ID)
                .expect("runtime model dir"),
            artifact_format: ModelArtifactFormat::from(entry.artifact_format),
            modality: ModelModality::Transcription,
            runtime_family: SHERPA_ONNX_RUNTIME_ID.to_string(),
        };
        let registry = crate::workflows::default_transcription_runtime_registry();
        let runtime = registry
            .select(SHERPA_ONNX_RUNTIME_ID, &installed)
            .expect("sherpa-onnx runtime is ready");
        let output = runtime
            .transcribe_installed(
                &TranscriptionRuntimeJob {
                    media_id: "en".to_string(),
                    source_path: clip.display().to_string(),
                    model_path: installed.model_dir.display().to_string(),
                    language_mode: "auto".to_string(),
                    output_artifact_path: None,
                },
                &installed,
            )
            .expect("real transcription");
        let text = output
            .tokens
            .iter()
            .map(|token| token.token.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        println!("transcript: {text}");
        assert!(text.to_lowercase().contains("country"), "{text}");
        assert!(output.tokens.len() >= 10);
        assert!(output
            .tokens
            .windows(2)
            .all(|pair| pair[0].start <= pair[1].start && pair[0].end <= pair[1].start + 1e-9));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn failing_helper_errors_are_surfaced_with_exit_code() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().expect("model dir");
        for name in SHERPA_ONNX_REQUIRED_MODEL_FILES {
            std::fs::write(directory.path().join(name), b"x").expect("model file");
        }
        let helper = directory.path().join("helper.sh");
        std::fs::write(
            &helper,
            "#!/bin/sh\ncat >/dev/null\necho '{\"schemaVersion\":1,\"error\":\"Media file not found at /missing.wav\"}'\nexit 1\n",
        )
        .expect("helper script");
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let runtime = SherpaOnnxRuntime::new(helper);
        let error = runtime
            .transcribe_installed(
                &TranscriptionRuntimeJob {
                    media_id: "media".to_string(),
                    source_path: "/missing.wav".to_string(),
                    model_path: directory.path().display().to_string(),
                    language_mode: "auto".to_string(),
                    output_artifact_path: None,
                },
                &installed(directory.path().to_path_buf()),
            )
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "ONNX inference failed: speech helper failed with exit code 1: Media file not found at /missing.wav"
        );
    }
}
