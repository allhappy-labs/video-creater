use super::model::{ModelArtifactProviderKind, ModelArtifactSource};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelAcquisitionError {
    #[error("unsupported artifact source provider {0:?}")]
    UnsupportedProvider(ModelArtifactProviderKind),
    #[error("download cancelled")]
    Cancelled,
    #[error("source repo mismatch: expected {expected}, found {found}")]
    SourceRepoMismatch { expected: String, found: String },
    #[error("source revision mismatch: expected {expected}, found {found}")]
    SourceRevisionMismatch { expected: String, found: String },
    #[error("source file missing: {0}")]
    SourceFileMissing(String),
    #[error("failed to inspect source: {0}")]
    Inspect(String),
    #[error("failed to download source file: {0}")]
    Download(String),
    #[error("download progress failed: {0}")]
    Progress(String),
    #[error("insufficient storage while downloading model: {0}")]
    StorageInsufficient(String),
}

fn acquisition_io_error(error: std::io::Error) -> ModelAcquisitionError {
    if error.kind() == std::io::ErrorKind::StorageFull
        || error
            .raw_os_error()
            .is_some_and(|code| code == 28 || code == 122)
    {
        ModelAcquisitionError::StorageInsufficient(error.to_string())
    } else {
        ModelAcquisitionError::Download(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteModelArtifact {
    pub repo_id: String,
    pub revision: String,
    pub file_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedModelFile {
    pub relative_path: String,
    pub absolute_path: PathBuf,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedModelArtifact {
    pub source: ModelArtifactSource,
    pub files: Vec<DownloadedModelFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDownloadProgressSnapshot {
    pub downloaded_files: u32,
    pub total_files: u32,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

pub trait ModelDownloadProgress: Send + Sync {
    fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError>;
    fn file_completed(&self, downloaded_files: u32) -> Result<(), ModelAcquisitionError>;
    fn progress_updated(
        &self,
        _snapshot: ModelDownloadProgressSnapshot,
    ) -> Result<(), ModelAcquisitionError> {
        Ok(())
    }
}

pub trait ModelArtifactProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;
    fn supports(&self, source: &ModelArtifactSource) -> bool;
    fn inspect_remote(
        &self,
        source: &ModelArtifactSource,
    ) -> Result<RemoteModelArtifact, ModelAcquisitionError>;
    fn download(
        &self,
        source: &ModelArtifactSource,
        target_dir: &Path,
        progress: &dyn ModelDownloadProgress,
    ) -> Result<DownloadedModelArtifact, ModelAcquisitionError>;
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct HuggingFaceModelResponse {
    id: String,
    sha: String,
    siblings: Vec<HuggingFaceSibling>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HuggingFaceSibling {
    rfilename: String,
}

pub fn parse_hugging_face_model_response(
    value: &serde_json::Value,
) -> Result<RemoteModelArtifact, ModelAcquisitionError> {
    let response: HuggingFaceModelResponse = serde_json::from_value(value.clone())
        .map_err(|error| ModelAcquisitionError::Inspect(error.to_string()))?;
    Ok(RemoteModelArtifact {
        repo_id: response.id,
        revision: response.sha,
        file_paths: response
            .siblings
            .into_iter()
            .map(|sibling| sibling.rfilename)
            .collect(),
    })
}

pub struct HuggingFaceHubProvider {
    #[allow(dead_code)]
    client: reqwest::blocking::Client,
    #[allow(dead_code)]
    api_base_url: String,
    resolve_base_url: String,
}

impl HuggingFaceHubProvider {
    pub fn new(client: reqwest::blocking::Client) -> Self {
        Self::with_base_urls(
            client,
            "https://huggingface.co/api/models",
            "https://huggingface.co",
        )
    }

    pub fn with_base_urls(
        client: reqwest::blocking::Client,
        api_base_url: impl Into<String>,
        resolve_base_url: impl Into<String>,
    ) -> Self {
        Self {
            client,
            api_base_url: api_base_url.into().trim_end_matches('/').to_string(),
            resolve_base_url: resolve_base_url.into().trim_end_matches('/').to_string(),
        }
    }

    pub fn resolve_url(
        &self,
        source: &ModelArtifactSource,
        file_path: &str,
    ) -> Result<String, ModelAcquisitionError> {
        hugging_face_resolve_url_with_base_url(&self.resolve_base_url, source, file_path)
    }

    fn hugging_face_revision_api_url(&self, source: &ModelArtifactSource) -> String {
        format!(
            "{}/{}/revision/{}",
            self.api_base_url, source.repo_id, source.revision
        )
    }
}

impl ModelArtifactProvider for HuggingFaceHubProvider {
    fn provider_id(&self) -> &'static str {
        "hugging_face_hub"
    }

    fn supports(&self, source: &ModelArtifactSource) -> bool {
        source.provider == ModelArtifactProviderKind::HuggingFaceHub
    }

    fn inspect_remote(
        &self,
        source: &ModelArtifactSource,
    ) -> Result<RemoteModelArtifact, ModelAcquisitionError> {
        if !self.supports(source) {
            return Err(ModelAcquisitionError::UnsupportedProvider(source.provider));
        }
        let url = self.hugging_face_revision_api_url(source);
        let value = self
            .client
            .get(&url)
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|error| ModelAcquisitionError::Inspect(error.to_string()))?
            .text()
            .map_err(|error| ModelAcquisitionError::Inspect(error.to_string()))?;
        let value = serde_json::from_str::<serde_json::Value>(&value)
            .map_err(|error| ModelAcquisitionError::Inspect(error.to_string()))?;
        let remote = parse_hugging_face_model_response(&value)?;
        validate_remote_required_files(source, &remote)?;
        Ok(remote)
    }

    fn download(
        &self,
        source: &ModelArtifactSource,
        target_dir: &Path,
        progress: &dyn ModelDownloadProgress,
    ) -> Result<DownloadedModelArtifact, ModelAcquisitionError> {
        if !self.supports(source) {
            return Err(ModelAcquisitionError::UnsupportedProvider(source.provider));
        }

        fs::create_dir_all(target_dir).map_err(acquisition_io_error)?;
        let canonical_target_dir = target_dir.canonicalize().map_err(acquisition_io_error)?;
        let include_files = source
            .include_files
            .iter()
            .map(|relative_path| sanitized_artifact_file_path(relative_path))
            .collect::<Result<Vec<_>, _>>()?;
        let mut files = Vec::with_capacity(include_files.len());
        let uses_http = resolve_base_url_is_http(&self.resolve_base_url);
        let total_files = include_files.len() as u32;
        let mut downloaded_bytes = 0_u64;
        let mut total_bytes = 0_u64;
        for (index, relative_path) in include_files.iter().enumerate() {
            if progress.is_cancelled()? {
                return Err(ModelAcquisitionError::Cancelled);
            }

            let target_path =
                checked_target_path(&canonical_target_dir, &relative_path.relative_path)?;
            let part_path = target_path.with_extension(format!(
                "{}part",
                target_path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .map(|extension| format!("{extension}."))
                    .unwrap_or_default()
            ));

            if uses_http {
                let url = self.resolve_url(source, &relative_path.remote_path)?;
                download_http_file_to_part(
                    &self.client,
                    &url,
                    &part_path,
                    HttpDownloadProgress {
                        observer: progress,
                        downloaded_files: index as u32,
                        total_files,
                        downloaded_bytes: &mut downloaded_bytes,
                        total_bytes: &mut total_bytes,
                    },
                )?;
            } else {
                let source_path = local_source_file_path(
                    &self.resolve_base_url,
                    source,
                    &relative_path.remote_path,
                )?;
                let source_size = match fs::metadata(&source_path) {
                    Ok(metadata) => metadata.len(),
                    Err(error) => {
                        remove_part_file(&part_path);
                        return Err(acquisition_io_error(error));
                    }
                };
                total_bytes = total_bytes.saturating_add(source_size);
                copy_local_file_to_part(
                    &source_path,
                    &part_path,
                    progress,
                    index as u32,
                    total_files,
                    &mut downloaded_bytes,
                    total_bytes,
                )?;
            }

            match progress.is_cancelled() {
                Ok(true) => {
                    remove_part_file(&part_path);
                    return Err(ModelAcquisitionError::Cancelled);
                }
                Ok(false) => {}
                Err(error) => {
                    remove_part_file(&part_path);
                    return Err(error);
                }
            }

            if let Err(error) = fs::rename(&part_path, &target_path) {
                remove_part_file(&part_path);
                return Err(acquisition_io_error(error));
            }
            let metadata = fs::metadata(&target_path).map_err(acquisition_io_error)?;
            let sha256 = sha256_file(&target_path)?;
            files.push(DownloadedModelFile {
                relative_path: relative_path.remote_path.clone(),
                absolute_path: target_path,
                size_bytes: metadata.len(),
                sha256,
            });
            total_bytes = total_bytes.max(downloaded_bytes);
            progress.progress_updated(ModelDownloadProgressSnapshot {
                downloaded_files: (index + 1) as u32,
                total_files,
                downloaded_bytes,
                total_bytes,
            })?;
            progress.file_completed((index + 1) as u32)?;
        }

        Ok(DownloadedModelArtifact {
            source: source.clone(),
            files,
        })
    }
}

pub fn hugging_face_resolve_url(
    source: &ModelArtifactSource,
    file_path: &str,
) -> Result<String, ModelAcquisitionError> {
    hugging_face_resolve_url_with_base_url("https://huggingface.co", source, file_path)
}

pub fn hugging_face_resolve_url_with_base_url(
    resolve_base_url: &str,
    source: &ModelArtifactSource,
    file_path: &str,
) -> Result<String, ModelAcquisitionError> {
    Ok(format!(
        "{}/{}/resolve/{}/{}",
        resolve_base_url.trim_end_matches('/'),
        source.repo_id,
        source.revision,
        remote_model_file_path(source, file_path)?
    ))
}

#[derive(Debug, Clone)]
struct SanitizedArtifactPath {
    relative_path: PathBuf,
    remote_path: String,
}

fn sanitized_artifact_file_path(
    file_path: &str,
) -> Result<SanitizedArtifactPath, ModelAcquisitionError> {
    let components = sanitize_artifact_relative_components(file_path, "artifact include file")?;
    Ok(SanitizedArtifactPath {
        relative_path: components.iter().collect(),
        remote_path: components.join("/"),
    })
}

fn remote_model_file_path(
    source: &ModelArtifactSource,
    file_path: &str,
) -> Result<String, ModelAcquisitionError> {
    let mut components = match source.path_prefix.as_deref() {
        Some(path_prefix) => {
            sanitize_artifact_relative_components(path_prefix, "artifact path prefix")?
        }
        None => Vec::new(),
    };
    components.extend(sanitize_artifact_relative_components(
        file_path,
        "artifact include file",
    )?);
    Ok(components.join("/"))
}

fn sanitize_artifact_relative_components(
    path: &str,
    label: &str,
) -> Result<Vec<String>, ModelAcquisitionError> {
    if path.is_empty() {
        return Err(invalid_artifact_path(label, path, "path is empty"));
    }
    if path.contains('\\') {
        return Err(invalid_artifact_path(
            label,
            path,
            "backslash path separators are not supported",
        ));
    }

    if Path::new(path).is_absolute() {
        return Err(invalid_artifact_path(label, path, "path must be relative"));
    }

    let mut components = Vec::new();
    for component in path.split('/') {
        if component.is_empty() {
            return Err(invalid_artifact_path(label, path, "path is empty"));
        }
        if component == "." {
            return Err(invalid_artifact_path(
                label,
                path,
                "path must not contain '.' components",
            ));
        }
        if component == ".." {
            return Err(invalid_artifact_path(
                label,
                path,
                "path must not contain '..' components",
            ));
        }
        components.push(component.to_string());
    }

    if components.is_empty() {
        return Err(invalid_artifact_path(label, path, "path is empty"));
    }
    if components
        .first()
        .is_some_and(|component| looks_like_windows_drive_prefix(component))
    {
        return Err(invalid_artifact_path(label, path, "path must be relative"));
    }

    Ok(components)
}

fn looks_like_windows_drive_prefix(component: &str) -> bool {
    let bytes = component.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn invalid_artifact_path(label: &str, path: &str, reason: &str) -> ModelAcquisitionError {
    ModelAcquisitionError::Download(format!("invalid {label} path {path:?}: {reason}"))
}

fn resolve_base_url_is_http(resolve_base_url: &str) -> bool {
    resolve_base_url.starts_with("http://") || resolve_base_url.starts_with("https://")
}

fn local_source_file_path(
    resolve_base_url: &str,
    source: &ModelArtifactSource,
    file_path: &str,
) -> Result<PathBuf, ModelAcquisitionError> {
    Ok(Path::new(resolve_base_url).join(remote_model_file_path(source, file_path)?))
}

fn copy_local_file_to_part(
    source_path: &Path,
    part_path: &Path,
    progress: &dyn ModelDownloadProgress,
    downloaded_files: u32,
    total_files: u32,
    downloaded_bytes: &mut u64,
    total_bytes: u64,
) -> Result<(), ModelAcquisitionError> {
    let result = copy_local_file_to_part_inner(
        source_path,
        part_path,
        progress,
        downloaded_files,
        total_files,
        downloaded_bytes,
        total_bytes,
    );
    if result.is_err() {
        remove_part_file(part_path);
    }
    result
}

fn copy_local_file_to_part_inner(
    source_path: &Path,
    part_path: &Path,
    progress: &dyn ModelDownloadProgress,
    downloaded_files: u32,
    total_files: u32,
    downloaded_bytes: &mut u64,
    total_bytes: u64,
) -> Result<(), ModelAcquisitionError> {
    let mut part_file = create_part_file(part_path)?;
    let mut source_file = fs::File::open(source_path).map_err(acquisition_io_error)?;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if progress.is_cancelled()? {
            return Err(ModelAcquisitionError::Cancelled);
        }
        let read = source_file
            .read(&mut buffer)
            .map_err(acquisition_io_error)?;
        if read == 0 {
            break;
        }
        part_file
            .write_all(&buffer[..read])
            .map_err(acquisition_io_error)?;
        *downloaded_bytes = downloaded_bytes.saturating_add(read as u64);
        progress.progress_updated(ModelDownloadProgressSnapshot {
            downloaded_files,
            total_files,
            downloaded_bytes: *downloaded_bytes,
            total_bytes,
        })?;
    }
    part_file.flush().map_err(acquisition_io_error)
}

struct HttpDownloadProgress<'a> {
    observer: &'a dyn ModelDownloadProgress,
    downloaded_files: u32,
    total_files: u32,
    downloaded_bytes: &'a mut u64,
    total_bytes: &'a mut u64,
}

fn download_http_file_to_part(
    client: &reqwest::blocking::Client,
    url: &str,
    part_path: &Path,
    progress: HttpDownloadProgress<'_>,
) -> Result<(), ModelAcquisitionError> {
    let part_path_for_cleanup = part_path.to_path_buf();
    let result = download_http_file_to_part_inner(client, url, part_path, progress);
    if result.is_err() {
        remove_part_file(&part_path_for_cleanup);
    }
    result
}

fn download_http_file_to_part_inner(
    client: &reqwest::blocking::Client,
    url: &str,
    part_path: &Path,
    progress: HttpDownloadProgress<'_>,
) -> Result<(), ModelAcquisitionError> {
    let HttpDownloadProgress {
        observer,
        downloaded_files,
        total_files,
        downloaded_bytes,
        total_bytes,
    } = progress;
    let mut part_file = create_part_file(part_path)?;
    let mut response = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| ModelAcquisitionError::Download(error.to_string()))?;
    *total_bytes = total_bytes.saturating_add(response.content_length().unwrap_or(0));
    observer.progress_updated(ModelDownloadProgressSnapshot {
        downloaded_files,
        total_files,
        downloaded_bytes: *downloaded_bytes,
        total_bytes: *total_bytes,
    })?;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if observer.is_cancelled()? {
            return Err(ModelAcquisitionError::Cancelled);
        }
        let read = response.read(&mut buffer).map_err(acquisition_io_error)?;
        if read == 0 {
            break;
        }
        part_file
            .write_all(&buffer[..read])
            .map_err(acquisition_io_error)?;
        *downloaded_bytes = downloaded_bytes.saturating_add(read as u64);
        observer.progress_updated(ModelDownloadProgressSnapshot {
            downloaded_files,
            total_files,
            downloaded_bytes: *downloaded_bytes,
            total_bytes: *total_bytes,
        })?;
    }
    part_file.flush().map_err(acquisition_io_error)
}

fn create_part_file(part_path: &Path) -> Result<fs::File, ModelAcquisitionError> {
    match fs::remove_file(part_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(acquisition_io_error(error)),
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(part_path)
        .map_err(acquisition_io_error)
}

fn checked_target_path(
    canonical_target_dir: &Path,
    relative_path: &Path,
) -> Result<PathBuf, ModelAcquisitionError> {
    let target_path = canonical_target_dir.join(relative_path);
    if !target_path.starts_with(canonical_target_dir) {
        return Err(ModelAcquisitionError::Download(format!(
            "target path escaped model directory: {}",
            relative_path.display()
        )));
    }

    let parent = target_path.parent().ok_or_else(|| {
        ModelAcquisitionError::Download(format!(
            "target path has no parent: {}",
            target_path.display()
        ))
    })?;
    create_contained_parent_dirs(canonical_target_dir, parent, relative_path)?;

    Ok(target_path)
}

fn create_contained_parent_dirs(
    canonical_target_dir: &Path,
    parent: &Path,
    relative_path: &Path,
) -> Result<(), ModelAcquisitionError> {
    let relative_parent = parent.strip_prefix(canonical_target_dir).map_err(|_| {
        ModelAcquisitionError::Download(format!(
            "target path escaped model directory: {}",
            relative_path.display()
        ))
    })?;
    let mut current = canonical_target_dir.to_path_buf();
    for component in relative_parent.components() {
        match component {
            std::path::Component::Normal(component) => current.push(component),
            std::path::Component::CurDir => continue,
            _ => {
                return Err(ModelAcquisitionError::Download(format!(
                    "target path escaped model directory: {}",
                    relative_path.display()
                )));
            }
        }

        match fs::symlink_metadata(&current) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&current)
                .map_err(|error| ModelAcquisitionError::Download(error.to_string()))?,
            Err(error) => return Err(ModelAcquisitionError::Download(error.to_string())),
        }

        let canonical_current = current
            .canonicalize()
            .map_err(|error| ModelAcquisitionError::Download(error.to_string()))?;
        if !canonical_current.starts_with(canonical_target_dir) {
            return Err(ModelAcquisitionError::Download(format!(
                "target path escaped model directory: {}",
                relative_path.display()
            )));
        }
        if !canonical_current.is_dir() {
            return Err(ModelAcquisitionError::Download(format!(
                "target parent is not a directory: {}",
                current.display()
            )));
        }
    }

    Ok(())
}

fn remove_part_file(part_path: &Path) {
    let _ = fs::remove_file(part_path);
}

fn sha256_file(path: &Path) -> Result<String, ModelAcquisitionError> {
    let mut file =
        fs::File::open(path).map_err(|error| ModelAcquisitionError::Download(error.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| ModelAcquisitionError::Download(error.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn validate_remote_required_files(
    source: &ModelArtifactSource,
    remote: &RemoteModelArtifact,
) -> Result<(), ModelAcquisitionError> {
    if remote.repo_id != source.repo_id {
        return Err(ModelAcquisitionError::SourceRepoMismatch {
            expected: source.repo_id.clone(),
            found: remote.repo_id.clone(),
        });
    }
    if remote.revision != source.revision {
        return Err(ModelAcquisitionError::SourceRevisionMismatch {
            expected: source.revision.clone(),
            found: remote.revision.clone(),
        });
    }
    let mut missing_files = Vec::new();
    for file in &source.include_files {
        let remote_file = remote_model_file_path(source, file)?;
        if !remote
            .file_paths
            .iter()
            .any(|file_path| file_path == &remote_file)
        {
            missing_files.push(file.clone());
        }
    }
    if !missing_files.is_empty() {
        return Err(ModelAcquisitionError::SourceFileMissing(
            missing_files.join(", "),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        acquisition_io_error, HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider,
        ModelDownloadProgress, ModelDownloadProgressSnapshot,
    };
    use crate::transcription::model::{ModelArtifactProviderKind, ModelArtifactSource};
    use std::io::{ErrorKind, Read, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;
    use std::time::Duration;

    struct RecordingProgress {
        snapshots: Mutex<Vec<ModelDownloadProgressSnapshot>>,
    }

    #[test]
    fn storage_full_io_errors_keep_a_typed_model_failure() {
        let error = acquisition_io_error(std::io::Error::from_raw_os_error(28));

        assert!(matches!(
            error,
            ModelAcquisitionError::StorageInsufficient(_)
        ));
    }

    impl ModelDownloadProgress for RecordingProgress {
        fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
            Ok(false)
        }

        fn file_completed(&self, _downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
            Ok(())
        }

        fn progress_updated(
            &self,
            snapshot: ModelDownloadProgressSnapshot,
        ) -> Result<(), ModelAcquisitionError> {
            self.snapshots.lock().expect("snapshot lock").push(snapshot);
            Ok(())
        }
    }

    #[test]
    fn http_download_reports_monotonic_bytes_and_final_file_equality() {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == ErrorKind::PermissionDenied => return,
            Err(error) => panic!("bind model fixture server: {error}"),
        };
        let address = listener.local_addr().expect("fixture address");
        let body = (0..150_000)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        let served_body = body.clone();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept model request");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("read timeout");
            let mut request = Vec::new();
            let mut chunk = [0_u8; 512];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut chunk).expect("read request");
                assert_ne!(read, 0, "request ended before headers");
                request.extend_from_slice(&chunk[..read]);
            }
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                served_body.len()
            );
            stream.write_all(headers.as_bytes()).expect("write headers");
            for chunk in served_body.chunks(32 * 1024) {
                stream.write_all(chunk).expect("write body chunk");
                stream.flush().expect("flush body chunk");
            }
        });
        let source = ModelArtifactSource {
            provider: ModelArtifactProviderKind::HuggingFaceHub,
            repo_id: "org/model".to_string(),
            revision: "fixture".to_string(),
            path_prefix: None,
            include_files: vec!["model.bin".to_string()],
            license: Some("apache-2.0".to_string()),
        };
        let provider = HuggingFaceHubProvider::with_base_urls(
            reqwest::blocking::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("fixture client"),
            "http://127.0.0.1/unused",
            format!("http://{address}"),
        );
        let target = tempfile::tempdir().expect("target");
        let progress = RecordingProgress {
            snapshots: Mutex::new(Vec::new()),
        };

        let artifact = provider
            .download(&source, target.path(), &progress)
            .expect("download fixture");
        server.join().expect("fixture server");

        let snapshots = progress.snapshots.lock().expect("snapshot lock");
        assert!(snapshots.len() >= 3);
        assert!(snapshots
            .windows(2)
            .all(|pair| pair[0].downloaded_bytes <= pair[1].downloaded_bytes));
        let final_snapshot = snapshots.last().expect("final snapshot");
        assert_eq!(final_snapshot.downloaded_files, final_snapshot.total_files);
        assert_eq!(final_snapshot.downloaded_bytes, body.len() as u64);
        assert_eq!(final_snapshot.total_bytes, body.len() as u64);
        assert_eq!(
            std::fs::read(&artifact.files[0].absolute_path).expect("downloaded file"),
            body
        );
    }
}
