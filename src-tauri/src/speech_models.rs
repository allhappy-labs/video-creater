use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;

#[cfg(not(target_os = "linux"))]
pub const SPEECH_ANALYSIS_RUNTIME_ID: &str = "fluid_audio_speech_analysis";
#[cfg(not(target_os = "linux"))]
pub const SPEECH_ANALYSIS_MODEL_SET_ID: &str = "silero-vad+wspk-vbx-v1";
#[cfg(not(target_os = "linux"))]
pub const VAD_REPO: &str = "FluidInference/silero-vad-coreml";
#[cfg(not(target_os = "linux"))]
pub const VAD_REVISION: &str = "b419383c55c110e2c9271fa6ee0ea83d03c70d96";
#[cfg(not(target_os = "linux"))]
pub const DIARIZATION_REPO: &str = "FluidInference/speaker-diarization-coreml";
#[cfg(not(target_os = "linux"))]
pub const DIARIZATION_REVISION: &str = "1ed7a662fdc7109e36d822db793ee6eebdaf8594";
#[cfg(not(target_os = "linux"))]
const ARTIFACT_FORMAT: &str = "compiled_core_ml_bundles";

// Linux: ONNX models run by the `video-creater-speech` helper (sherpa-onnx + ONNX Runtime).
#[cfg(target_os = "linux")]
pub const SPEECH_ANALYSIS_RUNTIME_ID: &str = "sherpa_onnx_speech_analysis";
#[cfg(target_os = "linux")]
pub const SPEECH_ANALYSIS_MODEL_SET_ID: &str =
    "silero-vad-v6+pyannote-seg3+wespeaker-r34lm-onnx-v1";
/// Upstream Silero VAD v6.0 (MIT), fetched from GitHub at an immutable commit.
#[cfg(target_os = "linux")]
pub const VAD_REPO: &str = "snakers4/silero-vad";
#[cfg(target_os = "linux")]
pub const VAD_REVISION: &str = "fba061dc5559f696e62171e9a0741782b0fdc23c";
/// pyannote segmentation 3.0 ONNX export (MIT).
#[cfg(target_os = "linux")]
pub const DIARIZATION_REPO: &str = "csukuangfj/sherpa-onnx-pyannote-segmentation-3-0";
#[cfg(target_os = "linux")]
pub const DIARIZATION_REVISION: &str = "9403a6902bb58e3d5ae8c7e77c3422de279db2e0";
/// WeSpeaker VoxCeleb ResNet34-LM speaker embeddings, 256 dimensions (CC-BY-4.0).
#[cfg(target_os = "linux")]
pub const SPEAKER_EMBEDDING_REPO: &str = "csukuangfj/speaker-embedding-models";
#[cfg(target_os = "linux")]
pub const SPEAKER_EMBEDDING_REVISION: &str = "0743f301363dec56491a490f6d6cbc9d67f9a3bf";
#[cfg(target_os = "linux")]
const ARTIFACT_FORMAT: &str = "onnx_models";

#[derive(Debug, Error)]
pub enum SpeechModelError {
    #[error("speech model download cancelled")]
    Cancelled,
    #[error("speech model download failed: {0}")]
    Download(String),
    #[error("speech model I/O failed: {0}")]
    Io(String),
    #[error("speech model verification failed: {0}")]
    Verification(String),
    #[error("speech model progress could not be persisted: {0}")]
    ProgressPersistence(String),
    #[error("speech model removal failed: {0}")]
    Remove(String),
}

