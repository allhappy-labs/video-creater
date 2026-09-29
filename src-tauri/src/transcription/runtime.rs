#[cfg(target_os = "macos")]
use std::path::Path;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::project::model::{TranscriptSegment, TranscriptWord};

#[cfg(target_os = "macos")]
use super::fluidaudio::FluidAudioCoreMlRuntime;
use super::model::{
    TranscriptionModelArtifactFormat, TranscriptionModelCatalogEntry, FLUID_AUDIO_COREML_RUNTIME_ID,
};
#[cfg(target_os = "macos")]
use super::store::COREML_INSPECTION_FILE_NAME;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCapability {
    Ready,
    UnsupportedPlatform,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSelection {
    Native,
    UnsupportedPlatform,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelModality {
    Transcription,
    ImageGeneration,
    VideoGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelArtifactFormat(pub String);

impl From<TranscriptionModelArtifactFormat> for ModelArtifactFormat {
    fn from(format: TranscriptionModelArtifactFormat) -> Self {
        let format = match format {
            TranscriptionModelArtifactFormat::CoreMlBundle => "core_ml_bundle",
            TranscriptionModelArtifactFormat::SherpaOnnxTransducer => "sherpa_onnx_transducer",
        };

        Self(format.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModel {
    pub model_id: String,
    pub model_dir: PathBuf,
    pub artifact_format: ModelArtifactFormat,
    pub modality: ModelModality,
    pub runtime_family: String,
}

pub trait ModelRuntime: std::fmt::Debug + Send + Sync {
    fn runtime_id(&self) -> &str;
    fn modality(&self) -> ModelModality;
    fn supports_installed_model(&self, model: &InstalledModel) -> bool;
    fn probe_installed_model(&self, model: &InstalledModel) -> RuntimeCapability;
    fn readiness_diagnostic(
        &self,
        _model: &InstalledModel,
        _capability: &RuntimeCapability,
    ) -> Option<String> {
        None
    }
}

pub trait TranscriptionRuntime {
    fn id(&self) -> &'static str;
    fn supports(&self, model: &TranscriptionModelCatalogEntry) -> bool;
    fn probe(&self, model: &TranscriptionModelCatalogEntry) -> RuntimeCapability;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptToken {
    pub token: String,
    pub start: f64,
    pub end: f64,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionRuntimeJob {
    pub media_id: String,
    pub source_path: String,
    pub model_path: String,
    pub language_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_artifact_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionRuntimeOutput {
    pub runtime_id: String,
    pub model_id: String,
    pub tokens: Vec<TranscriptToken>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeTranscriptionOutput {
    pub engine: String,
    pub raw_artifact_path: PathBuf,
    pub words: Vec<TranscriptWord>,
    pub segments: Vec<TranscriptSegment>,
}

pub trait TranscriptionBackend {
    fn transcribe(
        &self,
        job: &TranscriptionRuntimeJob,
        model_id: &str,
    ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TranscriptionRuntimeError {
    #[error("unsupported platform")]
    UnsupportedPlatform,
    #[error("Core ML inference failed: {0}")]
    Inference(String),
    #[error("ONNX inference failed: {0}")]
    OnnxInference(String),
}

pub fn coreml_unsupported_platform_diagnostic() -> &'static str {
    "FluidAudio Core ML transcription requires macOS with the packaged native helper and a verified Core ML Parakeet model"
}

pub trait TranscriptionRuntimeEngine: ModelRuntime {
    fn transcribe_installed(
        &self,
        job: &TranscriptionRuntimeJob,
        model: &InstalledModel,
    ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RuntimeRegistryError {
    #[error("unknown runtime `{runtime_id}`")]
    UnknownRuntime { runtime_id: String },
    #[error("runtime `{runtime_id}` modality mismatch: expected {expected:?}, found {actual:?}")]
    ModalityMismatch {
        runtime_id: String,
        expected: ModelModality,
        actual: ModelModality,
    },
    #[error("runtime `{runtime_id}` does not support installed model `{model_id}`")]
    UnsupportedModel {
        runtime_id: String,
        model_id: String,
    },
    #[error("{}", runtime_not_ready_message(runtime_id, model_id, capability, diagnostic.as_deref()))]
    RuntimeNotReady {
        runtime_id: String,
        model_id: String,
        capability: RuntimeCapability,
        diagnostic: Option<String>,
    },
}

fn runtime_not_ready_message(
    runtime_id: &str,
    model_id: &str,
    capability: &RuntimeCapability,
    diagnostic: Option<&str>,
) -> String {
    let base = format!(
        "runtime `{runtime_id}` is not ready for installed model `{model_id}`: {capability:?}"
    );
    match diagnostic {
        Some(diagnostic) if !diagnostic.trim().is_empty() => format!("{base}; {diagnostic}"),
        _ => base,
    }
}

#[derive(Debug)]
pub struct TranscriptionRuntimeRegistry {
    runtimes: Vec<Box<dyn TranscriptionRuntimeEngine>>,
}

impl TranscriptionRuntimeRegistry {
    pub fn new(runtimes: Vec<Box<dyn TranscriptionRuntimeEngine>>) -> Self {
        Self { runtimes }
    }

    pub fn iter(&self) -> impl Iterator<Item = &dyn TranscriptionRuntimeEngine> {
        self.runtimes.iter().map(|runtime| runtime.as_ref())
    }

    pub fn select(
        &self,
        runtime_id: &str,
        model: &InstalledModel,
    ) -> Result<&dyn TranscriptionRuntimeEngine, RuntimeRegistryError> {
        let runtime = self
            .runtimes
            .iter()
            .find(|runtime| runtime.runtime_id() == runtime_id)
            .map(|runtime| runtime.as_ref())
            .ok_or_else(|| RuntimeRegistryError::UnknownRuntime {
                runtime_id: runtime_id.to_string(),
            })?;

        if runtime.modality() != ModelModality::Transcription {
            return Err(RuntimeRegistryError::ModalityMismatch {
                runtime_id: runtime_id.to_string(),
                expected: ModelModality::Transcription,
                actual: runtime.modality(),
            });
        }

        if model.modality != ModelModality::Transcription {
            return Err(RuntimeRegistryError::ModalityMismatch {
                runtime_id: runtime_id.to_string(),
                expected: ModelModality::Transcription,
                actual: model.modality,
            });
        }

        if !runtime.supports_installed_model(model) {
            return Err(RuntimeRegistryError::UnsupportedModel {
                runtime_id: runtime_id.to_string(),
                model_id: model.model_id.clone(),
            });
        }

        let capability = runtime.probe_installed_model(model);
        if capability != RuntimeCapability::Ready {
            let diagnostic = runtime.readiness_diagnostic(model, &capability);
            return Err(RuntimeRegistryError::RuntimeNotReady {
                runtime_id: runtime_id.to_string(),
                model_id: model.model_id.clone(),
                capability,
                diagnostic,
            });
        }

        Ok(runtime)
    }
}

#[derive(Debug, Clone)]
pub struct FixtureTranscriptionBackend {
    pub tokens: Vec<TranscriptToken>,
}

impl TranscriptionBackend for FixtureTranscriptionBackend {
    fn transcribe(
        &self,
        _job: &TranscriptionRuntimeJob,
        model_id: &str,
    ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
        Ok(TranscriptionRuntimeOutput {
            runtime_id: FLUID_AUDIO_COREML_RUNTIME_ID.to_string(),
            model_id: model_id.to_string(),
            tokens: self.tokens.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct FixtureTranscriptionRuntime {
    pub runtime_id: &'static str,
    pub capability: RuntimeCapability,
    pub supported: bool,
    pub tokens: Vec<TranscriptToken>,
}

impl ModelRuntime for FixtureTranscriptionRuntime {
    fn runtime_id(&self) -> &str {
        self.runtime_id
    }

    fn modality(&self) -> ModelModality {
        ModelModality::Transcription
    }

    fn supports_installed_model(&self, _model: &InstalledModel) -> bool {
        self.supported
    }

    fn probe_installed_model(&self, _model: &InstalledModel) -> RuntimeCapability {
        self.capability.clone()
    }
}

impl TranscriptionRuntimeEngine for FixtureTranscriptionRuntime {
    fn transcribe_installed(
        &self,
        _job: &TranscriptionRuntimeJob,
        model: &InstalledModel,
    ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
        Ok(TranscriptionRuntimeOutput {
            runtime_id: self.runtime_id.to_string(),
            model_id: model.model_id.clone(),
            tokens: self.tokens.clone(),
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CoreMlParakeetBackend;

impl CoreMlParakeetBackend {
    pub fn with_helper_path(helper_path: PathBuf) -> ConfiguredCoreMlParakeetBackend {
        ConfiguredCoreMlParakeetBackend { helper_path }
    }
}

impl TranscriptionBackend for CoreMlParakeetBackend {
    fn transcribe(
        &self,
        job: &TranscriptionRuntimeJob,
        model_id: &str,
    ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = job;
            let _ = model_id;
            Err(TranscriptionRuntimeError::UnsupportedPlatform)
        }

        #[cfg(target_os = "macos")]
        {
            run_coreml_parakeet(
                job,
                model_id,
                FluidAudioCoreMlRuntime::default_helper_path(),
            )
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfiguredCoreMlParakeetBackend {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    helper_path: PathBuf,
}

impl TranscriptionBackend for ConfiguredCoreMlParakeetBackend {
    fn transcribe(
        &self,
        job: &TranscriptionRuntimeJob,
        model_id: &str,
    ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = job;
            let _ = model_id;
            Err(TranscriptionRuntimeError::UnsupportedPlatform)
        }

        #[cfg(target_os = "macos")]
        {
            run_coreml_parakeet(job, model_id, self.helper_path.clone())
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CoreMlParakeetRuntime;

impl TranscriptionRuntime for CoreMlParakeetRuntime {
    fn id(&self) -> &'static str {
        FLUID_AUDIO_COREML_RUNTIME_ID
    }

    fn supports(&self, model: &TranscriptionModelCatalogEntry) -> bool {
        model.artifact_format == TranscriptionModelArtifactFormat::CoreMlBundle
    }

    fn probe(&self, model: &TranscriptionModelCatalogEntry) -> RuntimeCapability {
        if !self.supports(model) {
            return RuntimeCapability::Unavailable;
        }

        #[cfg(target_os = "macos")]
        {
            RuntimeCapability::Ready
        }

        #[cfg(not(target_os = "macos"))]
        {
            RuntimeCapability::UnsupportedPlatform
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NativeParakeetRuntime;

impl TranscriptionRuntime for NativeParakeetRuntime {
    fn id(&self) -> &'static str {
        "native_parakeet"
    }

    fn supports(&self, model: &TranscriptionModelCatalogEntry) -> bool {
        CoreMlParakeetRuntime.supports(model)
    }

    fn probe(&self, model: &TranscriptionModelCatalogEntry) -> RuntimeCapability {
        CoreMlParakeetRuntime.probe(model)
    }
}

pub fn select_transcription_runtime<R>(
    model: &TranscriptionModelCatalogEntry,
    runtime: &R,
) -> RuntimeSelection
where
    R: TranscriptionRuntime,
{
    if !runtime.supports(model) {
        return RuntimeSelection::Unavailable;
    }

    match runtime.probe(model) {
        RuntimeCapability::Ready => RuntimeSelection::Native,
        RuntimeCapability::UnsupportedPlatform => RuntimeSelection::UnsupportedPlatform,
        RuntimeCapability::Unavailable => RuntimeSelection::Unavailable,
    }
}

#[cfg(target_os = "macos")]
fn run_coreml_parakeet(
    job: &TranscriptionRuntimeJob,
    model_id: &str,
    helper_path: PathBuf,
) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
    let model_root = Path::new(&job.model_path);
    validate_coreml_parakeet_model(model_root)?;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CoreMlInspectionReport {
        runtime_id: String,
    }

    let inspection_report_path = model_root.join(COREML_INSPECTION_FILE_NAME);
    if !inspection_report_path.is_file() {
        return Err(TranscriptionRuntimeError::Inference(format!(
            "missing Core ML inspection report {}",
            inspection_report_path.display()
        )));
    }

    let inspection_report_text =
        std::fs::read_to_string(&inspection_report_path).map_err(|error| {
            TranscriptionRuntimeError::Inference(format!(
                "failed to read Core ML inspection report {}: {}",
                inspection_report_path.display(),
                error
            ))
        })?;
    let inspection_report: CoreMlInspectionReport =
        match serde_json::from_str(&inspection_report_text) {
            Ok(report) => report,
            Err(error) => {
                return Err(TranscriptionRuntimeError::Inference(format!(
                    "failed to parse Core ML inspection report {}: {}",
                    inspection_report_path.display(),
                    error
                )));
            }
        };
    if inspection_report.runtime_id != FLUID_AUDIO_COREML_RUNTIME_ID {
        return Err(TranscriptionRuntimeError::Inference(format!(
            "Core ML inspection report runtimeId mismatch at {}: expected {}, found {}",
            inspection_report_path.display(),
            FLUID_AUDIO_COREML_RUNTIME_ID,
            inspection_report.runtime_id
        )));
    }

    let installed_model = InstalledModel {
        model_id: model_id.to_string(),
        model_dir: model_root.to_path_buf(),
        artifact_format: ModelArtifactFormat::from(TranscriptionModelArtifactFormat::CoreMlBundle),
        modality: ModelModality::Transcription,
        runtime_family: FLUID_AUDIO_COREML_RUNTIME_ID.to_string(),
    };
    FluidAudioCoreMlRuntime::new(helper_path).transcribe_installed(job, &installed_model)
}

#[cfg(target_os = "macos")]
fn validate_coreml_parakeet_model(model_root: &Path) -> Result<(), TranscriptionRuntimeError> {
    for bundle in [
        "Preprocessor.mlmodelc",
        "Encoder.mlmodelc",
        "Decoder.mlmodelc",
        "JointDecisionv3.mlmodelc",
    ] {
        let bundle_path = model_root.join(bundle);
        if !bundle_path.is_dir() {
            return Err(TranscriptionRuntimeError::Inference(format!(
                "missing Core ML bundle {}",
                bundle_path.display()
            )));
        }
    }

    Ok(())
}
