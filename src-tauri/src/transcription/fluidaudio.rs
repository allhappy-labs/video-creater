use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::{
    io::Write,
    process::{Child, Command, Output, Stdio},
};

#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;

use serde::Deserialize;
#[cfg(target_os = "macos")]
use serde::Serialize;

#[cfg(target_os = "macos")]
use super::store::COREML_INSPECTION_FILE_NAME;
use super::{
    model::{TranscriptionModelArtifactFormat, FLUID_AUDIO_COREML_RUNTIME_ID},
    runtime::{
        InstalledModel, ModelArtifactFormat, ModelModality, ModelRuntime, RuntimeCapability,
        TranscriptToken, TranscriptionRuntimeEngine, TranscriptionRuntimeError,
        TranscriptionRuntimeJob, TranscriptionRuntimeOutput,
    },
};

const HELPER_SCHEMA_VERSION: u32 = 1;
const HELPER_EXECUTABLE_NAME: &str = "video-creater-fluidaudio-transcribe";
#[cfg(target_os = "macos")]
const REQUIRED_COREML_BUNDLES: [&str; 4] = [
    "Preprocessor.mlmodelc",
    "Encoder.mlmodelc",
    "Decoder.mlmodelc",
    "JointDecisionv3.mlmodelc",
];

#[derive(Debug, Clone)]
pub struct FluidAudioCoreMlRuntime {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    helper_path: PathBuf,
}

impl FluidAudioCoreMlRuntime {
    pub fn new(helper_path: PathBuf) -> Self {
        Self { helper_path }
    }

