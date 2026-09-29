use super::semantic_visual::{
    load_semantic_visual_index_input, query_semantic_visual_indexes, write_semantic_visual_index,
    SemanticVisualFrameRow, SemanticVisualIndexExpectation, SemanticVisualIndexInput,
    SemanticVisualModelSpec, SemanticVisualQueryResult,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use thiserror::Error;
use zip::ZipArchive;

pub const SEMANTIC_ENCODER_MANIFEST_SCHEMA_VERSION: u32 = 1;
pub const SEMANTIC_ENCODER_MANIFEST_ENV: &str = "VIDEO_CREATER_SEMANTIC_ENCODER_MANIFEST";
pub const SEMANTIC_ENCODER_STORE_ENV: &str = "VIDEO_CREATER_MODEL_STORE";
pub const SEMANTIC_ENCODER_BINARY_ENV: &str = "VIDEO_CREATER_SEMANTIC_ENCODER";
pub const PALMIER_SIGLIP2_REPOSITORY: &str = "palmier-io/siglip2-base-coreml";
pub const PALMIER_SIGLIP2_REVISION: &str = "753b68cedb8061dde4ca3d5fb90dff1e382642a0";
pub const PALMIER_SIGLIP2_LICENSE: &str = "Apache-2.0";
pub const PALMIER_SIGLIP2_MODEL_ID: &str = "siglip2-base-patch16-256";
/// ONNX export of google/siglip2-base-patch16-256 (Apache-2.0) used by the Linux helper.
pub const ONNX_SIGLIP2_REPOSITORY: &str = "onnx-community/siglip2-base-patch16-256-ONNX";
pub const ONNX_SIGLIP2_REVISION: &str = "d1114256522a37ffa257a0a58017348ab0058db2";
pub const ONNX_SIGLIP2_LICENSE: &str = "Apache-2.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticEncoderManifest {
    pub schema_version: u32,
    pub model: SemanticVisualModelSpec,
    pub file_name: String,
    pub sha256: String,
    pub license_spdx: String,
    pub license_reviewed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticEncoderArtifact {
    pub role: String,
    pub file_name: String,
    pub sha256: String,
    pub bytes: u64,
    /// Repository-relative download path when it differs from `file_name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticEncoderBundleManifest {
    pub schema_version: u32,
    pub repository: String,
    pub revision: String,
    pub model: SemanticVisualModelSpec,
    pub image_size: usize,
    pub context_length: usize,
    pub license_spdx: String,
    pub license_reviewed: bool,
    pub source_url: String,
    pub artifacts: Vec<SemanticEncoderArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticEncoderStatus {
    pub status: String,
    pub model: Option<SemanticVisualModelSpec>,
    pub manifest_configured: bool,
    pub license_reviewed: bool,
    pub hash_verified: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticAcquisitionProgress {
    pub phase: String,
    pub completed_bytes: usize,
    pub total_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticIndexRunReport {
    pub media_id: String,
    pub encoded_shots: usize,
    pub reused_shots: usize,
    pub total_shots: usize,
    pub sidecar_path: String,
}

#[derive(Debug, Error)]
pub enum SemanticRuntimeError {
    #[error("semantic encoder is not installed: {0}")]
    NotInstalled(String),
    #[error("semantic encoder manifest is invalid: {0}")]
    Manifest(String),
    #[error("semantic encoder acquisition was cancelled")]
    Cancelled,
    #[error("semantic encoder IO failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("semantic encoder failed: {0}")]
    Encoder(String),
    #[error("semantic index failed: {0}")]
    Index(String),
}

pub trait LocalSemanticEncoder {
    fn model(&self) -> SemanticVisualModelSpec;
    fn encode_image(&self, bytes: &[u8]) -> Result<Vec<f32>, SemanticRuntimeError>;
    fn encode_text(&self, text: &str) -> Result<Vec<f32>, SemanticRuntimeError>;
}

pub fn palmier_siglip2_bundle_manifest() -> SemanticEncoderBundleManifest {
    SemanticEncoderBundleManifest {
        schema_version: 1,
        repository: PALMIER_SIGLIP2_REPOSITORY.to_string(),
        revision: PALMIER_SIGLIP2_REVISION.to_string(),
        model: SemanticVisualModelSpec {
            id: PALMIER_SIGLIP2_MODEL_ID.to_string(),
            version: "1-palettized-8bit".to_string(),
            dimensions: 768,
        },
        image_size: 256,
        context_length: 64,
        license_spdx: PALMIER_SIGLIP2_LICENSE.to_string(),
        license_reviewed: true,
        source_url: format!(
            "https://huggingface.co/{PALMIER_SIGLIP2_REPOSITORY}/resolve/{PALMIER_SIGLIP2_REVISION}"
        ),
        artifacts: vec![
            SemanticEncoderArtifact {
                role: "imageEncoder".to_string(),
                file_name: "ImageEncoder.mlpackage.zip".to_string(),
                sha256: "426115f240ead5faf69b073e08dd1b959d850ca5c592537cd81886992283b2fb"
                    .to_string(),
                bytes: 91_700_398,
                source_path: None,
            },
            SemanticEncoderArtifact {
                role: "textEncoder".to_string(),
                file_name: "TextEncoder.mlpackage.zip".to_string(),
                sha256: "48f80e35ce40a9dcdc55bef986a104d3153e1cfa78229bb45c4724f3f3427368"
                    .to_string(),
                bytes: 258_593_083,
                source_path: None,
            },
            SemanticEncoderArtifact {
                role: "tokenizer".to_string(),
                file_name: "tokenizer.zip".to_string(),
                sha256: "c37f2a8e8555d8561109564c4f60ee962b0072abddcfcfd599d321469d6d1ef5"
                    .to_string(),
                bytes: 5_460_173,
                source_path: None,
            },
        ],
    }
}

/// Pinned SigLIP 2 ONNX Runtime bundle for the Linux helper: fp16 vision tower (int8 dynamic
/// quantization of the vision tower measurably degraded image embeddings) and int8 text tower.
/// The model version differs from the Core ML bundle so semantic indexes built by one runtime
/// are never reused by the other.
pub fn onnx_siglip2_bundle_manifest() -> SemanticEncoderBundleManifest {
    let artifact =
        |role: &str, source_path: &str, sha256: &str, bytes: u64| SemanticEncoderArtifact {
            role: role.to_string(),
            file_name: source_path
                .rsplit('/')
                .next()
                .unwrap_or(source_path)
                .to_string(),
            sha256: sha256.to_string(),
            bytes,
            source_path: source_path.contains('/').then(|| source_path.to_string()),
        };
    SemanticEncoderBundleManifest {
        schema_version: 1,
        repository: ONNX_SIGLIP2_REPOSITORY.to_string(),
        revision: ONNX_SIGLIP2_REVISION.to_string(),
        model: SemanticVisualModelSpec {
            id: PALMIER_SIGLIP2_MODEL_ID.to_string(),
            version: "1-onnx-vision-fp16-text-int8".to_string(),
            dimensions: 768,
        },
        image_size: 256,
        context_length: 64,
        license_spdx: ONNX_SIGLIP2_LICENSE.to_string(),
        license_reviewed: true,
        source_url: format!(
            "https://huggingface.co/{ONNX_SIGLIP2_REPOSITORY}/resolve/{ONNX_SIGLIP2_REVISION}"
        ),
        artifacts: vec![
            artifact(
                "imageEncoder",
                "onnx/vision_model_fp16.onnx",
                "fe9ad8020a6d3d98d394c9be8f07064066135fc2f87ec11692de0b677c0ac4db",
                186_131_676,
            ),
            artifact(
                "textEncoder",
                "onnx/text_model_quantized.onnx",
                "6f59b39d880c413042314b79302b74d0dd93b273caf8fbfdb1eb2df61a7fefd4",
                283_438_275,
            ),
            artifact(
                "tokenizer",
                "tokenizer.json",
                "cb9140fae3ac5122c972d37adf83e1248471a38147ad76f8215c8872c6fd8322",
                34_363_039,
            ),
        ],
    }
}

/// The pinned SigLIP 2 bundle for the helper runtime of this platform.
pub fn siglip2_bundle_manifest() -> SemanticEncoderBundleManifest {
    #[cfg(target_os = "macos")]
    {
        palmier_siglip2_bundle_manifest()
    }
    #[cfg(not(target_os = "macos"))]
    {
        onnx_siglip2_bundle_manifest()
    }
}

fn reviewed_bundle_identity() -> (&'static str, &'static str, &'static str) {
    #[cfg(target_os = "macos")]
    {
        (
            PALMIER_SIGLIP2_REPOSITORY,
            PALMIER_SIGLIP2_LICENSE,
            "Pinned Apache-2.0 SigLIP 2 Core ML encoder is verified.",
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        (
            ONNX_SIGLIP2_REPOSITORY,
            ONNX_SIGLIP2_LICENSE,
            "Pinned Apache-2.0 SigLIP 2 ONNX encoder is verified.",
        )
    }
}

pub fn configured_semantic_encoder_status() -> SemanticEncoderStatus {
    let Some(manifest_path) = std::env::var_os(SEMANTIC_ENCODER_MANIFEST_ENV).map(PathBuf::from)
    else {
        let manifest = siglip2_bundle_manifest();
        let model_dir = semantic_bundle_install_dir(&manifest);
        if verify_semantic_encoder_bundle(&manifest, &model_dir).is_ok() {
            return SemanticEncoderStatus {
                status: "installed".to_string(),
                model: Some(manifest.model),
                manifest_configured: true,
                license_reviewed: true,
                hash_verified: true,
                message: reviewed_bundle_identity().2.to_string(),
            };
        }
        return SemanticEncoderStatus {
            status: "notInstalled".to_string(),
            model: Some(manifest.model),
            manifest_configured: true,
            license_reviewed: true,
            hash_verified: false,
            message: "Pinned Apache-2.0 SigLIP 2 encoder is available but not installed."
                .to_string(),
        };
    };
    match load_manifest(&manifest_path).and_then(|manifest| {
        let model_path = semantic_model_store_root()
            .join(&manifest.model.id)
            .join(&manifest.file_name);
        verify_installed_model(&manifest, &model_path)?;
        Ok(manifest)
    }) {
        Ok(manifest) => SemanticEncoderStatus {
            status: "installed".to_string(),
            model: Some(manifest.model),
            manifest_configured: true,
            license_reviewed: true,
            hash_verified: true,
            message: "License-reviewed semantic encoder artifact is verified.".to_string(),
        },
        Err(error) => SemanticEncoderStatus {
            status: "notInstalled".to_string(),
            model: None,
            manifest_configured: true,
            license_reviewed: false,
            hash_verified: false,
            message: error.to_string(),
        },
    }
}

pub fn acquire_semantic_encoder_with(
    manifest: &SemanticEncoderManifest,
    fetch: impl FnOnce(&str) -> Result<Vec<u8>, SemanticRuntimeError>,
    is_cancelled: impl Fn() -> bool,
    mut progress: impl FnMut(SemanticAcquisitionProgress),
) -> Result<PathBuf, SemanticRuntimeError> {
    validate_manifest(manifest)?;
    let source_url = manifest.source_url.as_deref().ok_or_else(|| {
        SemanticRuntimeError::Manifest("sourceUrl is required for acquisition".to_string())
    })?;
    if is_cancelled() {
        return Err(SemanticRuntimeError::Cancelled);
    }
    progress(SemanticAcquisitionProgress {
        phase: "fetching".to_string(),
        completed_bytes: 0,
        total_bytes: 0,
    });
    let bytes = fetch(source_url)?;
    let total = bytes.len();
    let model_dir = semantic_model_store_root().join(&manifest.model.id);
    fs::create_dir_all(&model_dir)?;
    let destination = model_dir.join(&manifest.file_name);
    let temporary = model_dir.join(format!(".model-{}.tmp", uuid::Uuid::new_v4()));
    let mut written = 0;
    let mut output = Vec::with_capacity(total);
    for chunk in bytes.chunks(64 * 1024) {
        if is_cancelled() {
            let _ = fs::remove_file(&temporary);
            return Err(SemanticRuntimeError::Cancelled);
        }
        output.extend_from_slice(chunk);
        written += chunk.len();
        progress(SemanticAcquisitionProgress {
            phase: "writing".to_string(),
            completed_bytes: written,
            total_bytes: total,
        });
    }
    fs::write(&temporary, &output)?;
    if sha256_bytes(&output) != manifest.sha256 {
        let _ = fs::remove_file(&temporary);
        return Err(SemanticRuntimeError::Manifest(
            "downloaded model hash does not match manifest".to_string(),
        ));
    }
    fs::rename(&temporary, &destination)?;
    fs::write(
        model_dir.join("manifest.json"),
        serde_json::to_vec_pretty(manifest)
            .map_err(|error| SemanticRuntimeError::Manifest(error.to_string()))?,
    )?;
    progress(SemanticAcquisitionProgress {
        phase: "completed".to_string(),
        completed_bytes: total,
        total_bytes: total,
    });
    Ok(destination)
}

pub fn index_semantic_shots_incrementally(
    project_dir: &Path,
    expectation: SemanticVisualIndexExpectation,
    frames: Vec<SemanticVisualFrameRow>,
    encoder: &dyn LocalSemanticEncoder,
    is_cancelled: impl Fn() -> bool,
    mut progress: impl FnMut(usize, usize),
) -> Result<SemanticIndexRunReport, SemanticRuntimeError> {
    if encoder.model() != expectation.model {
        return Err(SemanticRuntimeError::Encoder(
            "encoder model does not match index expectation".to_string(),
        ));
    }
    let existing = load_semantic_visual_index_input(project_dir, &expectation).ok();
    let reusable = existing
        .map(|index| {
            index
                .frames
                .into_iter()
                .zip(index.vectors)
                .map(|(frame, vector)| (frame_key(&frame), vector))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let total = frames.len();
    let mut vectors = Vec::with_capacity(total);
    let mut encoded = 0;
    let mut reused = 0;
    for (index, frame) in frames.iter().enumerate() {
        if is_cancelled() {
            return Err(SemanticRuntimeError::Cancelled);
        }
        if let Some(vector) = reusable.get(&frame_key(frame)) {
            vectors.push(vector.clone());
            reused += 1;
        } else {
            let bytes = fs::read(project_dir.join(&frame.thumbnail_relative_path))?;
            vectors.push(encoder.encode_image(&bytes)?);
            encoded += 1;
        }
        progress(index + 1, total);
    }
    let input = SemanticVisualIndexInput {
        media_id: expectation.media_id.clone(),
        source_fingerprint: expectation.source_fingerprint,
        sampling_policy: expectation.sampling_policy,
        model: expectation.model,
        frames,
        vectors,
    };
    let path = write_semantic_visual_index(project_dir, &input)
        .map_err(|error| SemanticRuntimeError::Index(error.to_string()))?;
    Ok(SemanticIndexRunReport {
        media_id: input.media_id,
        encoded_shots: encoded,
        reused_shots: reused,
        total_shots: total,
        sidecar_path: path.display().to_string(),
    })
}

pub fn query_semantic_visual_with_text(
    project_dir: &Path,
    expectations: &[SemanticVisualIndexExpectation],
    query: &str,
    encoder: &dyn LocalSemanticEncoder,
    limit: usize,
) -> Result<SemanticVisualQueryResult, SemanticRuntimeError> {
    if query.trim().is_empty() {
        return Err(SemanticRuntimeError::Encoder(
            "semantic query must not be blank".to_string(),
        ));
    }
    let vector = encoder.encode_text(query)?;
    Ok(query_semantic_visual_indexes(
        project_dir,
        expectations,
        &vector,
        limit,
        None,
    ))
}

pub fn install_palmier_siglip2_with(
    mut fetch: impl FnMut(&str) -> Result<Vec<u8>, SemanticRuntimeError>,
    is_cancelled: impl Fn() -> bool,
    mut progress: impl FnMut(SemanticAcquisitionProgress),
) -> Result<PathBuf, SemanticRuntimeError> {
    let manifest = siglip2_bundle_manifest();
    validate_bundle_manifest(&manifest)?;
    let destination = semantic_bundle_install_dir(&manifest);
    if verify_semantic_encoder_bundle(&manifest, &destination).is_ok() {
        let total_bytes = manifest
            .artifacts
            .iter()
            .map(|artifact| artifact.bytes)
            .sum::<u64>();
        progress(SemanticAcquisitionProgress {
            phase: "verifiedExisting".to_string(),
            completed_bytes: usize::try_from(total_bytes).unwrap_or(usize::MAX),
            total_bytes: usize::try_from(total_bytes).unwrap_or(usize::MAX),
        });
        return Ok(destination);
    }
    if is_cancelled() {
        return Err(SemanticRuntimeError::Cancelled);
    }

    let parent = destination.parent().ok_or_else(|| {
        SemanticRuntimeError::Manifest("semantic model destination has no parent".to_string())
    })?;
    fs::create_dir_all(parent)?;
    let staging = parent.join(format!(".siglip2-install-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&staging)?;
    let result = (|| {
        let total_bytes: u64 = manifest
            .artifacts
            .iter()
            .map(|artifact| artifact.bytes)
            .sum();
        let mut completed_bytes = 0_u64;
        for artifact in &manifest.artifacts {
            if is_cancelled() {
                return Err(SemanticRuntimeError::Cancelled);
            }
            progress(SemanticAcquisitionProgress {
                phase: format!("fetching:{}", artifact.role),
                completed_bytes: usize::try_from(completed_bytes).unwrap_or(usize::MAX),
                total_bytes: usize::try_from(total_bytes).unwrap_or(usize::MAX),
            });
            let url = format!(
                "{}/{}",
                manifest.source_url,
                artifact
                    .source_path
                    .as_deref()
                    .unwrap_or(&artifact.file_name)
            );
            let bytes = fetch(&url)?;
            if bytes.len() as u64 != artifact.bytes || sha256_bytes(&bytes) != artifact.sha256 {
                return Err(SemanticRuntimeError::Manifest(format!(
                    "{} size or hash does not match the pinned manifest",
                    artifact.file_name
                )));
            }
            fs::write(staging.join(&artifact.file_name), &bytes)?;
            if artifact.file_name.ends_with(".zip") {
                extract_zip_safely(&bytes, &staging)?;
            }
            completed_bytes += artifact.bytes;
            progress(SemanticAcquisitionProgress {
                phase: format!("verified:{}", artifact.role),
                completed_bytes: usize::try_from(completed_bytes).unwrap_or(usize::MAX),
                total_bytes: usize::try_from(total_bytes).unwrap_or(usize::MAX),
            });
        }
        fs::write(
            staging.join("spec.json"),
            serde_json::to_vec_pretty(&semantic_bundle_spec(&manifest))
                .map_err(|error| SemanticRuntimeError::Manifest(error.to_string()))?,
        )?;
        fs::write(
            staging.join("bundle-manifest.json"),
            serde_json::to_vec_pretty(&manifest)
                .map_err(|error| SemanticRuntimeError::Manifest(error.to_string()))?,
        )?;
        progress(SemanticAcquisitionProgress {
            phase: "compiling".to_string(),
            completed_bytes: usize::try_from(total_bytes).unwrap_or(usize::MAX),
            total_bytes: usize::try_from(total_bytes).unwrap_or(usize::MAX),
        });
        prepare_semantic_encoder_bundle(&staging)?;
        verify_semantic_encoder_bundle(&manifest, &staging)?;
        atomic_replace_directory(&staging, &destination)?;
        progress(SemanticAcquisitionProgress {
            phase: "completed".to_string(),
            completed_bytes: usize::try_from(total_bytes).unwrap_or(usize::MAX),
            total_bytes: usize::try_from(total_bytes).unwrap_or(usize::MAX),
        });
        Ok(destination.clone())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

#[cfg(target_os = "macos")]
fn semantic_bundle_spec(manifest: &SemanticEncoderBundleManifest) -> serde_json::Value {
    serde_json::json!({
        "model": PALMIER_SIGLIP2_MODEL_ID,
        "version": 1,
        "embeddingDim": manifest.model.dimensions,
        "imageSize": manifest.image_size,
        "contextLength": manifest.context_length,
    })
}

/// The ONNX helper reads its model file names from spec.json.
#[cfg(not(target_os = "macos"))]
fn semantic_bundle_spec(manifest: &SemanticEncoderBundleManifest) -> serde_json::Value {
    let file_for = |role: &str| {
        manifest
            .artifacts
            .iter()
            .find(|artifact| artifact.role == role)
            .map(|artifact| artifact.file_name.clone())
    };
    serde_json::json!({
        "model": PALMIER_SIGLIP2_MODEL_ID,
        "version": 1,
        "embeddingDim": manifest.model.dimensions,
        "imageSize": manifest.image_size,
        "contextLength": manifest.context_length,
        "imageEncoder": file_for("imageEncoder"),
        "textEncoder": file_for("textEncoder"),
        "tokenizer": file_for("tokenizer"),
    })
}

/// Client for the platform SigLIP 2 helper process (Core ML on macOS, ONNX Runtime on Linux).
pub struct PalmierSiglip2Encoder {
    manifest: SemanticEncoderBundleManifest,
    process: Mutex<SemanticEncoderProcess>,
    next_request_id: AtomicU64,
}

impl PalmierSiglip2Encoder {
    pub fn installed() -> Result<Self, SemanticRuntimeError> {
        let manifest = siglip2_bundle_manifest();
        let model_dir = semantic_bundle_install_dir(&manifest);
        verify_semantic_encoder_bundle(&manifest, &model_dir)?;
        Ok(Self {
            manifest,
            process: Mutex::new(SemanticEncoderProcess::spawn(&model_dir)?),
            next_request_id: AtomicU64::new(1),
        })
    }

    fn request(&self, request_type: &str, value: &str) -> Result<Vec<f32>, SemanticRuntimeError> {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let request = match request_type {
            "text" => serde_json::json!({"id": request_id, "type": "text", "text": value}),
            "image" => {
                serde_json::json!({"id": request_id, "type": "image", "imageBase64": value})
            }
            _ => {
                return Err(SemanticRuntimeError::Encoder(
                    "unsupported semantic encoder request".to_string(),
                ))
            }
        };
        let mut process = self.process.lock().map_err(|_| {
            SemanticRuntimeError::Encoder("encoder process lock failed".to_string())
        })?;
        process.request(request_id, &request)
    }
}

impl LocalSemanticEncoder for PalmierSiglip2Encoder {
    fn model(&self) -> SemanticVisualModelSpec {
        self.manifest.model.clone()
    }

    fn encode_image(&self, bytes: &[u8]) -> Result<Vec<f32>, SemanticRuntimeError> {
        if bytes.is_empty() {
            return Err(SemanticRuntimeError::Encoder(
                "image bytes must not be empty".to_string(),
            ));
        }
        self.request(
            "image",
            &base64::engine::general_purpose::STANDARD.encode(bytes),
        )
    }

    fn encode_text(&self, text: &str) -> Result<Vec<f32>, SemanticRuntimeError> {
        if text.trim().is_empty() {
            return Err(SemanticRuntimeError::Encoder(
                "semantic query must not be blank".to_string(),
            ));
        }
        self.request("text", text)
    }
}

struct SemanticEncoderProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl SemanticEncoderProcess {
    fn spawn(model_dir: &Path) -> Result<Self, SemanticRuntimeError> {
        let mut child = Command::new(semantic_encoder_binary_path()?)
            .arg("serve")
            .arg(format!("--model-dir={}", model_dir.display()))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| SemanticRuntimeError::Encoder(error.to_string()))?;
        let stdin = child.stdin.take().ok_or_else(|| {
            SemanticRuntimeError::Encoder("encoder process stdin is unavailable".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            SemanticRuntimeError::Encoder("encoder process stdout is unavailable".to_string())
        })?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    fn request(
        &mut self,
        request_id: u64,
        request: &serde_json::Value,
    ) -> Result<Vec<f32>, SemanticRuntimeError> {
        serde_json::to_writer(&mut self.stdin, request)
            .map_err(|error| SemanticRuntimeError::Encoder(error.to_string()))?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        let mut response_line = String::new();
        if self.stdout.read_line(&mut response_line)? == 0 {
            return Err(SemanticRuntimeError::Encoder(format!(
                "encoder process exited before responding: {:?}",
                self.child.try_wait().ok().flatten()
            )));
        }
        let response: serde_json::Value = serde_json::from_str(&response_line)
            .map_err(|error| SemanticRuntimeError::Encoder(error.to_string()))?;
        if let Some(error) = response.get("error").and_then(serde_json::Value::as_str) {
            return Err(SemanticRuntimeError::Encoder(error.to_string()));
        }
        if response.get("id").and_then(serde_json::Value::as_u64) != Some(request_id) {
            return Err(SemanticRuntimeError::Encoder(
                "encoder response id does not match request".to_string(),
            ));
        }
        let embedding = response
            .get("embedding")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| SemanticRuntimeError::Encoder("embedding is missing".to_string()))?
            .iter()
            .map(|value| {
                value
                    .as_f64()
                    .map(|value| value as f32)
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| {
                        SemanticRuntimeError::Encoder(
                            "embedding contains a non-finite value".to_string(),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if embedding.len() != 768 {
            return Err(SemanticRuntimeError::Encoder(format!(
                "expected 768 embedding dimensions, received {}",
                embedding.len()
            )));
        }
        Ok(embedding)
    }
}

impl Drop for SemanticEncoderProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn validate_bundle_manifest(
    manifest: &SemanticEncoderBundleManifest,
) -> Result<(), SemanticRuntimeError> {
    let (repository, license, _) = reviewed_bundle_identity();
    if manifest.schema_version != 1
        || manifest.repository != repository
        || manifest.revision.len() != 40
        || manifest.model.dimensions != 768
        || manifest.image_size != 256
        || manifest.context_length != 64
        || manifest.license_spdx != license
        || !manifest.license_reviewed
        || manifest.artifacts.len() != 3
        || manifest.artifacts.iter().any(|artifact| {
            artifact.file_name.contains('/')
                || artifact.file_name.contains("..")
                || artifact.sha256.len() != 64
                || artifact.bytes == 0
                || artifact.source_path.as_deref().is_some_and(|path| {
                    path.contains("..")
                        || path.starts_with('/')
                        || !path.ends_with(&artifact.file_name)
                })
        })
    {
        return Err(SemanticRuntimeError::Manifest(
            "SigLIP 2 bundle manifest does not match the reviewed production contract".to_string(),
        ));
    }
    Ok(())
}

fn verify_semantic_encoder_bundle(
    manifest: &SemanticEncoderBundleManifest,
    model_dir: &Path,
) -> Result<(), SemanticRuntimeError> {
    validate_bundle_manifest(manifest)?;
    let installed: SemanticEncoderBundleManifest =
        serde_json::from_slice(&fs::read(model_dir.join("bundle-manifest.json"))?)
            .map_err(|error| SemanticRuntimeError::Manifest(error.to_string()))?;
    if &installed != manifest {
        return Err(SemanticRuntimeError::Manifest(
            "installed bundle manifest does not match the pinned catalog".to_string(),
        ));
    }
    for artifact in &manifest.artifacts {
        let bytes = fs::read(model_dir.join(&artifact.file_name))?;
        if bytes.len() as u64 != artifact.bytes || sha256_bytes(&bytes) != artifact.sha256 {
            return Err(SemanticRuntimeError::Manifest(format!(
                "installed {} does not match its size/hash",
                artifact.file_name
            )));
        }
    }
    #[cfg(target_os = "macos")]
    let required_files = [
        "ImageEncoder.mlmodelc/model.mil",
        "TextEncoder.mlmodelc/model.mil",
        "tokenizer/tokenizer.json",
        "spec.json",
    ];
    #[cfg(not(target_os = "macos"))]
    let required_files = ["spec.json"];
    for required in required_files {
        if !model_dir.join(required).is_file() {
            return Err(SemanticRuntimeError::Manifest(format!(
                "installed semantic encoder is missing {required}"
            )));
        }
    }
    Ok(())
}

fn semantic_bundle_install_dir(manifest: &SemanticEncoderBundleManifest) -> PathBuf {
    semantic_model_store_root().join(&manifest.model.id)
}

fn prepare_semantic_encoder_bundle(model_dir: &Path) -> Result<(), SemanticRuntimeError> {
    let status = Command::new(semantic_encoder_binary_path()?)
        .arg("prepare")
        .arg(format!("--model-dir={}", model_dir.display()))
        .status()
        .map_err(|error| SemanticRuntimeError::Encoder(error.to_string()))?;
    if !status.success() {
        return Err(SemanticRuntimeError::Encoder(format!(
            "semantic encoder preparation exited with {status}"
        )));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
const SEMANTIC_ENCODER_EXECUTABLE: &str = "video-creater-semantic-encoder";

#[cfg(not(target_os = "macos"))]
fn semantic_encoder_binary_path() -> Result<PathBuf, SemanticRuntimeError> {
    if let Some(path) = std::env::var_os(SEMANTIC_ENCODER_BINARY_ENV).map(PathBuf::from) {
        if path.is_file() {
            return Ok(path);
        }
    }
    let current = std::env::current_exe().ok();
    semantic_encoder_binary_candidates(current.as_deref(), cfg!(debug_assertions))
        .into_iter()
        .find(|candidate| {
            fs::metadata(candidate).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
        })
        .ok_or_else(|| {
            SemanticRuntimeError::NotInstalled(
                "bundled semantic encoder runtime was not found".to_string(),
            )
        })
}

/// The packaged sidecar next to the application executable, then (development builds) the
/// staged Tauri sidecar and Cargo target outputs. Empty placeholder sidecars are skipped by the
/// caller.
#[cfg(not(target_os = "macos"))]
fn semantic_encoder_binary_candidates(
    current_exe: Option<&Path>,
    development: bool,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(parent) = current_exe.and_then(Path::parent) {
        let parent = if parent.file_name().and_then(|name| name.to_str()) == Some("deps") {
            parent.parent().unwrap_or(parent)
        } else {
            parent
        };
        candidates.push(parent.join(SEMANTIC_ENCODER_EXECUTABLE));
    }
    if !development {
        return candidates;
    }
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let target = if cfg!(target_arch = "aarch64") {
        "aarch64-unknown-linux-gnu"
    } else {
        "x86_64-unknown-linux-gnu"
    };
    candidates.push(
        manifest_dir
            .join("binaries")
            .join(format!("{SEMANTIC_ENCODER_EXECUTABLE}-{target}")),
    );
    for profile in ["release", "debug"] {
        candidates.push(
            manifest_dir
                .join("target")
                .join(profile)
                .join(SEMANTIC_ENCODER_EXECUTABLE),
        );
    }
    candidates
}

#[cfg(target_os = "macos")]
fn semantic_encoder_binary_path() -> Result<PathBuf, SemanticRuntimeError> {
    if let Some(path) = std::env::var_os(SEMANTIC_ENCODER_BINARY_ENV).map(PathBuf::from) {
        if path.is_file() {
            return Ok(path);
        }
    }
    let target = if cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else {
        "x86_64-apple-darwin"
    };
    let development = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join(format!("video-creater-semantic-encoder-{target}"));
    if development.is_file() {
        return Ok(development);
    }
    if let Ok(current) = std::env::current_exe() {
        if let Some(parent) = current.parent() {
            let bundled = parent.join("video-creater-semantic-encoder");
            if bundled.is_file() {
                return Ok(bundled);
            }
        }
    }
    Err(SemanticRuntimeError::NotInstalled(
        "bundled semantic encoder runtime was not found".to_string(),
    ))
}

fn extract_zip_safely(bytes: &[u8], destination: &Path) -> Result<(), SemanticRuntimeError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| SemanticRuntimeError::Manifest(error.to_string()))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| SemanticRuntimeError::Manifest(error.to_string()))?;
        let relative = entry.enclosed_name().ok_or_else(|| {
            SemanticRuntimeError::Manifest("model archive contains an unsafe path".to_string())
        })?;
        let output = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&output)?;
            continue;
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(output)?;
        std::io::copy(&mut entry, &mut file)?;
    }
    Ok(())
}

fn atomic_replace_directory(
    staging: &Path,
    destination: &Path,
) -> Result<(), SemanticRuntimeError> {
    let backup = destination.with_file_name(format!(
        ".{}-backup-{}",
        destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("semantic-encoder"),
        uuid::Uuid::new_v4()
    ));
    let had_destination = destination.exists();
    if had_destination {
        fs::rename(destination, &backup)?;
    }
    if let Err(error) = fs::rename(staging, destination) {
        if had_destination {
            let _ = fs::rename(&backup, destination);
        }
        return Err(SemanticRuntimeError::Io(error));
    }
    if had_destination {
        let _ = fs::remove_dir_all(backup);
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct DeterministicFixtureEncoder {
    model: SemanticVisualModelSpec,
}

impl DeterministicFixtureEncoder {
    pub fn new(dimensions: usize) -> Self {
        Self {
            model: SemanticVisualModelSpec {
                id: "deterministic-fixture-encoder".to_string(),
                version: "1".to_string(),
                dimensions,
            },
        }
    }
}

impl LocalSemanticEncoder for DeterministicFixtureEncoder {
    fn model(&self) -> SemanticVisualModelSpec {
        self.model.clone()
    }

    fn encode_image(&self, bytes: &[u8]) -> Result<Vec<f32>, SemanticRuntimeError> {
        fixture_vector(bytes, self.model.dimensions)
    }

    fn encode_text(&self, text: &str) -> Result<Vec<f32>, SemanticRuntimeError> {
        fixture_vector(text.trim().as_bytes(), self.model.dimensions)
    }
}

fn fixture_vector(bytes: &[u8], dimensions: usize) -> Result<Vec<f32>, SemanticRuntimeError> {
    if bytes.is_empty() || dimensions == 0 {
        return Err(SemanticRuntimeError::Encoder(
            "fixture encoder input and dimensions must be non-empty".to_string(),
        ));
    }
    let digest = Sha256::digest(bytes);
    let mut vector = vec![0.0; dimensions];
    for (index, byte) in digest.iter().enumerate() {
        vector[index % dimensions] += *byte as f32 / 255.0;
    }
    Ok(vector)
}

fn validate_manifest(manifest: &SemanticEncoderManifest) -> Result<(), SemanticRuntimeError> {
    if manifest.schema_version != SEMANTIC_ENCODER_MANIFEST_SCHEMA_VERSION
        || !manifest.license_reviewed
        || manifest.license_spdx.trim().is_empty()
        || manifest.model.id.trim().is_empty()
        || manifest.model.version.trim().is_empty()
        || manifest.model.dimensions == 0
        || manifest.file_name.trim().is_empty()
        || manifest.file_name.contains('/')
        || manifest.file_name.contains("..")
        || manifest.sha256.len() != 64
    {
        return Err(SemanticRuntimeError::Manifest(
            "manifest must be versioned, license-reviewed, and contain safe model identity/hash fields"
                .to_string(),
        ));
    }
    Ok(())
}

fn load_manifest(path: &Path) -> Result<SemanticEncoderManifest, SemanticRuntimeError> {
    let manifest: SemanticEncoderManifest = serde_json::from_slice(&fs::read(path)?)
        .map_err(|error| SemanticRuntimeError::Manifest(error.to_string()))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn verify_installed_model(
    manifest: &SemanticEncoderManifest,
    path: &Path,
) -> Result<(), SemanticRuntimeError> {
    if sha256_bytes(&fs::read(path)?) != manifest.sha256 {
        return Err(SemanticRuntimeError::Manifest(
            "installed model hash does not match manifest".to_string(),
        ));
    }
    Ok(())
}

fn semantic_model_store_root() -> PathBuf {
    if let Some(root) = std::env::var_os(SEMANTIC_ENCODER_STORE_ENV) {
        return PathBuf::from(root).join("semantic-visual");
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Library/Application Support/video-creater/models/semantic-visual")
    }
    #[cfg(not(target_os = "macos"))]
    {
        linux_app_data_dir(
            std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
            std::env::var_os("HOME").map(PathBuf::from),
        )
        .join("models/semantic-visual")
    }
}

/// Tauri's `app_data_dir` on Linux: `$XDG_DATA_HOME/<identifier>` or
/// `$HOME/.local/share/<identifier>`.
#[cfg(not(target_os = "macos"))]
fn linux_app_data_dir(xdg_data_home: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
    xdg_data_home
        .filter(|path| path.is_absolute())
        .or_else(|| home.map(|home| home.join(".local/share")))
        .unwrap_or_else(std::env::temp_dir)
        .join("com.olhapi.video-creater")
}

fn frame_key(frame: &SemanticVisualFrameRow) -> (u64, u64, u64, String) {
    (
        frame.time_seconds.to_bits(),
        frame.shot_start_seconds.to_bits(),
        frame.shot_end_seconds.to_bits(),
        frame.thumbnail_relative_path.clone(),
    )
}

fn sha256_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::model::VideoProject;
    use crate::search::{
        query_project_search_for_project_dir_with_semantic_encoder, ProjectSearchQuery, SearchScope,
    };

    fn manifest(bytes: &[u8]) -> SemanticEncoderManifest {
        SemanticEncoderManifest {
            schema_version: 1,
            model: DeterministicFixtureEncoder::new(8).model(),
            file_name: "fixture.bin".to_string(),
            sha256: sha256_bytes(bytes),
            license_spdx: "MIT".to_string(),
            license_reviewed: true,
            source_url: Some("fixture://encoder".to_string()),
        }
    }

    #[test]
    fn acquisition_is_license_hash_gated_and_supports_cancel_retry_progress() {
        let store = tempfile::tempdir().unwrap();
        std::env::set_var(SEMANTIC_ENCODER_STORE_ENV, store.path());
        let bytes = b"approved fixture model".to_vec();
        let mut progress = Vec::new();
        let cancelled = acquire_semantic_encoder_with(
            &manifest(&bytes),
            |_| Ok(bytes.clone()),
            || true,
            |event| progress.push(event),
        );
        assert!(matches!(cancelled, Err(SemanticRuntimeError::Cancelled)));
        let mut invalid = manifest(&bytes);
        invalid.sha256 = "0".repeat(64);
        assert!(
            acquire_semantic_encoder_with(&invalid, |_| Ok(bytes.clone()), || false, |_| {})
                .is_err()
        );
        let path = acquire_semantic_encoder_with(
            &manifest(&bytes),
            |_| Ok(bytes.clone()),
            || false,
            |event| progress.push(event),
        )
        .expect("retry succeeds");
        assert!(path.is_file());
        assert!(progress.iter().any(|event| event.phase == "completed"));
        std::env::remove_var(SEMANTIC_ENCODER_STORE_ENV);
    }

    #[test]
    fn deterministic_encoder_incrementally_indexes_and_text_queries_offline() {
        let project = tempfile::tempdir().unwrap();
        fs::create_dir_all(project.path().join("frames")).unwrap();
        fs::write(project.path().join("frames/a.png"), b"red product").unwrap();
        fs::write(project.path().join("frames/b.png"), b"blue product").unwrap();
        let encoder = DeterministicFixtureEncoder::new(8);
        let expectation = SemanticVisualIndexExpectation {
            media_id: "media-1".to_string(),
            source_fingerprint: "source-v1".to_string(),
            sampling_policy: "shots-v1".to_string(),
            model: encoder.model(),
        };
        let frames = vec![
            SemanticVisualFrameRow {
                time_seconds: 0.5,
                shot_start_seconds: 0.0,
                shot_end_seconds: 1.0,
                thumbnail_relative_path: "frames/a.png".to_string(),
            },
            SemanticVisualFrameRow {
                time_seconds: 1.5,
                shot_start_seconds: 1.0,
                shot_end_seconds: 2.0,
                thumbnail_relative_path: "frames/b.png".to_string(),
            },
        ];
        let first = index_semantic_shots_incrementally(
            project.path(),
            expectation.clone(),
            frames.clone(),
            &encoder,
            || false,
            |_, _| {},
        )
        .unwrap();
        assert_eq!((first.encoded_shots, first.reused_shots), (2, 0));
        let second = index_semantic_shots_incrementally(
            project.path(),
            expectation.clone(),
            frames,
            &encoder,
            || false,
            |_, _| {},
        )
        .unwrap();
        assert_eq!((second.encoded_shots, second.reused_shots), (0, 2));
        let result = query_semantic_visual_with_text(
            project.path(),
            std::slice::from_ref(&expectation),
            "red product",
            &encoder,
            5,
        )
        .unwrap();
        assert_eq!(result.indexed_media_ids, vec!["media-1"]);
        assert!(!result.hits.is_empty());
        let search = query_project_search_for_project_dir_with_semantic_encoder(
            project.path(),
            &VideoProject::new_empty(
                "project-1".to_string(),
                "Semantic fixture".to_string(),
                "2026-07-11T00:00:00Z".to_string(),
            ),
            ProjectSearchQuery {
                query: "red product".to_string(),
                limit: 5,
                scope: SearchScope::Visual,
                media_id: None,
            },
            &[expectation],
            &encoder,
        )
        .unwrap();
        assert_eq!(search["semanticEncoder"]["status"], "installed");
        assert!(!search["groups"]["visual"].as_array().unwrap().is_empty());
    }

    #[test]
    fn production_status_exposes_reviewed_pinned_catalog_before_install() {
        let store = tempfile::tempdir().unwrap();
        std::env::set_var(SEMANTIC_ENCODER_STORE_ENV, store.path());
        std::env::remove_var(SEMANTIC_ENCODER_MANIFEST_ENV);
        let status = configured_semantic_encoder_status();
        assert_eq!(status.status, "notInstalled");
        assert!(status.manifest_configured);
        assert!(status.license_reviewed);
        assert!(!status.hash_verified);
        assert_eq!(status.model.unwrap().dimensions, 768);
        std::env::remove_var(SEMANTIC_ENCODER_STORE_ENV);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn linux_catalog_pins_onnx_revision_license_sizes_hashes_and_distinct_version() {
        let manifest = siglip2_bundle_manifest();
        assert_eq!(manifest, onnx_siglip2_bundle_manifest());
        validate_bundle_manifest(&manifest).expect("reviewed ONNX manifest");
        assert_eq!(manifest.repository, ONNX_SIGLIP2_REPOSITORY);
        assert_eq!(manifest.revision, ONNX_SIGLIP2_REVISION);
        assert_eq!(manifest.license_spdx, "Apache-2.0");
        assert_eq!(manifest.model.id, PALMIER_SIGLIP2_MODEL_ID);
        assert_eq!(manifest.model.dimensions, 768);
        assert_ne!(
            manifest.model.version,
            palmier_siglip2_bundle_manifest().model.version,
            "ONNX and Core ML embeddings must not share semantic indexes"
        );
        assert_eq!(
            manifest
                .artifacts
                .iter()
                .map(|artifact| (
                    artifact.role.as_str(),
                    artifact.file_name.as_str(),
                    artifact.source_path.as_deref()
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    "imageEncoder",
                    "vision_model_fp16.onnx",
                    Some("onnx/vision_model_fp16.onnx")
                ),
                (
                    "textEncoder",
                    "text_model_quantized.onnx",
                    Some("onnx/text_model_quantized.onnx")
                ),
                ("tokenizer", "tokenizer.json", None),
            ]
        );
        assert_eq!(
            manifest
                .artifacts
                .iter()
                .map(|artifact| artifact.bytes)
                .sum::<u64>(),
            503_932_990
        );
        let spec = semantic_bundle_spec(&manifest);
        assert_eq!(spec["imageEncoder"], "vision_model_fp16.onnx");
        assert_eq!(spec["textEncoder"], "text_model_quantized.onnx");
        assert_eq!(spec["tokenizer"], "tokenizer.json");
        assert_eq!(spec["contextLength"], 64);
        // Core ML manifests serialize without the Linux-only sourcePath field.
        let palmier = serde_json::to_value(palmier_siglip2_bundle_manifest()).unwrap();
        assert!(palmier["artifacts"][0].get("sourcePath").is_none());

        let mut unsafe_manifest = manifest.clone();
        unsafe_manifest.artifacts[0].source_path = Some("../vision_model_fp16.onnx".to_string());
        assert!(validate_bundle_manifest(&unsafe_manifest).is_err());
        assert!(validate_bundle_manifest(&palmier_siglip2_bundle_manifest()).is_err());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn linux_binary_and_store_resolution_follow_packaging_and_xdg() {
        let packaged = semantic_encoder_binary_candidates(
            Some(Path::new("/usr/lib/video-creater/video-creater")),
            false,
        );
        assert_eq!(
            packaged,
            vec![PathBuf::from(
                "/usr/lib/video-creater/video-creater-semantic-encoder"
            )]
        );
        let development = semantic_encoder_binary_candidates(
            Some(Path::new(
                "/w/src-tauri/target/debug/deps/video_creater_lib-1",
            )),
            true,
        );
        assert_eq!(
            development[0],
            PathBuf::from("/w/src-tauri/target/debug/video-creater-semantic-encoder")
        );
        assert!(development.contains(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("binaries/video-creater-semantic-encoder-x86_64-unknown-linux-gnu")
        ));
        assert_eq!(
            linux_app_data_dir(Some(PathBuf::from("/data")), Some(PathBuf::from("/home/u")))
                .join("models/semantic-visual"),
            PathBuf::from("/data/com.olhapi.video-creater/models/semantic-visual")
        );
        assert_eq!(
            linux_app_data_dir(None, Some(PathBuf::from("/home/u"))),
            PathBuf::from("/home/u/.local/share/com.olhapi.video-creater")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn production_catalog_pins_revision_license_sizes_and_hashes() {
        let manifest = palmier_siglip2_bundle_manifest();
        validate_bundle_manifest(&manifest).expect("reviewed manifest");
        assert_eq!(manifest.repository, PALMIER_SIGLIP2_REPOSITORY);
        assert_eq!(manifest.revision, PALMIER_SIGLIP2_REVISION);
        assert_eq!(manifest.license_spdx, "Apache-2.0");
        assert!(manifest.license_reviewed);
        assert_eq!(manifest.model.dimensions, 768);
        assert_eq!(manifest.artifacts.len(), 3);
        assert_eq!(
            manifest
                .artifacts
                .iter()
                .map(|artifact| artifact.bytes)
                .sum::<u64>(),
            355_753_654
        );
        assert!(manifest
            .artifacts
            .iter()
            .all(|artifact| artifact.sha256.len() == 64));
    }
}
