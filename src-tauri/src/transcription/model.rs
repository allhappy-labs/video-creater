use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const FLUID_AUDIO_COREML_RUNTIME_ID: &str = "fluid_audio_coreml";
pub const FLUID_AUDIO_COREML_RUNTIME_MODEL_DIR_NAME: &str = "fluid-audio-coreml";
pub const FLUID_AUDIO_PARAKEET_V3_COREML_REPO_ID: &str =
    "FluidInference/parakeet-tdt-0.6b-v3-coreml";
pub const FLUID_AUDIO_PARAKEET_V3_COREML_REVISION: &str =
    "aed02740059203c4a87495924f685de3722ae9ce";
pub const SHERPA_ONNX_RUNTIME_ID: &str = "sherpa_onnx";
pub const SHERPA_ONNX_RUNTIME_MODEL_DIR_NAME: &str = "sherpa-onnx";
pub const SHERPA_ONNX_PARAKEET_V3_REPO_ID: &str =
    "csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8";
pub const SHERPA_ONNX_PARAKEET_V3_REVISION: &str = "2bda32ec70b097a55adaa07d9a7173915b43cc78";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionModelCatalogEntry {
    pub id: &'static str,
    pub display_name: &'static str,
    pub provider: &'static str,
    pub modality: ModelModality,
    pub family: TranscriptionModelFamily,
    pub revision: &'static str,
    pub artifact_format: TranscriptionModelArtifactFormat,
    pub approximate_size_bytes: u64,
    pub supported_runtimes: Vec<TranscriptionRuntimeId>,
    pub required_files: Vec<TranscriptionModelFile>,
    pub artifact_sources: Vec<ModelArtifactSource>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelModality {
    Transcription,
    ImageGeneration,
    VideoGeneration,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelArtifactProviderKind {
    HuggingFaceHub,
    LocalImport,
    HttpMirror,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelArtifactSource {
    pub provider: ModelArtifactProviderKind,
    pub repo_id: String,
    pub revision: String,
    pub path_prefix: Option<String>,
    pub include_files: Vec<String>,
    pub license: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionModelFamily {
    Parakeet,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionModelArtifactFormat {
    CoreMlBundle,
    SherpaOnnxTransducer,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionRuntimeId {
    #[serde(rename = "fluid_audio_coreml")]
    FluidAudioCoreMl,
    #[serde(rename = "sherpa_onnx")]
    SherpaOnnx,
}

impl TranscriptionRuntimeId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FluidAudioCoreMl => FLUID_AUDIO_COREML_RUNTIME_ID,
            Self::SherpaOnnx => SHERPA_ONNX_RUNTIME_ID,
        }
    }

    /// Short user-facing name of the on-device helper technology.
    pub fn display_label(self) -> &'static str {
        match self {
            Self::FluidAudioCoreMl => "Core ML",
            Self::SherpaOnnx => "ONNX",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionModelFile {
    pub path: String,
    pub size_bytes: Option<u64>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelInstallStatus {
    Missing,
    Downloading,
    Verifying,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModelManifest {
    #[serde(default = "installed_model_manifest_schema_version")]
    pub schema_version: u32,
    pub model_id: String,
    pub revision: String,
    #[serde(default)]
    pub modality: Option<ModelModality>,
    #[serde(default)]
    pub artifact_format: Option<TranscriptionModelArtifactFormat>,
    #[serde(default)]
    pub source: Option<InstalledModelSource>,
    pub installed_files: Vec<TranscriptionModelFile>,
    pub installed_at: String,
    pub verified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModelSource {
    pub provider: ModelArtifactProviderKind,
    pub repo_id: String,
    pub revision: String,
    pub license: Option<String>,
}

fn installed_model_manifest_schema_version() -> u32 {
    1
}

/// The Parakeet TDT 0.6B v3 entry for the current platform: Core ML bundles run by the FluidAudio
/// helper everywhere except Linux, which uses the sherpa-onnx INT8 ONNX export.
pub fn parakeet_v3_catalog_entry() -> TranscriptionModelCatalogEntry {
    #[cfg(target_os = "linux")]
    {
        parakeet_v3_sherpa_onnx_catalog_entry()
    }

    #[cfg(not(target_os = "linux"))]
    {
        parakeet_v3_coreml_catalog_entry()
    }
}

pub fn parakeet_v3_coreml_catalog_entry() -> TranscriptionModelCatalogEntry {
    let required_files = parakeet_v3_required_files();

    TranscriptionModelCatalogEntry {
        id: "nvidia/parakeet-tdt-0.6b-v3",
        display_name: "Parakeet TDT 0.6B v3",
        provider: "NVIDIA",
        modality: ModelModality::Transcription,
        family: TranscriptionModelFamily::Parakeet,
        revision: "main",
        artifact_format: TranscriptionModelArtifactFormat::CoreMlBundle,
        approximate_size_bytes: 485_000_000,
        supported_runtimes: vec![TranscriptionRuntimeId::FluidAudioCoreMl],
        artifact_sources: vec![parakeet_v3_coreml_source(&required_files)],
        required_files,
    }
}

fn parakeet_v3_required_files() -> Vec<TranscriptionModelFile> {
    vec![
        required_file("config.json"),
        required_file("parakeet_v3_vocab.json"),
        required_file("Encoder.mlmodelc/coremldata.bin"),
        required_file("Encoder.mlmodelc/metadata.json"),
        required_file("Encoder.mlmodelc/model.mil"),
        required_file("Encoder.mlmodelc/weights/weight.bin"),
        required_file("Decoder.mlmodelc/coremldata.bin"),
        required_file("Decoder.mlmodelc/metadata.json"),
        required_file("Decoder.mlmodelc/model.mil"),
        required_file("Decoder.mlmodelc/weights/weight.bin"),
        required_file("JointDecisionv3.mlmodelc/coremldata.bin"),
        required_file("JointDecisionv3.mlmodelc/metadata.json"),
        required_file("JointDecisionv3.mlmodelc/model.mil"),
        required_file("JointDecisionv3.mlmodelc/weights/weight.bin"),
        required_file("Preprocessor.mlmodelc/coremldata.bin"),
        required_file("Preprocessor.mlmodelc/metadata.json"),
        required_file("Preprocessor.mlmodelc/model.mil"),
        required_file("Preprocessor.mlmodelc/weights/weight.bin"),
    ]
}

fn parakeet_v3_coreml_source(include_files: &[TranscriptionModelFile]) -> ModelArtifactSource {
    ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: FLUID_AUDIO_PARAKEET_V3_COREML_REPO_ID.to_string(),
        revision: FLUID_AUDIO_PARAKEET_V3_COREML_REVISION.to_string(),
        path_prefix: None,
        include_files: include_files.iter().map(|file| file.path.clone()).collect(),
        license: Some("cc-by-4.0".to_string()),
    }
}

pub fn parakeet_v3_sherpa_onnx_catalog_entry() -> TranscriptionModelCatalogEntry {
    let required_files = vec![
        pinned_file(
            "encoder.int8.onnx",
            652_184_281,
            "acfc2b4456377e15d04f0243af540b7fe7c992f8d898d751cf134c3a55fd2247",
        ),
        pinned_file(
            "decoder.int8.onnx",
            11_845_275,
            "179e50c43d1a9de79c8a24149a2f9bac6eb5981823f2a2ed88d655b24248db4e",
        ),
        pinned_file(
            "joiner.int8.onnx",
            6_355_277,
            "3164c13fc2821009440d20fcb5fdc78bff28b4db2f8d0f0b329101719c0948b3",
        ),
        pinned_file(
            "tokens.txt",
            93_939,
            "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d",
        ),
    ];

    TranscriptionModelCatalogEntry {
        id: "nvidia/parakeet-tdt-0.6b-v3",
        display_name: "Parakeet TDT 0.6B v3",
        provider: "NVIDIA",
        modality: ModelModality::Transcription,
        family: TranscriptionModelFamily::Parakeet,
        revision: "main",
        artifact_format: TranscriptionModelArtifactFormat::SherpaOnnxTransducer,
        approximate_size_bytes: 670_478_772,
        supported_runtimes: vec![TranscriptionRuntimeId::SherpaOnnx],
        artifact_sources: vec![ModelArtifactSource {
            provider: ModelArtifactProviderKind::HuggingFaceHub,
            repo_id: SHERPA_ONNX_PARAKEET_V3_REPO_ID.to_string(),
            revision: SHERPA_ONNX_PARAKEET_V3_REVISION.to_string(),
            path_prefix: None,
            include_files: required_files
                .iter()
                .map(|file| file.path.clone())
                .collect(),
            license: Some("cc-by-4.0".to_string()),
        }],
        required_files,
    }
}

/// Runtime id that owns an entry's artifacts (the first supported runtime).
pub fn catalog_entry_runtime_id(entry: &TranscriptionModelCatalogEntry) -> &'static str {
    entry
        .supported_runtimes
        .first()
        .copied()
        .unwrap_or(TranscriptionRuntimeId::FluidAudioCoreMl)
        .as_str()
}

pub fn catalog_entry_runtime_label(entry: &TranscriptionModelCatalogEntry) -> &'static str {
    entry
        .supported_runtimes
        .first()
        .copied()
        .unwrap_or(TranscriptionRuntimeId::FluidAudioCoreMl)
        .display_label()
}

#[cfg(test)]
thread_local! {
    static TEST_CATALOG_OVERRIDE: std::cell::RefCell<Option<Vec<TranscriptionModelCatalogEntry>>> =
        const { std::cell::RefCell::new(None) };
}

/// Unit tests that fabricate small fixture artifacts use the unpinned Core ML catalog shape on
/// every platform; the override is scoped to the calling test thread.
#[cfg(test)]
pub(crate) fn override_catalog_for_current_test_thread(
    entries: Vec<TranscriptionModelCatalogEntry>,
) {
    TEST_CATALOG_OVERRIDE.with(|catalog| *catalog.borrow_mut() = Some(entries));
}

pub fn transcription_model_catalog() -> Vec<TranscriptionModelCatalogEntry> {
    #[cfg(test)]
    if let Some(entries) = TEST_CATALOG_OVERRIDE.with(|catalog| catalog.borrow().clone()) {
        return entries;
    }
    vec![parakeet_v3_catalog_entry()]
}

pub fn transcription_model_catalog_entry(model_id: &str) -> Option<TranscriptionModelCatalogEntry> {
    transcription_model_catalog()
        .into_iter()
        .find(|entry| entry.id == model_id)
}

pub fn safe_model_dir_name(model_id: &str) -> String {
    model_id.replace('/', "__")
}

impl InstalledModelManifest {
    pub fn validate_against(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        model_root: &Path,
    ) -> ModelInstallStatus {
        if self.model_id != entry.id || self.revision != entry.revision {
            return ModelInstallStatus::Failed;
        }
        if self
            .modality
            .is_some_and(|modality| modality != entry.modality)
        {
            return ModelInstallStatus::Failed;
        }
        if self
            .artifact_format
            .is_some_and(|artifact_format| artifact_format != entry.artifact_format)
        {
            return ModelInstallStatus::Failed;
        }

        let model_dir = model_root.join(safe_model_dir_name(entry.id));
        let required_files_dir = required_files_dir_for_entry(entry, &model_dir);
        for required in &entry.required_files {
            let Some(installed) = self
                .installed_files
                .iter()
                .find(|installed| installed.path == required.path)
            else {
                return ModelInstallStatus::Failed;
            };

            let path = required_files_dir.join(&required.path);
            let Ok(metadata) = path.metadata() else {
                return ModelInstallStatus::Failed;
            };
            if !metadata.is_file() || metadata.len() == 0 {
                return ModelInstallStatus::Failed;
            }
            if installed
                .size_bytes
                .is_some_and(|size_bytes| size_bytes != metadata.len())
            {
                return ModelInstallStatus::Failed;
            }
            if let Some(expected_sha256) = &installed.sha256 {
                let Ok(actual_sha256) = sha256_file(&path) else {
                    return ModelInstallStatus::Failed;
                };
                if &actual_sha256 != expected_sha256 {
                    return ModelInstallStatus::Failed;
                }
            }
        }

        if self.verified_at.is_some() {
            ModelInstallStatus::Ready
        } else {
            ModelInstallStatus::Verifying
        }
    }
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn runtime_model_subdir_name(runtime_id: &str) -> Option<&'static str> {
    match runtime_id {
        FLUID_AUDIO_COREML_RUNTIME_ID => Some(FLUID_AUDIO_COREML_RUNTIME_MODEL_DIR_NAME),
        SHERPA_ONNX_RUNTIME_ID => Some(SHERPA_ONNX_RUNTIME_MODEL_DIR_NAME),
        _ => None,
    }
}

pub fn required_files_dir_for_entry(
    entry: &TranscriptionModelCatalogEntry,
    model_dir: &Path,
) -> PathBuf {
    if entry.artifact_format == TranscriptionModelArtifactFormat::CoreMlBundle {
        return model_dir.join(FLUID_AUDIO_COREML_RUNTIME_MODEL_DIR_NAME);
    }
    if entry.artifact_format == TranscriptionModelArtifactFormat::SherpaOnnxTransducer {
        return model_dir.join(SHERPA_ONNX_RUNTIME_MODEL_DIR_NAME);
    }

    model_dir.to_path_buf()
}

fn pinned_file(path: &str, size_bytes: u64, sha256: &str) -> TranscriptionModelFile {
    TranscriptionModelFile {
        path: path.to_string(),
        size_bytes: Some(size_bytes),
        sha256: Some(sha256.to_string()),
    }
}

fn required_file(path: &str) -> TranscriptionModelFile {
    TranscriptionModelFile {
        path: path.to_string(),
        size_bytes: None,
        sha256: None,
    }
}