impl SpeechModelError {
    pub fn stable_code(&self) -> &'static str {
        match self {
            Self::Cancelled => "speechModels.download.cancelled",
            Self::Download(_) => "speechModels.download.failed",
            Self::Io(_) => "speechModels.io.failed",
            Self::Verification(_) => "speechModels.verification.failed",
            Self::ProgressPersistence(_) => "speechModels.progressPersistenceFailed",
            Self::Remove(_) => "speechModels.remove.failed",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ModelFile {
    repo: &'static str,
    revision: &'static str,
    repo_dir: &'static str,
    path: &'static str,
    bytes: u64,
    sha256: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProductionSpeechModelStatus {
    pub model_set_id: String,
    pub runtime_id: String,
    pub ready: bool,
    pub installed_files: u32,
    pub total_files: u32,
    pub installed_bytes: u64,
    pub total_bytes: u64,
    pub root_path: String,
    pub vad_repo: String,
    pub vad_revision: String,
    pub diarization_repo: String,
    pub diarization_revision: String,
    pub artifact_format: String,
    pub licenses: Vec<String>,
    pub last_error_code: Option<String>,
    pub last_error_detail: Option<String>,
    pub last_error_recovery_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct PersistedSpeechModelError {
    code: String,
    detail: String,
    recovery_action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeechModelProgress {
    pub downloaded_files: u32,
    pub total_files: u32,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct ProductionSpeechModelStore {
    root: PathBuf,
}

impl ProductionSpeechModelStore {
    pub fn new(model_root: PathBuf) -> Self {
        Self {
            root: model_root.join("speech-analysis/production-v1"),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn status(&self) -> ProductionSpeechModelStatus {
        let mut installed_files = 0_u32;
        let mut installed_bytes = 0_u64;
        for file in MODEL_FILES {
            let path = self.file_path(file);
            if path
                .metadata()
                .is_ok_and(|metadata| metadata.is_file() && metadata.len() == file.bytes)
                && sha256_file(&path).is_ok_and(|sha| sha == file.sha256)
            {
                installed_files += 1;
                installed_bytes += file.bytes;
            }
        }
        let persisted_error = self.read_last_error();
        ProductionSpeechModelStatus {
            model_set_id: SPEECH_ANALYSIS_MODEL_SET_ID.to_string(),
            runtime_id: SPEECH_ANALYSIS_RUNTIME_ID.to_string(),
            ready: installed_files as usize == MODEL_FILES.len(),
            installed_files,
            total_files: MODEL_FILES.len() as u32,
            installed_bytes,
            total_bytes: MODEL_FILES.iter().map(|file| file.bytes).sum(),
            root_path: self.root.display().to_string(),
            vad_repo: VAD_REPO.to_string(),
            vad_revision: VAD_REVISION.to_string(),
            diarization_repo: DIARIZATION_REPO.to_string(),
            diarization_revision: DIARIZATION_REVISION.to_string(),
            artifact_format: ARTIFACT_FORMAT.to_string(),
            licenses: vec!["MIT".to_string(), "CC-BY-4.0".to_string()],
            last_error_code: persisted_error.as_ref().map(|error| error.code.clone()),
            last_error_detail: persisted_error.as_ref().map(|error| error.detail.clone()),
            last_error_recovery_action: persisted_error.map(|error| error.recovery_action),
        }
    }

    pub fn download_and_verify(&self) -> Result<ProductionSpeechModelStatus, SpeechModelError> {
        self.download_and_verify_with_observer(|_| Ok(()))
    }

    pub fn download_and_verify_with_observer<F>(
        &self,
        observer: F,
    ) -> Result<ProductionSpeechModelStatus, SpeechModelError>
    where
        F: FnMut(SpeechModelProgress) -> Result<(), SpeechModelError>,
    {
        self.download_and_verify_with_cancellation(observer, || Ok(()))
    }

    /// Check cancellation before requesting files, between chunks, and before publishing a file.
    pub fn download_and_verify_with_cancellation<F, C>(
        &self,
        observer: F,
        check_cancelled: C,
    ) -> Result<ProductionSpeechModelStatus, SpeechModelError>
    where
        F: FnMut(SpeechModelProgress) -> Result<(), SpeechModelError>,
        C: Fn() -> Result<(), SpeechModelError>,
    {
        let result = self.download_and_verify_with_observer_inner(observer, check_cancelled);
        match result {
            Ok(status) => Ok(status),
            Err(error) => {
                self.persist_last_error(&error)?;
                Err(error)
            }
        }
    }

    fn download_and_verify_with_observer_inner<F, C>(
        &self,
        observer: F,
        check_cancelled: C,
    ) -> Result<ProductionSpeechModelStatus, SpeechModelError>
    where
        F: FnMut(SpeechModelProgress) -> Result<(), SpeechModelError>,
        C: Fn() -> Result<(), SpeechModelError>,
    {
        check_cancelled()?;
        #[cfg(feature = "web-host")]
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| SpeechModelError::Download(error.to_string()))?;
        #[cfg(feature = "web-host")]
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .read_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(600))
            .user_agent("video-creater-production-speech-models/1")
            .build()
            .map_err(|error| SpeechModelError::Download(error.to_string()))?;
        #[cfg(not(feature = "web-host"))]
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(120))
            .user_agent("video-creater-production-speech-models/1")
            .build()
            .map_err(|error| SpeechModelError::Download(error.to_string()))?;
        self.download_files_cancellable_with(
            MODEL_FILES,
            |file, partial| {
                check_cancelled()?;
                let mut output = fs::File::create(partial)
                    .map_err(|error| SpeechModelError::Io(error.to_string()))?;
                #[cfg(feature = "web-host")]
                runtime.block_on(async {
                    let mut response = client
                        .get(model_file_url(file))
                        .send()
                        .await
                        .and_then(reqwest::Response::error_for_status)
                        .map_err(|error| SpeechModelError::Download(error.to_string()))?;
                    let mut downloaded = 0;
                    loop {
                        check_cancelled()?;
                        let Some(chunk) = response
                            .chunk()
                            .await
                            .map_err(|error| SpeechModelError::Download(error.to_string()))?
                        else {
                            break;
                        };
                        check_cancelled()?;
                        write_model_chunk(&mut output, &chunk, &mut downloaded, file.bytes)?;
                    }
                    Ok::<_, SpeechModelError>(())
                })?;
                #[cfg(not(feature = "web-host"))]
                {
                    let mut response = client
                        .get(model_file_url(file))
                        .send()
                        .and_then(reqwest::blocking::Response::error_for_status)
                        .map_err(|error| SpeechModelError::Download(error.to_string()))?;
                    copy_model_download(&mut response, &mut output, file.bytes, &check_cancelled)?;
                }
                output
                    .flush()
                    .map_err(|error| SpeechModelError::Io(error.to_string()))
            },
            observer,
            &check_cancelled,
        )?;
        check_cancelled()?;
        self.verify_files()?;
        self.clear_last_error()?;
        let status = self.status();
        let manifest = serde_json::to_vec_pretty(&status)
            .map_err(|error| SpeechModelError::Io(error.to_string()))?;
        atomic_write(&self.root.join("manifest.json"), &manifest)?;
        atomic_write(
            &self.root.join("ATTRIBUTION.txt"),
            attribution_text().as_bytes(),
        )?;
        Ok(status)
    }

    pub fn verify(&self) -> Result<ProductionSpeechModelStatus, SpeechModelError> {
        let result = self.verify_files();
        match result {
            Ok(()) => {
                self.clear_last_error()?;
                Ok(self.status())
            }
            Err(error) => {
                self.persist_last_error(&error)?;
                Err(error)
            }
        }
    }

    fn verify_files(&self) -> Result<(), SpeechModelError> {
        for file in MODEL_FILES {
            verify_file(&self.file_path(file), file)?;
        }
        let status = self.status();
        if !status.ready {
            return Err(SpeechModelError::Verification(
                "installed model set is incomplete".to_string(),
            ));
        }
        Ok(())
    }

    pub fn remove(&self) -> Result<ProductionSpeechModelStatus, SpeechModelError> {
        if self.root.exists() {
            fs::remove_dir_all(&self.root)
                .map_err(|error| SpeechModelError::Remove(error.to_string()))?;
        }
        Ok(self.status())
    }

    pub fn clear_last_error(&self) -> Result<(), SpeechModelError> {
        let path = self.last_error_path();
        if path.exists() {
            fs::remove_file(path).map_err(|error| SpeechModelError::Io(error.to_string()))?;
        }
        Ok(())
    }

    pub fn record_error(&self, error: &SpeechModelError) -> Result<(), SpeechModelError> {
        self.persist_last_error(error)
    }

    fn persist_last_error(&self, error: &SpeechModelError) -> Result<(), SpeechModelError> {
        let persisted = PersistedSpeechModelError {
            code: error.stable_code().to_string(),
            detail: error.to_string(),
            recovery_action: "Retry the verified speech model setup.".to_string(),
        };
        let bytes = serde_json::to_vec_pretty(&persisted)
            .map_err(|serialize_error| SpeechModelError::Io(serialize_error.to_string()))?;
        atomic_write(&self.last_error_path(), &bytes)
    }

    fn read_last_error(&self) -> Option<PersistedSpeechModelError> {
        serde_json::from_slice(&fs::read(self.last_error_path()).ok()?).ok()
    }

    fn last_error_path(&self) -> PathBuf {
        self.root.join(".last-error.json")
    }

    #[cfg(test)]
    fn download_files_with<D, F>(
        &self,
        files: &[ModelFile],
        download: D,
        observer: F,
    ) -> Result<(), SpeechModelError>
    where
        D: FnMut(&ModelFile, &Path) -> Result<(), SpeechModelError>,
        F: FnMut(SpeechModelProgress) -> Result<(), SpeechModelError>,
    {
        self.download_files_cancellable_with(files, download, observer, || Ok(()))
    }

    fn download_files_cancellable_with<D, F, C>(
        &self,
        files: &[ModelFile],
        mut download: D,
        mut observer: F,
        check_cancelled: C,
    ) -> Result<(), SpeechModelError>
    where
        D: FnMut(&ModelFile, &Path) -> Result<(), SpeechModelError>,
        F: FnMut(SpeechModelProgress) -> Result<(), SpeechModelError>,
        C: Fn() -> Result<(), SpeechModelError>,
    {
        let total_bytes = files.iter().map(|file| file.bytes).sum();
        let mut downloaded_bytes = 0_u64;
        for (index, file) in files.iter().enumerate() {
            check_cancelled()?;
            let destination = self.file_path(file);
            if verify_file(&destination, file).is_err() {
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|error| SpeechModelError::Io(error.to_string()))?;
                }
                let partial = destination.with_extension(format!(
                    "{}.partial",
                    destination
                        .extension()
                        .and_then(|value| value.to_str())
                        .unwrap_or("file")
                ));
                let result = (|| {
                    download(file, &partial)?;
                    verify_file(&partial, file)?;
                    check_cancelled()?;
                    fs::rename(&partial, &destination)
                        .map_err(|error| SpeechModelError::Io(error.to_string()))
                })();
                if result.is_err() {
                    let _ = fs::remove_file(&partial);
                }
                result?;
            }
            downloaded_bytes += file.bytes;
            observer(SpeechModelProgress {
                downloaded_files: (index + 1) as u32,
                total_files: files.len() as u32,
                downloaded_bytes,
                total_bytes,
            })?;
        }
        Ok(())
    }

    fn file_path(&self, file: &ModelFile) -> PathBuf {
        self.root.join(file.repo_dir).join(file.path)
    }
}

fn write_model_chunk<W: Write>(
    output: &mut W,
    chunk: &[u8],
    downloaded: &mut u64,
    expected_bytes: u64,
) -> Result<(), SpeechModelError> {
    let next = downloaded
        .checked_add(chunk.len() as u64)
        .filter(|bytes| *bytes <= expected_bytes)
        .ok_or_else(|| {
            SpeechModelError::Verification("download exceeds pinned model size".into())
        })?;
    output
        .write_all(chunk)
        .map_err(|error| SpeechModelError::Io(error.to_string()))?;
    *downloaded = next;
    Ok(())
}

#[cfg(any(not(feature = "web-host"), test))]
fn copy_model_download<R: Read, W: Write, C: Fn() -> Result<(), SpeechModelError>>(
    input: &mut R,
    output: &mut W,
    expected_bytes: u64,
    check_cancelled: C,
) -> Result<(), SpeechModelError> {
    let mut buffer = [0_u8; 64 * 1024];
    let mut downloaded = 0;
    loop {
        check_cancelled()?;
        let count = input
            .read(&mut buffer)
            .map_err(|error| SpeechModelError::Io(error.to_string()))?;
        check_cancelled()?;
        if count == 0 {
            return Ok(());
        }
        write_model_chunk(output, &buffer[..count], &mut downloaded, expected_bytes)?;
    }
}

#[cfg(not(target_os = "linux"))]
fn model_file_url(file: &ModelFile) -> String {
    format!(
        "https://huggingface.co/{}/resolve/{}/{}",
        file.repo, file.revision, file.path
    )
}

#[cfg(target_os = "linux")]
fn model_file_url(file: &ModelFile) -> String {
    if file.repo == VAD_REPO {
        return format!(
            "https://raw.githubusercontent.com/{}/{}/src/silero_vad/data/{}",
            file.repo, file.revision, file.path
        );
    }
    format!(
        "https://huggingface.co/{}/resolve/{}/{}",
        file.repo, file.revision, file.path
    )
}

#[cfg(not(target_os = "linux"))]
fn attribution_text() -> String {
    format!(
        "Silero VAD Core ML\nSource: https://huggingface.co/{VAD_REPO}/tree/{VAD_REVISION}\nLicense: MIT\n\nFluidAudio speaker diarization Core ML (Pyannote segmentation, WeSpeaker embeddings, VBx)\nSource: https://huggingface.co/{DIARIZATION_REPO}/tree/{DIARIZATION_REVISION}\nLicense: CC BY 4.0\n"
    )
}

#[cfg(target_os = "linux")]
fn attribution_text() -> String {
    format!(
        "Silero VAD v6.0 (ONNX)\nSource: https://github.com/{VAD_REPO}/tree/{VAD_REVISION}\nLicense: MIT\n\npyannote segmentation 3.0 (ONNX export for sherpa-onnx)\nSource: https://huggingface.co/{DIARIZATION_REPO}/tree/{DIARIZATION_REVISION}\nUpstream: https://huggingface.co/pyannote/segmentation-3.0\nLicense: MIT\n\nWeSpeaker VoxCeleb ResNet34-LM speaker embedding (ONNX)\nSource: https://huggingface.co/{SPEAKER_EMBEDDING_REPO}/tree/{SPEAKER_EMBEDDING_REVISION}\nUpstream: https://github.com/wenet-e2e/wespeaker (see also https://huggingface.co/pyannote/wespeaker-voxceleb-resnet34-LM)\nLicense: CC BY 4.0\n"
    )
}

fn verify_file(path: &Path, file: &ModelFile) -> Result<(), SpeechModelError> {
    let metadata = path
        .metadata()
        .map_err(|error| SpeechModelError::Verification(format!("{}: {error}", path.display())))?;
    if !metadata.is_file() || metadata.len() != file.bytes {
        return Err(SpeechModelError::Verification(format!(
            "{} has {} bytes; expected {}",
            path.display(),
            metadata.len(),
            file.bytes
        )));
    }
    let actual = sha256_file(path).map_err(|error| SpeechModelError::Io(error.to_string()))?;
    if actual != file.sha256 {
        return Err(SpeechModelError::Verification(format!(
            "{} SHA-256 mismatch",
            path.display()
        )));
    }
    Ok(())
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut input = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), SpeechModelError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| SpeechModelError::Io(error.to_string()))?;
    }
    let temporary = path.with_extension("partial");
    fs::write(&temporary, bytes).map_err(|error| SpeechModelError::Io(error.to_string()))?;
    fs::rename(temporary, path).map_err(|error| SpeechModelError::Io(error.to_string()))
}

