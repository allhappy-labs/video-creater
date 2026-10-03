use std::ffi::CString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletedUpload {
    pub upload_id: String,
    pub display_name: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub created_at_unix_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadError {
    InvalidName,
    TooLarge,
    SizeMismatch,
    InsufficientSpace,
    UnsupportedMedia,
    AmbiguousAnimation,
    InvalidId,
    Io,
    Busy,
}

#[derive(Debug, Clone)]
pub struct UploadStore {
    root: PathBuf,
    max_bytes: u64,
    reserve_free_bytes: u64,
    reservations: Arc<Mutex<UploadReservations>>,
}

#[derive(Debug, Default)]
struct UploadReservations {
    in_flight: usize,
    reserved_bytes: u64,
}

impl UploadStore {
    pub fn new(
        root: PathBuf,
        max_bytes: u64,
        reserve_free_bytes: u64,
    ) -> Result<Self, UploadError> {
        fs::create_dir_all(&root).map_err(|_| UploadError::Io)?;
        Ok(Self {
            root,
            max_bytes: max_bytes.max(1),
            reserve_free_bytes,
            reservations: Arc::new(Mutex::new(UploadReservations::default())),
        })
    }

    pub fn stage<I, B>(
        &self,
        display_name: &str,
        declared_size: u64,
        chunks: I,
    ) -> Result<CompletedUpload, UploadError>
    where
        I: IntoIterator<Item = B>,
        B: AsRef<[u8]>,
    {
        let mut writer = self.begin(display_name, declared_size)?;
        for chunk in chunks {
            writer.write_chunk(chunk.as_ref())?;
        }
        writer.finish()
    }

    pub fn begin(
        &self,
        display_name: &str,
        declared_size: u64,
    ) -> Result<UploadWriter, UploadError> {
        validate_name(display_name)?;
        if declared_size > self.max_bytes {
            return Err(UploadError::TooLarge);
        }
        let available = available_space(&self.root).map_err(|_| UploadError::Io)?;
        let mut reservations = self
            .reservations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if reservations.in_flight >= 4 {
            return Err(UploadError::Busy);
        }
        if available
            .saturating_sub(reservations.reserved_bytes)
            .saturating_sub(declared_size)
            < self.reserve_free_bytes
        {
            return Err(UploadError::InsufficientSpace);
        }
        let upload_id = uuid::Uuid::new_v4().simple().to_string();
        let partial = self.root.join(format!("{upload_id}.partial"));
        let file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&partial)
            .map_err(|_| UploadError::Io)?;
        reservations.in_flight += 1;
        reservations.reserved_bytes = reservations.reserved_bytes.saturating_add(declared_size);
        drop(reservations);
        Ok(UploadWriter {
            root: self.root.clone(),
            max_bytes: self.max_bytes,
            declared_size,
            upload_id,
            display_name: display_name.to_owned(),
            partial,
            file: Some(file),
            size: 0,
            digest: Sha256::new(),
            signature: Vec::with_capacity(16),
            committed: false,
            reservations: Arc::clone(&self.reservations),
            reservation_released: false,
        })
    }

    pub fn completed_path(&self, upload_id: &str) -> Result<PathBuf, UploadError> {
        let metadata = self.metadata(upload_id)?;
        let extension = media_extension(&metadata.media_type).ok_or(UploadError::InvalidId)?;
        let path = self.root.join(format!("{upload_id}.{extension}"));
        path.is_file().then_some(path).ok_or(UploadError::InvalidId)
    }

    pub fn metadata(&self, upload_id: &str) -> Result<CompletedUpload, UploadError> {
        validate_id(upload_id)?;
        serde_json::from_slice(
            &fs::read(self.root.join(format!("{upload_id}.json")))
                .map_err(|_| UploadError::InvalidId)?,
        )
        .map_err(|_| UploadError::InvalidId)
    }

    pub fn revoke(&self, upload_id: &str) -> Result<(), UploadError> {
        validate_id(upload_id)?;
        if let Ok(path) = self.completed_path(upload_id) {
            let _ = fs::remove_file(path);
        }
        let _ = fs::remove_file(self.root.join(format!("{upload_id}.json")));
        Ok(())
    }

    /// Removes completed uploads older than `max_age_seconds` and returns the revoked IDs.
    /// Malformed sidecars are left untouched so storage cleanup never guesses which file to delete.
    pub fn cleanup_expired(
        &self,
        now_unix_seconds: u64,
        max_age_seconds: u64,
    ) -> Result<Vec<String>, UploadError> {
        let mut removed = Vec::new();
        let entries = fs::read_dir(&self.root).map_err(|_| UploadError::Io)?;
        for entry in entries {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else { continue };
            let Ok(metadata) = serde_json::from_slice::<CompletedUpload>(&bytes) else {
                continue;
            };
            if now_unix_seconds.saturating_sub(metadata.created_at_unix_seconds) < max_age_seconds {
                continue;
            }
            self.revoke(&metadata.upload_id)?;
            removed.push(metadata.upload_id);
        }
        removed.sort();
        Ok(removed)
    }
}

pub struct UploadWriter {
    root: PathBuf,
    max_bytes: u64,
    declared_size: u64,
    upload_id: String,
    display_name: String,
    partial: PathBuf,
    file: Option<fs::File>,
    size: u64,
    digest: Sha256,
    signature: Vec<u8>,
    committed: bool,
    reservations: Arc<Mutex<UploadReservations>>,
    reservation_released: bool,
}