    pub fn default_helper_path() -> PathBuf {
        let package_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("native/fluidaudio-parakeet");
        let release = package_dir
            .join(".build")
            .join("release")
            .join(HELPER_EXECUTABLE_NAME);
        if release.is_file() {
            return release;
        }

        package_dir
            .join(".build")
            .join("debug")
            .join(HELPER_EXECUTABLE_NAME)
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HelperRequest {
    schema_version: u32,
    model_id: String,
    model_path: String,
    media_path: String,
    language_mode: Option<String>,
    output_artifact_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HelperResponse {
    schema_version: u32,
    runtime_id: String,
    model_id: String,
    #[allow(dead_code)]
    text: String,
    #[allow(dead_code)]
    duration_seconds: f64,
    #[allow(dead_code)]
    processing_seconds: f64,
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

pub fn runtime_output_from_helper_json(
    expected_model_id: &str,
    stdout: &[u8],
) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
    if let Ok(response) = serde_json::from_slice::<HelperErrorResponse>(stdout) {
        if response.schema_version != HELPER_SCHEMA_VERSION {
            return Err(TranscriptionRuntimeError::Inference(format!(
                "FluidAudio helper schemaVersion mismatch: expected {}, found {}",
                HELPER_SCHEMA_VERSION, response.schema_version
            )));
        }

        return Err(TranscriptionRuntimeError::Inference(format!(
            "FluidAudio helper failed: {}",
            response.error
        )));
    }

    let response: HelperResponse = serde_json::from_slice(stdout).map_err(|error| {
        TranscriptionRuntimeError::Inference(format!(
            "failed to parse FluidAudio helper JSON: {error}"
        ))
    })?;

    if response.schema_version != HELPER_SCHEMA_VERSION {
        return Err(TranscriptionRuntimeError::Inference(format!(
            "FluidAudio helper schemaVersion mismatch: expected {}, found {}",
            HELPER_SCHEMA_VERSION, response.schema_version
        )));
    }
    if response.runtime_id != FLUID_AUDIO_COREML_RUNTIME_ID {
        return Err(TranscriptionRuntimeError::Inference(format!(
            "FluidAudio helper runtimeId mismatch: expected {}, found {}",
            FLUID_AUDIO_COREML_RUNTIME_ID, response.runtime_id
        )));
    }
    if response.model_id != expected_model_id {
        return Err(TranscriptionRuntimeError::Inference(format!(
            "FluidAudio helper modelId mismatch: expected {}, found {}",
            expected_model_id, response.model_id
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
        return Err(TranscriptionRuntimeError::Inference(
            "FluidAudio helper returned no usable words".to_string(),
        ));
    }

    Ok(TranscriptionRuntimeOutput {
        runtime_id: FLUID_AUDIO_COREML_RUNTIME_ID.to_string(),
        model_id: expected_model_id.to_string(),
        tokens,
    })
}

impl ModelRuntime for FluidAudioCoreMlRuntime {
    fn runtime_id(&self) -> &str {
        FLUID_AUDIO_COREML_RUNTIME_ID
    }

    fn modality(&self) -> ModelModality {
        ModelModality::Transcription
    }

    fn supports_installed_model(&self, model: &InstalledModel) -> bool {
        model.modality == ModelModality::Transcription
            && model.runtime_family == FLUID_AUDIO_COREML_RUNTIME_ID
            && model.artifact_format
                == ModelArtifactFormat::from(TranscriptionModelArtifactFormat::CoreMlBundle)
    }

    fn probe_installed_model(&self, model: &InstalledModel) -> RuntimeCapability {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = model;
            RuntimeCapability::UnsupportedPlatform
        }

        #[cfg(target_os = "macos")]
        {
            if helper_is_executable(&self.helper_path)
                && self.supports_installed_model(model)
                && first_missing_coreml_prerequisite(&model.model_dir).is_none()
            {
                RuntimeCapability::Ready
            } else {
                RuntimeCapability::Unavailable
            }
        }
    }

    fn readiness_diagnostic(
        &self,
        model: &InstalledModel,
        capability: &RuntimeCapability,
    ) -> Option<String> {
        if capability == &RuntimeCapability::Ready {
            return None;
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = model;
            Some(super::runtime::coreml_unsupported_platform_diagnostic().to_string())
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(error) = first_missing_coreml_prerequisite(&model.model_dir) {
                return Some(error.to_string());
            }

            if !helper_is_executable(&self.helper_path) {
                return Some(format!(
                    "FluidAudio helper executable is unavailable at {}",
                    self.helper_path.display()
                ));
            }

            None
        }
    }
}

impl TranscriptionRuntimeEngine for FluidAudioCoreMlRuntime {
    fn transcribe_installed(
        &self,
        job: &TranscriptionRuntimeJob,
        model: &InstalledModel,
    ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = job;
            let _ = model;
            Err(TranscriptionRuntimeError::UnsupportedPlatform)
        }

        #[cfg(target_os = "macos")]
        {
            if !self.supports_installed_model(model) {
                return Err(TranscriptionRuntimeError::Inference(format!(
                    "FluidAudio runtime does not support installed model {}",
                    model.model_id
                )));
            }

            if let Some(error) = first_missing_coreml_prerequisite(&model.model_dir) {
                return Err(error);
            }

            if !helper_is_executable(&self.helper_path) {
                return Err(TranscriptionRuntimeError::Inference(format!(
                    "FluidAudio helper executable is unavailable at {}",
                    self.helper_path.display()
                )));
            }

            let request = HelperRequest {
                schema_version: HELPER_SCHEMA_VERSION,
                model_id: model.model_id.clone(),
                model_path: model.model_dir.display().to_string(),
                media_path: job.source_path.clone(),
                language_mode: non_blank_string(&job.language_mode),
                output_artifact_path: job
                    .output_artifact_path
                    .as_deref()
                    .and_then(non_blank_string),
            };
            let request_json = serde_json::to_vec(&request).map_err(|error| {
                TranscriptionRuntimeError::Inference(format!(
                    "failed to serialize FluidAudio helper request: {error}"
                ))
            })?;

            let mut child = Command::new(&self.helper_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|error| {
                    TranscriptionRuntimeError::Inference(format!(
                        "failed to start FluidAudio helper {}: {}",
                        self.helper_path.display(),
                        error
                    ))
                })?;

            let mut stdin = child.stdin.take().ok_or_else(|| {
                TranscriptionRuntimeError::Inference(
                    "failed to open FluidAudio helper stdin".to_string(),
                )
            })?;
            if let Err(error) = stdin.write_all(&request_json) {
                drop(stdin);
                return Err(helper_stdin_write_error(error, child));
            }
            drop(stdin);

            let output = child.wait_with_output().map_err(|error| {
                TranscriptionRuntimeError::Inference(format!(
                    "failed waiting for FluidAudio helper: {error}"
                ))
            })?;

            if !output.status.success() {
                return Err(helper_exit_error(
                    output.status.code(),
                    &output.stdout,
                    &output.stderr,
                ));
            }

            runtime_output_from_helper_json(&model.model_id, &output.stdout)
        }
    }
}

#[cfg(target_os = "macos")]
fn helper_is_executable(path: &Path) -> bool {
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn first_missing_coreml_prerequisite(model_dir: &Path) -> Option<TranscriptionRuntimeError> {
    for bundle in REQUIRED_COREML_BUNDLES {
        let bundle_path = model_dir.join(bundle);
        if !bundle_path.is_dir() {
            return Some(TranscriptionRuntimeError::Inference(format!(
                "missing Core ML bundle {}",
                bundle_path.display()
            )));
        }
    }

    let inspection_report_path = model_dir.join(COREML_INSPECTION_FILE_NAME);
    if !inspection_report_path.is_file() {
        return Some(TranscriptionRuntimeError::Inference(format!(
            "missing Core ML inspection report {}",
            inspection_report_path.display()
        )));
    }

    None
}

#[cfg(target_os = "macos")]
fn helper_stdin_write_error(error: std::io::Error, child: Child) -> TranscriptionRuntimeError {
    match child.wait_with_output() {
        Ok(output) => helper_request_write_error(error, output),
        Err(wait_error) => TranscriptionRuntimeError::Inference(format!(
            "failed to write FluidAudio helper request: {error}; additionally failed waiting for FluidAudio helper: {wait_error}"
        )),
    }
}

#[cfg(target_os = "macos")]
fn helper_request_write_error(error: std::io::Error, output: Output) -> TranscriptionRuntimeError {
    if let Ok(response) = serde_json::from_slice::<HelperErrorResponse>(&output.stdout) {
        return TranscriptionRuntimeError::Inference(format!(
            "failed to write FluidAudio helper request: {error}; helper failed{}: {}",
            exit_code_suffix(output.status.code()),
            response.error
        ));
    }

    let details = if output.stdout.iter().any(|byte| !byte.is_ascii_whitespace()) {
        format!("stdout: {}", concise_output(&output.stdout))
    } else {
        format!("stderr: {}", concise_stderr(&output.stderr))
    };

    TranscriptionRuntimeError::Inference(format!(
        "failed to write FluidAudio helper request: {error}; helper failed{}; {details}",
        exit_code_suffix(output.status.code())
    ))
}

#[cfg(target_os = "macos")]
fn helper_exit_error(
    status_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> TranscriptionRuntimeError {
    if let Ok(response) = serde_json::from_slice::<HelperErrorResponse>(stdout) {
        return TranscriptionRuntimeError::Inference(format!(
            "FluidAudio helper failed{}: {}",
            exit_code_suffix(status_code),
            response.error
        ));
    }

    TranscriptionRuntimeError::Inference(format!(
        "FluidAudio helper failed{}: {}",
        exit_code_suffix(status_code),
        concise_stderr(stderr)
    ))
}

#[cfg(target_os = "macos")]
fn exit_code_suffix(status_code: Option<i32>) -> String {
    status_code
        .map(|code| format!(" with exit code {code}"))
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn concise_output(output: &[u8]) -> String {
    concise_text(&String::from_utf8_lossy(output), "no output details")
}

#[doc(hidden)]
pub fn concise_stderr(stderr: &[u8]) -> String {
    concise_text(
        &String::from_utf8_lossy(stderr),
        "no error details on stderr",
    )
}

fn concise_text(text: &str, empty_message: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return empty_message.to_string();
    }

    const MAX_LEN: usize = 512;
    if text.len() <= MAX_LEN {
        text.to_string()
    } else {
        let mut truncate_at = 0;
        for (index, character) in text.char_indices() {
            let next_index = index + character.len_utf8();
            if next_index > MAX_LEN {
                break;
            }
            truncate_at = next_index;
        }

        format!("{}...", &text[..truncate_at])
    }
}

#[cfg(target_os = "macos")]
fn non_blank_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