macro_rules! model_file {
    ($repo:expr, $revision:expr, $dir:expr, $path:expr, $bytes:expr, $sha:expr) => {
        ModelFile {
            repo: $repo,
            revision: $revision,
            repo_dir: $dir,
            path: $path,
            bytes: $bytes,
            sha256: $sha,
        }
    };
}

#[cfg(target_os = "linux")]
const MODEL_FILES: &[ModelFile] = &[
    model_file!(
        VAD_REPO,
        VAD_REVISION,
        "silero-vad",
        "silero_vad.onnx",
        2327524,
        "597d30b3ec076608d059477bb14cfeffdf951bf5cae370d38f65d33bbfe82004"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "pyannote-segmentation-3-0",
        "model.onnx",
        5992913,
        "220ad67ca923bef2fa91f2390c786097bf305bceb5e261d4af67b38e938e1079"
    ),
    model_file!(
        SPEAKER_EMBEDDING_REPO,
        SPEAKER_EMBEDDING_REVISION,
        "speaker-embedding-models",
        "wespeaker_en_voxceleb_resnet34_LM.onnx",
        26530550,
        "e9848563da86f263117134dfd7ad63c92355b37de492b55e325400c9d9c39012"
    ),
];

#[cfg(not(target_os = "linux"))]
const MODEL_FILES: &[ModelFile] = &[
    model_file!(
        VAD_REPO,
        VAD_REVISION,
        "silero-vad-coreml",
        "silero-vad-unified-256ms-v6.0.0.mlmodelc/analytics/coremldata.bin",
        243,
        "30945d54e32c3f15ec35dc6ee32128a27a6cdc03b0a12ffab04434069c49dfb5"
    ),
    model_file!(
        VAD_REPO,
        VAD_REVISION,
        "silero-vad-coreml",
        "silero-vad-unified-256ms-v6.0.0.mlmodelc/coremldata.bin",
        625,
        "0c3063bd09ba71c26ede0308d7c33591d0770e971a3fcc603ccad7ba1e8fb88d"
    ),
    model_file!(
        VAD_REPO,
        VAD_REVISION,
        "silero-vad-coreml",
        "silero-vad-unified-256ms-v6.0.0.mlmodelc/metadata.json",
        3335,
        "e00405801b86542dbb722a2ec2fff285e539836990f4fdc6aa321ce1105eb32c"
    ),
    model_file!(
        VAD_REPO,
        VAD_REVISION,
        "silero-vad-coreml",
        "silero-vad-unified-256ms-v6.0.0.mlmodelc/model.mil",
        176918,
        "4f93e2b5920e851fbc0be1c21a2a76e170124467ed7b01125190aa32e795f8af"
    ),
    model_file!(
        VAD_REPO,
        VAD_REVISION,
        "silero-vad-coreml",
        "silero-vad-unified-256ms-v6.0.0.mlmodelc/weights/weight.bin",
        882304,
        "853cf34740d3f5061f977ebe2976f7c921b064261c9c4753b3a1196f2dba42b4"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "Embedding.mlmodelc/analytics/coremldata.bin",
        243,
        "8d6706436639b53830b4dbe8aaf9c9a843f7f582d63e16f3cb8bb7c6ccd58682"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "Embedding.mlmodelc/coremldata.bin",
        704,
        "4a705bac27d151d9642f37609296042a15602a42253039e0921dc9e75da7e004"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "Embedding.mlmodelc/metadata.json",
        2818,
        "1854371eb6b438fb8aeac96afb45c999af7902581c06afdfcd7ff3cb1ce66be5"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "Embedding.mlmodelc/model.mil",
        78432,
        "22fa958aef72a561c21f874a07cbdcd30fdf40ee961c0bc2fb67c119273b46d3"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "Embedding.mlmodelc/weights/weight.bin",
        13412288,
        "99356b2985b8d43880a657024d941d450b38820451ccff903f76ed4e52d1868b"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "FBank.mlmodelc/analytics/coremldata.bin",
        243,
        "0e8bd3a8b82ac123580989f490e4d9245127c535857630b543311268accc3f0a"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "FBank.mlmodelc/coremldata.bin",
        853,
        "57ac436bb0671cbb5527a339134d695f752eb77f7a18966b93c6835335595759"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "FBank.mlmodelc/metadata.json",
        3409,
        "2623785f5d186893b82d01e84aa33a7704ef763c3309e02055f22dc9d871ce9a"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "FBank.mlmodelc/model.mil",
        15667,
        "27aaeb21569e81bdbe2eef87789f50a37cfea800039bd134448a9417de2f30ed"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "FBank.mlmodelc/weights/weight.bin",
        1776896,
        "9e83fdd3ea78064b078069e4d9141603c61c47a27fd19e7e3142ff7476f8db36"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "PldaRho.mlmodelc/analytics/coremldata.bin",
        243,
        "8940ea6044dbcbefa22da8cc41e0b485e1fb5ed89aecaf37c6e0c483a97ddcd7"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "PldaRho.mlmodelc/coremldata.bin",
        763,
        "4d9741477f721c79b09fcdfe455110c4b7d4272e2de3496bf1729d966d3ee418"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "PldaRho.mlmodelc/metadata.json",
        2749,
        "b314cf25a93e46b4076883a6f5a2f8848b73c3851bd9d36074d067f35a1c7945"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "PldaRho.mlmodelc/model.mil",
        7613,
        "83aee2e5310d19b5f202aea97d07a0e12102556d1b32ef3ed08b36f7f9725041"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "PldaRho.mlmodelc/weights/weight.bin",
        200192,
        "80f7d229202636d372428c90596f11a91545f07da77259f07153aaf225914a36"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "Segmentation.mlmodelc/analytics/coremldata.bin",
        243,
        "64265f8e7ad41a5f68d630c15288c2499cca5892ad49e20096819cdeac004cdb"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "Segmentation.mlmodelc/coremldata.bin",
        812,
        "ea51481b8bd3e496ad3cf16f066ddaa37f20e8772eaac76b3393c28de20e06bc"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "Segmentation.mlmodelc/metadata.json",
        3410,
        "88dbf0b07208fe142e1729c2b4c974ad3599fcb2ae5d5f18fce782b225384124"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "Segmentation.mlmodelc/model.mil",
        43063,
        "d37e4ce30b406a6b34f765f769b9baed3178cc0c2b2e299c641daa43a052dd3f"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "Segmentation.mlmodelc/weights/weight.bin",
        5959360,
        "c3189a64946c75bc24fcb98afe89ad78c52bdbadfdf65e857fb1b81e2cc9fbb2"
    ),
    model_file!(
        DIARIZATION_REPO,
        DIARIZATION_REVISION,
        "speaker-diarization-coreml",
        "plda-parameters.json",
        89416,
        "38ee28d4269c076cef254ee760bbd811f0738a92e0f01f9699ad372828c5de8f"
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    fn fixture_file(repo_dir: &'static str, path: &'static str, bytes: &'static [u8]) -> ModelFile {
        ModelFile {
            repo: "fixture/repo",
            revision: "0123456789012345678901234567890123456789",
            repo_dir,
            path,
            bytes: bytes.len() as u64,
            sha256: Box::leak(format!("{:x}", Sha256::digest(bytes)).into_boxed_str()),
        }
    }

    #[test]
    fn production_manifest_is_immutable_complete_and_license_declared() {
        assert_eq!(VAD_REVISION.len(), 40);
        assert_eq!(DIARIZATION_REVISION.len(), 40);
        #[cfg(not(target_os = "linux"))]
        assert!(MODEL_FILES.len() >= 20);
        #[cfg(target_os = "linux")]
        {
            assert_eq!(SPEAKER_EMBEDDING_REVISION.len(), 40);
            assert_eq!(MODEL_FILES.len(), 3);
            assert!(MODEL_FILES.iter().all(|file| file.path.ends_with(".onnx")));
        }
        assert!(MODEL_FILES
            .iter()
            .all(|file| file.sha256.len() == 64 && file.bytes > 0));
        let store = ProductionSpeechModelStore::new(PathBuf::from("/tmp/models"));
        let status = store.status();
        assert!(!status.ready);
        assert_eq!(status.licenses, ["MIT", "CC-BY-4.0"]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_model_urls_are_immutable_upstream_revisions() {
        let urls = MODEL_FILES.iter().map(model_file_url).collect::<Vec<_>>();
        assert_eq!(
            urls,
            [
                "https://raw.githubusercontent.com/snakers4/silero-vad/fba061dc5559f696e62171e9a0741782b0fdc23c/src/silero_vad/data/silero_vad.onnx",
                "https://huggingface.co/csukuangfj/sherpa-onnx-pyannote-segmentation-3-0/resolve/9403a6902bb58e3d5ae8c7e77c3422de279db2e0/model.onnx",
                "https://huggingface.co/csukuangfj/speaker-embedding-models/resolve/0743f301363dec56491a490f6d6cbc9d67f9a3bf/wespeaker_en_voxceleb_resnet34_LM.onnx",
            ]
        );
        assert_eq!(SPEECH_ANALYSIS_RUNTIME_ID, "sherpa_onnx_speech_analysis");
        assert!(attribution_text().contains("CC BY 4.0"));
    }

    #[test]
    fn download_reports_monotonic_file_and_byte_progress() {
        let directory = tempdir().expect("temporary model root");
        let store = ProductionSpeechModelStore::new(directory.path().to_path_buf());
        let files = [
            fixture_file("one", "one.bin", b"one"),
            fixture_file("two", "two.bin", b"second"),
        ];
        let snapshots = Arc::new(Mutex::new(Vec::new()));
        let observed = snapshots.clone();

        store
            .download_files_with(
                &files,
                |file, destination| {
                    let bytes = if file.path == "one.bin" {
                        b"one".as_slice()
                    } else {
                        b"second".as_slice()
                    };
                    fs::write(destination, bytes)
                        .map_err(|error| SpeechModelError::Download(error.to_string()))
                },
                move |snapshot| {
                    observed.lock().expect("progress lock").push(snapshot);
                    Ok(())
                },
            )
            .expect("fixture download");

        let snapshots = snapshots.lock().expect("progress lock");
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].downloaded_files, 1);
        assert_eq!(snapshots[0].downloaded_bytes, 3);
        assert_eq!(snapshots[1].downloaded_files, 2);
        assert_eq!(snapshots[1].downloaded_bytes, 9);
        assert_eq!(snapshots[1].total_files, 2);
        assert_eq!(snapshots[1].total_bytes, 9);
    }

    #[test]
    fn failed_transfer_removes_partial_without_publishing_a_model() {
        let directory = tempdir().expect("temporary model root");
        let store = ProductionSpeechModelStore::new(directory.path().to_path_buf());
        let file = fixture_file("fixture", "model.bin", b"verified");
        let destination = store.file_path(&file);
        let error = store
            .download_files_with(
                &[file],
                |_file, partial| {
                    fs::write(partial, b"incomplete").unwrap();
                    Err(SpeechModelError::Download("fixture failure".into()))
                },
                |_| Ok(()),
            )
            .unwrap_err();
        assert_eq!(error.stable_code(), "speechModels.download.failed");
        assert!(!destination.exists());
        assert!(!destination.with_extension("bin.partial").exists());
    }

    #[test]
    fn cancellation_before_fetch_leaves_existing_files_untouched() {
        let directory = tempdir().expect("temporary model root");
        let store = ProductionSpeechModelStore::new(directory.path().to_path_buf());
        let file = fixture_file("fixture", "model.bin", b"verified");
        let unrelated = directory.path().join("keep.bin");
        fs::write(&unrelated, b"keep").unwrap();
        let result = store.download_files_cancellable_with(
            &[file],
            |_, _| panic!("cancelled download must never fetch"),
            |_| Ok(()),
            || Err(SpeechModelError::Cancelled),
        );
        assert!(matches!(result, Err(SpeechModelError::Cancelled)));
        assert!(!store.root().exists());
        assert_eq!(fs::read(unrelated).unwrap(), b"keep");
    }

    #[test]
    fn cancellation_after_transfer_does_not_publish_partial_model() {
        let directory = tempdir().unwrap();
        let store = ProductionSpeechModelStore::new(directory.path().to_path_buf());
        let file = fixture_file("fixture", "model.bin", b"verified");
        let cancelled = std::cell::Cell::new(false);
        let result = store.download_files_cancellable_with(
            &[file],
            |_, partial| {
                fs::write(partial, b"verified").unwrap();
                cancelled.set(true);
                Ok(())
            },
            |_| Ok(()),
            || {
                if cancelled.get() {
                    Err(SpeechModelError::Cancelled)
                } else {
                    Ok(())
                }
            },
        );
        assert!(matches!(result, Err(SpeechModelError::Cancelled)));
        let destination = store.file_path(&file);
        assert!(!destination.exists());
        assert!(!destination.with_extension("bin.partial").exists());
    }

    #[test]
    fn streamed_copy_stops_on_cancellation_and_rejects_oversized_response() {
        let bytes = vec![1; 128 * 1024];
        let checks = std::cell::Cell::new(0);
        let mut output = Vec::new();
        let result = copy_model_download(
            &mut bytes.as_slice(),
            &mut output,
            bytes.len() as u64,
            || {
                checks.set(checks.get() + 1);
                if checks.get() > 2 {
                    Err(SpeechModelError::Cancelled)
                } else {
                    Ok(())
                }
            },
        );
        assert!(matches!(result, Err(SpeechModelError::Cancelled)));
        assert_eq!(output.len(), 64 * 1024);
        let mut output = Vec::new();
        assert!(matches!(
            copy_model_download(&mut b"too large".as_slice(), &mut output, 3, || Ok(())),
            Err(SpeechModelError::Verification(_))
        ));
        assert!(output.is_empty());
    }

    #[test]
    fn retry_replaces_an_invalid_partial_and_verifies_the_result() {
        let directory = tempdir().expect("temporary model root");
        let store = ProductionSpeechModelStore::new(directory.path().to_path_buf());
        let file = fixture_file("fixture", "model.bin", b"verified");
        let destination = store.file_path(&file);
        fs::create_dir_all(destination.parent().expect("destination parent"))
            .expect("create fixture directory");
        fs::write(&destination, b"corrupt").expect("write corrupt fixture");

        store
            .download_files_with(
                &[file],
                |_file, partial| {
                    fs::write(partial, b"verified")
                        .map_err(|error| SpeechModelError::Download(error.to_string()))
                },
                |_| Ok(()),
            )
            .expect("retry download");

        verify_file(&destination, &file).expect("verified replacement");
    }

    #[test]
    fn verification_and_removal_are_scoped_to_the_speech_model_root() {
        let directory = tempdir().expect("temporary model root");
        let unrelated = directory.path().join("transcription/keep.bin");
        fs::create_dir_all(unrelated.parent().expect("unrelated parent"))
            .expect("create unrelated directory");
        fs::write(&unrelated, b"keep").expect("write unrelated file");
        let store = ProductionSpeechModelStore::new(directory.path().to_path_buf());
        fs::create_dir_all(store.root()).expect("create speech root");
        fs::write(store.root().join("invalid.bin"), b"invalid").expect("write speech file");

        assert_eq!(
            store
                .verify()
                .expect_err("incomplete set must fail")
                .stable_code(),
            "speechModels.verification.failed"
        );
        let status = store.remove().expect("remove speech models");

        assert!(!status.ready);
        assert!(!store.root().exists());
        assert_eq!(
            fs::read(unrelated).expect("unrelated file retained"),
            b"keep"
        );
    }

    #[test]
    fn observer_failure_aborts_before_the_next_file_is_published() {
        let directory = tempdir().expect("temporary model root");
        let store = ProductionSpeechModelStore::new(directory.path().to_path_buf());
        let files = [
            fixture_file("one", "one.bin", b"one"),
            fixture_file("two", "two.bin", b"two"),
        ];

        let error = store
            .download_files_with(
                &files,
                |file, partial| {
                    let bytes = if file.path == "one.bin" {
                        b"one".as_slice()
                    } else {
                        b"two".as_slice()
                    };
                    fs::write(partial, bytes)
                        .map_err(|error| SpeechModelError::Download(error.to_string()))
                },
                |_| {
                    Err(SpeechModelError::ProgressPersistence(
                        "journal failed".to_string(),
                    ))
                },
            )
            .expect_err("observer failure");

        assert_eq!(
            error.stable_code(),
            "speechModels.progressPersistenceFailed"
        );
        assert!(store.file_path(&files[0]).is_file());
        assert!(!store.file_path(&files[1]).exists());
    }

    #[test]
    fn speech_readiness_is_independent_and_exposes_pinned_provenance() {
        let directory = tempdir().expect("temporary model root");
        fs::create_dir_all(directory.path().join("nvidia__parakeet-tdt-0.6b-v3"))
            .expect("create unrelated transcription model");
        let store = ProductionSpeechModelStore::new(directory.path().to_path_buf());
        let status = store.status();

        assert!(!status.ready);
        assert_eq!(status.runtime_id, SPEECH_ANALYSIS_RUNTIME_ID);
        assert_eq!(status.vad_repo, VAD_REPO);
        assert_eq!(status.vad_revision, VAD_REVISION);
        assert_eq!(status.diarization_repo, DIARIZATION_REPO);
        assert_eq!(status.diarization_revision, DIARIZATION_REVISION);
        assert_eq!(status.artifact_format, ARTIFACT_FORMAT);
        assert_eq!(status.licenses, ["MIT", "CC-BY-4.0"]);
    }

    #[test]
    fn failed_verification_persists_stable_status_for_refresh_and_retry() {
        let directory = tempdir().expect("temporary model root");
        let store = ProductionSpeechModelStore::new(directory.path().to_path_buf());

        let error = store.verify().expect_err("missing models must fail");
        let failed = store.status();

        assert_eq!(failed.last_error_code.as_deref(), Some(error.stable_code()));
        assert!(failed
            .last_error_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("verification")));

        store
            .clear_last_error()
            .expect("clear persisted failure before retry");
        let retry = store.status();
        assert_eq!(retry.last_error_code, None);
        assert_eq!(retry.last_error_detail, None);
    }

    #[test]
    fn removal_clears_persisted_failure_with_the_scoped_root() {
        let directory = tempdir().expect("temporary model root");
        let store = ProductionSpeechModelStore::new(directory.path().to_path_buf());
        store.verify().expect_err("missing models must fail");
        assert!(store.status().last_error_code.is_some());

        let removed = store.remove().expect("remove failed speech setup");

        assert_eq!(removed.last_error_code, None);
        assert_eq!(removed.last_error_detail, None);
        assert!(!store.root().exists());
    }
}