impl UploadWriter {
    pub fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), UploadError> {
        self.size = self.size.saturating_add(bytes.len() as u64);
        if self.size > self.max_bytes {
            return Err(UploadError::TooLarge);
        }
        if self.size > self.declared_size {
            return Err(UploadError::SizeMismatch);
        }
        if self.signature.len() < 16 {
            self.signature
                .extend_from_slice(&bytes[..bytes.len().min(16 - self.signature.len())]);
        }
        self.digest.update(bytes);
        self.file
            .as_mut()
            .ok_or(UploadError::Io)?
            .write_all(bytes)
            .map_err(|_| UploadError::Io)
    }

    pub fn finish(mut self) -> Result<CompletedUpload, UploadError> {
        if self.size != self.declared_size {
            return Err(UploadError::SizeMismatch);
        }
        let file = self.file.take().ok_or(UploadError::Io)?;
        file.sync_all().map_err(|_| UploadError::Io)?;
        drop(file);
        let media_type = match media_type(&self.signature) {
            Some(media_type) => media_type,
            None if self.size <= 16 * 1024 * 1024 => {
                let bytes = crate::project::import::read_lottie_source_bytes(&self.partial)
                    .map_err(|_| UploadError::UnsupportedMedia)?;
                if bytes.starts_with(b"PK\x03\x04") {
                    crate::project::import::valid_dotlottie_bytes(&bytes).map_err(|message| {
                        if message.contains("multiple animations") {
                            UploadError::AmbiguousAnimation
                        } else {
                            UploadError::UnsupportedMedia
                        }
                    })?;
                    "application/vnd.lottie"
                } else if crate::project::import::valid_lottie_json_bytes(&bytes) {
                    "application/vnd.lottie+json"
                } else {
                    return Err(UploadError::UnsupportedMedia);
                }
            }
            None => return Err(UploadError::UnsupportedMedia),
        };
        let extension = media_extension(media_type).ok_or(UploadError::UnsupportedMedia)?;
        let completed_path = self.root.join(format!("{}.{}", self.upload_id, extension));
        fs::rename(&self.partial, &completed_path).map_err(|_| UploadError::Io)?;
        let completed = CompletedUpload {
            upload_id: self.upload_id.clone(),
            display_name: self.display_name.clone(),
            media_type: media_type.into(),
            size_bytes: self.size,
            sha256: format!("{:x}", self.digest.clone().finalize()),
            created_at_unix_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| UploadError::Io)?
                .as_secs(),
        };
        if persist_metadata(&self.root, &completed).is_err() {
            let _ = fs::remove_file(&completed_path);
            return Err(UploadError::Io);
        }
        self.committed = true;
        self.release_reservation();
        Ok(completed)
    }

    fn release_reservation(&mut self) {
        if self.reservation_released {
            return;
        }
        let mut reservations = self
            .reservations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        reservations.in_flight = reservations.in_flight.saturating_sub(1);
        reservations.reserved_bytes = reservations
            .reserved_bytes
            .saturating_sub(self.declared_size);
        self.reservation_released = true;
    }
}

impl Drop for UploadWriter {
    fn drop(&mut self) {
        self.release_reservation();
        if !self.committed {
            self.file.take();
            let _ = fs::remove_file(&self.partial);
        }
    }
}

fn validate_name(name: &str) -> Result<(), UploadError> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > 255
        || trimmed
            .chars()
            .any(|character| matches!(character, '/' | '\\' | '\0'))
        || matches!(trimmed, "." | "..")
    {
        Err(UploadError::InvalidName)
    } else {
        Ok(())
    }
}

fn validate_id(id: &str) -> Result<(), UploadError> {
    if id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(UploadError::InvalidId)
    }
}

fn media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.get(4..8) == Some(b"ftyp") {
        Some("video/mp4")
    } else if bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        Some("video/webm")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        Some("audio/wav")
    } else {
        None
    }
}

fn media_extension(media_type: &str) -> Option<&'static str> {
    match media_type {
        "video/mp4" => Some("mp4"),
        "video/webm" => Some("webm"),
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "audio/wav" => Some("wav"),
        // Upload metadata already occupies {id}.json; animation data needs a distinct name.
        "application/vnd.lottie+json" => Some("lottie.json"),
        "application/vnd.lottie" => Some("lottie"),
        _ => None,
    }
}

fn persist_metadata(root: &Path, upload: &CompletedUpload) -> Result<(), UploadError> {
    let target = root.join(format!("{}.json", upload.upload_id));
    let partial = root.join(format!("{}.json.partial", upload.upload_id));
    fs::write(
        &partial,
        serde_json::to_vec(upload).map_err(|_| UploadError::Io)?,
    )
    .map_err(|_| UploadError::Io)?;
    fs::rename(partial, target).map_err(|_| UploadError::Io)
}

#[cfg(unix)]
fn available_space(path: &Path) -> io::Result<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid path"))?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `path` is a valid NUL-terminated C string and `stats` points to writable memory.
    if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: statvfs returned success and initialized the structure.
    let stats = unsafe { stats.assume_init() };
    Ok(stats.f_bavail.saturating_mul(stats.f_frsize))
}

#[cfg(not(unix))]
fn available_space(_path: &Path) -> io::Result<u64> {
    Ok(u64::MAX)
}
